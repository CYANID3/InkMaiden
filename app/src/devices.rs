//! Читалки: следим за дисками, новую сохраняем сами (Readers\PB743K3.json + фото с читалки),
//! при подключении ставим иконку сна, которая ждала подключения. Сведения для «Моего устройства».

use crate::usb::{self, Drive};
use crate::App;
use base64::Engine;
use pb_core::devices::{now, write_mark, Device, DevicePaths, Seen, KINDS};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct UsbState { pub drives: Vec<Drive>, pub seen: Vec<Seen>, photos: HashMap<String, String>, /** диски уже просмотрены хоть раз */ pub scanned: bool,
    /** безопасно извлечённые диски: не считаются подключёнными, пока буква не пропадёт (кабель вынут) */ pub ejected: HashMap<String, String>,
    /** с какого времени подключена читалка (по её id) */ pub since: HashMap<String, i64> }

pub fn png_url(bytes: &[u8]) -> String { format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)) }

/// фото устройства из system/device<код>.ico → PNG
fn ico_png(path: &str) -> Option<Vec<u8>> {
    let b = std::fs::read(path).ok()?;
    let img = image::load_from_memory_with_format(&b, image::ImageFormat::Ico).ok()?;
    let mut out = std::io::Cursor::new(vec![]);
    trim(img).write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// Срезать прозрачные поля вокруг читалки: на значке 256×256 она занимает меньше половины
fn trim(img: image::DynamicImage) -> image::DynamicImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in rgba.enumerate_pixels() { if p[3] > 8 { x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x); y1 = y1.max(y) } }
    if x1 < x0 || y1 < y0 { return img }
    let pad = ((x1 - x0).max(y1 - y0) / 40).max(1);
    let (x0, y0) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
    let (x1, y1) = ((x1 + pad).min(w - 1), (y1 + pad).min(h - 1));
    image::DynamicImage::ImageRgba8(image::imageops::crop_imm(&rgba, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image())
}

/// свежие сведения с читалки, но без потери того, что было известно раньше
fn merge_seen(old: &Seen, new: &Seen) -> Seen {
    let mut s = new.clone();
    macro_rules! keep { ($($f:ident),*) => { $( if s.$f.is_none() { s.$f = old.$f.clone() } )* } }
    keep!(total, free, usb_vendor, usb_product, usb_revision, kernel, language, timezone, night_mode, theme, last_update, photo);
    s
}

/// Подключённая читалка: найти её профиль по метке или сохранить новую (если включено «сохранять сами»)
/// Читалка без метки, а такая модель уже сохранена: после полного сброса метка стирается, и без вопроса получилась бы
/// «новая» PB743K3-2 с пустыми папками. Такую не сохраняем сами — окно спрашивает: это прежняя или другая такая же.
fn needs_claim(app: &App, s: &Seen, busy: &[String]) -> bool {
    s.mark.is_none() && app.store().list().iter().any(|d| d.seen.code == s.code && !busy.contains(&d.id))
}

fn adopt(app: &App, s: &mut Seen) -> Option<Device> {
    let store = app.store();
    if let Some(mut d) = store.match_seen(s) {
        d.last_seen = now();
        d.seen = merge_seen(&d.seen, s);
        let _ = store.save(&d);
        if !store.photo_path(&d.id).exists() { if let Some(png) = s.photo.as_deref().and_then(ico_png) { let _ = std::fs::write(store.photo_path(&d.id), png); } }
        return Some(d);
    }
    if !app.behavior("autoSave") { return None }
    app.models.get(&s.code)?;
    // метка есть, а профиля нет (его забыли) — вернуть то же имя, если свободно
    let id = s.mark.clone().filter(|m| store.get(m).is_none() && m.starts_with("PB")).unwrap_or_else(|| store.new_id(&s.code));
    let d = Device::from_seen(s, &app.models, &id, None)?;
    store.save(&d).ok()?;
    if let Some(png) = s.photo.as_deref().and_then(ico_png) { let _ = std::fs::write(store.photo_path(&id), png); }
    let _ = write_mark(Path::new(&s.root), &d);
    s.mark = Some(id);
    Some(d)
}

