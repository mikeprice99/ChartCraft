//! Importing database tables straight from MySQL (like the dia_mysql Lazarus tool):
//! saved HeidiSQL / SQLyog connections or connect.ini, then SHOW TABLE STATUS and
//! SHOW FULL COLUMNS for the chosen tables.

use crate::geom::r;
use crate::model::{Col, Element};
use crate::record::{Column, Record};
use mysql::prelude::Queryable;
use std::time::Duration;

/// How to reach a MySQL server.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Conn {
    /// Shown in the list, e.g. "HeidiSQL: live server".
    pub label: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    /// The database to start on (connect.ini's default), if any.
    pub database: String,
}

#[derive(Clone, Debug, Default)]
pub struct TableInfo {
    pub name: String,
    pub comment: String,
}

#[derive(Clone, Debug, Default)]
pub struct ColumnInfo {
    pub field: String,
    pub ty: String,
    pub null: String,
    pub key: String,
    pub extra: String,
    pub comment: String,
}

// ---------------------------------------------------------------- saved connections

/// HeidiSQL's password scrambling: hex pairs shifted by the salt digit at the end.
pub fn decrypt_heidi(s: &str) -> String {
    let b = s.as_bytes();
    if b.is_empty() {
        return String::new();
    }
    let salt = (b[b.len() - 1] as char).to_digit(10).unwrap_or(0) as i32;
    let mut out = Vec::new();
    let mut j = 0;
    while j + 2 < b.len() {
        let Ok(v) = u8::from_str_radix(&s[j..j + 2], 16) else { break };
        let mut nr = v as i32 - salt;
        if nr < 0 {
            nr += 255;
        }
        out.push(nr as u8);
        j += 2;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// SQLyog's password scrambling: base64, then each byte rotated one bit.
pub fn decrypt_yog(s: &str) -> String {
    let Some(bytes) = crate::dia::base64_decode(s.trim()) else { return String::new() };
    let out: Vec<u8> = bytes.iter().map(|z| z.rotate_left(1)).collect();
    String::from_utf8_lossy(&out).to_string()
}

/// A simple .ini reader: section -> key -> value.
fn read_ini(path: &std::path::Path) -> Vec<(String, Vec<(String, String)>)> {
    let Ok(bytes) = std::fs::read(path) else { return vec![] };
    let text = String::from_utf8_lossy(&bytes);
    let mut out: Vec<(String, Vec<(String, String)>)> = vec![];
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            out.push((line[1..line.len() - 1].trim().to_string(), vec![]));
        } else if let Some((k, v)) = line.split_once('=') {
            if let Some(sec) = out.last_mut() {
                sec.1.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    out
}

fn ini_get<'a>(ini: &'a [(String, Vec<(String, String)>)], section: &str, key: &str) -> Option<&'a str> {
    ini.iter()
        .find(|(s, _)| s.eq_ignore_ascii_case(section))
        .and_then(|(_, kv)| kv.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str()))
}

#[cfg(windows)]
fn heidi_connections() -> Vec<Conn> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let mut out = vec![];
    let Ok(servers) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\HeidiSQL\\Servers") else { return out };
    for name in servers.enum_keys().flatten() {
        let Ok(k) = servers.open_subkey(&name) else { continue };
        let host: String = k.get_value("Host").unwrap_or_default();
        if host.is_empty() {
            continue; // a folder of sessions, not a session
        }
        // Only MySQL / MariaDB sessions (not SQL Server, PostgreSQL, SQLite, Interbase...).
        let net: u32 = k.get_value::<u32, _>("NetType").ok().or_else(|| k.get_value::<String, _>("NetType").ok().and_then(|v| v.parse().ok())).unwrap_or(0);
        if (3..=9).contains(&net) || (11..=14).contains(&net) || host.contains('\\') {
            continue;
        }
        let port = k
            .get_value::<String, _>("Port")
            .ok()
            .and_then(|p| p.trim().parse().ok())
            .or_else(|| k.get_value::<u32, _>("Port").ok().map(|p| p as u16))
            .unwrap_or(3306);
        out.push(Conn {
            label: format!("HeidiSQL: {name}"),
            host,
            port,
            user: k.get_value("User").unwrap_or_default(),
            password: decrypt_heidi(&k.get_value::<String, _>("Password").unwrap_or_default()),
            database: String::new(),
        });
    }
    out
}

