//! Checks for reading/writing .dia files and exporting.

use crate::model::*;
use crate::*;

/// A small file written the way the original program writes them.
const SAMPLE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<dia:diagram xmlns:dia="http://www.lysator.liu.se/~alla/dia/">
  <dia:diagramdata>
    <dia:attribute name="background"><dia:color val="#ffffff"/></dia:attribute>
    <dia:attribute name="paper">
      <dia:composite type="paper">
        <dia:attribute name="name"><dia:string>#A4#</dia:string></dia:attribute>
        <dia:attribute name="tmargin"><dia:real val="2.8222000598907471"/></dia:attribute>
        <dia:attribute name="bmargin"><dia:real val="2.8222000598907471"/></dia:attribute>
        <dia:attribute name="lmargin"><dia:real val="2.8222000598907471"/></dia:attribute>
        <dia:attribute name="rmargin"><dia:real val="2.8222000598907471"/></dia:attribute>
        <dia:attribute name="is_portrait"><dia:boolean val="true"/></dia:attribute>
        <dia:attribute name="scaling"><dia:real val="1"/></dia:attribute>
        <dia:attribute name="fitto"><dia:boolean val="false"/></dia:attribute>
      </dia:composite>
    </dia:attribute>
  </dia:diagramdata>
  <dia:layer name="Background" visible="true" active="true">
    <dia:object type="Flowchart - Box" version="0" id="O0">
      <dia:attribute name="obj_pos"><dia:point val="5,3"/></dia:attribute>
      <dia:attribute name="obj_bb"><dia:rectangle val="4.95,2.95;9.05,5.05"/></dia:attribute>
      <dia:attribute name="elem_corner"><dia:point val="5,3"/></dia:attribute>
      <dia:attribute name="elem_width"><dia:real val="4"/></dia:attribute>
      <dia:attribute name="elem_height"><dia:real val="2"/></dia:attribute>
      <dia:attribute name="show_background"><dia:boolean val="true"/></dia:attribute>
      <dia:attribute name="inner_color"><dia:color val="#ffe0a0"/></dia:attribute>
      <dia:attribute name="padding"><dia:real val="0.5"/></dia:attribute>
      <dia:attribute name="text">
        <dia:composite type="text">
          <dia:attribute name="string"><dia:string>#Do the thing#</dia:string></dia:attribute>
          <dia:attribute name="font"><dia:font family="sans" style="80" name="Helvetica-Bold"/></dia:attribute>
          <dia:attribute name="height"><dia:real val="0.8"/></dia:attribute>
          <dia:attribute name="pos"><dia:point val="7,4.2"/></dia:attribute>
          <dia:attribute name="color"><dia:color val="#000000"/></dia:attribute>
          <dia:attribute name="alignment"><dia:enum val="1"/></dia:attribute>
        </dia:composite>
      </dia:attribute>
    </dia:object>
    <dia:object type="Flowchart - Diamond" version="0" id="O1">
      <dia:attribute name="elem_corner"><dia:point val="5,8"/></dia:attribute>
      <dia:attribute name="elem_width"><dia:real val="4"/></dia:attribute>
      <dia:attribute name="elem_height"><dia:real val="2.5"/></dia:attribute>
      <dia:attribute name="text">
        <dia:composite type="text">
          <dia:attribute name="string"><dia:string>#OK?#</dia:string></dia:attribute>
          <dia:attribute name="height"><dia:real val="0.8"/></dia:attribute>
        </dia:composite>
      </dia:attribute>
    </dia:object>
    <dia:object type="Standard - Line" version="0" id="O2">
      <dia:attribute name="obj_pos"><dia:point val="7,5"/></dia:attribute>
      <dia:attribute name="conn_endpoints"><dia:point val="7,5"/><dia:point val="7,8"/></dia:attribute>
      <dia:attribute name="numcp"><dia:int val="1"/></dia:attribute>
      <dia:attribute name="end_arrow"><dia:enum val="22"/></dia:attribute>
      <dia:attribute name="end_arrow_length"><dia:real val="0.5"/></dia:attribute>
      <dia:attribute name="end_arrow_width"><dia:real val="0.5"/></dia:attribute>
      <dia:connections>
        <dia:connection handle="0" to="O0" connection="13"/>
        <dia:connection handle="1" to="O1" connection="0"/>
      </dia:connections>
    </dia:object>
    <dia:object type="Standard - ZigZagLine" version="1" id="O3">
      <dia:attribute name="orth_points"><dia:point val="9,9.25"/><dia:point val="12,9.25"/><dia:point val="12,4"/><dia:point val="9,4"/></dia:attribute>
      <dia:attribute name="orth_orient"><dia:enum val="0"/><dia:enum val="1"/><dia:enum val="0"/></dia:attribute>
      <dia:attribute name="line_style"><dia:enum val="1"/></dia:attribute>
      <dia:connections>
        <dia:connection handle="0" to="O1" connection="4"/>
        <dia:connection handle="2" to="O0" connection="8"/>
      </dia:connections>
    </dia:object>
    <dia:object type="Standard - Text" version="1" id="O4">
      <dia:attribute name="text">
        <dia:composite type="text">
          <dia:attribute name="string"><dia:string>#Two
