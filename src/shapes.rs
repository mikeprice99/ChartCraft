//! The kinds of shapes: how each looks, where lines attach and where its text goes.

use crate::geom::*;
use crate::model::{attach_point, Attachment, Element};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    StdBox,
    StdEllipse,
    FlowBuiltin,
    Custom,
    /// UML and database objects (written with line_colour/fill_colour like the original).
    Uml,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cps {
    Box9,
    Box17,
    Ellipse8,
    Ellipse16,
    Diamond16,
    OnOutline17,
    /// Class/table boxes: 8 around the box, then left and right of every row, then the middle.
    Record,
}

pub struct ShapeDef {
    pub key: &'static str,
    pub name: &'static str,
    pub dia_type: &'static str,
    pub fam: Family,
    pub sheet: &'static str,
    pub has_text: bool,
    pub def_w: f64,
    pub def_h: f64,
    pub def_corner: f64,
    pub outline: fn(&Element) -> Path,
    pub details: Option<fn(R) -> Path>,
    /// Parts filled with the line colour (dots, bars).
    pub solid: Option<fn(R) -> Path>,
    pub cps: Cps,
    pub text_area: fn(&Element) -> R,
}

pub const FLOWCHART: &str = "Flowchart";
pub const BASIC: &str = "Basic shapes";
pub const UML: &str = "UML";
pub const DATABASE: &str = "Database";
pub const SHEETS: &[&str] = &[FLOWCHART, UML, DATABASE, BASIC];

/// Ready-made connector lines on a sheet (UML relationships and so on).
pub struct LinePreset {
    pub name: &'static str,
    pub sheet: &'static str,
    pub start: i64,
    pub end: i64,
    pub dashed: bool,
}

pub const LINE_PRESETS: &[LinePreset] = &[
    LinePreset { name: "Association", sheet: UML, start: 0, end: 0, dashed: false },
    LinePreset { name: "Directed association", sheet: UML, start: 0, end: 1, dashed: false },
    LinePreset { name: "Generalization (inherits from)", sheet: UML, start: 0, end: 2, dashed: false },
    LinePreset { name: "Realization (implements)", sheet: UML, start: 0, end: 2, dashed: true },
    LinePreset { name: "Dependency", sheet: UML, start: 0, end: 1, dashed: true },
    LinePreset { name: "Aggregation", sheet: UML, start: 4, end: 0, dashed: false },
    LinePreset { name: "Composition", sheet: UML, start: 5, end: 0, dashed: false },
    LinePreset { name: "Reference (foreign key)", sheet: DATABASE, start: 0, end: 1, dashed: false },
    LinePreset { name: "One to many", sheet: DATABASE, start: 0, end: 3, dashed: false },
];


const BOX9: [(f64, f64); 9] = [(0., 0.), (0.5, 0.), (1., 0.), (0., 0.5), (1., 0.5), (0., 1.), (0.5, 1.), (1., 1.), (0.5, 0.5)];
const BOX17: [(f64, f64); 17] = [
    (0., 0.),
    (0.25, 0.),
    (0.5, 0.),
    (0.75, 0.),
    (1., 0.),
    (0., 0.25),
    (1., 0.25),
    (0., 0.5),
    (1., 0.5),
    (0., 0.75),
    (1., 0.75),
    (0., 1.),
    (0.25, 1.),
    (0.5, 1.),
    (0.75, 1.),
    (1., 1.),
    (0.5, 0.5),
];

