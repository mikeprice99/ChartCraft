//! Editing a diagram with the mouse and keyboard (independent of the window toolkit).

use crate::geom::*;
use crate::model::*;
use crate::shapes::{self, Family};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Magnify,
    Scroll,
    Text,
    Box,
    Ellipse,
    Polygon,
    Line,
    Arc,
    ZigZag,
    PolyLine,
    Bezier,
    Image,
    TextEdit,
    Shape(usize),
    /// A ready-made connector from a sheet (UML relationships...).
    Connector(usize),
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}

#[derive(Clone, Debug)]
enum Drag {
    None,
    Move { anchor: P, applied: P },
    Handle { id: u64, handle: usize, creating: bool },
    Rubber,
    CreateElement { id: u64 },
    CreatePolygon { id: u64 },
}

pub struct TextEdit {
    pub id: u64,
    pub text: String,
    pub before: Diagram,
    pub just_started: bool,
}

pub const BASE_SCALE: f64 = 20.0; // screen points per centimetre at 100%

pub struct Editor {
    pub d: Diagram,
    pub sel: Vec<u64>,
    pub tool: Tool,
    pub sticky: bool,
    pub style: DefaultStyle,
    pub zoom: f64,
    pub origin: P,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub show_cps: bool,
    pub show_pages: bool,
    pub show_rulers: bool,
    pub object_snap: bool,
    undo: Vec<Diagram>,
    redo: Vec<Diagram>,
    pub rev: u64,
    pub modified: bool,
    drag: Drag,
    drag_start: P,
    drag_changed: bool,
    pending: Option<Diagram>,
    pub rubber: Option<R>,
    pub snap_target: Option<u64>,
    pub snap_point: Option<P>,
    pub editing: Option<TextEdit>,
    pub(crate) clipboard: Vec<Obj>,
    paste_count: u32,
    pub image_request: Option<P>,
    pub props_request: bool,
    pub context_world: P,
}

// Handle ids: 0..7 box handles (or 0 start / 1 end for lines); 100+i points; 200+k zigzag segments; 300 arc middle.
pub const H_POINT: usize = 100;
pub const H_SEG: usize = 200;
pub const H_ARC: usize = 300;

impl Editor {
    pub fn new() -> Self {
        Editor {
            d: Diagram::default(),
            sel: vec![],
            tool: Tool::Select,
            sticky: false,
            style: DefaultStyle::default(),
            zoom: 1.0,
            origin: p(0.0, 0.0),
            show_grid: true,
            snap_to_grid: false,
            show_cps: true,
            show_pages: true,
            show_rulers: true,
            object_snap: true,
            undo: vec![],
            redo: vec![],
            rev: 1,
            modified: false,
            drag: Drag::None,
            drag_start: P::default(),
            drag_changed: false,
            pending: None,
            rubber: None,
            snap_target: None,
            snap_point: None,
            editing: None,
            clipboard: vec![],
            paste_count: 0,
            image_request: None,
            props_request: false,
            context_world: P::default(),
        }
    }

    pub fn scale(&self) -> f64 {
        BASE_SCALE * self.zoom
    }

    pub fn touch(&mut self) {
        self.rev += 1;
    }

    fn changed(&mut self) {
        for o in self.d.objects.iter_mut() {
            if let Kind::Element(e) = &mut o.kind {
                if e.rec.is_some() {
                    crate::record::fit(e);
                }
            }
        }
        self.d.update_connections();
        self.modified = true;
        self.touch();
    }

    pub fn load(&mut self, d: Diagram) {
        self.editing = None;
        self.d = d;
        self.undo.clear();
        self.redo.clear();
        self.sel.clear();
        self.zoom = 1.0;
        self.origin = match self.d.extents() {
            Some(e) => p((e.x - 1.0).min(0.0), (e.y - 1.0).min(0.0)),
            None => p(0.0, 0.0),
        };
        self.modified = false;
        self.touch();
    }

    // ------------------------------------------------------------ undo

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn checkpoint(&mut self) {
        let snap = self.d.clone();
        self.push_undo(snap);
    }
    fn push_undo(&mut self, d: Diagram) {
        self.undo.push(d);
        self.redo.clear();
        if self.undo.len() > 200 {
            self.undo.drain(0..50);
        }
    }
    pub fn undo(&mut self) {
        self.end_edit(true);
        if let Some(prev) = self.undo.pop() {
            let cur = std::mem::replace(&mut self.d, prev);
            self.redo.push(cur);
            self.sel.clear();
            self.changed();
        }
    }
    pub fn redo(&mut self) {
        self.end_edit(true);
        if let Some(next) = self.redo.pop() {
            let cur = std::mem::replace(&mut self.d, next);
            self.undo.push(cur);
            self.sel.clear();
            self.changed();
        }
    }

    // ------------------------------------------------------------ queries

    pub fn object_at(&self, w: P) -> Option<u64> {
        let tol = 5.0 / self.scale();
        let near_end = 10.0 / self.scale();
        let hits: Vec<&Obj> = self.d.objects.iter().rev().filter(|o| self.d.layer_visible(&o.layer) && o.hit(w, tol)).collect();
        // A connector's hooked-on end sits on its shape: a click there means the shape,
        // so dragging moves the shape (and the line follows) instead of tearing the line off.
        let on_hooked_end = |o: &Obj| match &o.kind {
            Kind::Line(l) => (l.start_att.is_some() && l.start().dist(w) <= near_end) || (l.end_att.is_some() && l.end().dist(w) <= near_end),
            _ => false,
        };
        if let Some(first) = hits.first() {
            if on_hooked_end(first) {
                if let Some(shape) = hits.iter().find(|o| matches!(o.kind, Kind::Element(_))) {
                    return Some(shape.id);
                }
            }
        }
        hits.first().map(|o| o.id)
    }

