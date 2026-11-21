//! The diagram: shapes, lines, text, pictures and what they are hooked to.

use crate::dia::Node;
use crate::fonts;
use crate::geom::*;
use crate::shapes::{self, Family};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Col(pub u8, pub u8, pub u8, pub u8);

impl Col {
    pub const BLACK: Col = Col(0, 0, 0, 255);
    pub const WHITE: Col = Col(255, 255, 255, 255);
    pub const fn rgb(r: u8, g: u8, b: u8) -> Col {
        Col(r, g, b, 255)
    }
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
    pub fn parse(s: &str) -> Option<Col> {
        let s = s.trim().trim_start_matches('#');
        if s.len() < 6 {
            return None;
        }
        let h = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        let a = if s.len() >= 8 { h(6)? } else { 255 };
        Some(Col(h(0)?, h(2)?, h(4)?, a))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineStyle {
    Solid,
    Dashed,
    DashDot,
    DashDotDot,
    Dotted,
}

impl LineStyle {
    pub const ALL: [LineStyle; 5] = [LineStyle::Solid, LineStyle::Dashed, LineStyle::DashDot, LineStyle::DashDotDot, LineStyle::Dotted];
    pub fn from_i(i: i64) -> LineStyle {
        *LineStyle::ALL.get(i.clamp(0, 4) as usize).unwrap()
    }
    pub fn to_i(self) -> i64 {
        LineStyle::ALL.iter().position(|s| *s == self).unwrap() as i64
    }
    pub fn name(self) -> &'static str {
        ["Solid", "Dashed", "Dash-dot", "Dash-dot-dot", "Dotted"][self.to_i() as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineKind {
    Line,
    ZigZag,
    PolyLine,
    Arc,
    Bezier,
}

/// Font proportions relative to the text "height" (the line height), shared by every renderer.
pub const EM_FACTOR: f64 = 0.85;
pub const ASCENT_FACTOR: f64 = 0.79;

#[derive(Clone, Debug, PartialEq)]
pub struct TextSpec {
    pub text: String,
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    pub height: f64,
    pub color: Col,
    pub align: i64, // 0 left, 1 centre, 2 right
}

impl Default for TextSpec {
    fn default() -> Self {
        TextSpec { text: String::new(), family: "sans".into(), bold: false, italic: false, height: 0.8, color: Col::BLACK, align: 1 }
    }
}

impl TextSpec {
    pub fn lines(&self) -> Vec<&str> {
        self.text.split('\n').collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrowSpec {
    pub kind: i64, // 0 = none
    pub length: f64,
    pub width: f64,
}

impl Default for ArrowSpec {
    fn default() -> Self {
        ArrowSpec { kind: 0, length: 0.5, width: 0.5 }
    }
}

pub const ARROW_KINDS: &[(i64, &str)] = &[
    (0, "None"),
    (1, "Lines"),
    (2, "Hollow triangle"),
    (3, "Filled triangle"),
    (22, "Filled concave"),
    (4, "Hollow diamond"),
    (5, "Filled diamond"),
    (9, "Hollow dot"),
    (8, "Filled dot"),
    (10, "Double hollow triangle"),
    (11, "Double filled triangle"),
    (7, "Slashed cross"),
];

pub fn arrow_name(kind: i64) -> &'static str {
    ARROW_KINDS.iter().find(|k| k.0 == kind).map(|k| k.1).unwrap_or("Filled triangle")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attachment {
    pub target: u64,
    pub u: f64,
    pub v: f64,
    /// For class and table boxes: the numbered connection point (rows move as the box changes).
    pub cp: Option<usize>,
}

impl Attachment {
    pub fn center(target: u64) -> Self {
        Attachment { target, u: 0.5, v: 0.5, cp: None }
    }
    pub fn is_center(&self) -> bool {
        (self.u - 0.5).abs() < 1e-6 && (self.v - 0.5).abs() < 1e-6
    }
}

#[derive(Clone, Debug)]
pub struct Element {
    pub def: usize,
    pub rect: R,
    pub line_width: f64,
    pub line_color: Col,
    pub fill_color: Col,
    pub filled: bool,
    pub style: LineStyle,
    pub dash: f64,
    pub corner_radius: f64,
    pub shear_angle: f64,
    pub padding: f64,
    pub text: Option<TextSpec>,
    pub raw: Option<Node>,
    pub raw_type: String,
    /// Compartments, for UML classes and database tables.
    pub rec: Option<crate::record::Record>,
}

impl Element {
    pub fn new(def: usize, rect: R) -> Self {
        let d = shapes::def(def);
        Element {
            def,
            rect,
            line_width: 0.1,
            line_color: Col::BLACK,
            filled: true,
            style: LineStyle::Solid,
            dash: 1.0,
            corner_radius: d.def_corner,
            shear_angle: 45.0,
            padding: 0.25,
            text: if d.has_text { Some(TextSpec::default()) } else { None },
            raw: None,
            raw_type: String::new(),
            rec: match d.key {
                "uml-class" => Some(crate::record::Record::class()),
                "db-table" => Some(crate::record::Record::table()),
                _ => None,
            },
            fill_color: if d.solid.is_some() && d.key != "uml-final" { Col::BLACK } else { Col::WHITE },
        }
    }
    pub fn shape(&self) -> &'static shapes::ShapeDef {
        shapes::def(self.def)
    }
    pub fn outline(&self) -> Path {
        (self.shape().outline)(self)
    }
    pub fn cps(&self) -> Vec<P> {
        shapes::connection_points(self)
    }
    pub fn text_area(&self) -> R {
        let a = (self.shape().text_area)(self);
        let pd = self.padding;
        r(a.x + pd, a.y + pd, (a.w - 2.0 * pd).max(0.0), (a.h - 2.0 * pd).max(0.0))
    }
    /// Baseline of the first text line, as stored in .dia files.
    pub fn text_pos(&self) -> P {
        let Some(t) = &self.text else { return self.rect.center() };
        let a = self.text_area();
        let n = t.lines().len() as f64;
        let top = a.cy() - n * t.height / 2.0;
        let x = match t.align {
            0 => a.x,
            2 => a.right(),
            _ => a.cx(),
        };
        p(x, top + t.height * ASCENT_FACTOR)
    }
    /// Grow the shape (around its middle) so the text fits.
    pub fn fit_text(&mut self) {
        if self.rec.is_some() {
            crate::record::fit(self);
            return;
        }
        if self.shape().key == "uml-actor" {
            return;
        }
        let Some(t) = &self.text else { return };
        if t.text.is_empty() {
            return;
        }
        let (tw, th) = fonts::text_size(t);
        let area = (self.shape().text_area)(self);
        let (need_w, need_h) = (tw + 2.0 * self.padding, th + 2.0 * self.padding);
        let rw = if area.w > 0.0 { self.rect.w / area.w } else { 1.0 };
        let rh = if area.h > 0.0 { self.rect.h / area.h } else { 1.0 };
        let c = self.rect.center();
        if need_w > area.w + 1e-6 {
            self.rect.w = need_w * rw;
        }
        if need_h > area.h + 1e-6 {
            self.rect.h = need_h * rh;
        }
        self.rect.x = c.x - self.rect.w / 2.0;
        self.rect.y = c.y - self.rect.h / 2.0;
    }
    /// A copy with only what's needed for geometry.
    pub fn geometry_copy(&self) -> Element {
        Element { raw: None, raw_type: String::new(), ..self.clone() }
    }
}

#[derive(Clone, Debug)]
pub struct TextObj {
    pub pos: P,
    pub text: TextSpec,
}

impl TextObj {
    pub fn bounds(&self) -> R {
        let (w, h) = fonts::text_size(&self.text);
        let w = w.max(0.3);
        let x = match self.text.align {
            0 => self.pos.x,
            2 => self.pos.x - w,
            _ => self.pos.x - w / 2.0,
        };
        r(x, self.pos.y - self.text.height * ASCENT_FACTOR, w, h)
    }
}

#[derive(Clone, Debug)]
pub struct LineObj {
    pub kind: LineKind,
    pub pts: Vec<P>,
    pub curve: f64,
    pub start_arrow: ArrowSpec,
    pub end_arrow: ArrowSpec,
    pub color: Col,
    pub width: f64,
    pub style: LineStyle,
    pub dash: f64,
    pub corner_radius: f64,
    pub start_att: Option<Attachment>,
    pub end_att: Option<Attachment>,
    /// A name shown in the middle of the line (UML associations).
    pub label: String,
    /// UML lines from a file: the original object, saved back as the same kind.
    pub raw: Option<Node>,
}

impl LineObj {
    pub fn new(kind: LineKind, pts: Vec<P>) -> Self {
        LineObj {
            kind,
            pts,
            curve: 0.0,
            start_arrow: ArrowSpec::default(),
            end_arrow: ArrowSpec::default(),
            color: Col::BLACK,
            width: 0.1,
            style: LineStyle::Solid,
            dash: 1.0,
            corner_radius: 0.0,
            start_att: None,
            end_att: None,
            label: String::new(),
            raw: None,
        }
    }
    pub fn start(&self) -> P {
        self.pts[0]
    }
    pub fn end(&self) -> P {
        *self.pts.last().unwrap()
    }
    pub fn polyline(&self) -> Vec<P> {
        match self.kind {
            LineKind::Arc => arc_points(self.start(), self.end(), self.curve, 48),
            LineKind::Bezier => {
                let mut res = vec![self.pts[0]];
                let mut i = 1;
                while i + 2 < self.pts.len() {
                    for s in 1..=20 {
                        res.push(bez(self.pts[i - 1], self.pts[i], self.pts[i + 1], self.pts[i + 2], s as f64 / 20.0));
                    }
                    i += 3;
                }
                res
            }
            _ => self.pts.clone(),
        }
    }
    pub fn bounds(&self) -> R {
        let mut pts = self.polyline();
        if self.kind == LineKind::Bezier {
            pts.extend_from_slice(&self.pts);
        }
        let mut pad = self.width / 2.0;
        if self.start_arrow.kind != 0 {
            pad = pad.max(self.start_arrow.width);
        }
        if self.end_arrow.kind != 0 {
            pad = pad.max(self.end_arrow.width);
        }
        R::bounding(&pts).inflate(pad)
    }

    /// Move one end, keeping zigzag lines at right angles.
    pub fn set_end(&mut self, start: bool, q: P) {
        let n = self.pts.len();
        let (i, j) = if start { (0, 1) } else { (n - 1, n - 2) };
        match self.kind {
            LineKind::ZigZag => {
                if n == 2 {
                    self.pts[i] = q;
                    let (a, b) = (self.pts[0], self.pts[1]);
                    self.pts = orth_route(a, b, (b.x - a.x).abs() < (b.y - a.y).abs());
                    return;
                }
                let old = self.pts[i];
                let horiz = (old.y - self.pts[j].y).abs() <= (old.x - self.pts[j].x).abs();
                self.pts[i] = q;
                self.pts[j] = if horiz { p(self.pts[j].x, q.y) } else { p(q.x, self.pts[j].y) };
            }
            LineKind::Bezier => {
                let delta = q - self.pts[i];
                self.pts[i] = q;
                self.pts[j] = self.pts[j] + delta;
            }
            _ => self.pts[i] = q,
        }
    }
}

pub fn orth_route(a: P, b: P, start_vertical: bool) -> Vec<P> {
    if start_vertical {
        let my = (a.y + b.y) / 2.0;
        vec![a, p(a.x, my), p(b.x, my), b]
    } else {
        let mx = (a.x + b.x) / 2.0;
        vec![a, p(mx, a.y), p(mx, b.y), b]
    }
}

#[derive(Clone, Debug)]
pub struct PolygonObj {
    pub pts: Vec<P>,
    pub line_width: f64,
    pub line_color: Col,
    pub fill_color: Col,
    pub filled: bool,
    pub style: LineStyle,
    pub dash: f64,
}

#[derive(Clone, Debug)]
pub struct ImageObj {
    pub rect: R,
    pub file: String,
    pub data: Option<Arc<Vec<u8>>>,
    pub pic: Option<Arc<image::RgbaImage>>,
    pub inline: bool,
    pub keep_aspect: bool,
    pub draw_border: bool,
    pub line_width: f64,
    pub line_color: Col,
}

impl ImageObj {
    pub fn new(rect: R) -> Self {
        ImageObj {
            rect,
            file: String::new(),
            data: None,
            pic: None,
            inline: false,
            keep_aspect: true,
            draw_border: false,
            line_width: 0.1,
            line_color: Col::BLACK,
        }
    }
    /// Decode the picture bytes (from `data`, or read from `file`).
    pub fn load(&mut self) {
        if self.data.is_none() && !self.file.is_empty() {
            if let Ok(b) = std::fs::read(&self.file) {
                self.data = Some(Arc::new(b));
            }
        }
        if let Some(d) = &self.data {
            if let Ok(img) = image::load_from_memory(d) {
                self.pic = Some(Arc::new(img.to_rgba8()));
            }
        }
    }
}

/// An object kind we can't draw; kept as-is so saving doesn't lose it.
#[derive(Clone, Debug)]
pub struct RawObj {
    pub xml: Node,
    pub bb: R,
    pub type_name: String,
}

#[derive(Clone, Debug)]
pub enum Kind {
    Element(Element),
    Text(TextObj),
    Line(LineObj),
    Polygon(PolygonObj),
    Image(ImageObj),
    Raw(RawObj),
}

#[derive(Clone, Debug)]
pub struct Obj {
    pub id: u64,
    pub layer: String,
    pub orig_id: Option<String>,
    pub kind: Kind,
}

impl Obj {
    pub fn bounds(&self) -> R {
        match &self.kind {
            Kind::Element(e) => {
                let b = e.rect.inflate(e.line_width / 2.0);
                // An actor's name hangs below the figure.
                match &e.text {
                    Some(t) if e.shape().key == "uml-actor" && !t.text.is_empty() => {
                        let (w, h) = fonts::text_size(t);
                        let pos = e.text_pos();
                        b.union(&r(pos.x - w / 2.0, pos.y - t.height * ASCENT_FACTOR, w, h))
                    }
                    _ => b,
                }
            }
            Kind::Text(t) => t.bounds(),
            Kind::Line(l) => l.bounds(),
            Kind::Polygon(pg) => R::bounding(&pg.pts).inflate(pg.line_width / 2.0),
            Kind::Image(im) => im.rect.inflate(if im.draw_border { im.line_width / 2.0 } else { 0.0 }),
            Kind::Raw(rw) => rw.bb,
        }
    }

    pub fn move_by(&mut self, dx: f64, dy: f64) {
        let d = p(dx, dy);
        match &mut self.kind {
            Kind::Element(e) => {
                e.rect.x += dx;
                e.rect.y += dy;
            }
            Kind::Text(t) => t.pos = t.pos + d,
            Kind::Line(l) => l.pts.iter_mut().for_each(|q| *q = *q + d),
            Kind::Polygon(pg) => pg.pts.iter_mut().for_each(|q| *q = *q + d),
            Kind::Image(im) => {
                im.rect.x += dx;
                im.rect.y += dy;
            }
            Kind::Raw(rw) => {
                rw.bb.x += dx;
                rw.bb.y += dy;
                crate::dia::offset_raw(&mut rw.xml, dx, dy);
            }
        }
    }

    pub fn hit(&self, q: P, tol: f64) -> bool {
        match &self.kind {
            Kind::Element(e) => e.rect.inflate(tol).contains(q),
            Kind::Line(l) => dist_to_polyline(q, &l.polyline()) <= tol + l.width / 2.0,
            Kind::Polygon(pg) => {
                if pg.filled && point_in_polygon(q, &pg.pts) {
                    return true;
                }
                let mut closed = pg.pts.clone();
                closed.push(pg.pts[0]);
                dist_to_polyline(q, &closed) <= tol + pg.line_width / 2.0
            }
            _ => self.bounds().inflate(tol).contains(q),
        }
    }

    pub fn text(&self) -> Option<&TextSpec> {
        match &self.kind {
            Kind::Element(e) if e.rec.is_some() => None,
            Kind::Element(e) => e.text.as_ref(),
            Kind::Text(t) => Some(&t.text),
            _ => None,
        }
    }

    pub fn text_mut(&mut self) -> Option<&mut TextSpec> {
        match &mut self.kind {
            Kind::Element(e) => e.text.as_mut(),
            Kind::Text(t) => Some(&mut t.text),
            _ => None,
        }
    }

    /// The box used for aligning objects.
    pub fn align_box(&self) -> R {
        match &self.kind {
            Kind::Element(e) => e.rect,
            Kind::Image(im) => im.rect,
            _ => self.bounds(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
}

#[derive(Clone, Debug)]
pub struct Paper {
    pub name: String,
    pub portrait: bool,
    pub tm: f64,
    pub bm: f64,
    pub lm: f64,
    pub rm: f64,
    pub scaling: f64,
}

pub const PAPER_SIZES: &[(&str, f64, f64)] =
    &[("A3", 29.7, 42.0), ("A4", 21.0, 29.7), ("A5", 14.8, 21.0), ("Letter", 21.59, 27.94), ("Legal", 21.59, 35.56)];

impl Default for Paper {
    fn default() -> Self {
        Paper { name: "A4".into(), portrait: true, tm: 2.82, bm: 2.82, lm: 2.82, rm: 2.82, scaling: 1.0 }
    }
}

impl Paper {
    pub fn size(&self) -> (f64, f64) {
        let s = PAPER_SIZES.iter().find(|s| s.0.eq_ignore_ascii_case(&self.name)).unwrap_or(&PAPER_SIZES[1]);
        if self.portrait {
            (s.1, s.2)
        } else {
            (s.2, s.1)
        }
    }
    /// One page of the diagram, in diagram units.
    pub fn page_w(&self) -> f64 {
        ((self.size().0 - self.lm - self.rm) / self.scaling.max(0.01)).max(1.0)
    }
    pub fn page_h(&self) -> f64 {
        ((self.size().1 - self.tm - self.bm) / self.scaling.max(0.01)).max(1.0)
    }
}

#[derive(Clone, Debug)]
pub struct Diagram {
    pub objects: Vec<Obj>,
    pub layers: Vec<Layer>,
    pub background: Col,
    pub paper: Paper,
    pub grid: f64,
    pub next_id: u64,
    /// The layer new objects go on.
    pub active: usize,
}

impl Default for Diagram {
    fn default() -> Self {
        Diagram {
            objects: Vec::new(),
            layers: vec![Layer { name: "Background".into(), visible: true }],
            background: Col::WHITE,
            paper: Paper::default(),
            grid: 1.0,
            next_id: 1,
            active: 0,
        }
    }
}

impl Diagram {
    pub fn add(&mut self, kind: Kind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let layer = self.layers.get(self.active).or(self.layers.first()).map(|l| l.name.clone()).unwrap_or_else(|| "Background".into());
        self.objects.push(Obj { id, layer, orig_id: None, kind });
        id
    }
    pub fn get(&self, id: u64) -> Option<&Obj> {
        self.objects.iter().find(|o| o.id == id)
    }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Obj> {
        self.objects.iter_mut().find(|o| o.id == id)
    }
    pub fn element(&self, id: u64) -> Option<&Element> {
        match &self.get(id)?.kind {
            Kind::Element(e) => Some(e),
            _ => None,
        }
    }
    pub fn layer_visible(&self, name: &str) -> bool {
        self.layers.iter().find(|l| l.name == name).map(|l| l.visible).unwrap_or(true)
    }
    pub fn visible(&self) -> impl Iterator<Item = &Obj> {
        self.objects.iter().filter(|o| self.layer_visible(&o.layer))
    }
    pub fn extents(&self) -> Option<R> {
        let mut res: Option<R> = None;
        for o in self.visible() {
            let b = o.bounds();
            res = Some(match res {
                Some(r0) => r0.union(&b),
                None => b,
            });
        }
        res
    }

    /// Re-route every line that is hooked to a shape.
    pub fn update_connections(&mut self) {
        for i in 0..self.objects.len() {
            let (sa, ea) = match &self.objects[i].kind {
                Kind::Line(l) => (l.start_att, l.end_att),
                _ => continue,
            };
            if sa.is_none() && ea.is_none() {
                continue;
            }
            let se = sa.and_then(|a| self.element(a.target).map(|e| e.geometry_copy()));
            let ee = ea.and_then(|a| self.element(a.target).map(|e| e.geometry_copy()));
            if let Kind::Line(l) = &mut self.objects[i].kind {
                if se.is_none() {
                    l.start_att = None;
                }
                if ee.is_none() {
                    l.end_att = None;
                }
                update_line(l, se.as_ref(), ee.as_ref());
            }
        }
    }

    /// Copy objects with fresh ids; lines stay hooked to shapes copied with them.
    pub fn clone_objects(&mut self, objs: &[Obj]) -> Vec<Obj> {
        let mut map = std::collections::HashMap::new();
        let mut res: Vec<Obj> = Vec::new();
        for o in objs {
            let mut c = o.clone();
            c.id = self.next_id;
            self.next_id += 1;
            c.orig_id = None;
            map.insert(o.id, c.id);
            res.push(c);
        }
        for o in res.iter_mut() {
            if let Kind::Line(l) = &mut o.kind {
                l.start_att = l.start_att.and_then(|a| map.get(&a.target).map(|t| Attachment { target: *t, ..a }));
                l.end_att = l.end_att.and_then(|a| map.get(&a.target).map(|t| Attachment { target: *t, ..a }));
            }
        }
        res
    }
}

/// Where a line end attached to a shape should be. Middle attachments end at the outline.
pub fn attach_point(e: &Element, a: &Attachment, toward: P) -> P {
    if let (Some(k), Some(_)) = (a.cp, &e.rec) {
        let cps = e.cps();
        if k + 1 < cps.len() {
            return cps[k];
        }
    }
    if !a.is_center() {
        return e.rect.at(a.u, a.v);
    }
    let c = e.rect.center();
    let mut best = 2.0;
    for (pts, closed) in e.outline().flatten(12) {
        let n = pts.len();
        let m = if closed { n } else { n.saturating_sub(1) };
        for i in 0..m {
            if let Some(t) = seg_intersect(c, toward, pts[i], pts[(i + 1) % n]) {
                if t < best {
                    best = t;
                }
            }
        }
    }
    if best > 1.0 {
        c
    } else {
        c + (toward - c) * best
    }
}

fn update_line(l: &mut LineObj, se: Option<&Element>, ee: Option<&Element>) {
    let end_ref = match (ee, &l.end_att) {
        (Some(e), Some(a)) if a.is_center() => e.rect.center(),
        (Some(e), Some(a)) => attach_point(e, a, l.end()),
        _ => l.end(),
    };
    let start_ref = match (se, &l.start_att) {
        (Some(e), Some(a)) if a.is_center() => e.rect.center(),
        (Some(e), Some(a)) => attach_point(e, a, l.start()),
        _ => l.start(),
    };
    let simple = matches!(l.kind, LineKind::Line | LineKind::Arc) || l.pts.len() == 2;
    if let (Some(e), Some(a)) = (se, l.start_att) {
        let toward = if simple { end_ref } else { neighbour_toward(l, true, start_ref) };
        l.set_end(true, attach_point(e, &a, toward));
    }
    if let (Some(e), Some(a)) = (ee, l.end_att) {
        let toward = if simple {
            if l.start_att.is_some() {
                l.start()
            } else {
                start_ref
            }
        } else {
            neighbour_toward(l, false, end_ref)
        };
        l.set_end(false, attach_point(e, &a, toward));
    }
}

fn neighbour_toward(l: &LineObj, start: bool, self_ref: P) -> P {
    let n = l.pts.len();
    let nb = if start { l.pts[1] } else { l.pts[n - 2] };
    if l.kind == LineKind::ZigZag && n >= 3 {
        let old = if start { l.pts[0] } else { l.pts[n - 1] };
        let horiz = (old.y - nb.y).abs() <= (old.x - nb.x).abs();
        return if horiz { p(nb.x, self_ref.y) } else { p(self_ref.x, nb.y) };
    }
    nb
}

/// Colours and widths for new objects (the toolbox's attribute area).
#[derive(Clone, Debug)]
pub struct DefaultStyle {
    pub line_color: Col,
    pub fill_color: Col,
    pub line_width: f64,
    pub style: LineStyle,
    pub start_arrow: i64,
    pub end_arrow: i64,
    pub text_height: f64,
}

impl Default for DefaultStyle {
    fn default() -> Self {
        DefaultStyle {
            line_color: Col::BLACK,
            fill_color: Col::WHITE,
            line_width: 0.1,
            style: LineStyle::Solid,
            start_arrow: 0,
            end_arrow: 22,
            text_height: 0.8,
        }
    }
}

pub fn is_unknown(e: &Element) -> bool {
    e.shape().fam == Family::Unknown
}
