//! The window: menus, toolbar, toolbox, drawing area, status bar and dialogs.

use crate::editor::{Editor, Mods, Tool, BASE_SCALE, H_POINT};
use crate::geom::{self, p, P, R};
use crate::icons::{paint_icon, paint_tool, Icon};
use crate::model::*;
use crate::pdf::PageMode;
use crate::shapes;
use crate::skia::{self, SkiaPainter};
use crate::theme::{pal, ThemeChoice};
use egui::{pos2, vec2, Align2, Color32, CursorIcon, Event, FontId, Key, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui};
use std::path::PathBuf;

#[path = "app_canvas.rs"]
mod app_canvas;
#[path = "app_dialogs.rs"]
mod app_dialogs;
#[path = "app_mysql.rs"]
mod app_mysql;
#[path = "app_ui.rs"]
mod app_ui;
#[cfg(target_os = "macos")]
#[path = "app_macos.rs"]
mod app_macos;
#[cfg(target_os = "macos")]
pub(crate) use app_macos::install_open_handler;
#[cfg(target_os = "macos")]
#[path = "app_macos_menu.rs"]
mod app_macos_menu;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    New,
    Open,
    Save,
    SaveAs,
    ExportPdf,
    ExportPicture,
    PageSetup,
    Print,
    Exit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    SelectAll,
    SelectNone,
    ZoomIn,
    ZoomOut,
    Zoom(f64),
    BestFit,
    Props,
    EditText,
    Reorder(u8),
    Align(u8),
    Example,
    Tips,
    McpHelp,
    About,
    AddCorner,
    RemoveCorner,
    ShowAllLayers,
    OpenRecent(usize),
    ImportMysql,
    ClearRecent,
    SelectInvert,
    SelectSameType,
    SelectConnected,
    AddLayer,
    MoveToLayer,
    SwitchTab(usize),
    CloseTab(usize),
    NextTab,
}

/// A diagram open in a tab that isn't showing right now.
struct Doc {
    ed: Editor,
    file: Option<PathBuf>,
    untitled: u32,
}

struct PropsDialog {
    draft: Obj,
    /// Table editor: which tab and which column are showing.
    tab: u8,
    row: Option<usize>,
    /// Class editor: the selected parameter of the selected operation.
    prow: Option<usize>,
}

struct PageDialog {
    paper: Paper,
    background: Col,
    grid: f64,
}

pub struct App {
    ed: Editor,
    file: Option<PathBuf>,
    untitled: u32,
    /// One entry per tab; the showing tab's entry is empty (its diagram is in `ed`/`file`).
    tabs: Vec<Option<Doc>>,
    cur: usize,
    next_untitled: u32,
    /// Recently opened or saved diagrams, newest first.
    recent: Vec<PathBuf>,
    /// The MySQL import window, when open.
    mysql: Option<app_mysql::MysqlDialog>,
    tex: Option<egui::TextureHandle>,
    tex_key: Option<(u64, u64, u64, u64, [usize; 2], u8, Option<u64>)>,
    /// Toolbox shape pictures, and whether they were made for the dark theme.
    icons: std::collections::HashMap<usize, egui::TextureHandle>,
    icons_dark: bool,
    theme: ThemeChoice,
    sheet: usize,
    status: String,
    props: Option<PropsDialog>,
    page: Option<PageDialog>,
    pdf_open: bool,
    pdf_mode: PageMode,
    shot: Option<PathBuf>,
    frames: u32,
    shot_sent: bool,
    allow_close: bool,
    canvas_drag: bool,
    pan: Option<(Pos2, P)>,
    mouse_world: Option<P>,
    compress: bool,
    title: String,
    coalesce: Option<(u32, u64)>,
    tips_open: bool,
    mcp_help_open: bool,
    /// The open file's last-seen change time, to pick up edits made by other programs.
    watch: Option<(PathBuf, std::time::SystemTime)>,
    last_watch: f64,
    about_open: bool,
    /// The drawing area on screen, from the last frame (used for view maths).
    pub(crate) last_area: Rect,
    scroll_drag: Option<(bool, f32, f64)>,
    /// macOS: the menus in the system menu bar (made on the first frame).
    #[cfg(target_os = "macos")]
    native: Option<app_macos_menu::NativeMenu>,
    /// macOS: whether the close button shows the unsaved-changes dot.
    #[cfg(target_os = "macos")]
    edited_shown: Option<bool>,
    /// macOS: a Control-click is being passed on as a right-click.
    #[cfg(target_os = "macos")]
    control_click: bool,
}