    pub fn handles(o: &Obj) -> Vec<(P, usize)> {
        let box_handles = |b: R| {
            vec![
                (b.at(0., 0.), 0),
                (b.at(0.5, 0.), 1),
                (b.at(1., 0.), 2),
                (b.at(0., 0.5), 3),
                (b.at(1., 0.5), 4),
                (b.at(0., 1.), 5),
                (b.at(0.5, 1.), 6),
                (b.at(1., 1.), 7),
            ]
        };
        match &o.kind {
            Kind::Element(e) => box_handles(e.rect),
            Kind::Image(im) => box_handles(im.rect),
            Kind::Line(l) => {
                let mut v = vec![(l.start(), 0), (l.end(), 1)];
                match l.kind {
                    LineKind::PolyLine | LineKind::Bezier => {
                        for i in 1..l.pts.len() - 1 {
                            v.push((l.pts[i], H_POINT + i));
                        }
                    }
                    LineKind::ZigZag => {
                        for k in 1..l.pts.len().saturating_sub(2) {
                            v.push(((l.pts[k] + l.pts[k + 1]) * 0.5, H_SEG + k));
                        }
                    }
                    LineKind::Arc => {
                        let pts = l.polyline();
                        v.push((pts[pts.len() / 2], H_ARC));
                    }
                    LineKind::Line => {}
                }
                v
            }
            Kind::Polygon(pg) => pg.pts.iter().enumerate().map(|(i, q)| (*q, H_POINT + i)).collect(),
            _ => vec![],
        }
    }

    fn handle_at(&self, w: P) -> Option<(u64, usize)> {
        let tol = 6.0 / self.scale();
        for id in self.sel.iter().rev() {
            if let Some(o) = self.d.get(*id) {
                for (q, h) in Editor::handles(o) {
                    if (q.x - w.x).abs() <= tol && (q.y - w.y).abs() <= tol {
                        return Some((*id, h));
                    }
                }
            }
        }
        None
    }

    /// A shape to hook a line end to: (shape id, attachment, point).
    fn find_connection(&self, w: P, exclude: u64) -> Option<(u64, Attachment, P)> {
        if !self.object_snap {
            return None;
        }
        let tol = 8.0 / self.scale();
        for o in self.d.objects.iter().rev() {
            if o.id == exclude || !self.d.layer_visible(&o.layer) {
                continue;
            }
            let Kind::Element(e) = &o.kind else { continue };
            if !e.rect.inflate(tol).contains(w) {
                continue;
            }
            let cps = e.cps();
            let mut best: Option<usize> = None;
            let mut bd = tol;
            for (k, c) in cps.iter().enumerate().take(cps.len() - 1) {
                let dd = c.dist(w);
                if dd <= bd {
                    bd = dd;
                    best = Some(k);
                }
            }
            if let Some(k) = best {
                let c = cps[k];
                let mut at = Attachment {
                    target: o.id,
                    u: if e.rect.w > 0.0 { (c.x - e.rect.x) / e.rect.w } else { 0.5 },
                    v: if e.rect.h > 0.0 { (c.y - e.rect.y) / e.rect.h } else { 0.5 },
                    cp: e.rec.as_ref().map(|_| k),
                };
                if at.is_center() {
                    at.u += 1e-4;
                }
                return Some((o.id, at, c));
            }
            // Anywhere else on the shape: lock to its nearest connection point, so the
            // line stays at that spot when things move.
            let k = (0..cps.len() - 1).min_by(|&i, &j| cps[i].dist(w).partial_cmp(&cps[j].dist(w)).unwrap()).unwrap_or(0);
            let c = cps[k];
            let mut at = Attachment {
                target: o.id,
                u: if e.rect.w > 0.0 { (c.x - e.rect.x) / e.rect.w } else { 0.5 },
                v: if e.rect.h > 0.0 { (c.y - e.rect.y) / e.rect.h } else { 0.5 },
                cp: e.rec.as_ref().map(|_| k),
            };
            if at.is_center() {
                at.u += 1e-4;
            }
            return Some((o.id, at, c));
        }
        None
    }

    fn snap_w(&self, w: P) -> P {
        if self.snap_to_grid {
            snap(w, self.d.grid)
        } else {
            w
        }
    }

    pub fn cursor_hint(&self, w: P) -> Option<usize> {
        if self.tool != Tool::Select {
            return None;
        }
        self.handle_at(w).map(|h| h.1)
    }

    fn select_only(&mut self, id: u64) {
        self.sel.clear();
        self.sel.push(id);
    }

    fn revert_tool(&mut self) {
        if !self.sticky {
            self.tool = Tool::Select;
        }
    }

    // ------------------------------------------------------------ mouse

    pub fn mouse_down(&mut self, w: P, mods: Mods) {
        self.end_edit(true);
        self.drag_start = w;
        self.drag_changed = false;
        if self.tool == Tool::Magnify {
            return;
        }
        if self.tool == Tool::TextEdit {
            if let Some(id) = self.object_at(w) {
                if self.d.get(id).and_then(|o| o.text()).is_some() {
                    self.select_only(id);
                    self.begin_edit(id, None);
                }
            }
            return;
        }
        if self.tool != Tool::Select {
            self.start_create(w);
            return;
        }
        if let Some((id, h)) = self.handle_at(w) {
            self.pending = Some(self.d.clone());
            self.drag = Drag::Handle { id, handle: h, creating: false };
            return;
        }
        let add = mods.shift || mods.ctrl;
        if let Some(id) = self.object_at(w) {
            if add {
                if let Some(i) = self.sel.iter().position(|s| *s == id) {
                    self.sel.remove(i);
                    return;
                }
                self.sel.push(id);
            } else if !self.sel.contains(&id) {
                self.select_only(id);
            }
            self.pending = Some(self.d.clone());
            let anchor = self.d.get(self.sel[0]).map(|o| {
                let b = o.align_box();
                p(b.x, b.y)
            });
            self.drag = Drag::Move { anchor: anchor.unwrap_or(w), applied: P::default() };
            return;
        }
        if !add {
            self.sel.clear();
        }
        self.drag = Drag::Rubber;
        self.rubber = Some(r(w.x, w.y, 0.0, 0.0));
    }

