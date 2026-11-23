//! Файлы: каталог работ (обои, иконки сна), установка иконки сна с резервной копией и откатом,
//! загрузка готовых книг и комиксов на читалку, картинка, перетащенная в окно.

use crate::App;
use base64::Engine;
use pb_core::devices::now;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use pb_core::files::{clean_name, copy_checked, install, reader_rel as rel, write_checked};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

fn b64(s: &str) -> Result<Vec<u8>, String> { base64::engine::general_purpose::STANDARD.decode(s).map_err(|e| e.to_string()) }
fn enc(b: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(b) }
/// Картинка, перетащенная в окно: только изображения, не больше 60 МБ
/// Открыть папку (или показать файл) в Проводнике. Своя команда: у плагина opener путь ограничен списком разрешений,
/// и «Открыть» у папок готового отказывало («not allowed by ACL»).
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() { return Err("Папки ещё нет — она появится при первом сохранении.".into()) }
    #[cfg(windows)] {
        // explorer не понимает «"/select,C:\путь с пробелами"» (так кавычит Rust) и открывает «Документы»:
        // аргумент передаётся как есть, в кавычках — только путь
        use std::os::windows::process::CommandExt;
        let win = p.to_string_lossy().replace('/', "\\");
        let mut c = std::process::Command::new("explorer.exe");
        if p.is_file() { c.raw_arg(format!("/select,\"{win}\"")); } else { c.raw_arg(format!("\"{win}\"")); }
        c.spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))] { std::process::Command::new("xdg-open").arg(&p).spawn().map_err(|e| e.to_string())?; }
    Ok(())
}

/// Картинки в папке (с вложенными, до 2000): путь, имя, размер, превью 200 px — для выбора обоев из своей папки
#[tauri::command]
pub async fn list_images(dir: String) -> Result<Vec<Value>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use rayon::prelude::*;
        let root = PathBuf::from(&dir);
        if !root.is_dir() { return Err("Такой папки нет — укажите её в «Настройках» → «Папки и история» (Обои → Откуда брать).".to_string()) }
        let mut files = vec![];
        fn walk(d: &Path, out: &mut Vec<PathBuf>, depth: u32) {
            let Ok(rd) = std::fs::read_dir(d) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() { if depth < 4 { walk(&p, out, depth + 1) } continue }
                if p.extension().map(|x| ["jpg", "jpeg", "png", "bmp", "webp", "gif"].contains(&x.to_string_lossy().to_lowercase().as_str())).unwrap_or(false) { out.push(p) }
                if out.len() >= 2000 { return }
            }
        }
        walk(&root, &mut files, 0);
        files.sort();
        Ok(files.par_iter().map(|p| {
            let thumb = image::open(p).ok().map(|i| { let i = i.thumbnail(200, 260); let mut o = std::io::Cursor::new(vec![]); let _ = i.to_rgb8().write_to(&mut o, image::ImageFormat::Jpeg); format!("data:image/jpeg;base64,{}", enc(&o.into_inner())) });
            let m = std::fs::metadata(p).ok();
            json!({ "path": p, "name": p.file_name().map(|n| n.to_string_lossy().to_string()), "rel": p.strip_prefix(&root).map(|r| r.display().to_string()).unwrap_or_default(),
                "size": m.as_ref().map(|m| m.len()), "thumb": thumb })
        }).collect())
    }).await.map_err(|e| e.to_string())?
}

/// Скопировать файлы в папку (выбранные обои → своя папка). Имена не перезаписываются: «имя (2).bmp». Возвращает, сколько скопировано.
#[tauri::command]
pub fn copy_files(files: Vec<String>, dir: String) -> Result<usize, String> {
    let d = PathBuf::from(&dir);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let mut n = 0;
    for f in files {
        let src = PathBuf::from(&f);
        let Some(name) = src.file_name() else { continue };
        let mut dst = d.join(name);
        let (stem, ext) = (src.file_stem().unwrap_or_default().to_string_lossy().to_string(), src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default());
        let mut j = 2; while dst.exists() { dst = d.join(format!("{stem} ({j}){ext}")); j += 1 }
        std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?; n += 1;
    }
    Ok(n)
}

