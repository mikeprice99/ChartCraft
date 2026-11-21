//! Fonts for drawing diagram text: the system's Arial / Times / Courier when present,
//! otherwise the font bundled with the UI library.

use crate::model::{TextSpec, EM_FACTOR};
use ab_glyph::{Font, FontArc, GlyphId, OutlineCurve};
use std::sync::OnceLock;

pub struct FontSet {
    /// [family][bold*2 + italic], family 0 sans, 1 serif, 2 mono
    fonts: Vec<Vec<FontArc>>,
}

fn load(files: &[&str]) -> Option<FontArc> {
    let dir = std::env::var("WINDIR").map(|w| format!("{w}\\Fonts")).unwrap_or_else(|_| "C:\\Windows\\Fonts".into());
    for f in files {
        for base in [dir.as_str(), "/usr/share/fonts/truetype/msttcorefonts", "/Library/Fonts", "/System/Library/Fonts/Supplemental"] {
            let path = std::path::Path::new(base).join(f);
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(font) = FontArc::try_from_vec(bytes) {
                    return Some(font);
                }
            }
        }
    }
    None
}

pub fn set() -> &'static FontSet {
    static F: OnceLock<FontSet> = OnceLock::new();
    F.get_or_init(|| {
        let fallback = FontArc::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).expect("bundled font");
        let mono_fallback = FontArc::try_from_slice(epaint_default_fonts::HACK_REGULAR).expect("bundled font");
        let names: [[&[&str]; 4]; 3] = [
            [&["arial.ttf", "Arial.ttf"], &["ariali.ttf", "Arial Italic.ttf"], &["arialbd.ttf", "Arial Bold.ttf"], &["arialbi.ttf", "Arial Bold Italic.ttf"]],
            [&["times.ttf", "Times New Roman.ttf"], &["timesi.ttf"], &["timesbd.ttf"], &["timesbi.ttf"]],
            [&["cour.ttf", "Courier New.ttf"], &["couri.ttf"], &["courbd.ttf"], &["courbi.ttf"]],
        ];
        let mut fonts = Vec::new();
        for (fi, fam) in names.iter().enumerate() {
            let regular = load(fam[0]).unwrap_or_else(|| if fi == 2 { mono_fallback.clone() } else { fallback.clone() });
            let v: Vec<FontArc> = fam.iter().enumerate().map(|(i, f)| if i == 0 { regular.clone() } else { load(f).unwrap_or_else(|| regular.clone()) }).collect();
            fonts.push(v);
        }
        FontSet { fonts }
    })
}

pub fn family_index(family: &str) -> usize {
    let f = family.to_ascii_lowercase();
    if f.contains("mono") || f.contains("courier") {
        2
    } else if f == "serif" || f.contains("times") || f.contains("roman") {
        1
    } else {
        0
    }
}

pub fn font_for(t: &TextSpec) -> &'static FontArc {
    &set().fonts[family_index(&t.family)][(t.bold as usize) * 2 + t.italic as usize]
}

fn upem(f: &FontArc) -> f64 {
    f.units_per_em().unwrap_or(2048.0) as f64
}

/// Width of one line of text in diagram units.
pub fn line_width(s: &str, t: &TextSpec) -> f64 {
    let f = font_for(t);
    let em = t.height * EM_FACTOR;
    let mut total = 0.0;
    let mut prev: Option<GlyphId> = None;
    for ch in s.chars() {
        let g = f.glyph_id(ch);
        if let Some(pg) = prev {
            total += f.kern_unscaled(pg, g) as f64;
        }
        total += f.h_advance_unscaled(g) as f64;
        prev = Some(g);
    }
    total / upem(f) * em
}

pub fn text_size(t: &TextSpec) -> (f64, f64) {
    let lines = t.lines();
    let w = lines.iter().map(|l| line_width(l, t)).fold(0.0, f64::max);
    (w, lines.len() as f64 * t.height)
}

/// Glyph outlines for a line of text, starting at (x, baseline), in diagram units (y down).
/// Each curve: points (already positioned), as Line(a,b) / Quad(a,b,c) / Cubic(a,b,c,d).
pub enum GlyphSeg {
    Line([(f64, f64); 2]),
    Quad([(f64, f64); 3]),
    Cubic([(f64, f64); 4]),
}

pub fn glyph_outlines(s: &str, t: &TextSpec, x: f64, baseline: f64) -> Vec<Vec<GlyphSeg>> {
    let f = font_for(t);
    let k = t.height * EM_FACTOR / upem(f);
    let mut pen = 0.0f64;
    let mut prev: Option<GlyphId> = None;
    let mut res = Vec::new();
    for ch in s.chars() {
        let g = f.glyph_id(ch);
        if let Some(pg) = prev {
            pen += f.kern_unscaled(pg, g) as f64;
        }
        if let Some(o) = f.outline(g) {
            let ox = pen;
            let m = |pt: ab_glyph::Point| (x + (ox + pt.x as f64) * k, baseline - pt.y as f64 * k);
            let segs = o
                .curves
                .iter()
                .map(|c| match c {
                    OutlineCurve::Line(a, b) => GlyphSeg::Line([m(*a), m(*b)]),
                    OutlineCurve::Quad(a, b, c) => GlyphSeg::Quad([m(*a), m(*b), m(*c)]),
                    OutlineCurve::Cubic(a, b, c, d) => GlyphSeg::Cubic([m(*a), m(*b), m(*c), m(*d)]),
                })
                .collect();
            res.push(segs);
        }
        pen += f.h_advance_unscaled(g) as f64;
        prev = Some(g);
    }
    res
}