    pub fn right_click(&mut self, w: P) {
        self.end_edit(true);
        self.context_world = w;
        match self.object_at(w) {
            Some(id) if !self.sel.contains(&id) => self.select_only(id),
            None => self.sel.clear(),
            _ => {}
        }
    }

    fn start_create(&mut self, w: P) {
        self.pending = Some(self.d.clone());
        let sw = self.snap_w(w);
        let st = self.style.clone();
        match self.tool {
            Tool::Shape(_) | Tool::Box | Tool::Ellipse => {
                let def = match self.tool {
                    Tool::Box => shapes::index_of("std-box"),
                    Tool::Ellipse => shapes::index_of("std-ellipse"),
                    Tool::Shape(i) => i,
                    _ => unreachable!(),
                };
                let sd = shapes::def(def);
                let mut e = Element::new(def, r(sw.x, sw.y, sd.def_w, sd.def_h));
                e.line_color = st.line_color;
                e.fill_color = st.fill_color;
                e.line_width = st.line_width;
                e.style = st.style;
                if let Some(t) = &mut e.text {
                    t.color = st.line_color;
                    t.height = st.text_height;
                }
                let id = self.d.add(Kind::Element(e));
                self.select_only(id);
                self.drag = Drag::CreateElement { id };
                self.drag_start = sw;
            }
            Tool::Polygon => {
                let pg = PolygonObj {
                    pts: triangle(r(sw.x, sw.y, 3.0, 3.0)),
                    line_width: st.line_width,
                    line_color: st.line_color,
                    fill_color: st.fill_color,
                    filled: true,
                    style: st.style,
                    dash: 1.0,
                };
                let id = self.d.add(Kind::Polygon(pg));
                self.select_only(id);
                self.drag = Drag::CreatePolygon { id };
                self.drag_start = sw;
            }
            Tool::Text => {
                self.checkpoint();
                let text = TextSpec { color: st.line_color, height: st.text_height, align: 0, ..Default::default() };
                let id = self.d.add(Kind::Text(TextObj { pos: p(sw.x, sw.y + st.text_height * ASCENT_FACTOR), text }));
                self.select_only(id);
                self.revert_tool();
                self.changed();
                self.begin_edit(id, None);
                self.pending = None;
            }
            Tool::Image => {
                self.image_request = Some(sw);
                self.revert_tool();
                self.pending = None;
            }
            _ => {
                let kind = match self.tool {
                    Tool::Arc => LineKind::Arc,
                    Tool::ZigZag => LineKind::ZigZag,
                    Tool::PolyLine => LineKind::PolyLine,
                    Tool::Bezier => LineKind::Bezier,
                    Tool::Connector(_) => LineKind::ZigZag,
                    _ => LineKind::Line,
                };
                let mut l = LineObj::new(kind, vec![sw, sw]);
                l.color = st.line_color;
                l.width = st.line_width;
                l.style = st.style;
                l.start_arrow.kind = st.start_arrow;
                l.end_arrow.kind = st.end_arrow;
                if let Tool::Connector(i) = self.tool {
                    let pr = &shapes::LINE_PRESETS[i];
                    l.start_arrow.kind = pr.start;
                    l.end_arrow.kind = pr.end;
                    l.style = if pr.dashed { LineStyle::Dashed } else { LineStyle::Solid };
                    l.dash = 0.4;
                    if pr.start == 4 || pr.start == 5 {
                        l.start_arrow.length = 0.8;
                    }
                }
                if let Some((_, at, q)) = self.find_connection(w, 0) {
                    l.pts = vec![q, q];
                    l.start_att = Some(at);
                }
                let id = self.d.add(Kind::Line(l));
                self.select_only(id);
                self.drag = Drag::Handle { id, handle: 1, creating: true };
            }
        }
        self.drag_changed = true;
        self.touch();
    }

    pub fn mouse_move(&mut self, w: P, mods: Mods) {
        match self.drag.clone() {
            Drag::None => {}
            Drag::Rubber => {
                self.rubber = Some(R::from_points(self.drag_start, w));
            }
            Drag::Move { anchor, applied } => {
                let total = w - self.drag_start;
                if !self.drag_changed && total.len() * self.scale() < 3.0 {
                    return;
                }
                let target = self.snap_w(anchor + total);
                let now = target - anchor;
                let inc = now - applied;
                if inc.x.abs() < 1e-9 && inc.y.abs() < 1e-9 {
                    return;
                }
                if !self.drag_changed {
                    if let Some(pd) = self.pending.take() {
                        self.push_undo(pd);
                    }
                    self.drag_changed = true;
                }
                self.move_selection(inc.x, inc.y);
                self.drag = Drag::Move { anchor, applied: now };
            }
            Drag::Handle { id, handle, creating } => {
                if !self.drag_changed && !creating && (w - self.drag_start).len() * self.scale() < 2.0 {
                    return;
                }
                if !self.drag_changed {
                    if let Some(pd) = self.pending.take() {
                        self.push_undo(pd);
                    }
                    self.drag_changed = true;
                }
                self.apply_handle(id, handle, w, creating, false, mods);
                self.d.update_connections();
                self.touch();
            }
            Drag::CreateElement { id } => {
                let sw = self.snap_w(w);
                if (sw - self.drag_start).len() * self.scale() > 4.0 {
                    let b = R::from_points(self.drag_start, sw);
                    if let Some(Kind::Element(e)) = self.d.get_mut(id).map(|o| &mut o.kind) {
                        e.rect = r(b.x, b.y, b.w.max(0.2), b.h.max(0.2));
                    }
                    self.touch();
                }
            }
            Drag::CreatePolygon { id } => {
                let sw = self.snap_w(w);
                if (sw - self.drag_start).len() * self.scale() > 4.0 {
                    let b = R::from_points(self.drag_start, sw);
                    if let Some(Kind::Polygon(pg)) = self.d.get_mut(id).map(|o| &mut o.kind) {
                        pg.pts = triangle(b);
                    }
                    self.touch();
                }
            }
        }
    }

