//! Reading and writing .dia diagram files (XML, optionally gzip-compressed).

use crate::geom::*;
use crate::model::*;
use crate::record::{ClassAttr, ClassOp, Column, Param, RecKind, Record};
use crate::shapes::{self, Family};
use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;

pub const NS: &str = "http://www.lysator.liu.se/~alla/dia/";

/// A small XML element tree (names keep their "dia:" prefix).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: String,
}

impl Node {
    pub fn new(name: &str) -> Node {
        Node { name: name.to_string(), ..Default::default() }
    }
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }
    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }
    pub fn set_attr(&mut self, k: &str, v: &str) {
        if let Some(a) = self.attrs.iter_mut().find(|(a, _)| a == k) {
            a.1 = v.to_string();
        } else {
            self.attrs.push((k.to_string(), v.to_string()));
        }
    }
    pub fn with(mut self, k: &str, v: impl ToString) -> Node {
        self.set_attr(k, &v.to_string());
        self
    }
    pub fn child(mut self, c: Node) -> Node {
        self.children.push(c);
        self
    }
    pub fn kids<'a>(&'a self, local: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.local() == local)
    }
    pub fn kid(&self, local: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.local() == local)
    }
    fn first_descendant(&self, local: &str) -> Option<&Node> {
        for c in &self.children {
            if c.local() == local {
                return Some(c);
            }
            if let Some(d) = c.first_descendant(local) {
                return Some(d);
            }
        }
        None
    }
}

fn convert(n: roxmltree::Node) -> Node {
    let tag = n.tag_name();
    let name = if tag.namespace() == Some(NS) { format!("dia:{}", tag.name()) } else { tag.name().to_string() };
    let mut out = Node::new(&name);
    for a in n.attributes() {
        out.attrs.push((a.name().to_string(), a.value().to_string()));
    }
    for c in n.children() {
        if c.is_element() {
            out.children.push(convert(c));
        } else if c.is_text() {
            out.text.push_str(c.text().unwrap_or(""));
        }
    }
    out
}

fn esc(s: &str, attr: bool) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' if attr => o.push_str("&quot;"),
            '\n' if attr => o.push_str("&#10;"),
            _ => o.push(c),
        }
    }
    o
}

fn write_node(n: &Node, out: &mut String, depth: usize) {
    let pad = "  ".repeat(depth);
    out.push_str(&pad);
    out.push('<');
    out.push_str(&n.name);
    for (k, v) in &n.attrs {
        out.push_str(&format!(" {}=\"{}\"", k, esc(v, true)));
    }
    if n.children.is_empty() && n.text.is_empty() {
        out.push_str("/>\n");
        return;
    }
    out.push('>');
    if n.children.is_empty() {
        out.push_str(&esc(&n.text, false));
    } else {
        out.push('\n');
        for c in &n.children {
            write_node(c, out, depth + 1);
        }
        out.push_str(&pad);
    }
    out.push_str(&format!("</{}>\n", n.name));
}

// ---------------------------------------------------------------- base64

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 4 / 3 + 4);
    for ch in data.chunks(3) {
        let b = [ch[0], *ch.get(1).unwrap_or(&0), *ch.get(2).unwrap_or(&0)];
        let v = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(B64[(v >> 18) as usize & 63] as char);
        s.push(B64[(v >> 12) as usize & 63] as char);
        s.push(if ch.len() > 1 { B64[(v >> 6) as usize & 63] as char } else { '=' });
        s.push(if ch.len() > 2 { B64[v as usize & 63] as char } else { '=' });
    }
    s
}

pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

// ---------------------------------------------------------------- reading helpers

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

pub fn parse_point(s: &str) -> P {
    let mut it = s.split(',');
    let x = it.next().and_then(num).unwrap_or(0.0);
    let y = it.next().and_then(num).unwrap_or(0.0);
    p(x, y)
}

fn parse_rect(s: &str) -> R {
    let mut it = s.split(';');
    let a = parse_point(it.next().unwrap_or("0,0"));
    let b = it.next().map(parse_point).unwrap_or(a);
    R::from_points(a, b)
}

fn attr_el<'a>(obj: &'a Node, names: &[&str]) -> Option<&'a Node> {
    for n in names {
        if let Some(a) = obj.children.iter().find(|c| c.local() == "attribute" && c.attr("name") == Some(n)) {
            return Some(a);
        }
    }
    None
}

fn val<'a>(obj: &'a Node, names: &[&str], ty: &str) -> Option<&'a Node> {
    attr_el(obj, names)?.first_descendant(ty)
}

fn real(obj: &Node, def: f64, names: &[&str]) -> f64 {
    val(obj, names, "real").and_then(|v| v.attr("val")).and_then(num).unwrap_or(def)
}

fn int(obj: &Node, def: i64, names: &[&str]) -> i64 {
    attr_el(obj, names)
        .and_then(|a| a.children.iter().find(|c| c.local() == "enum" || c.local() == "int"))
        .and_then(|v| v.attr("val"))
        .and_then(num)
        .map(|v| v.round() as i64)
        .unwrap_or(def)
}

fn boolean(obj: &Node, def: bool, names: &[&str]) -> bool {
    val(obj, names, "boolean").and_then(|v| v.attr("val")).map(|v| v.trim().eq_ignore_ascii_case("true")).unwrap_or(def)
}

fn point(obj: &Node, names: &[&str]) -> Option<P> {
    val(obj, names, "point").and_then(|v| v.attr("val")).map(parse_point)
}

fn points(obj: &Node, name: &str) -> Vec<P> {
    attr_el(obj, &[name]).map(|a| a.kids("point").filter_map(|q| q.attr("val")).map(parse_point).collect()).unwrap_or_default()
}

fn color(obj: &Node, def: Col, names: &[&str]) -> Col {
    val(obj, names, "color").and_then(|v| v.attr("val")).and_then(Col::parse).unwrap_or(def)
}

pub fn parse_string(n: &Node) -> String {
    let s = n.text.as_str();
    let s = s.strip_prefix('#').unwrap_or(s);
    let s = s.strip_suffix('#').unwrap_or(s);
    s.replace("\r\n", "\n")
}

fn string(obj: &Node, name: &str) -> Option<String> {
    val(obj, &[name], "string").map(parse_string)
}

fn read_text(obj: &Node) -> Option<(TextSpec, Option<P>)> {
    let comp = attr_el(obj, &["text"]).and_then(|a| a.kid("composite")).or_else(|| attr_el(obj, &["name"]).and_then(|a| a.kid("composite")))?;
    let mut t = TextSpec {
        text: string(comp, "string").unwrap_or_default(),
        height: real(comp, 0.8, &["height"]),
        color: color(comp, Col::BLACK, &["color"]),
        align: int(comp, 1, &["alignment"]).clamp(0, 2),
        ..Default::default()
    };
    if let Some(font) = val(comp, &["font"], "font") {
        t.family = font.attr("family").unwrap_or("sans").to_string();
        let style = font.attr("style").and_then(num).unwrap_or(0.0) as i64;
        t.italic = style & 0x0C != 0;
        t.bold = style & 0x70 >= 0x40;
        let name = font.attr("name").unwrap_or("").to_ascii_lowercase();
        if name.contains("bold") {
            t.bold = true;
        }
        if name.contains("italic") || name.contains("oblique") {
            t.italic = true;
        }
    }
    if t.height <= 0.0 {
        t.height = 0.8;
    }
    Some((t, point(comp, &["pos"])))
}