/// Connection points of a shape; the last one is always the middle.
pub fn connection_points(e: &Element) -> Vec<P> {
    let b = e.rect;
    match def(e.def).cps {
        Cps::Box9 => BOX9.iter().map(|&(u, v)| b.at(u, v)).collect(),
        Cps::Box17 => BOX17.iter().map(|&(u, v)| b.at(u, v)).collect(),
        Cps::Ellipse8 => {
            let mut v: Vec<P> = [225.0f64, 270., 315., 180., 0., 135., 90., 45.]
                .iter()
                .map(|a| {
                    let t = a.to_radians();
                    b.at(0.5 + 0.5 * t.cos(), 0.5 + 0.5 * t.sin())
                })
                .collect();
            v.push(b.center());
            v
        }
        Cps::Ellipse16 => {
            let mut v: Vec<P> = (0..16)
                .map(|i| {
                    let t = (270.0 + i as f64 * 22.5f64).to_radians();
                    b.at(0.5 + 0.5 * t.cos(), 0.5 + 0.5 * t.sin())
                })
                .collect();
            v.push(b.center());
            v
        }
        Cps::Diamond16 => {
            let c = [b.at(0.5, 0.), b.at(1., 0.5), b.at(0.5, 1.), b.at(0., 0.5)];
            let mut v = Vec::new();
            for i in 0..4 {
                for k in 0..4 {
                    v.push(c[i] + (c[(i + 1) % 4] - c[i]) * (k as f64 / 4.0));
                }
            }
            v.push(b.center());
            v
        }
        Cps::Record => {
            let mut v: Vec<P> = BOX9[..8].iter().map(|&(u, v)| b.at(u, v)).collect();
            // The side points sit beside the class name, as in the original.
            let ty = crate::record::title_center(e);
            v[3] = p(b.x, ty);
            v[4] = p(b.right(), ty);
            for y in crate::record::row_centers(e) {
                v.push(p(b.x, y));
                v.push(p(b.right(), y));
            }
            v.push(b.center());
            v
        }
        Cps::OnOutline17 => {
            let geo = e.geometry_copy();
            BOX17
                .iter()
                .map(|&(u, v)| {
                    if (u - 0.5).abs() < 1e-9 && (v - 0.5).abs() < 1e-9 {
                        return b.center();
                    }
                    let t = b.at(u, v);
                    attach_point(&geo, &Attachment::center(0), t + (t - b.center()))
                })
                .collect()
        }
    }
}

fn pt(b: R, u: f64, v: f64) -> P {
    b.at(u, v)
}

fn shear_offset(e: &Element) -> f64 {
    let ang = e.shear_angle.clamp(10.0, 170.0).to_radians();
    (e.rect.h / ang.tan()).clamp(-e.rect.w * 0.45, e.rect.w * 0.45)
}

fn inset(b: R, x0: f64, y0: f64, w: f64, h: f64) -> R {
    r(b.x + b.w * x0, b.y + b.h * y0, b.w * w, b.h * h)
}

fn whole(e: &Element) -> R {
    e.rect
}
fn ellipse_area(e: &Element) -> R {
    inset(e.rect, 0.146, 0.146, 0.708, 0.708)
}
fn diamond_area(e: &Element) -> R {
    inset(e.rect, 0.25, 0.25, 0.5, 0.5)
}

const K: f64 = 0.5523;

