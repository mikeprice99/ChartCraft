//! Drawing a diagram onto anything that can draw: the screen, PNG, PDF and SVG.

use crate::geom::*;
use crate::model::*;

pub trait Painter {
    fn path(&mut self, path: &Path, fill: Option<Col>, stroke: Option<Col>, width: f64, style: LineStyle, dash: f64);
    /// Draw one line of text with its left end at x and its baseline at `baseline`.
    fn text_line(&mut self, s: &str, t: &TextSpec, x: f64, baseline: f64);
    fn text_width(&mut self, s: &str, t: &TextSpec) -> f64 {
        crate::fonts::line_width(s, t)
    }
    fn image(&mut self, img: &ImageObj);
}

/// Draw every visible object; `hide_text_of` leaves out the text of the object being typed into.
pub fn draw_diagram(d: &Diagram, pt: &mut dyn Painter, hide_text_of: Option<u64>) {
    for o in d.visible() {
        draw_object(o, pt, d.background, Some(o.id) == hide_text_of);
    }
}

pub fn draw_object(o: &Obj, pt: &mut dyn Painter, bg: Col, hide_text: bool) {
    match &o.kind {
        Kind::Element(e) => draw_element(e, pt, hide_text),
        Kind::Text(t) => {
            if !hide_text {
                draw_text(&t.text, t.pos, pt)
            }
        }
        Kind::Line(l) => draw_line(l, pt, bg),
        Kind::Polygon(pg) => pt.path(
            &Path::new().poly(&pg.pts, true),
            if pg.filled { Some(pg.fill_color) } else { None },
            Some(pg.line_color),
            pg.line_width,
            pg.style,
            pg.dash,
        ),
        Kind::Image(im) => {
            pt.image(im);
            if im.draw_border {
                pt.path(&Path::rect(im.rect), None, Some(im.line_color), im.line_width, LineStyle::Solid, 1.0);
            }
        }
        Kind::Raw(rw) => {
            pt.path(&Path::rect(rw.bb), None, Some(Col::rgb(140, 140, 140)), 0.03, LineStyle::Dashed, 0.3);
            let t = TextSpec { text: rw.type_name.clone(), height: 0.5, color: Col::rgb(128, 128, 128), align: 0, ..Default::default() };
            draw_text(&t, p(rw.bb.x + 0.1, rw.bb.y + 0.5), pt);
        }
    }
}

pub fn draw_element(e: &Element, pt: &mut dyn Painter, hide_text: bool) {
    if let Some(r) = &e.rec {
        crate::record::draw(e, r, pt);
        return;
    }
    let stroke = if e.line_width > 0.0 { Some(e.line_color) } else { None };
    pt.path(&e.outline(), if e.filled { Some(e.fill_color) } else { None }, stroke, e.line_width, e.style, e.dash);
    if let Some(det) = e.shape().details {
        pt.path(&det(e.rect), None, Some(e.line_color), e.line_width, e.style, e.dash);
    }
    if let Some(solid) = e.shape().solid {
        pt.path(&solid(e.rect), Some(e.line_color), None, 0.0, LineStyle::Solid, 1.0);
    }
    if is_unknown(e) && e.text.as_ref().map(|t| t.text.is_empty()).unwrap_or(true) {
        let t = TextSpec { text: e.raw_type.clone(), height: 0.5, color: Col::rgb(128, 128, 128), ..Default::default() };
        draw_text(&t, p(e.rect.cx(), e.rect.cy() + 0.15), pt);
    }
    if hide_text {
        return;
    }
    if let Some(t) = &e.text {
        if !t.text.is_empty() {
            draw_text(t, e.text_pos(), pt);
        }
    }
}

pub fn draw_text(t: &TextSpec, pos: P, pt: &mut dyn Painter) {
    for (i, line) in t.lines().iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let w = pt.text_width(line, t);
        let x = match t.align {
            0 => pos.x,
            2 => pos.x - w,
            _ => pos.x - w / 2.0,
        };
        pt.text_line(line, t, x, pos.y + i as f64 * t.height);
    }
}

fn trim_for(a: &ArrowSpec) -> f64 {
    match a.kind {
        0 | 1 | 7 => 0.0,
        22 => a.length * 0.75,
        10 | 11 => a.length * 1.5,
        _ => a.length,
    }
}