fn line_style(obj: &Node) -> (LineStyle, f64) {
    let a = attr_el(obj, &["line_style"]);
    let st = a.and_then(|a| a.kid("enum")).and_then(|e| e.attr("val")).and_then(num).map(|v| LineStyle::from_i(v as i64)).unwrap_or(LineStyle::Solid);
    let mut dash = real(obj, 1.0, &["dashlength"]);
    if let Some(r0) = a.and_then(|a| a.kid("real")).and_then(|e| e.attr("val")).and_then(num) {
        dash = r0;
    }
    (st, dash)
}

fn arrow(obj: &Node, prefix: &str) -> ArrowSpec {
    ArrowSpec {
        kind: int(obj, 0, &[prefix]),
        length: real(obj, 0.5, &[&format!("{prefix}_length")]),
        width: real(obj, 0.5, &[&format!("{prefix}_width")]),
    }
}

// ---------------------------------------------------------------- loading

pub fn load(path: &std::path::Path) -> Result<Diagram, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut d = parse(&bytes)?;
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    for o in d.objects.iter_mut() {
        if let Kind::Image(im) = &mut o.kind {
            if im.pic.is_none() && !im.file.is_empty() {
                let f = std::path::Path::new(&im.file);
                if !f.is_absolute() && dir.join(f).exists() {
                    im.file = dir.join(f).to_string_lossy().to_string();
                }
                im.load();
            }
        }
    }
    Ok(d)
}

pub fn parse(bytes: &[u8]) -> Result<Diagram, String> {
    let raw: Vec<u8> = if bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes).read_to_end(&mut out).map_err(|e| format!("The file is damaged ({e})."))?;
        out
    } else {
        bytes.to_vec()
    };
    let text = String::from_utf8_lossy(&raw);
    let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(&text, opts).map_err(|e| format!("This isn't a readable diagram file ({e})."))?;
    let root = convert(doc.root_element());
    from_xml(&root)
}

fn from_xml(root: &Node) -> Result<Diagram, String> {
    if root.local() != "diagram" {
        return Err("This file is not a diagram file.".into());
    }
    let mut d = Diagram { layers: vec![], ..Default::default() };
    if let Some(data) = root.kid("diagramdata") {
        d.background = color(data, Col::WHITE, &["background"]);
        if let Some(paper) = val(data, &["paper"], "composite") {
            if let Some(name) = string(paper, "name") {
                if !name.is_empty() {
                    d.paper.name = name;
                }
            }
            d.paper.tm = real(paper, 2.82, &["tmargin"]);
            d.paper.bm = real(paper, 2.82, &["bmargin"]);
            d.paper.lm = real(paper, 2.82, &["lmargin"]);
            d.paper.rm = real(paper, 2.82, &["rmargin"]);
            d.paper.portrait = boolean(paper, true, &["is_portrait"]);
            d.paper.scaling = real(paper, 1.0, &["scaling"]).max(0.01);
        }
        if let Some(grid) = val(data, &["grid"], "composite") {
            let g = real(grid, 1.0, &["width_x"]);
            d.grid = if g > 0.0 { g } else { 1.0 };
        }
    }
    let mut by_id: HashMap<String, u64> = HashMap::new();
    let mut pending: Vec<(u64, Node)> = Vec::new();
    for layer in root.kids("layer") {
        let mut name = layer.attr("name").unwrap_or("Background").to_string();
        if d.layers.iter().any(|l| l.name == name) {
            name = format!("{} {}", name, d.layers.len() + 1);
        }
        if layer.attr("active") == Some("true") {
            d.active = d.layers.len();
        }
        d.layers.push(Layer { name: name.clone(), visible: layer.attr("visible") != Some("false") });
        read_objects(layer, &name, &mut d, &mut by_id, &mut pending);
    }
    if d.layers.is_empty() {
        d.layers.push(Layer { name: "Background".into(), visible: true });
    }

    // Hook lines up to the shapes they were connected to.
    for (line_id, xml) in pending {
        let Some(conns) = xml.kid("connections") else { continue };
        for c in conns.kids("connection") {
            let Some(target) = c.attr("to").and_then(|t| by_id.get(t)).copied() else { continue };
            let Some(el) = d.element(target).map(|e| e.geometry_copy()) else { continue };
            let handle = c.attr("handle").and_then(num).unwrap_or(0.0) as i64;
            let cp = c.attr("connection").and_then(num).map(|v| v as i64).unwrap_or(-1);
            let start = handle == 0;
            let Some(Kind::Line(line)) = d.get(line_id).map(|o| &o.kind) else { continue };
            let mut q = if start { line.start() } else { line.end() };
            let att = if el.shape().fam != Family::Unknown && cp == shapes::center_index(&el) as i64 && !el.rect.inflate(0.3).contains(q) {
                Attachment::center(target)
            } else {
                let cps = el.cps();
                // Class and table boxes: trust the numbered point (each row has its own).
                let row_cp = if el.rec.is_some() && cp >= 0 && (cp as usize) + 1 < cps.len() { Some(cp as usize) } else { None };
                if row_cp.is_some() || (!el.rect.inflate(0.3).contains(q) && cp >= 0 && (cp as usize) < cps.len()) {
                    q = cps[cp as usize];
                }
                let mut a = Attachment {
                    target,
                    u: if el.rect.w > 0.0 { ((q.x - el.rect.x) / el.rect.w).clamp(0.0, 1.0) } else { 0.5 },
                    v: if el.rect.h > 0.0 { ((q.y - el.rect.y) / el.rect.h).clamp(0.0, 1.0) } else { 0.5 },
                    cp: row_cp,
                };
                if a.is_center() {
                    a.u += 1e-4; // a fixed point, not the outline-clipped middle
                }
                a
            };
            if let Some(Kind::Line(line)) = d.get_mut(line_id).map(|o| &mut o.kind) {
                if start {
                    line.start_att = Some(att);
                } else {
                    line.end_att = Some(att);
                }
            }
        }
    }
    d.update_connections();
    Ok(d)
}

