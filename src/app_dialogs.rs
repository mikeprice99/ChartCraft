//! Dialogs (properties, page setup, PDF export, tips) and the per-frame window layout.

use super::*;

fn color_row(ui: &mut Ui, name: &str, c: &mut Col) {
    ui.label(name);
    let mut v = c32(*c);
    if egui::color_picker::color_edit_button_srgba(ui, &mut v, egui::color_picker::Alpha::Opaque).changed() {
        *c = col(v);
    }
    ui.end_row();
}

fn num_row(ui: &mut Ui, name: &str, v: &mut f64, lo: f64, hi: f64, speed: f64) {
    ui.label(name);
    ui.add(egui::DragValue::new(v).range(lo..=hi).speed(speed).max_decimals(2));
    ui.end_row();
}

fn style_row(ui: &mut Ui, s: &mut LineStyle) {
    ui.label("Line style");
    egui::ComboBox::from_id_salt(ui.scope_id().with("style")).selected_text(s.name()).show_ui(ui, |ui| {
        for v in LineStyle::ALL {
            ui.selectable_value(s, v, v.name());
        }
    });
    ui.end_row();
}

fn arrow_row(ui: &mut Ui, name: &str, k: &mut i64) {
    ui.label(name);
    egui::ComboBox::from_id_salt(ui.scope_id().with(name)).selected_text(arrow_name(*k)).show_ui(ui, |ui| {
        for (id, n) in ARROW_KINDS {
            ui.selectable_value(k, *id, *n);
        }
    });
    ui.end_row();
}

fn text_rows(ui: &mut Ui, t: &mut TextSpec, edit_text: bool) {
    if edit_text {
        ui.label("Text");
        ui.add(egui::TextEdit::multiline(&mut t.text).desired_rows(3).desired_width(240.0));
        ui.end_row();
    }
    ui.label("Font");
    let fams = ["Sans (Arial)", "Serif (Times)", "Monospace (Courier)"];
    let mut fi = crate::fonts::family_index(&t.family);
    let before = fi;
    egui::ComboBox::from_id_salt(ui.scope_id().with("font")).selected_text(fams[fi]).show_ui(ui, |ui| {
        for (i, f) in fams.iter().enumerate() {
            ui.selectable_value(&mut fi, i, *f);
        }
    });
    if fi != before {
        t.family = ["sans", "serif", "monospace"][fi].into();
    }
    ui.end_row();
    ui.label("Style");
    ui.horizontal(|ui| {
        ui.checkbox(&mut t.bold, "Bold");
        ui.checkbox(&mut t.italic, "Italic");
    });
    ui.end_row();
    num_row(ui, "Text height (cm)", &mut t.height, 0.1, 20.0, 0.05);
    color_row(ui, "Text colour", &mut t.color);
    ui.label("Alignment");
    ui.horizontal(|ui| {
        ui.radio_value(&mut t.align, 0, "Left");
        ui.radio_value(&mut t.align, 1, "Centre");
        ui.radio_value(&mut t.align, 2, "Right");
    });
    ui.end_row();
}

fn copy_text_style(from: &TextSpec, to: &mut TextSpec) {
    to.family = from.family.clone();
    to.bold = from.bold;
    to.italic = from.italic;
    to.height = from.height;
    to.color = from.color;
    to.align = from.align;
}

/// Copy the look (not the position) of `from` onto `to` if they're the same kind.
fn copy_style(from: &Obj, to: &mut Obj) {
    let same = from.id == to.id;
    match (&from.kind, &mut to.kind) {
        (Kind::Element(a), Kind::Element(b)) => {
            b.line_color = a.line_color;
            b.fill_color = a.fill_color;
            b.filled = a.filled;
            b.line_width = a.line_width;
            b.style = a.style;
            b.dash = a.dash;
            if b.shape().fam == shapes::Family::StdBox && a.shape().fam == shapes::Family::StdBox {
                b.corner_radius = a.corner_radius;
                b.def = shapes::index_of(if b.corner_radius > 0.0 { "std-roundbox" } else { "std-box" });
            }
            if a.shape().key == b.shape().key {
                b.shear_angle = a.shear_angle;
            }
            if same {
                b.rec = a.rec.clone().map(|mut r| {
                    r.tidy();
                    r
                });
            }
            if let (Some(ta), Some(tb)) = (&a.text, &mut b.text) {
                copy_text_style(ta, tb);
                b.padding = a.padding;
            }
        }
        (Kind::Text(a), Kind::Text(b)) => {
            copy_text_style(&a.text, &mut b.text);
            if same {
                b.text.text = a.text.text.clone();
            }
        }
        (Kind::Line(a), Kind::Line(b)) => {
            b.color = a.color;
            b.width = a.width;
            b.style = a.style;
            b.dash = a.dash;
            b.start_arrow = a.start_arrow;
            b.end_arrow = a.end_arrow;
            if same {
                b.curve = a.curve;
            }
        }
        (Kind::Polygon(a), Kind::Polygon(b)) => {
            b.line_color = a.line_color;
            b.fill_color = a.fill_color;
            b.filled = a.filled;
            b.line_width = a.line_width;
            b.style = a.style;
        }
        (Kind::Image(a), Kind::Image(b)) => {
            b.keep_aspect = a.keep_aspect;
            b.draw_border = a.draw_border;
            b.line_color = a.line_color;
            b.line_width = a.line_width;
        }
        _ => {}
    }
}

