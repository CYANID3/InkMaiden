//! «Рабочий стол»: какие приложения видны в «Приложениях» читалки и на главном экране, их названия, значки, группы.
//! Файлы на читалке: system/config/desktop/view.json (пользовательская копия; если её нет — прошивка берёт заводскую
//! из /ebrmain, по USB её не видно) и system/config/apps.json (панель главного экрана).
//! Чтение — здесь; запись — только из «На читалку» (desktop_apply / desktop_restore / desktop_reset).
//! Правки хранятся как план и накладываются на файл, который лежит на читалке в момент записи: если на самой читалке
//! что-то поменяли после снимка, это не затрётся. Перед записью прежние файлы копируются на компьютер
//! (<папка программы>\<читалка>\Desktop_backups\<время>).
use crate::App;
use base64::Engine;
use pb_core::desktop::{app_files, apply_panel, apply_plan, default_view, Plan};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

const VIEW: &str = "system/config/desktop/view.json";
const PANEL: &str = "system/config/apps.json";

fn pc_dir(app: &App, device: &str) -> PathBuf { app.root.join(device).join("Desktop") }
/// заводской рабочий стол из отчёта модуля диагностики (точный: из /ebrmain самой читалки)
fn sys_dir(app: &App, device: &str) -> PathBuf { pc_dir(app, device).join("system") }
fn sys_info(dir: &Path) -> Value {
    let view = read_json(&dir.join("view.json"));
    let db = read_json(&dir.join("apps_db.json"));
    let bins: Vec<String> = std::fs::read_to_string(dir.join("bin.txt")).map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default();
    // проверка модуля на самой читалке: путь программы → есть ли файл
    let mut paths = Map::new();
    if let Ok(t) = std::fs::read_to_string(dir.join("paths.txt")) {
        for l in t.lines() { if let Some((p, v)) = l.rsplit_once('\t') { paths.insert(p.to_string(), json!(v.trim() == "1")); } }
    }
    // значения ключей из настроек читалки (flags.txt, например have_pbcloud — условие has_cloud в каталоге)
    let mut flags = Map::new();
    if let Ok(t) = std::fs::read_to_string(dir.join("flags.txt")) {
        for l in t.lines() { if let Some((k, v)) = l.split_once('=') { flags.insert(k.trim().to_string(), json!(v.trim())); } }
    }
    let mut lang = Map::new();
    if let Ok(t) = std::fs::read_to_string(dir.join("lang.txt")) {
        for l in t.lines() { if let Some((k, v)) = l.split_once('=') { if k.starts_with('@') { lang.insert(k.to_string(), json!(v.trim())); } } }
    }
    let at = std::fs::metadata(dir.join("view.json")).or_else(|_| std::fs::metadata(dir.join("apps_db.json"))).and_then(|m| m.modified()).ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
    json!({ "known": view.is_some() || db.is_some(), "view": view, "db": db, "bins": bins, "paths": paths, "flags": flags, "lang": lang, "at": at })
}
fn backups(app: &App, device: &str) -> PathBuf { app.root.join(device).join("Desktop_backups") }

fn read_json(p: &Path) -> Option<Value> {
    let b = std::fs::read(p).ok()?;
    let s = String::from_utf8_lossy(b.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&b)).to_string();
    serde_json::from_str(&s).ok()
}
/// запись без потерь: во временный файл, затем переименование
fn write_atomic(p: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())? }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, data).map_err(|e| format!("{}: {e}", p.display()))?;
    std::fs::rename(&tmp, p).map_err(|e| format!("{}: {e}", p.display()))
}
/// записать и сразу прочитать обратно: если файл не читается как JSON — вернуть прежний из копии (или убрать новый)
fn write_checked(p: &Path, data: &[u8], backup: &Path) -> Result<(), String> {
    write_atomic(p, data)?;
    let ok = std::fs::read(p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).is_some();
    if ok { return Ok(()) }
    match std::fs::read(backup) { Ok(b) => { write_atomic(p, &b)?; } Err(_) => { let _ = std::fs::remove_file(p); } }
    Err(format!("{}: записанный файл не прочитался — вернул прежний, ничего не изменилось.", p.display()))
}
fn pretty(v: &Value) -> Vec<u8> {
    // как у прошивки: отступ 4 пробела, UTF-8 без BOM
    let mut out = vec![];
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    serde::Serialize::serialize(v, &mut ser).ok();
    out.push(b'\n');
    out
}

/// Значок с читалки (BMP) → data URL для показа
fn icon_url(root: &Path, rel: &str) -> Option<String> {
    let p = if rel.starts_with('/') { root.join(rel.trim_start_matches("/mnt/ext1/")) } else { root.join(rel) };
    let img = image::open(&p).ok()?;
    let mut out = std::io::Cursor::new(vec![]);
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner())))
}

