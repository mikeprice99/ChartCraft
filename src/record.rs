//! Boxes with compartments: UML classes and database tables.

use crate::dia::Node;
use crate::fonts;
use crate::geom::*;
use crate::model::*;
use crate::render::Painter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecKind {
    Class,
    Table,
}

/// One column of a database table.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Column {
    pub name: String,
    pub ty: String,
    pub comment: String,
    pub pk: bool,
    pub nullable: bool,
    pub unique: bool,
    pub default: String,
}

/// One attribute of a UML class.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ClassAttr {
    pub name: String,
    pub ty: String,
    pub value: String,
    pub comment: String,
    /// 0 public, 1 private, 2 protected, 3 implementation
    pub vis: i64,
    pub is_abstract: bool,
    pub class_scope: bool,
}

/// One parameter of an operation.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Param {
    pub name: String,
    pub ty: String,
    pub value: String,
    pub comment: String,
    /// 0 undefined, 1 in, 2 out, 3 in/out
    pub kind: i64,
}

/// One operation (method) of a UML class.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ClassOp {
    pub name: String,
    pub ty: String,
    pub stereotype: String,
    pub comment: String,
    pub vis: i64,
    pub is_abstract: bool,
    /// 0 abstract, 1 polymorphic (virtual), 2 leaf (final)
    pub inheritance: i64,
    pub query: bool,
    pub class_scope: bool,
    pub params: Vec<Param>,
}

/// The contents of a class or table, kept field by field like the original.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub kind: RecKind,
    pub name: String,
    pub stereotype: String,
    pub is_abstract: bool,
    pub comment: String,
    pub show_comment: bool,
    pub tag_comment: bool,
    // Classes:
    pub attrs: Vec<ClassAttr>,
    pub ops: Vec<ClassOp>,
    pub wrap_ops: bool,
    pub wrap_after: i64,
    pub comment_len: i64,
    pub font_normal: String,
    pub font_name: String,
    pub font_comment: String,
    pub h_normal: f64,
    pub h_name: f64,
    pub h_comment: f64,
    // Tables:
    pub cols: Vec<Column>,
    pub underline_pk: bool,
    pub bold_pk: bool,
    /// Settings from the file this app doesn't edit, written back unchanged.
    pub extra: Vec<Node>,
}

impl Record {
    pub fn class() -> Self {
        Record {
            kind: RecKind::Class,
            name: "Class".into(),
            stereotype: String::new(),
            is_abstract: false,
            comment: String::new(),
            show_comment: false,
            tag_comment: false,
            attrs: vec![],
            ops: vec![],
            wrap_ops: true,
            wrap_after: 40,
            comment_len: 40,
            font_normal: "monospace".into(),
            font_name: "sans".into(),
            font_comment: "sans".into(),
            h_normal: 0.8,
            h_name: 1.0,
            h_comment: 0.6,
            cols: vec![],
            underline_pk: true,
            bold_pk: false,
            extra: vec![],
        }
    }
    pub fn table() -> Self {
        Record {
            kind: RecKind::Table,
            name: "Table".into(),
            cols: vec![Column { name: "id".into(), ty: "INTEGER".into(), pk: true, ..Default::default() }],
            ..Record::class()
        }
    }
    /// Tidy names after editing.
    pub fn tidy(&mut self) {
        for c in self.cols.iter_mut() {
            c.name = c.name.trim().to_string();
            c.ty = c.ty.trim().to_string();
        }
        for a in self.attrs.iter_mut() {
            a.name = a.name.trim().to_string();
            a.ty = a.ty.trim().to_string();
        }
        for o in self.ops.iter_mut() {
            o.name = o.name.trim().to_string();
            o.ty = o.ty.trim().to_string();
        }
    }
}

// ---------------------------------------------------------------- how rows read

pub fn vis_char(v: i64) -> &'static str {
    match v {
        1 => "-",
        2 => "#",
        3 => "",
        _ => "+",
    }
}

pub fn attr_text(a: &ClassAttr) -> String {
    let mut s = format!("{}{}", vis_char(a.vis), a.name);
    if !a.ty.is_empty() {
        s += &format!(": {}", a.ty);
    }
    if !a.value.is_empty() {
        s += &format!(" = {}", a.value);
    }
    s
}

pub fn param_text(p: &Param) -> String {
    let mut s = match p.kind {
        1 => "in ".to_string(),
        2 => "out ".to_string(),
        3 => "inout ".to_string(),
        _ => String::new(),
    };
    s += &p.name;
    if !p.ty.is_empty() {
        s += &format!(":{}", p.ty);
    }
    if !p.value.is_empty() {
        s += &format!("={}", p.value);
    }
    s
}