/// Есть ли в папке готового хоть один файл (для «На читалку»: добавлять нечего, пока ничего не сохранено)
#[tauri::command]
pub fn dir_has_files(path: String) -> bool {
    fn any(p: &std::path::Path, depth: u32) -> bool {
        let Ok(rd) = std::fs::read_dir(p) else { return false };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            if t.is_file() && !e.file_name().to_string_lossy().starts_with('.') { return true }
            if t.is_dir() && depth < 6 && any(&e.path(), depth + 1) { return true }
        }
        false
    }
    any(std::path::Path::new(&path), 0)
}

/// Содержимое папки готового: файлы (до 10 000, вложенность до 8) — путь от папки, размер, время изменения.
/// exts — только эти расширения (без точки, в нижнем регистре); пусто — все.
/// reader — та же папка на подключённой читалке: тогда у каждого файла onReader: "same" (есть, тот же размер),
/// "diff" (есть, другой размер), "none" (нет); без читалки — null.
#[tauri::command]
pub async fn ready_list(dir: String, exts: Vec<String>, reader: Option<String>) -> Result<Vec<Value>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        fn walk(root: &std::path::Path, p: &std::path::Path, depth: u32, exts: &[String], out: &mut Vec<Value>) {
            let Ok(rd) = std::fs::read_dir(p) else { return };
            let mut es: Vec<_> = rd.flatten().collect();
            es.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());
            for e in es {
                if out.len() >= 10_000 { return }
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') { continue }
                let Ok(t) = e.file_type() else { continue };
                let path = e.path();
                if t.is_dir() { if depth < 8 { walk(root, &path, depth + 1, exts, out) } continue }
                let ext = path.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
                let low = name.to_lowercase();
                if !exts.is_empty() && !exts.iter().any(|x| *x == ext || low.ends_with(&format!(".{x}"))) { continue }
                let m = e.metadata().ok();
                let mtime = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                out.push(json!({ "rel": rel, "path": path, "size": m.map(|m| m.len()).unwrap_or(0), "mtime": mtime }));
            }
        }
        let root = PathBuf::from(&dir);
        let mut out = vec![];
        walk(&root, &root, 0, &exts, &mut out);
        if let Some(r) = reader.map(PathBuf::from) {
            for f in out.iter_mut() {
                let rel = f["rel"].as_str().unwrap_or("").to_string();
                let st = match std::fs::metadata(r.join(&rel)) { Ok(m) if m.len() == f["size"].as_u64().unwrap_or(0) => "same", Ok(_) => "diff", Err(_) => "none" };
                f["onReader"] = json!(st);
            }
        }
        Ok(out)
    }).await.map_err(|e| e.to_string())?
}

/// Убрать файлы из папки готового — в корзину Windows (их можно вернуть оттуда). Только обычные файлы.
#[tauri::command]
pub fn ready_remove(paths: Vec<String>) -> Result<usize, String> {
    let files: Vec<PathBuf> = paths.iter().map(PathBuf::from).filter(|p| p.is_file()).collect();
    if files.is_empty() { return Ok(0) }
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW, FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT};
        // список путей через \0, в конце — двойной \0
        let mut buf: Vec<u16> = vec![];
        for f in &files { buf.extend(f.as_os_str().encode_wide()); buf.push(0); }
        buf.push(0);
        let mut op: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
        op.wFunc = FO_DELETE;
        op.pFrom = buf.as_ptr();
        op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as u16;
        let r = unsafe { SHFileOperationW(&mut op) };
        if r != 0 || op.fAnyOperationsAborted != 0 { return Err(format!("Не удалось убрать в корзину (код {r}).")) }
    }
    #[cfg(not(windows))] { for f in &files { std::fs::remove_file(f).map_err(|e| e.to_string())?; } }
    Ok(files.len())
}

