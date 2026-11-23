//! «Книги»: найти книги в папке, обработать книгу (обложка, ссылки → текст, правки, TXT → FB2).
//! Обложки рисует окно (canvas) — сюда приходит готовый JPEG.

use crate::App;
use base64::Engine;
use pb_books::{Book, Job, Scan};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct BooksState { scan: Option<Scan> }

fn item(i: usize, b: &Book) -> Value {
    json!({ "index": i, "rel": b.rel, "path": b.path, "name": b.path.file_name().map(|n| n.to_string_lossy().to_string()), "format": b.format.label(), "fmt": b.format,
        "size": b.size, "author": b.meta.author, "title": b.meta.title, "series": b.meta.series, "number": b.meta.number,
        "hasCover": b.meta.has_cover, "genres": b.meta.genres, "error": b.error })
}

#[derive(Deserialize)]
pub struct ScanArgs { path: String, txt: bool }

/// Книги в папке (с вложенными). Чего с ними делать нельзя (MOBI, PDF …) — только счётчик.
#[tauri::command]
pub async fn books_scan(h: AppHandle, args: ScanArgs) -> Result<Value, String> {
    let root = PathBuf::from(&args.path);
    if !root.is_dir() { return Err("Такой папки нет.".into()) }
    let h2 = h.clone();
    let s = tauri::async_runtime::spawn_blocking(move || pb_books::scan_with(&root, args.txt, &|done, total| { let _ = h2.emit("books://scan", json!({ "done": done, "total": total })); }))
        .await.map_err(|e| e.to_string())?;
    let items: Vec<Value> = s.books.iter().enumerate().map(|(i, b)| item(i, b)).collect();
    let total: u64 = s.books.iter().map(|b| b.size).sum();
    let out = json!({ "root": s.root, "items": items, "skipped": s.skipped, "total": total });
    let st = h.state::<App>();
    let mut bs = st.books.lock().unwrap();
    bs.scan = Some(s);
    Ok(out)
}

#[derive(Deserialize)]
pub struct ProcessArgs {
    index: usize,
    /// читалка: прежняя версия готовой книги уходит в её «Бэкапы\Книги»
    #[serde(default)]
    device: String,
    /// не трогать книгу, если готовая уже есть и не старше оригинала
    #[serde(default)]
    skip_done: bool,
    /// куда класть готовое (папка книг читалки)
    out: String,
    /// обложка: JPEG в base64 или ничего
    cover: Option<String>,
    /// внешние ссылки → обычный текст
    #[serde(default)]
    unlink: bool,
    /// новые название, автор, серия
    #[serde(default)]
    meta: pb_books::edit::MetaEdit,
    /// поправленные части книги
    #[serde(default)]
    parts: Vec<pb_books::edit::PartEdit>,
    /// имя готового файла («Автор — Название.epub»); нет — как у исходного
    #[serde(default)]
    name: Option<String>,
}

/// Обработать одну книгу. Оригинал не меняется, готовая книга ложится в out с той же вложенностью.
#[tauri::command]
pub async fn books_process(h: AppHandle, args: ProcessArgs) -> Result<Value, String> {
    let book = book(&h.state::<App>(), args.index)?;
    let cover = match args.cover { Some(c) => Some(base64::engine::general_purpose::STANDARD.decode(c).map_err(|e| e.to_string())?), None => None };
    if let Some(n) = &args.name { if !pb_books::scan::safe_name(n) { return Err(format!("Неподходящее имя файла: {n}")) } }
    if args.skip_done && pb_books::scan::is_done_as(&book, &PathBuf::from(&args.out), args.name.as_deref()) { return Ok(json!({ "skipped": true, "rel": pb_books::scan::out_rel(&book, args.name.as_deref()), "links": 0, "bytes": 0 })) }
    let backup = (!args.device.is_empty()).then(|| crate::backups::dir(&h.state::<App>(), &args.device).join(crate::backups::BOOKS));
    tauri::async_runtime::spawn_blocking(move || {
        // сбой на одной книге — её ошибка, а не падение всей обработки
        let job = Job { cover, unlink: args.unlink, meta: args.meta, parts: args.parts, backup, out_name: args.name, ..Default::default() };
        let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pb_books::process(&book, &PathBuf::from(&args.out), &job)))
            .unwrap_or_else(|_| Err("не удалось обработать книгу (внутренняя ошибка)".to_string()))?;
        Ok(json!({ "path": done.path, "rel": done.rel, "links": done.links, "bytes": done.bytes }))
    }).await.map_err(|e| e.to_string())?
}

fn book(app: &App, index: usize) -> Result<Book, String> {
    app.books.lock().unwrap().scan.as_ref().and_then(|s| s.books.get(index)).cloned().ok_or_else(|| "Нет такой книги.".into())
}

/// Своя обложка книги — уменьшенная картинка (data URL) или null
#[tauri::command]
pub async fn books_cover(h: AppHandle, index: usize, height: u32) -> Result<Value, String> {
    let b = book(&h.state::<App>(), index)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(raw) = std::fs::read(&b.path).ok().and_then(|d| pb_books::edit::cover(&d, b.format)) else { return Ok(Value::Null) };
        let Ok(img) = image::load_from_memory(&raw) else { return Ok(Value::Null) };
        let img = if img.height() > height { img.resize(u32::MAX, height, image::imageops::FilterType::Triangle) } else { img };
        let mut out = std::io::Cursor::new(vec![]);
        img.to_rgb8().write_to(&mut out, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
        Ok(json!(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner()))))
    }).await.map_err(|e| e.to_string())?
}