/// Раз в 2 секунды смотрим, какие читалки подключены; при изменении — читаем их, сохраняем новые, сообщаем окну.
pub fn watch(h: AppHandle) {
    std::thread::spawn(move || loop {
        let st = h.state::<App>();
        // извлечённый диск забывается, когда его буква пропала — значит, кабель вынули
        let skip: std::collections::HashSet<String> = { let mut u = st.usb.lock().unwrap(); let l = usb::letters(); u.ejected.retain(|r, _| l.contains(r)); u.ejected.keys().cloned().collect() };
        let list = usb::list_except(&skip);
        // первый просмотр — всегда: окно ждёт его, чтобы не показать «Подключите читалку» раньше времени
        let changed = { let u = st.usb.lock().unwrap(); !u.scanned || u.drives != list };
        if changed {
            let mut seen: Vec<Seen> = list.iter().map(|d| usb::inspect(d, &st.models)).collect();
            let busy: Vec<String> = seen.iter().filter_map(|s| s.mark.clone()).collect();
            for s in seen.iter_mut() {
                if needs_claim(&st, s, &busy) { continue }
                // запись на читалку при подключении — только метка (чтобы узнавать её); всё остальное — через «На читалку»
                let _ = adopt(&st, s);
            }
            {
                let mut u = st.usb.lock().unwrap();
                // время подключения; у отключившихся — запомнить, когда отключились
                let ids: Vec<String> = seen.iter().filter_map(|s| s.mark.clone()).collect();
                let gone: Vec<String> = u.since.keys().filter(|k| !ids.contains(k)).cloned().collect();
                for g in gone { u.since.remove(&g); if let Some(mut d) = st.store().get(&g) { d.last_off = Some(now()); let _ = st.store().save(&d); } }
                for i in ids { u.since.entry(i).or_insert_with(now); }
                u.drives = list; u.seen = seen; u.scanned = true;
            }
            let _ = h.emit("devices://changed", ());
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    });
}

fn photo_of(app: &App, d: &Device) -> Option<String> {
    let p = app.store().photo_path(&d.id);
    let key = p.display().to_string();
    let mut u = app.usb.lock().unwrap();
    if let Some(x) = u.photos.get(&key) { return Some(x.clone()) }
    let raw = std::fs::read(&p).ok()?;
    // фото, сохранённые до 0.4, — с полями: срезаем при показе
    let png = image::load_from_memory_with_format(&raw, image::ImageFormat::Png).ok()
        .and_then(|i| { let mut o = std::io::Cursor::new(vec![]); trim(i).write_to(&mut o, image::ImageFormat::Png).ok().map(|_| o.into_inner()) })
        .unwrap_or(raw);
    let url = png_url(&png);
    u.photos.insert(key, url.clone());
    Some(url)
}

fn device_json(app: &App, d: &Device, connected: Option<&Seen>) -> Value {
    let dirs: serde_json::Map<String, Value> = KINDS.iter().map(|(k, _)| (k.to_string(), json!(app.pc_dir(d, k)))).collect();
    let rdirs: serde_json::Map<String, Value> = ["books", "comics", "manga", "wallpapers", "other"].iter().map(|k| (k.to_string(), json!(d.reader_path(k)))).collect();
    let m = app.models.get(&d.code);
    json!({ "id": d.id, "name": d.name, "code": d.code, "verified": d.verified, "remembered": d.remembered, "lastSeen": d.last_seen, "lastOff": d.last_off, "ejected": app.usb.lock().unwrap().ejected.values().any(|v| v == &d.id), "since": app.usb.lock().unwrap().since.get(&d.id).copied(),
        "seen": connected.map(|s| merge_seen(&d.seen, s)).unwrap_or_else(|| d.seen.clone()), "reader": d.reader, "photo": photo_of(app, d),
        "connected": connected.map(|s| s.root.clone()), "paths": d.paths, "dirs": dirs, "readerDirs": rdirs, "firmware": d.firmware, "sleep": d.sleep,
        "details": d.details,
        "model": m.map(|m| json!({ "name": m.full_name(), "display": m.display, "ppi": m.ppi(), "colorPpi": m.color_ppi, "year": m.year, "body": m.body_mm,
            "cpu": m.cpu, "ramMb": m.ram_mb, "storageGb": m.storage_gb, "batteryMah": m.battery_mah, "wifi": m.wifi, "bluetooth": m.bluetooth, "audio": m.audio, "water": m.water })) })
}

/// Сохранённые читалки и что подключено сейчас
#[tauri::command]
pub fn devices_state(app: tauri::State<App>) -> Value {
    let (seen, scanned) = { let u = app.usb.lock().unwrap(); (u.seen.clone(), u.scanned) };
    let devices = app.store().list();
    let list: Vec<Value> = devices.iter().map(|d| device_json(&app, d, seen.iter().find(|s| s.mark.as_deref() == Some(&d.id)))).collect();
    // подключённые, но не сохранённые: модели нет в справочнике или сохранение выключено
    let other: Vec<Value> = seen.iter().filter(|s| s.mark.as_ref().map(|m| !devices.iter().any(|d| &d.id == m)).unwrap_or(true))
        .map(|s| {
            // такие же сохранённые модели, сейчас не подключённые — кандидаты «это она же после сброса»
            let cands: Vec<Value> = if s.mark.is_none() { devices.iter().filter(|d| d.seen.code == s.code && !seen.iter().any(|x| x.mark.as_deref() == Some(&d.id)))
                .map(|d| json!({ "id": d.id, "name": d.name, "lastSeen": d.last_seen })).collect() } else { vec![] };
            json!({ "seen": s, "known": app.models.get(&s.code).is_some(), "model": app.models.get(&s.code).map(|m| m.full_name()), "candidates": cands })
        }).collect();
    json!({ "devices": list, "unknown": other, "scanned": scanned })
}

/// Правка читалки: название, папки, версия прошивки
#[tauri::command]
pub fn device_update(app: tauri::State<App>, id: String, name: Option<String>, paths: Option<DevicePaths>, firmware: Option<String>) -> Result<(), String> {
    let store = app.store();
    let mut d = app.device(&id)?;
    if let Some(n) = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) { d.name = n.clone(); d.reader.name = n }
    if let Some(p) = paths { d.paths = p }
    if let Some(f) = firmware { d.firmware = Some(f.trim().to_string()).filter(|f| !f.is_empty()) }
    store.save(&d).map_err(|e| e.to_string())
}

