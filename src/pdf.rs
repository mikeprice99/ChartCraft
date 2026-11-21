//! Vector PDF export using the standard PDF fonts (Helvetica, Times, Courier).

use crate::geom::{Path, Seg, P};
use crate::model::*;
use crate::render::{self, Painter};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fmt::Write as _;
use std::io::Write;

const PT_PER_CM: f64 = 72.0 / 2.54;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageMode {
    DiagramSize,
    FitToPaper,
}

struct Img {
    rgb: Vec<u8>,
    alpha: Option<Vec<u8>>,
    w: u32,
    h: u32,
}

pub struct PdfPainter {
    out: String,
    k: f64,
    ox: f64,
    oy: f64,
    offx: f64,
    offy: f64,
    page_h: f64,
    fonts: Vec<&'static str>,
    images: Vec<Img>,
}

fn n(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s
    }
}

fn col(c: Col) -> String {
    format!("{} {} {}", n(c.0 as f64 / 255.0), n(c.1 as f64 / 255.0), n(c.2 as f64 / 255.0))
}

// Helvetica advance widths for ASCII 32..126 (per 1000 em) - the same as Arial's.
const HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584,
    584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278,
    278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500,
    500, 334, 260, 334, 584,
];

fn base_font(t: &TextSpec) -> &'static str {
    match (crate::fonts::family_index(&t.family), t.bold, t.italic) {
        (2, false, false) => "Courier",
        (2, true, false) => "Courier-Bold",
        (2, false, true) => "Courier-Oblique",
        (2, true, true) => "Courier-BoldOblique",
        (1, false, false) => "Times-Roman",
        (1, true, false) => "Times-Bold",
        (1, false, true) => "Times-Italic",
        (1, true, true) => "Times-BoldItalic",
        (_, false, false) => "Helvetica",
        (_, true, false) => "Helvetica-Bold",
        (_, false, true) => "Helvetica-Oblique",
        (_, true, true) => "Helvetica-BoldOblique",
    }
}

/// Map text to the PDF's WinAnsi encoding.
fn encode(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c as u32 {
            0..=127 => c as u8,
            160..=255 => c as u32 as u8,
            _ => match c {
                '€' => 0x80,
                '…' => 0x85,
                '‘' => 0x91,
                '’' => 0x92,
                '“' => 0x93,
                '”' => 0x94,
                '•' => 0x95,
                '–' => 0x96,
                '—' => 0x97,
                '™' => 0x99,
                _ => b'?',
            },
        })
        .collect()
}

impl PdfPainter {
    fn m(&self, q: P) -> (f64, f64) {
        ((q.x - self.ox) * self.k + self.offx, self.page_h - ((q.y - self.oy) * self.k + self.offy))
    }
}

impl Painter for PdfPainter {
    fn path(&mut self, path: &Path, fill: Option<Col>, stroke: Option<Col>, width: f64, style: LineStyle, dash: f64) {
        if fill.is_none() && stroke.is_none() {
            return;
        }
        let mut s = String::from("q\n");
        for seg in &path.segs {
            match *seg {
                Seg::Move(a) => {
                    let (x, y) = self.m(a);
                    let _ = writeln!(s, "{} {} m", n(x), n(y));
                }
                Seg::Line(a) => {
                    let (x, y) = self.m(a);
                    let _ = writeln!(s, "{} {} l", n(x), n(y));
                }
                Seg::Curve(a, b, c) => {
                    let (x1, y1) = self.m(a);
                    let (x2, y2) = self.m(b);
                    let (x3, y3) = self.m(c);
                    let _ = writeln!(s, "{} {} {} {} {} {} c", n(x1), n(y1), n(x2), n(y2), n(x3), n(y3));
                }
                Seg::Close => s.push_str("h\n"),
            }
        }
        if let Some(f) = fill {
            let _ = writeln!(s, "{} rg", col(f));
        }
        if let Some(c) = stroke {
            let _ = writeln!(s, "{} RG {} w 0 J 0 j 4 M", col(c), n((width * self.k).max(0.1)));
            if let Some(pat) = render::dash_pattern(style, dash) {
                let v: Vec<String> = pat.iter().map(|x| n(x * self.k)).collect();
                let _ = writeln!(s, "[{}] 0 d", v.join(" "));
            }
        }
        s.push_str(match (fill.is_some(), stroke.is_some()) {
            (true, true) => "B*\n",
            (true, false) => "f*\n",
            _ => "S\n",
        });
        s.push_str("Q\n");
        self.out.push_str(&s);
    }

    fn text_width(&mut self, s: &str, t: &TextSpec) -> f64 {
        let bf = base_font(t);
        let em = t.height * EM_FACTOR;
        let mut total: f64 = s
            .chars()
            .map(|c| if bf.starts_with("Courier") { 600.0 } else if (32..=126).contains(&(c as u32)) { HELV[c as usize - 32] as f64 } else { 556.0 })
            .sum();
        if bf.starts_with("Times") {
            total *= 0.9;
        }
        if t.bold && !bf.starts_with("Courier") {
            total *= 1.07;
        }
        total / 1000.0 * em
    }

    fn text_line(&mut self, s: &str, t: &TextSpec, x: f64, baseline: f64) {
        let bf = base_font(t);
        let idx = match self.fonts.iter().position(|f| *f == bf) {
            Some(i) => i,
            None => {
                self.fonts.push(bf);
                self.fonts.len() - 1
            }
        };
        let (px, py) = self.m(crate::geom::p(x, baseline));
        let mut st = String::new();
        for b in encode(s) {
            match b {
                b'(' | b')' | b'\\' => {
                    st.push('\\');
                    st.push(b as char);
                }
                32..=126 => st.push(b as char),
                _ => {
                    let _ = write!(st, "\\{:03o}", b);
                }
            }
        }
        let _ = writeln!(self.out, "BT /F{} {} Tf {} rg {} {} Td ({}) Tj ET", idx + 1, n(t.height * EM_FACTOR * self.k), col(t.color), n(px), n(py), st);
    }

