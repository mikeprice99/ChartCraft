//! An MCP (Model Context Protocol) server, so Claude Code and other AI tools can build
//! diagrams: run `ChartCraft --mcp`. Messages are JSON-RPC 2.0, one per line, on stdin/stdout.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

#[path = "mcp_tools.rs"]
mod tools;

const PROTOCOL: &str = "2025-06-18";

fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

fn tool(name: &str, description: &str, input: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input })
}

/// The tools on offer.
fn tool_list() -> Value {
    let path = json!({ "type": "string", "description": "The .dia file (created if it doesn't exist). Relative paths are from the current folder." });
    let num = |d: &str| json!({ "type": "number", "description": d });
    let s = |d: &str| json!({ "type": "string", "description": d });
    let colour = s("Colour: #rrggbb or a name (red, green, blue, yellow, orange, purple, pink, grey, white, black, lightblue, lightgreen, lightyellow, lightgrey).");
    let attr_item = json!({ "description": "\"-name: string = 'x'\" or {name, type, value, visibility: public|private|protected|package, comment, static}", "oneOf": [{ "type": "string" }, { "type": "object" }] });
    let op_item = json!({ "description": "\"+getName(id: int): string\" or {name, type, visibility, params: [{name, type, default}], comment, static, abstract}", "oneOf": [{ "type": "string" }, { "type": "object" }] });
    let col_item = json!({ "description": "\"id INT PK\" / \"email VARCHAR(255) NOT NULL UNIQUE\" or {name, type, primary_key, nullable, unique, default, comment}", "oneOf": [{ "type": "string" }, { "type": "object" }] });
    json!([
        tool("new_diagram",
            "Start a new, empty diagram file. Coordinates in all tools are centimetres, x to the right and y downwards; an A4 page is about 15 x 24 cm of drawing area.",
            schema(json!({ "path": path, "landscape": { "type": "boolean" }, "paper": s("A3, A4, A5, Letter or Legal (default A4)"), "overwrite": { "type": "boolean", "description": "Replace an existing file (default false)." } }), &["path"])),
        tool("list_shape_kinds", "List the shape kinds, line styles, arrowheads and UML/database relations that can be used.", schema(json!({}), &[])),
        tool("add_shape",
            "Add a shape (flowchart process, decision, terminal, document, UML note, actor, use case, ...). Returns its id. Leave out x/y to place it next to the others (then use layout).",
            schema(json!({ "path": path, "kind": s("Shape kind, e.g. process, decision, terminal, document, input/output, database, note, actor, use case, box, ellipse. See list_shape_kinds."),
                "text": s("Text inside the shape (use \\n for new lines)."), "x": num("Left edge (cm)"), "y": num("Top edge (cm)"), "width": num("cm (default: fits the text)"), "height": num("cm"),
                "fill": colour.clone(), "line_color": colour.clone(), "text_color": colour.clone(), "text_size": num("Text height in cm (default 0.8)") }), &["path", "kind"])),
        tool("add_text", "Add free text. Returns its id.",
            schema(json!({ "path": path, "text": s("The text (\\n for new lines)"), "x": num("cm"), "y": num("Top of the text (cm)"), "size": num("Text height in cm (default 0.8)"),
                "color": colour.clone(), "align": s("left, center or right"), "bold": { "type": "boolean" } }), &["path", "text"])),
        tool("add_class", "Add a UML class with attributes and operations. Returns its id.",
            schema(json!({ "path": path, "name": s("Class name"), "stereotype": s("e.g. interface (optional)"), "abstract": { "type": "boolean" },
                "attributes": { "type": "array", "items": attr_item }, "operations": { "type": "array", "items": op_item },
                "show_comments": { "type": "boolean" }, "x": num("cm"), "y": num("cm"), "fill": colour.clone() }), &["path", "name"])),
        tool("add_table", "Add a database table with its columns. Returns its id.",
            schema(json!({ "path": path, "name": s("Table name"), "comment": s("Table comment"), "columns": { "type": "array", "items": col_item },
                "x": num("cm"), "y": num("cm"), "fill": colour.clone() }), &["path", "name"])),
        tool("connect",
            "Draw a line from one object to another; its ends stay attached when things move. Returns its id.",
            schema(json!({ "path": path, "from": s("Id, or the text/name of the object"), "to": s("Id, or the text/name of the object"),
                "line": s("zigzag (right angles, default), straight, curved, arc or polyline"),
                "relation": s("UML/database meaning: association, directed, generalization, realization, dependency, aggregation, composition, reference, one_to_many"),
                "start_arrow": s("Arrowhead at the start: none, open, triangle, hollow_triangle, concave, diamond, filled_diamond, dot, filled_dot"),
                "end_arrow": s("Arrowhead at the end (default concave, or set by relation)"),
                "dashed": { "type": "boolean" }, "label": s("Text by the line (e.g. Yes / No)"), "color": colour.clone(), "width": num("Line width in cm (default 0.1)"),
                "from_side": s("top, bottom, left, right, center or auto (default)"), "to_side": s("top, bottom, left, right, center or auto (default)"),
                "from_row": s("For classes/tables: the attribute, operation or column to attach to"), "to_row": s("For classes/tables: the attribute, operation or column to attach to") }),
                &["path", "from", "to"])),
        tool("update_object", "Change an object: its text, position, size or colours. Moving a shape takes its lines along.",
            schema(json!({ "path": path, "id": { "type": "integer" }, "text": s("New text (shapes, free text, line labels, class/table names)"), "x": num("cm"), "y": num("cm"),
                "width": num("cm"), "height": num("cm"), "fill": colour.clone(), "line_color": colour.clone(), "text_color": colour }), &["path", "id"])),
        tool("delete_objects", "Delete objects by id (lines attached to deleted shapes come loose).",
            schema(json!({ "path": path, "ids": { "type": "array", "items": { "type": "integer" } } }), &["path", "ids"])),
        tool("list_objects", "Describe everything in a diagram: ids, kinds, text, positions, sizes, rows and what lines connect.", schema(json!({ "path": path }), &["path"])),
        tool("layout", "Arrange the shapes neatly in layers following the lines (top to bottom, or left to right), and tidy the lines.",
            schema(json!({ "path": path, "direction": s("down (default) or right"), "gap": num("Space between shapes in cm (default 1.5)") }), &["path"])),
        tool("render_preview", "Get a picture of the diagram, to check how it looks.",
            schema(json!({ "path": path, "max_size": { "type": "integer", "description": "Longest side in pixels (default 1200)" } }), &["path"])),
        tool("export", "Export the diagram as PDF, PNG or SVG.",
            schema(json!({ "path": path, "format": s("pdf, png or svg"), "output": s("Where to write it (default: next to the diagram)"),
                "page": s("For PDF: diagram (page the size of the drawing, default) or fit (fit on the paper size)") }), &["path", "format"])),
    ])
}