fn read_objects(parent: &Node, layer: &str, d: &mut Diagram, by_id: &mut HashMap<String, u64>, pending: &mut Vec<(u64, Node)>) {
    for x in &parent.children {
        let ty = x.attr("type").unwrap_or("");
        if x.local() == "group" || (x.local() == "object" && ty == "Group") {
            read_objects(x, layer, d, by_id, pending);
            continue;
        }
        if x.local() != "object" {
            continue;
        }
        let Some(kind) = read_object(x, ty) else { continue };
        let is_line = matches!(kind, Kind::Line(_));
        let id = d.add(kind);
        let o = d.objects.last_mut().unwrap();
        o.layer = layer.to_string();
        o.orig_id = x.attr("id").map(|s| s.to_string());
        if let Some(oid) = &o.orig_id {
            by_id.insert(oid.clone(), id);
        }
        if is_line {
            pending.push((id, x.clone()));
        }
    }
}

fn make_raw(x: &Node, ty: &str) -> Kind {
    let bb = val(x, &["obj_bb"], "rectangle").and_then(|v| v.attr("val")).map(parse_rect).unwrap_or_else(|| {
        let q = point(x, &["obj_pos"]).unwrap_or_default();
        r(q.x, q.y, 1.0, 1.0)
    });
    Kind::Raw(RawObj { xml: x.clone(), bb, type_name: ty.to_string() })
}

fn read_object(x: &Node, ty: &str) -> Option<Kind> {
    let (style, dash) = line_style(x);
    match ty {
        "Standard - Line" | "Standard - ZigZagLine" | "Standard - PolyLine" | "Standard - Arc" | "Standard - BezierLine" => {
            let (kind, pts) = match ty {
                "Standard - Line" => (LineKind::Line, points(x, "conn_endpoints")),
                "Standard - Arc" => (LineKind::Arc, points(x, "conn_endpoints")),
                "Standard - ZigZagLine" => (LineKind::ZigZag, points(x, "orth_points")),
                "Standard - PolyLine" => (LineKind::PolyLine, points(x, "poly_points")),
                _ => {
                    let b = points(x, "bez_points");
                    if b.len() >= 4 && (b.len() - 1) % 3 == 0 {
                        (LineKind::Bezier, b)
                    } else {
                        (LineKind::PolyLine, b)
                    }
                }
            };
            if pts.len() < 2 {
                return Some(make_raw(x, ty));
            }
            let mut l = LineObj::new(kind, pts);
            l.color = color(x, Col::BLACK, &["line_color", "arc_color", "line_colour"]);
            l.width = real(x, 0.1, &["line_width"]);
            l.style = style;
            l.dash = dash;
            l.start_arrow = arrow(x, "start_arrow");
            l.end_arrow = arrow(x, "end_arrow");
            l.corner_radius = real(x, 0.0, &["corner_radius"]);
            l.curve = real(x, 0.0, &["curve_distance"]);
            Some(Kind::Line(l))
        }
        "Standard - Text" => {
            let (spec, pos) = read_text(x)?;
            let mut pos = pos.or_else(|| point(x, &["obj_pos"])).unwrap_or_default();
            let n = spec.lines().len() as f64;
            let asc = spec.height * ASCENT_FACTOR;
            // Convert other vertical alignments to "first line" (baseline of the first line).
            match int(x, 3, &["valign"]) {
                0 => pos.y += asc,
                1 => pos.y = pos.y - n * spec.height + asc,
                2 => pos.y = pos.y - n * spec.height / 2.0 + asc,
                _ => {}
            }
            Some(Kind::Text(TextObj { pos, text: spec }))
        }
        "Standard - Polygon" | "Standard - Beziergon" if !points(x, "poly_points").is_empty() => {
            let pts = points(x, "poly_points");
            if pts.len() < 2 {
                return Some(make_raw(x, ty));
            }
            Some(Kind::Polygon(PolygonObj {
                pts,
                line_color: color(x, Col::BLACK, &["line_color", "line_colour"]),
                line_width: real(x, 0.1, &["line_width"]),
                fill_color: color(x, Col::WHITE, &["inner_color", "fill_colour"]),
                filled: boolean(x, true, &["show_background"]),
                style,
                dash,
            }))
        }
        "Standard - Image" => {
            let c = point(x, &["elem_corner"]).unwrap_or_default();
            let mut im = ImageObj::new(r(c.x, c.y, real(x, 2.0, &["elem_width"]), real(x, 2.0, &["elem_height"])));
            im.file = string(x, "file").unwrap_or_default();
            im.keep_aspect = boolean(x, true, &["keep_aspect"]);
            im.draw_border = boolean(x, false, &["draw_border"]);
            im.line_width = real(x, 0.1, &["border_width"]);
            im.line_color = color(x, Col::BLACK, &["border_color"]);
            if let Some(data) = attr_el(x, &["pixbuf"]).and_then(|a| a.first_descendant("data")) {
                if let Some(b) = base64_decode(data.text.trim()) {
                    im.data = Some(Arc::new(b));
                    im.inline = true;
                }
            }
            im.load();
            Some(Kind::Image(im))
        }
        _ => {
            // Connector lines from other sheets (UML relationships, database references...).
            let corner = point(x, &["elem_corner"]);
            if corner.is_none() {
                if let Some(l) = read_other_line(x, ty, style, dash) {
                    return Some(Kind::Line(l));
                }
            }
            // Anything with a box (elem_corner) becomes a shape; many we know by name.
            let Some(corner) = corner else { return Some(make_raw(x, ty)) };
            let def = shapes::by_type(ty)
                .or_else(|| match ty {
                    "UML - SmallPackage" => Some(shapes::index_of("uml-package")),
                    "UML - Classicon" | "UML - Component Feature" => None,
                    _ => None,
                })
                .unwrap_or(shapes::unknown());
            let rect = r(corner.x, corner.y, real(x, 1.0, &["elem_width"]).max(0.01), real(x, 1.0, &["elem_height"]).max(0.01));
            let mut e = Element::new(def, rect);
            e.line_width = real(x, 0.1, &["border_width", "line_width"]);
            e.line_color = color(x, Col::BLACK, &["border_color", "line_colour", "line_color"]);
            e.fill_color = color(x, Col::WHITE, &["inner_color", "fill_colour", "fill_color"]);
            e.filled = boolean(x, true, &["show_background"]);
            e.style = style;
            e.dash = dash;
            e.corner_radius = real(x, 0.0, &["corner_radius"]);
            e.shear_angle = real(x, 45.0, &["shear_angle"]);
            e.padding = real(x, if e.shape().fam == Family::StdBox { 0.0 } else { 0.25 }, &["padding"]);
            if e.shape().key == "std-box" && e.corner_radius > 0.0 {
                e.def = shapes::index_of("std-roundbox");
            }
            if e.shape().key == "std-ellipse" && (rect.w - rect.h).abs() < 1e-6 {
                e.def = shapes::index_of("std-circle");
            }
            if e.shape().key == "uml-initial" && boolean(x, false, &["is_final"]) {
                e.def = shapes::index_of("uml-final");
                e.fill_color = color(x, Col::WHITE, &["fill_colour", "fill_color"]);
            }
            if e.shape().key == "uml-initial" {
                e.fill_color = color(x, Col::BLACK, &["line_colour", "line_color"]);
            }
            if e.shape().fam == Family::Uml {
                e.padding = real(x, 0.1, &["padding"]);
            }
            match read_text(x) {
                Some((t, _)) => e.text = Some(t),
                None if e.shape().fam == Family::Unknown => e.text = None,
                None => {}
            }
            if let Some(t) = &mut e.text {
                t.color = color(x, t.color, &["text_colour", "text_color"]);
            }
            match e.shape().key {
                "uml-class" => e.rec = Some(read_record(x, RecKind::Class)),
                "db-table" => e.rec = Some(read_record(x, RecKind::Table)),
                _ => {}
            }
            if e.rec.is_some() {
                crate::record::fit(&mut e);
            }
            if e.shape().fam == Family::Unknown {
                e.raw = Some(x.clone());
                e.raw_type = ty.to_string();
                if attr_el(x, &["border_width", "line_width"]).is_none() {
                    e.line_width = 0.05;
                }
            }
            Some(Kind::Element(e))
        }
    }
}

