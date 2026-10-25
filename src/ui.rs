use fontdue::{Font, FontSettings};
use mlua::{Lua, Table};
use std::fs;
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, Rect,
    SpreadMode, Transform,
};

use crate::library::AlbumArt;

pub struct NowPlaying<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub elapsed: f32,
    pub duration: f32,
}

pub struct UiFrame<'a> {
    pub items: &'a [String],
    pub selected_index: usize,
    pub title: &'a str,
    pub clock: &'a str,
    pub battery: f32,
    pub bluetooth: bool,
    pub headphones: bool,
    pub now: Option<NowPlaying<'a>>,
    pub now_art: Option<&'a AlbumArt>,
    pub pane_art: Option<&'a AlbumArt>,
    pub pane_art_alpha: f32,
    pub list_scroll: Option<(f32, f32)>,
    pub now_scroll: Option<(f32, f32)>,
    pub is_reversing: bool,
    pub scroll_offset: usize,
}

pub struct LayoutConfig {
    pub font_size_list: f32,
    pub list_item_max_width: f32,
    pub font_size_nowplay: f32,
    pub nowplay_max_width: f32,
    pub list_scroll_gap: f32,
    pub nowplay_scroll_gap: f32,
    pub visible_rows: usize,
}

pub struct UiEngine {
    lua: Lua,
    pixmap: Pixmap,
    font: Font,
    line_ratio: f32,
    pub layout: LayoutConfig,
    buffer: Vec<u32>,
    width: usize,
    height: usize,
}

impl UiEngine {
    pub fn new(width: usize, height: usize) -> Self {
        let pixmap = Pixmap::new(width as u32, height as u32).unwrap();
        let buffer = vec![0; width * height];

        let font_data = fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
            .expect("Could not find font file");
        let font =
            Font::from_bytes(font_data, FontSettings::default()).expect("Failed to parse font");

        let line_ratio = font
            .horizontal_line_metrics(16.0)
            .map(|m| (m.ascent - m.descent) / 16.0)
            .unwrap_or(1.16);

        let lua = Lua::new();
        let script = fs::read_to_string("ui.lua").expect("Could not find ui.lua");
        lua.load(&script).exec().expect("Failed to run Lua script");

        let config_fn: mlua::Function = lua.globals().get("get_layout_config").unwrap();
        let config_table: mlua::Table = config_fn.call(()).unwrap();

        let layout = LayoutConfig {
            font_size_list: config_table.get("ITEM_FONT").unwrap(),
            list_item_max_width: config_table.get("LIST_ITEM_MAX_WIDTH").unwrap(),
            font_size_nowplay: config_table.get("NOWPLAY_TITLE_FONT").unwrap(),
            nowplay_max_width: config_table.get("NOWPLAY_MAX_WIDTH").unwrap(),
            list_scroll_gap: config_table.get("LIST_SCROLL_GAP").unwrap(),
            nowplay_scroll_gap: config_table.get("NOWPLAY_SCROLL_GAP").unwrap(),
            visible_rows: config_table.get("VISIBLE_ROWS").unwrap(),
        };

        Self {
            lua,
            pixmap,
            font,
            line_ratio,
            layout,
            buffer,
            width,
            height,
        }
    }

    pub fn draw(&mut self, frame: UiFrame) -> &[u32] {
        self.pixmap.fill(Color::from_rgba8(0, 0, 0, 255));

        // Copy the borrowed-art handles out before we borrow `frame` for the
        // state table — `Option<&AlbumArt>` and `f32` are both Copy.
        let pane_art = frame.pane_art;
        let pane_alpha = frame.pane_art_alpha;
        let now_art = frame.now_art;

        let state = self.build_state_table(&frame);

        let draw_fn: mlua::Function = self.lua.globals().get("draw_screen").unwrap();
        let draw_cmds: Table = draw_fn.call(state).unwrap();

        for cmd in draw_cmds.sequence_values::<Table>() {
            let cmd = cmd.unwrap();
            let cmd_type: String = cmd.get("type").unwrap();
            match cmd_type.as_str() {
                "rect" => self.draw_rect(&cmd),
                "gradient_rect" => self.draw_gradient_rect(&cmd),
                "text" => self.draw_text(&cmd),
                "album_art" => self.draw_album_art(&cmd, pane_art, pane_alpha),
                "now_art_thumb" => self.draw_album_art(&cmd, now_art, 255.0),
                _ => {}
            }
        }

        self.pixmap_to_buffer();
        &self.buffer
    }