/// Своя картинка-обложка книги (уже под экран читалки, JPEG в base64) — на компьютер, в папку читалки Covers.
/// Возвращает путь: по нему выбор переживает «Обновить» и перезапуск.
#[tauri::command]
pub fn books_cover_keep(h: AppHandle, device: String, name: String, data: String) -> Result<String, String> {
    let app = h.state::<App>();
    let clean: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(64).collect();
    let dev: String = device.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    if clean.is_empty() || dev.is_empty() { return Err("Неверное имя.".into()) }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|e| e.to_string())?;
    let dir = app.root.join(dev).join("Covers");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join(format!("{clean}.jpg"));
    std::fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(p.to_string_lossy().to_string())
}

/// Готовая книга по пути (вкладка «Готово для читалки»): данные и обложка — как они записаны в файле
#[tauri::command]
pub async fn books_file_info(path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let fmt = pb_books::meta::Format::of(&name).ok_or("Этот формат здесь не показывается.")?;
        let data = std::fs::read(&p).map_err(|e| e.to_string())?;
        let m = pb_books::meta::read(&data, fmt).unwrap_or_else(|_| pb_books::meta::path_meta(&name));
        let cover = pb_books::edit::cover(&data, fmt).and_then(|raw| image::load_from_memory(&raw).ok()).and_then(|img| {
            let img = if img.height() > 600 { img.resize(u32::MAX, 600, image::imageops::FilterType::Triangle) } else { img };
            let mut out = std::io::Cursor::new(vec![]);
            img.to_rgb8().write_to(&mut out, image::ImageFormat::Jpeg).ok()?;
            Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner())))
        });
        Ok(json!({ "title": m.title, "author": m.author, "series": m.series, "number": m.number, "format": fmt.label(),
            "size": data.len(), "cover": cover, "parts": matches!(fmt, pb_books::meta::Format::Fb2 | pb_books::meta::Format::Fb2Zip | pb_books::meta::Format::Epub) }))
    }).await.map_err(|e| e.to_string())?
}
/// Части готовой книги (только посмотреть)
#[tauri::command]
pub async fn books_file_parts(path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        let fmt = pb_books::meta::Format::of(&p.to_string_lossy()).ok_or("Этот формат здесь не показывается.")?;
        let data = std::fs::read(&p).map_err(|e| e.to_string())?;
        Ok(json!(pb_books::edit::parts(&data, fmt)?))
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn books_file_part(path: String, id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        let fmt = pb_books::meta::Format::of(&p.to_string_lossy()).ok_or("Этот формат здесь не показывается.")?;
        let data = std::fs::read(&p).map_err(|e| e.to_string())?;
        Ok(json!(pb_books::edit::part(&data, fmt, &id)?))
    }).await.map_err(|e| e.to_string())?
}

/// Части книги для подробного окна
#[tauri::command]
pub async fn books_parts(h: AppHandle, index: usize) -> Result<Value, String> {
    let b = book(&h.state::<App>(), index)?;
    tauri::async_runtime::spawn_blocking(move || {
        let data = std::fs::read(&b.path).map_err(|e| e.to_string())?;
        Ok(json!(pb_books::edit::parts(&data, b.format)?))
    }).await.map_err(|e| e.to_string())?
}

/// Одна часть: HTML для просмотра и исходник для правки
#[tauri::command]
pub async fn books_part(h: AppHandle, index: usize, id: String) -> Result<Value, String> {
    let b = book(&h.state::<App>(), index)?;
    tauri::async_runtime::spawn_blocking(move || {
        let data = std::fs::read(&b.path).map_err(|e| e.to_string())?;
        Ok(json!(pb_books::edit::part(&data, b.format, &id)?))
    }).await.map_err(|e| e.to_string())?
}

/// HTML части по её исходнику (для правки текстом уже поправленной части)
#[tauri::command]
pub async fn books_render_part(h: AppHandle, index: usize, id: String, source: String) -> Result<String, String> {
    let b = book(&h.state::<App>(), index)?;
    tauri::async_runtime::spawn_blocking(move || {
        let data = std::fs::read(&b.path).map_err(|e| e.to_string())?;
        pb_books::edit::render(&data, b.format, &id, &source)
    }).await.map_err(|e| e.to_string())?
}

/// Проверить правку части перед тем, как её принять
#[tauri::command]
pub async fn books_check_part(h: AppHandle, index: usize, id: String, source: String) -> Result<(), String> {
    let b = book(&h.state::<App>(), index)?;
    tauri::async_runtime::spawn_blocking(move || {
        let data = std::fs::read(&b.path).map_err(|e| e.to_string())?;
        pb_books::edit::check_part(&data, b.format, &pb_books::edit::PartEdit { id, source, delete: false })
    }).await.map_err(|e| e.to_string())?
}

/// Какие книги уже сохранены в папку готового (копия есть и не старше оригинала) — индексы
#[tauri::command]
pub async fn books_done(h: AppHandle, out: String, names: Option<Vec<Option<String>>>) -> Result<Vec<usize>, String> {
    let books = h.state::<App>().books.lock().unwrap().scan.as_ref().map(|s| s.books.clone()).unwrap_or_default();
    // names — имена готовых файлов по номерам книг (как их назовёт «Сохранить в папку»)
    let names = names.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        let out = PathBuf::from(out);
        books.iter().enumerate().filter(|(i, b)| pb_books::scan::is_done_as(b, &out, names.get(*i).and_then(|n| n.as_deref()))).map(|(i, _)| i).collect()
    }).await.map_err(|e| e.to_string())
}