/// Lines of other kinds (UML relationships, database references, messages) become
/// ordinary lines with matching arrows and dashes.
fn read_other_line(x: &Node, ty: &str, style: LineStyle, dash: f64) -> Option<LineObj> {
    let orth = points(x, "orth_points");
    let ends = points(x, "conn_endpoints");
    let poly = points(x, "poly_points");
    let (kind, pts) = if orth.len() >= 2 {
        (LineKind::ZigZag, orth)
    } else if ends.len() >= 2 {
        (LineKind::Line, ends)
    } else if poly.len() >= 2 {
        (LineKind::PolyLine, poly)
    } else {
        return None;
    };
    let mut l = LineObj::new(kind, pts);
    l.color = color(x, Col::BLACK, &["line_colour", "line_color"]);
    l.width = real(x, 0.1, &["line_width"]);
    l.style = style;
    l.dash = dash;
    l.start_arrow = arrow(x, "start_arrow");
    l.end_arrow = arrow(x, "end_arrow");
    match ty {
        // The hollow triangle points at the parent class (the line's end in the original's files).
        "UML - Generalization" => l.end_arrow.kind = 2,
        "UML - Realizes" => {
            l.end_arrow.kind = 2;
            l.style = LineStyle::Dashed;
            l.dash = 0.4;
        }
        "UML - Dependency" => {
            l.end_arrow.kind = 1;
            l.style = LineStyle::Dashed;
            l.dash = 0.4;
        }
        "UML - Association" => {
            // Aggregation (hollow diamond) or composition (filled diamond) at the owning end.
            let diamond = match int(x, 0, &["assoc_type"]) {
                1 => 4,
                2 => 5,
                _ => 0,
            };
            if diamond != 0 {
                let a = ArrowSpec { kind: diamond, length: 1.0, width: 0.6 };
                if int(x, 0, &["direction"]) == 2 {
                    l.end_arrow = a;
                } else {
                    l.start_arrow = a;
                }
            }
        }
        "UML - Message" | "UML - Transition" => {
            if l.end_arrow.kind == 0 {
                l.end_arrow.kind = 1;
            }
        }
        _ => {}
    }
    l.label = string(x, "name").or_else(|| string(x, "text")).or_else(|| string(x, "trigger")).unwrap_or_default();
    if !ty.starts_with("Standard - ") {
        // Keep the original object so it is saved back as the same kind of line.
        let mut raw = x.clone();
        raw.children.retain(|c| c.local() != "connections");
        l.raw = Some(raw);
    }
    Some(l)
}

/// Object settings this app edits itself; everything else in a class/table is kept as it was.
const CLASS_KEYS: &[&str] = &[
    "obj_pos", "obj_bb", "elem_corner", "elem_width", "elem_height", "name", "stereotype", "comment", "abstract", "visible_comments",
    "wrap_operations", "wrap_after_char", "comment_line_length", "comment_tagging", "attributes", "operations", "line_width", "line_color",
    "fill_color", "text_color", "line_colour", "fill_colour", "text_colour", "visible_comment", "tagging_comment", "underline_primary_key",
    "bold_primary_keys",
];

fn read_record(x: &Node, kind: RecKind) -> Record {
    let mut r = match kind {
        RecKind::Class => Record::class(),
        RecKind::Table => Record::table(),
    };
    r.cols.clear();
    let s = |c: &Node, n: &str| string(c, n).unwrap_or_default();
    r.name = s(x, "name");
    r.comment = s(x, "comment");
    r.stereotype = s(x, "stereotype");
    r.is_abstract = boolean(x, false, &["abstract"]);
    r.extra = x.children.iter().filter(|c| c.local() == "attribute" && !CLASS_KEYS.contains(&c.attr("name").unwrap_or(""))).cloned().collect();
    let family = |n: &str, def: &str| val(x, &[n], "font").and_then(|f| f.attr("family")).unwrap_or(def).to_string();
    match kind {
        RecKind::Class => {
            r.show_comment = boolean(x, false, &["visible_comments"]);
            r.tag_comment = boolean(x, false, &["comment_tagging"]);
            r.wrap_ops = boolean(x, true, &["wrap_operations"]);
            r.wrap_after = int(x, 40, &["wrap_after_char"]);
            r.comment_len = int(x, 40, &["comment_line_length"]);
            r.font_normal = family("normal_font", "monospace");
            r.font_name = family("classname_font", "sans");
            r.font_comment = family("comment_font", "sans");
            r.h_normal = real(x, 0.8, &["normal_font_height", "font_height"]);
            r.h_name = real(x, 1.0, &["classname_font_height"]);
            r.h_comment = real(x, 0.7, &["comment_font_height"]);
            if let Some(at) = attr_el(x, &["attributes"]) {
                for c in at.kids("composite") {
                    r.attrs.push(ClassAttr {
                        name: s(c, "name"),
                        ty: s(c, "type"),
                        value: s(c, "value"),
                        comment: s(c, "comment"),
                        vis: int(c, 0, &["visibility"]),
                        is_abstract: boolean(c, false, &["abstract"]),
                        class_scope: boolean(c, false, &["class_scope"]),
                    });
                }
            }
            if let Some(ops) = attr_el(x, &["operations"]) {
                for c in ops.kids("composite") {
                    let params = attr_el(c, &["parameters"])
                        .map(|p| {
                            p.kids("composite")
                                .map(|q| Param { name: s(q, "name"), ty: s(q, "type"), value: s(q, "value"), comment: s(q, "comment"), kind: int(q, 0, &["kind"]) })
                                .collect()
                        })
                        .unwrap_or_default();
                    r.ops.push(ClassOp {
                        name: s(c, "name"),
                        ty: s(c, "type"),
                        stereotype: s(c, "stereotype"),
                        comment: s(c, "comment"),
                        vis: int(c, 0, &["visibility"]),
                        is_abstract: boolean(c, false, &["abstract"]),
                        inheritance: int(c, 2, &["inheritance_type"]),
                        query: boolean(c, false, &["query"]),
                        class_scope: boolean(c, false, &["class_scope"]),
                        params,
                    });
                }
            }
        }
        RecKind::Table => {
            r.show_comment = boolean(x, false, &["visible_comment"]);
            r.tag_comment = boolean(x, false, &["tagging_comment"]);
            r.underline_pk = boolean(x, true, &["underline_primary_key"]);
            r.bold_pk = boolean(x, false, &["bold_primary_keys"]);
            if let Some(at) = attr_el(x, &["attributes"]) {
                for c in at.kids("composite") {
                    r.cols.push(Column {
                        name: s(c, "name"),
                        ty: s(c, "type"),
                        comment: s(c, "comment"),
                        pk: boolean(c, false, &["primary_key"]),
                        nullable: boolean(c, true, &["nullable"]),
                        unique: boolean(c, false, &["unique"]),
                        default: s(c, "default_value"),
                    });
                }
            }
        }
    }
    r
}

