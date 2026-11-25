//! Menus, toolbar and toolbox.

use super::*;

pub(super) fn icon_button(ui: &mut Ui, icon: Icon, tip: &str, enabled: bool, checked: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    let painter = ui.painter();
    let pal = pal(ui.ctx());
    if checked {
        painter.rect_filled(rect, 3.0, pal.selected);
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0, pal.selected_border), StrokeKind::Inside);
    } else if enabled && resp.hovered() {
        painter.rect_filled(rect, 3.0, pal.hover);
    }
    paint_icon(painter, rect.shrink(4.0), icon, enabled);
    let resp = resp.on_hover_text(tip);
    enabled && resp.clicked()
}

/// A square toolbox button with a light frame, like the original's.
pub(super) fn tool_cell(ui: &mut Ui, size: f32, selected: bool, sticky: bool) -> (Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let painter = ui.painter();
    let pal = pal(ui.ctx());
    let (fill, border) = if selected {
        (pal.selected, if sticky { pal.danger } else { pal.selected_border })
    } else if resp.hovered() {
        (pal.hover, pal.hover_border)
    } else {
        (pal.cell, pal.cell_border)
    };
    painter.rect_filled(rect, 3.0, fill);
    painter.rect_stroke(rect, 3.0, Stroke::new(if selected && sticky { 2.0 } else { 1.0 }, border), StrokeKind::Inside);
    (rect, resp)
}

/// A shortcut as shown in a menu: Windows/Linux wording, or the Mac symbols.
pub(super) fn keys(other: &'static str, mac: &'static str) -> &'static str {
    if cfg!(target_os = "macos") {
        mac
    } else {
        other
    }
}

pub(super) fn menu_item(ui: &mut Ui, text: &str, shortcut: &str, enabled: bool) -> bool {
    let b = egui::Button::new(text).shortcut_text(shortcut);
    let clicked = ui.add_enabled(enabled, b).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// Lets the diagram renderer draw small pictures (arrowheads) with the UI painter.
struct EguiPainter<'a> {
    p: &'a egui::Painter,
    scale: f32,
}

impl crate::render::Painter for EguiPainter<'_> {
    fn path(&mut self, path: &geom::Path, fill: Option<Col>, stroke: Option<Col>, width: f64, _style: LineStyle, _dash: f64) {
        for (pts, closed) in path.flatten(8) {
            let v: Vec<Pos2> = pts.iter().map(|q| pos2(q.x as f32 * self.scale, q.y as f32 * self.scale)).collect();
            if let Some(f) = fill {
                if closed && v.len() >= 3 {
                    self.p.add(Shape::convex_polygon(v.clone(), c32(f), Stroke::NONE));
                }
            }
            if let Some(s) = stroke {
                let st = Stroke::new((width as f32 * self.scale).max(1.0), c32(s));
                self.p.add(if closed { Shape::closed_line(v, st) } else { Shape::line(v, st) });
            }
        }
    }
    fn text_line(&mut self, _s: &str, _t: &TextSpec, _x: f64, _baseline: f64) {}
    fn image(&mut self, _img: &ImageObj) {}
}

fn paint_arrow(p: &egui::Painter, r: Rect, kind: i64, start: bool) {
    let y = r.center().y;
    let (tip, from) = if start { (pos2(r.min.x + 4.0, y), pos2(r.max.x - 4.0, y)) } else { (pos2(r.max.x - 4.0, y), pos2(r.min.x + 4.0, y)) };
    let pal = pal(p.ctx());
    let ink = pal.ink;
    if kind == 0 {
        p.line_segment([from, tip], Stroke::new(1.5, ink));
        return;
    }
    let s = 20.0f32;
    let a = ArrowSpec { kind, length: 0.6, width: 0.6 };
    let trim = if matches!(kind, 1 | 7) { 0.0 } else { 0.5 * s };
    let dir = if start { -1.0 } else { 1.0 };
    p.line_segment([from, pos2(tip.x - dir * trim, y)], Stroke::new(1.5, ink));
    let mut ep = EguiPainter { p, scale: s };
    let w = |q: Pos2| geom::p((q.x / s) as f64, (q.y / s) as f64);
    crate::render::draw_arrow(&mut ep, &a, w(tip), w(from), col(ink), 0.075, col(pal.cell));
}