#[tauri::command]
pub fn read_image(path: String) -> Result<Value, String> {
    let p = PathBuf::from(&path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let mime = match ext.as_str() { "jpg" | "jpeg" => "image/jpeg", "png" => "image/png", "webp" => "image/webp", "bmp" => "image/bmp", "gif" => "image/gif", _ => return Err("Это не картинка — нужен JPG, PNG, WebP, BMP или GIF.".into()) };
    if std::fs::metadata(&p).map_err(|e| e.to_string())?.len() > 60 << 20 { return Err("Картинка слишком большая (больше 60 МБ).".into()) }
    let b = std::fs::read(&p).map_err(|e| e.to_string())?;
    Ok(json!({ "name": p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), "mime": mime, "data": enc(&b) }))
}

/* ================================================================ каталог работ */
// Обои:       Wallpapers\<имя>.bmp + Wallpapers\<имя>.inkmaiden\{project.json, source.png, thumb.png}
// Иконка сна: Sleep_icons\<имя>\{файлы для читалки…, project.json, source.png, thumb.png}

#[derive(Deserialize)]
pub struct NamedFile { name: String, data: String }

fn work_dir(app: &App, device: &str, kind: &str) -> Result<PathBuf, String> {
    let d = app.device(device)?;
    match kind { "wallpapers" | "sleep" => Ok(app.pc_dir(&d, kind)), _ => Err("Неизвестный вид работ.".into()) }
}
fn meta_dir(dir: &Path, kind: &str, name: &str) -> PathBuf { if kind == "sleep" { dir.join(name) } else { dir.join(format!("{name}.inkmaiden")) } }

/// Сохранить работу в каталог. files — готовые файлы (BMP …), project/source/thumb — чтобы потом открыть и править дальше.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn works_save(app: tauri::State<App>, device: String, kind: String, name: String, files: Vec<NamedFile>, project: Value, source: Option<String>, thumb: Option<String>) -> Result<Value, String> {
    let dir = work_dir(&app, &device, &kind)?;
    let name = clean_name(&name);
    if name.is_empty() { return Err("Укажите название работы.".into()) }
    let md = meta_dir(&dir, &kind, &name);
    let mut saved = vec![];
    for f in &files {
        let p = if kind == "sleep" { md.join(clean_name(&f.name)) } else { dir.join(clean_name(&f.name)) };
        write_checked(&p, &b64(&f.data)?)?;
        saved.push(p);
    }
    write_checked(&md.join("project.json"), serde_json::to_string_pretty(&json!({ "name": name, "kind": kind, "saved": now(), "files": files.iter().map(|f| clean_name(&f.name)).collect::<Vec<_>>(), "state": project })).unwrap().as_bytes())?;
    if let Some(s) = source { write_checked(&md.join("source.png"), &b64(&s)?)? }
    if let Some(t) = thumb { write_checked(&md.join("thumb.png"), &b64(&t)?)? }
    Ok(json!({ "name": name, "files": saved, "dir": dir }))
}

/// Работы в каталоге, новые сверху
#[tauri::command]
pub fn works_list(app: tauri::State<App>, device: String, kind: String) -> Result<Vec<Value>, String> {
    let dir = work_dir(&app, &device, &kind)?;
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(vec![]) };
    let mut out: Vec<(i64, Value)> = rd.flatten().filter(|e| e.path().is_dir()).filter_map(|e| {
        let p = e.path().join("project.json");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?;
        let thumb = std::fs::read(e.path().join("thumb.png")).ok().map(|b| crate::devices::png_url(&b));
        let saved = v["saved"].as_i64().unwrap_or(0);
        Some((saved, json!({ "name": v["name"], "saved": saved, "files": v["files"], "thumb": thumb })))
    }).collect();
    out.sort_by_key(|x| std::cmp::Reverse(x.0));
    Ok(out.into_iter().map(|x| x.1).collect())
}

/// Открыть работу: настройки редактора и исходная картинка
#[tauri::command]
pub fn works_load(app: tauri::State<App>, device: String, kind: String, name: String) -> Result<Value, String> {
    let md = meta_dir(&work_dir(&app, &device, &kind)?, &kind, &clean_name(&name));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(md.join("project.json")).map_err(|_| "У этой работы нет сохранённого проекта.")?).map_err(|e| e.to_string())?;
    let src = std::fs::read(md.join("source.png")).map_err(|_| "Исходная картинка работы потерялась.")?;
    Ok(json!({ "state": v["state"], "source": enc(&src), "name": v["name"] }))
}

