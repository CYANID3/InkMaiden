//! «Бэкапы» — одна папка резервных копий на каждую читалку, отдельно от готового и от ваших файлов:
//!   <данные читалки>\Бэкапы\Книги\<путь книги>   — прежняя версия готовой книги (одна на книгу, новая заменяет старую)
//!   <данные читалки>\Бэкапы\Иконка сна\<время>    — иконка сна с читалки перед заменой
//! Ваши исходные файлы программа не меняет никогда, поэтому их копий здесь нет.
use crate::App;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::State;

pub const BOOKS: &str = "Книги";
pub const SLEEP: &str = "Иконка сна";

/// Папка «Бэкапы» читалки. Старые копии иконки сна (Sleep_backups, до 0.12.8) переезжают сюда при первом обращении.
pub fn dir(app: &App, device: &str) -> PathBuf {
    let base = app.root.join(device);
    let d = base.join("Бэкапы");
    let old = base.join("Sleep_backups");
    if old.exists() {
        let to = d.join(SLEEP);
        if pb_core::files::move_tree(&old, &to).is_ok() { let _ = std::fs::remove_dir_all(&old); }
    }
    d
}

fn walk(root: &Path, at: &Path, out: &mut Vec<Value>) {
    let Ok(rd) = std::fs::read_dir(at) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() { walk(root, &p, out); continue }
        let Ok(m) = e.metadata() else { continue };
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('/', "\\");
        let mtime = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
        out.push(json!({ "rel": rel, "size": m.len(), "mtime": mtime }));
    }
}

/// Что лежит в «Бэкапах»: прежние версии книг (по пути в готовом) и сколько копий иконки сна
#[tauri::command]
pub fn backups_list(app: State<App>, device: String) -> Result<Value, String> {
    let d = dir(&app, &device);
    let mut books = vec![];
    let b = d.join(BOOKS);
    walk(&b, &b, &mut books);
    let sleep = std::fs::read_dir(d.join(SLEEP)).map(|r| r.flatten().filter(|e| e.path().is_dir()).count()).unwrap_or(0);
    Ok(json!({ "dir": d, "books": books, "sleep": sleep }))
}

/// Вернуть прежнюю версию готовой книги. Текущая при этом становится копией — шаг можно отменить тем же действием.
#[tauri::command]
pub fn backups_restore_book(app: State<App>, device: String, out: String, rel: String) -> Result<(), String> {
    let rel = PathBuf::from(rel.replace('\\', "/"));
    if rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) { return Err("Неверный путь".into()) }
    let bak = dir(&app, &device).join(BOOKS).join(&rel);
    let cur = PathBuf::from(out).join(&rel);
    if !bak.is_file() { return Err("Прежней версии этой книги нет.".into()) }
    if let Some(p) = cur.parent() { std::fs::create_dir_all(p).map_err(|e| e.to_string())? }
    if cur.exists() {
        // обмен через временное имя: при сбое на любом шаге обе версии остаются на диске
        let tmp = cur.with_extension("inkmaiden-swap");
        std::fs::rename(&cur, &tmp).map_err(|e| format!("Не переименовать {}: {e}", cur.display()))?;
        if let Err(e) = std::fs::rename(&bak, &cur) { let _ = std::fs::rename(&tmp, &cur); return Err(format!("Не вернуть копию: {e}")) }
        std::fs::rename(&tmp, &bak).map_err(|e| format!("Новая версия осталась как {}: {e}", tmp.display()))?;
    } else {
        std::fs::rename(&bak, &cur).map_err(|e| format!("Не вернуть копию: {e}"))?;
    }
    Ok(())
}

/// Удалить прежнюю версию книги из «Бэкапов» (в корзину Windows)
#[tauri::command]
pub fn backups_forget_book(app: State<App>, device: String, rel: String) -> Result<(), String> {
    let rel = PathBuf::from(rel.replace('\\', "/"));
    if rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) { return Err("Неверный путь".into()) }
    let p = dir(&app, &device).join(BOOKS).join(rel);
    if !p.exists() { return Ok(()) }
    crate::files::ready_remove(vec![p.to_string_lossy().into_owned()]).map(|_| ())
}