/// Ответ на вопрос «это прежняя читалка?»: id — прежняя (на неё снова пишется метка, папки и работа остаются),
/// None — другая такая же: сохранить как новую.
#[tauri::command]
pub fn device_claim(app: tauri::State<App>, root: String, id: Option<String>) -> Result<String, String> {
    let st = &*app;
    let mut s = { let u = st.usb.lock().unwrap(); u.seen.iter().find(|x| x.root == root).cloned() }.ok_or("Читалка не подключена.")?;
    let id = match id {
        Some(id) => { let d = st.device(&id)?; write_mark(Path::new(&root), &d).map_err(|e| e.to_string())?; id }
        None => { let d = adopt_new(st, &mut s).ok_or("Не удалось сохранить читалку.")?; d.id }
    };
    rescan(st);
    Ok(id)
}

fn adopt_new(app: &App, s: &mut Seen) -> Option<Device> {
    let store = app.store();
    let id = store.new_id(&s.code);
    let d = Device::from_seen(s, &app.models, &id, None)?;
    store.save(&d).ok()?;
    if let Some(png) = s.photo.as_deref().and_then(ico_png) { let _ = std::fs::write(store.photo_path(&id), png); }
    let _ = write_mark(Path::new(&s.root), &d);
    s.mark = Some(id);
    Some(d)
}

/// Объединить две записи одной читалки (например, PB743K3-2 после сброса → PB743K3): готовые файлы переносятся в папки
/// прежней (совпадающие имена не трогаются и остаются на месте), метка на читалке переписывается, лишняя запись удаляется.
#[tauri::command]
pub fn device_merge(app: tauri::State<App>, from: String, into: String) -> Result<Value, String> {
    if from == into { return Err("Это одна и та же читалка.".into()) }
    let st = &*app;
    let a = st.device(&from)?; let b = st.device(&into)?;
    if a.seen.code != b.seen.code { return Err("Это разные модели — объединить нельзя.".into()) }
    if !st.jobs.active().is_empty() { return Err("Дождитесь окончания текущей операции.".into()) }
    let (moved, kept) = pb_core::files::merge_dirs(&st.root.join(&from), &st.root.join(&into)).map_err(|e| e.to_string())?;
    // очередь загрузок и прочее по id — к прежней
    if let Some(mut q) = st.config.get("queue").as_object().cloned() {
        if let Some(Value::Array(x)) = q.remove(&from) {
            let mut all = q.get(&into).and_then(|v| v.as_array().cloned()).unwrap_or_default(); all.extend(x);
            q.insert(into.clone(), Value::Array(all)); let _ = st.config.set("queue", Value::Object(q));
        }
    }
    if let Some(root) = st.connected_root(&from) { let _ = write_mark(&root, &b); }
    st.store().forget(&from).map_err(|e| e.to_string())?;
    rescan(st);
    Ok(json!({ "moved": moved, "kept": kept }))
}

