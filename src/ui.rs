use fontdue::{Font, FontSettings};
use mlua::{Lua, Table};
use std::fs;
use std::sync::Arc;
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
    pub pane_art_prev: Option<&'a AlbumArt>,
    pub pane_art: Option<&'a AlbumArt>,
    pub pane_art_progress: f32,
    pub list_scroll: Option<(f32, f32)>,
    pub now_scroll: Option<(f32, f32)>,
    pub header_scroll: Option<(f32, f32)>,
    pub artist_scroll: Option<(f32, f32)>,
    pub is_reversing: bool,
    pub scroll_offset: usize,
    pub is_paused: bool,
}

pub struct LayoutConfig {
    pub font_size_list: f32,
    pub list_item_max_width: f32,
    pub font_size_nowplay: f32,
    pub nowplay_max_width: f32,
    pub font_size_header: f32,
    pub header_max_width: f32,
    pub font_size_artist: f32,
    pub artist_max_width: f32,
    pub list_scroll_gap: f32,
    pub nowplay_scroll_gap: f32,
    pub header_scroll_gap: f32,
    pub artist_scroll_gap: f32,
    pub visible_rows: usize,
}

pub struct UiEngine {
    lua: Lua,
    pixmap: Pixmap,
    font: Arc<Font>,
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
        let font = Arc::new(
            Font::from_bytes(font_data, FontSettings::default()).expect("Failed to parse font"),
        );

        let line_ratio = font
            .horizontal_line_metrics(16.0)
            .map(|m| (m.ascent - m.descent) / 16.0)
            .unwrap_or(1.16);

        let lua = Lua::new();
        let script = fs::read_to_string("ui.lua").expect("Could not find ui.lua");
        lua.load(&script).exec().expect("Failed to run Lua script");

        {
            let font_for_lua = font.clone();
            let measure_fn = lua
                .create_function(move |_, (text, size): (String, f32)| {
                    let mut width = 0.0f32;
                    for c in text.chars() {
                        let (metrics, _) = font_for_lua.rasterize(c, size);
                        width += metrics.advance_width;
                    }
                    Ok(width)
                })
                .expect("Failed to create Lua measure_text");
            lua.globals()
                .set("measure_text", measure_fn)
                .expect("Failed to register Lua measure_text");
        }

        let config_fn: mlua::Function = lua.globals().get("get_layout_config").unwrap();
        let config_table: mlua::Table = config_fn.call(()).unwrap();