lines#</dia:string></dia:attribute>
          <dia:attribute name="height"><dia:real val="0.6"/></dia:attribute>
          <dia:attribute name="pos"><dia:point val="13,2"/></dia:attribute>
          <dia:attribute name="alignment"><dia:enum val="0"/></dia:attribute>
        </dia:composite>
      </dia:attribute>
      <dia:attribute name="valign"><dia:enum val="3"/></dia:attribute>
    </dia:object>
    <dia:group>
      <dia:object type="Standard - Ellipse" version="0" id="O5">
        <dia:attribute name="elem_corner"><dia:point val="1,12"/></dia:attribute>
        <dia:attribute name="elem_width"><dia:real val="2"/></dia:attribute>
        <dia:attribute name="elem_height"><dia:real val="1"/></dia:attribute>
      </dia:object>
    </dia:group>
    <dia:object type="UML - Class" version="0" id="O6">
      <dia:attribute name="obj_bb"><dia:rectangle val="14,8;18,11"/></dia:attribute>
      <dia:attribute name="elem_corner"><dia:point val="14,8"/></dia:attribute>
      <dia:attribute name="elem_width"><dia:real val="4"/></dia:attribute>
      <dia:attribute name="elem_height"><dia:real val="3"/></dia:attribute>
      <dia:attribute name="name"><dia:string>#Order#</dia:string></dia:attribute>
    </dia:object>
    <dia:object type="Network - Router" version="0" id="O7">
      <dia:attribute name="obj_pos"><dia:point val="20,2"/></dia:attribute>
      <dia:attribute name="obj_bb"><dia:rectangle val="20,2;22,4"/></dia:attribute>
    </dia:object>
  </dia:layer>
</dia:diagram>
"##;

#[test]
fn reads_the_original_format() {
    let d = dia::parse(SAMPLE.as_bytes()).expect("parse");
    assert_eq!(d.objects.len(), 8);
    let Kind::Element(b) = &d.objects[0].kind else { panic!() };
    assert_eq!(b.shape().key, "fc-box");
    assert_eq!(b.fill_color, Col::rgb(0xff, 0xe0, 0xa0));
    let t = b.text.as_ref().unwrap();
    assert_eq!(t.text, "Do the thing");
    assert!(t.bold);
    let Kind::Line(l) = &d.objects[2].kind else { panic!() };
    assert!(l.start_att.is_some() && l.end_att.is_some());
    assert_eq!(l.end_arrow.kind, 22);
    let Kind::Line(z) = &d.objects[3].kind else { panic!() };
    assert_eq!(z.kind, LineKind::ZigZag);
    assert!(z.start_att.is_some() && z.end_att.is_some(), "zigzag hooked to both shapes");
    assert_eq!(z.style, LineStyle::Dashed);
    let Kind::Text(tx) = &d.objects[4].kind else { panic!() };
    assert_eq!(tx.text.text, "Two\nlines");
    assert!(matches!(&d.objects[5].kind, Kind::Element(e) if e.shape().key == "std-ellipse"));
    assert!(matches!(&d.objects[6].kind, Kind::Element(e) if e.rec.as_ref().map(|r| r.name == "Order").unwrap_or(false)));
    assert!(matches!(&d.objects[7].kind, Kind::Raw(_)));
}

#[test]
fn round_trips_through_save_and_load() {
    let d = dia::parse(SAMPLE.as_bytes()).unwrap();
    let objs: Vec<&Obj> = d.objects.iter().collect();
    let xml = dia::to_xml_string(&d, &objs);
    let d2 = dia::parse(xml.as_bytes()).expect("re-parse");
    assert_eq!(d2.objects.len(), d.objects.len());
    for (a, b) in d.objects.iter().zip(d2.objects.iter()) {
        let (ba, bb) = (a.bounds(), b.bounds());
        assert!((ba.x - bb.x).abs() < 1e-3 && (ba.y - bb.y).abs() < 1e-3, "{:?} vs {:?}", ba, bb);
    }
    let Kind::Line(l) = &d2.objects[2].kind else { panic!() };
    assert!(l.start_att.is_some() && l.end_att.is_some());
    assert!(xml.contains("UML - Class") && xml.contains("Network - Router"), "unknown objects are kept");

    // Moving a shape drags the connected line along.
    let mut d3 = d2.clone();
    let start_before = match &d3.objects[2].kind {
        Kind::Line(l) => l.start(),
        _ => unreachable!(),
    };
    d3.objects[0].move_by(2.0, 1.0);
    d3.update_connections();
    let Kind::Line(l) = &d3.objects[2].kind else { panic!() };
    assert!((l.start().x - start_before.x - 2.0).abs() < 1e-6 && (l.start().y - start_before.y - 1.0).abs() < 1e-6);
}