/// Что сейчас на рабочем столе. Читалка подключена — с неё (и снимок на компьютер), иначе — последний снимок.
/// { source: 'reader'|'snapshot'|'none', view, custom (была ли своя копия view.json), panel, files: [приложения в applications], icons: { путь: dataURL } }
#[tauri::command]
pub async fn desktop_read(h: tauri::AppHandle, device: String) -> Result<Value, String> {
    use tauri::Manager;
    let app = h.state::<App>();
    let root = app.connected_root(&device);
    let snap = pc_dir(&app, &device);
    let sys = sys_dir(&app, &device);
    tauri::async_runtime::spawn_blocking(move || {
        let (base, source) = match &root { Some(r) => (r.clone(), "reader"), None => (snap.join("reader"), "snapshot") };
        if source == "snapshot" && !base.exists() { return Ok(json!({ "source": "none", "sys": sys_info(&sys) })) }
        let view = read_json(&base.join(VIEW));
        let panel = read_json(&base.join(PANEL));
        // свои приложения на читалке (applications/*.app)
        let mut files = vec![];
        if let Ok(rd) = std::fs::read_dir(base.join("applications")) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.to_lowercase().ends_with(".app") && e.file_type().map(|t| t.is_file()).unwrap_or(false) { files.push(n) }
            }
        }
        files.sort_by_key(|n| n.to_lowercase());
        // значки своих приложений
        let mut icons = Map::new();
        if let Some(apps) = view.as_ref().and_then(|v| v.get("applications")).and_then(|a| a.as_object()) {
            for a in apps.values() {
                for k in ["icon", "focused_icon"] {
                    if let Some(p) = a.get(k).and_then(|i| i.get("path")).and_then(|p| p.as_str()) {
                        if !icons.contains_key(p) { if let Some(u) = icon_url(&base, p) { icons.insert(p.to_string(), json!(u)); } }
                    }
                }
            }
        }
        // снимок на компьютер — чтобы видеть и без читалки (только сами файлы настроек и список приложений)
        if let Some(r) = &root {
            let s = snap.join("reader");
            for rel in [VIEW, PANEL] {
                let (src, dst) = (r.join(rel), s.join(rel));
                if src.exists() { let _ = std::fs::create_dir_all(dst.parent().unwrap()); let _ = std::fs::copy(&src, &dst); } else { let _ = std::fs::remove_file(&dst); }
            }
            let ad = s.join("applications");
            let _ = std::fs::remove_dir_all(&ad); let _ = std::fs::create_dir_all(ad.join("icons"));
            for f in &files { let _ = std::fs::write(ad.join(f), b""); }
            for p in icons.keys() { let rel = p.trim_start_matches("/mnt/ext1/"); let src = r.join(rel); let dst = s.join(rel);
                if src.exists() { let _ = std::fs::create_dir_all(dst.parent().unwrap()); let _ = std::fs::copy(&src, &dst); } }
        }
        // своей копии нет — показываем точный заводской (из отчёта модуля), иначе примерный
        let si = sys_info(&sys);
        let custom = view.is_some();
        let view = view.or_else(|| si["view"].as_object().map(|_| si["view"].clone()));
        let exact = custom || view.is_some();
        Ok(json!({ "source": source, "custom": custom, "exact": exact, "view": view.unwrap_or_else(default_view), "panel": panel, "files": files, "icons": icons, "sys": si }))
    }).await.map_err(|e| e.to_string())?
}

/// копия прежних файлов на компьютер перед записью
fn backup(app: &App, device: &str, root: &Path) -> Result<String, String> {
    let stamp = pb_core::util::stamp();
    let dir = backups(app, device).join(&stamp);
    let mut any = false;
    for rel in [VIEW, PANEL] {
        let src = root.join(rel);
        if src.exists() { std::fs::create_dir_all(dir.join(rel).parent().unwrap()).map_err(|e| e.to_string())?; std::fs::copy(&src, dir.join(rel)).map_err(|e| e.to_string())?; any = true; }
    }
    if !any { std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?; std::fs::write(dir.join("заводской.txt"), "Своей копии view.json не было — читалка показывала заводской рабочий стол.").ok(); }
    Ok(stamp)
}