fn font_node(family: &str, bold: bool) -> Node {
    let ps = match family {
        "monospace" => "Courier",
        "serif" => "Times-Roman",
        _ => "Helvetica",
    };
    let name = if bold { if family == "serif" { "Times-Bold".to_string() } else { format!("{ps}-Bold") } } else { ps.to_string() };
    Node::new("dia:font").with("family", family).with("style", if bold { 80 } else { 0 }).with("name", name)
}

fn write_record(x: &mut Node, r: &Record) {
    let st = |v: &str| vec![n_string(v)];
    if r.kind == RecKind::Class {
        x.children.extend([
            a("name", st(&r.name)),
            a("stereotype", st(&r.stereotype)),
            a("comment", st(&r.comment)),
            a("abstract", vec![n_bool(r.is_abstract)]),
            a("visible_comments", vec![n_bool(r.show_comment)]),
            a("wrap_operations", vec![n_bool(r.wrap_ops)]),
            a("wrap_after_char", vec![n_int(r.wrap_after)]),
            a("comment_line_length", vec![n_int(r.comment_len)]),
            a("comment_tagging", vec![n_bool(r.tag_comment)]),
        ]);
        let attrs = r
            .attrs
            .iter()
            .map(|at| {
                composite(
                    "umlattribute",
                    vec![
                        a("name", st(&at.name)),
                        a("type", st(&at.ty)),
                        a("value", st(&at.value)),
                        a("comment", st(&at.comment)),
                        a("visibility", vec![n_enum(at.vis)]),
                        a("abstract", vec![n_bool(at.is_abstract)]),
                        a("class_scope", vec![n_bool(at.class_scope)]),
                    ],
                )
            })
            .collect();
        let ops = r
            .ops
            .iter()
            .map(|op| {
                let params = op
                    .params
                    .iter()
                    .map(|p| {
                        composite(
                            "umlparameter",
                            vec![a("name", st(&p.name)), a("type", st(&p.ty)), a("value", st(&p.value)), a("comment", st(&p.comment)), a("kind", vec![n_enum(p.kind)])],
                        )
                    })
                    .collect();
                composite(
                    "umloperation",
                    vec![
                        a("name", st(&op.name)),
                        a("stereotype", st(&op.stereotype)),
                        a("type", st(&op.ty)),
                        a("visibility", vec![n_enum(op.vis)]),
                        a("comment", st(&op.comment)),
                        a("abstract", vec![n_bool(op.is_abstract)]),
                        a("inheritance_type", vec![n_enum(op.inheritance)]),
                        a("query", vec![n_bool(op.query)]),
                        a("class_scope", vec![n_bool(op.class_scope)]),
                        a("parameters", params),
                    ],
                )
            })
            .collect();
        x.children.extend([a("attributes", attrs), a("operations", ops)]);
        if r.extra.is_empty() {
            // A new class: the original's usual settings.
            x.children.extend([
                a("suppress_attributes", vec![n_bool(false)]),
                a("suppress_operations", vec![n_bool(false)]),
                a("visible_attributes", vec![n_bool(true)]),
                a("visible_operations", vec![n_bool(true)]),
                a("normal_font", vec![font_node(&r.font_normal, false)]),
                a("classname_font", vec![font_node(&r.font_name, true)]),
                a("comment_font", vec![font_node(&r.font_comment, false)]),
                a("normal_font_height", vec![n_real(r.h_normal)]),
                a("classname_font_height", vec![n_real(r.h_name)]),
                a("comment_font_height", vec![n_real(r.h_comment)]),
                a("template", vec![n_bool(false)]),
                a("templates", vec![]),
            ]);
        }
    } else {
        let cols = r
            .cols
            .iter()
            .map(|c| {
                composite(
                    "table_attribute",
                    vec![
                        a("name", st(&c.name)),
                        a("type", st(&c.ty)),
                        a("comment", st(&c.comment)),
                        a("primary_key", vec![n_bool(c.pk)]),
                        a("nullable", vec![n_bool(c.nullable)]),
                        a("unique", vec![n_bool(c.unique)]),
                        a("default_value", st(&c.default)),
                    ],
                )
            })
            .collect();
        x.children.extend([
            a("name", st(&r.name)),
            a("comment", st(&r.comment)),
            a("visible_comment", vec![n_bool(r.show_comment)]),
            a("tagging_comment", vec![n_bool(r.tag_comment)]),
            a("underline_primary_key", vec![n_bool(r.underline_pk)]),
            a("bold_primary_keys", vec![n_bool(r.bold_pk)]),
            a("attributes", cols),
        ]);
    }
    x.children.extend(r.extra.iter().cloned());
}

/// Replace all the values of an attribute (several points, say).
fn set_attr_values(obj: &mut Node, name: &str, values: Vec<Node>) {
    if let Some(at) = obj.children.iter_mut().find(|c| c.local() == "attribute" && c.attr("name") == Some(name)) {
        at.children = values;
    }
}