#[test]
fn reads_compressed_files() {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(SAMPLE.as_bytes()).unwrap();
    let d = dia::parse(&e.finish().unwrap()).unwrap();
    assert_eq!(d.objects.len(), 8);
}

#[test]
fn editing_with_the_mouse() {
    use editor::{Editor, Mods, Tool};
    let mut ed = Editor::new();
    let m = Mods::default();
    // Place two flowchart boxes by clicking.
    ed.tool = Tool::Shape(shapes::index_of("fc-box"));
    ed.mouse_down(geom::p(2.0, 2.0), m);
    ed.mouse_up(geom::p(2.0, 2.0));
    assert!(ed.editing.is_some(), "typing starts right after placing a shape");
    ed.editing.as_mut().unwrap().text = "First".into();
    ed.end_edit(true);
    ed.tool = Tool::Shape(shapes::index_of("fc-diamond"));
    ed.mouse_down(geom::p(2.0, 8.0), m);
    ed.mouse_move(geom::p(7.0, 11.0), m);
    ed.mouse_up(geom::p(7.0, 11.0));
    ed.end_edit(true);
    assert_eq!(ed.d.objects.len(), 2);
    let a = ed.d.objects[0].id;
    let b = ed.d.objects[1].id;
    assert_eq!(ed.d.objects[0].text().unwrap().text, "First");
    let Kind::Element(eb) = &ed.d.objects[1].kind else { panic!() };
    assert!((eb.rect.w - 5.0).abs() < 1e-9 && (eb.rect.h - 3.0).abs() < 1e-9, "dragging sets the size");

    // Draw a zigzag from the bottom middle of the box to inside the diamond.
    ed.tool = Tool::ZigZag;
    ed.mouse_down(geom::p(4.0, 4.0), m);
    ed.mouse_move(geom::p(4.5, 9.5), m);
    ed.mouse_up(geom::p(4.5, 9.5));
    let Kind::Line(l) = &ed.d.objects[2].kind else { panic!() };
    assert_eq!(l.start_att.map(|x| x.target), Some(a));
    assert_eq!(l.end_att.map(|x| x.target), Some(b));
    assert_eq!(ed.tool, Tool::Select, "tool goes back to the arrow");

    // Move the box: the line follows.
    ed.mouse_down(geom::p(3.0, 2.5), m);
    ed.mouse_move(geom::p(5.0, 3.5), m);
    ed.mouse_up(geom::p(5.0, 3.5));
    let Kind::Line(l) = &ed.d.objects[2].kind else { panic!() };
    assert!((l.start().x - 6.0).abs() < 1e-6 && (l.start().y - 5.0).abs() < 1e-6, "start {:?}", l.start());
    for w in l.pts.windows(2) {
        assert!((w[0].x - w[1].x).abs() < 1e-9 || (w[0].y - w[1].y).abs() < 1e-9, "zigzag stays at right angles: {:?}", l.pts);
    }

    // Undo the move, then undo the line.
    ed.undo();
    let Kind::Element(ea) = &ed.d.objects[0].kind else { panic!() };
    assert!((ea.rect.x - 2.0).abs() < 1e-9);
    ed.undo();
    assert_eq!(ed.d.objects.len(), 2);
    ed.redo();
    assert_eq!(ed.d.objects.len(), 3);

    // Copy both shapes and the line; the copy keeps its line hooked to the copies.
    ed.select_all();
    let xml = ed.copy().unwrap();
    assert!(xml.contains("Flowchart - Box"));
    ed.paste(Some(&xml));
    assert_eq!(ed.d.objects.len(), 6);
    let Kind::Line(l2) = &ed.d.objects[5].kind else { panic!() };
    let new_ids: Vec<u64> = ed.d.objects[3..5].iter().map(|o| o.id).collect();
    assert!(new_ids.contains(&l2.start_att.unwrap().target));

    // Delete a shape: lines hooked to it stay but come loose.
    ed.sel = vec![a];
    ed.delete_selection();
    let Kind::Line(l) = &ed.d.objects[1].kind else { panic!() };
    assert!(l.start_att.is_none());
}

#[test]
fn exports_pdf_png_svg() {
    let d = editor::example();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shots");
    std::fs::create_dir_all(&out).unwrap();
    let pdf_bytes = pdf::build(&d, pdf::PageMode::DiagramSize);
    assert!(pdf_bytes.starts_with(b"%PDF-1.4"));
    std::fs::write(out.join("example.pdf"), &pdf_bytes).unwrap();
    pdf::export(&d, &out.join("example-a4.pdf"), pdf::PageMode::FitToPaper).unwrap();
    skia::export_png(&d, &out.join("example.png"), 40.0).unwrap();
    svg::export(&d, &out.join("example.svg")).unwrap();
    dia::save(&d, &out.join("example.dia"), false).unwrap();
    let sample = dia::parse(SAMPLE.as_bytes()).unwrap();
    skia::export_png(&sample, &out.join("sample.png"), 40.0).unwrap();
    // Every flowchart shape, for a visual check.
    let mut all = Diagram::default();
    for (i, def) in shapes::all().iter().enumerate() {
        if def.sheet.is_empty() {
            continue;
        }
        let (col_, row) = (i % 6, i / 6);
        let mut e = Element::new(i, geom::r(1.0 + col_ as f64 * 5.0, 1.0 + row as f64 * 4.0, def.def_w, def.def_h));
        if let Some(t) = &mut e.text {
            t.text = def.name.split(' ').next().unwrap().to_string();
            t.height = 0.5;
        }
        all.add(Kind::Element(e));
    }
    skia::export_png(&all, &out.join("shapes.png"), 30.0).unwrap();
}