/// Picture of a ready-made connector: its line style with its arrowheads.
fn paint_preset(p: &egui::Painter, r: Rect, pr: &shapes::LinePreset) {
    paint_style(p, r, if pr.dashed { LineStyle::Dashed } else { LineStyle::Solid });
    let pal = pal(p.ctx());
    let (ink, bg) = (col(pal.ink), col(pal.cell));
    let s = 20.0f32;
    let y = r.center().y;
    let w = |q: Pos2| geom::p((q.x / s) as f64, (q.y / s) as f64);
    let mut ep = EguiPainter { p, scale: s };
    if pr.end != 0 {
        let a = ArrowSpec { kind: pr.end, length: 0.45, width: 0.45 };
        crate::render::draw_arrow(&mut ep, &a, w(pos2(r.max.x - 2.0, y)), w(pos2(r.min.x, y)), ink, 0.06, bg);
    }
    if pr.start != 0 {
        let a = ArrowSpec { kind: pr.start, length: 0.6, width: 0.4 };
        crate::render::draw_arrow(&mut ep, &a, w(pos2(r.min.x + 2.0, y)), w(pos2(r.max.x, y)), ink, 0.06, bg);
    }
}

fn paint_style(p: &egui::Painter, r: Rect, st: LineStyle) {
    let y = r.center().y;
    let (x0, x1) = (r.min.x + 4.0, r.max.x - 4.0);
    let stroke = Stroke::new(1.5, pal(p.ctx()).ink);
    match crate::render::dash_pattern(st, 0.3) {
        None => {
            p.line_segment([pos2(x0, y), pos2(x1, y)], stroke);
        }
        Some(pat) => {
            let mut x = x0;
            let mut i = 0;
            while x < x1 {
                let len = (pat[i % pat.len()] * 20.0) as f32;
                if i % 2 == 0 {
                    p.line_segment([pos2(x, y), pos2((x + len).min(x1), y)], stroke);
                }
                x += len.max(1.0);
                i += 1;
            }
        }
    }
}

pub(super) const TOOLBOX_TOOLS: [(Tool, &str); 14] = [
    (Tool::Select, "Modify: select, move and resize (Esc)"),
    (Tool::TextEdit, "Text edit: click a shape to type in it"),
    (Tool::Magnify, "Magnify: click to zoom in, right-click to zoom out"),
    (Tool::Scroll, "Scroll around the diagram"),
    (Tool::Text, "Text"),
    (Tool::Box, "Box"),
    (Tool::Ellipse, "Ellipse"),
    (Tool::Polygon, "Polygon"),
    (Tool::Line, "Line"),
    (Tool::Arc, "Arc"),
    (Tool::ZigZag, "Zigzag line (right angles)"),
    (Tool::PolyLine, "Polyline"),
    (Tool::Bezier, "Bezier (curved) line"),
    (Tool::Image, "Image"),
];

impl App {
    pub(super) fn status_line(&mut self) -> String {
        if !self.status.is_empty() {
            return self.status.clone();
        }
        match self.ed.tool {
            Tool::Select => match self.ed.sel.len() {
                0 => format!("{} object(s) in the diagram. Pick a shape on the left, then click or drag on the page.", self.ed.d.objects.len()),
                1 => format!("Selected '{}'. Double-click to edit it; right-click for more.", self.ed.describe(self.ed.sel[0])),
                n => format!("{n} objects selected."),
            },
            Tool::Shape(i) => format!("Click or drag on the diagram to place a {}.", shapes::def(i).name.to_lowercase()),
            Tool::Connector(i) => format!("Drag from one shape to another to draw a {} line.", shapes::LINE_PRESETS[i].name.to_lowercase()),
            Tool::Line | Tool::Arc | Tool::ZigZag | Tool::PolyLine | Tool::Bezier => "Drag from one shape to another to connect them.".into(),
            Tool::Text => "Click where the text should go, then type.".into(),
            Tool::TextEdit => "Click a shape or text to type in it.".into(),
            Tool::Magnify => "Click to zoom in; right-click or Shift+click to zoom out.".into(),
            Tool::Scroll => "Drag to move around the diagram.".into(),
            Tool::Image => "Click where the picture should go.".into(),
            _ => "Click or drag on the diagram.".into(),
        }
    }