/// Папки готового без записи читалки той же модели (PB743K3 после «Забыть» или сброса): их можно вернуть этой читалке
fn orphans(app: &App, d: &Device) -> Vec<Value> {
    let base = format!("PB{}", d.seen.code.to_uppercase().replace(|c: char| !c.is_ascii_alphanumeric(), ""));
    let known: Vec<String> = app.store().list().into_iter().map(|x| x.id).collect();
    let Ok(rd) = std::fs::read_dir(&app.root) else { return vec![] };
    let mut v: Vec<Value> = rd.flatten().filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)).filter_map(|e| {
        let n = e.file_name().to_string_lossy().to_string();
        let ok = n == base || n.strip_prefix(&format!("{base}-")).map(|r| r.parse::<u32>().is_ok()).unwrap_or(false);
        if !ok || known.contains(&n) { return None }
        let files = count_files(&e.path(), 0);
        (files > 0).then(|| json!({ "id": n, "files": files }))
    }).collect();
    v.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    v
}
fn count_files(p: &Path, depth: u32) -> u64 {
    std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| match e.file_type() {
        Ok(t) if t.is_file() => 1, Ok(t) if t.is_dir() && depth < 8 => count_files(&e.path(), depth + 1), _ => 0 }).sum()).unwrap_or(0)
}

#[tauri::command]
pub fn device_orphans(app: tauri::State<App>, id: String) -> Result<Vec<Value>, String> { Ok(orphans(&app, &app.device(&id)?)) }

/// Вернуть читалке прежнее имя (папку готового без записи): готовое из её нынешней папки переносится туда
/// (одноимённые файлы не трогаются), запись переименовывается, метка на читалке переписывается.
#[tauri::command]
pub fn device_rename(app: tauri::State<App>, id: String, to: String) -> Result<Value, String> {
    let st = &*app;
    let mut d = st.device(&id)?;
    if !orphans(st, &d).iter().any(|o| o["id"] == to) { return Err(format!("Папки {to} без записи читалки нет.")) }
    if !st.jobs.active().is_empty() { return Err("Дождитесь окончания текущей операции.".into()) }
    let (moved, kept) = pb_core::files::merge_dirs(&st.root.join(&id), &st.root.join(&to)).map_err(|e| e.to_string())?;
    let store = st.store();
    d.id = to.clone(); d.reader.id = to.clone();
    // свои пути, указывавшие в старую папку, — в новую
    let old = st.root.join(&id).display().to_string(); let new = st.root.join(&to).display().to_string();
    for v in d.paths.pc.values_mut() { if v.starts_with(&old) { *v = v.replacen(&old, &new, 1) } }
    store.save(&d).map_err(|e| e.to_string())?;
    let (po, pn) = (store.photo_path(&id), store.photo_path(&to));
    if po.exists() && !pn.exists() { let _ = std::fs::rename(&po, &pn); }
    store.forget(&id).map_err(|e| e.to_string())?;
    if let Some(mut q) = st.config.get("queue").as_object().cloned() { if let Some(x) = q.remove(&id) { q.insert(to.clone(), x); let _ = st.config.set("queue", Value::Object(q)); } }
    if st.config.get("current").as_str() == Some(id.as_str()) { let _ = st.config.set("current", json!(to)); }
    { let mut u = st.usb.lock().unwrap(); for s in u.seen.iter_mut() { if s.mark.as_deref() == Some(id.as_str()) { s.mark = Some(to.clone()) } }
      if let Some(t) = u.since.remove(&id) { u.since.insert(to.clone(), t); } }
    if let Some(root) = st.connected_root(&to) { let _ = write_mark(&root, &d); }
    rescan(st);
    Ok(json!({ "id": to, "moved": moved, "kept": kept }))
}

fn latest_report(dir: &Path) -> Option<(std::path::PathBuf, Value)> {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path())
        .filter(|p| p.file_name().map(|n| { let n = n.to_string_lossy(); n.starts_with("diag") && n.ends_with(".txt") }).unwrap_or(false)).collect();
    files.sort();
    let f = files.pop()?;
    let t = std::fs::read_to_string(&f).ok()?;
    Some((f, pb_core::diag::parse(&t)))
}