pub fn draw_line(l: &LineObj, pt: &mut dyn Painter, bg: Col) {
    let pts = l.polyline();
    if pts.len() < 2 {
        return;
    }
    let trimmed = trim(&pts, trim_for(&l.start_arrow), trim_for(&l.end_arrow));
    pt.path(&Path::new().poly(&trimmed, false), None, Some(l.color), l.width, l.style, l.dash);
    if l.end_arrow.kind != 0 {
        let from = point_back_from_end(&pts, l.end_arrow.length);
        draw_arrow(pt, &l.end_arrow, *pts.last().unwrap(), from, l.color, l.width, bg);
    }
    if l.start_arrow.kind != 0 {
        let mut rev = pts.clone();
        rev.reverse();
        let from = point_back_from_end(&rev, l.start_arrow.length);
        draw_arrow(pt, &l.start_arrow, *rev.last().unwrap(), from, l.color, l.width, bg);
    }
    if !l.label.is_empty() {
        let t = TextSpec { text: l.label.clone(), height: 0.6, color: l.color, ..Default::default() };
        draw_text(&t, label_pos(l), pt);
    }
}

/// Where a line's name goes: just above the middle of the line.
pub fn label_pos(l: &LineObj) -> P {
    let pts = l.polyline();
    let total: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    let mut left = total / 2.0;
    for w in pts.windows(2) {
        let d = w[0].dist(w[1]);
        if d >= left {
            let m = w[0] + (w[1] - w[0]) * (left / d.max(1e-9));
            return p(m.x, m.y - 0.2);
        }
        left -= d;
    }
    pts[0]
}

pub fn draw_arrow(pt: &mut dyn Painter, a: &ArrowSpec, tip: P, from: P, col: Col, lw: f64, bg: Col) {
    let dir = (tip - from).norm();
    let perp = dir.perp();
    let (l, w) = (a.length, a.width / 2.0);
    let back = tip - dir * l;
    let solid = LineStyle::Solid;
    match a.kind {
        1 => pt.path(&Path::new().poly(&[back + perp * w, tip, back - perp * w], false), None, Some(col), lw, solid, 1.0),
        2 | 3 => pt.path(
            &Path::polygon(&[tip, back + perp * w, back - perp * w]),
            Some(if a.kind == 3 { col } else { bg }),
            Some(col),
            lw,
            solid,
            1.0,
        ),
        10 | 11 => {
            let f = if a.kind == 11 { col } else { bg };
            let (back2, tip2) = (back - dir * (l * 0.5), tip - dir * (l * 0.5));
            pt.path(&Path::polygon(&[tip2, back2 + perp * w, back2 - perp * w]), Some(f), Some(col), lw, solid, 1.0);
            pt.path(&Path::polygon(&[tip, back + perp * w, back - perp * w]), Some(f), Some(col), lw, solid, 1.0);
        }
        4 | 5 => {
            let mid = tip - dir * (l / 2.0);
            pt.path(
                &Path::polygon(&[tip, mid + perp * w, back, mid - perp * w]),
                Some(if a.kind == 5 { col } else { bg }),
                Some(col),
                lw,
                solid,
                1.0,
            );
        }
        8 | 9 => {
            let c = tip - dir * (l / 2.0);
            pt.path(&Path::ellipse(r(c.x - l / 2.0, c.y - w, l, 2.0 * w)), Some(if a.kind == 8 { col } else { bg }), Some(col), lw, solid, 1.0);
        }
        7 => {
            let c = tip - dir * (l / 2.0);
            let (a1, a2, b1, b2) = (tip + perp * w, back - perp * w, c + perp * w, c - perp * w);
            pt.path(
                &Path::new().move_to(a1.x, a1.y).line_to(a2.x, a2.y).move_to(b1.x, b1.y).line_to(b2.x, b2.y),
                None,
                Some(col),
                lw,
                solid,
                1.0,
            );
        }
        22 => pt.path(&Path::polygon(&[tip, back + perp * w, tip - dir * (l * 0.75), back - perp * w]), Some(col), Some(col), lw, solid, 1.0),
        _ => pt.path(&Path::polygon(&[tip, back + perp * w, back - perp * w]), Some(col), Some(col), lw, solid, 1.0),
    }
}

/// Dash pattern in diagram units, or None for solid.
pub fn dash_pattern(s: LineStyle, dash: f64) -> Option<Vec<f64>> {
    let d = dash.max(0.05);
    match s {
        LineStyle::Solid => None,
        LineStyle::Dashed => Some(vec![d, d]),
        LineStyle::DashDot => Some(vec![d, d * 0.45, d * 0.1, d * 0.45]),
        LineStyle::DashDotDot => Some(vec![d, d * 0.3, d * 0.1, d * 0.3, d * 0.1, d * 0.3]),
        LineStyle::Dotted => Some(vec![d * 0.1, d * 0.1]),
    }
}