const ZOOMS: [f64; 9] = [0.25, 0.5, 0.707, 0.85, 1.0, 1.41, 2.0, 2.83, 4.0];

fn c32(c: Col) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0, c.1, c.2, c.3)
}
fn col(c: Color32) -> Col {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Col(r, g, b, a)
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Vec<PathBuf>, shot: Option<PathBuf>) -> Self {
        // --theme for checking the build; otherwise the saved choice (light, as before, if none).
        let theme = std::env::var("CHARTCRAFT_THEME").ok().and_then(|t| ThemeChoice::parse(&t)).or_else(load_theme).unwrap_or(ThemeChoice::Light);
        cc.egui_ctx.set_theme(theme.preference());
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        crate::theme::setup_styles(&cc.egui_ctx);
        crate::icons::install_font(&cc.egui_ctx);
        #[cfg(target_os = "macos")]
        app_macos::set_repaint(&cc.egui_ctx);
        let mut app = App {
            ed: Editor::new(),
            file: None,
            untitled: 1,
            tabs: vec![None],
            cur: 0,
            next_untitled: 2,
            recent: if shot.is_some() { vec![] } else { load_recent() },
            mysql: None,
            tex: None,
            tex_key: None,
            icons: Default::default(),
            icons_dark: false,
            theme,
            sheet: 0,
            status: String::new(),
            props: None,
            page: None,
            pdf_open: false,
            pdf_mode: PageMode::DiagramSize,
            shot,
            frames: 0,
            shot_sent: false,
            allow_close: false,
            canvas_drag: false,
            pan: None,
            mouse_world: None,
            compress: false,
            title: String::new(),
            coalesce: None,
            tips_open: false,
            mcp_help_open: false,
            watch: None,
            last_watch: 0.0,
            about_open: false,
            last_area: Rect::from_min_size(pos2(220.0, 80.0), vec2(900.0, 600.0)),
            scroll_drag: None,
            #[cfg(target_os = "macos")]
            native: None,
            #[cfg(target_os = "macos")]
            edited_shown: None,
            #[cfg(target_os = "macos")]
            control_click: false,
        };
        // Optional: start with a given shape sheet showing (e.g. --sheet UML).
        if let Ok(name) = std::env::var("CHARTCRAFT_SHEET") {
            if let Some(i) = shapes::SHEETS.iter().position(|s| s.eq_ignore_ascii_case(&name)) {
                app.sheet = i;
            }
        }
        for path in open {
            app.open_path(&path);
        }
        // Optional: start with the Claude Code help window open (--help-mcp).
        app.mcp_help_open = std::env::var("CHARTCRAFT_MCPHELP").is_ok();
        // Optional: start with the MySQL import window open (--mysql).
        if std::env::var("CHARTCRAFT_MYSQL").is_ok() {
            app.mysql = Some(app_mysql::MysqlDialog::new(!app.ed.d.objects.is_empty()));
        }
        // Optional: open the first object's properties on a given tab (e.g. --props 1).
        if let Ok(tab) = std::env::var("CHARTCRAFT_PROPS") {
            if let Some(id) = app.ed.d.objects.first().map(|o| o.id) {
                app.ed.sel = vec![id];
                app.open_props();
                if let Some(d) = app.props.as_mut() {
                    d.tab = tab.parse().unwrap_or(0);
                }
            }
        }
        app
    }

    /// Put a file at the top of the recent list.
    fn remember(&mut self, path: &std::path::Path) {
        if self.shot.is_some() {
            return;
        }
        let path = std::fs::canonicalize(path).map(|p| PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\").to_string())).unwrap_or_else(|_| path.to_path_buf());
        self.recent.retain(|p| !same_file(p, &path));
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
        save_recent(&self.recent);
    }

    fn base_name(&self) -> String {
        match &self.file {
            Some(f) => f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            None => format!("Diagram{}", self.untitled),
        }
    }

    // ------------------------------------------------------------ files

    fn confirm_discard(&mut self) -> bool {
        if !self.ed.modified || self.shot.is_some() {
            return true;
        }
        #[cfg(not(target_os = "macos"))]
        let r = rfd::MessageDialog::new()
            .set_title("ChartCraft")
            .set_description("Save the changes to this diagram first?")
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show();
        // The Mac wording and buttons: Save / Don't Save / Cancel.
        #[cfg(target_os = "macos")]
        let r = rfd::MessageDialog::new()
            .set_title(format!("Do you want to save the changes you made to “{}”?", self.base_name()))
            .set_description("Your changes will be lost if you don’t save them.")
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::YesNoCancelCustom("Save".into(), "Don’t Save".into(), "Cancel".into()))
            .show();
        match r {
            rfd::MessageDialogResult::Yes => self.save(false),
            rfd::MessageDialogResult::No => true,
            rfd::MessageDialogResult::Custom(b) if b == "Save" => self.save(false),
            rfd::MessageDialogResult::Custom(b) if b == "Don’t Save" => true,
            _ => false,
        }
    }

    fn open_path(&mut self, path: &std::path::Path) {
        // Already open? Just show that tab.
        let same = |f: &Option<PathBuf>| f.as_ref().map(|f| f == path).unwrap_or(false);
        if same(&self.file) {
            return;
        }
        if let Some(i) = self.tabs.iter().position(|t| t.as_ref().map(|d| same(&d.file)).unwrap_or(false)) {
            self.switch_tab(i);
            return;
        }
        match crate::dia::load(path) {
            Ok(d) => {
                let unknown = d.objects.iter().filter(|o| matches!(&o.kind, Kind::Raw(_)) || matches!(&o.kind, Kind::Element(e) if is_unknown(e))).count();
                if self.is_blank() {
                    self.ed.load(d);
                    self.file = Some(path.to_path_buf());
                    self.tex_key = None;
                } else {
                    self.new_tab(d, Some(path.to_path_buf()));
                }
                self.remember(path);
                self.status = if unknown > 0 {
                    format!("Opened. {unknown} object(s) of kinds this app doesn't draw are shown as outlines and kept when saving.")
                } else {
                    format!("Opened {}.", path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())
                };
            }
            Err(e) => {
                if self.shot.is_none() {
                    rfd::MessageDialog::new().set_title("Open diagram").set_description(format!("Couldn't open the file:\n{e}")).set_level(rfd::MessageLevel::Warning).show();
                }
            }
        }
    }

    // ------------------------------------------------------------ tabs

    /// An untouched new diagram (opening a file can reuse its tab).
    fn is_blank(&self) -> bool {
        self.file.is_none() && !self.ed.modified && self.ed.d.objects.is_empty() && !self.ed.can_undo()
    }

    fn tab_name(&self, i: usize) -> (String, bool) {
        let (file, untitled, modified) = if i == self.cur {
            (&self.file, self.untitled, self.ed.modified)
        } else {
            match &self.tabs[i] {
                Some(d) => (&d.file, d.untitled, d.ed.modified),
                None => return (String::new(), false),
            }
        };
        let name = file.as_ref().and_then(|f| f.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| format!("Diagram{untitled}.dia"));
        (name, modified)
    }

    fn reset_view_state(&mut self) {
        self.props = None;
        self.page = None;
        self.canvas_drag = false;
        self.pan = None;
        self.scroll_drag = None;
        self.coalesce = None;
        self.tex_key = None;
        self.status.clear();
    }

    fn switch_tab(&mut self, i: usize) {
        if i == self.cur || i >= self.tabs.len() || self.tabs[i].is_none() {
            return;
        }
        self.ed.end_edit(true);
        self.reset_view_state();
        let d = self.tabs[i].take().unwrap();
        let clip = std::mem::take(&mut self.ed.clipboard);
        let old = Doc {
            ed: std::mem::replace(&mut self.ed, d.ed),
            file: std::mem::replace(&mut self.file, d.file),
            untitled: std::mem::replace(&mut self.untitled, d.untitled),
        };
        self.tabs[self.cur] = Some(old);
        self.cur = i;
        // Copy and paste works between tabs.
        if !clip.is_empty() {
            self.ed.clipboard = clip;
        }
        self.ed.touch();
    }

    /// Open a diagram in a new tab and show it.
    fn new_tab(&mut self, d: Diagram, file: Option<PathBuf>) {
        let mut ed = Editor::new();
        ed.style = self.ed.style.clone();
        ed.show_grid = self.ed.show_grid;
        ed.snap_to_grid = self.ed.snap_to_grid;
        ed.object_snap = self.ed.object_snap;
        ed.show_cps = self.ed.show_cps;
        ed.show_pages = self.ed.show_pages;
        ed.show_rulers = self.ed.show_rulers;
        ed.load(d);
        let untitled = if file.is_none() {
            self.next_untitled += 1;
            self.next_untitled - 1
        } else {
            0
        };
        self.tabs.push(Some(Doc { ed, file, untitled }));
        self.switch_tab(self.tabs.len() - 1);
    }

    /// Close a tab, asking to save first if needed. Returns false if the person cancelled.
    fn close_tab(&mut self, i: usize) -> bool {
        self.switch_tab(i);
        if !self.confirm_discard() {
            return false;
        }
        self.reset_view_state();
        if self.tabs.len() == 1 {
            self.untitled = self.next_untitled;
            self.next_untitled += 1;
            self.file = None;
            let keep = std::mem::take(&mut self.ed.clipboard);
            self.ed.load(Diagram::default());
            self.ed.clipboard = keep;
            return true;
        }
        self.tabs.remove(self.cur);
        let j = self.cur.min(self.tabs.len() - 1);
        let d = self.tabs[j].take().unwrap();
        let clip = std::mem::take(&mut self.ed.clipboard);
        self.ed = d.ed;
        self.file = d.file;
        self.untitled = d.untitled;
        if !clip.is_empty() {
            self.ed.clipboard = clip;
        }
        self.cur = j;
        self.ed.touch();
        true
    }

    /// Before quitting: offer to save every changed diagram. Returns false if cancelled.
    fn confirm_all(&mut self) -> bool {
        for i in 0..self.tabs.len() {
            let modified = if i == self.cur { self.ed.modified } else { self.tabs[i].as_ref().map(|d| d.ed.modified).unwrap_or(false) };
            if modified {
                self.switch_tab(i);
                if !self.confirm_discard() {
                    return false;
                }
            }
        }
        true
    }

    fn save(&mut self, ask: bool) -> bool {
        self.ed.end_edit(true);
        let path = match (&self.file, ask) {
            (Some(f), false) => f.clone(),
            _ => {
                let name = format!("{}.dia", self.base_name());
                match rfd::FileDialog::new().set_title("Save diagram").add_filter("Diagram files", &["dia"]).set_file_name(name).save_file() {
                    Some(p) => {
                        if p.extension().is_none() {
                            p.with_extension("dia")
                        } else {
                            p
                        }
                    }
                    None => return false,
                }
            }
        };
        match crate::dia::save(&self.ed.d, &path, self.compress) {
            Ok(()) => {
                self.status = format!("Saved {}.", path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
                self.remember(&path);
                self.file = Some(path);
                self.ed.modified = false;
                self.note_saved();
                true
            }
            Err(e) => {
                rfd::MessageDialog::new().set_title("Save").set_description(format!("Couldn't save:\n{e}")).set_level(rfd::MessageLevel::Warning).show();
                false
            }
        }
    }

    fn export_pdf(&mut self) {
        let name = format!("{}.pdf", self.base_name());
        let Some(mut path) = rfd::FileDialog::new().set_title("Export to PDF").add_filter("PDF document", &["pdf"]).set_file_name(name).save_file() else { return };
        if path.extension().is_none() {
            path = path.with_extension("pdf");
        }
        match crate::pdf::export(&self.ed.d, &path, self.pdf_mode) {
            Ok(()) => self.status = format!("Exported {}.", path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()),
            Err(e) => {
                rfd::MessageDialog::new().set_title("Export").set_description(format!("Couldn't export:\n{e}")).show();
            }
        }
    }

    fn export_picture(&mut self) {
        let name = format!("{}.png", self.base_name());
        let Some(path) =
            rfd::FileDialog::new().set_title("Export picture").add_filter("PNG picture", &["png"]).add_filter("SVG drawing", &["svg"]).set_file_name(name).save_file()
        else {
            return;
        };
        let is_svg = path.extension().map(|e| e.eq_ignore_ascii_case("svg")).unwrap_or(false);
        let path = if path.extension().is_none() { path.with_extension("png") } else { path };
        let res = if is_svg { crate::svg::export(&self.ed.d, &path) } else { skia::export_png(&self.ed.d, &path, 40.0) };
        match res {
            Ok(()) => self.status = format!("Exported {}.", path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()),
            Err(e) => {
                rfd::MessageDialog::new().set_title("Export").set_description(format!("Couldn't export:\n{e}")).show();
            }
        }
    }

    /// Printing goes through a PDF fitted to the page, opened in the computer's PDF viewer.
    fn print(&mut self) {
        let path = std::env::temp_dir().join(format!("{} (print).pdf", self.base_name()));
        if let Err(e) = crate::pdf::export(&self.ed.d, &path, PageMode::FitToPaper) {
            self.status = format!("Couldn't prepare the print-out: {e}");
            return;
        }
        let ok = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd").args(["/C", "start", ""]).arg(&path).spawn().is_ok()
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("open").arg(&path).spawn().is_ok()
        } else {
            std::process::Command::new("xdg-open").arg(&path).spawn().is_ok()
        };
        self.status = if ok { "Opened the print-out in your PDF viewer; print it from there.".into() } else { format!("Saved the print-out to {}", path.display()) };
    }

    // ------------------------------------------------------------ actions

    fn run(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::New => self.new_tab(Diagram::default(), None),
            Action::Open => {
                if let Some(paths) = rfd::FileDialog::new()
                    .set_title("Open diagram")
                    .add_filter("Diagram files", &["dia"])
                    .add_filter("XML files", &["xml"])
                    .add_filter("All files", &["*"])
                    .pick_files()
                {
                    for path in paths {
                        self.open_path(&path);
                    }
                }
            }
            Action::SwitchTab(i) => self.switch_tab(i),
            Action::OpenRecent(i) => {
                if let Some(path) = self.recent.get(i).cloned() {
                    if path.exists() {
                        self.open_path(&path);
                    } else {
                        self.recent.remove(i);
                        save_recent(&self.recent);
                        self.status = format!("{} can't be found any more, so it was taken off the recent list.", path.display());
                    }
                }
            }
            Action::ImportMysql => self.mysql = Some(app_mysql::MysqlDialog::new(!self.ed.d.objects.is_empty())),
            Action::ClearRecent => {
                self.recent.clear();
                save_recent(&self.recent);
            }
            Action::CloseTab(i) => {
                let i = if i == usize::MAX { self.cur } else { i };
                self.close_tab(i);
            }
            Action::NextTab => {
                let n = self.tabs.len();
                self.switch_tab((self.cur + 1) % n);
            }
            Action::Save => {
                self.save(false);
            }
            Action::SaveAs => {
                self.save(true);
            }
            Action::ExportPdf => {
                self.ed.end_edit(true);
                self.pdf_open = true;
            }
            Action::ExportPicture => {
                self.ed.end_edit(true);
                self.export_picture();
            }
            Action::PageSetup => {
                self.page = Some(PageDialog { paper: self.ed.d.paper.clone(), background: self.ed.d.background, grid: self.ed.d.grid });
            }
            Action::Print => {
                self.ed.end_edit(true);
                self.print();
            }
            Action::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::Undo => self.ed.undo(),
            Action::Redo => self.ed.redo(),
            Action::Cut => {
                if let Some(x) = self.ed.cut() {
                    ctx.copy_text(x);
                }
            }
            Action::Copy => {
                if let Some(x) = self.ed.copy() {
                    ctx.copy_text(x);
                }
            }
            Action::Paste => self.ed.paste(None),
            Action::Duplicate => self.ed.duplicate(),
            Action::Delete => self.ed.delete_selection(),
            Action::SelectAll => self.ed.select_all(),
            Action::SelectNone => self.ed.select_none(),
            Action::ZoomIn => self.zoom_to(self.ed.zoom * 2f64.sqrt(), None),
            Action::ZoomOut => self.zoom_to(self.ed.zoom / 2f64.sqrt(), None),
            Action::Zoom(z) => self.zoom_to(z, None),
            Action::BestFit => self.best_fit(),
            Action::Props => self.open_props(),
            Action::EditText => {
                if let [id] = self.ed.sel[..] {
                    self.ed.begin_edit(id, None);
                }
            }
            Action::Reorder(h) => self.ed.reorder(h),
            Action::Align(h) => self.ed.align(h),
            Action::Example => {
                if self.is_blank() {
                    self.ed.load(crate::editor::example());
                    self.tex_key = None;
                } else {
                    self.new_tab(crate::editor::example(), None);
                }
                self.ed.modified = true;
                self.best_fit();
            }
            Action::Tips => self.tips_open = true,
            Action::McpHelp => self.mcp_help_open = true,
            Action::About => self.about_open = true,
            Action::AddCorner => self.ed.add_corner(),
            Action::RemoveCorner => self.ed.remove_corner(),
            Action::SelectInvert => self.ed.select_invert(),
            Action::SelectSameType => self.ed.select_same_type(),
            Action::SelectConnected => self.ed.select_connected(),
            Action::AddLayer => self.ed.add_layer(),
            Action::MoveToLayer => self.ed.move_selection_to_active_layer(),
            Action::ShowAllLayers => {
                for l in self.ed.d.layers.iter_mut() {
                    l.visible = true;
                }
                self.ed.notify_changed();
            }
        }
    }

    fn open_props(&mut self) {
        self.ed.end_edit(true);
        match self.ed.sel.first().and_then(|id| self.ed.d.get(*id)) {
            Some(o) => self.props = Some(PropsDialog { draft: o.clone(), tab: 0, row: if is_table(o) || is_class(o) { Some(0) } else { None }, prow: Some(0) }),
            None => self.run_page_setup(),
        }
    }

    fn run_page_setup(&mut self) {
        self.page = Some(PageDialog { paper: self.ed.d.paper.clone(), background: self.ed.d.background, grid: self.ed.d.grid });
    }

    // ------------------------------------------------------------ view

    fn area_rect(&self) -> Rect {
        self.last_area
    }

    fn zoom_to(&mut self, z: f64, at: Option<Pos2>) {
        let area = self.area_rect();
        let z = z.clamp(0.05, 25.0);
        let anchor = at.unwrap_or(area.center());
        let w = self.to_world(anchor);
        self.ed.zoom = z;
        let s = self.ed.scale();
        self.ed.origin = p(w.x - (anchor.x - area.min.x) as f64 / s, w.y - (anchor.y - area.min.y) as f64 / s);
        self.ed.end_edit(true);
        self.ed.touch();
    }

    fn best_fit(&mut self) {
        let area = self.area_rect();
        let ext = self.ed.d.extents().unwrap_or(geom::r(0.0, 0.0, self.ed.d.paper.page_w(), self.ed.d.paper.page_h())).inflate(1.0);
        let z = ((area.width() as f64) / (ext.w * BASE_SCALE)).min((area.height() as f64) / (ext.h * BASE_SCALE)).clamp(0.05, 25.0);
        self.ed.zoom = z;
        let s = self.ed.scale();
        self.ed.origin = p(ext.cx() - area.width() as f64 / s / 2.0, ext.cy() - area.height() as f64 / s / 2.0);
        self.ed.touch();
    }

    fn to_world(&self, pos: Pos2) -> P {
        let a = self.area_rect();
        let s = self.ed.scale();
        p(self.ed.origin.x + (pos.x - a.min.x) as f64 / s, self.ed.origin.y + (pos.y - a.min.y) as f64 / s)
    }

    fn to_screen(&self, w: P) -> Pos2 {
        let a = self.area_rect();
        let s = self.ed.scale();
        pos2(a.min.x + ((w.x - self.ed.origin.x) * s) as f32, a.min.y + ((w.y - self.ed.origin.y) * s) as f32)
    }

    fn visible_world(&self) -> R {
        let a = self.area_rect();
        let s = self.ed.scale();
        geom::r(self.ed.origin.x, self.ed.origin.y, a.width() as f64 / s, a.height() as f64 / s)
    }
}

