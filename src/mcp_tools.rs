//! What the MCP tools do to diagrams. Every change is saved to the .dia file straight away.

use crate::dia;
use crate::geom::*;
use crate::model::*;
use crate::record::{ClassAttr, ClassOp, Column, Param, RecKind, Record};
use crate::shapes;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::time::SystemTime;

#[derive(Default)]
pub struct Session {
    /// Diagrams in use, with the file time of our last save (to notice outside changes).
    docs: HashMap<PathBuf, (Diagram, Option<SystemTime>)>,
}

type Out = Result<Vec<Value>, String>;

fn text(s: impl Into<String>) -> Vec<Value> {
    vec![json!({ "type": "text", "text": s.into() })]
}

fn mtime(p: &FsPath) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn arg_s<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty())
}
fn arg_f(a: &Value, k: &str) -> Option<f64> {
    a.get(k).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())))
}
fn arg_b(a: &Value, k: &str) -> Option<bool> {
    a.get(k).and_then(|v| v.as_bool())
}

fn path_arg(a: &Value) -> Result<PathBuf, String> {
    let s = arg_s(a, "path").ok_or("Please give the diagram's path (a .dia file).")?;
    let mut p = PathBuf::from(s.trim());
    if p.extension().is_none() {
        p.set_extension("dia");
    }
    if p.is_relative() {
        p = std::env::current_dir().map_err(|e| e.to_string())?.join(p);
    }
    Ok(p)
}

fn colour(a: &Value, k: &str) -> Result<Option<Col>, String> {
    let Some(s) = arg_s(a, k) else { return Ok(None) };
    let named = match s.trim().to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
        "black" => Some(Col::rgb(0, 0, 0)),
        "white" => Some(Col::rgb(255, 255, 255)),
        "red" => Some(Col::rgb(220, 50, 47)),
        "green" => Some(Col::rgb(60, 160, 70)),
        "blue" => Some(Col::rgb(50, 100, 200)),
        "yellow" => Some(Col::rgb(255, 220, 60)),
        "orange" => Some(Col::rgb(245, 150, 40)),
        "purple" => Some(Col::rgb(140, 80, 180)),
        "pink" => Some(Col::rgb(250, 200, 215)),
        "grey" | "gray" => Some(Col::rgb(128, 128, 128)),
        "lightgrey" | "lightgray" => Some(Col::rgb(225, 225, 225)),
        "lightblue" => Some(Col::rgb(215, 230, 250)),
        "lightgreen" => Some(Col::rgb(215, 240, 208)),
        "lightyellow" => Some(Col::rgb(255, 248, 205)),
        "lightred" => Some(Col::rgb(250, 220, 220)),
        _ => None,
    };
    named.or_else(|| Col::parse(s)).map(Some).ok_or_else(|| format!("Unknown colour \"{s}\"; use #rrggbb or a colour name."))
}