fn window(title: impl Into<egui::WidgetText>) -> egui::Window<'static> {
    egui::Window::new(title).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0])
}

fn ok_cancel(ui: &mut Ui, ok: &mut bool, cancel: &mut bool) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("   OK   ").clicked() {
            *ok = true;
        }
        if ui.button("Cancel").clicked() {
            *cancel = true;
        }
    });
}

/// What a list's side buttons asked for.
enum ListOp {
    New,
    Delete,
    Up,
    Down,
}

/// A white list of rows with New / Delete / Move up / Move down beside it, like the original's editors.
fn row_list(ui: &mut Ui, id: &str, labels: &[String], sel: &mut Option<usize>, width: f32, height: f32) -> Option<ListOp> {
    let mut op = None;
    ui.horizontal_top(|ui| {
        egui::Frame::new().fill(pal(ui.ctx()).field).stroke(Stroke::new(1.0, pal(ui.ctx()).field_border)).inner_margin(2).show(ui, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            egui::ScrollArea::vertical().id_salt(id).max_height(height).auto_shrink([false, false]).show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for (i, l) in labels.iter().enumerate() {
                        let text = egui::RichText::new(if l.is_empty() { "(no name)" } else { l.as_str() }).monospace();
                        if ui.selectable_label(*sel == Some(i), text).clicked() {
                            *sel = Some(i);
                        }
                    }
                });
            });
        });
        ui.vertical(|ui| {
            let w = vec2(84.0, 0.0);
            if ui.add(egui::Button::new("New").min_size(w)).clicked() {
                op = Some(ListOp::New);
            }
            if ui.add_enabled(sel.is_some(), egui::Button::new("Delete").min_size(w)).clicked() {
                op = Some(ListOp::Delete);
            }
            if ui.add_enabled(matches!(*sel, Some(i) if i > 0), egui::Button::new("Move up").min_size(w)).clicked() {
                op = Some(ListOp::Up);
            }
            if ui.add_enabled(matches!(*sel, Some(i) if i + 1 < labels.len()), egui::Button::new("Move down").min_size(w)).clicked() {
                op = Some(ListOp::Down);
            }
        });
    });
    op
}

/// Apply a list button to a list of items.
fn list_apply<T: Clone>(items: &mut Vec<T>, sel: &mut Option<usize>, op: ListOp, new: T) {
    match op {
        ListOp::New => {
            let at = sel.map(|i| i + 1).unwrap_or(items.len()).min(items.len());
            items.insert(at, new);
            *sel = Some(at);
        }
        ListOp::Delete => {
            if let Some(i) = *sel {
                if i < items.len() {
                    items.remove(i);
                }
                *sel = if items.is_empty() { None } else { Some(i.min(items.len() - 1)) };
            }
        }
        ListOp::Up => {
            if let Some(i) = *sel {
                if i > 0 && i < items.len() {
                    items.swap(i, i - 1);
                    *sel = Some(i - 1);
                }
            }
        }
        ListOp::Down => {
            if let Some(i) = *sel {
                if i + 1 < items.len() {
                    items.swap(i, i + 1);
                    *sel = Some(i + 1);
                }
            }
        }
    }
}

const VISIBILITY: [&str; 4] = ["Public (+)", "Private (-)", "Protected (#)", "Implementation"];

fn vis_combo(ui: &mut Ui, id: &str, v: &mut i64) {
    egui::ComboBox::from_id_salt(id).selected_text(VISIBILITY[(*v).clamp(0, 3) as usize]).show_ui(ui, |ui| {
        for (i, n) in VISIBILITY.iter().enumerate() {
            ui.selectable_value(v, i as i64, *n);
        }
    });
}

fn field(ui: &mut Ui, label: &str, v: &mut String, hint: &str) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(v).desired_width(300.0).hint_text(hint));
    ui.end_row();
}

fn note_field(ui: &mut Ui, label: &str, v: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::multiline(v).desired_rows(2).desired_width(300.0));
    ui.end_row();
}

