use fontdue::{Font, FontSettings};
use mlua::{Lua, Table};
use std::fs;
use std::sync::Arc;
use tiny_skia::{Color, Pixmap};

use crate::library::AlbumArt;

mod draw;

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
    pub(super) pixmap: Pixmap,
    pub(super) font: Arc<Font>,
    line_ratio: f32,
    pub layout: LayoutConfig,
    pub(super) buffer: Vec<u32>,
    pub(super) width: usize,
    pub(super) height: usize,
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

    pub fn measure_text(&self, text: &str, size: f32) -> f32 {
        let mut width = 0.0;
        for c in text.chars() {
            let (metrics, _) = self.font.rasterize(c, size);
            width += metrics.advance_width;
        }
        width
    }
}