fn norm(s: &str) -> String {
    s.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// A shape kind from a friendly name, a key ("fc-box"), a menu name ("Process") or a file type.
fn find_kind(kind: &str) -> Option<usize> {
    let k = norm(kind);
    let alias = match k.as_str() {
        "rectangle" | "rect" | "step" | "action" | "box" | "task" => Some("fc-box"),
        "decision" | "diamond" | "if" | "question" | "condition" | "choice" => Some("fc-diamond"),
        "start" | "end" | "stop" | "begin" | "terminator" | "startend" | "terminal" => Some("fc-terminal"),
        "io" | "data" | "input" | "output" | "inputoutput" | "parallelogram" => Some("fc-parallelogram"),
        "database" | "db" | "disk" | "storage" | "storeddata" | "cylinder" => Some("fc-disk"),
        "document" | "doc" | "report" => Some("fc-document"),
        "subprocess" | "predefined" | "predefinedprocess" | "subroutine" => Some("fc-predefined"),
        "circle" | "connector" | "ellipse" | "oval" => Some("fc-ellipse"),
        "note" | "comment" => Some("uml-note"),
        "actor" | "user" | "person" => Some("uml-actor"),
        "usecase" => Some("uml-usecase"),
        "package" => Some("uml-package"),
        "component" => Some("uml-component"),
        "node" | "server" => Some("uml-node"),
        "state" => Some("uml-state"),
        "activity" => Some("uml-activity"),
        "initial" | "initialstate" => Some("uml-initial"),
        "final" | "finalstate" => Some("uml-final"),
        "fork" | "join" | "bar" => Some("uml-fork"),
        "branch" | "merge" => Some("uml-branch"),
        "plainbox" => Some("std-box"),
        "plainellipse" => Some("std-ellipse"),
        _ => None,
    };
    if let Some(key) = alias {
        return Some(shapes::index_of(key));
    }
    shapes::all().iter().position(|d| !d.sheet.is_empty() && (norm(d.key) == k || norm(d.name) == k || norm(d.dia_type) == k))
}

fn vis_from(s: &str) -> i64 {
    match s.trim().to_ascii_lowercase().as_str() {
        "private" | "-" => 1,
        "protected" | "#" => 2,
        "package" | "implementation" | "~" => 3,
        _ => 0,
    }
}

fn split_vis(s: &str) -> (i64, &str) {
    let s = s.trim();
    match s.chars().next() {
        Some(c @ ('+' | '-' | '#' | '~')) => (vis_from(&c.to_string()), s[1..].trim()),
        _ => (0, s),
    }
}

fn split_colon(s: &str) -> (String, String) {
    match s.split_once(':') {
        Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
        None => (s.trim().to_string(), String::new()),
    }
}

fn parse_attr(v: &Value) -> ClassAttr {
    if let Some(s) = v.as_str() {
        let (vis, rest) = split_vis(s);
        let (decl, value) = match rest.split_once('=') {
            Some((a, b)) => (a, b.trim().to_string()),
            None => (rest, String::new()),
        };
        let (name, ty) = split_colon(decl);
        return ClassAttr { name, ty, value, vis, ..Default::default() };
    }
    ClassAttr {
        name: arg_s(v, "name").unwrap_or("").to_string(),
        ty: arg_s(v, "type").unwrap_or("").to_string(),
        value: arg_s(v, "value").or(arg_s(v, "default")).unwrap_or("").to_string(),
        comment: arg_s(v, "comment").unwrap_or("").to_string(),
        vis: arg_s(v, "visibility").map(vis_from).unwrap_or(0),
        is_abstract: arg_b(v, "abstract").unwrap_or(false),
        class_scope: arg_b(v, "static").unwrap_or(false),
    }
}

fn parse_param(s: &str) -> Param {
    let (decl, value) = match s.split_once('=') {
        Some((a, b)) => (a, b.trim().to_string()),
        None => (s, String::new()),
    };
    let (name, ty) = split_colon(decl);
    Param { name, ty, value, ..Default::default() }
}

fn parse_op(v: &Value) -> ClassOp {
    if let Some(s) = v.as_str() {
        let (vis, rest) = split_vis(s);
        let (head, params, tail) = match (rest.find('('), rest.rfind(')')) {
            (Some(a), Some(b)) if b > a => (&rest[..a], &rest[a + 1..b], &rest[b + 1..]),
            _ => (rest, "", ""),
        };
        let ty = tail.trim().strip_prefix(':').map(|t| t.trim().to_string()).unwrap_or_default();
        let params = params.split(',').filter(|p| !p.trim().is_empty()).map(parse_param).collect();
        return ClassOp { name: head.trim().to_string(), ty, vis, inheritance: 2, params, ..Default::default() };
    }
    let params = v
        .get("params")
        .or(v.get("parameters"))
        .and_then(|p| p.as_array())
        .map(|ps| {
            ps.iter()
                .map(|p| match p.as_str() {
                    Some(s) => parse_param(s),
                    None => Param {
                        name: arg_s(p, "name").unwrap_or("").to_string(),
                        ty: arg_s(p, "type").unwrap_or("").to_string(),
                        value: arg_s(p, "default").or(arg_s(p, "value")).unwrap_or("").to_string(),
                        ..Default::default()
                    },
                })
                .collect()
        })
        .unwrap_or_default();
    ClassOp {
        name: arg_s(v, "name").unwrap_or("").to_string(),
        ty: arg_s(v, "type").or(arg_s(v, "returns")).unwrap_or("").to_string(),
        comment: arg_s(v, "comment").unwrap_or("").to_string(),
        vis: arg_s(v, "visibility").map(vis_from).unwrap_or(0),
        is_abstract: arg_b(v, "abstract").unwrap_or(false),
        class_scope: arg_b(v, "static").unwrap_or(false),
        inheritance: if arg_b(v, "abstract").unwrap_or(false) { 0 } else { 2 },
        params,
        ..Default::default()
    }
}

fn parse_column(v: &Value) -> Column {
    if let Some(s) = v.as_str() {
        let up = s.to_ascii_uppercase();
        let mut words = s.split_whitespace();
        let name = words.next().unwrap_or("").trim_end_matches(':').to_string();
        let ty = words.next().unwrap_or("").to_string();
        let pk = up.contains(" PK") || up.contains("PRIMARY KEY");
        let not_null = up.contains("NOT NULL");
        let default = up.find("DEFAULT ").map(|i| s[i + 8..].split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default();
        return Column { name, ty, pk, nullable: !(pk || not_null), unique: up.contains("UNIQUE"), default, comment: String::new() };
    }
    let pk = arg_b(v, "primary_key").or(arg_b(v, "pk")).unwrap_or(false);
    Column {
        name: arg_s(v, "name").unwrap_or("").to_string(),
        ty: arg_s(v, "type").unwrap_or("").to_string(),
        comment: arg_s(v, "comment").unwrap_or("").to_string(),
        pk,
        nullable: arg_b(v, "nullable").unwrap_or(!pk),
        unique: arg_b(v, "unique").unwrap_or(false),
        default: arg_s(v, "default").unwrap_or("").to_string(),
    }
}

fn arrow_kind(s: &str) -> Result<i64, String> {
    let k = norm(s);
    let id = match k.as_str() {
        "none" | "no" | "" => 0,
        "open" | "lines" | "line" | "simple" => 1,
        "hollowtriangle" | "hollow" | "inheritance" => 2,
        "triangle" | "filled" | "filledtriangle" | "arrow" => 3,
        "concave" | "filledconcave" | "default" => 22,
        "diamond" | "hollowdiamond" => 4,
        "filleddiamond" => 5,
        "dot" | "hollowdot" | "circle" => 9,
        "filleddot" => 8,
        "doublehollowtriangle" => 10,
        "doublefilledtriangle" | "double" => 11,
        "slashedcross" | "cross" => 7,
        _ => return Err(format!("Unknown arrowhead \"{s}\"; see list_shape_kinds.")),
    };
    Ok(id)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Side {
    Top,
    Bottom,
    Left,
    Right,
    Center,
}

fn side_from(s: &str) -> Option<Side> {
    match norm(s).as_str() {
        "top" | "up" | "north" => Some(Side::Top),
        "bottom" | "down" | "south" => Some(Side::Bottom),
        "left" | "west" => Some(Side::Left),
        "right" | "east" => Some(Side::Right),
        "center" | "centre" | "middle" => Some(Side::Center),
        _ => None,
    }
}

/// Which sides to join two boxes by. `pref` is the layout direction (down/right), if any.
fn auto_sides(a: R, b: R, pref: Option<bool>) -> (Side, Side) {
    let (dx, dy) = (b.cx() - a.cx(), b.cy() - a.cy());
    match pref {
        Some(true) => {
            if b.y >= a.bottom() - 1e-6 {
                (Side::Bottom, Side::Top)
            } else if b.bottom() <= a.y + 1e-6 {
                (Side::Right, Side::Right) // going back up: round the side
            } else if dx >= 0.0 {
                (Side::Right, Side::Left)
            } else {
                (Side::Left, Side::Right)
            }
        }
        Some(false) => {
            if b.x >= a.right() - 1e-6 {
                (Side::Right, Side::Left)
            } else if b.right() <= a.x + 1e-6 {
                (Side::Bottom, Side::Bottom)
            } else if dy >= 0.0 {
                (Side::Bottom, Side::Top)
            } else {
                (Side::Top, Side::Bottom)
            }
        }
        None => {
            if dy.abs() >= dx.abs() {
                if dy >= 0.0 {
                    (Side::Bottom, Side::Top)
                } else {
                    (Side::Top, Side::Bottom)
                }
            } else if dx >= 0.0 {
                (Side::Right, Side::Left)
            } else {
                (Side::Left, Side::Right)
            }
        }
    }
}

/// Attach to the middle of one side of a shape, on its real outline (slanted or round
/// shapes don't reach the corners of their box).
fn side_att(e: &Element, target: u64, s: Side) -> Attachment {
    let (u, v) = match s {
        Side::Top => (0.5, 0.0),
        Side::Bottom => (0.5, 1.0),
        Side::Left => (0.0, 0.5),
        Side::Right => (1.0, 0.5),
        Side::Center => return Attachment::center(target),
    };
    // Where the outline crosses the line from the middle out to that side.
    let c = e.rect.center();
    let want = e.rect.at(u, v);
    let hit = attach_point(e, &Attachment::center(target), want + (want - c));
    let (mut u, v) = ((hit.x - e.rect.x) / e.rect.w.max(1e-9), (hit.y - e.rect.y) / e.rect.h.max(1e-9));
    if (u - 0.5).abs() < 1e-9 && (v - 0.5).abs() < 1e-9 {
        u += 1e-4;
    }
    Attachment { target, u, v, cp: None }
}

/// The side an attachment is on (for routing).
fn att_side(a: &Attachment) -> Side {
    if a.is_center() {
        return Side::Center;
    }
    let d = [(a.u, Side::Left), (1.0 - a.u, Side::Right), (a.v, Side::Top), (1.0 - a.v, Side::Bottom)];
    d.iter().min_by(|x, y| x.0.partial_cmp(&y.0).unwrap()).map(|x| x.1).unwrap_or(Side::Center)
}

/// Attach to a class attribute/operation or a table column, on the left or right edge.
fn row_att(e: &Element, target: u64, row: &str, toward_x: f64, side: Option<Side>) -> Result<Attachment, String> {
    let r = e.rec.as_ref().ok_or("from_row/to_row only work with classes and tables.")?;
    let names: Vec<String> = match r.kind {
        RecKind::Class => r.attrs.iter().map(|a| a.name.clone()).chain(r.ops.iter().map(|o| o.name.clone())).collect(),
        RecKind::Table => r.cols.iter().map(|c| c.name.clone()).collect(),
    };
    let want = norm(row);
    let i = names
        .iter()
        .position(|n| norm(n) == want)
        .or_else(|| names.iter().position(|n| norm(n).trim_start_matches('$') == want.trim_start_matches('$')))
        .ok_or_else(|| format!("\"{}\" has no row called \"{row}\". Rows: {}", r.name, names.join(", ")))?;
    let right = match side {
        Some(Side::Left) => false,
        Some(Side::Right) => true,
        _ => toward_x > e.rect.cx(),
    };
    let cp = 8 + 2 * i + right as usize;
    let cps = e.cps();
    let c = cps.get(cp).copied().unwrap_or(e.rect.center());
    Ok(Attachment { target, u: (c.x - e.rect.x) / e.rect.w.max(1e-9), v: (c.y - e.rect.y) / e.rect.h.max(1e-9), cp: Some(cp) })
}

/// A sensible route between two attached ends.
fn route(kind: LineKind, a: P, sa: Side, ra: R, b: P, sb: Side, rb: R) -> Vec<P> {
    let horiz = |s: Side| matches!(s, Side::Left | Side::Right);
    match kind {
        LineKind::ZigZag => {
            if sa == sb && sa != Side::Center {
                // Both ends on the same side: go round that side.
                let m = 1.0;
                return match sa {
                    Side::Right => {
                        let x = ra.right().max(rb.right()) + m;
                        vec![a, p(x, a.y), p(x, b.y), b]
                    }
                    Side::Left => {
                        let x = ra.x.min(rb.x) - m;
                        vec![a, p(x, a.y), p(x, b.y), b]
                    }
                    Side::Bottom => {
                        let y = ra.bottom().max(rb.bottom()) + m;
                        vec![a, p(a.x, y), p(b.x, y), b]
                    }
                    _ => {
                        let y = ra.y.min(rb.y) - m;
                        vec![a, p(a.x, y), p(b.x, y), b]
                    }
                };
            }
            if horiz(sa) && !horiz(sb) && sb != Side::Center {
                return vec![a, p(b.x, a.y), b];
            }
            if !horiz(sa) && sa != Side::Center && horiz(sb) {
                return vec![a, p(a.x, b.y), b];
            }
            orth_route(a, b, !horiz(sa))
        }
        LineKind::Bezier => {
            let d = b - a;
            let k = d.len() * 0.4;
            let dir = |s: Side, sign: f64| match s {
                Side::Top => p(0.0, -k * sign),
                Side::Bottom => p(0.0, k * sign),
                Side::Left => p(-k * sign, 0.0),
                Side::Right => p(k * sign, 0.0),
                Side::Center => d * (0.33 * sign),
            };
            vec![a, a + dir(sa, 1.0), b + dir(sb, 1.0), b]
        }
        LineKind::PolyLine => vec![a, (a + b) * 0.5, b],
        _ => vec![a, b],
    }
}

/// Lay a line out again between its two shapes.
fn reroute(d: &mut Diagram, line_id: u64, pref: Option<bool>) {
    let Some(Kind::Line(l)) = d.get(line_id).map(|o| &o.kind) else { return };
    let (Some(sa), Some(sb)) = (l.start_att, l.end_att) else { return };
    let (Some(ea), Some(eb)) = (d.element(sa.target).map(|e| e.geometry_copy()), d.element(sb.target).map(|e| e.geometry_copy())) else { return };
    let kind = l.kind;
    // Side attachments (not rows) are chosen again for the new positions.
    let (mut na, mut nb) = (sa, sb);
    if sa.cp.is_none() && sb.cp.is_none() && sa.target != sb.target {
        let (s1, s2) = auto_sides(ea.rect, eb.rect, pref);
        if !(att_side(&sa) == Side::Center && att_side(&sb) == Side::Center) {
            na = side_att(&ea, sa.target, s1);
            nb = side_att(&eb, sb.target, s2);
        }
    }
    let a = attach_point(&ea, &na, eb.rect.center());
    let b = attach_point(&eb, &nb, ea.rect.center());
    let pts = route(kind, a, side_for(&na, &ea, b), ea.rect, b, side_for(&nb, &eb, a), eb.rect);
    if let Some(Kind::Line(l)) = d.get_mut(line_id).map(|o| &mut o.kind) {
        l.start_att = Some(na);
        l.end_att = Some(nb);
        l.pts = pts;
        if l.kind == LineKind::Arc && l.curve.abs() < 1e-9 {
            l.curve = a.dist(b) * 0.2;
        }
    }
}

/// Which way a line leaves an attachment (rows leave sideways).
fn side_for(at: &Attachment, e: &Element, other: P) -> Side {
    if at.cp.is_some() {
        return if at.u > 0.5 { Side::Right } else { Side::Left };
    }
    match att_side(at) {
        Side::Center => {
            let c = e.rect.center();
            if (other.y - c.y).abs() >= (other.x - c.x).abs() {
                if other.y > c.y { Side::Bottom } else { Side::Top }
            } else if other.x > c.x {
                Side::Right
            } else {
                Side::Left
            }
        }
        s => s,
    }
}

/// Where to put a new object when no position is given: to the right of everything.
fn next_spot(d: &Diagram) -> P {
    match d.extents() {
        Some(e) => p(e.right() + 1.5, e.y.max(1.0)),
        None => p(1.0, 1.0),
    }
}

/// An object from an id or its text / name.
fn find_obj(d: &Diagram, v: Option<&Value>) -> Result<u64, String> {
    let v = v.ok_or("Say which object (its id, or its text).")?;
    if let Some(n) = v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().trim_start_matches('#').parse().ok())) {
        if d.get(n).is_some() {
            return Ok(n);
        }
        return Err(format!("There's no object #{n}. Use list_objects to see the ids."));
    }
    let want = norm(v.as_str().unwrap_or(""));
    let label = |o: &Obj| -> String {
        match &o.kind {
            Kind::Element(e) => e.rec.as_ref().map(|r| r.name.clone()).or_else(|| e.text.as_ref().map(|t| t.text.clone())).unwrap_or_default(),
            Kind::Text(t) => t.text.text.clone(),
            _ => String::new(),
        }
    };
    let hits: Vec<u64> = d.objects.iter().filter(|o| matches!(o.kind, Kind::Element(_)) && norm(&label(o)) == want).map(|o| o.id).collect();
    match hits.len() {
        1 => Ok(hits[0]),
        0 => Err(format!("No shape has the text \"{}\". Use list_objects to see what's there.", v.as_str().unwrap_or(""))),
        _ => Err(format!("Several shapes say \"{}\" (ids {:?}); use an id.", v.as_str().unwrap_or(""), hits)),
    }
}

fn describe(o: &Obj, d: &Diagram) -> Value {
    let b = o.bounds();
    let round = |v: f64| (v * 100.0).round() / 100.0;
    match &o.kind {
        Kind::Element(e) => {
            let rb = e.rect;
            let mut v = json!({ "id": o.id, "x": round(rb.x), "y": round(rb.y), "width": round(rb.w), "height": round(rb.h) });
            match &e.rec {
                Some(r) if r.kind == RecKind::Class => {
                    v["type"] = json!("class");
                    v["name"] = json!(r.name);
                    v["attributes"] = json!(r.attrs.iter().map(crate::record::attr_text).collect::<Vec<_>>());
                    v["operations"] = json!(r.ops.iter().map(|op| crate::record::op_lines(op, false, 0).join("")).collect::<Vec<_>>());
                }
                Some(r) => {
                    v["type"] = json!("table");
                    v["name"] = json!(r.name);
                    v["columns"] = json!(r.cols.iter().map(crate::record::column_display).collect::<Vec<_>>());
                }
                None => {
                    v["type"] = json!("shape");
                    v["kind"] = json!(e.shape().name);
                    if is_unknown(e) {
                        v["kind"] = json!(e.raw_type);
                    }
                    if let Some(t) = &e.text {
                        v["text"] = json!(t.text);
                    }
                }
            }
            v
        }
        Kind::Text(t) => json!({ "id": o.id, "type": "text", "text": t.text.text, "x": round(b.x), "y": round(b.y) }),
        Kind::Line(l) => {
            let end = |a: &Option<Attachment>| -> Value {
                match a {
                    None => Value::Null,
                    Some(a) => {
                        let mut v = json!({ "id": a.target });
                        if let (Some(cp), Some(e)) = (a.cp, d.element(a.target)) {
                            if let Some(r) = &e.rec {
                                let names: Vec<String> = match r.kind {
                                    RecKind::Class => r.attrs.iter().map(|x| x.name.clone()).chain(r.ops.iter().map(|x| x.name.clone())).collect(),
                                    RecKind::Table => r.cols.iter().map(|c| c.name.clone()).collect(),
                                };
                                if cp >= 8 {
                                    if let Some(n) = names.get((cp - 8) / 2) {
                                        v["row"] = json!(n);
                                    }
                                }
                            }
                        }
                        v
                    }
                }
            };
            let mut v = json!({ "id": o.id, "type": "line", "from": end(&l.start_att), "to": end(&l.end_att),
                "start_arrow": arrow_name(l.start_arrow.kind), "end_arrow": arrow_name(l.end_arrow.kind) });
            if !l.label.is_empty() {
                v["label"] = json!(l.label);
            }
            v
        }
        Kind::Polygon(_) => json!({ "id": o.id, "type": "polygon", "x": round(b.x), "y": round(b.y), "width": round(b.w), "height": round(b.h) }),
        Kind::Image(_) => json!({ "id": o.id, "type": "picture", "x": round(b.x), "y": round(b.y), "width": round(b.w), "height": round(b.h) }),
        Kind::Raw(r) => json!({ "id": o.id, "type": "other", "what": r.type_name }),
    }
}

/// Arrange shapes in layers following the lines (a simple layered layout).
fn layout(d: &mut Diagram, down: bool, gap: f64) {
    let nodes: Vec<u64> = d.objects.iter().filter(|o| matches!(&o.kind, Kind::Element(e) if !is_unknown(e))).map(|o| o.id).collect();
    if nodes.is_empty() {
        return;
    }
    let mut edges: Vec<(u64, u64)> = vec![];
    for o in &d.objects {
        if let Kind::Line(l) = &o.kind {
            if let (Some(a), Some(b)) = (l.start_att, l.end_att) {
                if a.target != b.target && nodes.contains(&a.target) && nodes.contains(&b.target) {
                    edges.push((a.target, b.target));
                }
            }
        }
    }
    // Visit in reading order; edges back to something still being visited are loops, left out.
    let pos = |d: &Diagram, id: u64| d.element(id).map(|e| (e.rect.y, e.rect.x)).unwrap_or_default();
    let mut order = nodes.clone();
    order.sort_by(|a, b| pos(d, *a).partial_cmp(&pos(d, *b)).unwrap());
    let has_in: Vec<u64> = edges.iter().map(|e| e.1).collect();
    let mut roots: Vec<u64> = order.iter().copied().filter(|n| !has_in.contains(n)).collect();
    roots.extend(order.iter().copied().filter(|n| has_in.contains(n)));
    let mut state: HashMap<u64, u8> = HashMap::new(); // 1 visiting, 2 done
    let mut forward: Vec<(u64, u64)> = vec![];
    fn visit(n: u64, edges: &[(u64, u64)], state: &mut HashMap<u64, u8>, forward: &mut Vec<(u64, u64)>) {
        state.insert(n, 1);
        for &(a, b) in edges.iter().filter(|e| e.0 == n) {
            match state.get(&b) {
                Some(1) => {} // a loop back
                Some(2) => forward.push((a, b)),
                _ => {
                    forward.push((a, b));
                    visit(b, edges, state, forward);
                }
            }
        }
        state.insert(n, 2);
    }
    for r in roots {
        if !state.contains_key(&r) {
            visit(r, &edges, &mut state, &mut forward);
        }
    }
    // Rank = longest path from a root.
    let mut rank: HashMap<u64, usize> = nodes.iter().map(|n| (*n, 0)).collect();
    for _ in 0..nodes.len() {
        let mut changed = false;
        for &(a, b) in &forward {
            let ra = rank[&a];
            if rank[&b] < ra + 1 {
                rank.insert(b, ra + 1);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let max_rank = rank.values().copied().max().unwrap_or(0);
    let mut layers: Vec<Vec<u64>> = vec![vec![]; max_rank + 1];
    for n in &order {
        layers[rank[n]].push(*n);
    }
    // Order each layer by where its parents sit (one sweep), to cut crossings.
    let mut place: HashMap<u64, f64> = HashMap::new();
    for layer in layers.iter_mut() {
        let key = |n: &u64| {
            let parents: Vec<f64> = forward.iter().filter(|e| e.1 == *n).filter_map(|e| place.get(&e.0).copied()).collect();
            if parents.is_empty() { f64::MAX } else { parents.iter().sum::<f64>() / parents.len() as f64 }
        };
        let mut keyed: Vec<(f64, usize, u64)> = layer.iter().enumerate().map(|(i, n)| (key(n), i, *n)).collect();
        keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
        *layer = keyed.into_iter().map(|k| k.2).collect();
        for (i, n) in layer.iter().enumerate() {
            place.insert(*n, i as f64);
        }
    }
    let size = |d: &Diagram, n: u64| d.element(n).map(|e| (e.rect.w, e.rect.h)).unwrap_or((2.0, 2.0));
    let along = |s: (f64, f64)| if down { s.0 } else { s.1 }; // within a layer
    let across = |s: (f64, f64)| if down { s.1 } else { s.0 }; // layer thickness
    let widths: Vec<f64> = layers.iter().map(|l| l.iter().map(|n| along(size(d, *n))).sum::<f64>() + gap * (l.len().max(1) - 1) as f64).collect();
    let widest = widths.iter().copied().fold(0.0, f64::max);
    let mut depth = 1.0;
    for (li, layer) in layers.iter().enumerate() {
        let thick = layer.iter().map(|n| across(size(d, *n))).fold(0.0, f64::max);
        let mut at = 1.0 + (widest - widths[li]) / 2.0;
        for n in layer {
            let s = size(d, *n);
            let (x, y) = if down { (at, depth + (thick - s.1) / 2.0) } else { (depth + (thick - s.0) / 2.0, at) };
            if let Some(Kind::Element(e)) = d.get_mut(*n).map(|o| &mut o.kind) {
                e.rect.x = x;
                e.rect.y = y;
            }
            at += along(s) + gap;
        }
        depth += thick + gap * 1.3;
    }
    // Free text that labelled lines would now be out of place; leave it, but tidy the lines.
    let lines: Vec<u64> = d.objects.iter().filter(|o| matches!(o.kind, Kind::Line(_))).map(|o| o.id).collect();
    for l in lines {
        reroute(d, l, Some(down));
    }
    d.update_connections();
}

impl Session {
    fn diagram(&mut self, p: &FsPath) -> Result<&mut Diagram, String> {
        let disk = mtime(p);
        let stale = match self.docs.get(p) {
            Some((_, t)) => disk.is_some() && disk != *t,
            None => true,
        };
        if stale {
            let d = if p.exists() { dia::load(p)? } else { Diagram::default() };
            self.docs.insert(p.to_path_buf(), (d, disk));
        }
        Ok(&mut self.docs.get_mut(p).unwrap().0)
    }

    fn save(&mut self, p: &FsPath) -> Result<(), String> {
        let (d, t) = self.docs.get_mut(p).ok_or("Nothing to save.")?;
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        dia::save(d, p, false)?;
        *t = mtime(p);
        Ok(())
    }

    pub fn call(&mut self, name: &str, a: &Value) -> Out {
        match name {
            "list_shape_kinds" => Ok(text(kinds_text())),
            "new_diagram" => {
                let p = path_arg(a)?;
                if p.exists() && !arg_b(a, "overwrite").unwrap_or(false) {
                    return Err(format!("{} already exists. Add to it with the other tools, or set overwrite to replace it.", p.display()));
                }
                let mut d = Diagram::default();
                d.paper.portrait = !arg_b(a, "landscape").unwrap_or(false);
                if let Some(pp) = arg_s(a, "paper") {
                    d.paper.name = pp.to_string();
                }
                self.docs.insert(p.clone(), (d, None));
                self.save(&p)?;
                Ok(text(format!("Created {}.", p.display())))
            }
            "add_shape" => {
                let p = path_arg(a)?;
                let kind = arg_s(a, "kind").unwrap_or("process");
                let def = find_kind(kind).ok_or_else(|| format!("Unknown shape kind \"{kind}\". See list_shape_kinds."))?;
                if matches!(shapes::def(def).key, "uml-class" | "db-table") {
                    return Err("Use add_class or add_table for classes and tables.".into());
                }
                let (fill, line, tcol) = (colour(a, "fill")?, colour(a, "line_color")?, colour(a, "text_color")?);
                let d = self.diagram(&p)?;
                let sd = shapes::def(def);
                let spot = next_spot(d);
                let (x, y) = (arg_f(a, "x").unwrap_or(spot.x), arg_f(a, "y").unwrap_or(spot.y));
                let mut e = Element::new(def, r(x, y, sd.def_w, sd.def_h));
                if let Some(t) = &mut e.text {
                    t.text = arg_s(a, "text").unwrap_or("").replace("\\n", "\n");
                    if let Some(h) = arg_f(a, "text_size") {
                        t.height = h.clamp(0.1, 20.0);
                    }
                    if let Some(c) = tcol {
                        t.color = c;
                    }
                }
                if let Some(c) = fill {
                    e.fill_color = c;
                }
                if let Some(c) = line {
                    e.line_color = c;
                }
                if let Some(w) = arg_f(a, "width") {
                    e.rect.w = w.max(0.2);
                }
                if let Some(h) = arg_f(a, "height") {
                    e.rect.h = h.max(0.2);
                }
                e.fit_text();
                e.rect.x = x;
                e.rect.y = y;
                let rect = e.rect;
                let id = d.add(Kind::Element(e));
                self.save(&p)?;
                Ok(text(format!("Added {} #{id} at ({:.1}, {:.1}), {:.1} x {:.1} cm.", shapes::def(def).name, rect.x, rect.y, rect.w, rect.h)))
            }
            "add_text" => {
                let p = path_arg(a)?;
                let col = colour(a, "color")?;
                let d = self.diagram(&p)?;
                let spot = next_spot(d);
                let mut t = TextSpec { text: arg_s(a, "text").unwrap_or("").replace("\\n", "\n"), align: 0, ..Default::default() };
                if let Some(h) = arg_f(a, "size") {
                    t.height = h.clamp(0.1, 20.0);
                }
                if let Some(c) = col {
                    t.color = c;
                }
                t.bold = arg_b(a, "bold").unwrap_or(false);
                t.align = match arg_s(a, "align").map(norm).as_deref() {
                    Some("center") | Some("centre") => 1,
                    Some("right") => 2,
                    _ => 0,
                };
                let (x, y) = (arg_f(a, "x").unwrap_or(spot.x), arg_f(a, "y").unwrap_or(spot.y));
                let pos = crate::geom::p(x, y + t.height * ASCENT_FACTOR);
                let id = d.add(Kind::Text(TextObj { pos, text: t }));
                self.save(&p)?;
                Ok(text(format!("Added text #{id}.")))
            }
            "add_class" | "add_table" => {
                let p = path_arg(a)?;
                let fill = colour(a, "fill")?;
                let is_class = name == "add_class";
                let mut rec = if is_class { Record::class() } else { Record::table() };
                rec.name = arg_s(a, "name").ok_or("Please give a name.")?.to_string();
                if is_class {
                    rec.stereotype = arg_s(a, "stereotype").unwrap_or("").to_string();
                    rec.is_abstract = arg_b(a, "abstract").unwrap_or(false);
                    rec.show_comment = arg_b(a, "show_comments").unwrap_or(true);
                    rec.attrs = a.get("attributes").and_then(|v| v.as_array()).map(|v| v.iter().map(parse_attr).collect()).unwrap_or_default();
                    rec.ops = a.get("operations").and_then(|v| v.as_array()).map(|v| v.iter().map(parse_op).collect()).unwrap_or_default();
                } else {
                    rec.comment = arg_s(a, "comment").unwrap_or("").to_string();
                    rec.cols = a.get("columns").and_then(|v| v.as_array()).map(|v| v.iter().map(parse_column).collect()).unwrap_or_default();
                }
                let d = self.diagram(&p)?;
                let spot = next_spot(d);
                let (x, y) = (arg_f(a, "x").unwrap_or(spot.x), arg_f(a, "y").unwrap_or(spot.y));
                let mut e = Element::new(shapes::index_of(if is_class { "uml-class" } else { "db-table" }), r(x, y, 2.0, 2.0));
                e.rec = Some(rec);
                if let Some(c) = fill {
                    e.fill_color = c;
                }
                crate::record::fit(&mut e);
                let rect = e.rect;
                let id = d.add(Kind::Element(e));
                self.save(&p)?;
                Ok(text(format!("Added {} #{id} at ({:.1}, {:.1}), {:.1} x {:.1} cm.", if is_class { "class" } else { "table" }, rect.x, rect.y, rect.w, rect.h)))
            }
            "connect" => {
                let p = path_arg(a)?;
                let (col, start_arg, end_arg) = (colour(a, "color")?, arg_s(a, "start_arrow").map(arrow_kind).transpose()?, arg_s(a, "end_arrow").map(arrow_kind).transpose()?);
                let d = self.diagram(&p)?;
                let from = find_obj(d, a.get("from"))?;
                let to = find_obj(d, a.get("to"))?;
                let (Some(ea), Some(eb)) = (d.element(from).map(|e| e.clone()), d.element(to).map(|e| e.clone())) else {
                    return Err("Lines can only join shapes, classes and tables.".into());
                };
                let kind = match arg_s(a, "line").map(norm).as_deref() {
                    None | Some("zigzag") | Some("orthogonal") | Some("elbow") => LineKind::ZigZag,
                    Some("straight") | Some("line") | Some("direct") => LineKind::Line,
                    Some("curved") | Some("curve") | Some("bezier") => LineKind::Bezier,
                    Some("arc") => LineKind::Arc,
                    Some("polyline") => LineKind::PolyLine,
                    Some(other) => return Err(format!("Unknown line style \"{other}\" (zigzag, straight, curved, arc, polyline).")),
                };
                let (auto_a, auto_b) = auto_sides(ea.rect, eb.rect, None);
                let side_a = arg_s(a, "from_side").and_then(side_from);
                let side_b = arg_s(a, "to_side").and_then(side_from);
                let att_a = match arg_s(a, "from_row") {
                    Some(row) => row_att(&ea, from, row, eb.rect.cx(), side_a)?,
                    None => side_att(&ea, from, side_a.unwrap_or(auto_a)),
                };
                let att_b = match arg_s(a, "to_row") {
                    Some(row) => row_att(&eb, to, row, ea.rect.cx(), side_b)?,
                    None => side_att(&eb, to, side_b.unwrap_or(auto_b)),
                };
                let pa = attach_point(&ea, &att_a, eb.rect.center());
                let pb = attach_point(&eb, &att_b, ea.rect.center());
                let pts = route(kind, pa, side_for(&att_a, &ea, pb), ea.rect, pb, side_for(&att_b, &eb, pa), eb.rect);
                let mut l = LineObj::new(kind, pts);
                l.start_att = Some(att_a);
                l.end_att = Some(att_b);
                l.end_arrow.kind = 22;
                if kind == LineKind::Arc {
                    l.curve = pa.dist(pb) * 0.2;
                }
                if let Some(rel) = arg_s(a, "relation") {
                    let want = norm(rel);
                    let want = match want.as_str() {
                        "directed" => "directedassociation".to_string(),
                        "inheritance" | "extends" => "generalization".to_string(),
                        "implements" => "realization".to_string(),
                        "foreignkey" | "fk" => "reference".to_string(),
                        _ => want,
                    };
                    let pr = shapes::LINE_PRESETS
                        .iter()
                        .find(|pr| norm(pr.name).starts_with(&want))
                        .ok_or_else(|| format!("Unknown relation \"{rel}\" (association, directed, generalization, realization, dependency, aggregation, composition, reference, one_to_many)."))?;
                    l.start_arrow.kind = pr.start;
                    l.end_arrow.kind = pr.end;
                    if pr.dashed {
                        l.style = LineStyle::Dashed;
                        l.dash = 0.4;
                    }
                    if pr.start == 4 || pr.start == 5 {
                        l.start_arrow.length = 0.8;
                    }
                }
                if let Some(k) = start_arg {
                    l.start_arrow.kind = k;
                }
                if let Some(k) = end_arg {
                    l.end_arrow.kind = k;
                }
                if arg_b(a, "dashed").unwrap_or(false) {
                    l.style = LineStyle::Dashed;
                    l.dash = 0.4;
                }
                if let Some(c) = col {
                    l.color = c;
                }
                if let Some(w) = arg_f(a, "width") {
                    l.width = w.clamp(0.01, 5.0);
                }
                l.label = arg_s(a, "label").unwrap_or("").to_string();
                let id = d.add(Kind::Line(l));
                d.update_connections();
                self.save(&p)?;
                Ok(text(format!("Connected #{from} to #{to} with line #{id}.")))
            }
            "update_object" => {
                let p = path_arg(a)?;
                let (fill, line, tcol) = (colour(a, "fill")?, colour(a, "line_color")?, colour(a, "text_color")?);
                let d = self.diagram(&p)?;
                let id = find_obj(d, a.get("id"))?;
                let new_text = arg_s(a, "text").map(|s| s.replace("\\n", "\n"));
                let o = d.get_mut(id).unwrap();
                if let (Some(x), Some(y)) = (arg_f(a, "x").or(Some(o.align_box().x)), arg_f(a, "y").or(Some(o.align_box().y))) {
                    let b = o.align_box();
                    if (x - b.x).abs() > 1e-9 || (y - b.y).abs() > 1e-9 {
                        o.move_by(x - b.x, y - b.y);
                    }
                }
                match &mut o.kind {
                    Kind::Element(e) => {
                        if let Some(t) = &new_text {
                            match (&mut e.rec, &mut e.text) {
                                (Some(r), _) => r.name = t.clone(),
                                (None, Some(tx)) => tx.text = t.clone(),
                                _ => {}
                            }
                        }
                        if let Some(w) = arg_f(a, "width") {
                            e.rect.w = w.max(0.2);
                        }
                        if let Some(h) = arg_f(a, "height") {
                            e.rect.h = h.max(0.2);
                        }
                        if let Some(c) = fill {
                            e.fill_color = c;
                        }
                        if let Some(c) = line {
                            e.line_color = c;
                        }
                        if let (Some(c), Some(t)) = (tcol, &mut e.text) {
                            t.color = c;
                        }
                        let corner = crate::geom::p(e.rect.x, e.rect.y);
                        e.fit_text();
                        if e.rec.is_none() {
                            e.rect.x = corner.x;
                            e.rect.y = corner.y;
                        }
                    }
                    Kind::Text(t) => {
                        if let Some(s) = new_text {
                            t.text.text = s;
                        }
                        if let Some(c) = tcol.or(line) {
                            t.text.color = c;
                        }
                    }
                    Kind::Line(l) => {
                        if let Some(s) = new_text {
                            l.label = s;
                        }
                        if let Some(c) = line {
                            l.color = c;
                        }
                    }
                    _ => {}
                }
                d.update_connections();
                let desc = describe(d.get(id).unwrap(), d);
                self.save(&p)?;
                Ok(text(format!("Updated #{id}: {desc}")))
            }
            "delete_objects" => {
                let p = path_arg(a)?;
                let ids: Vec<u64> = a.get("ids").and_then(|v| v.as_array()).map(|v| v.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default();
                let d = self.diagram(&p)?;
                let before = d.objects.len();
                d.objects.retain(|o| !ids.contains(&o.id));
                let gone = before - d.objects.len();
                d.update_connections();
                self.save(&p)?;
                Ok(text(format!("Deleted {gone} object(s).")))
            }
            "list_objects" => {
                let p = path_arg(a)?;
                if !p.exists() && !self.docs.contains_key(&p) {
                    return Err(format!("{} doesn't exist yet.", p.display()));
                }
                let d = self.diagram(&p)?;
                let list: Vec<Value> = d.objects.iter().map(|o| describe(o, d)).collect();
                let ext = d.extents().unwrap_or_default();
                let head = format!("{} object(s); drawing spans {:.1} x {:.1} cm.\n", list.len(), ext.w, ext.h);
                Ok(text(head + &serde_json::to_string_pretty(&list).unwrap_or_default()))
            }
            "layout" => {
                let p = path_arg(a)?;
                let down = !matches!(arg_s(a, "direction").map(norm).as_deref(), Some("right") | Some("lefttoright") | Some("horizontal"));
                let gap = arg_f(a, "gap").unwrap_or(1.5).clamp(0.2, 20.0);
                let d = self.diagram(&p)?;
                layout(d, down, gap);
                let ext = d.extents().unwrap_or_default();
                self.save(&p)?;
                Ok(text(format!("Laid out; the drawing is now {:.1} x {:.1} cm.", ext.w, ext.h)))
            }
            "render_preview" => {
                let p = path_arg(a)?;
                let max = a.get("max_size").and_then(|v| v.as_u64()).unwrap_or(1200).clamp(200, 4000) as u32;
                let d = self.diagram(&p)?;
                let png = crate::skia::png_bytes(d, max)?;
                let ext = d.extents().unwrap_or_default();
                Ok(vec![
                    json!({ "type": "image", "data": dia::base64_encode(&png), "mimeType": "image/png" }),
                    json!({ "type": "text", "text": format!("{} object(s), {:.1} x {:.1} cm.", d.objects.len(), ext.w, ext.h) }),
                ])
            }
            "export" => {
                let p = path_arg(a)?;
                let fmt = arg_s(a, "format").unwrap_or("pdf").to_ascii_lowercase();
                let out = arg_s(a, "output").map(PathBuf::from).unwrap_or_else(|| p.with_extension(&fmt));
                let fit = matches!(arg_s(a, "page").map(norm).as_deref(), Some("fit"));
                let d = self.diagram(&p)?;
                match fmt.as_str() {
                    "pdf" => crate::pdf::export(d, &out, if fit { crate::pdf::PageMode::FitToPaper } else { crate::pdf::PageMode::DiagramSize })?,
                    "png" => crate::skia::export_png(d, &out, 40.0)?,
                    "svg" => crate::svg::export(d, &out)?,
                    _ => return Err("Format must be pdf, png or svg.".into()),
                }
                Ok(text(format!("Exported {}.", out.display())))
            }
            _ => Err(format!("Unknown tool \"{name}\".")),
        }
    }
}

fn kinds_text() -> String {
    let mut s = String::from("Shape kinds for add_shape (any of the names in brackets work too):\n");
    for sheet in shapes::SHEETS {
        s += &format!("\n{sheet}:\n");
        for d in shapes::all().iter().filter(|d| d.sheet == *sheet && !matches!(d.key, "uml-class" | "db-table")) {
            s += &format!("  - {} [{}] {:.1} x {:.1} cm{}\n", d.name, d.key, d.def_w, d.def_h, if d.has_text { "" } else { " (no text)" });
        }
    }
    s += "\nHandy names: process, decision, terminal (start/end), input/output, document, database, subprocess, note, actor, use case, state, activity, initial, final, fork, branch.\n";
    s += "\nClasses: add_class. Database tables: add_table.\n";
    s += "\nLines (connect, line=): zigzag (default), straight, curved, arc, polyline.\n";
    s += "Arrowheads: none, open, triangle, hollow_triangle, concave (default end), diamond, filled_diamond, dot, filled_dot, double, cross.\n";
    s += "Relations (connect, relation=): association, directed, generalization, realization, dependency, aggregation, composition, reference, one_to_many.\n";
    s += "Colours: #rrggbb or red, green, blue, yellow, orange, purple, pink, grey, white, black, lightblue, lightgreen, lightyellow, lightgrey, lightred.\n";
    s
}