#[tauri::command]
pub fn works_delete(app: tauri::State<App>, device: String, kind: String, name: String) -> Result<(), String> {
    let dir = work_dir(&app, &device, &kind)?;
    let name = clean_name(&name);
    let md = meta_dir(&dir, &kind, &name);
    if kind == "wallpapers" {
        if let Ok(v) = std::fs::read_to_string(md.join("project.json")).map(|t| serde_json::from_str::<Value>(&t).unwrap_or_default()) {
            for f in v["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()) { let _ = std::fs::remove_file(dir.join(f)); }
        }
    }
    std::fs::remove_dir_all(md).map_err(|e| e.to_string())
}

/* ================================================================ иконка сна */
// Прошивка берёт иконку из system\resources\<тема> (тема — из global.cfg, обычно Line).
// Перед заменой прежние файлы копируются на компьютер: <папка читалки>\Бэкапы\Иконка сна\<время> — оттуда же откат.
// Старые копии на самой читалке (system\resources\<тема>\old) при подключении переезжают туда же — читалку не засоряем.

/// Резервные копии иконки сна — на компьютере
fn backup_dir(app: &App, device: &str) -> PathBuf { crate::backups::dir(app, device).join(crate::backups::SLEEP) }
/// Копии, оставшиеся на читалке от прежних версий (old\<время>), — на компьютер, папка old с читалки убирается
pub fn move_reader_backups(app: &App, device: &str) -> Vec<String> {
    let Ok(td) = theme_dir(app, device) else { return vec![] };
    match pb_core::files::move_tree(&td.join("old"), &backup_dir(app, device)) {
        Ok(0) => vec![], Ok(n) => vec![format!("старые копии с читалки ({n} файлов) перенесены на компьютер, папка old убрана")],
        Err(e) => vec![format!("старые копии с читалки не перенеслись: {e}")],
    }
}

fn theme_dir(app: &App, device: &str) -> Result<PathBuf, String> {
    let d = app.device(device)?;
    let root = app.connected_root(device).ok_or("Читалка не подключена.")?;
    let theme = app.usb.lock().unwrap().seen.iter().find(|s| s.mark.as_deref() == Some(device)).and_then(|s| s.theme.clone()).or(d.seen.theme.clone()).unwrap_or_else(|| "Line".into());
    Ok(root.join("system").join("resources").join(clean_name(&theme)))
}

fn work_files(app: &App, device: &str, name: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let md = meta_dir(&work_dir(app, device, "sleep")?, "sleep", &clean_name(name));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(md.join("project.json")).map_err(|_| format!("В каталоге нет работы «{name}»."))?).map_err(|e| e.to_string())?;
    v["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).map(|f| std::fs::read(md.join(f)).map(|b| (f.to_string(), b)).map_err(|e| e.to_string())).collect()
}

/// Поставить иконку сна из каталога — только из «На читалку» и только на подключённую читалку
#[tauri::command]
pub fn sleep_install(app: tauri::State<App>, device: String, name: String) -> Result<Value, String> {
    let store = app.store();
    let mut d = app.device(&device)?;
    work_files(&app, &device, &name)?; // работа существует и читается
    app.connected_root(&device).ok_or("Читалка не подключена.")?;
    d.sleep.pending = Some(name);
    store.save(&d).map_err(|e| e.to_string())?;
    let log = install_pending(&app, &device)?;
    Ok(json!({ "installed": true, "log": log }))
}

/// Что запишется на читалку при установке иконки сна (для проверки в «На читалку»): файл, путь от корня читалки, размер, состояние
#[tauri::command]
pub fn sleep_plan(app: tauri::State<App>, device: String, name: String) -> Result<Vec<Value>, String> {
    let root = app.connected_root(&device).ok_or("Читалка не подключена.")?;
    let td = theme_dir(&app, &device)?;
    let md = meta_dir(&work_dir(&app, &device, "sleep")?, "sleep", &clean_name(&name));
    Ok(work_files(&app, &device, &name)?.into_iter().map(|(f, b)| {
        let dst = td.join(&f);
        // та же иконка — только если совпадает содержимое (иконки одного разрешения всегда одного размера)
        let state = match std::fs::read(&dst) { Ok(x) if x == b => "same", Ok(_) => "changed", Err(_) => "new" };
        json!({ "src": md.join(&f), "dst": dst.strip_prefix(&root).map(|p| p.display().to_string().replace('\\', "/")).unwrap_or_default(), "size": b.len(), "state": state })
    }).collect())
}

/// Иконка, ждавшая подключения в прежних версиях: снять отметку (пункт уже переложен в «На читалку»)
#[tauri::command]
pub fn sleep_unpend(app: tauri::State<App>, device: String) -> Result<(), String> {
    let mut d = app.device(&device)?;
    d.sleep.pending = None;
    app.store().save(&d).map_err(|e| e.to_string())
}

/// Иконка, отмеченная к установке (pending), — на читалку; вызывается только из sleep_install
pub fn install_pending(app: &App, device: &str) -> Result<Vec<String>, String> {
    let store = app.store();
    let mut d = app.device(device)?;
    let Some(name) = d.sleep.pending.clone() else { return Ok(vec![]) };
    let files = work_files(app, device, &name)?;
    let mut log = move_reader_backups(app, device);
    let bk = backup_dir(app, device);
    log.extend(install(&theme_dir(app, device)?, &files, app.behavior("backup").then_some(bk.as_path()), &pb_core::util::stamp())?);
    d.sleep.pending = None; d.sleep.installed = Some(name); d.sleep.installed_at = Some(now());
    store.save(&d).map_err(|e| e.to_string())?;
    Ok(log)
}

/// Резервные копии иконки (на компьютере), новые сверху; копии, ещё лежащие на читалке, сначала переносятся сюда
#[tauri::command]
pub fn sleep_backups(app: tauri::State<App>, device: String) -> Result<Vec<Value>, String> {
    let dir = backup_dir(&app, &device);
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(vec![]) };
    let mut v: Vec<Value> = rd.flatten().filter(|e| e.path().is_dir()).map(|e| {
        let files: Vec<String> = std::fs::read_dir(e.path()).map(|r| r.flatten().map(|f| f.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
        json!({ "stamp": e.file_name().to_string_lossy(), "files": files, "thumb": backup_thumb(&e.path()) })
    }).collect();
    v.sort_by(|a, b| b["stamp"].as_str().cmp(&a["stamp"].as_str()));
    Ok(v)
}

/// Предпросмотр копии: портретная иконка (или любая картинка из копии), уменьшенная до 240 px по высоте, PNG data URL
fn backup_thumb(dir: &Path) -> Option<String> {
    let p = ["taskmgr_lock_background.bmp", "taskmgr_lock_background_landscape.bmp"].iter().map(|n| dir.join(n)).find(|p| p.exists())
        .or_else(|| std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().map(|x| x.eq_ignore_ascii_case("bmp") || x.eq_ignore_ascii_case("png")).unwrap_or(false)))?;
    let img = image::open(&p).ok()?;
    let img = if img.height() > 240 { img.resize(u32::MAX, 240, image::imageops::FilterType::Triangle) } else { img };
    let mut out = std::io::Cursor::new(vec![]);
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", enc(&out.into_inner())))
}

/// Вернуть иконку из резервной копии (stamp) или стандартную от PocketBook (stamp = "default": наши файлы убираются).
/// Текущая иконка перед этим сама уходит в резерв.
#[tauri::command]
pub fn sleep_restore(app: tauri::State<App>, device: String, stamp: String) -> Result<Vec<String>, String> {
    let dir = theme_dir(&app, &device)?;
    let bk = backup_dir(&app, &device);
    let store = app.store();
    let mut d = app.device(&device)?;
    let mut pre = move_reader_backups(&app, &device);
    let log = if stamp == "default" {
        let names = ["taskmgr_lock_background.bmp", "taskmgr_lock_background_landscape.bmp"];
        let now_stamp = pb_core::util::stamp();
        let mut log = vec![];
        for n in names {
            let p = dir.join(n);
            if p.exists() {
                let b = bk.join(&now_stamp).join(n);
                std::fs::create_dir_all(b.parent().unwrap()).map_err(|e| e.to_string())?;
                std::fs::copy(&p, &b).map_err(|e| e.to_string())?; std::fs::remove_file(&p).map_err(|e| e.to_string())?;
                log.push(format!("{n} → копия на компьютере ({now_stamp})"));
            }
        }
        d.sleep.installed = None;
        log
    } else {
        let src = bk.join(clean_name(&stamp));
        let files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&src).map_err(|_| "Такой резервной копии нет.")?.flatten()
            .filter_map(|e| std::fs::read(e.path()).ok().map(|b| (e.file_name().to_string_lossy().to_string(), b))).collect();
        d.sleep.installed = Some(format!("резервная копия {stamp}"));
        install(&dir, &files, Some(&bk), &pb_core::util::stamp())?
    };
    pre.extend(log); let log = pre;
    d.sleep.installed_at = Some(now());
    d.sleep.pending = None;
    store.save(&d).map_err(|e| e.to_string())?;
    Ok(log)
}