/// A database table (it gets its own properties window).
fn is_table(o: &Obj) -> bool {
    matches!(&o.kind, Kind::Element(e) if e.rec.as_ref().map(|r| r.kind == crate::record::RecKind::Table).unwrap_or(false))
}

const MAX_RECENT: usize = 10;

fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}

/// Where the recent-files list is kept (in the person's own settings folder).
fn recent_file() -> Option<PathBuf> {
    Some(settings_dir()?.join("ChartCraft").join("recent.txt"))
}

/// macOS: ~/Library/Application Support, where Mac apps keep their settings.
#[cfg(target_os = "macos")]
fn settings_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let dir = home.join("Library").join("Application Support");
    // Earlier builds kept them in ~/.config/ChartCraft: copy them across the first time.
    let (old, new) = (home.join(".config").join("ChartCraft"), dir.join("ChartCraft"));
    if !new.exists() && old.is_dir() && std::fs::create_dir_all(&new).is_ok() {
        for f in ["recent.txt", "theme.txt"] {
            let _ = std::fs::copy(old.join(f), new.join(f));
        }
    }
    Some(dir)
}

#[cfg(not(target_os = "macos"))]
fn settings_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_CONFIG_HOME")).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

/// The theme picked in View → Theme, kept beside the recent-files list.
fn theme_file() -> Option<PathBuf> {
    Some(settings_dir()?.join("ChartCraft").join("theme.txt"))
}

