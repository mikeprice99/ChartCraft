//! The "Import tables from MySQL" window.

use super::*;
use crate::mysql_import::{self as mi, ColumnInfo, Conn, TableInfo};
use std::sync::mpsc::{channel, Receiver};

pub(super) enum Msg {
    Dbs(Result<Vec<String>, String>),
    Tables(Result<Vec<TableInfo>, String>),
    Cols(Result<Vec<(TableInfo, Vec<ColumnInfo>)>, String>),
}

pub(super) struct MysqlDialog {
    conns: Vec<Conn>,
    pick: Option<usize>,
    c: Conn,
    port_text: String,
    dbs: Vec<String>,
    db: String,
    tables: Vec<(TableInfo, bool)>,
    filter: String,
    fill: Col,
    into_new: bool,
    update_existing: bool,
    busy: Option<(String, Receiver<Msg>)>,
    message: String,
    failed: bool,
}

impl MysqlDialog {
    /// `has_diagram`: a diagram with something in it is open, so new tables go into it by default.
    pub(super) fn new(has_diagram: bool) -> Self {
        let conns = mi::saved_connections();
        MysqlDialog {
            conns,
            pick: None,
            c: Conn { host: "localhost".into(), port: 3306, ..Default::default() },
            port_text: "3306".into(),
            dbs: vec![],
            db: String::new(),
            tables: vec![],
            filter: String::new(),
            fill: Col::WHITE,
            into_new: !has_diagram,
            update_existing: true,
            busy: None,
            message: String::new(),
            failed: false,
        }
    }

    fn start(&mut self, what: &str, job: impl FnOnce() -> Msg + Send + 'static) {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(job());
        });
        self.busy = Some((what.to_string(), rx));
        self.message.clear();
        self.failed = false;
    }

    fn conn(&self) -> Conn {
        let mut c = self.c.clone();
        c.port = self.port_text.trim().parse().unwrap_or(3306);
        c
    }

    fn connect(&mut self) {
        let c = self.conn();
        self.dbs.clear();
        self.tables.clear();
        self.start("Connecting…", move || Msg::Dbs(mi::list_databases(&c)));
    }

    fn load_tables(&mut self) {
        let (c, db) = (self.conn(), self.db.clone());
        self.tables.clear();
        self.start("Reading the tables…", move || Msg::Tables(mi::list_tables(&c, &db)));
    }
}

