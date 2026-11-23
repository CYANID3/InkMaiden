//! Библиотека читалки: её база system/explorer-3/explorer-3.db (SQLite). Сколько книг, сколько читается и прочитано,
//! что открывали последним. Базу копируем во временную папку (вместе с журналом -wal) и читаем копию — на читалке ничего не меняется.
//! Схема базы не документирована, поэтому каждая колонка проверяется перед запросом; чего нет — того не показываем.

use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::path::Path;

fn columns(c: &Connection, table: &str) -> Vec<String> {
    let Ok(mut st) = c.prepare(&format!("PRAGMA table_info({table})")) else { return vec![] };
    st.query_map([], |r| r.get::<_, String>(1)).map(|it| it.flatten().collect()).unwrap_or_default()
}

pub fn read(root: &Path) -> Value {
    let src = root.join("system").join("explorer-3").join("explorer-3.db");
    if !src.exists() { return Value::Null }
    let tmp = std::env::temp_dir().join(format!("inkmaiden-lib-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    for ext in ["", "-wal", "-shm"] {
        let from = src.with_file_name(format!("explorer-3.db{ext}"));
        if from.exists() { let _ = std::fs::copy(&from, tmp.join(format!("explorer-3.db{ext}"))); }
    }
    let r = query(&tmp.join("explorer-3.db")).unwrap_or_else(|e| json!({ "error": e }));
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

fn query(db: &Path) -> Result<Value, String> {
    let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|e| e.to_string())?;
    let books = columns(&c, "books_impl");
    if books.is_empty() { return Err("в базе нет таблицы книг".into()) }
    let total: i64 = c.query_row("SELECT COUNT(*) FROM books_impl", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let set = columns(&c, "books_settings");
    let has = |v: &[String], n: &str| v.iter().any(|x| x == n);
    let mut out = json!({ "total": total });
    if has(&set, "bookid") && has(&set, "completed") {
        let n = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).ok();
        out["finished"] = json!(n("SELECT COUNT(DISTINCT bookid) FROM books_settings WHERE completed = 1"));
        if has(&set, "cpage") {
            out["reading"] = json!(n("SELECT COUNT(DISTINCT bookid) FROM books_settings WHERE IFNULL(completed,0) = 0 AND IFNULL(cpage,0) > 1"));
        }
    }
    if has(&set, "opentime") && has(&set, "bookid") && has(&books, "title") {
        let author = if has(&books, "author") { "b.author" } else if has(&books, "first_author") { "b.first_author" } else { "''" };
        let pages = if has(&set, "cpage") && has(&set, "npage") { "s.cpage, s.npage" } else { "0, 0" };
        let sql = format!("SELECT b.title, {author}, {pages}, s.opentime FROM books_settings s JOIN books_impl b ON b.id = s.bookid WHERE s.opentime > 0 ORDER BY s.opentime DESC LIMIT 3");
        if let Ok(mut st) = c.prepare(&sql) {
            let rows: Vec<Value> = st.query_map([], |r| Ok(json!({
                "title": r.get::<_, Option<String>>(0)?, "author": r.get::<_, Option<String>>(1)?,
                "page": r.get::<_, Option<i64>>(2)?, "pages": r.get::<_, Option<i64>>(3)?, "opened": r.get::<_, Option<i64>>(4)?,
            }))).map(|it| it.flatten().collect()).unwrap_or_default();
            out["recent"] = json!(rows);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_copy_of_db() {
        let root = std::env::temp_dir().join(format!("pblib-{}", std::process::id()));
        let dir = root.join("system/explorer-3");
        std::fs::create_dir_all(&dir).unwrap();
        let c = Connection::open(dir.join("explorer-3.db")).unwrap();
        c.execute_batch("CREATE TABLE books_impl(id INTEGER PRIMARY KEY, title TEXT, author TEXT);
            CREATE TABLE books_settings(bookid INTEGER, profileid INTEGER, cpage INTEGER, npage INTEGER, completed INTEGER, opentime INTEGER);
            INSERT INTO books_impl VALUES (1,'Солярис','Лем'),(2,'Пикник','Стругацкие'),(3,'Дюна','Герберт');
            INSERT INTO books_settings VALUES (1,1,120,120,1,100),(2,1,40,200,0,300),(3,1,1,500,0,0);").unwrap();
        drop(c);
        let v = read(&root);
        assert_eq!(v["total"], 3); assert_eq!(v["finished"], 1); assert_eq!(v["reading"], 1);
        assert_eq!(v["recent"][0]["title"], "Пикник"); assert_eq!(v["recent"][0]["page"], 40);
        let _ = std::fs::remove_dir_all(root);
    }
}
