//! macOS only: the menus live in the system menu bar at the top of the screen, as Mac apps
//! expect, instead of in a bar inside the window. The items and their order match the in-window
//! menu that Windows and Linux show (see `menu_bar` in app_ui.rs), with the Mac conventions:
//! About and Quit sit in the application menu, and there is a Window menu.
//!
//! Only the shortcuts that work everywhere (even while typing) are attached to menu items, since
//! macOS gives a menu's shortcut to the menu before the window sees the key. Undo, Copy, Paste
//! and the rest stay with the window's own keyboard handling, so text boxes keep them.

use super::app_ui::TOOLBOX_TOOLS;
use super::*;
use muda::accelerator::Accelerator;
use muda::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use std::collections::HashMap;
use std::sync::mpsc;

/// What a menu item does when chosen.
#[derive(Clone, Copy)]
enum Cmd {
    Act(Action),
    Toggle(Flag),
    Theme(ThemeChoice),
    Tool(Tool),
    ActiveLayer(usize),
    ShowLayer(usize),
}

/// The View-menu switches (and File → Save compressed).
#[derive(Clone, Copy)]
enum Flag {
    Compress,
    Grid,
    Snap,
    ObjectSnap,
    ConnectionPoints,
    PageBreaks,
    Rulers,
}

/// When an item can be chosen.
#[derive(Clone, Copy)]
enum Gate {
    Selection,
    OneSelected,
    TwoSelected,
    Undo,
    Redo,
    Paste,
    HiddenLayers,
}

/// When an item shows a tick.
#[derive(Clone, Copy)]
enum Tick {
    Flag(Flag),
    Theme(ThemeChoice),
    Tool(Tool),
}

pub(super) struct NativeMenu {
    _bar: Menu,
    rx: mpsc::Receiver<MenuId>,
    cmds: HashMap<MenuId, Cmd>,
    next_id: usize,
    gated: Vec<(MenuItem, Gate, Option<bool>)>,
    ticked: Vec<(CheckMenuItem, Tick, Option<bool>)>,
    recent: Submenu,
    recent_items: Vec<Box<dyn IsMenuItem>>,
    recent_key: Option<Vec<PathBuf>>,
    layers: Submenu,
    layer_items: Vec<Box<dyn IsMenuItem>>,
    layer_key: Option<Vec<(String, bool, bool)>>,
}

fn key(s: &str) -> Option<Accelerator> {
    s.parse().ok()
}