fn load_theme() -> Option<ThemeChoice> {
    ThemeChoice::parse(&std::fs::read_to_string(theme_file()?).ok()?)
}

fn save_theme(t: ThemeChoice) {
    let Some(f) = theme_file() else { return };
    if let Some(dir) = f.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(f, t.key());
}

fn load_recent() -> Vec<PathBuf> {
    let Some(mut f) = recent_file() else { return vec![] };
    if !f.exists() {
        // The app used to be called Diagrammer: carry its list over until the first save.
        if let Some(old) = settings_dir().map(|d| d.join("ChartCraft").join("recent.txt")) {
            f = old;
        }
    }
    std::fs::read_to_string(f).map(|s| s.lines().filter(|l| !l.trim().is_empty()).take(MAX_RECENT).map(PathBuf::from).collect()).unwrap_or_default()
}

fn save_recent(list: &[PathBuf]) {
    let Some(f) = recent_file() else { return };
    if let Some(dir) = f.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text: Vec<String> = list.iter().map(|p| p.to_string_lossy().to_string()).collect();
    let _ = std::fs::write(f, text.join("\n"));
}

/// A UML class (it gets its own properties window).
fn is_class(o: &Obj) -> bool {
    matches!(&o.kind, Kind::Element(e) if e.rec.as_ref().map(|r| r.kind == crate::record::RecKind::Class).unwrap_or(false))
}