impl App {
    pub(super) fn mysql_window(&mut self, ctx: &egui::Context) {
        let Some(dlg) = self.mysql.as_mut() else { return };
        // Collect anything the background work has finished.
        let mut place: Option<Vec<(TableInfo, Vec<ColumnInfo>)>> = None;
        if let Some((_, rx)) = &dlg.busy {
            match rx.try_recv() {
                Ok(msg) => {
                    dlg.busy = None;
                    match msg {
                        Msg::Dbs(Ok(list)) => {
                            dlg.message = format!("Connected: {} database(s).", list.len());
                            dlg.dbs = list;
                            if !dlg.c.database.is_empty() && dlg.dbs.contains(&dlg.c.database) {
                                dlg.db = dlg.c.database.clone();
                                dlg.load_tables();
                            }
                        }
                        Msg::Tables(Ok(list)) => {
                            dlg.message = format!("{} table(s) in {}.", list.len(), dlg.db);
                            dlg.tables = list.into_iter().map(|t| (t, false)).collect();
                        }
                        Msg::Cols(Ok(list)) => place = Some(list),
                        Msg::Dbs(Err(e)) | Msg::Tables(Err(e)) | Msg::Cols(Err(e)) => {
                            dlg.message = e;
                            dlg.failed = true;
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
                Err(_) => {
                    dlg.busy = None;
                    dlg.message = "The connection stopped unexpectedly.".into();
                    dlg.failed = true;
                }
            }
        }

        let (mut open, mut close, mut add) = (true, false, false);
        let busy = dlg.busy.is_some();
        egui::Window::new("Import tables from MySQL")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_min_width(470.0);
                ui.add_enabled_ui(!busy, |ui| {
                    egui::Grid::new("mysql-conn").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                        ui.label("Saved connection:");
                        let shown = dlg.pick.and_then(|i| dlg.conns.get(i)).map(|c| c.label.clone()).unwrap_or_else(|| {
                            if dlg.conns.is_empty() { "(none found; type the details below)".into() } else { "(choose one, or type the details below)".into() }
                        });
                        let mut chosen = None;
                        egui::ComboBox::from_id_salt("mysql-saved").width(320.0).selected_text(shown).show_ui(ui, |ui| {
                            for (i, c) in dlg.conns.iter().enumerate() {
                                if ui.selectable_label(dlg.pick == Some(i), &c.label).on_hover_text(format!("{}@{}:{}", c.user, c.host, c.port)).clicked() {
                                    chosen = Some(i);
                                }
                            }
                        });
                        if let Some(i) = chosen {
                            dlg.pick = Some(i);
                            dlg.c = dlg.conns[i].clone();
                            dlg.port_text = dlg.c.port.to_string();
                            dlg.dbs.clear();
                            dlg.tables.clear();
                            dlg.db.clear();
                        }
                        ui.end_row();
                        ui.label("Host:");
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut dlg.c.host).desired_width(220.0));
                            ui.label("Port:");
                            ui.add(egui::TextEdit::singleline(&mut dlg.port_text).desired_width(52.0));
                        });
                        ui.end_row();
                        ui.label("User:");
                        ui.add(egui::TextEdit::singleline(&mut dlg.c.user).desired_width(220.0));
                        ui.end_row();
                        ui.label("Password:");
                        ui.add(egui::TextEdit::singleline(&mut dlg.c.password).password(true).desired_width(220.0));
                        ui.end_row();
                    });
                    ui.add_space(4.0);
                    if ui.add_enabled(!dlg.c.host.trim().is_empty(), egui::Button::new("Connect")).clicked() {
                        dlg.connect();
                    }
                    ui.separator();
                    egui::Grid::new("mysql-db").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                        ui.label("Database:");
                        let mut chosen = None;
                        ui.add_enabled_ui(!dlg.dbs.is_empty(), |ui| {
                            let shown = if dlg.db.is_empty() { "(connect first, then choose)".to_string() } else { dlg.db.clone() };
                            egui::ComboBox::from_id_salt("mysql-dbs").width(320.0).selected_text(shown).show_ui(ui, |ui| {
                                for d in &dlg.dbs {
                                    if ui.selectable_label(dlg.db == *d, d).clicked() {
                                        chosen = Some(d.clone());
                                    }
                                }
                            });
                        });
                        if let Some(d) = chosen {
                            if d != dlg.db {
                                dlg.db = d;
                                dlg.load_tables();
                            }
                        }
                        ui.end_row();
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Tables:");
                        ui.add(egui::TextEdit::singleline(&mut dlg.filter).desired_width(160.0).hint_text("filter by name"));
                        let f = dlg.filter.to_lowercase();
                        if ui.button("Select all").clicked() {
                            for (t, on) in dlg.tables.iter_mut() {
                                if t.name.to_lowercase().contains(&f) {
                                    *on = true;
                                }
                            }
                        }
                        if ui.button("Select none").clicked() {
                            for (_, on) in dlg.tables.iter_mut() {
                                *on = false;
                            }
                        }
                    });
                    egui::Frame::new().fill(pal(ui.ctx()).field).stroke(Stroke::new(1.0, pal(ui.ctx()).field_border)).inner_margin(4).show(ui, |ui| {
                        ui.set_width(455.0);
                        ui.set_height(220.0);
                        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(220.0).show(ui, |ui| {
                            let f = dlg.filter.to_lowercase();
                            if dlg.tables.is_empty() {
                                ui.label(egui::RichText::new("Connect and choose a database to see its tables.").color(pal(ui.ctx()).hint));
                            }
                            for (t, on) in dlg.tables.iter_mut().filter(|(t, _)| t.name.to_lowercase().contains(&f)) {
                                ui.horizontal(|ui| {
                                    ui.checkbox(on, &t.name);
                                    if !t.comment.is_empty() {
                                        ui.label(egui::RichText::new(&t.comment).color(pal(ui.ctx()).hint).small());
                                    }
                                });
                            }
                        });
                    });
                    let ticked = dlg.tables.iter().filter(|(_, on)| *on).count();
                    ui.label(egui::RichText::new(format!("{ticked} of {} ticked", dlg.tables.len())).color(pal(ui.ctx()).hint));
                    ui.add_space(4.0);
                    egui::Grid::new("mysql-opts").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                        ui.label("Table colour:");
                        let mut c = c32(dlg.fill);
                        if egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::Opaque).changed() {
                            dlg.fill = col(c);
                        }
                        ui.end_row();
                        ui.label("Put them in:");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut dlg.into_new, false, "the current diagram");
                            ui.radio_value(&mut dlg.into_new, true, "a new diagram tab");
                        });
                        ui.end_row();
                        ui.label("");
                        ui.add_enabled(
                            !dlg.into_new,
                            egui::Checkbox::new(&mut dlg.update_existing, "Update tables already in the diagram (they keep their place and lines)"),
                        );
                        ui.end_row();
                    });
                });
                ui.add_space(4.0);
                if let Some((what, _)) = &dlg.busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(what);
                    });
                } else if !dlg.message.is_empty() {
                    let c = if dlg.failed { pal(ui.ctx()).danger } else { pal(ui.ctx()).good };
                    ui.label(egui::RichText::new(&dlg.message).color(c));
                }
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ticked = dlg.tables.iter().filter(|(_, on)| *on).count();
                    let label = if ticked == 1 { "Add 1 table".to_string() } else { format!("Add {ticked} tables") };
                    if ui.add_enabled(!busy && ticked > 0, egui::Button::new(label)).clicked() {
                        add = true;
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });

        if add {
            let chosen: Vec<TableInfo> = dlg.tables.iter().filter(|(_, on)| *on).map(|(t, _)| t.clone()).collect();
            let (c, db) = (dlg.conn(), dlg.db.clone());
            dlg.start("Reading the columns…", move || Msg::Cols(mi::table_columns(&c, &db, &chosen)));
        }
        if let Some(list) = place {
            let (fill, into_new, update, db) = (dlg.fill, dlg.into_new, dlg.update_existing, dlg.db.clone());
            self.place_tables(list, fill, into_new, update, &db);
            self.mysql = None;
            return;
        }
        if close || !open {
            self.mysql = None;
        }
    }

    /// Put imported tables into a diagram, laid out in columns.
    fn place_tables(&mut self, list: Vec<(TableInfo, Vec<ColumnInfo>)>, fill: Col, into_new: bool, update: bool, db: &str) {
        self.ed.end_edit(true);
        // Tables already in this diagram: refresh their columns where they are. Lines hooked
        // to a column stay on that column (matched by name).
        let mut updated = vec![];
        let mut fresh = vec![];
        for (t, cols) in &list {
            let existing = if !into_new && update {
                self.ed.d.objects.iter().find(|o| matches!(&o.kind, Kind::Element(e) if e.rec.as_ref().map(|r| r.kind == crate::record::RecKind::Table && r.name == t.name).unwrap_or(false))).map(|o| o.id)
            } else {
                None
            };
            match existing {
                Some(id) => {
                    let new_rec = mi::table_element(t, cols, fill).rec.unwrap();
                    self.ed.sel = vec![id];
                    self.ed.apply_to_selection(|o| {
                        if let Kind::Element(e) = &mut o.kind {
                            if let Some(r) = e.rec.as_mut() {
                                r.cols = new_rec.cols.clone();
                                r.comment = new_rec.comment.clone();
                            }
                        }
                    });
                    updated.push(id);
                }
                None => fresh.push(mi::table_element(t, cols, fill)),
            }
        }
        let mut elems = fresh;
        if into_new {
            if !self.is_blank() {
                self.new_tab(Diagram::default(), None);
            }
            self.ed.d.paper.portrait = false; // landscape, like dia_mysql
        }
        let (x0, y0) = match (into_new, self.ed.d.extents()) {
            (false, Some(e)) => (e.right() + 2.0, e.y.max(0.0)),
            _ => (0.0, 0.0),
        };
        mi::layout(&mut elems, x0, y0);
        let mut ids = vec![];
        if !elems.is_empty() {
            self.ed.checkpoint();
            ids = elems.into_iter().map(|e| self.ed.d.add(Kind::Element(e))).collect();
        }
        self.ed.sel = ids.iter().chain(updated.iter()).copied().collect();
        self.ed.notify_changed();
        if !ids.is_empty() {
            self.best_fit();
        }
        self.status = match (ids.len(), updated.len()) {
            (n, 0) => format!("Added {n} table(s) from {db}."),
            (0, u) => format!("Updated {u} table(s) from {db}."),
            (n, u) => format!("Added {n} and updated {u} table(s) from {db}."),
        };
    }
}