impl App {
    /// The UML class editor, laid out like the original's: Class / Attributes / Operations / Style tabs.
    fn class_window(&mut self, ctx: &egui::Context) {
        use crate::record::{op_lines, attr_text, ClassAttr, ClassOp, Param};
        let (mut open, mut ok, mut apply, mut close) = (true, false, false, false);
        let Some(dlg) = self.props.as_mut() else { return };
        let PropsDialog { draft, tab, row, prow } = dlg;
        let Kind::Element(e) = &mut draft.kind else { return };
        window("Properties: UML - Class").id(egui::Id::unique("class-props")).open(&mut open).show(ctx, |ui| {
            ui.set_min_width(430.0);
            ui.horizontal(|ui| {
                for (i, name) in ["Class", "Attributes", "Operations", "Style"].iter().enumerate() {
                    if ui.selectable_label(*tab == i as u8, *name).clicked() && *tab != i as u8 {
                        *tab = i as u8;
                        *row = Some(0);
                        *prow = Some(0);
                    }
                }
            });
            ui.separator();
            let Some(r) = e.rec.as_mut() else { return };
            match *tab {
                0 => {
                    egui::Grid::new("class-tab").num_columns(2).spacing(vec2(8.0, 8.0)).show(ui, |ui| {
                        field(ui, "Class name:", &mut r.name, "");
                        field(ui, "Stereotype:", &mut r.stereotype, "e.g. class, interface (optional)");
                        note_field(ui, "Comment:", &mut r.comment);
                    });
                    ui.add_space(6.0);
                    egui::Grid::new("class-flags").num_columns(2).spacing(vec2(30.0, 6.0)).show(ui, |ui| {
                        ui.checkbox(&mut r.is_abstract, "Abstract");
                        ui.checkbox(&mut r.show_comment, "Comments visible");
                        ui.end_row();
                        ui.checkbox(&mut r.wrap_ops, "Wrap operations");
                        ui.checkbox(&mut r.tag_comment, "Show documentation tag");
                        ui.end_row();
                    });
                    egui::Grid::new("class-nums").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                        ui.label("Wrap after this many characters:");
                        ui.add(egui::DragValue::new(&mut r.wrap_after).range(10..=200));
                        ui.end_row();
                        ui.label("Comment line length:");
                        ui.add(egui::DragValue::new(&mut r.comment_len).range(5..=400));
                        ui.end_row();
                    });
                    ui.add_space(60.0);
                }
                1 => {
                    let labels: Vec<String> = r.attrs.iter().map(attr_text).collect();
                    if let Some(op) = row_list(ui, "attr-list", &labels, row, 310.0, 130.0) {
                        let n = r.attrs.len() + 1;
                        list_apply(&mut r.attrs, row, op, ClassAttr { name: format!("attribute{n}"), ..Default::default() });
                    }
                    ui.add_space(8.0);
                    let sel = row.filter(|i| *i < r.attrs.len());
                    let mut empty = ClassAttr::default();
                    let a = match sel {
                        Some(i) => &mut r.attrs[i],
                        None => &mut empty,
                    };
                    ui.group(|ui| {
                        ui.set_min_width(410.0);
                        ui.label(egui::RichText::new("Attribute data").strong());
                        ui.add_enabled_ui(sel.is_some(), |ui| {
                            egui::Grid::new("attr-grid").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                                field(ui, "Name:", &mut a.name, "");
                                field(ui, "Type:", &mut a.ty, "e.g. int, string");
                                field(ui, "Value:", &mut a.value, "starting value (optional)");
                                note_field(ui, "Comment:", &mut a.comment);
                                ui.label("Visibility:");
                                vis_combo(ui, "attr-vis", &mut a.vis);
                                ui.end_row();
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut a.class_scope, "Class scope (static)");
                                ui.checkbox(&mut a.is_abstract, "Abstract");
                            });
                        });
                    });
                }
                2 => {
                    let labels: Vec<String> = r.ops.iter().map(|o| op_lines(o, false, 0).join("")).collect();
                    if let Some(op) = row_list(ui, "op-list", &labels, row, 310.0, 110.0) {
                        let n = r.ops.len() + 1;
                        list_apply(&mut r.ops, row, op, ClassOp { name: format!("operation{n}"), inheritance: 2, ..Default::default() });
                        *prow = None;
                    }
                    ui.add_space(6.0);
                    let sel = row.filter(|i| *i < r.ops.len());
                    let mut empty = ClassOp { inheritance: 2, ..Default::default() };
                    let o = match sel {
                        Some(i) => &mut r.ops[i],
                        None => &mut empty,
                    };
                    ui.group(|ui| {
                        ui.set_min_width(410.0);
                        ui.label(egui::RichText::new("Operation data").strong());
                        ui.add_enabled_ui(sel.is_some(), |ui| {
                            egui::Grid::new("op-grid").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
                                field(ui, "Name:", &mut o.name, "");
                                field(ui, "Type:", &mut o.ty, "what it returns (optional)");
                                field(ui, "Stereotype:", &mut o.stereotype, "");
                                note_field(ui, "Comment:", &mut o.comment);
                                ui.label("Visibility:");
                                vis_combo(ui, "op-vis", &mut o.vis);
                                ui.end_row();
                                ui.label("Inheritance:");
                                let kinds = ["Abstract", "Polymorphic (virtual)", "Leaf (final)"];
                                egui::ComboBox::from_id_salt("op-inh").selected_text(kinds[o.inheritance.clamp(0, 2) as usize]).show_ui(ui, |ui| {
                                    for (i, n) in kinds.iter().enumerate() {
                                        ui.selectable_value(&mut o.inheritance, i as i64, *n);
                                    }
                                });
                                ui.end_row();
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut o.class_scope, "Class scope (static)");
                                ui.checkbox(&mut o.query, "Query (const)");
                            });
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("Parameters").strong());
                            let plabels: Vec<String> = o.params.iter().map(crate::record::param_text).collect();
                            if let Some(op) = row_list(ui, "param-list", &plabels, prow, 310.0, 60.0) {
                                let n = o.params.len() + 1;
                                list_apply(&mut o.params, prow, op, Param { name: format!("param{n}"), ..Default::default() });
                            }
                            let psel = prow.filter(|i| *i < o.params.len());
                            let mut pempty = Param::default();
                            let pm = match psel {
                                Some(i) => &mut o.params[i],
                                None => &mut pempty,
                            };
                            ui.add_enabled_ui(psel.is_some(), |ui| {
                                egui::Grid::new("param-grid").num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
                                    field(ui, "Name:", &mut pm.name, "");
                                    field(ui, "Type:", &mut pm.ty, "");
                                    field(ui, "Default:", &mut pm.value, "");
                                    ui.label("Direction:");
                                    let dirs = ["Undefined", "In", "Out", "In and out"];
                                    egui::ComboBox::from_id_salt("param-kind").selected_text(dirs[pm.kind.clamp(0, 3) as usize]).show_ui(ui, |ui| {
                                        for (i, n) in dirs.iter().enumerate() {
                                            ui.selectable_value(&mut pm.kind, i as i64, *n);
                                        }
                                    });
                                    ui.end_row();
                                });
                            });
                        });
                    });
                }
                _ => {
                    egui::Grid::new("class-style").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                        color_row(ui, "Line colour", &mut e.line_color);
                        color_row(ui, "Fill colour", &mut e.fill_color);
                        if let Some(t) = &mut e.text {
                            color_row(ui, "Text colour", &mut t.color);
                        }
                        num_row(ui, "Line width (cm)", &mut e.line_width, 0.0, 5.0, 0.01);
                        num_row(ui, "Attribute text height (cm)", &mut r.h_normal, 0.1, 10.0, 0.05);
                        num_row(ui, "Class name height (cm)", &mut r.h_name, 0.1, 10.0, 0.05);
                        num_row(ui, "Comment height (cm)", &mut r.h_comment, 0.1, 10.0, 0.05);
                    });
                    ui.add_space(60.0);
                }
            }
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("OK").min_size(vec2(76.0, 0.0))).clicked() {
                    ok = true;
                }
                if ui.add(egui::Button::new("Apply").min_size(vec2(76.0, 0.0))).clicked() {
                    apply = true;
                }
                if ui.add(egui::Button::new("Close").min_size(vec2(76.0, 0.0))).clicked() {
                    close = true;
                }
            });
        });
        if ok || apply {
            let draft = self.props.as_ref().unwrap().draft.clone();
            self.ed.apply_to_selection(|o| copy_style(&draft, o));
            if apply {
                if let Some(o) = self.ed.d.get(draft.id) {
                    if let Some(d) = self.props.as_mut() {
                        d.draft = o.clone();
                    }
                }
            }
        }
        if ok || close || !open {
            self.props = None;
        }
    }

    /// The table editor, laid out like the original's: Table / Attributes / Style tabs.
    fn table_window(&mut self, ctx: &egui::Context) {
        let (mut open, mut ok, mut apply, mut close) = (true, false, false, false);
        let Some(dlg) = self.props.as_mut() else { return };
        let PropsDialog { draft, tab, row, .. } = dlg;
        let Kind::Element(e) = &mut draft.kind else { return };
        window("Properties: Database - Table").id(egui::Id::unique("table-props")).open(&mut open).show(ctx, |ui| {
            ui.set_min_width(390.0);
            ui.horizontal(|ui| {
                for (i, name) in ["Table", "Attributes", "Style"].iter().enumerate() {
                    ui.selectable_value(tab, i as u8, *name);
                }
            });
            ui.separator();
            let Some(r) = e.rec.as_mut() else { return };
            match *tab {
                0 => {
                    egui::Grid::new("table-tab").num_columns(2).spacing(vec2(8.0, 8.0)).show(ui, |ui| {
                        ui.label("Table name:");
                        ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(290.0));
                        ui.end_row();
                        ui.label("Comment:");
                        ui.add(egui::TextEdit::multiline(&mut r.comment).desired_rows(3).desired_width(290.0));
                        ui.end_row();
                    });
                    ui.add_space(6.0);
                    egui::Grid::new("table-flags").num_columns(2).spacing(vec2(30.0, 6.0)).show(ui, |ui| {
                        ui.checkbox(&mut r.show_comment, "Comment visible");
                        ui.checkbox(&mut r.tag_comment, "Show documentation tag");
                        ui.end_row();
                        ui.checkbox(&mut r.underline_pk, "Underline primary keys");
                        ui.checkbox(&mut r.bold_pk, "Use bold font for primary keys");
                        ui.end_row();
                    });
                    ui.add_space(120.0);
                }
                1 => {
                    ui.horizontal_top(|ui| {
                        // The list of columns.
                        egui::Frame::new().fill(pal(ui.ctx()).field).stroke(Stroke::new(1.0, pal(ui.ctx()).field_border)).inner_margin(2).show(ui, |ui| {
                            ui.set_width(270.0);
                            ui.set_height(120.0);
                            egui::ScrollArea::vertical().max_height(120.0).auto_shrink([false, false]).show(ui, |ui| {
                                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    for (i, c) in r.cols.iter().enumerate() {
                                        let mut label = if c.name.is_empty() { "(no name)".to_string() } else { c.name.clone() };
                                        if !c.ty.is_empty() {
                                            label += &format!("   {}", c.ty);
                                        }
                                        if c.pk {
                                            label += "   (PK)";
                                        }
                                        if ui.selectable_label(*row == Some(i), label).clicked() {
                                            *row = Some(i);
                                        }
                                    }
                                });
                            });
                        });
                        ui.vertical(|ui| {
                            let w = vec2(84.0, 0.0);
                            if ui.add(egui::Button::new("New").min_size(w)).clicked() {
                                let n = r.cols.len() + 1;
                                let at = row.map(|i| i + 1).unwrap_or(r.cols.len()).min(r.cols.len());
                                r.cols.insert(at, crate::record::Column { name: format!("column{n}"), ty: "VARCHAR(255)".into(), nullable: true, ..Default::default() });
                                *row = Some(at);
                            }
                            if ui.add_enabled(row.is_some(), egui::Button::new("Delete").min_size(w)).clicked() {
                                if let Some(i) = *row {
                                    r.cols.remove(i);
                                    *row = if r.cols.is_empty() { None } else { Some(i.min(r.cols.len() - 1)) };
                                }
                            }
                            if ui.add_enabled(matches!(*row, Some(i) if i > 0), egui::Button::new("Move up").min_size(w)).clicked() {
                                if let Some(i) = *row {
                                    r.cols.swap(i, i - 1);
                                    *row = Some(i - 1);
                                }
                            }
                            if ui.add_enabled(matches!(*row, Some(i) if i + 1 < r.cols.len()), egui::Button::new("Move down").min_size(w)).clicked() {
                                if let Some(i) = *row {
                                    r.cols.swap(i, i + 1);
                                    *row = Some(i + 1);
                                }
                            }
                        });
                    });
                    ui.add_space(8.0);
                    // The selected column's details.
                    let sel = row.filter(|i| *i < r.cols.len());
                    let mut empty = crate::record::Column { nullable: true, ..Default::default() };
                    let c = match sel {
                        Some(i) => &mut r.cols[i],
                        None => &mut empty,
                    };
                    ui.group(|ui| {
                        ui.set_min_width(370.0);
                        ui.label(egui::RichText::new("Attribute data").strong());
                        ui.add_enabled_ui(sel.is_some(), |ui| {
                            egui::Grid::new("col-grid").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                                ui.label("Name:");
                                ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(270.0));
                                ui.end_row();
                                ui.label("Type:");
                                ui.add(egui::TextEdit::singleline(&mut c.ty).desired_width(270.0).hint_text("e.g. INTEGER, VARCHAR(100), DATE"));
                                ui.end_row();
                                ui.label("Comment:");
                                ui.add(egui::TextEdit::multiline(&mut c.comment).desired_rows(2).desired_width(270.0));
                                ui.end_row();
                                ui.label("Default:");
                                ui.add(egui::TextEdit::singleline(&mut c.default).desired_width(270.0));
                                ui.end_row();
                            });
                            ui.add_space(4.0);
                            egui::Grid::new("col-flags").num_columns(2).spacing(vec2(60.0, 4.0)).show(ui, |ui| {
                                if ui.checkbox(&mut c.pk, "Primary key").changed() && c.pk {
                                    c.nullable = false;
                                }
                                ui.add_enabled(!c.pk, egui::Checkbox::new(&mut c.nullable, "Nullable"));
                                ui.end_row();
                                ui.label("");
                                ui.checkbox(&mut c.unique, "Unique");
                                ui.end_row();
                            });
                        });
                    });
                }
                _ => {
                    egui::Grid::new("table-style").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                        color_row(ui, "Line colour", &mut e.line_color);
                        color_row(ui, "Fill colour", &mut e.fill_color);
                        num_row(ui, "Line width (cm)", &mut e.line_width, 0.0, 5.0, 0.01);
                        if let Some(t) = &mut e.text {
                            color_row(ui, "Text colour", &mut t.color);
                            ui.label("Font");
                            let fams = ["Sans (Arial)", "Serif (Times)", "Monospace (Courier)"];
                            let mut fi = crate::fonts::family_index(&t.family);
                            let before = fi;
                            egui::ComboBox::from_id_salt("table-font").selected_text(fams[fi]).show_ui(ui, |ui| {
                                for (i, f) in fams.iter().enumerate() {
                                    ui.selectable_value(&mut fi, i, *f);
                                }
                            });
                            if fi != before {
                                t.family = ["sans", "serif", "monospace"][fi].into();
                            }
                            ui.end_row();
                            num_row(ui, "Text height (cm)", &mut t.height, 0.1, 20.0, 0.05);
                        }
                    });
                    ui.add_space(100.0);
                }
            }
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("OK").min_size(vec2(76.0, 0.0))).clicked() {
                    ok = true;
                }
                if ui.add(egui::Button::new("Apply").min_size(vec2(76.0, 0.0))).clicked() {
                    apply = true;
                }
                if ui.add(egui::Button::new("Close").min_size(vec2(76.0, 0.0))).clicked() {
                    close = true;
                }
            });
        });
        if ok || apply {
            let draft = self.props.as_ref().unwrap().draft.clone();
            self.ed.apply_to_selection(|o| copy_style(&draft, o));
            // Keep editing the updated table after Apply.
            if apply {
                if let Some(o) = self.ed.d.get(draft.id) {
                    if let Some(d) = self.props.as_mut() {
                        d.draft = o.clone();
                    }
                }
            }
        }
        if ok || close || !open {
            self.props = None;
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.mysql.is_some() {
            self.mysql_window(ctx);
        }
        match &self.props {
            Some(d) if is_table(&d.draft) => self.table_window(ctx),
            Some(d) if is_class(&d.draft) => self.class_window(ctx),
            Some(_) => self.props_window(ctx),
            None => {}
        }
        if self.page.is_some() {
            self.page_window(ctx);
        }
        if self.pdf_open {
            let (mut open, mut go, mut cancel) = (true, false, false);
            let (pw, ph) = self.ed.d.paper.size();
            let fit_label = format!(
                "Fit onto one {} {} page ({:.1} × {:.1} cm)",
                self.ed.d.paper.name,
                if self.ed.d.paper.portrait { "portrait" } else { "landscape" },
                pw,
                ph
            );
            let mode = &mut self.pdf_mode;
            window("Export to PDF").open(&mut open).show(ctx, |ui| {
                ui.label(egui::RichText::new("Page size").strong());
                ui.radio_value(mode, PageMode::DiagramSize, "Page exactly the size of the drawing");
                ui.radio_value(mode, PageMode::FitToPaper, fit_label);
                ui.label(egui::RichText::new("Paper size and margins come from File → Page setup.").color(pal(ui.ctx()).hint));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Export…").clicked() {
                        go = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
            if !open || cancel {
                self.pdf_open = false;
            }
            if go {
                self.pdf_open = false;
                self.export_pdf();
            }
        }
        if self.mcp_help_open {
            let mut open = true;
            let exe = std::env::current_exe().map(|p| p.to_string_lossy().trim_start_matches(r"\\?\").to_string()).unwrap_or_else(|_| "ChartCraft.exe".into());
            let cmd = format!("claude mcp add --scope user chartcraft -- \"{exe}\" --mcp");
            let json = format!("{{\n  \"mcpServers\": {{\n    \"chartcraft\": {{\n      \"command\": \"{}\",\n      \"args\": [\"--mcp\"]\n    }}\n  }}\n}}", exe.replace('\\', "\\\\"));
            window("Use with Claude Code (MCP)").open(&mut open).show(ctx, |ui| {
                ui.set_max_width(560.0);
                ui.label("Claude Code can build diagrams for you with this app. Run this once in a terminal (Command Prompt or PowerShell):");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut cmd.clone()).desired_width(470.0).font(egui::TextStyle::Monospace));
                    if ui.button("Copy").clicked() {
                        ui.ctx().copy_text(cmd.clone());
                    }
                });
                ui.add_space(8.0);
                ui.label("Then start Claude Code and ask, for example:");
                ui.label(egui::RichText::new("“Draw a flowchart of our order process in C:\\Diagrams\\orders.dia and export it to PDF.”").italics());
                ui.label(egui::RichText::new("“Make a UML class diagram of the classes in this project in classes.dia.”").italics());
                ui.add_space(8.0);
                ui.label("Open the same .dia file here to watch it being built: the app shows changes as Claude saves them.");
                ui.add_space(8.0);
                ui.collapsing("For VS Code or other tools: the .mcp.json settings", |ui| {
                    ui.add(egui::TextEdit::multiline(&mut json.clone()).desired_rows(8).desired_width(520.0).font(egui::TextStyle::Monospace));
                    if ui.button("Copy").clicked() {
                        ui.ctx().copy_text(json.clone());
                    }
                });
            });
            self.mcp_help_open = open;
        }
        if self.tips_open {
            let mut open = true;
            window("Tips").open(&mut open).show(ctx, |ui| {
                ui.set_max_width(460.0);
                for t in [
                    "Pick a shape on the left, then click or drag on the diagram. Start typing to label it.",
                    "Double-click a shape to change its text. Double-click a line to change its arrows and colour.",
                    "Draw a line or zigzag from one shape to another: its ends stick to the shapes (red handles) and follow them when you move them.",
                    "Double-click a tool to keep using it; Esc goes back to the arrow.",
                    "Shift+click, or drag a box around objects, to select several. Arrow keys nudge them.",
                    "Ctrl + mouse wheel zooms; the middle mouse button drags the view.",
                    "File → Export to PDF makes a PDF; File → Save keeps the diagram as a .dia file.",
                ] {
                    ui.label(format!("•  {t}"));
                }
            });
            self.tips_open = open;
        }
        if self.about_open {
            let mut open = true;
            window("About ChartCraft").open(&mut open).show(ctx, |ui| {
                ui.label("ChartCraft");
                ui.label("Draw flowcharts and diagrams, open and save .dia files, and export to PDF, PNG or SVG.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Licensed under MIT or Apache 2.0. Toolbar icons: Phosphor Icons (phosphoricons.com), MIT licence.").color(pal(ui.ctx()).hint).small());
            });
            self.about_open = open;
        }
    }

    fn props_window(&mut self, ctx: &egui::Context) {
        let (mut open, mut ok, mut cancel) = (true, false, false);
        let several = self.ed.sel.len() > 1;
        let Some(dlg) = self.props.as_mut() else { return };
        let title = match &dlg.draft.kind {
            Kind::Element(e) => format!("{} properties", e.shape().name),
            Kind::Line(_) => "Line properties".into(),
            Kind::Text(_) => "Text properties".into(),
            Kind::Polygon(_) => "Polygon properties".into(),
            Kind::Image(_) => "Picture properties".into(),
            Kind::Raw(_) => "Properties".into(),
        };
        window(title).id(egui::Id::unique("props")).open(&mut open).show(ctx, |ui| {
            egui::Grid::new("props-grid").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| match &mut dlg.draft.kind {
                Kind::Element(e) => {
                    color_row(ui, "Line colour", &mut e.line_color);
                    color_row(ui, "Fill colour", &mut e.fill_color);
                    ui.label("Fill");
                    ui.checkbox(&mut e.filled, "Draw background");
                    ui.end_row();
                    num_row(ui, "Line width (cm)", &mut e.line_width, 0.0, 5.0, 0.01);
                    style_row(ui, &mut e.style);
                    num_row(ui, "Dash length (cm)", &mut e.dash, 0.05, 10.0, 0.05);
                    if e.shape().fam == shapes::Family::StdBox {
                        num_row(ui, "Corner radius (cm)", &mut e.corner_radius, 0.0, 50.0, 0.05);
                    }
                    if e.shape().key == "fc-parallelogram" {
                        num_row(ui, "Slant angle (°)", &mut e.shear_angle, 20.0, 160.0, 1.0);
                    }
                    let mut pad = e.padding;
                    if let Some(t) = &mut e.text {
                        num_row(ui, "Text padding (cm)", &mut pad, 0.0, 10.0, 0.05);
                        text_rows(ui, t, false);
                    }
                    e.padding = pad;
                }
                Kind::Text(t) => text_rows(ui, &mut t.text, true),
                Kind::Line(l) => {
                    color_row(ui, "Colour", &mut l.color);
                    num_row(ui, "Width (cm)", &mut l.width, 0.01, 5.0, 0.01);
                    style_row(ui, &mut l.style);
                    num_row(ui, "Dash length (cm)", &mut l.dash, 0.05, 10.0, 0.05);
                    arrow_row(ui, "Start arrow", &mut l.start_arrow.kind);
                    arrow_row(ui, "End arrow", &mut l.end_arrow.kind);
                    let mut len = if l.end_arrow.kind != 0 { l.end_arrow.length } else { l.start_arrow.length };
                    let mut wid = if l.end_arrow.kind != 0 { l.end_arrow.width } else { l.start_arrow.width };
                    num_row(ui, "Arrow length (cm)", &mut len, 0.05, 10.0, 0.05);
                    num_row(ui, "Arrow width (cm)", &mut wid, 0.05, 10.0, 0.05);
                    l.start_arrow.length = len;
                    l.end_arrow.length = len;
                    l.start_arrow.width = wid;
                    l.end_arrow.width = wid;
                    if l.kind == LineKind::Arc {
                        num_row(ui, "Bulge (cm)", &mut l.curve, -100.0, 100.0, 0.05);
                    }
                }
                Kind::Polygon(pg) => {
                    color_row(ui, "Line colour", &mut pg.line_color);
                    color_row(ui, "Fill colour", &mut pg.fill_color);
                    ui.label("Fill");
                    ui.checkbox(&mut pg.filled, "Draw background");
                    ui.end_row();
                    num_row(ui, "Line width (cm)", &mut pg.line_width, 0.0, 5.0, 0.01);
                    style_row(ui, &mut pg.style);
                }
                Kind::Image(im) => {
                    ui.label("Shape");
                    ui.checkbox(&mut im.keep_aspect, "Keep proportions");
                    ui.end_row();
                    ui.label("Border");
                    ui.checkbox(&mut im.draw_border, "Draw a border");
                    ui.end_row();
                    color_row(ui, "Border colour", &mut im.line_color);
                    num_row(ui, "Border width (cm)", &mut im.line_width, 0.01, 5.0, 0.01);
                }
                Kind::Raw(rw) => {
                    ui.label(format!("This “{}” object came from another program; it can be moved and is kept when saving.", rw.type_name));
                    ui.end_row();
                }
            });
            if several {
                ui.label(egui::RichText::new("The settings apply to every selected object of this kind.").color(pal(ui.ctx()).hint));
            }
            ok_cancel(ui, &mut ok, &mut cancel);
        });
        if ok {
            let draft = self.props.take().unwrap().draft;
            self.ed.apply_to_selection(|o| copy_style(&draft, o));
        } else if cancel || !open {
            self.props = None;
        }
    }

    fn page_window(&mut self, ctx: &egui::Context) {
        let (mut open, mut ok, mut cancel) = (true, false, false);
        let Some(dlg) = self.page.as_mut() else { return };
        window("Page setup and diagram properties").open(&mut open).show(ctx, |ui| {
            egui::Grid::new("page-grid").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                ui.label("Paper size");
                egui::ComboBox::from_id_salt("paper").selected_text(dlg.paper.name.clone()).show_ui(ui, |ui| {
                    for (name, w, h) in PAPER_SIZES {
                        ui.selectable_value(&mut dlg.paper.name, name.to_string(), format!("{name} ({w} × {h} cm)"));
                    }
                });
                ui.end_row();
                ui.label("Orientation");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut dlg.paper.portrait, true, "Portrait");
                    ui.radio_value(&mut dlg.paper.portrait, false, "Landscape");
                });
                ui.end_row();
                num_row(ui, "Top margin (cm)", &mut dlg.paper.tm, 0.0, 20.0, 0.1);
                num_row(ui, "Bottom margin (cm)", &mut dlg.paper.bm, 0.0, 20.0, 0.1);
                num_row(ui, "Left margin (cm)", &mut dlg.paper.lm, 0.0, 20.0, 0.1);
                num_row(ui, "Right margin (cm)", &mut dlg.paper.rm, 0.0, 20.0, 0.1);
                let mut pct = dlg.paper.scaling * 100.0;
                num_row(ui, "Scaling (%)", &mut pct, 5.0, 1000.0, 1.0);
                dlg.paper.scaling = pct / 100.0;
                color_row(ui, "Background", &mut dlg.background);
                num_row(ui, "Grid spacing (cm)", &mut dlg.grid, 0.1, 20.0, 0.1);
            });
            ok_cancel(ui, &mut ok, &mut cancel);
        });
        if ok {
            let dlg = self.page.take().unwrap();
            self.ed.checkpoint();
            self.ed.d.paper = dlg.paper;
            self.ed.d.background = dlg.background;
            self.ed.d.grid = dlg.grid;
            self.ed.notify_changed();
        } else if cancel || !open {
            self.page = None;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut act: Option<Action> = None;

        // Screenshot mode: draw ~120 frames, save the window's picture and quit.
        if let Some(path) = self.shot.clone() {
            self.frames += 1;
            ctx.request_repaint();
            if self.frames >= 120 && !self.shot_sent {
                self.shot_sent = true;
                ctx.request_screenshot(move |img| {
                    let [w, h] = img.size;
                    let mut rgba = Vec::with_capacity(w * h * 4);
                    for px in &img.pixels {
                        rgba.extend_from_slice(&px.to_srgba_unmultiplied());
                    }
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    if let Err(e) = image::save_buffer(&path, &rgba, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                        eprintln!("Couldn't save the screenshot: {e}");
                    }
                    std::process::exit(0);
                });
            }
        }

        // Closing with unsaved changes asks first.
        let any_modified = self.ed.modified || self.tabs.iter().flatten().any(|d| d.ed.modified);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && any_modified && self.shot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.confirm_all() {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        let name = self.file.as_ref().and_then(|f| f.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| format!("Diagram{}.dia", self.untitled));
        let title = format!("{}{} — ChartCraft", if self.ed.modified { "*" } else { "" }, name);
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        self.watch_file(&ctx);
        self.keyboard(&ctx, &mut act);

        let pal = pal(&ctx);
        let bar_fill = pal.panel;
        egui::Panel::top("menu").frame(egui::Frame::new().fill(pal.bar).inner_margin(egui::Margin::symmetric(4, 2))).show(ui, |ui| self.menu_bar(ui, &mut act));
        egui::Panel::top("toolbar").frame(egui::Frame::new().fill(pal.bar).inner_margin(egui::Margin::symmetric(6, 3))).show(ui, |ui| self.toolbar(ui, &mut act));
        egui::Panel::bottom("status").frame(egui::Frame::new().fill(bar_fill).inner_margin(egui::Margin::symmetric(8, 3))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let st = self.status_line();
                ui.label(st);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}%", self.ed.zoom * 100.0));
                    ui.separator();
                    if let Some(w) = self.mouse_world {
                        ui.label(format!("{:.2}, {:.2} cm", w.x, w.y));
                    }
                });
            });
        });
        egui::Panel::left("toolbox")
            .resizable(false)
            .exact_size(160.0)
            .frame(egui::Frame::new().fill(bar_fill).inner_margin(egui::Margin { left: 4, right: 4, top: 4, bottom: 0 }))
            .show(ui, |ui| self.toolbox(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas(ui, &mut act));

        self.dialogs(&ctx);

        if let Some(a) = act {
            if !matches!(a, Action::Zoom(_) | Action::ZoomIn | Action::ZoomOut | Action::BestFit) {
                self.status.clear();
            }
            self.run(a, &ctx);
        }
    }
}
