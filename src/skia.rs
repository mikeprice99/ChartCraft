//! Raster drawing (screen and PNG export) with tiny-skia.

use crate::fonts::{glyph_outlines, GlyphSeg};
use crate::geom::{Path, Seg, P, R};
use crate::model::*;
use crate::render::{self, Painter};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, StrokeDash, Transform};

pub struct SkiaPainter<'a> {
    pub pm: &'a mut Pixmap,
    /// pixels per centimetre
    pub scale: f64,
    /// diagram point drawn at pixel (offx, offy)
    pub ox: f64,
    pub oy: f64,
    pub offx: f64,
    pub offy: f64,
    pub min_width: f32,
}

pub fn color(c: Col) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.0, c.1, c.2, c.3)
}

pub fn paint(c: Col) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color(c));
    p.anti_alias = true;
    p
}

impl SkiaPainter<'_> {
    pub fn map(&self, q: P) -> (f32, f32) {
        (((q.x - self.ox) * self.scale + self.offx) as f32, ((q.y - self.oy) * self.scale + self.offy) as f32)
    }

    fn build(&self, path: &Path) -> Option<tiny_skia::Path> {
        let mut pb = PathBuilder::new();
        for s in &path.segs {
            match *s {
                Seg::Move(a) => {
                    let (x, y) = self.map(a);
                    pb.move_to(x, y);
                }
                Seg::Line(a) => {
                    let (x, y) = self.map(a);
                    pb.line_to(x, y);
                }
                Seg::Curve(a, b, c) => {
                    let (x1, y1) = self.map(a);
                    let (x2, y2) = self.map(b);
                    let (x, y) = self.map(c);
                    pb.cubic_to(x1, y1, x2, y2, x, y);
                }
                Seg::Close => pb.close(),
            }
        }
        pb.finish()
    }

    pub fn stroke_px(&mut self, path: &tiny_skia::Path, c: Col, width_px: f32, dash_px: Option<Vec<f32>>) {
        let mut st = Stroke { width: width_px, line_cap: LineCap::Butt, line_join: LineJoin::Miter, miter_limit: 4.0, ..Default::default() };
        if let Some(d) = dash_px {
            st.dash = StrokeDash::new(d, 0.0);
        }
        self.pm.stroke_path(path, &paint(c), &st, Transform::identity(), None);
    }
}