fn file_time(p: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

impl App {
    /// About once a second: if another program (such as Claude Code through the MCP server)
    /// changed the open diagram's file, show the new version. Unsaved work here is never replaced.
    fn watch_file(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.last_watch < 1.0 || self.shot.is_some() {
            if self.file.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(1000));
            }
            return;
        }
        self.last_watch = now;
        let Some(path) = self.file.clone() else {
            self.watch = None;
            return;
        };
        ctx.request_repaint_after(std::time::Duration::from_millis(1000));
        let Some(t) = file_time(&path) else { return };
        match &self.watch {
            Some((p, seen)) if *p == path => {
                if *seen == t {
                    return;
                }
            }
            _ => {
                // A different file is showing now: just start watching it.
                self.watch = Some((path, t));
                return;
            }
        }
        self.watch = Some((path.clone(), t));
        if self.ed.modified {
            self.status = "This file was changed by another program; your unsaved changes here are kept (save to keep yours, or reopen to see theirs).".into();
            return;
        }
        if let Ok(d) = crate::dia::load(&path) {
            self.ed.end_edit(true);
            self.ed.checkpoint();
            self.ed.d = d;
            self.ed.sel.clear();
            self.ed.d.update_connections();
            self.ed.touch();
            self.ed.modified = false;
            self.status = "Updated: the file was changed by another program (Undo goes back).".into();
        }
    }

    /// Note our own saves, so they don't count as outside changes.
    fn note_saved(&mut self) {
        if let Some(p) = &self.file {
            if let Some(t) = file_time(p) {
                self.watch = Some((p.clone(), t));
            }
        }
    }
}