    pub(super) fn set_tool(&mut self, t: Tool, sticky: bool) {
        self.ed.end_edit(true);
        self.ed.tool = t;
        self.ed.sticky = sticky;
        self.status.clear();
    }

    /// Change the selection from the toolbox; several changes in a row (dragging a colour) make one undo step.
    fn style_change(&mut self, key: u32, f: impl Fn(&mut Obj)) {
        if self.ed.sel.is_empty() {
            return;
        }
        if self.coalesce == Some((key, self.ed.rev)) {
            let sel = self.ed.sel.clone();
            for o in self.ed.d.objects.iter_mut().filter(|o| sel.contains(&o.id)) {
                f(o);
                if let Kind::Element(e) = &mut o.kind {
                    e.fit_text();
                }
            }
            self.ed.notify_changed();
        } else {
            self.ed.apply_to_selection(f);
        }
        self.coalesce = Some((key, self.ed.rev));
    }

    pub(super) fn set_theme(&mut self, ctx: &egui::Context, t: ThemeChoice) {
        if self.theme == t {
            return;
        }
        self.theme = t;
        ctx.set_theme(t.preference());
        if self.shot.is_none() {
            save_theme(t);
        }
    }

    /// The menus inside the window (Windows and Linux; macOS uses the system menu bar instead).
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub(super) fn menu_bar(&mut self, ui: &mut Ui, act: &mut Option<Action>) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                let items: [(&str, &str, Action); 4] =
                    [("New diagram", "Ctrl+N", Action::New), ("Open…", "Ctrl+O", Action::Open), ("Save", "Ctrl+S", Action::Save), ("Save as…", "Ctrl+Shift+S", Action::SaveAs)];
                for (i, (t, k, a)) in items.iter().enumerate() {
                    if i == 2 {
                        ui.separator();
                    }
                    if menu_item(ui, t, k, true) {
                        *act = Some(*a);
                    }
                    if i == 1 {
                        // Recently used diagrams.
                        ui.add_enabled_ui(!self.recent.is_empty(), |ui| {
                            ui.menu_button("Open recent", |ui| {
                                for (n, path) in self.recent.iter().enumerate() {
                                    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                                    let folder = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                                    let label = format!("{}  {}", n + 1, name);
                                    let r = ui.add(egui::Button::new(label).shortcut_text(folder.clone())).on_hover_text(path.to_string_lossy());
                                    if r.clicked() {
                                        *act = Some(Action::OpenRecent(n));
                                        ui.close();
                                    }
                                }
                                ui.separator();
                                if menu_item(ui, "Clear the list", "", true) {
                                    *act = Some(Action::ClearRecent);
                                }
                            });
                        });
                    }
                }
                if menu_item(ui, "Close tab", "Ctrl+W", true) {
                    *act = Some(Action::CloseTab(usize::MAX));
                }
                ui.checkbox(&mut self.compress, "Save compressed (smaller files)");
                ui.separator();
                if menu_item(ui, "Export to PDF…", "Ctrl+E", true) {
                    *act = Some(Action::ExportPdf);
                }
                if menu_item(ui, "Export picture (PNG or SVG)…", "", true) {
                    *act = Some(Action::ExportPicture);
                }
                ui.separator();
                if menu_item(ui, "Import tables from MySQL…", "", true) {
                    *act = Some(Action::ImportMysql);
                }
                ui.separator();
                if menu_item(ui, "Page setup…", "", true) {
                    *act = Some(Action::PageSetup);
                }
                if menu_item(ui, "Print…", "Ctrl+P", true) {
                    *act = Some(Action::Print);
                }
                ui.separator();
                if menu_item(ui, "Exit", "Ctrl+Q", true) {
                    *act = Some(Action::Exit);
                }
            });
            ui.menu_button("Edit", |ui| {
                let any = !self.ed.sel.is_empty();
                let items: [(&str, &str, bool, Action); 7] = [
                    ("Undo", "Ctrl+Z", self.ed.can_undo(), Action::Undo),
                    ("Redo", "Ctrl+Y", self.ed.can_redo(), Action::Redo),
                    ("Cut", "Ctrl+X", any, Action::Cut),
                    ("Copy", "Ctrl+C", any, Action::Copy),
                    ("Paste", "Ctrl+V", self.ed.can_paste(), Action::Paste),
                    ("Duplicate", "Ctrl+D", any, Action::Duplicate),
                    ("Delete", "Del", any, Action::Delete),
                ];
                for (i, (t, k, en, a)) in items.iter().enumerate() {
                    if i == 2 {
                        ui.separator();
                    }
                    if menu_item(ui, t, k, *en) {
                        *act = Some(*a);
                    }
                }
                ui.separator();
                if menu_item(ui, "Diagram properties and page setup…", "", true) {
                    *act = Some(Action::PageSetup);
                }
            });
            ui.menu_button("View", |ui| {
                let items: [(&str, &str, Action); 4] = [
                    ("Zoom in", "Ctrl++", Action::ZoomIn),
                    ("Zoom out", "Ctrl+-", Action::ZoomOut),
                    ("Actual size (100%)", "Ctrl+1", Action::Zoom(1.0)),
                    ("Best fit", "Ctrl+Shift+E", Action::BestFit),
                ];
                for (t, k, a) in items {
                    if menu_item(ui, t, k, true) {
                        *act = Some(a);
                    }
                }
                ui.separator();
                let mut changed = false;
                changed |= ui.checkbox(&mut self.ed.show_grid, "Show grid").changed();
                changed |= ui.checkbox(&mut self.ed.snap_to_grid, "Snap to grid").changed();
                changed |= ui.checkbox(&mut self.ed.object_snap, "Snap lines to objects").changed();
                changed |= ui.checkbox(&mut self.ed.show_cps, "Show connection points").changed();
                changed |= ui.checkbox(&mut self.ed.show_pages, "Show page breaks").changed();
                changed |= ui.checkbox(&mut self.ed.show_rulers, "Show rulers").changed();
                if changed {
                    self.ed.touch();
                }
                ui.separator();
                ui.menu_button("Theme", |ui| {
                    for t in ThemeChoice::ALL {
                        if ui.radio(self.theme == t, t.label()).clicked() && self.theme != t {
                            self.set_theme(ui.ctx(), t);
                            ui.close();
                        }
                    }
                });
            });
            ui.menu_button("Layers", |ui| {
                if menu_item(ui, "Add layer", "", true) {
                    *act = Some(Action::AddLayer);
                }
                if menu_item(ui, "Move selection to the active layer", "", !self.ed.sel.is_empty()) {
                    *act = Some(Action::MoveToLayer);
                }
                if menu_item(ui, "Show all layers", "", self.ed.d.layers.iter().any(|l| !l.visible)) {
                    *act = Some(Action::ShowAllLayers);
                }
                ui.separator();
                ui.label(egui::RichText::new("Shown   Active layer").color(pal(ui.ctx()).hint));
                let mut changed = false;
                let n = self.ed.d.layers.len();
                for i in 0..n {
                    ui.horizontal(|ui| {
                        changed |= ui.checkbox(&mut self.ed.d.layers[i].visible, "").changed();
                        let name = self.ed.d.layers[i].name.clone();
                        if ui.radio(self.ed.d.active == i, name).clicked() {
                            self.ed.d.active = i;
                        }
                    });
                }
                if changed {
                    self.ed.sel.retain(|id| self.ed.d.get(*id).map(|o| self.ed.d.layer_visible(&o.layer)).unwrap_or(false));
                    self.ed.notify_changed();
                }
            });
            ui.menu_button("Objects", |ui| {
                let any = !self.ed.sel.is_empty();
                let two = self.ed.sel.len() >= 2;
                let items: [(&str, &str, bool, Action); 6] = [
                    ("Properties…", "Alt+Enter", true, Action::Props),
                    ("Edit text", "F2", self.ed.sel.len() == 1, Action::EditText),
                    ("Bring to front", "Ctrl+F", any, Action::Reorder(0)),
                    ("Send to back", "Ctrl+B", any, Action::Reorder(1)),
                    ("Raise", "", any, Action::Reorder(2)),
                    ("Lower", "", any, Action::Reorder(3)),
                ];
                for (i, (t, k, en, a)) in items.iter().enumerate() {
                    if i == 2 {
                        ui.separator();
                    }
                    if menu_item(ui, t, k, *en) {
                        *act = Some(*a);
                    }
                }
                ui.separator();
                ui.menu_button("Align", |ui| {
                    let items = [
                        ("Left edges", 0u8),
                        ("Centres (left–right)", 1),
                        ("Right edges", 2),
                        ("Top edges", 3),
                        ("Middles (up–down)", 4),
                        ("Bottom edges", 5),
                        ("Spread evenly across", 6),
                        ("Spread evenly down", 7),
                    ];
                    for (i, (name, h)) in items.iter().enumerate() {
                        if i == 3 || i == 6 {
                            ui.separator();
                        }
                        if menu_item(ui, name, "", two) {
                            *act = Some(Action::Align(*h));
                        }
                    }
                });
            });
            ui.menu_button("Select", |ui| {
                let any = !self.ed.sel.is_empty();
                let items: [(&str, &str, bool, Action); 5] = [
                    ("All", "Ctrl+A", true, Action::SelectAll),
                    ("None", "Ctrl+Shift+A", any, Action::SelectNone),
                    ("Invert", "", true, Action::SelectInvert),
                    ("Same type", "", any, Action::SelectSameType),
                    ("Connected", "", any, Action::SelectConnected),
                ];
                for (t, k, en, a) in items {
                    if menu_item(ui, t, k, en) {
                        *act = Some(a);
                    }
                }
            });
            ui.menu_button("Tools", |ui| {
                for (t, tip) in TOOLBOX_TOOLS {
                    let name = tip.split(':').next().unwrap_or(tip).to_string();
                    if ui.add(egui::Button::selectable(self.ed.tool == t, name)).clicked() {
                        self.set_tool(t, false);
                        ui.close();
                    }
                }
            });
            ui.menu_button("Help", |ui| {
                if menu_item(ui, "Open the example flowchart", "", true) {
                    *act = Some(Action::Example);
                }
                if menu_item(ui, "Use with Claude Code (MCP)…", "", true) {
                    *act = Some(Action::McpHelp);
                }
                if menu_item(ui, "Tips", "F1", true) {
                    *act = Some(Action::Tips);
                }
                if menu_item(ui, "About", "", true) {
                    *act = Some(Action::About);
                }
            });
        });
    }

    pub(super) fn toolbar(&mut self, ui: &mut Ui, act: &mut Option<Action>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let any = !self.ed.sel.is_empty();
            let items: [(Icon, &str, bool, Action); 14] = [
                (Icon::New, "New diagram", true, Action::New),
                (Icon::Open, "Open a diagram", true, Action::Open),
                (Icon::Save, "Save", true, Action::Save),
                (Icon::SaveAs, "Save as", true, Action::SaveAs),
                (Icon::Pdf, "Export to PDF", true, Action::ExportPdf),
                (Icon::Print, "Print", true, Action::Print),
                (Icon::Copy, "Copy", any, Action::Copy),
                (Icon::Cut, "Cut", any, Action::Cut),
                (Icon::Paste, "Paste", self.ed.can_paste(), Action::Paste),
                (Icon::Undo, "Undo", self.ed.can_undo(), Action::Undo),
                (Icon::Redo, "Redo", self.ed.can_redo(), Action::Redo),
                (Icon::ZoomIn, "Zoom in", true, Action::ZoomIn),
                (Icon::Fit, "Best fit", true, Action::BestFit),
                (Icon::ZoomOut, "Zoom out", true, Action::ZoomOut),
            ];
            for (i, (icon, tip, enabled, a)) in items.iter().enumerate() {
                if i == 6 || i == 9 || i == 11 {
                    ui.separator();
                }
                if icon_button(ui, *icon, tip, *enabled, false) {
                    *act = Some(*a);
                }
            }
            let cur = format!("{:.0}%", self.ed.zoom * 100.0);
            egui::ComboBox::from_id_salt("zoom").width(62.0).selected_text(cur).show_ui(ui, |ui| {
                for z in ZOOMS {
                    let name = format!("{:.1}%", z * 100.0).replace(".0%", "%");
                    if ui.selectable_label((self.ed.zoom - z).abs() < 1e-3, name).clicked() {
                        *act = Some(Action::Zoom(z));
                    }
                }
            });
            ui.separator();
            if icon_button(ui, Icon::Snap, "Snap to grid", true, self.ed.snap_to_grid) {
                self.ed.snap_to_grid = !self.ed.snap_to_grid;
            }
            if icon_button(ui, Icon::ObjSnap, "Snap lines to objects", true, self.ed.object_snap) {
                self.ed.object_snap = !self.ed.object_snap;
            }
        });
    }

    pub(super) fn toolbox(&mut self, ui: &mut Ui) {
        // The attribute area sits at the bottom of the toolbox.
        egui::Panel::bottom("toolbox-attrs").frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6))).show(ui, |ui| self.attributes(ui));
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                for (t, tip) in TOOLBOX_TOOLS {
                    let sel = self.ed.tool == t;
                    let (rect, resp) = tool_cell(ui, 36.0, sel, sel && self.ed.sticky);
                    paint_tool(ui.painter(), rect.shrink(9.0), t);
                    let resp = resp.on_hover_text(format!("{tip} — double-click to keep using it"));
                    if resp.double_clicked() {
                        self.set_tool(t, true);
                    } else if resp.clicked() {
                        self.set_tool(t, false);
                    }
                }
            });
            ui.add_space(6.0);
            egui::ComboBox::from_id_salt("sheet").width(146.0).selected_text(shapes::SHEETS[self.sheet]).show_ui(ui, |ui| {
                for (i, s) in shapes::SHEETS.iter().enumerate() {
                    ui.selectable_value(&mut self.sheet, i, *s);
                }
            });
            ui.add_space(4.0);
            let sheet = shapes::SHEETS[self.sheet];
            let defs: Vec<usize> = shapes::all().iter().enumerate().filter(|(_, d)| d.sheet == sheet).map(|(i, _)| i).collect();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                // Shape pictures are remade when the theme flips between light and dark.
                let dark = pal(ui.ctx()).dark;
                if dark != self.icons_dark {
                    self.icons.clear();
                    self.icons_dark = dark;
                }
                for i in defs {
                    let ctx = ui.ctx().clone();
                    let tex = self.icons.entry(i).or_insert_with(|| {
                        let pm = skia::shape_icon(i, 64);
                        let mut rgba = pm.data().to_vec();
                        if dark {
                            crate::theme::darken_icon(&mut rgba);
                        }
                        let img = egui::ColorImage::from_rgba_premultiplied([64, 64], &rgba);
                        ctx.load_texture(format!("shape-{i}"), img, egui::TextureOptions::LINEAR)
                    });
                    let id = tex.id();
                    let sel = self.ed.tool == Tool::Shape(i);
                    let (rect, resp) = tool_cell(ui, 36.0, sel, sel && self.ed.sticky);
                    ui.painter().image(id, rect.shrink(3.0), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    let resp = resp.on_hover_text(format!("{} — click, then click or drag on the diagram", shapes::def(i).name));
                    if resp.double_clicked() {
                        self.set_tool(Tool::Shape(i), true);
                    } else if resp.clicked() {
                        self.set_tool(Tool::Shape(i), false);
                    }
                }
                // Ready-made connector lines for this sheet.
                for (i, pr) in shapes::LINE_PRESETS.iter().enumerate().filter(|(_, pr)| pr.sheet == sheet) {
                    let sel = self.ed.tool == Tool::Connector(i);
                    let (rect, resp) = tool_cell(ui, 36.0, sel, sel && self.ed.sticky);
                    paint_preset(ui.painter(), rect.shrink(4.0), pr);
                    let resp = resp.on_hover_text(format!("{} — drag from one shape to another", pr.name));
                    if resp.double_clicked() {
                        self.set_tool(Tool::Connector(i), true);
                    } else if resp.clicked() {
                        self.set_tool(Tool::Connector(i), false);
                    }
                }
            });
        });
    }

    /// Line/fill colours, line width, and the start-arrow / line-style / end-arrow buttons.
    fn attributes(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // Colour swatch: front square = line colour, back square = fill colour.
            let (rect, _) = ui.allocate_exact_size(vec2(64.0, 50.0), Sense::hover());
            let back = Rect::from_min_size(rect.min + vec2(20.0, 18.0), vec2(30.0, 28.0));
            let front = Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(30.0, 28.0));
            let swap = Rect::from_min_size(rect.min + vec2(40.0, 0.0), vec2(16.0, 15.0));
            let reset = Rect::from_min_size(rect.min + vec2(3.0, 36.0), vec2(12.0, 12.0));
            let rb = ui.interact(back, ui.scope_id().with("fill-swatch"), Sense::click()).on_hover_text("Fill colour");
            let rf = ui.interact(front, ui.scope_id().with("line-swatch"), Sense::click()).on_hover_text("Line and text colour");
            let rs = ui.interact(swap, ui.scope_id().with("swap"), Sense::click()).on_hover_text("Swap the colours");
            let rr = ui.interact(reset, ui.scope_id().with("reset"), Sense::click()).on_hover_text("Black and white");
            let p = ui.painter();
            let pal = pal(ui.ctx());
            let edge = if pal.dark { Color32::from_gray(150) } else { Color32::from_gray(90) };
            p.rect_filled(back, 0.0, c32(self.ed.style.fill_color));
            p.rect_stroke(back, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
            p.rect_filled(front, 0.0, c32(self.ed.style.line_color));
            p.rect_stroke(front, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
            let sc = swap.center();
            let arrow = Stroke::new(1.3, pal.ink);
            p.line_segment([sc + vec2(-5.0, -4.0), sc + vec2(4.0, -4.0)], arrow);
            p.line_segment([sc + vec2(4.0, -4.0), sc + vec2(4.0, 5.0)], arrow);
            p.line_segment([sc + vec2(-5.0, -4.0), sc + vec2(-2.0, -7.0)], arrow);
            p.line_segment([sc + vec2(4.0, 5.0), sc + vec2(7.0, 2.0)], arrow);
            p.rect_filled(Rect::from_min_size(reset.min + vec2(4.0, 4.0), vec2(8.0, 8.0)), 0.0, Color32::WHITE);
            p.rect_stroke(Rect::from_min_size(reset.min + vec2(4.0, 4.0), vec2(8.0, 8.0)), 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
            p.rect_filled(Rect::from_min_size(reset.min, vec2(8.0, 8.0)), 0.0, Color32::BLACK);
            p.rect_stroke(Rect::from_min_size(reset.min, vec2(8.0, 8.0)), 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
            if rs.clicked() {
                let s = &mut self.ed.style;
                std::mem::swap(&mut s.line_color, &mut s.fill_color);
            }
            if rr.clicked() {
                self.ed.style.line_color = Col::BLACK;
                self.ed.style.fill_color = Col::WHITE;
            }
            let mut new_line = None;
            let mut new_fill = None;
            egui::Popup::menu(&rf).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                let mut c = c32(self.ed.style.line_color);
                if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
                    new_line = Some(col(c));
                }
            });
            egui::Popup::menu(&rb).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                let mut c = c32(self.ed.style.fill_color);
                if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
                    new_fill = Some(col(c));
                }
            });
            if let Some(v) = new_line {
                self.ed.style.line_color = v;
                self.style_change(1, move |o| set_line_color(o, v));
            }
            if let Some(v) = new_fill {
                self.ed.style.fill_color = v;
                self.style_change(2, move |o| set_fill_color(o, v));
            }

            // Line widths: thin to thick bars; the current one has a dotted frame.
            let (wrect, wresp) = ui.allocate_exact_size(vec2(72.0, 50.0), Sense::click());
            let bars: [(f64, f32); 5] = [(0.05, 1.0), (0.1, 2.0), (0.15, 3.5), (0.2, 5.0), (0.3, 7.0)];
            let p = ui.painter();
            let ink = pal.ink;
            let mut x = wrect.min.x + 6.0;
            let mut hit = None;
            for (w, px) in bars {
                let bar = Rect::from_min_size(pos2(x, wrect.min.y + 6.0), vec2(px, 38.0));
                p.rect_filled(bar, 0.0, ink);
                let cell = Rect::from_min_max(pos2(x - 4.0, wrect.min.y), pos2(x + px + 4.0, wrect.max.y));
                if (self.ed.style.line_width - w).abs() < 1e-6 {
                    let pts = vec![cell.left_top(), cell.right_top(), cell.right_bottom(), cell.left_bottom(), cell.left_top()];
                    p.extend(Shape::dashed_line(&pts, Stroke::new(1.0, ink), 2.0, 2.0));
                }
                if let Some(q) = wresp.interact_pointer_pos() {
                    if wresp.clicked() && q.x >= cell.min.x && q.x <= cell.max.x {
                        hit = Some(w);
                    }
                }
                x += px + 8.0;
            }
            let wresp = wresp.on_hover_text(format!("Line width: {:.2} cm", self.ed.style.line_width));
            let _ = wresp;
            if let Some(w) = hit {
                self.ed.style.line_width = w;
                self.style_change(3, move |o| set_width(o, w));
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for which in 0..3 {
                let (rect, resp) = ui.allocate_exact_size(vec2(48.0, 28.0), Sense::click());
                let p = ui.painter();
                let pal = pal(ui.ctx());
                p.rect_filled(rect, 3.0, if resp.hovered() { pal.hover } else { pal.cell });
                p.rect_stroke(rect, 3.0, Stroke::new(1.0, pal.cell_border), StrokeKind::Inside);
                match which {
                    0 => paint_arrow(p, rect, self.ed.style.start_arrow, true),
                    1 => paint_style(p, rect, self.ed.style.style),
                    _ => paint_arrow(p, rect, self.ed.style.end_arrow, false),
                }
                let tip = ["Arrow at the start of lines", "Line style", "Arrow at the end of lines"][which];
                let resp = resp.on_hover_text(tip);
                let mut pick: Option<i64> = None;
                egui::Popup::menu(&resp).show(|ui| {
                    let n = if which == 1 { 5 } else { ARROW_KINDS.len() };
                    for k in 0..n {
                        let (r, rsp) = ui.allocate_exact_size(vec2(110.0, 22.0), Sense::click());
                        let p = ui.painter();
                        if rsp.hovered() {
                            p.rect_filled(r, 2.0, pal.hover);
                        }
                        let inner = r.shrink2(vec2(8.0, 0.0));
                        match which {
                            1 => paint_style(p, inner, LineStyle::ALL[k]),
                            0 => paint_arrow(p, inner, ARROW_KINDS[k].0, true),
                            _ => paint_arrow(p, inner, ARROW_KINDS[k].0, false),
                        }
                        let label = if which == 1 { LineStyle::ALL[k].name() } else { ARROW_KINDS[k].1 };
                        if rsp.on_hover_text(label).clicked() {
                            pick = Some(if which == 1 { k as i64 } else { ARROW_KINDS[k].0 });
                            ui.close();
                        }
                    }
                });
                if let Some(v) = pick {
                    match which {
                        0 => {
                            self.ed.style.start_arrow = v;
                            self.style_change(5, move |o| {
                                if let Kind::Line(l) = &mut o.kind {
                                    l.start_arrow.kind = v;
                                }
                            });
                        }
                        1 => {
                            let st = LineStyle::from_i(v);
                            self.ed.style.style = st;
                            self.style_change(4, move |o| set_style(o, st));
                        }
                        _ => {
                            self.ed.style.end_arrow = v;
                            self.style_change(6, move |o| {
                                if let Kind::Line(l) = &mut o.kind {
                                    l.end_arrow.kind = v;
                                }
                            });
                        }
                    }
                }
            }
        });
    }
}