impl Painter for SkiaPainter<'_> {
    fn path(&mut self, path: &Path, fill: Option<Col>, stroke: Option<Col>, width: f64, style: LineStyle, dash: f64) {
        let Some(tp) = self.build(path) else { return };
        if let Some(f) = fill {
            self.pm.fill_path(&tp, &paint(f), FillRule::EvenOdd, Transform::identity(), None);
        }
        if let Some(s) = stroke {
            let w = ((width * self.scale) as f32).max(self.min_width);
            let pat = render::dash_pattern(style, dash).map(|v| v.iter().map(|x| ((x * self.scale) as f32).max(0.5)).collect());
            self.stroke_px(&tp, s, w, pat);
        }
    }

    fn text_line(&mut self, s: &str, t: &TextSpec, x: f64, baseline: f64) {
        if t.height * self.scale < 1.5 {
            return;
        }
        let mut pb = PathBuilder::new();
        let mut any = false;
        for glyph in glyph_outlines(s, t, x, baseline) {
            let mut last: Option<(f64, f64)> = None;
            for seg in glyph {
                let (start, rest): ((f64, f64), Vec<(f64, f64)>) = match seg {
                    GlyphSeg::Line(a) => (a[0], a[1..].to_vec()),
                    GlyphSeg::Quad(a) => (a[0], a[1..].to_vec()),
                    GlyphSeg::Cubic(a) => (a[0], a[1..].to_vec()),
                };
                let m = |q: (f64, f64)| self.map(crate::geom::p(q.0, q.1));
                let near = last.map(|l| (l.0 - start.0).abs() < 1e-9 && (l.1 - start.1).abs() < 1e-9).unwrap_or(false);
                if !near {
                    if last.is_some() {
                        pb.close();
                    }
                    let (sx, sy) = m(start);
                    pb.move_to(sx, sy);
                }
                match rest.len() {
                    1 => {
                        let (a, b) = m(rest[0]);
                        pb.line_to(a, b);
                    }
                    2 => {
                        let (a, b) = m(rest[0]);
                        let (c, d) = m(rest[1]);
                        pb.quad_to(a, b, c, d);
                    }
                    _ => {
                        let (a, b) = m(rest[0]);
                        let (c, d) = m(rest[1]);
                        let (e, f) = m(rest[2]);
                        pb.cubic_to(a, b, c, d, e, f);
                    }
                }
                last = Some(*rest.last().unwrap());
                any = true;
            }
            if last.is_some() {
                pb.close();
            }
        }
        if !any {
            return;
        }
        if let Some(tp) = pb.finish() {
            self.pm.fill_path(&tp, &paint(t.color), FillRule::Winding, Transform::identity(), None);
        }
    }

    fn image(&mut self, img: &ImageObj) {
        let (x0, y0) = self.map(crate::geom::p(img.rect.x, img.rect.y));
        let (x1, y1) = self.map(crate::geom::p(img.rect.right(), img.rect.bottom()));
        if let Some(pic) = &img.pic {
            let (w, h) = pic.dimensions();
            let mut data = pic.as_raw().clone();
            for px in data.chunks_exact_mut(4) {
                let a = px[3] as u16;
                for c in px.iter_mut().take(3) {
                    *c = ((*c as u16 * a + 127) / 255) as u8;
                }
            }
            if let Some(src) = tiny_skia::IntSize::from_wh(w, h).and_then(|s| Pixmap::from_vec(data, s)) {
                let sx = (x1 - x0) / w as f32;
                let sy = (y1 - y0) / h as f32;
                let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
                self.pm.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_row(sx, 0.0, 0.0, sy, x0, y0), None);
                return;
            }
        }
        // No picture: a grey placeholder with a cross.
        if let Some(rect) = tiny_skia::Rect::from_ltrb(x0, y0, x1.max(x0 + 1.0), y1.max(y0 + 1.0)) {
            self.pm.fill_rect(rect, &paint(Col::rgb(235, 235, 235)), Transform::identity(), None);
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            pb.move_to(x0, y0);
            pb.line_to(x1, y1);
            pb.move_to(x1, y0);
            pb.line_to(x0, y1);
            if let Some(tp) = pb.finish() {
                self.stroke_px(&tp, Col::rgb(128, 128, 128), 1.0, Some(vec![4.0, 3.0]));
            }
        }
    }
}

/// Render a whole diagram to a PNG file at `px_per_cm`.
pub fn export_png(d: &Diagram, path: &std::path::Path, px_per_cm: f64) -> Result<(), String> {
    let ext = d.extents().unwrap_or(crate::geom::r(0.0, 0.0, 10.0, 10.0)).inflate(0.5);
    let w = ((ext.w * px_per_cm).ceil() as u32).clamp(1, 16000);
    let h = ((ext.h * px_per_cm).ceil() as u32).clamp(1, 16000);
    let mut pm = Pixmap::new(w, h).ok_or("The picture would be too large.")?;
    pm.fill(color(d.background));
    {
        let mut sp = SkiaPainter { pm: &mut pm, scale: px_per_cm, ox: ext.x, oy: ext.y, offx: 0.0, offy: 0.0, min_width: 1.0 };
        render::draw_diagram(d, &mut sp, None);
    }
    let rgba = unpremultiply(pm.data());
    image::save_buffer(path, &rgba, w, h, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())
}

