//! SVG export.

use crate::geom::{Path, Seg, P};
use crate::model::*;
use crate::render::{self, Painter};
use std::fmt::Write as _;

const S: f64 = 20.0; // SVG units per centimetre

pub struct SvgPainter {
    out: String,
    ox: f64,
    oy: f64,
}

fn n(v: f64) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl SvgPainter {
    fn pt(&self, q: P) -> String {
        format!("{},{}", n((q.x - self.ox) * S), n((q.y - self.oy) * S))
    }
}

impl Painter for SvgPainter {
    fn path(&mut self, path: &Path, fill: Option<Col>, stroke: Option<Col>, width: f64, style: LineStyle, dash: f64) {
        let mut d = String::new();
        for s in &path.segs {
            match *s {
                Seg::Move(a) => d += &format!("M{} ", self.pt(a)),
                Seg::Line(a) => d += &format!("L{} ", self.pt(a)),
                Seg::Curve(a, b, c) => d += &format!("C{} {} {} ", self.pt(a), self.pt(b), self.pt(c)),
                Seg::Close => d += "Z ",
            }
        }
        let _ = write!(self.out, "<path d=\"{}\" fill=\"{}\" fill-rule=\"evenodd\"", d.trim(), fill.map(|c| c.hex()).unwrap_or("none".into()));
        if let Some(c) = stroke {
            let _ = write!(self.out, " stroke=\"{}\" stroke-width=\"{}\" stroke-miterlimit=\"4\"", c.hex(), n(width * S));
            if let Some(pat) = render::dash_pattern(style, dash) {
                let v: Vec<String> = pat.iter().map(|x| n(x * S)).collect();
                let _ = write!(self.out, " stroke-dasharray=\"{}\"", v.join(","));
            }
        }
        self.out.push_str("/>\n");
    }

    fn text_line(&mut self, s: &str, t: &TextSpec, x: f64, baseline: f64) {
        let (fam, generic) = match crate::fonts::family_index(&t.family) {
            1 => ("Times New Roman", "serif"),
            2 => ("Courier New", "monospace"),
            _ => ("Arial", "sans-serif"),
        };
        let _ = writeln!(
            self.out,
            "<text x=\"{}\" y=\"{}\" font-family=\"{}, {}\" font-size=\"{}\" fill=\"{}\"{}{} xml:space=\"preserve\">{}</text>",
            n((x - self.ox) * S),
            n((baseline - self.oy) * S),
            fam,
            generic,
            n(t.height * EM_FACTOR * S),
            t.color.hex(),
            if t.bold { " font-weight=\"bold\"" } else { "" },
            if t.italic { " font-style=\"italic\"" } else { "" },
            esc(s)
        );
    }

    fn image(&mut self, img: &ImageObj) {
        let Some(pic) = &img.pic else { return };
        let mut png = Vec::new();
        if image::DynamicImage::ImageRgba8((**pic).clone()).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_err() {
            return;
        }
        let _ = writeln!(
            self.out,
            "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"data:image/png;base64,{}\"/>",
            n((img.rect.x - self.ox) * S),
            n((img.rect.y - self.oy) * S),
            n(img.rect.w * S),
            n(img.rect.h * S),
            crate::dia::base64_encode(&png)
        );
    }
}

pub fn export(d: &Diagram, path: &std::path::Path) -> Result<(), String> {
    let ext = d.extents().unwrap_or(crate::geom::r(0.0, 0.0, 10.0, 10.0)).inflate(0.5);
    let mut sp = SvgPainter { out: String::new(), ox: ext.x, oy: ext.y };
    render::draw_diagram(d, &mut sp, None);
    let (w, h) = (n(ext.w * S), n(ext.h * S));
    let svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}cm\" height=\"{}cm\" viewBox=\"0 0 {} {}\">\n<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>\n{}</svg>\n",
        n(ext.w),
        n(ext.h),
        w,
        h,
        w,
        h,
        d.background.hex(),
        sp.out
    );
    std::fs::write(path, svg).map_err(|e| e.to_string())
}