pub(super) fn set_line_color(o: &mut Obj, c: Col) {
    match &mut o.kind {
        Kind::Element(e) => {
            e.line_color = c;
            if let Some(t) = &mut e.text {
                t.color = c;
            }
        }
        Kind::Line(l) => l.color = c,
        Kind::Polygon(pg) => pg.line_color = c,
        Kind::Text(t) => t.text.color = c,
        Kind::Image(im) => im.line_color = c,
        Kind::Raw(_) => {}
    }
}

pub(super) fn set_fill_color(o: &mut Obj, c: Col) {
    match &mut o.kind {
        Kind::Element(e) => {
            e.fill_color = c;
            e.filled = true;
        }
        Kind::Polygon(pg) => {
            pg.fill_color = c;
            pg.filled = true;
        }
        _ => {}
    }
}

pub(super) fn set_width(o: &mut Obj, w: f64) {
    match &mut o.kind {
        Kind::Element(e) => e.line_width = w,
        Kind::Line(l) => l.width = w,
        Kind::Polygon(pg) => pg.line_width = w,
        Kind::Image(im) => im.line_width = w,
        _ => {}
    }
}

pub(super) fn set_style(o: &mut Obj, s: LineStyle) {
    match &mut o.kind {
        Kind::Element(e) => e.style = s,
        Kind::Line(l) => l.style = s,
        Kind::Polygon(pg) => pg.style = s,
        _ => {}
    }
}
