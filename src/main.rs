#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! ChartCraft: draw flowcharts and diagrams, open and save .dia files, export to PDF.

mod app;
mod dia;
mod editor;
mod fonts;
mod geom;
mod icons;
mod mcp;
mod model;
mod mysql_import;
mod pdf;
mod record;
mod render;
mod shapes;
mod skia;
mod svg;
mod theme;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

fn window_icon() -> egui::IconData {
    let size = 64u32;
    let mut pm = tiny_skia::Pixmap::new(size, size).unwrap();
    let mut d = model::Diagram::default();
    let mut a = model::Element::new(shapes::index_of("fc-box"), geom::r(0.3, 0.4, 1.4, 1.0));
    a.fill_color = model::Col::rgb(70, 120, 200);
    let mut b = model::Element::new(shapes::index_of("fc-diamond"), geom::r(1.4, 1.5, 1.4, 1.4));
    b.fill_color = model::Col::rgb(240, 160, 60);
    let ia = d.add(model::Kind::Element(a));
    let ib = d.add(model::Kind::Element(b));
    let mut l = model::LineObj::new(model::LineKind::ZigZag, vec![geom::p(1.0, 1.4), geom::p(1.0, 2.2), geom::p(1.4, 2.2)]);
    l.width = 0.12;
    l.start_att = Some(model::Attachment { target: ia, u: 0.5, v: 1.0, cp: None });
    l.end_att = Some(model::Attachment { target: ib, u: 0.0, v: 0.5, cp: None });
    d.add(model::Kind::Line(l));
    for o in d.objects.iter_mut() {
        if let model::Kind::Element(e) = &mut o.kind {
            e.line_width = 0.12;
        }
    }
    {
        let mut sp = skia::SkiaPainter { pm: &mut pm, scale: size as f64 / 3.0, ox: 0.0, oy: 0.0, offx: 0.0, offy: 0.0, min_width: 1.0 };
        render::draw_diagram(&d, &mut sp, None);
    }
    egui::IconData { rgba: skia::unpremultiply(pm.data()), width: size, height: size }
}

fn main() -> eframe::Result<()> {
    // As an MCP server for Claude Code and other AI tools: no window, talk on stdin/stdout.
    if std::env::args().any(|a| a == "--mcp") {
        mcp::serve();
        return Ok(());
    }
    let mut shot: Option<PathBuf> = None;
    let mut open: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--help-mcp" {
            std::env::set_var("CHARTCRAFT_MCPHELP", "1");
        } else if a == "--mysql" {
            std::env::set_var("CHARTCRAFT_MYSQL", "1");
        } else if a == "--props" {
            if let Some(tab) = args.next() {
                std::env::set_var("CHARTCRAFT_PROPS", tab);
            }
        } else if a == "--sheet" {
            if let Some(name) = args.next() {
                std::env::set_var("CHARTCRAFT_SHEET", name);
            }
        } else if a == "--theme" {
            if let Some(t) = args.next() {
                std::env::set_var("CHARTCRAFT_THEME", t);
            }
        } else if a == "--screenshot" {
            shot = args.next().map(PathBuf::from);
        } else if !a.starts_with("--") {
            open.push(PathBuf::from(a));
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ChartCraft")
        .with_inner_size([1440.0, 830.0])
        .with_min_inner_size([720.0, 460.0])
        .with_icon(window_icon());
    if shot.is_some() {
        // Checking the build: open without taking focus, and behind other windows on Windows.
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if shot.is_some() {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        options.event_loop_builder = Some(Box::new(|b| {
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native("ChartCraft", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, open, shot)))))
}