    fn image(&mut self, img: &ImageObj) {
        let Some(pic) = &img.pic else {
            self.path(&Path::rect(img.rect), Some(Col::rgb(235, 235, 235)), Some(Col::rgb(128, 128, 128)), 0.02, LineStyle::Dashed, 0.2);
            return;
        };
        let (w, h) = pic.dimensions();
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        let mut alpha = Vec::with_capacity((w * h) as usize);
        let mut has_alpha = false;
        for px in pic.pixels() {
            rgb.extend_from_slice(&px.0[..3]);
            alpha.push(px.0[3]);
            has_alpha |= px.0[3] != 255;
        }
        self.images.push(Img { rgb, alpha: if has_alpha { Some(alpha) } else { None }, w, h });
        let (x0, y0) = self.m(crate::geom::p(img.rect.x, img.rect.bottom()));
        let _ = writeln!(self.out, "q {} 0 0 {} {} {} cm /Im{} Do Q", n(img.rect.w * self.k), n(img.rect.h * self.k), n(x0), n(y0), self.images.len());
    }
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    let _ = e.write_all(data);
    e.finish().unwrap_or_default()
}

fn stream(dict: &str, body: Vec<u8>) -> Vec<u8> {
    let mut v = format!("<< {} /Length {} >>\nstream\n", dict, body.len()).into_bytes();
    v.extend(body);
    v.extend_from_slice(b"\nendstream");
    v
}

pub fn export(d: &Diagram, path: &std::path::Path, mode: PageMode) -> Result<(), String> {
    std::fs::write(path, build(d, mode)).map_err(|e| e.to_string())
}

pub fn build(d: &Diagram, mode: PageMode) -> Vec<u8> {
    let mut ext = d.extents().unwrap_or(crate::geom::r(0.0, 0.0, 10.0, 10.0));
    if ext.w <= 0.0 || ext.h <= 0.0 {
        ext = ext.inflate(0.5);
    }
    let margin = 1.0;
    let (page_w, page_h, scale, offx, offy) = match mode {
        PageMode::DiagramSize => (
            (ext.w + 2.0 * margin) * PT_PER_CM,
            (ext.h + 2.0 * margin) * PT_PER_CM,
            1.0,
            margin * PT_PER_CM,
            margin * PT_PER_CM,
        ),
        PageMode::FitToPaper => {
            let (pw, ph) = d.paper.size();
            let aw = (pw - d.paper.lm - d.paper.rm).max(1.0);
            let ah = (ph - d.paper.tm - d.paper.bm).max(1.0);
            let s = (aw / ext.w).min(ah / ext.h).min(1.0);
            (pw * PT_PER_CM, ph * PT_PER_CM, s, (d.paper.lm + (aw - ext.w * s) / 2.0) * PT_PER_CM, (d.paper.tm + (ah - ext.h * s) / 2.0) * PT_PER_CM)
        }
    };
    let mut pp = PdfPainter { out: String::new(), k: scale * PT_PER_CM, ox: ext.x, oy: ext.y, offx, offy, page_h, fonts: vec![], images: vec![] };
    if d.background != Col::WHITE {
        let _ = writeln!(pp.out, "q {} rg 0 0 {} {} re f Q", col(d.background), n(page_w), n(page_h));
    }
    render::draw_diagram(d, &mut pp, None);

    // Objects: 1 catalog, 2 pages, 3 page, 4 content, then fonts, then images.
    let font_base = 5;
    let mut next = font_base + pp.fonts.len();
    let mut img_objs: Vec<Vec<u8>> = Vec::new();
    let mut img_ids = Vec::new();
    for im in &pp.images {
        let id = next;
        next += 1;
        let smask = if im.alpha.is_some() {
            next += 1;
            Some(next - 1)
        } else {
            None
        };
        img_ids.push(id);
        let mut dict = format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode", im.w, im.h);
        if let Some(sm) = smask {
            let _ = write!(dict, " /SMask {} 0 R", sm);
        }
        img_objs.push(stream(&dict, deflate(&im.rgb)));
        if let Some(a) = &im.alpha {
            img_objs.push(stream(
                &format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode", im.w, im.h),
                deflate(a),
            ));
        }
    }
    let font_res: Vec<String> = pp.fonts.iter().enumerate().map(|(i, _)| format!("/F{} {} 0 R", i + 1, font_base + i)).collect();
    let img_res: Vec<String> = img_ids.iter().enumerate().map(|(i, id)| format!("/Im{} {} 0 R", i + 1, id)).collect();

    let content = pp.out.clone().into_bytes();
    let mut objs: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /Font << {} >> /XObject << {} >> /ProcSet [/PDF /Text /ImageC] >> >>",
            n(page_w),
            n(page_h),
            font_res.join(" "),
            img_res.join(" ")
        )
        .into_bytes(),
        stream("/Filter /FlateDecode", deflate(&content)),
    ];
    for f in &pp.fonts {
        objs.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>", f).into_bytes());
    }
    objs.extend(img_objs);

    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).into_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let mut x = format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1);
    for o in offsets {
        let _ = writeln!(x, "{:010} 00000 n ", o);
    }
    let _ = write!(x, "trailer\n<< /Size {} /Root 1 0 R /Info << /Producer (ChartCraft) >> >>\nstartxref\n{}\n%%EOF\n", objs.len() + 1, xref);
    out.extend(x.into_bytes());
    out
}
