use mlua::Table;
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Point, Rect, SpreadMode,
    Transform,
};

use crate::library::AlbumArt;

use super::UiEngine;

impl UiEngine {
    pub(super) fn draw_rect(&mut self, cmd: &Table) {
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

    pub(super) fn draw_triangle(&mut self, cmd: &Table) {
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

    pub(super) fn draw_gradient_rect(&mut self, cmd: &Table) {
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

    pub(super) fn draw_text(&mut self, cmd: &Table) {
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

        let mut pen_x = x.round();
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

    pub(super) fn draw_album_art(&mut self, cmd: &Table, art: Option<&AlbumArt>, alpha_mult: f32) {
        let Some(art) = art else { return };
        let x: f32 = cmd.get("x").unwrap();
        let y: f32 = cmd.get("y").unwrap();
        let w: f32 = cmd.get("width").unwrap();
        let h: f32 = cmd.get("height").unwrap();
        let cmd_alpha: f32 = cmd.get("alpha").ok().unwrap_or(255.0);

        if w <= 0.0 || h <= 0.0 || art.width == 0 || art.height == 0 {
            return;
        }
        let alpha = (alpha_mult.clamp(0.0, 255.0) * cmd_alpha.clamp(0.0, 255.0)) / (255.0 * 255.0);
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

    pub(super) fn draw_album_art_crossfade(
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

        let p = if prev_s.is_none() {
            1.0
        } else if curr_s.is_none() {
            0.0
        } else {
            p
        };

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

fn rounded_triangle_path(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Option<tiny_skia::Path> {
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