/* ================================================================ загрузка на читалку */
// Источник — папка готового на компьютере (PB743K3\Books …), назначение — папка на читалке из настроек.
// Копируются новые файлы и те, что отличаются по размеру; одинаковые пропускаются.

/* ================================================================ загрузки на читалку */

#[derive(Deserialize)]
pub struct QItem { src: PathBuf, dst: String }

/// Проверить список загрузок: каждый файл, папка на читалке, что там уже есть, сколько места нужно и свободно
#[tauri::command]
pub async fn queue_check(h: AppHandle, device: String, items: Vec<QItem>) -> Result<Value, String> {
    let app = h.state::<App>();
    let root = app.connected_root(&device).ok_or("Читалка не подключена.")?;
    for it in &items { rel(&it.dst).or_else(|e| if it.dst.trim_matches(['/', '\\']).is_empty() { Ok(PathBuf::new()) } else { Err(e) })?; }
    let list: Vec<(PathBuf, String)> = items.into_iter().map(|i| (i.src, i.dst)).collect();
    let r2 = root.clone();
    let files = tauri::async_runtime::spawn_blocking(move || pb_core::files::plan(&list, &r2)).await.map_err(|e| e.to_string())?;
    let free = crate::usb::space(&root).map(|s| s.0);
    Ok(json!({ "files": files, "free": free, "root": root }))
}

