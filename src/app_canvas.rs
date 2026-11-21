//! The drawing area: mouse, keyboard, rendering, rulers, scroll bars and typing into shapes.

use super::app_ui::menu_item;
use super::*;

fn dashed_rect(p: &egui::Painter, r: Rect, stroke: Stroke, dash: f32) {
    let pts = vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.extend(Shape::dashed_line(&pts, stroke, dash, dash));
}

fn label(v: f64) -> String {
    let s = format!("{:.1}", v);
    s.trim_end_matches(".0").to_string()
}

impl App {
    pub(super) fn canvas(&mut self, ui: &mut Ui, act: &mut Option<Action>) {
        let ctx = ui.ctx().clone();
        let pal = pal(&ctx);
        let whole = ui.available_rect_before_wrap();
        // One tab per open diagram, like the original's.
        let tab_h = 26.0;
        let strip = Rect::from_min_max(whole.min, pos2(whole.max.x, whole.min.y + tab_h));
        ui.painter().rect_filled(strip, 0.0, pal.panel);
        let font = FontId::proportional(13.0);
        let mut x = strip.min.x + 2.0;
        for i in 0..self.tabs.len() {
            let (name, modified) = self.tab_name(i);
            let label = if modified { format!("*{name}") } else { name.clone() };
            let tw = ctx.fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), Color32::BLACK).size().x);
            let tab = Rect::from_min_size(pos2(x, strip.min.y + 2.0), vec2(tw + 36.0, tab_h - 2.0));
            x = tab.max.x + 2.0;
            let close = Rect::from_center_size(pos2(tab.max.x - 12.0, tab.center().y), vec2(14.0, 14.0));
            let tr = ui.interact(tab, ui.scope_id().with(("tab", i)), Sense::click());
            let cr = ui.interact(close, ui.scope_id().with(("tab-close", i)), Sense::click()).on_hover_text("Close this diagram");
            let active = i == self.cur;
            let fill = if active {
                pal.bar
            } else if tr.hovered() {
                pal.hover
            } else {
                pal.tab_idle
            };
            let corners = egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 };
            ui.painter().rect_filled(tab, corners, fill);
            if !active {
                ui.painter().rect_stroke(tab, corners, Stroke::new(1.0, pal.tab_border), StrokeKind::Inside);
            }
            ui.painter().text(pos2(tab.min.x + 6.0, tab.center().y), Align2::LEFT_CENTER, &label, font.clone(), if active { pal.ink } else { pal.tab_text_idle });
            let xc = if cr.hovered() { pal.danger } else { pal.close_x };
            ui.painter().line_segment([close.left_top() + vec2(3.0, 3.0), close.right_bottom() - vec2(3.0, 3.0)], Stroke::new(2.5, xc));
            ui.painter().line_segment([close.right_top() + vec2(-3.0, 3.0), close.left_bottom() + vec2(3.0, -3.0)], Stroke::new(2.5, xc));
            if cr.clicked() || tr.middle_clicked() {
                *act = Some(Action::CloseTab(i));
            } else if tr.clicked() {
                *act = Some(Action::SwitchTab(i));
            }
            let _ = tr.on_hover_text(name);
        }
        let full = Rect::from_min_max(pos2(whole.min.x, strip.max.y), whole.max);
        let rw = if self.ed.show_rulers { 18.0 } else { 0.0 };
        let sb = 12.0;
        let area = Rect::from_min_max(pos2(full.min.x + rw, full.min.y + rw), pos2(full.max.x - sb, full.max.y - sb));
        if area.width() < 10.0 || area.height() < 10.0 {
            return;
        }
        self.last_area = area;
        let resp = ui.allocate_rect(area, Sense::click_and_drag());

        // ---- input
        let (pos, pressed, released, sec_pressed, middle_down, mods, zoom_delta, events) = ctx.input(|i| {
            (
                i.pointer.interact_pos().or(i.pointer.hover_pos()),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.secondary_pressed(),
                i.pointer.middle_down(),
                i.modifiers,
                i.zoom_delta(),
                i.events.clone(),
            )
        });
        let emods = Mods { shift: mods.shift, ctrl: mods.command };
        let hovered = resp.hovered() && pos.map(|q| area.contains(q)).unwrap_or(false);
        if let Some(q) = pos.filter(|q| area.contains(*q)) {
            self.mouse_world = Some(self.to_world(q));
        }

        // Panning with the middle button or the scroll tool.
        let tool_pan = self.ed.tool == Tool::Scroll && ctx.input(|i| i.pointer.primary_down());
        if let Some(q) = pos {
            if (middle_down || tool_pan) && (hovered || self.pan.is_some()) {
                match self.pan {
                    None => self.pan = Some((q, self.ed.origin)),
                    Some((start, o)) => {
                        let s = self.ed.scale();
                        self.ed.origin = p(o.x - (q.x - start.x) as f64 / s, o.y - (q.y - start.y) as f64 / s);
                        self.ed.touch();
                    }
                }
            } else {
                self.pan = None;
            }
        }

        if let Some(q) = pos {
            let w = self.to_world(q);
            if pressed && hovered && self.ed.tool != Tool::Scroll {
                self.status.clear();
                if self.ed.tool == Tool::Magnify {
                    let z = if mods.shift { self.ed.zoom / 1.5 } else { self.ed.zoom * 1.5 };
                    self.zoom_to(z, Some(q));
                } else {
                    self.ed.mouse_down(w, emods);
                    self.canvas_drag = true;
                }
                ctx.memory_mut(|m| {
                    if let Some(f) = m.focused() {
                        m.surrender_focus(f);
                    }
                });
            }
            if sec_pressed && hovered {
                if self.ed.tool == Tool::Magnify {
                    self.zoom_to(self.ed.zoom / 1.5, Some(q));
                } else {
                    self.ed.right_click(w);
                }
            }
            if self.canvas_drag {
                self.ed.mouse_move(w, emods);
                if released {
                    self.ed.mouse_up(w);
                    self.canvas_drag = false;
                }
            }
            if resp.double_clicked() && hovered {
                self.ed.double_click(w);
            }
        } else if released && self.canvas_drag {
            self.canvas_drag = false;
            let w = self.mouse_world.unwrap_or_default();
            self.ed.mouse_up(w);
        }
        if self.ed.props_request {
            self.ed.props_request = false;
            *act = Some(Action::Props);
        }
        if let Some(at) = self.ed.image_request.take() {
            if let Some(path) = rfd::FileDialog::new().set_title("Insert picture").add_filter("Pictures", &["png", "jpg", "jpeg", "gif", "bmp"]).pick_file() {
                self.ed.insert_image(&path, at);
            }
        }

        // Wheel: scroll, or zoom with Ctrl.
        if hovered {
            if (zoom_delta - 1.0).abs() > 1e-4 {
                let z = self.ed.zoom * zoom_delta as f64;
                self.zoom_to(z, pos);
            } else {
                for e in &events {
                    if let Event::MouseWheel { unit, delta, .. } = e {
                        if mods.command {
                            continue;
                        }
                        let k = match unit {
                            egui::MouseWheelUnit::Point => 1.0,
                            egui::MouseWheelUnit::Line => 40.0,
                            egui::MouseWheelUnit::Page => area.height(),
                        };
                        let (mut dx, mut dy) = (delta.x * k, delta.y * k);
                        if mods.shift && dx == 0.0 {
                            dx = dy;
                            dy = 0.0;
                        }
                        let s = self.ed.scale();
                        self.ed.origin = p(self.ed.origin.x - dx as f64 / s, self.ed.origin.y - dy as f64 / s);
                        self.ed.end_edit(true);
                        self.ed.touch();
                    }
                }
            }
        }

        // Cursor.
        if hovered {
            let w = self.to_world(pos.unwrap_or_default());
            let icon = match self.ed.tool {
                Tool::Select => match self.ed.cursor_hint(w) {
                    Some(0) | Some(7) => CursorIcon::ResizeNwSe,
                    Some(2) | Some(5) => CursorIcon::ResizeNeSw,
                    Some(1) | Some(6) => CursorIcon::ResizeVertical,
                    Some(3) | Some(4) => CursorIcon::ResizeHorizontal,
                    Some(_) => CursorIcon::Crosshair,
                    None if self.ed.object_at(w).is_some() => CursorIcon::Move,
                    None => CursorIcon::Default,
                },
                Tool::Scroll => CursorIcon::Grab,
                Tool::Magnify => CursorIcon::ZoomIn,
                _ => CursorIcon::Crosshair,
            };
            ctx.set_cursor_icon(icon);
        }

        // ---- render the diagram (only when something changed)
        let ppp = ctx.pixels_per_point();
        let size = [(area.width() * ppp).round().max(1.0) as usize, (area.height() * ppp).round().max(1.0) as usize];
        let flags = (self.ed.show_grid as u8) | ((self.ed.show_pages as u8) << 1);
        let editing = self.ed.editing.as_ref().map(|e| e.id);
        let key = (self.ed.rev, self.ed.zoom.to_bits(), self.ed.origin.x.to_bits(), self.ed.origin.y.to_bits(), size, flags, editing);
        if self.tex_key != Some(key) || self.tex.is_none() {
            if let Some(mut pm) = tiny_skia::Pixmap::new(size[0] as u32, size[1] as u32) {
                pm.fill(skia::color(self.ed.d.background));
                let view = self.visible_world();
                {
                    let mut sp = SkiaPainter { pm: &mut pm, scale: self.ed.scale() * ppp as f64, ox: self.ed.origin.x, oy: self.ed.origin.y, offx: 0.0, offy: 0.0, min_width: 1.0 };
                    skia::draw_backdrop(&mut sp, &self.ed.d, view, self.ed.show_grid, self.ed.show_pages);
                    crate::render::draw_diagram(&self.ed.d, &mut sp, editing);
                }
                let img = egui::ColorImage::from_rgba_premultiplied(size, pm.data());
                match &mut self.tex {
                    Some(t) => t.set(img, egui::TextureOptions::NEAREST),
                    None => self.tex = Some(ctx.load_texture("diagram", img, egui::TextureOptions::NEAREST)),
                }
            }
            self.tex_key = Some(key);
        }
        let painter = ui.painter_at(area);
        if let Some(t) = &self.tex {
            painter.image(t.id(), area, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }

        // ---- overlays
        let vis = self.visible_world();
        if self.ed.show_cps && self.ed.scale() >= 8.0 {
            let blue = Stroke::new(1.0, Color32::from_rgb(0, 0, 255));
            // Show connection points where they help: everywhere while drawing a line,
            // otherwise only on the shape under the mouse.
            let drawing_lines = matches!(self.ed.tool, Tool::Line | Tool::Arc | Tool::ZigZag | Tool::PolyLine | Tool::Bezier) || self.ed.snap_target.is_some();
            let under = if hovered { pos.map(|q| self.to_world(q)) } else { None };
            for o in self.ed.d.visible() {
                let Kind::Element(e) = &o.kind else { continue };
                if !vis.intersects(&e.rect) {
                    continue;
                }
                if !drawing_lines && !under.map(|w| e.rect.inflate(0.3).contains(w)).unwrap_or(false) {
                    continue;
                }
                let cps = e.cps();
                for c in &cps[..cps.len() - 1] {
                    let s = self.to_screen(*c);
                    painter.line_segment([s + vec2(-2.5, -2.5), s + vec2(2.5, 2.5)], blue);
                    painter.line_segment([s + vec2(-2.5, 2.5), s + vec2(2.5, -2.5)], blue);
                }
            }
        }
        for id in self.ed.sel.clone() {
            let Some(o) = self.ed.d.get(id) else { continue };
            let boxed = matches!(&o.kind, Kind::Text(_) | Kind::Raw(_)) || matches!(&o.kind, Kind::Element(e) if is_unknown(e));
            if boxed {
                let b = o.bounds().inflate(0.05);
                let rr = Rect::from_min_max(self.to_screen(p(b.x, b.y)), self.to_screen(p(b.right(), b.bottom())));
                dashed_rect(&painter, rr, Stroke::new(1.0, Color32::from_rgb(0, 140, 0)), 3.0);
            }
            for (q, h) in Editor::handles(o) {
                let s = self.to_screen(q);
                let connected = matches!(&o.kind, Kind::Line(l) if (h == 0 && l.start_att.is_some()) || (h == 1 && l.end_att.is_some()));
                let fill = if connected {
                    Color32::from_rgb(255, 60, 60)
                } else if h >= H_POINT {
                    Color32::from_rgb(255, 210, 0)
                } else {
                    Color32::from_rgb(0, 200, 0)
                };
                let rr = Rect::from_center_size(s, vec2(7.0, 7.0));
                painter.rect_filled(rr, 0.0, fill);
                painter.rect_stroke(rr, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
            }
        }
        if let Some(tid) = self.ed.snap_target {
            if let Some(e) = self.ed.d.element(tid) {
                let rr = Rect::from_min_max(self.to_screen(p(e.rect.x, e.rect.y)), self.to_screen(p(e.rect.right(), e.rect.bottom()))).expand(2.0);
                dashed_rect(&painter, rr, Stroke::new(1.0, Color32::from_rgb(255, 80, 80)), 4.0);
            }
            if let Some(sp) = self.ed.snap_point {
                painter.circle_stroke(self.to_screen(sp), 5.0, Stroke::new(2.0, Color32::from_rgb(255, 0, 0)));
            }
        }
        if let Some(rb) = self.ed.rubber {
            let rr = Rect::from_min_max(self.to_screen(p(rb.x, rb.y)), self.to_screen(p(rb.right(), rb.bottom())));
            dashed_rect(&painter, rr, Stroke::new(1.0, Color32::BLACK), 2.0);
        }

        // ---- rulers and scroll bars
        if self.ed.show_rulers {
            self.rulers(ui, full, area);
        }
        self.scrollbars(ui, area, sb);

        // ---- context menu
        resp.context_menu(|ui| {
            let any = !self.ed.sel.is_empty();
            let one_text = self.ed.sel.len() == 1 && self.ed.d.get(self.ed.sel[0]).and_then(|o| o.text()).is_some();
            let poly = self.ed.sel.len() == 1 && matches!(self.ed.d.get(self.ed.sel[0]).map(|o| &o.kind), Some(Kind::Line(l)) if l.kind == LineKind::PolyLine);
            if menu_item(ui, "Properties…", "Alt+Enter", true) {
                *act = Some(Action::Props);
            }
            if one_text && menu_item(ui, "Edit text", "F2", true) {
                *act = Some(Action::EditText);
            }
            ui.separator();
            let items: [(&str, &str, bool, Action); 7] = [
                ("Cut", "Ctrl+X", any, Action::Cut),
                ("Copy", "Ctrl+C", any, Action::Copy),
                ("Paste", "Ctrl+V", self.ed.can_paste(), Action::Paste),
                ("Duplicate", "Ctrl+D", any, Action::Duplicate),
                ("Delete", "Del", any, Action::Delete),
                ("Bring to front", "Ctrl+F", any, Action::Reorder(0)),
                ("Send to back", "Ctrl+B", any, Action::Reorder(1)),
            ];
            for (i, (t, k, en, a)) in items.iter().enumerate() {
                if i == 5 {
                    ui.separator();
                }
                if menu_item(ui, t, k, *en) {
                    *act = Some(*a);
                }
            }
            if poly {
                ui.separator();
                if menu_item(ui, "Add corner here", "", true) {
                    *act = Some(Action::AddCorner);
                }
                if menu_item(ui, "Remove nearest corner", "", true) {
                    *act = Some(Action::RemoveCorner);
                }
            }
        });

        // ---- typing into a shape
        self.text_editor(&ctx);
    }

    fn rulers(&self, ui: &Ui, full: Rect, area: Rect) {
        let painter = ui.painter_at(full);
        let pal = pal(ui.ctx());
        let rw = 18.0;
        let bg = pal.bar;
        let ink = pal.ink;
        painter.rect_filled(Rect::from_min_max(full.min, pos2(full.max.x, full.min.y + rw)), 0.0, bg);
        painter.rect_filled(Rect::from_min_max(full.min, pos2(full.min.x + rw, full.max.y)), 0.0, bg);
        painter.line_segment([pos2(area.min.x, area.min.y - 0.5), pos2(area.max.x, area.min.y - 0.5)], Stroke::new(1.0, pal.ruler_edge));
        painter.line_segment([pos2(area.min.x - 0.5, area.min.y), pos2(area.min.x - 0.5, area.max.y)], Stroke::new(1.0, pal.ruler_edge));
        let s = self.ed.scale();
        let mut step = 1.0;
        for c in [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0] {
            step = c;
            if c * s >= 70.0 {
                break;
            }
        }
        let minor = step / if [2.0, 20.0, 200.0].contains(&step) { 4.0 } else { 5.0 };
        let vis = self.visible_world();
        let font = FontId::proportional(9.0);
        let top = painter.with_clip_rect(Rect::from_min_max(pos2(area.min.x, full.min.y), pos2(area.max.x, area.min.y)));
        let mut x = (vis.x / minor).floor() * minor;
        while x <= vis.right() {
            let sx = self.to_screen(p(x, 0.0)).x.round() + 0.5;
            let major = ((x / step) - (x / step).round()).abs() < 1e-6;
            top.line_segment([pos2(sx, if major { full.min.y + 3.0 } else { area.min.y - 5.0 }), pos2(sx, area.min.y - 1.0)], Stroke::new(1.0, ink));
            if major {
                top.text(pos2(sx + 2.0, full.min.y + 1.0), Align2::LEFT_TOP, label(x), font.clone(), ink);
            }
            x += minor;
        }
        let left = painter.with_clip_rect(Rect::from_min_max(pos2(full.min.x, area.min.y), pos2(area.min.x, area.max.y)));
        let mut y = (vis.y / minor).floor() * minor;
        while y <= vis.bottom() {
            let sy = self.to_screen(p(0.0, y)).y.round() + 0.5;
            let major = ((y / step) - (y / step).round()).abs() < 1e-6;
            left.line_segment([pos2(if major { full.min.x + 3.0 } else { area.min.x - 5.0 }, sy), pos2(area.min.x - 1.0, sy)], Stroke::new(1.0, ink));
            if major {
                for (i, ch) in label(y).chars().enumerate() {
                    left.text(pos2(full.min.x + 2.0, sy + 2.0 + i as f32 * 8.5), Align2::LEFT_TOP, ch, font.clone(), ink);
                }
            }
            y += minor;
        }
        // Where the mouse is.
        if let Some(w) = self.mouse_world {
            let sp = self.to_screen(w);
            let m = Stroke::new(1.0, pal.danger);
            top.line_segment([pos2(sp.x, full.min.y), pos2(sp.x, area.min.y)], m);
            left.line_segment([pos2(full.min.x, sp.y), pos2(area.min.x, sp.y)], m);
        }
        painter.rect_filled(Rect::from_min_size(full.min, vec2(rw, rw)), 0.0, bg);
    }

    fn scrollbars(&mut self, ui: &mut Ui, area: Rect, sb: f32) {
        let vis = self.visible_world();
        let range = self.ed.view_range().union(&vis);
        let pal = pal(ui.ctx());
        let track_bg = pal.track;
        for horizontal in [true, false] {
            let track = if horizontal {
                Rect::from_min_max(pos2(area.min.x, area.max.y), pos2(area.max.x, area.max.y + sb))
            } else {
                Rect::from_min_max(pos2(area.max.x, area.min.y), pos2(area.max.x + sb, area.max.y))
            };
            let id = ui.scope_id().with(("scroll", horizontal));
            let resp = ui.interact(track, id, Sense::click_and_drag());
            let (len, start, vstart, vlen, rstart, rlen) = if horizontal {
                (track.width(), track.min.x, vis.x, vis.w, range.x, range.w)
            } else {
                (track.height(), track.min.y, vis.y, vis.h, range.y, range.h)
            };
            let t0 = start + ((vstart - rstart) / rlen) as f32 * len;
            let tl = ((vlen / rlen) as f32 * len).max(20.0).min(len);
            let thumb = if horizontal {
                Rect::from_min_max(pos2(t0, track.min.y + 2.0), pos2(t0 + tl, track.max.y - 2.0))
            } else {
                Rect::from_min_max(pos2(track.min.x + 2.0, t0), pos2(track.max.x - 2.0, t0 + tl))
            };
            let painter = ui.painter();
            painter.rect_filled(track, 0.0, track_bg);
            painter.rect_filled(thumb, 3.0, if resp.dragged() || resp.hovered() { pal.thumb_hot } else { pal.thumb });
            if resp.drag_started() {
                let q = resp.interact_pointer_pos().unwrap_or_default();
                let at = if horizontal { q.x } else { q.y };
                self.scroll_drag = Some((horizontal, at, if horizontal { self.ed.origin.x } else { self.ed.origin.y }));
            }
            if resp.dragged() {
                if let (Some((h, at0, o0)), Some(q)) = (self.scroll_drag, resp.interact_pointer_pos()) {
                    if h == horizontal {
                        let at = if horizontal { q.x } else { q.y };
                        let v = o0 + ((at - at0) / len) as f64 * rlen;
                        if horizontal {
                            self.ed.origin.x = v;
                        } else {
                            self.ed.origin.y = v;
                        }
                        self.ed.touch();
                    }
                }
            } else if resp.clicked() {
                if let Some(q) = resp.interact_pointer_pos() {
                    let at = if horizontal { q.x } else { q.y };
                    let dir = if at < t0 { -1.0 } else { 1.0 };
                    if horizontal {
                        self.ed.origin.x += dir * vis.w * 0.9;
                    } else {
                        self.ed.origin.y += dir * vis.h * 0.9;
                    }
                    self.ed.touch();
                }
            }
            if resp.drag_stopped() {
                self.scroll_drag = None;
            }
        }
        ui.painter().rect_filled(Rect::from_min_max(area.max, area.max + vec2(sb, sb)), 0.0, track_bg);
    }

    fn text_editor(&mut self, ctx: &egui::Context) {
        let Some(id) = self.ed.editing.as_ref().map(|e| e.id) else { return };
        let Some(o) = self.ed.d.get(id) else {
            self.ed.editing = None;
            return;
        };
        let Some(t) = o.text().cloned() else {
            self.ed.editing = None;
            return;
        };
        let bx = match &o.kind {
            Kind::Element(e) => e.text_area(),
            _ => o.bounds(),
        };
        let is_text_obj = matches!(o.kind, Kind::Text(_));
        let font_px = ((t.height * EM_FACTOR * self.ed.scale()) as f32).max(11.0);
        let font = FontId::proportional(font_px);
        let text_now = self.ed.editing.as_ref().map(|e| e.text.clone()).unwrap_or_default();
        let lines: Vec<&str> = text_now.split('\n').collect();
        let widest = lines.iter().map(|l| ctx.fonts_mut(|f| f.layout_no_wrap(l.to_string(), font.clone(), Color32::BLACK).size().x)).fold(0.0, f32::max);
        let w = (widest + 24.0).max(70.0);
        let h = lines.len() as f32 * font_px * 1.25 + 10.0;
        let centre = self.to_screen(bx.center());
        let left_top = if is_text_obj {
            let tl = self.to_screen(p(bx.x, bx.y));
            match t.align {
                0 => pos2(tl.x - 4.0, tl.y - 5.0),
                2 => pos2(self.to_screen(p(bx.right(), 0.0)).x - w + 4.0, tl.y - 5.0),
                _ => pos2(centre.x - w / 2.0, tl.y - 5.0),
            }
        } else {
            pos2(centre.x - w / 2.0, centre.y - h / 2.0)
        };
        let align = match t.align {
            0 => egui::Align2::LEFT_TOP,
            2 => egui::Align2::RIGHT_TOP,
            _ => egui::Align2::CENTER_TOP,
        };
        let mut finished = false;
        egui::Area::new(egui::Id::unique("text-editor")).order(egui::Order::Foreground).fixed_pos(left_top).show(ctx, |ui| {
            let Some(ed) = self.ed.editing.as_mut() else { return };
            let te = egui::TextEdit::multiline(&mut ed.text)
                .font(font.clone())
                .text_color(c32(t.color))
                .desired_width(w)
                .desired_rows(lines.len().max(1))
                .align(align)
                .background_color(Color32::from_rgb(255, 255, 240));
            let r = ui.add(te);
            if ed.just_started {
                r.request_focus();
                ed.just_started = false;
                // Put the cursor at the end of the text.
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), r.id) {
                    let end = egui::text::CCursor::new(ed.text.chars().count());
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
                    state.store(ui.ctx(), r.id);
                }
            } else if r.lost_focus() {
                finished = true;
            }
            if r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.command) {
                finished = true;
            }
        });
        if finished {
            self.ed.end_edit(true);
        }
    }

    pub(super) fn keyboard(&mut self, ctx: &egui::Context, act: &mut Option<Action>) {
        let typing = ctx.text_edit_focused();
        let dialog = self.props.is_some() || self.page.is_some() || self.pdf_open;
        let cmd = egui::Modifiers::COMMAND;
        let cs = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        let shortcuts: [(egui::Modifiers, Key, Action, bool); 24] = [
            (cmd, Key::N, Action::New, false),
            (cmd, Key::O, Action::Open, false),
            (cs, Key::S, Action::SaveAs, false),
            (cmd, Key::S, Action::Save, false),
            (cs, Key::E, Action::BestFit, false),
            (cmd, Key::E, Action::ExportPdf, false),
            (cmd, Key::P, Action::Print, false),
            (cmd, Key::Q, Action::Exit, false),
            (cmd, Key::W, Action::CloseTab(usize::MAX), false),
            (cmd, Key::Tab, Action::NextTab, false),
            (cs, Key::Z, Action::Redo, true),
            (cmd, Key::Z, Action::Undo, true),
            (cmd, Key::Y, Action::Redo, true),
            (cmd, Key::D, Action::Duplicate, true),
            (cs, Key::A, Action::SelectNone, true),
            (cmd, Key::A, Action::SelectAll, true),
            (cmd, Key::F, Action::Reorder(0), true),
            (cmd, Key::B, Action::Reorder(1), true),
            (cmd, Key::Plus, Action::ZoomIn, true),
            (cmd, Key::Equals, Action::ZoomIn, true),
            (cmd, Key::Minus, Action::ZoomOut, true),
            (cmd, Key::Num1, Action::Zoom(1.0), true),
            (egui::Modifiers::ALT, Key::Enter, Action::Props, true),
            (egui::Modifiers::NONE, Key::F1, Action::Tips, false),
        ];
        for (m, k, a, needs_canvas) in shortcuts {
            if needs_canvas && (typing || dialog) {
                continue;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(m, k))) {
                *act = Some(a);
            }
        }
        if typing || dialog {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                Event::Copy => *act = Some(Action::Copy),
                Event::Cut => *act = Some(Action::Cut),
                Event::Paste(s) => self.ed.paste(Some(&s)),
                Event::Text(t) => {
                    if let [id] = self.ed.sel[..] {
                        if self.ed.d.get(id).and_then(|o| o.text()).is_some() && self.ed.editing.is_none() {
                            self.ed.begin_edit(id, Some(&t));
                        }
                    }
                }
                Event::Key { key, pressed: true, modifiers, .. } => {
                    let st = if modifiers.shift {
                        1.0
                    } else if self.ed.snap_to_grid {
                        self.ed.d.grid
                    } else {
                        0.1
                    };
                    match key {
                        Key::Delete | Key::Backspace if modifiers.is_none() => self.ed.delete_selection(),
                        Key::ArrowLeft if !modifiers.command => self.ed.nudge(-st, 0.0),
                        Key::ArrowRight if !modifiers.command => self.ed.nudge(st, 0.0),
                        Key::ArrowUp if !modifiers.command => self.ed.nudge(0.0, -st),
                        Key::ArrowDown if !modifiers.command => self.ed.nudge(0.0, st),
                        Key::Escape => {
                            if self.ed.tool != Tool::Select {
                                self.set_tool(Tool::Select, false);
                            } else {
                                self.ed.select_none();
                            }
                        }
                        Key::F2 | Key::Enter if modifiers.is_none() => {
                            if let [id] = self.ed.sel[..] {
                                self.ed.begin_edit(id, None);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}