/// A UML line read from a file, written back as the same kind with its new route and hooks.
fn write_raw_line(d: &Diagram, raw: &Node, l: &LineObj, id: &str, ids: &HashMap<u64, String>) -> Node {
    let mut x = raw.clone();
    x.set_attr("id", id);
    let pts: Vec<Node> = l.pts.iter().map(|q| n_point(*q)).collect();
    let has = |x: &Node, n: &str| attr_el(x, &[n]).is_some();
    let end_handle = if has(&x, "orth_points") {
        set_attr_values(&mut x, "orth_points", pts);
        let orient: Vec<Node> = l.pts.windows(2).map(|w| n_enum(if (w[0].y - w[1].y).abs() <= (w[0].x - w[1].x).abs() { 0 } else { 1 })).collect();
        set_attr_values(&mut x, "orth_orient", orient);
        1
    } else if has(&x, "conn_endpoints") {
        set_attr_values(&mut x, "conn_endpoints", vec![n_point(l.start()), n_point(l.end())]);
        1
    } else {
        set_attr_values(&mut x, "poly_points", pts);
        l.pts.len().saturating_sub(1).max(1)
    };
    set_attr_values(&mut x, "obj_pos", vec![n_point(l.start())]);
    set_attr_values(&mut x, "obj_bb", vec![n_rect(l.bounds())]);
    if has(&x, "name") {
        set_attr_values(&mut x, "name", vec![n_string(&l.label)]);
    }
    x.children.retain(|c| c.local() != "connections");
    let mut conns = Node::new("dia:connections");
    for (handle, att) in [(0usize, &l.start_att), (end_handle, &l.end_att)] {
        if let Some(at) = att {
            if let (Some(tid), Some(el)) = (ids.get(&at.target), d.element(at.target)) {
                conns.children.push(Node::new("dia:connection").with("handle", handle).with("to", tid).with("connection", conn_index(el, at)));
            }
        }
    }
    if !conns.children.is_empty() {
        x.children.push(conns);
    }
    x
}

fn write_uml(x: &mut Node, e: &Element) {
    x.children.extend([
        a("line_width", vec![n_real(e.line_width)]),
        a("line_colour", vec![n_color(e.line_color)]),
        a("fill_colour", vec![n_color(e.fill_color)]),
        a("line_color", vec![n_color(e.line_color)]),
        a("fill_color", vec![n_color(e.fill_color)]),
    ]);
    if let Some(t) = &e.text {
        x.children.push(a("text_colour", vec![n_color(t.color)]));
        x.children.push(a("text_color", vec![n_color(t.color)]));
    }
    let key = e.shape().key;
    match key {
        "uml-initial" | "uml-final" => x.children.push(a("is_final", vec![n_bool(key == "uml-final")])),
        "uml-class" | "db-table" => {
            if let Some(r) = &e.rec {
                write_record(x, r);
            }
        }
        _ => {
            if let Some(t) = &e.text {
                let mut n = text_attr(t, e.text_pos());
                if key == "uml-node" {
                    n.set_attr("name", "name");
                }
                x.children.push(n);
            }
        }
    }
}

/// Shift every point and rectangle inside an unknown object's XML.
pub fn offset_raw(x: &mut Node, dx: f64, dy: f64) {
    match x.local() {
        "point" => {
            if let Some(v) = x.attr("val").map(parse_point) {
                x.set_attr("val", &pt(p(v.x + dx, v.y + dy)));
            }
        }
        "rectangle" => {
            if let Some(v) = x.attr("val").map(|s| s.to_string()) {
                let mut it = v.split(';');
                if let (Some(a), Some(b)) = (it.next(), it.next()) {
                    let (a, b) = (parse_point(a), parse_point(b));
                    x.set_attr("val", &format!("{};{}", pt(p(a.x + dx, a.y + dy)), pt(p(b.x + dx, b.y + dy))));
                }
            }
        }
        _ => {}
    }
    for c in x.children.iter_mut() {
        offset_raw(c, dx, dy);
    }
}

// ---------------------------------------------------------------- writing

fn f(v: f64) -> String {
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}
fn pt(q: P) -> String {
    format!("{},{}", f(q.x), f(q.y))
}

fn a(name: &str, content: Vec<Node>) -> Node {
    Node { name: "dia:attribute".into(), attrs: vec![("name".into(), name.into())], children: content, text: String::new() }
}
fn n_real(v: f64) -> Node {
    Node::new("dia:real").with("val", f(v))
}
fn n_int(v: i64) -> Node {
    Node::new("dia:int").with("val", v)
}
fn n_enum(v: i64) -> Node {
    Node::new("dia:enum").with("val", v)
}
fn n_bool(v: bool) -> Node {
    Node::new("dia:boolean").with("val", if v { "true" } else { "false" })
}
fn n_point(q: P) -> Node {
    Node::new("dia:point").with("val", pt(q))
}
fn n_color(c: Col) -> Node {
    Node::new("dia:color").with("val", c.hex())
}
fn n_string(s: &str) -> Node {
    Node { name: "dia:string".into(), text: format!("#{}#", s), ..Default::default() }
}
fn n_rect(b: R) -> Node {
    Node::new("dia:rectangle").with("val", format!("{};{}", pt(p(b.x, b.y)), pt(p(b.right(), b.bottom()))))
}
fn composite(ty: &str, content: Vec<Node>) -> Node {
    Node { name: "dia:composite".into(), attrs: vec![("type".into(), ty.into())], children: content, text: String::new() }
}

fn text_attr(t: &TextSpec, pos: P) -> Node {
    let style = (if t.bold { 0x50 } else { 0 }) | (if t.italic { 0x08 } else { 0 });
    let fam = match crate::fonts::family_index(&t.family) {
        1 => "serif",
        2 => "monospace",
        _ => "sans",
    };
    let mut ps = match fam {
        "monospace" => "Courier".to_string(),
        "serif" => "Times-Roman".to_string(),
        _ => "Helvetica".to_string(),
    };
    if t.bold || t.italic {
        if fam == "serif" {
            ps = "Times-".into();
        } else {
            ps.push('-');
        }
        if t.bold {
            ps.push_str("Bold");
        }
        if t.italic {
            ps.push_str(if fam == "serif" { "Italic" } else { "Oblique" });
        }
    }
    a(
        "text",
        vec![composite(
            "text",
            vec![
                a("string", vec![n_string(&t.text)]),
                a("font", vec![Node::new("dia:font").with("family", fam).with("style", style).with("name", ps)]),
                a("height", vec![n_real(t.height)]),
                a("pos", vec![n_point(pos)]),
                a("color", vec![n_color(t.color)]),
                a("alignment", vec![n_enum(t.align)]),
            ],
        )],
    )
}

fn obj(ty: &str, id: &str, version: i64) -> Node {
    Node::new("dia:object").with("type", ty).with("version", version).with("id", id)
}

fn set_attr_value(x: &mut Node, name: &str, value: Node) {
    if let Some(at) = x.children.iter_mut().find(|c| c.local() == "attribute" && c.attr("name") == Some(name)) {
        at.children = vec![value];
    }
}

fn remap_connections(x: &mut Node, old_to_new: &HashMap<String, String>) {
    if let Some(conns) = x.children.iter_mut().find(|c| c.local() == "connections") {
        conns.children.retain_mut(|c| match c.attr("to").and_then(|t| old_to_new.get(t)) {
            Some(n) => {
                let n = n.clone();
                c.set_attr("to", &n);
                true
            }
            None => false,
        });
    }
    x.children.retain(|c| c.local() != "connections" || !c.children.is_empty());
}