/// UML and database objects written the way the original program writes them.
const UML_SAMPLE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<dia:diagram xmlns:dia="http://www.lysator.liu.se/~alla/dia/">
  <dia:layer name="Background" visible="true" active="true">
    <dia:object type="UML - Class" version="0" id="O0">
      <dia:attribute name="elem_corner"><dia:point val="2,2"/></dia:attribute>
      <dia:attribute name="elem_width"><dia:real val="4"/></dia:attribute>
      <dia:attribute name="elem_height"><dia:real val="3"/></dia:attribute>
      <dia:attribute name="name"><dia:string>#Animal#</dia:string></dia:attribute>
      <dia:attribute name="stereotype"><dia:string>##</dia:string></dia:attribute>
      <dia:attribute name="abstract"><dia:boolean val="true"/></dia:attribute>
      <dia:attribute name="attributes">
        <dia:composite type="umlattribute">
          <dia:attribute name="name"><dia:string>#name#</dia:string></dia:attribute>
          <dia:attribute name="type"><dia:string>#String#</dia:string></dia:attribute>
          <dia:attribute name="value"><dia:string>##</dia:string></dia:attribute>
          <dia:attribute name="visibility"><dia:enum val="1"/></dia:attribute>
        </dia:composite>
      </dia:attribute>
      <dia:attribute name="operations">
        <dia:composite type="umloperation">
          <dia:attribute name="name"><dia:string>#speak#</dia:string></dia:attribute>
          <dia:attribute name="type"><dia:string>#void#</dia:string></dia:attribute>
          <dia:attribute name="visibility"><dia:enum val="0"/></dia:attribute>
          <dia:attribute name="parameters">
            <dia:composite type="umlparameter">
              <dia:attribute name="name"><dia:string>#loud#</dia:string></dia:attribute>
              <dia:attribute name="type"><dia:string>#bool#</dia:string></dia:attribute>
            </dia:composite>
          </dia:attribute>
        </dia:composite>
      </dia:attribute>
    </dia:object>
    <dia:object type="UML - Class" version="0" id="O1">
      <dia:attribute name="elem_corner"><dia:point val="2,10"/></dia:attribute>
      <dia:attribute name="elem_width"><dia:real val="4"/></dia:attribute>
      <dia:attribute name="elem_height"><dia:real val="2"/></dia:attribute>
      <dia:attribute name="name"><dia:string>#Dog#</dia:string></dia:attribute>
    </dia:object>
    <dia:object type="UML - Generalization" version="1" id="O2">
      <dia:attribute name="orth_points"><dia:point val="4,5"/><dia:point val="4,7.5"/><dia:point val="4,7.5"/><dia:point val="4,10"/></dia:attribute>
      <dia:attribute name="name"><dia:string>##</dia:string></dia:attribute>
      <dia:connections>
        <dia:connection handle="0" to="O0" connection="6"/>
        <dia:connection handle="1" to="O1" connection="1"/>
      </dia:connections>
    </dia:object>
    <dia:object type="Database - Table" version="0" id="O3">
      <dia:attribute name="elem_corner"><dia:point val="10,2"/></dia:attribute>
      <dia:attribute name="elem_width"><dia:real val="5"/></dia:attribute>
      <dia:attribute name="elem_height"><dia:real val="3"/></dia:attribute>
      <dia:attribute name="name"><dia:string>#customer#</dia:string></dia:attribute>
      <dia:attribute name="attributes">
        <dia:composite type="table_attribute">
          <dia:attribute name="name"><dia:string>#id#</dia:string></dia:attribute>
          <dia:attribute name="type"><dia:string>#INTEGER#</dia:string></dia:attribute>
          <dia:attribute name="primary_key"><dia:boolean val="true"/></dia:attribute>
          <dia:attribute name="nullable"><dia:boolean val="false"/></dia:attribute>
          <dia:attribute name="unique"><dia:boolean val="true"/></dia:attribute>
        </dia:composite>
        <dia:composite type="table_attribute">
          <dia:attribute name="name"><dia:string>#email#</dia:string></dia:attribute>
          <dia:attribute name="type"><dia:string>#TEXT#</dia:string></dia:attribute>
          <dia:attribute name="primary_key"><dia:boolean val="false"/></dia:attribute>
          <dia:attribute name="nullable"><dia:boolean val="false"/></dia:attribute>
          <dia:attribute name="unique"><dia:boolean val="false"/></dia:attribute>
        </dia:composite>
      </dia:attribute>
    </dia:object>
    <dia:object type="UML - Actor" version="0" id="O4">
      <dia:attribute name="elem_corner"><dia:point val="18,2"/></dia:attribute>
      <dia:attribute name="elem_width"><dia:real val="1.4"/></dia:attribute>
      <dia:attribute name="elem_height"><dia:real val="3"/></dia:attribute>
      <dia:attribute name="text"><dia:composite type="text">
        <dia:attribute name="string"><dia:string>#Customer#</dia:string></dia:attribute>
        <dia:attribute name="height"><dia:real val="0.8"/></dia:attribute>
      </dia:composite></dia:attribute>
    </dia:object>
    <dia:object type="UML - State Term" version="0" id="O5">
      <dia:attribute name="elem_corner"><dia:point val="18,8"/></dia:attribute>
      <dia:attribute name="elem_width"><dia:real val="1"/></dia:attribute>
      <dia:attribute name="elem_height"><dia:real val="1"/></dia:attribute>
      <dia:attribute name="is_final"><dia:boolean val="true"/></dia:attribute>
    </dia:object>
  </dia:layer>