    fn build_state_table(&self, frame: &UiFrame) -> Table {
        let lua_items = self.lua.create_table().unwrap();
        for (i, item) in frame.items.iter().enumerate() {
            lua_items.set(i + 1, item.clone()).unwrap();
        }

        let state = self.lua.create_table().unwrap();
        state.set("items", lua_items).unwrap();
        state
            .set("selected_index", frame.selected_index + 1)
            .unwrap();
        state
            .set("line_height_ratio", self.line_ratio as f64)
            .unwrap();
        state.set("title", frame.title).unwrap();
        state.set("clock", frame.clock).unwrap();
        state.set("battery", frame.battery).unwrap();
        state.set("bluetooth", frame.bluetooth).unwrap();
        state.set("headphones", frame.headphones).unwrap();
        state.set("is_reversing", frame.is_reversing).unwrap();
        state.set("scroll_offset", frame.scroll_offset).unwrap();
        state.set("has_pane_art", frame.pane_art.is_some()).unwrap();
        state.set("has_now_art", frame.now_art.is_some()).unwrap();
        state.set("pane_art_alpha", frame.pane_art_alpha).unwrap();

        if let Some(np) = &frame.now {
            let table = self.lua.create_table().unwrap();
            table.set("title", np.title).unwrap();
            table.set("artist", np.artist).unwrap();
            table.set("elapsed", np.elapsed).unwrap();
            table.set("duration", np.duration).unwrap();
            state.set("now", table).unwrap();
        }

        if let Some((offset, width)) = frame.list_scroll {
            let t = self.lua.create_table().unwrap();
            t.set(1, offset).unwrap();
            t.set(2, width).unwrap();
            state.set("list_scroll", t).unwrap();
        }

        if let Some((offset, width)) = frame.now_scroll {
            let t = self.lua.create_table().unwrap();
            t.set(1, offset).unwrap();
            t.set(2, width).unwrap();
            state.set("now_scroll", t).unwrap();
        }

        state
    }

    fn pixmap_to_buffer(&mut self) {
        for (i, pixel) in self.pixmap.pixels().iter().enumerate() {
            let r = pixel.red() as u32;
            let g = pixel.green() as u32;
            let b = pixel.blue() as u32;
            self.buffer[i] = (r << 16) | (g << 8) | b;
        }
    }

    // --- Primitive drawing ----------------------------------------------