        let layout = LayoutConfig {
            font_size_list: config_table.get("ITEM_FONT").unwrap(),
            list_item_max_width: config_table.get("LIST_ITEM_MAX_WIDTH").unwrap(),
            font_size_nowplay: config_table.get("NOWPLAY_TITLE_FONT").unwrap(),
            nowplay_max_width: config_table.get("NOWPLAY_MAX_WIDTH").unwrap(),
            font_size_header: config_table.get("HEADER_FONT").unwrap(),
            header_max_width: config_table.get("HEADER_MAX_WIDTH").unwrap(),
            font_size_artist: config_table.get("NOWPLAY_ARTIST_FONT").unwrap(),
            artist_max_width: config_table.get("NOWPLAY_ARTIST_MAX_WIDTH").unwrap(),
            list_scroll_gap: config_table.get("LIST_SCROLL_GAP").unwrap(),
            nowplay_scroll_gap: config_table.get("NOWPLAY_SCROLL_GAP").unwrap(),
            header_scroll_gap: config_table.get("HEADER_SCROLL_GAP").unwrap(),
            artist_scroll_gap: config_table.get("ARTIST_SCROLL_GAP").unwrap(),
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

        let pane_art_prev = frame.pane_art_prev;
        let pane_art = frame.pane_art;
        let pane_art_progress = frame.pane_art_progress;
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
                "triangle" => self.draw_triangle(&cmd),
                "album_art_prev" => {
                    if pane_art.is_none() {
                        self.draw_album_art(
                            &cmd,
                            pane_art_prev,
                            (1.0 - pane_art_progress) * 255.0,
                        );
                    }
                }
                "album_art" => self
                    .draw_album_art_crossfade(&cmd, pane_art_prev, pane_art, pane_art_progress),
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
        state
            .set("has_pane_art_prev", frame.pane_art_prev.is_some())
            .unwrap();
        state.set("is_paused", frame.is_paused).unwrap();

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

        if let Some((offset, width)) = frame.header_scroll {
            let t = self.lua.create_table().unwrap();
            t.set(1, offset).unwrap();
            t.set(2, width).unwrap();
            state.set("header_scroll", t).unwrap();
        }

        if let Some((offset, width)) = frame.artist_scroll {
            let t = self.lua.create_table().unwrap();
            t.set(1, offset).unwrap();
            t.set(2, width).unwrap();
            state.set("artist_scroll", t).unwrap();
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

    fn draw_rect(&mut self, cmd: &Table) {
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let w: f32 = cmd.get("width").unwrap();
        let h: f32 = cmd.get("height").unwrap();
        let color_hex: u32 = cmd.get("color").unwrap();
        let radius: f32 = cmd.get("radius").ok().unwrap_or(0.0);

        let color = Color::from_rgba8(
            ((color_hex >> 16) & 0xFF) as u8,
            ((color_hex >> 8) & 0xFF) as u8,
            (color_hex & 0xFF) as u8,
            ((color_hex >> 24) & 0xFF) as u8,
        );

        let mut paint = Paint::default();
        paint.set_color(color);

        let path = if radius > 0.0 {
            rounded_rect_path(x, y, w, h, radius)
        } else {
            Rect::from_xywh(x, y, w, h).map(PathBuilder::from_rect)
        };

        if let Some(path) = path {
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    fn draw_triangle(&mut self, cmd: &Table) {
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let w: f32 = cmd.get("width").unwrap();
        let h: f32 = cmd.get("height").unwrap();
        let color_hex: u32 = cmd.get("color").unwrap();
        let radius: f32 = cmd.get("radius").ok().unwrap_or(0.0);

        let color = Color::from_rgba8(
            ((color_hex >> 16) & 0xFF) as u8,
            ((color_hex >> 8) & 0xFF) as u8,
            (color_hex & 0xFF) as u8,
            ((color_hex >> 24) & 0xFF) as u8,
        );

        let mut paint = Paint::default();
        paint.set_color(color);

        if let Some(path) = rounded_triangle_path(x, y, w, h, radius) {
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
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

    fn draw_album_art(&mut self, cmd: &Table, art: Option<&AlbumArt>, alpha_mult: f32) {
        let Some(art) = art else { return };
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let w: f32 = cmd.get("width").unwrap();
        let h: f32 = cmd.get("height").unwrap();
        let cmd_alpha: f32 = cmd.get("alpha").ok().unwrap_or(255.0);

        if w <= 0.0 || h <= 0.0 || art.width == 0 || art.height == 0 {
            return;
        }
        let alpha =
            (alpha_mult.clamp(0.0, 255.0) * cmd_alpha.clamp(0.0, 255.0)) / (255.0 * 255.0);
        if alpha <= 0.0 {
            return;
        }

        let sampler = ArtSampler::new(art, w, h);
        let px_start = x.round() as i32;
        let py_start = y.round() as i32;
        let rows = h.round() as i32;
        let cols = w.round() as i32;

        for row in 0..rows {
            let dy = py_start + row;
            if dy < 0 || dy >= self.height as i32 {
                continue;
            }
            for col in 0..cols {
                let dx = px_start + col;
                if dx < 0 || dx >= self.width as i32 {
                    continue;
                }
                let [r, g, b, a] = sampler.sample(col as f32, row as f32);
                let src_a = (a / 255.0) * alpha;
                if src_a <= 0.0 {
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

    fn draw_album_art_crossfade(
        &mut self,
        cmd: &Table,
        prev: Option<&AlbumArt>,
        curr: Option<&AlbumArt>,
        progress: f32,
    ) {
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let w: f32 = cmd.get("width").unwrap();
        let h: f32 = cmd.get("height").unwrap();
        let cmd_alpha: f32 = cmd.get("alpha").ok().unwrap_or(255.0) / 255.0;

        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let p = progress.clamp(0.0, 1.0);
        let prev_s = prev
            .filter(|a| a.width > 0 && a.height > 0)
            .map(|a| ArtSampler::new(a, w, h));
        let curr_s = curr
            .filter(|a| a.width > 0 && a.height > 0)
            .map(|a| ArtSampler::new(a, w, h));
        if prev_s.is_none() && curr_s.is_none() {
            return;
        }

        let px_start = x.round() as i32;
        let py_start = y.round() as i32;
        let rows = h.round() as i32;
        let cols = w.round() as i32;

        for row in 0..rows {
            let dy = py_start + row;
            if dy < 0 || dy >= self.height as i32 {
                continue;
            }
            for col in 0..cols {
                let dx = px_start + col;
                if dx < 0 || dx >= self.width as i32 {
                    continue;
                }

                let mut acc = [0.0f32; 4];
                if p < 1.0 {
                    if let Some(s) = &prev_s {
                        let c = s.sample(col as f32, row as f32);
                        for i in 0..4 {
                            acc[i] += c[i] * (1.0 - p);
                        }
                    }
                }
                if p > 0.0 {
                    if let Some(s) = &curr_s {
                        let c = s.sample(col as f32, row as f32);
                        for i in 0..4 {
                            acc[i] += c[i] * p;
                        }
                    }
                }

                let src_a = (acc[3] / 255.0) * cmd_alpha;
                if src_a <= 0.0 {
                    continue;
                }
                let dst_idx = dy as usize * self.width + dx as usize;
                let dst = &mut self.pixmap.pixels_mut()[dst_idx];
                let fr = (acc[0] * src_a + dst.red() as f32 * (1.0 - src_a)) as u8;
                let fg = (acc[1] * src_a + dst.green() as f32 * (1.0 - src_a)) as u8;
                let fb = (acc[2] * src_a + dst.blue() as f32 * (1.0 - src_a)) as u8;
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

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Option<tiny_skia::Path> {
    let r = radius.min(w / 2.0).min(h / 2.0);
    if r <= 0.0 {
        let rect = Rect::from_xywh(x, y, w, h)?;
        return Some(PathBuilder::from_rect(rect));
    }
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

fn rounded_triangle_path(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
) -> Option<tiny_skia::Path> {
    let (ax, ay) = (x, y);
    let (bx, by) = (x, y + h);
    let (cx, cy) = (x + w, y + h / 2.0);

    let dist = |px: f32, py: f32, qx: f32, qy: f32| -> f32 {
        ((qx - px).powi(2) + (qy - py).powi(2)).sqrt()
    };
    let lerp = |px: f32, py: f32, qx: f32, qy: f32, t: f32| -> (f32, f32) {
        (px + (qx - px) * t, py + (qy - py) * t)
    };

    let ab_len = dist(ax, ay, bx, by);
    let ac_len = dist(ax, ay, cx, cy);
    let bc_len = dist(bx, by, cx, cy);

    let r_ab = if ab_len > 0.0 {
        (radius / ab_len).min(0.5)
    } else {
        0.0
    };
    let r_ac = if ac_len > 0.0 {
        (radius / ac_len).min(0.5)
    } else {
        0.0
    };
    let r_bc = if bc_len > 0.0 {
        (radius / bc_len).min(0.5)
    } else {
        0.0
    };

    let (a_bx, a_by) = lerp(ax, ay, bx, by, r_ab);
    let (a_cx, a_cy) = lerp(ax, ay, cx, cy, r_ac);
    let (b_ax, b_ay) = lerp(bx, by, ax, ay, r_ab);
    let (b_cx, b_cy) = lerp(bx, by, cx, cy, r_bc);
    let (c_ax, c_ay) = lerp(cx, cy, ax, ay, r_ac);
    let (c_bx, c_by) = lerp(cx, cy, bx, by, r_bc);

    let mut pb = PathBuilder::new();
    pb.move_to(a_bx, a_by);
    pb.line_to(b_ax, b_ay);
    pb.quad_to(bx, by, b_cx, b_cy);
    pb.line_to(c_bx, c_by);
    pb.quad_to(cx, cy, c_ax, c_ay);
    pb.line_to(a_cx, a_cy);
    pb.quad_to(ax, ay, a_bx, a_by);
    pb.close();
    pb.finish()
}

struct ArtSampler<'a> {
    art: &'a AlbumArt,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
}

impl<'a> ArtSampler<'a> {
    fn new(art: &'a AlbumArt, w: f32, h: f32) -> Self {
        let scale = (w / art.width as f32).max(h / art.height as f32);
        Self {
            art,
            scale,
            offset_x: (art.width as f32 * scale - w) / 2.0,
            offset_y: (art.height as f32 * scale - h) / 2.0,
        }
    }

    fn sample(&self, col: f32, row: f32) -> [f32; 4] {
        let art = self.art;
        let sx = (col + self.offset_x) / self.scale;
        let sy = (row + self.offset_y) / self.scale;
        let x0 = sx.floor() as i32;
        let y0 = sy.floor() as i32;
        let wx = sx - x0 as f32;
        let wy = sy - y0 as f32;
        let x_max = art.width as i32 - 1;
        let y_max = art.height as i32 - 1;
        let x0c = x0.clamp(0, x_max) as usize;
        let x1c = (x0 + 1).clamp(0, x_max) as usize;
        let y0c = y0.clamp(0, y_max) as usize;
        let y1c = (y0 + 1).clamp(0, y_max) as usize;

        let c00 = art.rgba[y0c * art.width + x0c];
        let c10 = art.rgba[y0c * art.width + x1c];
        let c01 = art.rgba[y1c * art.width + x0c];
        let c11 = art.rgba[y1c * art.width + x1c];

        let w00 = (1.0 - wx) * (1.0 - wy);
        let w10 = wx * (1.0 - wy);
        let w01 = (1.0 - wx) * wy;
        let w11 = wx * wy;

        [
            w00 * c00[0] as f32 + w10 * c10[0] as f32 + w01 * c01[0] as f32 + w11 * c11[0] as f32,
            w00 * c00[1] as f32 + w10 * c10[1] as f32 + w01 * c01[1] as f32 + w11 * c11[1] as f32,
            w00 * c00[2] as f32 + w10 * c10[2] as f32 + w01 * c01[2] as f32 + w11 * c11[2] as f32,
            w00 * c00[3] as f32 + w10 * c10[3] as f32 + w01 * c01[3] as f32 + w11 * c11[3] as f32,
        ]
    }
}
