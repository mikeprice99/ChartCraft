//! Small pictures: the toolbar uses the Phosphor icon font; the toolbox tools are drawn with simple shapes.

use crate::editor::Tool;
use egui::{pos2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
// Some icons are mapped but not on the toolbar yet; keep them ready.
#[allow(dead_code)]
pub enum Icon {
    New,
    Open,
    Save,
    SaveAs,
    Export,
    ObjSnap,
    Pdf,
    Print,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    ZoomIn,
    ZoomOut,
    Fit,
    Grid,
    Snap,
    AlignLeft,
    AlignCenter,
    AlignTop,
    AlignMiddle,
    Front,
    Back,
    Props,
}


fn at(r: Rect, u: f32, v: f32) -> Pos2 {
    pos2(r.min.x + r.width() * u, r.min.y + r.height() * v)
}

fn sub(r: Rect, u0: f32, v0: f32, u1: f32, v1: f32) -> Rect {
    Rect::from_min_max(at(r, u0, v0), at(r, u1, v1))
}

/// The Phosphor icon font (regular weight), MIT licence: see assets/phosphor/LICENSE.
const PHOSPHOR_TTF: &[u8] = include_bytes!("../assets/phosphor/Phosphor.ttf");
const PHOSPHOR: &str = "phosphor";

/// Make the Phosphor icons available to egui (as their own font family).
pub fn install_font(ctx: &egui::Context) {
    ctx.add_font(egui::FontInsert::new(
        PHOSPHOR,
        egui::FontData::from_static(PHOSPHOR_TTF),
        vec![egui::InsertFontFamily { family: egui::FontFamily::Name(PHOSPHOR.into()), priority: egui::FontPriority::Highest }],
    ));
}

/// Each toolbar picture's character in the Phosphor font (names as on phosphoricons.com).
pub fn glyph(icon: Icon) -> char {
    match icon {
        Icon::New => '\u{e236}',         // file-plus
        Icon::Open => '\u{e256}',        // folder-open
        Icon::Save => '\u{e248}',        // floppy-disk
        Icon::SaveAs => '\u{eaf4}',      // floppy-disk-back
        Icon::Export => '\u{eaf0}',      // export
        Icon::ObjSnap => '\u{e680}',     // magnet
        Icon::Pdf => '\u{e702}',         // file-pdf
        Icon::Print => '\u{e3dc}',       // printer
        Icon::Undo => '\u{e08a}',        // arrow-u-up-left
        Icon::Redo => '\u{e08c}',        // arrow-u-up-right
        Icon::Cut => '\u{eae0}',         // scissors
        Icon::Copy => '\u{e1ca}',        // copy
        Icon::Paste => '\u{e198}',       // clipboard-text
        Icon::Delete => '\u{e4a6}',      // trash
        Icon::ZoomIn => '\u{e310}',      // magnifying-glass-plus
        Icon::ZoomOut => '\u{e30e}',     // magnifying-glass-minus
        Icon::Fit => '\u{e626}',         // frame-corners
        Icon::Grid => '\u{ec8c}',        // grid-nine
        Icon::Snap => '\u{e1fc}',        // dots-nine
        Icon::AlignLeft => '\u{e50e}',   // align-left
        Icon::AlignCenter => '\u{e50a}', // align-center-horizontal
        Icon::AlignTop => '\u{e512}',    // align-top
        Icon::AlignMiddle => '\u{e50c}', // align-center-vertical
        Icon::Front => '\u{eaf6}',       // selection-foreground
        Icon::Back => '\u{eaf8}',        // selection-background
        Icon::Props => '\u{e434}',       // sliders-horizontal
    }
}

/// Draw a toolbar picture filling `r`, in the theme's text colour (greyed when disabled).
pub fn paint_icon(p: &Painter, r: Rect, icon: Icon, enabled: bool) {
    let pal = crate::theme::pal(p.ctx());
    let ink = if enabled { pal.ink } else { pal.ink_disabled };
    let font = egui::FontId::new(r.height(), egui::FontFamily::Name(PHOSPHOR.into()));
    p.text(r.center(), egui::Align2::CENTER_CENTER, glyph(icon), font, ink);
}

pub fn paint_tool(p: &Painter, r: Rect, tool: Tool) {
    let pal = crate::theme::pal(p.ctx());
    let ink = pal.ink;
    let s = Stroke::new(1.4, ink);
    let fill = pal.paper;
    match tool {
        Tool::Select => {
            let pts = vec![at(r, 0.25, 0.0), at(r, 0.25, 0.85), at(r, 0.45, 0.65), at(r, 0.62, 1.0), at(r, 0.74, 0.93), at(r, 0.57, 0.6), at(r, 0.85, 0.6)];
            // Not convex: draw as two convex halves plus the outline.
            p.add(Shape::convex_polygon(vec![pts[0], pts[1], pts[2], pts[5], pts[6]], fill, Stroke::NONE));
            p.add(Shape::convex_polygon(vec![pts[2], pts[3], pts[4], pts[5]], fill, Stroke::NONE));
            p.add(Shape::closed_line(pts, s));
        }
        Tool::Magnify => {
            let c = at(r, 0.38, 0.38);
            p.circle_filled(c, r.width() * 0.3, fill);
            p.circle_stroke(c, r.width() * 0.3, s);
            p.line_segment([at(r, 0.6, 0.6), at(r, 1.0, 1.0)], Stroke::new(3.0, ink));
            p.line_segment([at(r, 0.23, 0.38), at(r, 0.53, 0.38)], s);
            p.line_segment([at(r, 0.38, 0.23), at(r, 0.38, 0.53)], s);
        }
        Tool::Scroll => {
            p.line_segment([at(r, 0.0, 0.5), at(r, 1.0, 0.5)], s);
            p.line_segment([at(r, 0.5, 0.0), at(r, 0.5, 1.0)], s);
            let a = 0.2;
            for (tip, l, rr) in [
                (at(r, 0.5, 0.0), at(r, 0.5 - a, a), at(r, 0.5 + a, a)),
                (at(r, 0.5, 1.0), at(r, 0.5 - a, 1.0 - a), at(r, 0.5 + a, 1.0 - a)),
                (at(r, 0.0, 0.5), at(r, a, 0.5 - a), at(r, a, 0.5 + a)),
                (at(r, 1.0, 0.5), at(r, 1.0 - a, 0.5 - a), at(r, 1.0 - a, 0.5 + a)),
            ] {
                p.add(Shape::convex_polygon(vec![tip, l, rr], ink, Stroke::NONE));
            }
        }
        Tool::TextEdit => {
            let x = r.center().x;
            p.line_segment([pos2(x, r.min.y + 1.0), pos2(x, r.max.y - 1.0)], s);
            p.line_segment([pos2(x - 3.0, r.min.y + 1.0), pos2(x + 3.0, r.min.y + 1.0)], s);
            p.line_segment([pos2(x - 3.0, r.max.y - 1.0), pos2(x + 3.0, r.max.y - 1.0)], s);
        }
        Tool::Text => {
            p.text(r.center(), egui::Align2::CENTER_CENTER, "T", egui::FontId::proportional(r.height() * 1.15), ink);
            p.text(r.center() + egui::vec2(0.6, 0.0), egui::Align2::CENTER_CENTER, "T", egui::FontId::proportional(r.height() * 1.15), ink);
        }
        Tool::Box => {
            let q = sub(r, 0.0, 0.18, 1.0, 0.82);
            p.rect_filled(q, 0.0, fill);
            p.rect_stroke(q, 0.0, s, StrokeKind::Inside);
        }
        Tool::Ellipse => {
            let c = r.center();
            let pts: Vec<Pos2> = (0..32)
                .map(|i| {
                    let t = i as f32 / 32.0 * std::f32::consts::TAU;
                    pos2(c.x + r.width() * 0.5 * t.cos(), c.y + r.height() * 0.32 * t.sin())
                })
                .collect();
            p.add(Shape::convex_polygon(pts, fill, s));
        }
        Tool::Polygon => {
            p.add(Shape::convex_polygon(vec![at(r, 0.3, 0.0), at(r, 1.0, 0.35), at(r, 0.75, 1.0), at(r, 0.0, 0.7)], fill, s));
        }
        Tool::Line => {
            p.line_segment([at(r, 0.0, 1.0), at(r, 0.85, 0.15)], s);
            p.add(Shape::convex_polygon(vec![at(r, 1.0, 0.0), at(r, 0.62, 0.15), at(r, 0.85, 0.38)], ink, Stroke::NONE));
        }
        Tool::Arc => {
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let t = std::f32::consts::PI * (1.1 + 0.8 * i as f32 / 16.0);
                    pos2(r.center().x + r.width() * 0.55 * t.cos(), r.max.y + r.height() * 0.25 + r.height() * 0.9 * t.sin())
                })
                .collect();
            p.add(Shape::line(pts, s));
        }
        Tool::ZigZag => {
            p.add(Shape::line(vec![at(r, 0.0, 0.1), at(r, 0.5, 0.1), at(r, 0.5, 0.9), at(r, 1.0, 0.9)], s));
        }
        Tool::PolyLine => {
            p.add(Shape::line(vec![at(r, 0.0, 1.0), at(r, 0.3, 0.1), at(r, 0.65, 0.8), at(r, 1.0, 0.0)], s));
        }
        Tool::Bezier => {
            let (a, b, c, d) = (at(r, 0.0, 1.0), at(r, 0.2, -0.4), at(r, 0.8, 1.4), at(r, 1.0, 0.0));
            let pts: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let t = i as f32 / 20.0;
                    let u = 1.0 - t;
                    let k = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    pos2(a.x * k[0] + b.x * k[1] + c.x * k[2] + d.x * k[3], a.y * k[0] + b.y * k[1] + c.y * k[2] + d.y * k[3])
                })
                .collect();
            p.add(Shape::line(pts, s));
        }
        Tool::Image => {
            let q = sub(r, 0.0, 0.12, 1.0, 0.88);
            p.rect_filled(q, 0.0, Color32::from_rgb(200, 225, 250));
            p.add(Shape::convex_polygon(vec![at(q, 0.0, 1.0), at(q, 0.35, 0.4), at(q, 0.7, 1.0)], Color32::from_rgb(90, 160, 90), Stroke::NONE));
            p.add(Shape::convex_polygon(vec![at(q, 0.45, 1.0), at(q, 0.72, 0.55), at(q, 1.0, 1.0)], Color32::from_rgb(70, 140, 70), Stroke::NONE));
            p.circle_filled(at(q, 0.8, 0.25), q.width() * 0.1, Color32::from_rgb(240, 200, 60));
            p.rect_stroke(q, 0.0, s, StrokeKind::Inside);
        }
        Tool::Shape(_) | Tool::Connector(_) => {}
    }
}