    pub fn mouse_up(&mut self, w: P) {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Rubber => {
                let b = R::from_points(self.drag_start, w);
                let ids: Vec<u64> = self.d.visible().filter(|o| b.contains_rect(&o.bounds())).map(|o| o.id).collect();
                for id in ids {
                    if !self.sel.contains(&id) {
                        self.sel.push(id);
                    }
                }
                self.rubber = None;
            }
            Drag::Move { .. } => {
                if self.drag_changed {
                    self.changed();
                }
            }
            Drag::Handle { id, creating, .. } => {
                if creating {
                    let (start, end) = match self.d.get(id).map(|o| &o.kind) {
                        Some(Kind::Line(l)) => (l.start(), l.end()),
                        _ => (w, w),
                    };
                    if start.dist(end) < 0.3 {
                        // A click without a drag: a line of default length.
                        self.apply_handle(id, 1, start + p(3.0, 0.0), true, true, Mods::default());
                    }
                    if let Some(Kind::Line(l)) = self.d.get_mut(id).map(|o| &mut o.kind) {
                        if l.kind == LineKind::Arc && l.curve.abs() < 1e-6 {
                            l.curve = l.start().dist(l.end()) * 0.2;
                        }
                    }
                    if let Some(pd) = self.pending.take() {
                        self.push_undo(pd);
                    }
                    self.revert_tool();
                }
                self.snap_target = None;
                self.snap_point = None;
                if self.drag_changed {
                    self.changed();
                }
            }
            Drag::CreateElement { id } | Drag::CreatePolygon { id } => {
                if let Some(pd) = self.pending.take() {
                    self.push_undo(pd);
                }
                self.revert_tool();
                self.changed();
                let has_text = matches!(self.d.get(id).map(|o| &o.kind), Some(Kind::Element(e)) if e.text.is_some() && e.rec.is_none());
                let is_record = matches!(self.d.get(id).map(|o| &o.kind), Some(Kind::Element(e)) if e.rec.is_some());
                if is_record {
                    // A new class or table opens its editor straight away.
                    self.props_request = true;
                } else if has_text {
                    self.begin_edit(id, None);
                }
            }
            Drag::None => {}
        }
        self.pending = None;
        self.touch();
    }

    pub fn double_click(&mut self, w: P) {
        if self.tool != Tool::Select {
            return;
        }
        let Some(id) = self.object_at(w) else { return };
        self.select_only(id);
        let has_text = self.d.get(id).and_then(|o| o.text()).is_some();
        if has_text {
            self.begin_edit(id, None);
        } else {
            self.props_request = true;
        }
    }

    fn move_selection(&mut self, dx: f64, dy: f64) {
        let sel = self.sel.clone();
        for o in self.d.objects.iter_mut() {
            if !sel.contains(&o.id) {
                continue;
            }
            o.move_by(dx, dy);
            if let Kind::Line(l) = &mut o.kind {
                // A connector stays locked to its shapes. It only comes loose when it is
                // dragged by itself (none of the shapes it is hooked to are moving with it).
                let moving_with_shape = [l.start_att, l.end_att].iter().flatten().any(|a| sel.contains(&a.target));
                if !moving_with_shape {
                    l.start_att = None;
                    l.end_att = None;
                }
            }
        }
        self.d.update_connections();
        self.touch();
    }

    fn apply_handle(&mut self, id: u64, h: usize, w: P, creating: bool, no_snap: bool, mods: Mods) {
        let sw = if no_snap { w } else { self.snap_w(w) };
        let conn = if no_snap { None } else { self.find_connection(w, id) };
        let start_geo = match self.d.get(id).map(|o| &o.kind) {
            Some(Kind::Line(l)) => l.start_att.and_then(|a| self.d.element(a.target).map(|e| (a, e.geometry_copy()))),
            _ => None,
        };
        let Some(o) = self.d.get_mut(id) else { return };
        match &mut o.kind {
            Kind::Element(e) => e.rect = resize_rect(e.rect, h, sw, false),
            Kind::Image(im) => im.rect = resize_rect(im.rect, h, sw, im.keep_aspect && !mods.shift),
            Kind::Polygon(pg) if h >= H_POINT => {
                if let Some(q) = pg.pts.get_mut(h - H_POINT) {
                    *q = sw;
                }
            }
            Kind::Line(l) => {
                if h == 0 || h == 1 {
                    let start = h == 0;
                    let q = match conn {
                        Some((tid, at, q)) => {
                            self.snap_target = Some(tid);
                            self.snap_point = if at.is_center() { None } else { Some(q) };
                            if start {
                                l.start_att = Some(at);
                            } else {
                                l.end_att = Some(at);
                            }
                            q
                        }
                        None => {
                            self.snap_target = None;
                            self.snap_point = None;
                            if start {
                                l.start_att = None;
                            } else {
                                l.end_att = None;
                            }
                            sw
                        }
                    };
                    if creating {
                        shape_new_line(l, q, start_geo.as_ref());
                    } else {
                        l.set_end(start, q);
                    }
                } else if h >= H_ARC {
                    let (a, b) = (l.start(), l.end());
                    let dir = (b - a).norm();
                    let perp = p(dir.y, -dir.x);
                    let mid = (a + b) * 0.5;
                    l.curve = (w.x - mid.x) * perp.x + (w.y - mid.y) * perp.y;
                } else if h >= H_SEG {
                    let k = h - H_SEG;
                    if k + 1 < l.pts.len() {
                        let (a, b) = (l.pts[k], l.pts[k + 1]);
                        if (a.y - b.y).abs() <= (a.x - b.x).abs() {
                            l.pts[k] = p(a.x, sw.y);
                            l.pts[k + 1] = p(b.x, sw.y);
                        } else {
                            l.pts[k] = p(sw.x, a.y);
                            l.pts[k + 1] = p(sw.x, b.y);
                        }
                    }
                } else if h >= H_POINT {
                    if let Some(q) = l.pts.get_mut(h - H_POINT) {
                        *q = sw;
                    }
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ text editing

    pub fn begin_edit(&mut self, id: u64, append: Option<&str>) {
        self.end_edit(true);
        let Some(t) = self.d.get(id).and_then(|o| o.text()) else { return };
        let mut text = t.text.clone();
        if let Some(a) = append {
            text.push_str(a);
        }
        self.editing = Some(TextEdit { id, text, before: self.d.clone(), just_started: true });
        self.touch();
    }

    pub fn end_edit(&mut self, commit: bool) {
        let Some(ed) = self.editing.take() else { return };
        self.touch();
        if !commit {
            return;
        }
        let text = ed.text.replace("\r\n", "\n");
        let Some(o) = self.d.get(ed.id) else { return };
        let is_text_obj = matches!(o.kind, Kind::Text(_));
        let same = o.text().map(|t| t.text == text).unwrap_or(true);
        if same && !(is_text_obj && text.trim().is_empty()) {
            return;
        }
        self.push_undo(ed.before);
        if is_text_obj && text.trim().is_empty() {
            self.d.objects.retain(|o| o.id != ed.id);
            self.sel.retain(|s| *s != ed.id);
        } else if let Some(o) = self.d.get_mut(ed.id) {
            if let Some(t) = o.text_mut() {
                t.text = text;
            }
            if let Kind::Element(e) = &mut o.kind {
                e.fit_text();
            }
        }
        self.changed();
    }

    // ------------------------------------------------------------ commands

    pub fn delete_selection(&mut self) {
        if self.sel.is_empty() {
            return;
        }
        self.checkpoint();
        let sel = std::mem::take(&mut self.sel);
        self.d.objects.retain(|o| !sel.contains(&o.id));
        self.changed();
    }

    pub fn selected_objects(&self) -> Vec<Obj> {
        self.d.objects.iter().filter(|o| self.sel.contains(&o.id)).cloned().collect()
    }

    pub fn copy(&mut self) -> Option<String> {
        if self.sel.is_empty() {
            return None;
        }
        self.clipboard = self.selected_objects();
        self.paste_count = 0;
        let refs: Vec<&Obj> = self.d.objects.iter().filter(|o| self.sel.contains(&o.id)).collect();
        Some(crate::dia::to_xml_string(&self.d, &refs))
    }

    pub fn cut(&mut self) -> Option<String> {
        let s = self.copy();
        self.delete_selection();
        s
    }

    pub fn can_paste(&self) -> bool {
        !self.clipboard.is_empty()
    }

    /// Paste our own clipboard, or objects from .dia text copied elsewhere.
    pub fn paste(&mut self, text: Option<&str>) {
        let external: Option<Vec<Obj>> = text.filter(|t| t.contains("dia:diagram")).and_then(|t| crate::dia::parse(t.as_bytes()).ok()).map(|d| d.objects);
        let from_ours = match (&external, self.clipboard.is_empty()) {
            (Some(ext), false) => {
                // Same objects as our own clipboard? Then use ours (keeps the paste offset going).
                ext.len() == self.clipboard.len()
            }
            (None, false) => true,
            _ => false,
        };
        let source: Vec<Obj> = if from_ours {
            self.clipboard.clone()
        } else if let Some(ext) = external {
            self.clipboard = ext.clone();
            self.paste_count = 0;
            ext
        } else {
            return;
        };
        self.end_edit(true);
        self.checkpoint();
        self.paste_count += 1;
        let k = self.paste_count as f64;
        let mut items = self.d.clone_objects(&source);
        let layer = self.d.layers.get(self.d.active).or(self.d.layers.first()).map(|l| l.name.clone()).unwrap_or_default();
        for o in items.iter_mut() {
            o.move_by(k, k);
            o.layer = layer.clone();
        }
        self.sel = items.iter().map(|o| o.id).collect();
        self.d.objects.extend(items);
        self.changed();
    }

    pub fn duplicate(&mut self) {
        if self.copy().is_some() {
            self.paste(None);
        }
    }

    pub fn select_all(&mut self) {
        self.sel = self.d.visible().map(|o| o.id).collect();
        self.touch();
    }

    pub fn select_invert(&mut self) {
        let cur = self.sel.clone();
        self.sel = self.d.visible().map(|o| o.id).filter(|id| !cur.contains(id)).collect();
        self.touch();
    }

    /// Select every object of the same kind as the selected ones.
    pub fn select_same_type(&mut self) {
        let kinds: Vec<String> = self.sel.iter().filter_map(|id| self.d.get(*id)).map(kind_key).collect();
        self.sel = self.d.visible().filter(|o| kinds.contains(&kind_key(o))).map(|o| o.id).collect();
        self.touch();
    }

    /// Add lines hooked to the selected shapes, and shapes at the ends of selected lines.
    pub fn select_connected(&mut self) {
        let mut add = vec![];
        for o in self.d.objects.iter() {
            if let Kind::Line(l) = &o.kind {
                let ends: Vec<u64> = [l.start_att, l.end_att].iter().flatten().map(|a| a.target).collect();
                if ends.iter().any(|t| self.sel.contains(t)) {
                    add.push(o.id);
                }
                if self.sel.contains(&o.id) {
                    add.extend(ends);
                }
            }
        }
        for id in add {
            if !self.sel.contains(&id) {
                self.sel.push(id);
            }
        }
        self.touch();
    }

    pub fn add_layer(&mut self) {
        self.checkpoint();
        let mut n = self.d.layers.len() + 1;
        while self.d.layers.iter().any(|l| l.name == format!("Layer {n}")) {
            n += 1;
        }
        self.d.layers.push(Layer { name: format!("Layer {n}"), visible: true });
        self.d.active = self.d.layers.len() - 1;
        self.changed();
    }

    pub fn move_selection_to_active_layer(&mut self) {
        if self.sel.is_empty() {
            return;
        }
        self.checkpoint();
        let name = self.d.layers[self.d.active.min(self.d.layers.len() - 1)].name.clone();
        let sel = self.sel.clone();
        for o in self.d.objects.iter_mut().filter(|o| sel.contains(&o.id)) {
            o.layer = name.clone();
        }
        self.changed();
    }

    pub fn select_none(&mut self) {
        self.sel.clear();
        self.touch();
    }

    /// 0 front, 1 back, 2 raise, 3 lower
    pub fn reorder(&mut self, how: u8) {
        if self.sel.is_empty() {
            return;
        }
        self.checkpoint();
        let sel = self.sel.clone();
        let objs = &mut self.d.objects;
        match how {
            0 | 1 => {
                let (mut chosen, rest): (Vec<Obj>, Vec<Obj>) = objs.drain(..).partition(|o| sel.contains(&o.id));
                if how == 0 {
                    *objs = rest;
                    objs.append(&mut chosen);
                } else {
                    chosen.extend(rest);
                    *objs = chosen;
                }
            }
            2 => {
                for i in (0..objs.len().saturating_sub(1)).rev() {
                    if sel.contains(&objs[i].id) && !sel.contains(&objs[i + 1].id) {
                        objs.swap(i, i + 1);
                    }
                }
            }
            _ => {
                for i in 1..objs.len() {
                    if sel.contains(&objs[i].id) && !sel.contains(&objs[i - 1].id) {
                        objs.swap(i, i - 1);
                    }
                }
            }
        }
        self.changed();
    }

    /// 0 left, 1 centre, 2 right, 3 top, 4 middle, 5 bottom, 6 spread across, 7 spread down
    pub fn align(&mut self, how: u8) {
        let ids: Vec<u64> = self.sel.iter().copied().filter(|id| !matches!(self.d.get(*id).map(|o| &o.kind), Some(Kind::Line(_)) | None)).collect();
        if ids.len() < 2 {
            return;
        }
        self.checkpoint();
        let boxes: Vec<(u64, R)> = ids.iter().map(|id| (*id, self.d.get(*id).unwrap().align_box())).collect();
        let left = boxes.iter().map(|b| b.1.x).fold(f64::MAX, f64::min);
        let right = boxes.iter().map(|b| b.1.right()).fold(f64::MIN, f64::max);
        let top = boxes.iter().map(|b| b.1.y).fold(f64::MAX, f64::min);
        let bottom = boxes.iter().map(|b| b.1.bottom()).fold(f64::MIN, f64::max);
        let first = boxes[0].1;
        let mut moves: Vec<(u64, f64, f64)> = vec![];
        if how >= 6 {
            let horiz = how == 6;
            let mut sorted = boxes.clone();
            sorted.sort_by(|a, b| {
                let (ka, kb) = if horiz { (a.1.cx(), b.1.cx()) } else { (a.1.cy(), b.1.cy()) };
                ka.partial_cmp(&kb).unwrap()
            });
            let n = sorted.len();
            let (f0, f1) = if horiz { (sorted[0].1.cx(), sorted[n - 1].1.cx()) } else { (sorted[0].1.cy(), sorted[n - 1].1.cy()) };
            for (i, (id, b)) in sorted.iter().enumerate().take(n - 1).skip(1) {
                let target = f0 + (f1 - f0) * i as f64 / (n - 1) as f64;
                if horiz {
                    moves.push((*id, target - b.cx(), 0.0));
                } else {
                    moves.push((*id, 0.0, target - b.cy()));
                }
            }
        } else {
            for (id, b) in &boxes {
                let m = match how {
                    0 => (left - b.x, 0.0),
                    1 => (first.cx() - b.cx(), 0.0),
                    2 => (right - b.right(), 0.0),
                    3 => (0.0, top - b.y),
                    4 => (0.0, first.cy() - b.cy()),
                    _ => (0.0, bottom - b.bottom()),
                };
                moves.push((*id, m.0, m.1));
            }
        }
        for (id, dx, dy) in moves {
            if let Some(o) = self.d.get_mut(id) {
                o.move_by(dx, dy);
            }
        }
        self.changed();
    }

    pub fn nudge(&mut self, dx: f64, dy: f64) {
        if self.sel.is_empty() {
            return;
        }
        self.checkpoint();
        self.move_selection(dx, dy);
        self.changed();
    }

    pub fn add_corner(&mut self) {
        let w = self.context_world;
        let Some(&id) = self.sel.first() else { return };
        let Some(Kind::Line(l)) = self.d.get(id).map(|o| &o.kind) else { return };
        if l.kind != LineKind::PolyLine {
            return;
        }
        let best = (0..l.pts.len() - 1).min_by(|&i, &j| dist_to_segment(w, l.pts[i], l.pts[i + 1]).partial_cmp(&dist_to_segment(w, l.pts[j], l.pts[j + 1])).unwrap()).unwrap_or(0);
        self.checkpoint();
        if let Some(Kind::Line(l)) = self.d.get_mut(id).map(|o| &mut o.kind) {
            l.pts.insert(best + 1, w);
        }
        self.changed();
    }

    pub fn remove_corner(&mut self) {
        let w = self.context_world;
        let Some(&id) = self.sel.first() else { return };
        let Some(Kind::Line(l)) = self.d.get(id).map(|o| &o.kind) else { return };
        if l.kind != LineKind::PolyLine || l.pts.len() <= 2 {
            return;
        }
        let best = (1..l.pts.len() - 1).min_by(|&i, &j| l.pts[i].dist(w).partial_cmp(&l.pts[j].dist(w)).unwrap()).unwrap_or(1);
        self.checkpoint();
        if let Some(Kind::Line(l)) = self.d.get_mut(id).map(|o| &mut o.kind) {
            l.pts.remove(best);
        }
        self.changed();
    }

    /// Change every selected object (one undo step).
    pub fn apply_to_selection(&mut self, f: impl Fn(&mut Obj)) {
        if self.sel.is_empty() {
            return;
        }
        self.checkpoint();
        let sel = self.sel.clone();
        // Remember the rows of classes/tables, so lines hooked to a row can follow it.
        let before: Vec<(u64, Vec<String>)> = self
            .d
            .objects
            .iter()
            .filter(|o| sel.contains(&o.id))
            .filter_map(|o| match &o.kind {
                Kind::Element(e) => e.rec.as_ref().map(|r| (o.id, crate::record::row_keys(r))),
                _ => None,
            })
            .collect();
        for o in self.d.objects.iter_mut().filter(|o| sel.contains(&o.id)) {
            f(o);
            if let Kind::Element(e) = &mut o.kind {
                e.fit_text();
            }
        }
        for (id, old) in before {
            let Some(new) = self.d.element(id).and_then(|e| e.rec.as_ref()).map(crate::record::row_keys) else { continue };
            if new == old {
                continue;
            }
            for o in self.d.objects.iter_mut() {
                if let Kind::Line(l) = &mut o.kind {
                    for at in [&mut l.start_att, &mut l.end_att].into_iter().flatten() {
                        if at.target == id {
                            if let Some(k) = at.cp {
                                at.cp = follow_row(k, &old, &new);
                            }
                        }
                    }
                }
            }
        }
        self.changed();
    }

    pub fn notify_changed(&mut self) {
        self.changed();
    }

    pub fn insert_image(&mut self, path: &std::path::Path, at: P) {
        let mut im = ImageObj::new(r(at.x, at.y, 5.0, 4.0));
        im.file = path.to_string_lossy().to_string();
        im.load();
        if let Some(pic) = &im.pic {
            let (w, h) = pic.dimensions();
            im.rect.h = 5.0 * h as f64 / (w.max(1)) as f64;
        }
        self.checkpoint();
        let id = self.d.add(Kind::Image(im));
        self.select_only(id);
        self.changed();
    }

    pub fn describe(&self, id: u64) -> String {
        match self.d.get(id).map(|o| &o.kind) {
            Some(Kind::Element(e)) if e.shape().fam == Family::Unknown => format!("“{}” shape", e.raw_type),
            Some(Kind::Element(e)) => e.shape().name.to_string(),
            Some(Kind::Line(l)) => match l.kind {
                LineKind::ZigZag => "Zigzag connector",
                LineKind::Arc => "Arc",
                LineKind::PolyLine => "Polyline",
                LineKind::Bezier => "Curved line",
                LineKind::Line => "Line",
            }
            .to_string(),
            Some(Kind::Text(_)) => "Text".into(),
            Some(Kind::Polygon(_)) => "Polygon".into(),
            Some(Kind::Image(_)) => "Picture".into(),
            Some(Kind::Raw(rw)) => format!("“{}” object", rw.type_name),
            None => String::new(),
        }
    }

    pub fn view_range(&self) -> R {
        let mut b = r(0.0, 0.0, self.d.paper.page_w(), self.d.paper.page_h());
        if let Some(e) = self.d.extents() {
            b = b.union(&e);
        }
        b.inflate(5.0)
    }
}

fn triangle(b: R) -> Vec<P> {
    vec![b.at(0.5, 0.0), b.at(1.0, 1.0), b.at(0.0, 1.0)]
}

/// While a new line is dragged out, give it a nice default shape.
fn shape_new_line(l: &mut LineObj, end: P, start_geo: Option<&(Attachment, Element)>) {
    let start = match start_geo {
        Some((a, e)) => attach_point(e, a, end),
        None => l.start(),
    };
    l.pts = match l.kind {
        LineKind::ZigZag => {
            let vertical = match start_geo {
                Some((a, _)) if !a.is_center() && (a.v < 0.01 || a.v > 0.99) && a.u > 0.01 && a.u < 0.99 => true,
                Some((a, _)) if !a.is_center() && (a.u < 0.01 || a.u > 0.99) => false,
                _ => (end.y - start.y).abs() > (end.x - start.x).abs(),
            };
            orth_route(start, end, vertical)
        }
        LineKind::PolyLine => vec![start, (start + end) * 0.5, end],
        LineKind::Bezier => {
            let dd = end - start;
            let perp = dd.perp() * 0.2;
            vec![start, start + dd * (1.0 / 3.0) + perp, start + dd * (2.0 / 3.0) - perp, end]
        }
        _ => vec![start, end],
    };
}

fn resize_rect(b: R, h: usize, w: P, keep_aspect: bool) -> R {
    let (mut x0, mut y0, mut x1, mut y1) = (b.x, b.y, b.right(), b.bottom());
    if matches!(h, 0 | 3 | 5) {
        x0 = w.x.min(x1 - 0.1);
    }
    if matches!(h, 2 | 4 | 7) {
        x1 = w.x.max(x0 + 0.1);
    }
    if matches!(h, 0..=2) {
        y0 = w.y.min(y1 - 0.1);
    }
    if matches!(h, 5..=7) {
        y1 = w.y.max(y0 + 0.1);
    }
    let mut nr = r(x0, y0, x1 - x0, y1 - y0);
    if keep_aspect && b.w > 0.0 && b.h > 0.0 {
        let ar = b.w / b.h;
        if matches!(h, 1 | 6) {
            nr.w = nr.h * ar;
        } else {
            nr.h = nr.w / ar;
        }
        if matches!(h, 0..=2) {
            nr.y = y1 - nr.h;
        }
        if matches!(h, 0 | 3 | 5) {
            nr.x = x1 - nr.w;
        }
    }
    nr
}

/// The built-in example flowchart (Help menu).
pub fn example() -> Diagram {
    let mut d = Diagram::default();
    let shape = |d: &mut Diagram, key: &str, text: &str, b: R, fill: Col| -> u64 {
        let mut e = Element::new(shapes::index_of(key), b);
        e.fill_color = fill;
        if let Some(t) = &mut e.text {
            t.text = text.into();
        }
        e.fit_text();
        d.add(Kind::Element(e))
    };
    let green = Col::rgb(0xd7, 0xf0, 0xd0);
    let blue = Col::rgb(0xe3, 0xec, 0xfa);
    let start = shape(&mut d, "fc-terminal", "Start", r(6.0, 1.0, 4.0, 1.4), green);
    let read = shape(&mut d, "fc-parallelogram", "Read the order", r(5.5, 3.8, 5.0, 1.8), blue);
    let ok = shape(&mut d, "fc-diamond", "In stock?", r(5.5, 7.0, 5.0, 2.8), Col::rgb(0xff, 0xf2, 0xc8));
    let ship = shape(&mut d, "fc-box", "Pack and ship", r(5.5, 11.4, 5.0, 1.8), blue);
    let back = shape(&mut d, "fc-document", "Send a\nback-order note", r(13.0, 7.3, 4.6, 2.3), Col::rgb(0xfa, 0xe0, 0xe0));
    let end = shape(&mut d, "fc-terminal", "End", r(6.0, 14.8, 4.0, 1.4), green);
    let link = |d: &mut Diagram, a: u64, au: f64, av: f64, b: u64, bu: f64, bv: f64, kind: LineKind| {
        let pa = d.element(a).unwrap().rect.at(au, av);
        let pb = d.element(b).unwrap().rect.at(bu, bv);
        let pts = if kind == LineKind::ZigZag { vec![pa, p(pa.x, pb.y), pb] } else { vec![pa, pb] };
        let mut l = LineObj::new(kind, pts);
        l.end_arrow.kind = 22;
        l.start_att = Some(Attachment { target: a, u: au, v: av, cp: None });
        l.end_att = Some(Attachment { target: b, u: bu, v: bv, cp: None });
        d.add(Kind::Line(l));
    };
    link(&mut d, start, 0.5, 1.0, read, 0.5, 0.0, LineKind::Line);
    link(&mut d, read, 0.5, 1.0, ok, 0.5, 0.0, LineKind::Line);
    link(&mut d, ok, 0.5, 1.0, ship, 0.5, 0.0, LineKind::Line);
    link(&mut d, ship, 0.5, 1.0, end, 0.5, 0.0, LineKind::Line);
    link(&mut d, ok, 1.0, 0.5, back, 0.0, 0.5, LineKind::Line);
    link(&mut d, back, 0.5, 0.0, read, 0.9999, 0.5, LineKind::ZigZag);
    let label = |d: &mut Diagram, s: &str, x: f64, y: f64| {
        d.add(Kind::Text(TextObj { pos: p(x, y), text: TextSpec { text: s.into(), height: 0.6, align: 0, ..Default::default() } }));
    };
    label(&mut d, "Yes", 8.3, 10.6);
    label(&mut d, "No", 11.0, 8.1);
    d.update_connections();
    d
}

fn kind_key(o: &Obj) -> String {
    match &o.kind {
        Kind::Element(e) => format!("e{}", e.def),
        Kind::Line(l) => format!("l{:?}", l.kind),
        Kind::Text(_) => "t".into(),
        Kind::Polygon(_) => "p".into(),
        Kind::Image(_) => "i".into(),
        Kind::Raw(r) => format!("r{}", r.type_name),
    }
}

/// Where a row's connection point went after the rows were edited: the row with the same
/// text (or the same position if it was renamed). Points around the box stay as they are.
fn follow_row(k: usize, old: &[String], new: &[String]) -> Option<usize> {
    if k < 8 {
        return Some(k);
    }
    let (row, side) = ((k - 8) / 2, (k - 8) % 2);
    let Some(text) = old.get(row) else { return None };
    let new_row = new.iter().position(|t| t == text).or(if row < new.len() { Some(row) } else { None })?;
    Some(8 + 2 * new_row + side)
}