impl NativeMenu {
    /// Builds the menus and puts them in the system menu bar. Call on the main thread, once the
    /// application is running (the first frame).
    pub(super) fn install(ctx: &egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let _ = tx.send(e.id);
            repaint.request_repaint();
        }));

        let bar = Menu::new();
        let mut m = NativeMenu {
            _bar: Menu::new(),
            rx,
            cmds: HashMap::new(),
            next_id: 0,
            gated: Vec::new(),
            ticked: Vec::new(),
            recent: Submenu::new("Open Recent", false),
            recent_items: Vec::new(),
            recent_key: None,
            layers: Submenu::new("Layers", true),
            layer_items: Vec::new(),
            layer_key: None,
        };
        let sep = || PredefinedMenuItem::separator();

        // The application menu (shown in bold with the app's name).
        let app = Submenu::new("ChartCraft", true);
        let _ = app.append(&m.item("About ChartCraft", None, Cmd::Act(Action::About)));
        let _ = app.append(&sep());
        let _ = app.append(&PredefinedMenuItem::services(None));
        let _ = app.append(&sep());
        let _ = app.append(&PredefinedMenuItem::hide(None));
        let _ = app.append(&PredefinedMenuItem::hide_others(None));
        let _ = app.append(&PredefinedMenuItem::show_all(None));
        let _ = app.append(&sep());
        // Our own Quit rather than the stock one, so unsaved diagrams are asked about first.
        let _ = app.append(&m.item("Quit ChartCraft", key("Cmd+Q"), Cmd::Act(Action::Exit)));

        let file = Submenu::new("File", true);
        let _ = file.append(&m.item("New Diagram", key("Cmd+N"), Cmd::Act(Action::New)));
        let _ = file.append(&m.item("Open…", key("Cmd+O"), Cmd::Act(Action::Open)));
        let _ = file.append(&m.recent);
        let _ = file.append(&sep());
        let _ = file.append(&m.item("Close Tab", key("Cmd+W"), Cmd::Act(Action::CloseTab(usize::MAX))));
        let _ = file.append(&m.item("Save", key("Cmd+S"), Cmd::Act(Action::Save)));
        let _ = file.append(&m.item("Save As…", key("Cmd+Shift+S"), Cmd::Act(Action::SaveAs)));
        let _ = file.append(&m.check("Save Compressed (Smaller Files)", Flag::Compress));
        let _ = file.append(&sep());
        let _ = file.append(&m.item("Export to PDF…", key("Cmd+E"), Cmd::Act(Action::ExportPdf)));
        let _ = file.append(&m.item("Export Picture (PNG or SVG)…", None, Cmd::Act(Action::ExportPicture)));
        let _ = file.append(&sep());
        let _ = file.append(&m.item("Import Tables from MySQL…", None, Cmd::Act(Action::ImportMysql)));
        let _ = file.append(&sep());
        let _ = file.append(&m.item("Page Setup…", None, Cmd::Act(Action::PageSetup)));
        let _ = file.append(&m.item("Print…", key("Cmd+P"), Cmd::Act(Action::Print)));

        let edit = Submenu::new("Edit", true);
        let _ = edit.append(&m.gated("Undo", Action::Undo, Gate::Undo));
        let _ = edit.append(&m.gated("Redo", Action::Redo, Gate::Redo));
        let _ = edit.append(&sep());
        let _ = edit.append(&m.gated("Cut", Action::Cut, Gate::Selection));
        let _ = edit.append(&m.gated("Copy", Action::Copy, Gate::Selection));
        let _ = edit.append(&m.gated("Paste", Action::Paste, Gate::Paste));
        let _ = edit.append(&m.gated("Duplicate", Action::Duplicate, Gate::Selection));
        let _ = edit.append(&m.gated("Delete", Action::Delete, Gate::Selection));
        let _ = edit.append(&sep());
        let _ = edit.append(&m.item("Diagram Properties and Page Setup…", None, Cmd::Act(Action::PageSetup)));

        let view = Submenu::new("View", true);
        let _ = view.append(&m.item("Zoom In", None, Cmd::Act(Action::ZoomIn)));
        let _ = view.append(&m.item("Zoom Out", None, Cmd::Act(Action::ZoomOut)));
        let _ = view.append(&m.item("Actual Size (100%)", None, Cmd::Act(Action::Zoom(1.0))));
        let _ = view.append(&m.item("Best Fit", key("Cmd+Shift+E"), Cmd::Act(Action::BestFit)));
        let _ = view.append(&sep());
        let _ = view.append(&m.check("Show Grid", Flag::Grid));
        let _ = view.append(&m.check("Snap to Grid", Flag::Snap));
        let _ = view.append(&m.check("Snap Lines to Objects", Flag::ObjectSnap));
        let _ = view.append(&m.check("Show Connection Points", Flag::ConnectionPoints));
        let _ = view.append(&m.check("Show Page Breaks", Flag::PageBreaks));
        let _ = view.append(&m.check("Show Rulers", Flag::Rulers));
        let _ = view.append(&sep());
        let theme = Submenu::new("Theme", true);
        for t in ThemeChoice::ALL {
            let _ = theme.append(&m.ticked(t.label(), Cmd::Theme(t), Tick::Theme(t)));
        }
        let _ = view.append(&theme);

        let layers = m.layers.clone();
        let _ = layers.append(&m.item("Add Layer", None, Cmd::Act(Action::AddLayer)));
        let _ = layers.append(&m.gated("Move Selection to the Active Layer", Action::MoveToLayer, Gate::Selection));
        let _ = layers.append(&m.gated("Show All Layers", Action::ShowAllLayers, Gate::HiddenLayers));

        let objects = Submenu::new("Objects", true);
        let _ = objects.append(&m.item("Properties…", None, Cmd::Act(Action::Props)));
        let _ = objects.append(&m.gated("Edit Text", Action::EditText, Gate::OneSelected));
        let _ = objects.append(&sep());
        let _ = objects.append(&m.gated("Bring to Front", Action::Reorder(0), Gate::Selection));
        let _ = objects.append(&m.gated("Send to Back", Action::Reorder(1), Gate::Selection));
        let _ = objects.append(&m.gated("Raise", Action::Reorder(2), Gate::Selection));
        let _ = objects.append(&m.gated("Lower", Action::Reorder(3), Gate::Selection));
        let _ = objects.append(&sep());
        let align = Submenu::new("Align", true);
        let aligns = [
            "Left Edges",
            "Centres (Left–Right)",
            "Right Edges",
            "Top Edges",
            "Middles (Up–Down)",
            "Bottom Edges",
            "Spread Evenly Across",
            "Spread Evenly Down",
        ];
        for (i, name) in aligns.iter().enumerate() {
            if i == 3 || i == 6 {
                let _ = align.append(&sep());
            }
            let _ = align.append(&m.gated(name, Action::Align(i as u8), Gate::TwoSelected));
        }
        let _ = objects.append(&align);

        let select = Submenu::new("Select", true);
        let _ = select.append(&m.item("All", None, Cmd::Act(Action::SelectAll)));
        let _ = select.append(&m.gated("None", Action::SelectNone, Gate::Selection));
        let _ = select.append(&m.item("Invert", None, Cmd::Act(Action::SelectInvert)));
        let _ = select.append(&m.gated("Same Type", Action::SelectSameType, Gate::Selection));
        let _ = select.append(&m.gated("Connected", Action::SelectConnected, Gate::Selection));

        let tools = Submenu::new("Tools", true);
        for (t, tip) in TOOLBOX_TOOLS {
            let name = tip.split(':').next().unwrap_or(tip);
            let _ = tools.append(&m.ticked(name, Cmd::Tool(t), Tick::Tool(t)));
        }

        let window = Submenu::new("Window", true);
        let _ = window.append(&PredefinedMenuItem::minimize(None));
        let _ = window.append(&PredefinedMenuItem::maximize(None));
        let _ = window.append(&PredefinedMenuItem::fullscreen(None));
        let _ = window.append(&sep());
        let _ = window.append(&m.item("Show Next Tab", key("Ctrl+Tab"), Cmd::Act(Action::NextTab)));
        let _ = window.append(&sep());
        let _ = window.append(&PredefinedMenuItem::bring_all_to_front(None));

        let help = Submenu::new("Help", true);
        let _ = help.append(&m.item("Tips", key("Cmd+Shift+Slash"), Cmd::Act(Action::Tips)));
        let _ = help.append(&m.item("Open the Example Flowchart", None, Cmd::Act(Action::Example)));
        let _ = help.append(&m.item("Use with Claude Code (MCP)…", None, Cmd::Act(Action::McpHelp)));

        for sub in [&app, &file, &edit, &view, &m.layers, &objects, &select, &tools, &window, &help] {
            let _ = bar.append(sub);
        }
        bar.init_for_nsapp();
        window.set_as_windows_menu_for_nsapp();
        help.set_as_help_menu_for_nsapp();
        m._bar = bar;
        m
    }

    fn id(&mut self, cmd: Cmd) -> MenuId {
        self.next_id += 1;
        let id = MenuId::new(format!("cc{}", self.next_id));
        self.cmds.insert(id.clone(), cmd);
        id
    }

    fn item(&mut self, text: &str, accel: Option<Accelerator>, cmd: Cmd) -> MenuItem {
        MenuItem::with_id(self.id(cmd), text, true, accel)
    }

    fn gated(&mut self, text: &str, a: Action, gate: Gate) -> MenuItem {
        let it = self.item(text, None, Cmd::Act(a));
        self.gated.push((it.clone(), gate, None));
        it
    }

    fn ticked(&mut self, text: &str, cmd: Cmd, tick: Tick) -> CheckMenuItem {
        let it = CheckMenuItem::with_id(self.id(cmd), text, true, false, None);
        self.ticked.push((it.clone(), tick, None));
        it
    }

    fn check(&mut self, text: &str, f: Flag) -> CheckMenuItem {
        self.ticked(text, Cmd::Toggle(f), Tick::Flag(f))
    }

    /// Swaps out the items a dynamic submenu showed last time.
    fn clear(sub: &Submenu, old: &mut Vec<Box<dyn IsMenuItem>>) {
        for it in old.drain(..) {
            let _ = sub.remove(it.as_ref());
        }
    }
}