#[cfg(not(windows))]
fn heidi_connections() -> Vec<Conn> {
    vec![]
}

fn yog_connections() -> Vec<Conn> {
    let Some(appdata) = std::env::var_os("APPDATA") else { return vec![] };
    let ini = read_ini(&std::path::Path::new(&appdata).join("SQLyog").join("sqlyog.ini"));
    ini.iter()
        .filter(|(s, _)| s.starts_with("Connection"))
        .map(|(s, _)| Conn {
            label: format!("SQLyog: {}", ini_get(&ini, s, "Name").unwrap_or(s)),
            host: ini_get(&ini, s, "Host").unwrap_or("").to_string(),
            port: ini_get(&ini, s, "Port").and_then(|p| p.parse().ok()).unwrap_or(3306),
            user: ini_get(&ini, s, "User").unwrap_or("").to_string(),
            password: decrypt_yog(ini_get(&ini, s, "Password").unwrap_or("")),
            database: String::new(),
        })
        .collect()
}

/// connect.ini (as used by dia_mysql): [mysql] host/default, [db] rw=user, [user] password.
fn ini_connection() -> Option<Conn> {
    let mut places = vec![std::path::PathBuf::from("connect.ini")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            places.push(dir.join("connect.ini"));
        }
    }
    let path = places.into_iter().find(|p| p.exists())?;
    let ini = read_ini(&path);
    let host = ini_get(&ini, "mysql", "host").unwrap_or("localhost").to_string();
    let database = ini_get(&ini, "mysql", "default").unwrap_or("").to_string();
    let user = ini_get(&ini, &database, "rw").unwrap_or("").to_string();
    let password = ini_get(&ini, &user, "password").unwrap_or("").to_string();
    let port = ini_get(&ini, "mysql", "port").and_then(|p| p.parse().ok()).unwrap_or(3306);
    Some(Conn { label: "connect.ini (default connection)".into(), host, port, user, password, database })
}

/// Every connection saved on this computer.
pub fn saved_connections() -> Vec<Conn> {
    let mut v = vec![];
    v.extend(ini_connection());
    v.extend(heidi_connections());
    v.extend(yog_connections());
    v
}

// ---------------------------------------------------------------- talking to MySQL

fn open(c: &Conn, db: Option<&str>) -> Result<mysql::Conn, String> {
    match open_with(c, db, false) {
        Ok(conn) => Ok(conn),
        // Some hosts only accept encrypted connections: try again that way.
        Err(first) => open_with(c, db, true).map_err(|_| first),
    }
}

fn open_with(c: &Conn, db: Option<&str>, secure: bool) -> Result<mysql::Conn, String> {
    let host = if c.host.trim().is_empty() { "localhost".to_string() } else { c.host.trim().to_string() };
    let ssl = if secure { Some(mysql::SslOpts::default().with_danger_accept_invalid_certs(true).with_danger_skip_domain_validation(true)) } else { None };
    let opts = mysql::OptsBuilder::new()
        .ssl_opts(ssl)
        .ip_or_hostname(Some(host))
        .tcp_port(if c.port == 0 { 3306 } else { c.port })
        .user(Some(c.user.clone()))
        .pass(Some(c.password.clone()))
        .db_name(db.map(|d| d.to_string()))
        .prefer_socket(false)
        .tcp_connect_timeout(Some(Duration::from_secs(10)))
        .read_timeout(Some(Duration::from_secs(60)));
    mysql::Conn::new(opts).map_err(|e| format!("Couldn't connect to {}: {e}", c.host))
}