</dia:diagram>
"##;

#[test]
fn reads_and_writes_uml_and_database() {
    let d = dia::parse(UML_SAMPLE.as_bytes()).expect("parse");
    assert_eq!(d.objects.len(), 6);
    let Kind::Element(c) = &d.objects[0].kind else { panic!() };
    let r = c.rec.as_ref().expect("class");
    assert_eq!(r.name, "Animal");
    assert!(r.is_abstract);
    assert_eq!(r.attrs.iter().map(record::attr_text).collect::<Vec<_>>(), vec!["-name: String".to_string()]);
    assert_eq!(r.ops.iter().map(|o| record::op_lines(o, false, 0).join("")).collect::<Vec<_>>(), vec!["+speak(loud:bool): void".to_string()]);
    let Kind::Line(g) = &d.objects[2].kind else { panic!("generalization is a line") };
    assert_eq!(g.end_arrow.kind, 2);
    assert!(g.start_att.is_some() && g.end_att.is_some());
    let Kind::Element(t) = &d.objects[3].kind else { panic!() };
    let tr = t.rec.as_ref().expect("table");
    assert_eq!(tr.cols.len(), 2);
    assert!(tr.cols[0].pk && tr.cols[0].unique && tr.cols[0].name == "id" && tr.cols[0].ty == "INTEGER");
    assert!(!tr.cols[1].nullable && !tr.cols[1].pk && tr.cols[1].name == "email");
    assert!(matches!(&d.objects[4].kind, Kind::Element(e) if e.shape().key == "uml-actor"));
    assert!(matches!(&d.objects[5].kind, Kind::Element(e) if e.shape().key == "uml-final"));

    // Save and read back: same contents.
    let objs: Vec<&Obj> = d.objects.iter().collect();
    let xml = dia::to_xml_string(&d, &objs);
    assert!(xml.contains("type=\"UML - Class\"") && xml.contains("type=\"Database - Table\"") && xml.contains("umlparameter"));
    let d2 = dia::parse(xml.as_bytes()).expect("re-parse");
    let Kind::Element(c2) = &d2.objects[0].kind else { panic!() };
    // Same contents (a class that had no font settings gets the original's defaults when saved).
    let plain = |r: &Option<record::Record>| r.clone().map(|mut r| {
        r.extra.clear();
        r
    });
    assert_eq!(plain(&c2.rec), plain(&c.rec));
    let Kind::Element(t2) = &d2.objects[3].kind else { panic!() };
    assert_eq!(plain(&t2.rec), plain(&t.rec));
    assert!(matches!(&d2.objects[5].kind, Kind::Element(e) if e.shape().key == "uml-final"));

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shots");
    std::fs::create_dir_all(&out).unwrap();
    skia::export_png(&d, &out.join("uml.png"), 40.0).unwrap();

    // Every UML and database shape, for a visual check.
    let mut all = Diagram::default();
    let mut n = 0;
    for (i, def) in shapes::all().iter().enumerate() {
        if def.sheet != shapes::UML && def.sheet != shapes::DATABASE {
            continue;
        }
        let mut e = Element::new(i, geom::r(1.0 + (n % 5) as f64 * 6.0, 1.0 + (n / 5) as f64 * 5.0, def.def_w, def.def_h));
        if let Some(t) = &mut e.text {
            if e.rec.is_none() && def.has_text {
                t.text = def.name.split(' ').next().unwrap().to_string();
                t.height = 0.6;
            }
        }
        e.fit_text();
        all.add(Kind::Element(e));
        n += 1;
    }
    skia::export_png(&all, &out.join("uml-shapes.png"), 30.0).unwrap();
}