    fn draw_rect(&mut self, cmd: &Table) {
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let w: f32 = cmd.get("width").unwrap();
        let h: f32 = cmd.get("height").unwrap();
        let color_hex: u32 = cmd.get("color").unwrap();

        let color = Color::from_rgba8(
            ((color_hex >> 16) & 0xFF) as u8,
            ((color_hex >> 8) & 0xFF) as u8,
            (color_hex & 0xFF) as u8,
            ((color_hex >> 24) & 0xFF) as u8,
        );

        let mut paint = Paint::default();
        paint.set_color(color);
        let rect = Rect::from_xywh(x, y, w, h).unwrap();
        let path = PathBuilder::from_rect(rect);
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    fn draw_gradient_rect(&mut self, cmd: &Table) {
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let w: f32 = cmd.get("width").unwrap();
        let h: f32 = cmd.get("height").unwrap();
        let color_start_hex: u32 = cmd.get("color_start").unwrap();
        let color_end_hex: u32 = cmd.get("color_end").unwrap();

        let c_start = Color::from_rgba8(
            ((color_start_hex >> 16) & 0xFF) as u8,
            ((color_start_hex >> 8) & 0xFF) as u8,
            (color_start_hex & 0xFF) as u8,
            ((color_start_hex >> 24) & 0xFF) as u8,
        );
        let c_end = Color::from_rgba8(
            ((color_end_hex >> 16) & 0xFF) as u8,
            ((color_end_hex >> 8) & 0xFF) as u8,
            (color_end_hex & 0xFF) as u8,
            ((color_end_hex >> 24) & 0xFF) as u8,
        );

        let shader = LinearGradient::new(
            Point::from_xy(x, y),
            Point::from_xy(x, y + h),
            vec![
                GradientStop::new(0.0, c_start),
                GradientStop::new(1.0, c_end),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap();

        let mut paint = Paint::default();
        paint.shader = shader;

        let rect = Rect::from_xywh(x, y, w, h).unwrap();
        let path = PathBuilder::from_rect(rect);
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    fn draw_text(&mut self, cmd: &Table) {
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let text: String = cmd.get("text").unwrap();
        let color_hex: u32 = cmd.get("color").unwrap();
        let size: f32 = cmd.get("size").unwrap();

        let clip_left: Option<f32> = cmd.get("clip_left").ok();
        let clip_right: Option<f32> = cmd.get("clip_right").ok();

        let r = ((color_hex >> 16) & 0xFF) as u8;
        let g = ((color_hex >> 8) & 0xFF) as u8;
        let b = (color_hex & 0xFF) as u8;

        let ascent = self
            .font
            .horizontal_line_metrics(size)
            .map(|m| m.ascent)
            .unwrap_or(size);
        let baseline = (y + ascent).round() as i32;

        let left_limit = clip_left.map(|l| l.round() as i32).unwrap_or(0);
        let right_limit = clip_right
            .map(|r| r.round() as i32)
            .unwrap_or(self.width as i32);

        let mut pen_x = x;
        for c in text.chars() {
            if pen_x >= right_limit as f32 {
                break;
            }

            let (metrics, bitmap) = self.font.rasterize(c, size);
            let top = baseline - metrics.ymin - metrics.height as i32;
            let left = pen_x.round() as i32 + metrics.xmin;

            for row in 0..metrics.height {
                let py = top + row as i32;
                if py < 0 || py >= self.height as i32 {
                    continue;
                }
                for col in 0..metrics.width {
                    let px = left + col as i32;
                    if px < left_limit || px >= right_limit {
                        continue;
                    }

                    let alpha = bitmap[row * metrics.width + col];
                    if alpha == 0 {
                        continue;
                    }

                    let idx = py as usize * self.width + px as usize;
                    let a = alpha as f32 / 255.0;
                    let dst = &mut self.pixmap.pixels_mut()[idx];

                    let fr = (r as f32 * a + dst.red() as f32 * (1.0 - a)) as u8;
                    let fg = (g as f32 * a + dst.green() as f32 * (1.0 - a)) as u8;
                    let fb = (b as f32 * a + dst.blue() as f32 * (1.0 - a)) as u8;

                    *dst = tiny_skia::PremultipliedColorU8::from_rgba(fr, fg, fb, 255).unwrap();
                }
            }
            pen_x += metrics.advance_width;
        }
    }

    /// Bilinear-sampled, cover-cropped blit of `art` into the rect described
    /// by `cmd`. `base_alpha` (0..=255) lets the caller fade the whole image;
    /// the command's optional `alpha` field (0..=255, default 255) multiplies
    /// on top so Lua can still dim individual placements.
    fn draw_album_art(&mut self, cmd: &Table, art: Option<&AlbumArt>, base_alpha: f32) {
        let Some(art) = art else { return };
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let w: f32 = cmd.get("width").unwrap();
        let h: f32 = cmd.get("height").unwrap();
        let cmd_alpha: f32 = cmd.get("alpha").ok().unwrap_or(255.0);

        if w <= 0.0 || h <= 0.0 || art.width == 0 || art.height == 0 {
            return;
        }
        let alpha = (base_alpha.clamp(0.0, 255.0) * cmd_alpha.clamp(0.0, 255.0))
            / (255.0 * 255.0);
        if alpha <= 0.0 {
            return;
        }

        // Cover: scale so image fills the rect, crop overflow centered.
        let scale = (w / art.width as f32).max(h / art.height as f32);
        let offset_x = (art.width as f32 * scale - w) / 2.0;
        let offset_y = (art.height as f32 * scale - h) / 2.0;

        let px_start = x.round() as i32;
        let py_start = y.round() as i32;
        let x_max = art.width as i32 - 1;
        let y_max = art.height as i32 - 1;
        let rows = h.round() as i32;
        let cols = w.round() as i32;

        for row in 0..rows {
            let sy = (row as f32 + offset_y) / scale;
            let y0 = sy.floor() as i32;
            let wy = sy - y0 as f32;
            let y0c = y0.clamp(0, y_max);
            let y1c = (y0 + 1).clamp(0, y_max);

            for col in 0..cols {
                let sx = (col as f32 + offset_x) / scale;
                let x0 = sx.floor() as i32;
                let wx = sx - x0 as f32;
                let x0c = x0.clamp(0, x_max);
                let x1c = (x0 + 1).clamp(0, x_max);

                let i00 = y0c as usize * art.width + x0c as usize;
                let i10 = y0c as usize * art.width + x1c as usize;
                let i01 = y1c as usize * art.width + x0c as usize;
                let i11 = y1c as usize * art.width + x1c as usize;

                let c00 = art.rgba[i00];
                let c10 = art.rgba[i10];
                let c01 = art.rgba[i01];
                let c11 = art.rgba[i11];

                let w00 = (1.0 - wx) * (1.0 - wy);
                let w10 = wx * (1.0 - wy);
                let w01 = (1.0 - wx) * wy;
                let w11 = wx * wy;

                let r = w00 * c00[0] as f32
                    + w10 * c10[0] as f32
                    + w01 * c01[0] as f32
                    + w11 * c11[0] as f32;
                let g = w00 * c00[1] as f32
                    + w10 * c10[1] as f32
                    + w01 * c01[1] as f32
                    + w11 * c11[1] as f32;
                let b = w00 * c00[2] as f32
                    + w10 * c10[2] as f32
                    + w01 * c01[2] as f32
                    + w11 * c11[2] as f32;
                let a = w00 * c00[3] as f32
                    + w10 * c10[3] as f32
                    + w01 * c01[3] as f32
                    + w11 * c11[3] as f32;

                let src_a = (a / 255.0) * alpha;
                if src_a <= 0.0 {
                    continue;
                }

                let dx = px_start + col;
                let dy = py_start + row;
                if dx < 0 || dy < 0 || dx >= self.width as i32 || dy >= self.height as i32 {
                    continue;
                }
                let dst_idx = dy as usize * self.width + dx as usize;
                let dst = &mut self.pixmap.pixels_mut()[dst_idx];

                let fr = (r * src_a + dst.red() as f32 * (1.0 - src_a)) as u8;
                let fg = (g * src_a + dst.green() as f32 * (1.0 - src_a)) as u8;
                let fb = (b * src_a + dst.blue() as f32 * (1.0 - src_a)) as u8;

                *dst = tiny_skia::PremultipliedColorU8::from_rgba(fr, fg, fb, 255).unwrap();
            }
        }
    }

    pub fn measure_text(&self, text: &str, size: f32) -> f32 {
        let mut width = 0.0;
        for c in text.chars() {
            let (metrics, _) = self.font.rasterize(c, size);
            width += metrics.advance_width;
        }
        width
    }
}