/// The numbered connection point nearest to where a line is attached.
fn conn_index(e: &Element, at: &Attachment) -> usize {
    let cps = e.cps();
    if let (Some(k), Some(_)) = (at.cp, &e.rec) {
        if k + 1 < cps.len() {
            return k;
        }
    }
    if at.is_center() {
        return cps.len() - 1;
    }
    let q = e.rect.at(at.u, at.v);
    (0..cps.len() - 1).min_by(|&i, &j| cps[i].dist(q).partial_cmp(&cps[j].dist(q)).unwrap()).unwrap_or(0)
}

pub fn to_xml_string(d: &Diagram, objects: &[&Obj]) -> String {
    let ids: HashMap<u64, String> = objects.iter().enumerate().map(|(i, o)| (o.id, format!("O{i}"))).collect();
    let old_to_new: HashMap<String, String> = objects.iter().filter_map(|o| o.orig_id.clone().map(|oid| (oid, ids[&o.id].clone()))).collect();
    let pp = &d.paper;
    let data = Node::new("dia:diagramdata").child(a("background", vec![n_color(d.background)])).child(a("pagebreak", vec![n_color(Col::rgb(0, 0, 0x99))])).child(a(
        "paper",
        vec![composite(
            "paper",
            vec![
                a("name", vec![n_string(&pp.name)]),
                a("tmargin", vec![n_real(pp.tm)]),
                a("bmargin", vec![n_real(pp.bm)]),
                a("lmargin", vec![n_real(pp.lm)]),
                a("rmargin", vec![n_real(pp.rm)]),
                a("is_portrait", vec![n_bool(pp.portrait)]),
                a("scaling", vec![n_real(pp.scaling)]),
                a("fitto", vec![n_bool(false)]),
            ],
        )],
    ));
    let data = data
        .child(a(
            "grid",
            vec![composite(
                "grid",
                vec![
                    a("width_x", vec![n_real(d.grid)]),
                    a("width_y", vec![n_real(d.grid)]),
                    a("visible_x", vec![n_int(1)]),
                    a("visible_y", vec![n_int(1)]),
                    composite("color", vec![]),
                ],
            )],
        ))
        .child(a("color", vec![n_color(Col::rgb(0xd8, 0xe5, 0xe5))]));
    let mut root = Node::new("dia:diagram").with("xmlns:dia", NS).child(data);
    let first = d.layers.first().map(|l| l.name.clone()).unwrap_or_default();
    for (li, layer) in d.layers.iter().enumerate() {
        let mut lx = Node::new("dia:layer").with("name", &layer.name).with("visible", layer.visible).with("active", li == d.active.min(d.layers.len() - 1));
        for o in objects.iter().filter(|o| o.layer == layer.name || (layer.name == first && !d.layers.iter().any(|l| l.name == o.layer))) {
            if let Some(x) = write_object(d, o, &ids[&o.id], &ids, &old_to_new) {
                lx.children.push(x);
            }
            if let Kind::Line(l) = &o.kind {
                if !l.label.is_empty() && l.raw.is_none() {
                    let t = Obj {
                        id: o.id,
                        layer: o.layer.clone(),
                        orig_id: None,
                        kind: Kind::Text(TextObj {
                            pos: crate::render::label_pos(l),
                            text: TextSpec { text: l.label.clone(), height: 0.6, color: l.color, ..Default::default() },
                        }),
                    };
                    if let Some(x) = write_object(d, &t, &format!("{}L", ids[&o.id]), &ids, &old_to_new) {
                        lx.children.push(x);
                    }
                }
            }
        }
        root.children.push(lx);
    }
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_node(&root, &mut out, 0);
    out
}