#[test]
fn connectors_stay_locked_to_shapes() {
    use editor::{Editor, Mods, Tool};
    let mut ed = Editor::new();
    let m = Mods::default();
    let a = ed.d.add(Kind::Element(Element::new(shapes::index_of("fc-box"), geom::r(2.0, 2.0, 4.0, 2.0))));
    let b = ed.d.add(Kind::Element(Element::new(shapes::index_of("fc-box"), geom::r(10.0, 8.0, 4.0, 2.0))));
    // Draw a line from the middle of A's right side to somewhere inside B (not on a connection point).
    ed.tool = Tool::Line;
    ed.mouse_down(geom::p(6.0, 3.0), m);
    ed.mouse_move(geom::p(11.3, 9.3), m);
    ed.mouse_up(geom::p(11.3, 9.3));
    let line = ed.d.objects[2].id;
    let end_of = |ed: &Editor| match &ed.d.get(line).unwrap().kind {
        Kind::Line(l) => (l.start(), l.end(), l.start_att, l.end_att),
        _ => unreachable!(),
    };
    let (_, end0, sa, ea) = end_of(&ed);
    assert_eq!(sa.map(|x| x.target), Some(a));
    assert_eq!(ea.map(|x| x.target), Some(b));
    let rel = |p: geom::P| geom::p(p.x - 10.0, p.y - 8.0);
    let end_rel = rel(end0);

    // Move B: the end stays at the same spot on B.
    ed.mouse_down(geom::p(13.5, 9.8), m);
    ed.mouse_move(geom::p(15.5, 12.8), m);
    ed.mouse_up(geom::p(15.5, 12.8));
    let (_, end1, _, ea) = end_of(&ed);
    assert_eq!(ea.map(|x| x.target), Some(b));
    assert!((end1.x - 12.0 - end_rel.x).abs() < 1e-6 && (end1.y - 11.0 - end_rel.y).abs() < 1e-6, "end locked: {:?}", end1);

    // Clicking right on the line's hooked end picks the shape, so dragging moves A, not the line.
    let (start, ..) = end_of(&ed);
    ed.mouse_down(start + geom::p(-0.1, 0.0), m);
    assert_eq!(ed.sel, vec![a]);
    ed.mouse_move(start + geom::p(-1.1, 1.0), m);
    ed.mouse_up(start + geom::p(-1.1, 1.0));
    let (s2, _, sa, ea) = end_of(&ed);
    assert!(sa.is_some() && ea.is_some());
    assert!((s2.x - (start.x - 1.0)).abs() < 1e-6);

    // Moving A together with the line keeps the line hooked to B too.
    ed.sel = vec![a, line];
    ed.nudge(1.0, 0.0);
    let (_, _, sa, ea) = end_of(&ed);
    assert!(sa.is_some() && ea.is_some(), "still hooked to both");

    // Dragging the line by itself (selected and moved) lets it come loose.
    ed.sel = vec![line];
    ed.nudge(0.0, 3.0);
    let (_, _, sa, ea) = end_of(&ed);
    assert!(sa.is_none() && ea.is_none());
}