/// Render a whole diagram to PNG bytes, at most `max_px` on its longer side.
pub fn png_bytes(d: &Diagram, max_px: u32) -> Result<Vec<u8>, String> {
    let ext = d.extents().unwrap_or(crate::geom::r(0.0, 0.0, 10.0, 10.0)).inflate(0.5);
    let px_per_cm = (max_px as f64 / ext.w.max(ext.h)).min(40.0);
    let w = ((ext.w * px_per_cm).ceil() as u32).clamp(1, 16000);
    let h = ((ext.h * px_per_cm).ceil() as u32).clamp(1, 16000);
    let mut pm = Pixmap::new(w, h).ok_or("The picture would be too large.")?;
    pm.fill(color(d.background));
    {
        let mut sp = SkiaPainter { pm: &mut pm, scale: px_per_cm, ox: ext.x, oy: ext.y, offx: 0.0, offy: 0.0, min_width: 1.0 };
        render::draw_diagram(d, &mut sp, None);
    }
    let img = image::RgbaImage::from_raw(w, h, unpremultiply(pm.data())).ok_or("Couldn't make the picture.")?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn unpremultiply(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    for px in out.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in px.iter_mut().take(3) {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    out
}

/// Small picture of a sheet shape for the toolbox.
pub fn shape_icon(def: usize, size: u32) -> Pixmap {
    let mut pm = Pixmap::new(size, size).unwrap();
    let d = crate::shapes::def(def);
    let ar = d.def_w / d.def_h;
    let avail = size as f64 - 4.0;
    let (w, h) = if ar > 1.0 { (avail, avail / ar) } else { (avail * ar, avail) };
    let scale = w / d.def_w;
    let mut e = Element::new(def, crate::geom::r(0.0, 0.0, d.def_w, d.def_h));
    e.line_width = 2.6 / scale;
    let mut sp = SkiaPainter { pm: &mut pm, scale, ox: 0.0, oy: 0.0, offx: (size as f64 - w) / 2.0, offy: (size as f64 - h) / 2.0, min_width: 1.0 };
    render::draw_element(&e, &mut sp, true);
    pm
}

/// Grid lines and page edges, drawn under the diagram on screen.
pub fn draw_backdrop(sp: &mut SkiaPainter, d: &Diagram, view: R, show_grid: bool, show_pages: bool) {
    let (w, h) = (sp.pm.width() as f32, sp.pm.height() as f32);
    if show_grid {
        let base = d.grid.max(0.01);
        let mut g = base;
        while g * sp.scale < 7.0 {
            g *= 2.0;
        }
        // Minor lines dashed, every fifth line solid (like the original).
        let mut minor = PathBuilder::new();
        let mut major = PathBuilder::new();
        let is_major = |v: f64| {
            let k = (v / base).round() as i64;
            k.rem_euclid(5) == 0
        };
        let mut x = (view.x / g).floor() * g;
        while x <= view.right() {
            let (sx, _) = sp.map(crate::geom::p(x, 0.0));
            let sx = sx.round() + 0.5;
            let pb = if is_major(x) { &mut major } else { &mut minor };
            pb.move_to(sx, 0.0);
            pb.line_to(sx, h);
            x += g;
        }
        let mut y = (view.y / g).floor() * g;
        while y <= view.bottom() {
            let (_, sy) = sp.map(crate::geom::p(0.0, y));
            let sy = sy.round() + 0.5;
            let pb = if is_major(y) { &mut major } else { &mut minor };
            pb.move_to(0.0, sy);
            pb.line_to(w, sy);
            y += g;
        }
        let k = (sp.scale / 20.0).max(1.0) as f32;
        if let Some(tp) = minor.finish() {
            sp.stroke_px(&tp, Col::rgb(0xd8, 0xe5, 0xe5), 1.0, Some(vec![4.0 * k, 3.0 * k]));
        }
        if let Some(tp) = major.finish() {
            sp.stroke_px(&tp, Col::rgb(0xd8, 0xe5, 0xe5), 1.0, None);
        }
    }
    if show_pages {
        let (pw, ph) = (d.paper.page_w(), d.paper.page_h());
        // Page edges repeat across the whole view, like the original.
        let area = view;
        let (x0, x1) = ((area.x / pw).floor() as i64, (area.right() / pw).ceil() as i64);
        let (y0, y1) = ((area.y / ph).floor() as i64, (area.bottom() / ph).ceil() as i64);
        if (x1 - x0) * (y1 - y0) <= 400 {
            let mut pb = PathBuilder::new();
            let (ax, ay) = sp.map(crate::geom::p(x0 as f64 * pw, y0 as f64 * ph));
            let (bx, by) = sp.map(crate::geom::p(x1 as f64 * pw, y1 as f64 * ph));
            for i in x0..=x1 {
                let (sx, _) = sp.map(crate::geom::p(i as f64 * pw, 0.0));
                let sx = sx.round() + 0.5;
                pb.move_to(sx, ay);
                pb.line_to(sx, by);
            }
            for j in y0..=y1 {
                let (_, sy) = sp.map(crate::geom::p(0.0, j as f64 * ph));
                let sy = sy.round() + 0.5;
                pb.move_to(ax, sy);
                pb.line_to(bx, sy);
            }
            if let Some(tp) = pb.finish() {
                sp.stroke_px(&tp, Col::rgb(0, 0, 0x99), 1.0, None);
            }
        }
    }
}