/// Загрузить отчёт диагностики из файла (diag.txt с читалки или копия из папки Diagnostics) и показать его у этой читалки
#[tauri::command]
pub fn device_diag_load(app: tauri::State<App>, id: String, path: String) -> Result<Value, String> {
    let t = std::fs::read_to_string(&path).map_err(|e| format!("Не открылся файл: {e}"))?;
    if !t.contains("@@@ ") { return Err("Это не отчёт модуля диагностики InkMaiden.".into()) }
    let g = pb_core::diag::parse(&t);
    let mut d = app.device(&id)?;
    // копия — в папку Diagnostics этой читалки, если файл не оттуда
    let dir = app.root.join(&id).join("Diagnostics");
    let src = std::path::PathBuf::from(&path);
    let file = if src.starts_with(&dir) { src } else {
        let _ = std::fs::create_dir_all(&dir);
        let t0 = g["time"].as_i64().unwrap_or_else(now);
        let f = dir.join(format!("diag-{}.txt", pb_core::util::stamp_at(t0)));
        if !f.exists() { std::fs::write(&f, &t).map_err(|e| e.to_string())?; } f
    };
    let mut det = d.details.clone().unwrap_or_else(|| json!({}));
    det["diag"] = g.clone(); det["diagFile"] = json!(file);
    if let Some(fw) = g["items"].as_array().and_then(|it| it.iter().find(|x| x["label"] == "Прошивка")).and_then(|x| x["value"].as_str()) { d.firmware = Some(fw.to_string()) }
    d.details = Some(det.clone());
    app.store().save(&d).map_err(|e| e.to_string())?;
    Ok(det)
}

/// Забыть читалку: удаляется её файл в Readers (готовые файлы остаются). При подключении она сохранится снова.
#[tauri::command]
pub fn device_forget(app: tauri::State<App>, id: String) -> Result<(), String> {
    app.store().forget(&id).map_err(|e| e.to_string())?;
    rescan(&app);
    Ok(())
}

/// Пересмотреть подключённые диски на следующем шаге наблюдателя: подключённая читалка, которую забыли, сохранится снова
pub fn rescan(app: &App) { app.usb.lock().unwrap().drives.clear() }

#[tauri::command]
pub fn devices_forget_all(app: tauri::State<App>) -> Result<(), String> {
    app.store().forget_all().map_err(|e| e.to_string())?;
    rescan(&app);
    Ok(())
}

/// Подробности с диска подключённой читалки: память по видам файлов, настройки, приложения, библиотека,
/// диск со стороны Windows и отчёт модуля диагностики. Сохраняются в файл читалки — без подключения виден последний снимок.
/// Прогресс — событие «details://progress» { stage, files }.
#[tauri::command]
pub async fn device_details(h: tauri::AppHandle, id: String) -> Result<Value, String> {
    let app = h.state::<App>();
    let Some(root) = app.connected_root(&id) else { return Ok(app.device(&id)?.details.unwrap_or(Value::Null)) };
    app.jobs.start("details"); // чтобы «Безопасное извлечение» подождало
    let diag_dir = app.root.join(&id).join("Diagnostics");
    let auto_remove = app.behavior("diagAutoRemove");
    let h2 = h.clone();
    let v = tauri::async_runtime::spawn_blocking(move || {
        let stage = |s: &str, n: u64| { let _ = h2.emit("details://progress", json!({ "stage": s, "files": n })); };
        stage("Смотрю файлы на читалке", 0);
        let mut v = pb_core::details::read_with(&root, &|n| stage("Смотрю файлы на читалке", n));
        stage("Читаю библиотеку", 0);
        v["library"] = pb_core::library::read(&root);
        stage("Спрашиваю Windows о диске", 0);
        v["disk"] = crate::usb::disk(&root);
        // отчёт модуля: копия с датой в папку читалки на компьютере; убрать модуль с читалки — пунктом в «На читалку»
        if let Some(l) = pb_core::diag::unfinished(&root) { v["diagUnfinished"] = json!(l); }
        // заводской рабочий стол из отчёта — для «Рабочего стола» (точный список приложений)
        let _ = pb_core::diag::collect_desktop(&root, &diag_dir.parent().unwrap_or(&diag_dir).join("Desktop").join("system"));
        match pb_core::diag::collect(&root, &diag_dir) {
            Ok(Some((file, d))) => {
                stage("Забираю отчёт диагностики", 0);
                v["diag"] = d; v["diagFile"] = json!(file); v["diagNew"] = json!(true);
                v["diagAutoRemove"] = json!(auto_remove);
            }
            Ok(None) => {}
            Err(e) => v["diagError"] = json!(e.to_string()),
        }
        v["diagInstalled"] = json!(root.join(pb_core::diag::APP).exists());
        v
    }).await.map_err(|e| e.to_string());
    let app = h.state::<App>();
    app.jobs.finish("details");
    let mut v = v?;
    let mut d = app.device(&id)?;
    // прошивка заполняется сама: из отчёта модуля или из system/config/firmware (ручной ввод не затираем, если источник молчит)
    let fw = v.get("diag").and_then(|g| g["items"].as_array()).and_then(|it| it.iter().find(|x| x["label"] == "Прошивка")).and_then(|x| x["value"].as_str()).map(String::from)
        .or_else(|| v.get("firmware").and_then(|f| f.as_str()).map(String::from));
    if let Some(f) = fw { d.firmware = Some(f) }
    // нового отчёта нет — остаётся прежний; если и его нет в записи (например, запись создана заново), берётся
    // самый свежий файл из папки Diagnostics этой читалки
    if v.get("diag").map(|x| x.is_null()).unwrap_or(true) {
        if let Some(old) = d.details.as_ref().filter(|o| o.get("diag").map(|x| !x.is_null()).unwrap_or(false)) { for k in ["diag", "diagFile"] { if let Some(x) = old.get(k) { v[k] = x.clone() } } }
        else if let Some((f, g)) = latest_report(&h.state::<App>().root.join(&id).join("Diagnostics")) { v["diag"] = g; v["diagFile"] = json!(f); }
    }
    d.details = Some(v.clone());
    app.store().save(&d).map_err(|e| e.to_string())?;
    Ok(v)
}