/// A cell as text (NULL becomes empty).
fn cell(row: &mysql::Row, name: &str) -> String {
    let Some(i) = row.columns_ref().iter().position(|c| c.name_str().eq_ignore_ascii_case(name)) else { return String::new() };
    match row.as_ref(i) {
        None | Some(mysql::Value::NULL) => String::new(),
        Some(mysql::Value::Bytes(b)) => String::from_utf8_lossy(b).to_string(),
        Some(mysql::Value::Int(v)) => v.to_string(),
        Some(mysql::Value::UInt(v)) => v.to_string(),
        Some(v) => format!("{v:?}"),
    }
}

fn quote(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

pub fn list_databases(c: &Conn) -> Result<Vec<String>, String> {
    let mut conn = open(c, None)?;
    let rows: Vec<mysql::Row> = conn.query("SHOW DATABASES").map_err(|e| e.to_string())?;
    Ok(rows.iter().map(|r| r.as_ref(0).map(|v| match v {
        mysql::Value::Bytes(b) => String::from_utf8_lossy(b).to_string(),
        other => format!("{other:?}"),
    }).unwrap_or_default()).collect())
}

pub fn list_tables(c: &Conn, db: &str) -> Result<Vec<TableInfo>, String> {
    let mut conn = open(c, Some(db))?;
    let rows: Vec<mysql::Row> = conn.query("SHOW TABLE STATUS").map_err(|e| e.to_string())?;
    Ok(rows.iter().map(|r| TableInfo { name: cell(r, "Name"), comment: cell(r, "Comment") }).collect())
}

pub fn table_columns(c: &Conn, db: &str, tables: &[TableInfo]) -> Result<Vec<(TableInfo, Vec<ColumnInfo>)>, String> {
    let mut conn = open(c, Some(db))?;
    let mut out = vec![];
    for t in tables {
        let rows: Vec<mysql::Row> = conn.query(format!("SHOW FULL COLUMNS FROM {}", quote(&t.name))).map_err(|e| format!("{}: {e}", t.name))?;
        let cols = rows
            .iter()
            .map(|r| ColumnInfo {
                field: cell(r, "Field"),
                ty: cell(r, "Type"),
                null: cell(r, "Null"),
                key: cell(r, "Key"),
                extra: cell(r, "Extra"),
                comment: cell(r, "Comment"),
            })
            .collect();
        out.push((t.clone(), cols));
    }
    Ok(out)
}

// ---------------------------------------------------------------- building the diagram

/// One table box, the way dia_mysql builds it: primary key from Key = PRI, nullable from
/// Null <> NO, comment = Extra + "  " + Comment.
pub fn table_element(t: &TableInfo, cols: &[ColumnInfo], fill: Col) -> Element {
    let mut e = Element::new(crate::shapes::index_of("db-table"), r(0.0, 0.0, 3.0, 2.0));
    e.fill_color = fill;
    let mut rec = Record::table();
    rec.name = t.name.clone();
    rec.comment = t.comment.clone();
    rec.cols = cols
        .iter()
        .map(|c| {
            let mut comment = String::new();
            if !c.extra.trim().is_empty() {
                comment = format!("{}  ", c.extra);
            }
            comment += &c.comment;
            Column {
                name: c.field.clone(),
                ty: c.ty.clone(),
                comment: comment.trim_end().to_string(),
                pk: c.key == "PRI",
                nullable: c.null != "NO",
                unique: c.key == "UNI",
                default: String::new(),
            }
        })
        .collect();
    e.rec = Some(rec);
    crate::record::fit(&mut e);
    e
}

/// Place tables in columns so the whole lot is about A4-shaped (like dia_mysql's Layout).
pub fn layout(tables: &mut [Element], x0: f64, y0: f64) {
    if tables.is_empty() {
        return;
    }
    let colwidth = tables.iter().map(|e| e.rect.w).fold(10.0f64, f64::max) + 1.0;
    let total: f64 = tables.iter().map(|e| e.rect.h).sum();
    let colheight = (colwidth * total / 2f64.sqrt()).sqrt();
    let (mut x, mut y) = (0.0, 0.0);
    for e in tables.iter_mut() {
        if y > 0.0 && y + e.rect.h > colheight {
            y = 0.0;
            x += colwidth;
        }
        e.rect.x = x0 + x;
        e.rect.y = y0 + y;
        y += e.rect.h + 1.0;
    }
}