#[test]
fn class_rows_have_connection_points() {
    let mut d = Diagram::default();
    let mut c = Element::new(shapes::index_of("uml-class"), geom::r(2.0, 2.0, 3.0, 2.0));
    {
        let r = c.rec.as_mut().unwrap();
        let attr = |n: &str| record::ClassAttr { name: n.into(), vis: 1, ..Default::default() };
        r.attrs = vec![attr("name"), attr("age")];
        r.ops = vec![record::ClassOp { name: "speak".into(), inheritance: 2, ..Default::default() }];
    }
    record::fit(&mut c);
    let cps = c.cps();
    assert_eq!(cps.len(), 8 + 2 * 3 + 1, "8 around the box, left+right per row, middle");
    let rows = record::row_centers(&c);
    assert_eq!(rows.len(), 3);
    // Row points sit on the left and right edges at each row's height.
    assert!((cps[8].x - c.rect.x).abs() < 1e-9 && (cps[8].y - rows[0]).abs() < 1e-9);
    assert!((cps[11].x - c.rect.right()).abs() < 1e-9 && (cps[11].y - rows[1]).abs() < 1e-9);
    let cid = d.add(Kind::Element(c));
    let other = d.add(Kind::Element(Element::new(shapes::index_of("uml-class"), geom::r(12.0, 2.0, 3.0, 2.0))));

    // Hook a line to the right side of the "age" row (point 11).
    let mut ed = editor::Editor::new();
    ed.load(d);
    let w = { ed.d.element(cid).unwrap().cps()[11] };
    ed.tool = editor::Tool::Line;
    ed.mouse_down(w, editor::Mods::default());
    ed.mouse_move(geom::p(12.5, 2.5), editor::Mods::default());
    ed.mouse_up(geom::p(12.5, 2.5));
    let line = ed.d.objects[2].id;
    let start_att = match &ed.d.get(line).unwrap().kind {
        Kind::Line(l) => l.start_att.unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(start_att.cp, Some(11));
    let _ = other;

    // Add an attribute above: the line follows the "age" row down.
    ed.sel = vec![cid];
    ed.apply_to_selection(|o| {
        if let Kind::Element(e) = &mut o.kind {
            e.rec.as_mut().unwrap().attrs.insert(0, record::ClassAttr { name: "id".into(), ty: "int".into(), vis: 1, ..Default::default() });
        }
    });
    let e = ed.d.element(cid).unwrap();
    let age_y = record::row_centers(e)[2];
    let Kind::Line(l) = &ed.d.get(line).unwrap().kind else { panic!() };
    assert!((l.start().y - age_y).abs() < 1e-6, "line follows its row");
    let cp_now = l.start_att.unwrap().cp;

    // Save and reopen: still hooked to that row's point.
    let objs: Vec<&Obj> = ed.d.objects.iter().collect();
    let xml = dia::to_xml_string(&ed.d, &objs);
    let d2 = dia::parse(xml.as_bytes()).unwrap();
    let Kind::Line(l2) = &d2.objects[2].kind else { panic!() };
    assert_eq!(l2.start_att.unwrap().cp, cp_now);
    assert!((l2.start().y - age_y).abs() < 1e-6);
}

/// The person's own class diagram, made with the original (only read, never changed).
#[test]
fn reads_and_keeps_a_real_class_diagram() {
    let path = std::path::Path::new(r"C:\Projects\Amtek\Guides\JARS_Classes.dia");
    if !path.exists() {
        eprintln!("sample not here; skipped");
        return;
    }
    let d = dia::load(path).expect("opens");
    let classes: Vec<&Element> = d.objects.iter().filter_map(|o| match &o.kind {
        Kind::Element(e) if e.rec.is_some() => Some(e),
        _ => None,
    }).collect();
    assert_eq!(classes.len(), 7);
    let lines: Vec<&LineObj> = d.objects.iter().filter_map(|o| match &o.kind {
        Kind::Line(l) => Some(l),
        _ => None,
    }).collect();
    assert_eq!(lines.len(), 6);
    for l in &lines {
        assert!(l.start_att.is_some() && l.end_att.is_some(), "every line is hooked at both ends");
        assert!(l.raw.is_some(), "UML lines keep their kind");
    }
    // JARSPage: comments, parameters with defaults, wrapping.
    let page = classes.iter().find(|e| e.rec.as_ref().unwrap().name == "JARSPage").unwrap();
    let r = page.rec.as_ref().unwrap();
    assert_eq!(r.attrs[0].comment, "additional page headers");
    assert!(r.show_comment && r.wrap_ops);
    assert_eq!(r.font_normal, "monospace");
    assert_eq!(record::op_lines(&r.ops[0], r.wrap_ops, r.wrap_after), vec!["+__construct(minlevel:int=1)".to_string()]);
    let button = r.ops.iter().find(|o| o.name == "button").unwrap();
    assert_eq!(record::op_lines(button, r.wrap_ops, r.wrap_after).len(), 2, "long operation wraps like the original");
    // The association hooked to the $user row's left side (point 18) stays on that row.
    let assoc = lines.iter().find(|l| l.start_att.map(|a| a.cp == Some(18)).unwrap_or(false)).expect("hooked to $user row");
    let user_row_y = record::row_centers(page)[5];
    assert!((assoc.start().y - user_row_y).abs() < 1e-6);
    assert_eq!(assoc.start_arrow.kind, 4, "aggregation diamond");

    // Save to a copy (in memory) and read it back: nothing lost.
    let objs: Vec<&Obj> = d.objects.iter().collect();
    let xml = dia::to_xml_string(&d, &objs);
    assert_eq!(xml.matches("type=\"UML - Association\"").count(), 3);
    assert_eq!(xml.matches("type=\"UML - Generalization\"").count(), 3);
    assert!(xml.contains("normal_font_height") && xml.contains("#additional page headers#"));
    let d2 = dia::parse(xml.as_bytes()).unwrap();
    for (a, b) in d.objects.iter().zip(d2.objects.iter()) {
        if let (Kind::Element(x), Kind::Element(y)) = (&a.kind, &b.kind) {
            assert_eq!(x.rec, y.rec);
        }
        if let (Kind::Line(x), Kind::Line(y)) = (&a.kind, &b.kind) {
            assert_eq!(x.start_att.map(|a| a.cp), y.start_att.map(|a| a.cp));
            assert_eq!(x.end_att.map(|a| a.cp), y.end_att.map(|a| a.cp));
            assert!((x.start().dist(y.start())) < 1e-6 && (x.end().dist(y.end())) < 1e-6);
        }
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shots");
    std::fs::create_dir_all(&out).unwrap();
    skia::export_png(&d, &out.join("jars.png"), 20.0).unwrap();
}

#[test]
fn mysql_import_pieces() {
    use mysql_import as mi;
    // HeidiSQL: "abc" with salt 3 is stored as hex(a+3, b+3, c+3) followed by the salt digit.
    assert_eq!(mi::decrypt_heidi("6465663"), "abc");
    assert_eq!(mi::decrypt_heidi(""), "");
    // SQLyog: each byte rotated one bit, then base64.
    let scrambled: Vec<u8> = "secret".bytes().map(|b| b.rotate_right(1)).collect();
    assert_eq!(mi::decrypt_yog(&dia::base64_encode(&scrambled)), "secret");

    // Columns become a table like dia_mysql builds them.
    let t = mi::TableInfo { name: "customer".into(), comment: "People".into() };
    let cols = vec![
        mi::ColumnInfo { field: "id".into(), ty: "int(11)".into(), null: "NO".into(), key: "PRI".into(), extra: "auto_increment".into(), comment: "".into() },
        mi::ColumnInfo { field: "email".into(), ty: "varchar(255)".into(), null: "NO".into(), key: "UNI".into(), extra: "".into(), comment: "login".into() },
        mi::ColumnInfo { field: "note".into(), ty: "text".into(), null: "YES".into(), key: "".into(), extra: "".into(), comment: "".into() },
    ];
    let e = mi::table_element(&t, &cols, model::Col::rgb(255, 255, 200));
    let r = e.rec.as_ref().unwrap();
    assert_eq!(r.name, "customer");
    assert_eq!(r.comment, "People");
    assert!(r.cols[0].pk && !r.cols[0].nullable && r.cols[0].comment == "auto_increment");
    assert!(r.cols[1].unique && !r.cols[1].nullable && r.cols[1].comment == "login");
    assert!(r.cols[2].nullable);

    // Layout: tables in columns, none overlapping.
    let mut many: Vec<Element> = (0..12).map(|i| mi::table_element(&mi::TableInfo { name: format!("t{i}"), comment: String::new() }, &cols, model::Col::WHITE)).collect();
    mi::layout(&mut many, 0.0, 0.0);
    for i in 0..many.len() {
        for j in i + 1..many.len() {
            assert!(!many[i].rect.intersects(&many[j].rect), "tables {i} and {j} overlap");
        }
    }
    let columns = many.iter().map(|e| (e.rect.x * 100.0) as i64).collect::<std::collections::BTreeSet<_>>().len();
    assert!(columns >= 2, "laid out in more than one column");

    // The saved connections on this computer (names only).
    for c in mi::saved_connections() {
        println!("saved connection: {} ({}:{}) user set: {}, password set: {}", c.label, c.host, c.port, !c.user.is_empty(), !c.password.is_empty());
    }
}

#[test]
fn mcp_server_answers() {
    use serde_json::json;
    let dir = std::env::temp_dir().join(format!("chartcraft-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("t.dia");
    let path = file.to_string_lossy().to_string();
    let mut s = mcp::Session::default();
    let ask = |s: &mut mcp::Session, id: u64, method: &str, params: serde_json::Value| mcp::handle(s, &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).unwrap().unwrap();
    let init = ask(&mut s, 1, "initialize", json!({ "protocolVersion": "2025-06-18" }));
    assert_eq!(init["serverInfo"]["name"], "chartcraft");
    assert!(mcp::handle(&mut s, &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none(), "notifications get no reply");
    let tools = ask(&mut s, 2, "tools/list", json!({}));
    assert!(tools["tools"].as_array().unwrap().len() >= 12);
    let call = |s: &mut mcp::Session, args: serde_json::Value, name: &str| {
        let r = ask(s, 3, "tools/call", json!({ "name": name, "arguments": args }));
        assert!(r.get("isError").is_none(), "{name} failed: {r}");
        r
    };
    call(&mut s, json!({ "path": path }), "new_diagram");
    call(&mut s, json!({ "path": path, "kind": "start", "text": "Begin" }), "add_shape");
    call(&mut s, json!({ "path": path, "kind": "decision", "text": "OK?" }), "add_shape");
    call(&mut s, json!({ "path": path, "from": "Begin", "to": "OK?" }), "connect");
    call(&mut s, json!({ "path": path }), "layout");
    let pic = call(&mut s, json!({ "path": path }), "render_preview");
    assert_eq!(pic["content"][0]["type"], "image");
    // The file on disk has the shapes, joined.
    let d = dia::load(&file).unwrap();
    assert_eq!(d.objects.len(), 3);
    let Kind::Line(l) = &d.objects[2].kind else { panic!() };
    assert!(l.start_att.is_some() && l.end_att.is_some());
    // Mistakes come back as tool errors, not crashes.
    let bad = ask(&mut s, 4, "tools/call", json!({ "name": "connect", "arguments": { "path": path, "from": "Nope", "to": "OK?" } }));
    assert_eq!(bad["isError"], true);
    let _ = std::fs::remove_dir_all(&dir);
}