/// Поставить модуль диагностики на читалку: один файл applications/InkMaiden_Diag.app (shell-скрипт, только чтение системы)
#[tauri::command]
pub fn diag_install(app: tauri::State<App>, id: String) -> Result<(), String> {
    let root = app.connected_root(&id).ok_or("Читалка не подключена.")?;
    pb_core::files::write_checked(&root.join(pb_core::diag::APP), pb_core::diag::SCRIPT.as_bytes())
}

/// Удалить модуль диагностики с читалки: его файл и папку system/inkmaiden (отчёт уже на компьютере)
#[tauri::command]
pub fn diag_remove(app: tauri::State<App>, id: String) -> Result<(), String> {
    let root = app.connected_root(&id).ok_or("Читалка не подключена.")?;
    // отчёт, который ещё не забрали, сначала копируется на компьютер — чтобы не пропал вместе с модулем
    pb_core::diag::collect(&root, &app.root.join(&id).join("Diagnostics")).map_err(|e| e.to_string())?;
    let _ = pb_core::diag::collect_desktop(&root, &app.root.join(&id).join("Desktop").join("system"));
    pb_core::diag::remove(&root).map(|_| ()).map_err(|e| e.to_string())
}

/// Безопасно извлечь читалку. Пока идёт загрузка или чтение диска — нельзя.
#[tauri::command]
pub async fn device_eject(h: tauri::AppHandle, id: String) -> Result<(), String> {
    let app = h.state::<App>();
    let root = app.connected_root(&id).ok_or("Читалка не подключена.")?;
    let busy: Vec<String> = app.jobs.active().into_iter().filter(|j| ["queue", "details"].contains(&j.as_str())).collect();
    if !busy.is_empty() { return Err(format!("Сейчас идёт {} — дождитесь окончания.", if busy.iter().any(|b| b == "queue") { "загрузка на читалку" } else { "чтение диска читалки" })) }
    let r2 = root.clone();
    tauri::async_runtime::spawn_blocking(move || crate::usb::eject(&r2)).await.map_err(|e| e.to_string())??;
    // после извлечения диск больше не трогаем (обращение подключило бы том снова) — читалка считается отключённой
    // сразу считать читалку отключённой и сообщить окну — не ждать следующего просмотра дисков
    {
        let app = h.state::<App>();
        let r = root.display().to_string();
        let mut u = app.usb.lock().unwrap();
        u.ejected.insert(r.clone(), id.clone());
        u.seen.retain(|s| s.root != r);
        u.drives.retain(|d| d.root.display().to_string() != r);
        u.since.remove(&id);
        drop(u);
        if let Some(mut d) = app.store().get(&id) { d.last_off = Some(now()); let _ = app.store().save(&d); }
    }
    let _ = h.emit("devices://changed", ());
    Ok(())
}