/// Записать план на читалку (вызывается только из «На читалку»)
#[tauri::command]
pub fn desktop_apply(app: tauri::State<App>, device: String, plan: Plan) -> Result<Vec<String>, String> {
    let root = app.connected_root(&device).ok_or("Читалка не подключена.")?;
    let stamp = backup(&app, &device, &root)?;
    let mut log = vec![format!("прежние файлы — копия на компьютере ({stamp})")];
    if !plan.apps.is_empty() {
        // основа — файл на читалке; если своей копии нет — точный заводской из отчёта модуля; по догадке не пишем
        let mut view = match read_json(&root.join(VIEW)) {
            Some(v) => v,
            None => read_json(&sys_dir(&app, &device).join("view.json"))
                .ok_or("Заводской рабочий стол этой читалки неизвестен — сначала снимите отчёт модулем диагностики («Рабочий стол» → «Узнать точно»).")?,
        };
        // удалить свои приложения: файл и значки — сначала копия на компьютер (в ту же резервную копию), потом с читалки
        let bk = backups(&app, &device).join(&stamp);
        for op in plan.apps.iter().filter(|o| o.uninstall) {
            for rel in app_files(&view, &op.key, op.path.as_deref()) {
                let src = root.join(&rel);
                if !src.is_file() { continue }
                let dst = bk.join(&rel);
                std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
                std::fs::copy(&src, &dst).map_err(|e| format!("{rel}: {e}"))?;
                std::fs::remove_file(&src).map_err(|e| format!("{rel}: {e}"))?;
                log.push(format!("{rel} удалён (копия — на компьютере)"));
            }
        }
        apply_plan(&mut view, &plan);
        write_checked(&root.join(VIEW), &pretty(&view), &backups(&app, &device).join(&stamp).join(VIEW))?;
        log.push(format!("{VIEW}: изменений {}", plan.apps.len()));
    }
    if let Some(keys) = &plan.panel {
        let p = apply_panel(read_json(&root.join(PANEL)), keys);
        write_checked(&root.join(PANEL), &pretty(&p), &backups(&app, &device).join(&stamp).join(PANEL))?;
        log.push(format!("{PANEL}: на главном экране {}", keys.len()));
    }
    Ok(log)
}

/// Резервные копии на компьютере: [{ stamp, files: [..], factory }]
#[tauri::command]
pub fn desktop_backups(app: tauri::State<App>, device: String) -> Vec<Value> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(backups(&app, &device)) {
        for e in rd.flatten() {
            let p = e.path(); if !p.is_dir() { continue }
            let files: Vec<&str> = [VIEW, PANEL].into_iter().filter(|r| p.join(r).exists()).collect();
            out.push(json!({ "stamp": e.file_name().to_string_lossy(), "files": files, "factory": p.join("заводской.txt").exists() }));
        }
    }
    out.sort_by(|a, b| b["stamp"].as_str().cmp(&a["stamp"].as_str()));
    out
}

/// Вернуть из копии (stamp) или к заводскому ("factory": своя копия view.json удаляется) — только из «На читалку»
#[tauri::command]
pub fn desktop_restore(app: tauri::State<App>, device: String, stamp: String) -> Result<Vec<String>, String> {
    let root = app.connected_root(&device).ok_or("Читалка не подключена.")?;
    let now = backup(&app, &device, &root)?;
    let mut log = vec![format!("текущие файлы — копия на компьютере ({now})")];
    if stamp == "factory" {
        if root.join(VIEW).exists() { std::fs::remove_file(root.join(VIEW)).map_err(|e| e.to_string())?; log.push(format!("{VIEW} удалён — читалка возьмёт заводской")); }
        return Ok(log)
    }
    let src = backups(&app, &device).join(stamp.replace(['/', '\\', '.'], ""));
    if !src.is_dir() { return Err("Такой резервной копии нет.".into()) }
    if src.join("заводской.txt").exists() && !src.join(VIEW).exists() {
        if root.join(VIEW).exists() { std::fs::remove_file(root.join(VIEW)).map_err(|e| e.to_string())?; }
        log.push("рабочий стол — заводской, как было тогда".into());
    }
    for rel in [VIEW, PANEL] {
        if let Ok(b) = std::fs::read(src.join(rel)) { write_atomic(&root.join(rel), &b)?; log.push(format!("{rel} возвращён")); }
    }
    // удалённые тогда приложения и значки — обратно
    fn walk(p: &Path, out: &mut Vec<PathBuf>) { if let Ok(rd) = std::fs::read_dir(p) { for e in rd.flatten() { let q = e.path(); if q.is_dir() { walk(&q, out) } else { out.push(q) } } } }
    let mut files = vec![]; walk(&src.join("applications"), &mut files);
    for f in files {
        let rel = f.strip_prefix(&src).unwrap().to_path_buf();
        let dst = root.join(&rel);
        if !dst.exists() { std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?; std::fs::copy(&f, &dst).map_err(|e| e.to_string())?;
            log.push(format!("{} возвращён", rel.to_string_lossy().replace('\\', "/"))); }
    }
    Ok(log)
}

/// Значок для своего приложения: BMP (уже готовые байты из окна) → в папку программы, для переноса в applications/icons
#[tauri::command]
pub fn desktop_icon_save(app: tauri::State<App>, device: String, name: String, icon: String, focused: String) -> Result<Vec<String>, String> {
    let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
    if name.is_empty() { return Err("Пустое имя значка.".into()) }
    let dir = pc_dir(&app, &device).join("icons");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dec = |s: &str| base64::engine::general_purpose::STANDARD.decode(s.rsplit(',').next().unwrap_or("")).map_err(|e| e.to_string());
    let (a, b) = (dir.join(format!("{name}.bmp")), dir.join(format!("{name}_f.bmp")));
    std::fs::write(&a, dec(&icon)?).map_err(|e| e.to_string())?;
    std::fs::write(&b, dec(&focused)?).map_err(|e| e.to_string())?;
    Ok(vec![a.to_string_lossy().into(), b.to_string_lossy().into()])
}