fn build() -> Vec<ShapeDef> {
    use Family::*;
    let f = FLOWCHART;
    let b = BASIC;
    let u = UML;
    let db = DATABASE;
    macro_rules! shape {
        ($key:expr, $name:expr, $ty:expr, $fam:expr, $sheet:expr, $w:expr, $h:expr, $outline:expr) => {
            ShapeDef {
                key: $key,
                name: $name,
                dia_type: $ty,
                fam: $fam,
                sheet: $sheet,
                has_text: true,
                def_w: $w,
                def_h: $h,
                def_corner: 0.0,
                outline: $outline,
                details: None,
                solid: None,
                cps: match $fam {
                    StdBox => Cps::Box9,
                    FlowBuiltin | Uml => Cps::Box17,
                    _ => Cps::OnOutline17,
                },
                text_area: whole,
            }
        };
    }
    vec![
        // ---- Flowchart shapes with text inside.
        shape!("fc-box", "Process", "Flowchart - Box", FlowBuiltin, f, 4.0, 2.0, |e| Path::rect(e.rect)),
        ShapeDef {
            cps: Cps::Diamond16,
            text_area: diamond_area,
            ..shape!("fc-diamond", "Decision", "Flowchart - Diamond", FlowBuiltin, f, 4.0, 2.5, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0.5, 0.), pt(b, 1., 0.5), pt(b, 0.5, 1.), pt(b, 0., 0.5)])
            })
        },
        ShapeDef {
            cps: Cps::Ellipse16,
            text_area: ellipse_area,
            ..shape!("fc-ellipse", "Start / end (ellipse)", "Flowchart - Ellipse", FlowBuiltin, f, 4.0, 2.0, |e| Path::ellipse(e.rect))
        },
        ShapeDef {
            cps: Cps::OnOutline17,
            text_area: |e| {
                let o = shear_offset(e).abs();
                r(e.rect.x + o / 2.0, e.rect.y, e.rect.w - o, e.rect.h)
            },
            ..shape!("fc-parallelogram", "Input / output", "Flowchart - Parallelogram", FlowBuiltin, f, 4.0, 2.0, |e| {
                let b = e.rect;
                let o = shear_offset(e);
                if o >= 0.0 {
                    Path::polygon(&[p(b.x + o, b.y), p(b.right(), b.y), p(b.right() - o, b.bottom()), p(b.x, b.bottom())])
                } else {
                    Path::polygon(&[p(b.x, b.y), p(b.right() + o, b.y), p(b.right(), b.bottom()), p(b.x - o, b.bottom())])
                }
            })
        },
        ShapeDef {
            text_area: |e| r(e.rect.x + e.rect.h / 4.0, e.rect.y, e.rect.w - e.rect.h / 2.0, e.rect.h),
            ..shape!("fc-terminal", "Terminal", "Flowchart - Terminal", Custom, f, 4.0, 1.5, |e| Path::round_rect(e.rect, e.rect.h / 2.0))
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0., 0., 1., 0.85),
            ..shape!("fc-document", "Document", "Flowchart - Document", Custom, f, 4.0, 2.2, |e| {
                let b = e.rect;
                let wy = b.bottom() - b.h * 0.12;
                Path::new()
                    .move_to(b.x, b.y)
                    .line_to(b.right(), b.y)
                    .line_to(b.right(), wy)
                    .curve_to(b.x + b.w * 0.75, wy - b.h * 0.25, b.x + b.w * 0.5, b.bottom() + b.h * 0.05, b.x + b.w * 0.25, b.bottom())
                    .curve_to(b.x + b.w * 0.12, b.bottom() - b.h * 0.02, b.x + b.w * 0.04, b.bottom() - b.h * 0.06, b.x, wy)
                    .close()
            })
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0., 0.25, 1., 0.75),
            ..shape!("fc-manual-input", "Manual input", "Flowchart - Manual Input", Custom, f, 4.0, 2.0, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0., 0.3), pt(b, 1., 0.), pt(b, 1., 1.), pt(b, 0., 1.)])
            })
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0.15, 0., 0.7, 1.),
            ..shape!("fc-manual-op", "Manual operation", "Flowchart - Manual Operation", Custom, f, 4.0, 2.0, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0., 0.), pt(b, 1., 0.), pt(b, 0.8, 1.), pt(b, 0.2, 1.)])
            })
        },
        ShapeDef {
            details: Some(|b| {
                Path::new()
                    .move_to(b.x + b.w * 0.1, b.y)
                    .line_to(b.x + b.w * 0.1, b.bottom())
                    .move_to(b.x + b.w * 0.9, b.y)
                    .line_to(b.x + b.w * 0.9, b.bottom())
            }),
            text_area: |e| inset(e.rect, 0.1, 0., 0.8, 1.),
            ..shape!("fc-predefined", "Predefined process", "Flowchart - Predefined Process", Custom, f, 4.0, 2.0, |e| Path::rect(e.rect))
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0.15, 0., 0.7, 1.),
            ..shape!("fc-preparation", "Preparation", "Flowchart - Preparation", Custom, f, 4.0, 2.0, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0.2, 0.), pt(b, 0.8, 0.), pt(b, 1., 0.5), pt(b, 0.8, 1.), pt(b, 0.2, 1.), pt(b, 0., 0.5)])
            })
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0., 0., 0.85, 1.),
            ..shape!("fc-delay", "Delay", "Flowchart - Delay", Custom, f, 4.0, 2.0, |e| {
                let b = e.rect;
                let (rx, ry) = (b.w * 0.4, b.h / 2.0);
                let x0 = b.right() - rx;
                Path::new()
                    .move_to(b.x, b.y)
                    .line_to(x0, b.y)
                    .curve_to(x0 + rx * K, b.y, b.right(), b.cy() - ry * K, b.right(), b.cy())
                    .curve_to(b.right(), b.cy() + ry * K, x0 + rx * K, b.bottom(), x0, b.bottom())
                    .line_to(b.x, b.bottom())
                    .close()
            })
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0.15, 0., 0.7, 1.),
            ..shape!("fc-display", "Display", "Flowchart - Display", Custom, f, 4.0, 2.0, |e| {
                let b = e.rect;
                let (rx, ry) = (b.w * 0.2, b.h / 2.0);
                let x0 = b.right() - rx;
                Path::new()
                    .move_to(b.x, b.cy())
                    .line_to(b.x + b.w * 0.2, b.y)
                    .line_to(x0, b.y)
                    .curve_to(x0 + rx * K, b.y, b.right(), b.cy() - ry * K, b.right(), b.cy())
                    .curve_to(b.right(), b.cy() + ry * K, x0 + rx * K, b.bottom(), x0, b.bottom())
                    .line_to(b.x + b.w * 0.2, b.bottom())
                    .close()
            })
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0., 0., 1., 0.65),
            ..shape!("fc-offpage", "Off-page connector", "Flowchart - Off Page Connector", Custom, f, 2.0, 2.0, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0., 0.), pt(b, 1., 0.), pt(b, 1., 0.6), pt(b, 0.5, 1.), pt(b, 0., 0.6)])
            })
        },
        ShapeDef {
            details: Some(|b| {
                let (ry, rx) = (b.h * 0.12, b.w / 2.0);
                Path::new()
                    .move_to(b.x, b.y + ry)
                    .curve_to(b.x, b.y + ry + ry * K, b.cx() - rx * K, b.y + 2.0 * ry, b.cx(), b.y + 2.0 * ry)
                    .curve_to(b.cx() + rx * K, b.y + 2.0 * ry, b.right(), b.y + ry + ry * K, b.right(), b.y + ry)
            }),
            text_area: |e| inset(e.rect, 0., 0.24, 1., 0.64),
            ..shape!("fc-disk", "Stored data (disk)", "Flowchart - Magnetic Disk", Custom, f, 3.0, 2.5, |e| {
                let b = e.rect;
                let (ry, rx) = (b.h * 0.12, b.w / 2.0);
                Path::new()
                    .move_to(b.x, b.y + ry)
                    .curve_to(b.x, b.y + ry - ry * K, b.cx() - rx * K, b.y, b.cx(), b.y)
                    .curve_to(b.cx() + rx * K, b.y, b.right(), b.y + ry - ry * K, b.right(), b.y + ry)
                    .line_to(b.right(), b.bottom() - ry)
                    .curve_to(b.right(), b.bottom() - ry + ry * K, b.cx() + rx * K, b.bottom(), b.cx(), b.bottom())
                    .curve_to(b.cx() - rx * K, b.bottom(), b.x, b.bottom() - ry + ry * K, b.x, b.bottom() - ry)
                    .close()
            })
        },
        ShapeDef {
            details: Some(|b| {
                Path::new()
                    .move_to(b.x + b.w * 0.15, b.y)
                    .line_to(b.x + b.w * 0.15, b.bottom())
                    .move_to(b.x, b.y + b.h * 0.2)
                    .line_to(b.right(), b.y + b.h * 0.2)
            }),
            text_area: |e| inset(e.rect, 0.15, 0.2, 0.85, 0.8),
            ..shape!("fc-internal", "Internal storage", "Flowchart - Internal Storage", Custom, f, 3.0, 2.0, |e| Path::rect(e.rect))
        },
        shape!("fc-punched-card", "Card", "Flowchart - Punched Card", Custom, f, 4.0, 2.0, |e| {
            let b = e.rect;
            Path::polygon(&[pt(b, 0.2, 0.), pt(b, 1., 0.), pt(b, 1., 1.), pt(b, 0., 1.), pt(b, 0., 0.25)])
        }),
        ShapeDef {
            details: Some(|b| Path::new().move_to(b.cx(), b.bottom()).line_to(b.right(), b.bottom())),
            text_area: ellipse_area,
            ..shape!("fc-tape", "Sequential data (tape)", "Flowchart - Magnetic Tape", Custom, f, 2.0, 2.0, |e| Path::ellipse(e.rect))
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0.25, 0., 0.5, 0.5),
            ..shape!("fc-merge", "Merge", "Flowchart - Merge", Custom, f, 2.0, 2.0, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0., 0.), pt(b, 1., 0.), pt(b, 0.5, 1.)])
            })
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0.25, 0.5, 0.5, 0.5),
            ..shape!("fc-extract", "Extract", "Flowchart - Extract", Custom, f, 2.0, 2.0, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0.5, 0.), pt(b, 1., 1.), pt(b, 0., 1.)])
            })
        },
        ShapeDef {
            details: Some(|b| Path::new().move_to(b.x + b.w * 0.4, b.y + b.h * 0.8).line_to(b.x + b.w * 0.6, b.y + b.h * 0.8)),
            text_area: |e| inset(e.rect, 0.25, 0., 0.5, 0.5),
            ..shape!("fc-offline", "Offline storage", "Flowchart - Offline Storage", Custom, f, 2.0, 2.0, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0., 0.), pt(b, 1., 0.), pt(b, 0.5, 1.)])
            })
        },
        ShapeDef {
            details: Some(|b| Path::new().move_to(b.x, b.cy()).line_to(b.right(), b.cy())),
            text_area: diamond_area,
            ..shape!("fc-sort", "Sort", "Flowchart - Sort", Custom, f, 2.0, 2.5, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0.5, 0.), pt(b, 1., 0.5), pt(b, 0.5, 1.), pt(b, 0., 0.5)])
            })
        },
        ShapeDef {
            text_area: |e| inset(e.rect, 0.3, 0.3, 0.4, 0.4),
            ..shape!("fc-collate", "Collate", "Flowchart - Collate", Custom, f, 2.0, 2.5, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0., 0.), pt(b, 1., 0.), pt(b, 0.5, 0.5)]).append(Path::polygon(&[pt(b, 0.5, 0.5), pt(b, 1., 1.), pt(b, 0., 1.)]))
            })
        },
        ShapeDef {
            details: Some(|b| Path::new().move_to(b.cx(), b.y).line_to(b.cx(), b.bottom()).move_to(b.x, b.cy()).line_to(b.right(), b.cy())),
            text_area: ellipse_area,
            ..shape!("fc-or", "Or", "Flowchart - Or", Custom, f, 1.5, 1.5, |e| Path::ellipse(e.rect))
        },
        ShapeDef {
            details: Some(|b| {
                Path::new()
                    .move_to(b.x + b.w * 0.146, b.y + b.h * 0.146)
                    .line_to(b.x + b.w * 0.854, b.y + b.h * 0.854)
                    .move_to(b.x + b.w * 0.854, b.y + b.h * 0.146)
                    .line_to(b.x + b.w * 0.146, b.y + b.h * 0.854)
            }),
            text_area: ellipse_area,
            ..shape!("fc-summing", "Summing junction", "Flowchart - Summing Junction", Custom, f, 1.5, 1.5, |e| Path::ellipse(e.rect))
        },
        // ---- Basic objects: no text, like the original's Box and Ellipse tools.
        ShapeDef {
            has_text: false,
            ..shape!("std-box", "Box", "Standard - Box", StdBox, b, 3.0, 2.0, |e| Path::round_rect(e.rect, e.corner_radius))
        },
        ShapeDef {
            has_text: false,
            def_corner: 0.5,
            ..shape!("std-roundbox", "Rounded box", "Standard - Box", StdBox, b, 3.0, 2.0, |e| Path::round_rect(e.rect, e.corner_radius))
        },
        ShapeDef {
            has_text: false,
            cps: Cps::Ellipse8,
            ..shape!("std-ellipse", "Ellipse", "Standard - Ellipse", StdEllipse, b, 3.0, 2.0, |e| Path::ellipse(e.rect))
        },
        ShapeDef {
            has_text: false,
            cps: Cps::Ellipse8,
            ..shape!("std-circle", "Circle", "Standard - Ellipse", StdEllipse, b, 2.0, 2.0, |e| Path::ellipse(e.rect))
        },
        // ---- UML.
        ShapeDef { cps: Cps::Record, ..shape!("uml-class", "Class", "UML - Class", Uml, u, 3.0, 2.0, |e| Path::rect(e.rect)) },
        ShapeDef {
            details: Some(|b| {
                let f = 0.5f64.min(b.w / 3.0).min(b.h / 2.0);
                Path::new().move_to(b.right() - f, b.y).line_to(b.right() - f, b.y + f).line_to(b.right(), b.y + f)
            }),
            ..shape!("uml-note", "Note", "UML - Note", Uml, u, 4.0, 2.0, |e| {
                let b = e.rect;
                let f = 0.5f64.min(b.w / 3.0).min(b.h / 2.0);
                Path::polygon(&[p(b.x, b.y), p(b.right() - f, b.y), p(b.right(), b.y + f), p(b.right(), b.bottom()), p(b.x, b.bottom())])
            })
        },
        ShapeDef {
            cps: Cps::Box9,
            details: Some(|b| {
                Path::new()
                    .move_to(b.cx(), b.y + b.h * 0.3)
                    .line_to(b.cx(), b.y + b.h * 0.56)
                    .move_to(b.x + b.w * 0.05, b.y + b.h * 0.38)
                    .line_to(b.right() - b.w * 0.05, b.y + b.h * 0.38)
                    .move_to(b.x + b.w * 0.1, b.y + b.h * 0.78)
                    .line_to(b.cx(), b.y + b.h * 0.56)
                    .line_to(b.right() - b.w * 0.1, b.y + b.h * 0.78)
            }),
            text_area: |e| r(e.rect.x - 2.0, e.rect.y + e.rect.h * 0.8, e.rect.w + 4.0, e.rect.h * 0.2 + 0.4),
            ..shape!("uml-actor", "Actor", "UML - Actor", Uml, u, 1.4, 3.0, |e| {
                let b = e.rect;
                let rad = (b.h * 0.15).min(b.w * 0.3);
                Path::ellipse(r(b.cx() - rad, b.y + b.h * 0.3 - 2.0 * rad, 2.0 * rad, 2.0 * rad))
            })
        },
        ShapeDef { cps: Cps::Ellipse16, text_area: ellipse_area, ..shape!("uml-usecase", "Use case", "UML - Usecase", Uml, u, 4.0, 2.0, |e| Path::ellipse(e.rect)) },
        ShapeDef { ..shape!("uml-object", "Object", "UML - Object", Uml, u, 3.5, 1.6, |e| Path::rect(e.rect)) },
        ShapeDef {
            details: Some(|b| {
                Path::rect(r(b.x, b.y + b.h * 0.2, 0.8f64.min(b.w * 0.3), b.h * 0.18))
                    .append(Path::rect(r(b.x, b.y + b.h * 0.58, 0.8f64.min(b.w * 0.3), b.h * 0.18)))
            }),
            text_area: |e| r(e.rect.x + 0.4f64.min(e.rect.w * 0.15), e.rect.y, e.rect.w - 0.4f64.min(e.rect.w * 0.15), e.rect.h),
            ..shape!("uml-component", "Component", "UML - Component", Uml, u, 4.0, 2.0, |e| {
                let b = e.rect;
                let o = 0.4f64.min(b.w * 0.15);
                Path::rect(r(b.x + o, b.y, b.w - o, b.h))
            })
        },
        ShapeDef {
            text_area: |e| r(e.rect.x, e.rect.y + 0.6f64.min(e.rect.h * 0.3), e.rect.w, e.rect.h - 0.6f64.min(e.rect.h * 0.3)),
            ..shape!("uml-package", "Package", "UML - LargePackage", Uml, u, 5.0, 3.0, |e| {
                let b = e.rect;
                let t = 0.6f64.min(b.h * 0.3);
                Path::polygon(&[p(b.x, b.y), p(b.x + b.w * 0.4, b.y), p(b.x + b.w * 0.4, b.y + t), p(b.right(), b.y + t), p(b.right(), b.bottom()), p(b.x, b.bottom())])
            })
        },
        ShapeDef {
            details: Some(|b| {
                let d = 0.5f64.min(b.w * 0.2).min(b.h * 0.3);
                Path::new().move_to(b.x, b.y + d).line_to(b.right() - d, b.y + d).line_to(b.right(), b.y).move_to(b.right() - d, b.y + d).line_to(b.right() - d, b.bottom())
            }),
            text_area: |e| {
                let d = 0.5f64.min(e.rect.w * 0.2).min(e.rect.h * 0.3);
                r(e.rect.x, e.rect.y + d, e.rect.w - d, e.rect.h - d)
            },
            ..shape!("uml-node", "Node", "UML - Node", Uml, u, 4.0, 2.5, |e| {
                let b = e.rect;
                let d = 0.5f64.min(b.w * 0.2).min(b.h * 0.3);
                Path::polygon(&[p(b.x, b.y + d), p(b.x + d, b.y), p(b.right(), b.y), p(b.right(), b.bottom() - d), p(b.right() - d, b.bottom()), p(b.x, b.bottom())])
            })
        },
        ShapeDef { ..shape!("uml-state", "State", "UML - State", Uml, u, 4.0, 2.0, |e| Path::round_rect(e.rect, 0.5)) },
        ShapeDef { ..shape!("uml-activity", "Activity", "UML - Activity", Uml, u, 4.0, 1.5, |e| Path::round_rect(e.rect, e.rect.h / 2.0)) },
        ShapeDef {
            has_text: false,
            cps: Cps::Ellipse16,
            solid: Some(|b| Path::ellipse(b)),
            ..shape!("uml-initial", "Start (initial state)", "UML - State Term", Uml, u, 1.0, 1.0, |e| Path::ellipse(e.rect))
        },
        ShapeDef {
            has_text: false,
            cps: Cps::Ellipse16,
            solid: Some(|b| Path::ellipse(r(b.x + b.w * 0.22, b.y + b.h * 0.22, b.w * 0.56, b.h * 0.56))),
            ..shape!("uml-final", "End (final state)", "UML - State Term", Uml, u, 1.0, 1.0, |e| Path::ellipse(e.rect))
        },
        ShapeDef {
            has_text: false,
            cps: Cps::Diamond16,
            ..shape!("uml-branch", "Branch (decision)", "UML - Branch", Uml, u, 1.4, 1.4, |e| {
                let b = e.rect;
                Path::polygon(&[pt(b, 0.5, 0.), pt(b, 1., 0.5), pt(b, 0.5, 1.), pt(b, 0., 0.5)])
            })
        },
        ShapeDef {
            has_text: false,
            solid: Some(|b| Path::rect(b)),
            ..shape!("uml-fork", "Fork / join", "UML - Fork", Uml, u, 4.0, 0.3, |e| Path::rect(e.rect))
        },
        // ---- Database.
        ShapeDef { cps: Cps::Record, ..shape!("db-table", "Table", "Database - Table", Uml, db, 3.0, 2.0, |e| Path::rect(e.rect)) },
        // ---- Placeholder for shapes from files that we don't know.
        ShapeDef { cps: Cps::Box9, ..shape!("unknown", "Unknown shape", "", Unknown, "", 2.0, 2.0, |e| Path::rect(e.rect)) },
    ]
}

pub fn all() -> &'static [ShapeDef] {
    static S: OnceLock<Vec<ShapeDef>> = OnceLock::new();
    S.get_or_init(build)
}

pub fn def(i: usize) -> &'static ShapeDef {
    let a = all();
    a.get(i).unwrap_or(&a[a.len() - 1])
}

pub fn index_of(key: &str) -> usize {
    all().iter().position(|d| d.key == key).unwrap_or(all().len() - 1)
}

pub fn unknown() -> usize {
    all().len() - 1
}

pub fn by_type(dia_type: &str) -> Option<usize> {
    all().iter().position(|d| d.fam != Family::Unknown && d.dia_type.eq_ignore_ascii_case(dia_type))
}

pub fn center_index(e: &Element) -> usize {
    connection_points(e).len() - 1
}
