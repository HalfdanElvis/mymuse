use fontdue::{Font, FontSettings};
use mlua::{Lua, Table};
use std::{fs, os::linux::raw::stat};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

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
}

pub struct UiEngine {
    lua: Lua,
    pixmap: Pixmap,
    font: Font,
    line_ratio: f32,
    buffer: Vec<u32>,
    width: usize,
    height: usize,
}

impl UiEngine {
    pub fn new(width: usize, height: usize) -> Self {
        // 1. Setup Pixmap and buffer
        let pixmap = Pixmap::new(width as u32, height as u32).unwrap();
        let buffer = vec![0; width * height];

        // 2. Load font
        let font_data = fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
            .expect("Could not find font file");
        let font =
            Font::from_bytes(font_data, FontSettings::default()).expect("Failed to parse font");

        let line_ratio = font
            .horizontal_line_metrics(16.0)
            .map(|m| (m.ascent - m.descent) / 16.0)
            .unwrap_or(1.16);

        // 3. Setup Lua
        let lua = Lua::new();
        let script = fs::read_to_string("ui.lua").expect("Could not find ui.lua");
        lua.load(&script).exec().expect("Failed to run Lua script");

        Self {
            lua,
            pixmap,
            font,
            line_ratio,
            buffer,
            width,
            height,
        }
    }

    // Returns a slice of the pixel buffer. No windowing logic here!
    pub fn draw(&mut self, frame: UiFrame) -> &[u32] {
        self.pixmap.fill(Color::from_rgba8(0, 0, 0, 255));

        // 1. Convert Rust items to Lua Table
        let lua_items = self.lua.create_table().unwrap();
        for (i, item) in frame.items.iter().enumerate() {
            lua_items.set(i + 1, item.clone()).unwrap();
        }

        // 2. Create the State Table
        let state = self.lua.create_table().unwrap();
        state.set("items", lua_items).unwrap();
        state
            .set("selected_index", frame.selected_index + 1)
            .unwrap(); // 1-based for ui.lua
        state
            .set("line_height_ratio", self.line_ratio as f64)
            .unwrap();
        state.set("title", frame.title).unwrap();
        state.set("clock", frame.clock).unwrap();
        state.set("battery", frame.battery).unwrap();
        state.set("bluetooth", frame.bluetooth).unwrap();
        state.set("headphones", frame.headphones).unwrap();

        if let Some(np) = frame.now {
            let table = self.lua.create_table().unwrap();
            table.set("title", np.title).unwrap();
            table.set("artist", np.artist).unwrap();
            table.set("elapsed", np.elapsed).unwrap();
            table.set("duration", np.duration).unwrap();
            state.set("now", table).unwrap();
        }

        // 3. Call Lua draw_screen
        let draw_fn: mlua::Function = self.lua.globals().get("draw_screen").unwrap();
        let draw_cmds: Table = draw_fn.call(state).unwrap();

        // 4. Parse commands
        for cmd in draw_cmds.sequence_values::<Table>() {
            let cmd = cmd.unwrap();
            let cmd_type: String = cmd.get("type").unwrap();

            if cmd_type == "rect" {
                self.draw_rect(&cmd);
            } else if cmd_type == "text" {
                self.draw_text(&cmd);
            }
        }

        // 5. Copy Pixmap to Buffer
        for (i, pixel) in self.pixmap.pixels().iter().enumerate() {
            let r = pixel.red() as u32;
            let g = pixel.green() as u32;
            let b = pixel.blue() as u32;
            self.buffer[i] = (r << 16) | (g << 8) | b;
        }

        &self.buffer
    }

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

    fn draw_text(&mut self, cmd: &Table) {
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let text: String = cmd.get("text").unwrap();
        let color_hex: u32 = cmd.get("color").unwrap();
        let size: f32 = cmd.get("size").unwrap();
        let max_width: Option<f32> = cmd.get("max_width").ok();

        let r = ((color_hex >> 16) & 0xFF) as u8;
        let g = ((color_hex >> 8) & 0xFF) as u8;
        let b = (color_hex & 0xFF) as u8;

        // Color parsing and Baselin calculation
        let ascent = self
            .font
            .horizontal_line_metrics(size)
            .map(|m| m.ascent)
            .unwrap_or(size);
        let baseline = (y + ascent).round() as i32;

        let right_limit = match max_width {
            Some(mw) => (x + mw) as i32,
            None => self.width as i32,
        };

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
                    if px < 0 || px >= right_limit {
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
}