/// An operation as shown, wrapped after `wrap_after` characters like the original
/// (continuation lines line up after the opening bracket).
pub fn op_lines(o: &ClassOp, wrap: bool, wrap_after: i64) -> Vec<String> {
    let mut prefix = String::new();
    if !o.stereotype.is_empty() {
        prefix += &format!("<<{}>>", o.stereotype);
    }
    prefix += &format!("{}{}(", vis_char(o.vis), o.name);
    let mut tail = ")".to_string();
    if !o.ty.is_empty() {
        tail += &format!(": {}", o.ty);
    }
    if o.query {
        tail += " const";
    }
    let params: Vec<String> = o.params.iter().map(param_text).collect();
    let limit = wrap_after.max(1) as usize;
    let indent = " ".repeat(prefix.chars().count());
    let mut lines = vec![];
    let mut cur = prefix;
    for (i, p) in params.iter().enumerate() {
        cur += p;
        if i + 1 < params.len() {
            cur.push(',');
            if wrap && cur.chars().count() >= limit {
                lines.push(std::mem::replace(&mut cur, indent.clone()));
            }
        }
    }
    cur += &tail;
    lines.push(cur);
    lines
}

/// Break a comment into lines of at most `len` characters (on spaces).
pub fn wrap_words(text: &str, len: i64) -> Vec<String> {
    let len = len.max(1) as usize;
    let mut out = vec![];
    for para in text.lines() {
        let mut cur = String::new();
        for w in para.split_whitespace() {
            if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > len {
                out.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur += w;
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

/// What a table row shows.
pub fn column_display(c: &Column) -> String {
    let mut s = c.name.clone();
    if !c.ty.is_empty() {
        s += &format!(": {}", c.ty);
    }
    if !c.nullable && !c.pk {
        s += " NOT NULL";
    }
    if c.unique {
        s += " UNIQUE";
    }
    if !c.default.is_empty() {
        s += &format!(" = {}", c.default);
    }
    s
}

/// Each row's name, top to bottom (to recognise rows after editing).
pub fn row_keys(r: &Record) -> Vec<String> {
    match r.kind {
        RecKind::Class => r.attrs.iter().map(|a| format!("a:{}", a.name)).chain(r.ops.iter().map(|o| format!("o:{}", o.name))).collect(),
        RecKind::Table => r.cols.iter().map(|c| c.name.clone()).collect(),
    }
}

// ---------------------------------------------------------------- layout

const PAD: f64 = 0.2;

struct Line {
    text: String,
    spec: TextSpec,
    /// Underline the first characters (table primary keys)...
    underline: Option<String>,
    /// ...or the whole line (class-scope members).
    underline_all: bool,
}

/// One attribute / operation / column: its lines (text plus any comment).
struct Item {
    lines: Vec<Line>,
}

fn base_spec(e: &Element) -> TextSpec {
    let mut t = e.text.clone().unwrap_or_default();
    t.align = 0;
    t.text.clear();
    t
}

fn line(text: String, spec: &TextSpec) -> Line {
    Line { text, spec: spec.clone(), underline: None, underline_all: false }
}

fn comment_lines(r: &Record, text: &str, spec: &TextSpec) -> Vec<Line> {
    if !r.show_comment || text.trim().is_empty() {
        return vec![];
    }
    let text = if r.tag_comment { format!("{{documentation = {}}}", text.trim()) } else { text.trim().to_string() };
    wrap_words(&text, r.comment_len).into_iter().map(|l| line(l, spec)).collect()
}

/// Title lines and compartments, ready to measure or draw.
fn content(e: &Element, r: &Record) -> (Vec<Line>, Vec<Vec<Item>>) {
    let base = base_spec(e);
    match r.kind {
        RecKind::Class => {
            let normal = TextSpec { family: r.font_normal.clone(), height: r.h_normal, bold: false, italic: false, ..base.clone() };
            let comment = TextSpec { family: r.font_comment.clone(), height: r.h_comment, bold: false, italic: false, ..base.clone() };
            let name = TextSpec { family: r.font_name.clone(), height: r.h_name, bold: true, italic: r.is_abstract, ..base.clone() };
            let mut titles = vec![];
            if !r.stereotype.trim().is_empty() {
                titles.push(line(format!("<<{}>>", r.stereotype.trim()), &TextSpec { family: r.font_name.clone(), ..normal.clone() }));
            }
            titles.push(line(r.name.clone(), &name));
            titles.extend(comment_lines(r, &r.comment, &comment));
            let attrs = r
                .attrs
                .iter()
                .map(|a| {
                    let spec = TextSpec { italic: a.is_abstract, ..normal.clone() };
                    let mut lines = vec![Line { underline_all: a.class_scope, ..line(attr_text(a), &spec) }];
                    lines.extend(comment_lines(r, &a.comment, &comment));
                    Item { lines }
                })
                .collect();
            let ops = r
                .ops
                .iter()
                .map(|o| {
                    let spec = TextSpec { italic: o.is_abstract || o.inheritance == 0, ..normal.clone() };
                    let mut lines: Vec<Line> = op_lines(o, r.wrap_ops, r.wrap_after).into_iter().map(|t| Line { underline_all: o.class_scope, ..line(t, &spec) }).collect();
                    lines.extend(comment_lines(r, &o.comment, &comment));
                    Item { lines }
                })
                .collect();
            (titles, vec![attrs, ops])
        }
        RecKind::Table => {
            let mut name = base.clone();
            name.bold = true;
            let mut titles = vec![line(r.name.clone(), &name)];
            if r.show_comment && !r.comment.trim().is_empty() {
                let mut it = base.clone();
                it.italic = true;
                it.height *= 0.8;
                let text = if r.tag_comment { format!("{{documentation = {}}}", r.comment.trim()) } else { r.comment.trim().to_string() };
                for l in text.lines() {
                    titles.push(line(l.to_string(), &it));
                }
            }
            let rows = r
                .cols
                .iter()
                .map(|c| {
                    let mut spec = base.clone();
                    spec.bold = c.pk && r.bold_pk;
                    Item {
                        lines: vec![Line {
                            text: column_display(c),
                            spec,
                            underline: if c.pk && r.underline_pk { Some(c.name.clone()) } else { None },
                            underline_all: false,
                        }],
                    }
                })
                .collect();
            (titles, vec![rows])
        }
    }
}

fn lines_height(lines: &[Line]) -> f64 {
    lines.iter().map(|l| l.spec.height).sum()
}

/// The middle height of the name block (where connection points 3 and 4 sit).
pub fn title_center(e: &Element) -> f64 {
    let Some(r) = &e.rec else { return e.rect.cy() };
    let (titles, _) = content(e, r);
    e.rect.y + PAD + lines_height(&titles) / 2.0
}

/// The middle height of the first line of every attribute/operation/column, top to bottom.
pub fn row_centers(e: &Element) -> Vec<f64> {
    let Some(r) = &e.rec else { return vec![] };
    let (titles, sections) = content(e, r);
    let mut y = e.rect.y + PAD + lines_height(&titles) + PAD;
    let mut out = vec![];
    for sec in &sections {
        y += PAD;
        for item in sec {
            out.push(y + item.lines.first().map(|l| l.spec.height / 2.0).unwrap_or(0.0));
            y += lines_height(&item.lines);
        }
        y += PAD;
    }
    out
}

/// Size the box to fit its contents (it may be made wider, not narrower).
pub fn fit(e: &mut Element) {
    let Some(r) = e.rec.clone() else { return };
    let (titles, sections) = content(e, &r);
    let mut w: f64 = 0.0;
    let mut h = lines_height(&titles) + 2.0 * PAD;
    for l in titles.iter().chain(sections.iter().flatten().flat_map(|i| i.lines.iter())) {
        w = w.max(fonts::line_width(&l.text, &l.spec));
    }
    for sec in &sections {
        h += sec.iter().map(|i| lines_height(&i.lines)).sum::<f64>() + 2.0 * PAD;
    }
    let w = (w + 2.0 * PAD).max(2.0);
    e.rect.w = e.rect.w.max(w);
    e.rect.h = h;
}

fn underline(pt: &mut dyn Painter, l: &Line, text: &str, x: f64, baseline: f64) {
    let w = pt.text_width(text, &l.spec);
    let lh = l.spec.height;
    let uy = baseline + lh * 0.08;
    pt.path(&Path::new().move_to(x, uy).line_to(x + w, uy), None, Some(l.spec.color), lh * 0.05, LineStyle::Solid, 1.0);
}

pub fn draw(e: &Element, r: &Record, pt: &mut dyn Painter) {
    let b = e.rect;
    let stroke = if e.line_width > 0.0 { Some(e.line_color) } else { None };
    pt.path(&Path::rect(b), if e.filled { Some(e.fill_color) } else { None }, stroke, e.line_width, e.style, e.dash);
    let (titles, sections) = content(e, r);
    let mut y = b.y + PAD;
    for l in &titles {
        let w = pt.text_width(&l.text, &l.spec);
        pt.text_line(&l.text, &l.spec, b.cx() - w / 2.0, y + l.spec.height * ASCENT_FACTOR);
        y += l.spec.height;
    }
    y += PAD;
    for sec in &sections {
        pt.path(&Path::new().move_to(b.x, y).line_to(b.right(), y), None, Some(e.line_color), e.line_width, LineStyle::Solid, 1.0);
        y += PAD;
        for item in sec {
            for l in &item.lines {
                let lh = l.spec.height;
                let base = y + lh * ASCENT_FACTOR;
                pt.text_line(&l.text, &l.spec, b.x + PAD, base);
                if l.underline_all {
                    underline(pt, l, &l.text, b.x + PAD, base);
                } else if let Some(u) = &l.underline {
                    underline(pt, l, u, b.x + PAD, base);
                }
                y += lh;
            }
        }
        y += PAD;
    }
}