impl App {
    fn flag(&mut self, f: Flag) -> &mut bool {
        match f {
            Flag::Compress => &mut self.compress,
            Flag::Grid => &mut self.ed.show_grid,
            Flag::Snap => &mut self.ed.snap_to_grid,
            Flag::ObjectSnap => &mut self.ed.object_snap,
            Flag::ConnectionPoints => &mut self.ed.show_cps,
            Flag::PageBreaks => &mut self.ed.show_pages,
            Flag::Rulers => &mut self.ed.show_rulers,
        }
    }

    /// Runs what was chosen in the menu bar since the last frame, then brings the menus' ticks,
    /// greyed-out items and lists up to date. Called every frame.
    pub(super) fn native_menu(&mut self, ctx: &egui::Context, act: &mut Option<Action>) {
        let mut m = match self.native.take() {
            Some(m) => m,
            None => NativeMenu::install(ctx),
        };

        while let Ok(id) = m.rx.try_recv() {
            let Some(cmd) = m.cmds.get(&id).copied() else { continue };
            if matches!(cmd, Cmd::ActiveLayer(_) | Cmd::ShowLayer(_)) {
                // macOS flips the tick itself on a click; redraw the list from the diagram.
                m.layer_key = None;
            }
            match cmd {
                Cmd::Act(a) => *act = Some(a),
                Cmd::Toggle(f) => {
                    let v = self.flag(f);
                    *v = !*v;
                    if !matches!(f, Flag::Compress) {
                        self.ed.touch();
                    }
                }
                Cmd::Theme(t) => self.set_theme(ctx, t),
                Cmd::Tool(t) => self.set_tool(t, false),
                Cmd::ActiveLayer(i) => {
                    if i < self.ed.d.layers.len() {
                        self.ed.d.active = i;
                    }
                }
                Cmd::ShowLayer(i) => {
                    if let Some(l) = self.ed.d.layers.get_mut(i) {
                        l.visible = !l.visible;
                        self.ed.sel.retain(|id| self.ed.d.get(*id).map(|o| self.ed.d.layer_visible(&o.layer)).unwrap_or(false));
                        self.ed.notify_changed();
                    }
                }
            }
        }

        // Greyed out or not.
        let n = self.ed.sel.len();
        let hidden = self.ed.d.layers.iter().any(|l| !l.visible);
        for (it, gate, last) in m.gated.iter_mut() {
            let on = match gate {
                Gate::Selection => n > 0,
                Gate::OneSelected => n == 1,
                Gate::TwoSelected => n >= 2,
                Gate::Undo => self.ed.can_undo(),
                Gate::Redo => self.ed.can_redo(),
                Gate::Paste => self.ed.can_paste(),
                Gate::HiddenLayers => hidden,
            };
            if *last != Some(on) {
                it.set_enabled(on);
                *last = Some(on);
            }
        }
        // Ticks. Always set when the menu itself may have flipped one on a click.
        let tool = self.ed.tool;
        let theme = self.theme;
        let mut flags = [false; 7];
        for (i, f) in [Flag::Compress, Flag::Grid, Flag::Snap, Flag::ObjectSnap, Flag::ConnectionPoints, Flag::PageBreaks, Flag::Rulers].into_iter().enumerate() {
            flags[i] = *self.flag(f);
        }
        for (it, tick, last) in m.ticked.iter_mut() {
            let on = match tick {
                Tick::Flag(f) => flags[*f as usize],
                Tick::Theme(t) => *t == theme,
                Tick::Tool(t) => *t == tool,
            };
            if *last != Some(on) || it.is_checked() != on {
                it.set_checked(on);
                *last = Some(on);
            }
        }

        // File → Open Recent.
        if m.recent_key.as_ref() != Some(&self.recent) {
            NativeMenu::clear(&m.recent, &mut m.recent_items);
            for (n, path) in self.recent.iter().enumerate() {
                let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let it = m.item(&name, None, Cmd::Act(Action::OpenRecent(n)));
                let _ = m.recent.append(&it);
                m.recent_items.push(Box::new(it));
            }
            let sep = PredefinedMenuItem::separator();
            let _ = m.recent.append(&sep);
            m.recent_items.push(Box::new(sep));
            let it = m.item("Clear Menu", None, Cmd::Act(Action::ClearRecent));
            let _ = m.recent.append(&it);
            m.recent_items.push(Box::new(it));
            m.recent.set_enabled(!self.recent.is_empty());
            m.recent_key = Some(self.recent.clone());
        }

        // Layers: which is active, and which are shown.
        let layers: Vec<(String, bool, bool)> = self.ed.d.layers.iter().enumerate().map(|(i, l)| (l.name.clone(), l.visible, self.ed.d.active == i)).collect();
        if m.layer_key.as_ref() != Some(&layers) {
            let layers_menu = m.layers.clone();
            NativeMenu::clear(&layers_menu, &mut m.layer_items);
            let add = |m: &mut NativeMenu, it: Box<dyn IsMenuItem>| {
                let _ = layers_menu.append(it.as_ref());
                m.layer_items.push(it);
            };
            add(&mut m, Box::new(PredefinedMenuItem::separator()));
            add(&mut m, Box::new(MenuItem::new("Active Layer", false, None)));
            for (i, (name, _, active)) in layers.iter().enumerate() {
                let it = CheckMenuItem::with_id(m.id(Cmd::ActiveLayer(i)), name, true, *active, None);
                add(&mut m, Box::new(it));
            }
            add(&mut m, Box::new(PredefinedMenuItem::separator()));
            let shown = Submenu::new("Shown Layers", true);
            for (i, (name, visible, _)) in layers.iter().enumerate() {
                let _ = shown.append(&CheckMenuItem::with_id(m.id(Cmd::ShowLayer(i)), name, true, *visible, None));
            }
            add(&mut m, Box::new(shown));
            m.layer_key = Some(layers);
        }

        self.native = Some(m);
    }
}