fn reply(out: &mut impl Write, id: &Value, result: Value) {
    let msg = json!({ "jsonrpc": "2.0", "id": id, "result": result });
    let _ = writeln!(out, "{msg}");
    let _ = out.flush();
}

fn reply_error(out: &mut impl Write, id: &Value, code: i64, message: &str) {
    let msg = json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } });
    let _ = writeln!(out, "{msg}");
    let _ = out.flush();
}

/// Answer one request (None for notifications, which get no reply).
pub fn handle(session: &mut tools::Session, msg: &Value) -> Option<Result<Value, (i64, String)>> {
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    msg.get("id")?;
    Some(match method {
        "initialize" => {
            let version = msg.pointer("/params/protocolVersion").and_then(|v| v.as_str()).unwrap_or(PROTOCOL);
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "chartcraft", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Build Dia (.dia) diagrams: flowcharts, UML class diagrams and database diagrams. Add shapes/classes/tables, connect them, call layout to arrange them, render_preview to look at the result, export for PDF. Changes are saved to the .dia file straight away; the ChartCraft app shows them when the file is open there."
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_list() })),
        "tools/call" => {
            let name = msg.pointer("/params/name").and_then(|v| v.as_str()).unwrap_or("");
            let empty = json!({});
            let args = msg.pointer("/params/arguments").unwrap_or(&empty);
            Ok(match session.call(name, args) {
                Ok(content) => json!({ "content": content }),
                Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
            })
        }
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ => Err((-32601, format!("Unknown method: {method}"))),
    })
}

/// Run the server until stdin closes.
pub fn serve() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut session = tools::Session::default();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                reply_error(&mut out, &Value::Null, -32700, &format!("Parse error: {e}"));
                continue;
            }
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        match handle(&mut session, &msg) {
            None => {}
            Some(Ok(result)) => reply(&mut out, &id, result),
            Some(Err((code, m))) => reply_error(&mut out, &id, code, &m),
        }
    }
}

#[cfg(test)]
pub use tools::Session;