#[derive(Deserialize)]
pub struct QFile { src: PathBuf, dst: String }
#[derive(Serialize, Clone)]
struct QProgress { done: usize, total: usize, name: String, bytes: u64, all: u64, secs: f64 }

/// Скопировать файлы на читалку по одному, с проверкой размера. События «queue://progress».
#[tauri::command]
pub async fn queue_run(h: AppHandle, device: String, files: Vec<QFile>) -> Result<Value, String> {
    let root = h.state::<App>().connected_root(&device).ok_or("Читалка не подключена.")?;
    for f in &files { rel(&f.dst)?; }
    let cancel = h.state::<App>().jobs.start("queue");
    tauri::async_runtime::spawn_blocking(move || {
        let all: u64 = files.iter().map(|f| std::fs::metadata(&f.src).map(|m| m.len()).unwrap_or(0)).sum();
        let t0 = std::time::Instant::now();
        let (mut done, mut bytes, mut errors, mut ok) = (0, 0u64, vec![], vec![]);
        for f in &files {
            if cancel.is_cancelled() { break }
            let _ = h.emit("queue://progress", QProgress { done, total: files.len(), name: f.dst.clone(), bytes, all, secs: t0.elapsed().as_secs_f64() });
            match copy_checked(&f.src, &root.join(rel(&f.dst).unwrap())) {
                Ok(n) => { bytes += n; ok.push(f.dst.clone()) }
                Err(e) => errors.push(format!("{}: {e}", f.dst)),
            }
            done += 1;
        }
        h.state::<App>().jobs.finish("queue");
        let secs = t0.elapsed().as_secs_f64();
        let _ = h.emit("queue://progress", QProgress { done, total: files.len(), name: String::new(), bytes, all, secs });
        Ok(json!({ "copied": ok, "bytes": bytes, "secs": secs, "errors": errors, "cancelled": cancel.is_cancelled() }))
    }).await.map_err(|e| e.to_string())?
}