fn write_object(d: &Diagram, o: &Obj, id: &str, ids: &HashMap<u64, String>, old_to_new: &HashMap<String, String>) -> Option<Node> {
    match &o.kind {
        Kind::Element(e) if e.raw.is_some() && is_unknown(e) => {
            let mut x = e.raw.clone().unwrap();
            x.set_attr("id", id);
            set_attr_value(&mut x, "obj_pos", n_point(p(e.rect.x, e.rect.y)));
            set_attr_value(&mut x, "obj_bb", n_rect(o.bounds()));
            set_attr_value(&mut x, "elem_corner", n_point(p(e.rect.x, e.rect.y)));
            set_attr_value(&mut x, "elem_width", n_real(e.rect.w));
            set_attr_value(&mut x, "elem_height", n_real(e.rect.h));
            if let Some(t) = &e.text {
                if let Some(at) = x.children.iter_mut().find(|c| c.local() == "attribute" && c.attr("name") == Some("text")) {
                    *at = text_attr(t, e.text_pos());
                }
            }
            remap_connections(&mut x, old_to_new);
            Some(x)
        }
        Kind::Element(e) => {
            let sd = e.shape();
            let mut x = obj(sd.dia_type, id, if sd.fam == Family::Custom { 1 } else { 0 });
            let corner = p(e.rect.x, e.rect.y);
            x.children.extend([
                a("obj_pos", vec![n_point(corner)]),
                a("obj_bb", vec![n_rect(o.bounds())]),
                a("elem_corner", vec![n_point(corner)]),
                a("elem_width", vec![n_real(e.rect.w)]),
                a("elem_height", vec![n_real(e.rect.h)]),
            ]);
            if sd.fam == Family::Uml {
                write_uml(&mut x, e);
            } else if sd.fam == Family::Custom {
                x.children.extend([
                    a("line_width", vec![n_real(e.line_width)]),
                    a("line_colour", vec![n_color(e.line_color)]),
                    a("fill_colour", vec![n_color(e.fill_color)]),
                    a("show_background", vec![n_bool(e.filled)]),
                    a("line_style", vec![n_enum(e.style.to_i()), n_real(e.dash)]),
                    a("padding", vec![n_real(e.padding)]),
                ]);
                if let Some(t) = &e.text {
                    x.children.push(text_attr(t, e.text_pos()));
                }
                x.children.extend([a("flip_horizontal", vec![n_bool(false)]), a("flip_vertical", vec![n_bool(false)]), a("subscale", vec![n_real(1.0)])]);
            } else {
                x.children.extend([
                    a("border_width", vec![n_real(e.line_width)]),
                    a("border_color", vec![n_color(e.line_color)]),
                    a("inner_color", vec![n_color(e.fill_color)]),
                    a("show_background", vec![n_bool(e.filled)]),
                    a("line_style", vec![n_enum(e.style.to_i())]),
                    a("dashlength", vec![n_real(e.dash)]),
                ]);
                if sd.fam == Family::StdBox {
                    x.children.push(a("corner_radius", vec![n_real(e.corner_radius)]));
                }
                if sd.fam == Family::FlowBuiltin {
                    if sd.key == "fc-parallelogram" {
                        x.children.push(a("shear_angle", vec![n_real(e.shear_angle)]));
                    }
                    x.children.push(a("padding", vec![n_real(e.padding)]));
                    if let Some(t) = &e.text {
                        x.children.push(text_attr(t, e.text_pos()));
                    }
                }
            }
            Some(x)
        }
        Kind::Text(t) => {
            let mut x = obj("Standard - Text", id, 1);
            x.children.extend([a("obj_pos", vec![n_point(t.pos)]), a("obj_bb", vec![n_rect(t.bounds())]), text_attr(&t.text, t.pos), a("valign", vec![n_enum(3)])]);
            Some(x)
        }
        Kind::Line(l) if l.raw.is_some() => Some(write_raw_line(d, l.raw.as_ref().unwrap(), l, id, ids)),
        Kind::Line(l) => {
            let ty = match l.kind {
                LineKind::ZigZag => "Standard - ZigZagLine",
                LineKind::PolyLine => "Standard - PolyLine",
                LineKind::Arc => "Standard - Arc",
                LineKind::Bezier => "Standard - BezierLine",
                LineKind::Line => "Standard - Line",
            };
            let mut x = obj(ty, id, if l.kind == LineKind::ZigZag { 1 } else { 0 });
            x.children.extend([a("obj_pos", vec![n_point(l.start())]), a("obj_bb", vec![n_rect(l.bounds())])]);
            let pts: Vec<Node> = l.pts.iter().map(|q| n_point(*q)).collect();
            match l.kind {
                LineKind::Line => {
                    x.children.extend([a("conn_endpoints", vec![n_point(l.start()), n_point(l.end())]), a("numcp", vec![n_int(1)]), a("line_color", vec![n_color(l.color)])]);
                }
                LineKind::Arc => {
                    x.children.extend([
                        a("conn_endpoints", vec![n_point(l.start()), n_point(l.end())]),
                        a("curve_distance", vec![n_real(l.curve)]),
                        a("arc_color", vec![n_color(l.color)]),
                    ]);
                }
                LineKind::ZigZag => {
                    let orient: Vec<Node> = l.pts.windows(2).map(|w| n_enum(if (w[0].y - w[1].y).abs() <= (w[0].x - w[1].x).abs() { 0 } else { 1 })).collect();
                    x.children.extend([a("orth_points", pts), a("orth_orient", orient), a("autorouting", vec![n_bool(false)]), a("line_color", vec![n_color(l.color)])]);
                }
                LineKind::PolyLine => x.children.extend([a("poly_points", pts), a("line_color", vec![n_color(l.color)])]),
                LineKind::Bezier => {
                    let corners: Vec<Node> = (0..(l.pts.len() - 1) / 3 + 1).map(|_| n_enum(0)).collect();
                    x.children.extend([a("bez_points", pts), a("corner_types", corners), a("line_color", vec![n_color(l.color)])]);
                }
            }
            x.children.extend([a("line_width", vec![n_real(l.width)]), a("line_style", vec![n_enum(l.style.to_i())]), a("dashlength", vec![n_real(l.dash)])]);
            if matches!(l.kind, LineKind::ZigZag | LineKind::PolyLine) {
                x.children.push(a("corner_radius", vec![n_real(l.corner_radius)]));
            }
            for (prefix, ar) in [("start_arrow", &l.start_arrow), ("end_arrow", &l.end_arrow)] {
                if ar.kind != 0 {
                    x.children.extend([
                        a(prefix, vec![n_enum(ar.kind)]),
                        a(&format!("{prefix}_length"), vec![n_real(ar.length)]),
                        a(&format!("{prefix}_width"), vec![n_real(ar.width)]),
                    ]);
                }
            }
            let last_handle = match l.kind {
                LineKind::ZigZag => 1,
                LineKind::Bezier | LineKind::PolyLine => l.pts.len() - 1,
                _ => 1,
            }
            .max(1);
            let mut conns = Node::new("dia:connections");
            for (handle, att) in [(0usize, &l.start_att), (last_handle, &l.end_att)] {
                if let Some(at) = att {
                    if let (Some(tid), Some(el)) = (ids.get(&at.target), d.element(at.target)) {
                        conns.children.push(Node::new("dia:connection").with("handle", handle).with("to", tid).with("connection", conn_index(el, at)));
                    }
                }
            }
            if !conns.children.is_empty() {
                x.children.push(conns);
            }
            Some(x)
        }
        Kind::Polygon(pg) => {
            let mut x = obj("Standard - Polygon", id, 0);
            x.children.extend([
                a("obj_pos", vec![n_point(pg.pts[0])]),
                a("obj_bb", vec![n_rect(o.bounds())]),
                a("poly_points", pg.pts.iter().map(|q| n_point(*q)).collect()),
                a("line_color", vec![n_color(pg.line_color)]),
                a("line_width", vec![n_real(pg.line_width)]),
                a("inner_color", vec![n_color(pg.fill_color)]),
                a("show_background", vec![n_bool(pg.filled)]),
                a("line_style", vec![n_enum(pg.style.to_i())]),
                a("dashlength", vec![n_real(pg.dash)]),
            ]);
            Some(x)
        }
        Kind::Image(im) => {
            let mut x = obj("Standard - Image", id, 0);
            let corner = p(im.rect.x, im.rect.y);
            x.children.extend([
                a("obj_pos", vec![n_point(corner)]),
                a("obj_bb", vec![n_rect(o.bounds())]),
                a("elem_corner", vec![n_point(corner)]),
                a("elem_width", vec![n_real(im.rect.w)]),
                a("elem_height", vec![n_real(im.rect.h)]),
                a("draw_border", vec![n_bool(im.draw_border)]),
                a("keep_aspect", vec![n_bool(im.keep_aspect)]),
                a("file", vec![n_string(&im.file)]),
            ]);
            if im.draw_border {
                x.children.extend([a("border_width", vec![n_real(im.line_width)]), a("border_color", vec![n_color(im.line_color)])]);
            }
            let file_ok = !im.file.is_empty() && std::path::Path::new(&im.file).exists();
            if let Some(data) = &im.data {
                if im.inline || !file_ok {
                    x.children.push(a("inline_data", vec![n_bool(true)]));
                    let data_node = Node { name: "dia:data".into(), text: base64_encode(data), ..Default::default() };
                    x.children.push(a("pixbuf", vec![composite("pixbuf", vec![a("data", vec![data_node])])]));
                }
            }
            Some(x)
        }
        Kind::Raw(rw) => {
            let mut x = rw.xml.clone();
            x.set_attr("id", id);
            remap_connections(&mut x, old_to_new);
            Some(x)
        }
    }
}

pub fn save(d: &Diagram, path: &std::path::Path, compress: bool) -> Result<(), String> {
    let objs: Vec<&Obj> = d.objects.iter().collect();
    let xml = to_xml_string(d, &objs);
    let bytes = if compress {
        use std::io::Write;
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
        e.finish().map_err(|e| e.to_string())?
    } else {
        xml.into_bytes()
    };
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}
