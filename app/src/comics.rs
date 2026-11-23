//! Команды раздела «Комиксы» для окна. Тяжёлое — в отдельных потоках, прогресс — событиями.

use crate::App;
use base64::Engine;
use pb_comics::archive;
use pb_comics::process::{self, RunMode};
use pb_comics::Plan;
use pb_core::folders::Folders;
use pb_core::util::format_size;
use pb_image::page::{convert, PageOpt};
use pb_image::{encode, ops, Pix};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct ComicsState {
    /// план выбранного вида (манга или комиксы); план другого вида ждёт в `parked`
    pub plan: Option<Plan>,
    /// «Манга» и «Комиксы» — две независимые папки: при переключении план откладывается, а не теряется
    pub kind: String,
    pub parked: std::collections::HashMap<String, Plan>,
    /// открытый в предпросмотре том: (номер, страницы, ч/б или цветной)
    pub open: Option<(usize, Arc<Vec<Vec<u8>>>, String)>,
    /// оценки сжатия: «размер|время изменения|настройки» → доля. Переживает переименование и повторный выбор папки.
    pub est: std::collections::HashMap<String, f64>,
}

/// Переключить вид: манга ↔ комиксы. Возвращает отложенный план вида (если папку уже выбирали) — без повторного сканирования.
#[tauri::command]
pub fn comics_use(h: AppHandle, kind: String) -> Result<Value, String> {
    if !["manga", "comics"].contains(&kind.as_str()) { return Err("Неизвестный вид.".into()) }
    app(&h).jobs.cancel("estimate");
    let st = app(&h); let mut s = st.comics.lock().unwrap();
    if s.kind == kind { return Ok(s.plan.as_ref().map(items_json).unwrap_or(Value::Null)) }
    let old = std::mem::replace(&mut s.kind, kind.clone());
    if let Some(p) = s.plan.take() { s.parked.insert(old, p); }
    s.plan = s.parked.remove(&kind); s.open = None;
    Ok(s.plan.as_ref().map(items_json).unwrap_or(Value::Null))
}

/// ключ оценки: том узнаётся по размеру и времени изменения (после переименования они те же)
/// Кэш оценок на диске (Cache/comics-estimates.json рядом с программой): после перезапуска оценка папки берётся из него
/// мгновенно — ключ включает путь, размер, время изменения и настройки, так что изменённые тома пересчитаются.
fn cache_path(root: &std::path::Path) -> PathBuf { root.join("Cache").join("comics-estimates.json") }
pub fn load_cache(root: &std::path::Path) -> std::collections::HashMap<String, f64> {
    std::fs::read_to_string(cache_path(root)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}
/// пора ли сохранить кэш оценок по ходу пересчёта (раз в 5 секунд)
fn save_due() -> bool {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let last = LAST.load(Ordering::Relaxed);
    if now.saturating_sub(last) < 5000 { return false }
    LAST.compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed).is_ok()
}
fn save_cache(h: &AppHandle) {
    let a = app(h);
    let mut m = a.comics.lock().unwrap().est.clone();
    // не больше 50 000 оценок: лишние отбрасываются (HashSet — без квадратичного перебора)
    if m.len() > 50_000 { let keep: std::collections::HashSet<String> = m.keys().take(50_000).cloned().collect(); m.retain(|k, _| keep.contains(k)); }
    let p = cache_path(&a.root);
    if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_string(&m).unwrap_or_default()).is_ok() { let _ = std::fs::rename(&tmp, &p); }
}

fn est_key(path: &std::path::Path, size: u64, tag: &str, opt: &PageOpt) -> String {
    let mt = std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0);
    format!("{size}|{mt}|{tag}|{}", serde_json::to_string(opt).unwrap_or_default())
}

fn app(h: &AppHandle) -> tauri::State<'_, App> { h.state::<App>() }

fn items_json(p: &Plan) -> Value {
    json!({
        "root": p.root, "total": p.total_size(),
        "items": p.items.iter().map(|i| json!({ "rel": i.rel, "name": i.file_name, "newName": i.new_name, "kind": i.kind.label(), "size": i.size, "series": i.parts.series, "vol": i.parts.vol })).collect::<Vec<_>>(),
    })
}

/// настройки страниц для «читалка × режим» — режимы описаны в одном месте (pb-image), окно их не дублирует
#[tauri::command]
pub fn comics_mode_opt(h: AppHandle, reader: String, mode: String) -> Result<PageOpt, String> {
    let rd = app(&h).reader(&reader).ok_or("Нет такой читалки.")?;
    Ok(pb_image::page::mode_opt(&rd, &mode))
}

#[derive(Deserialize)]
pub struct ScanArgs { path: String, recurse: bool, rename: bool, template: String, folders: Folders, #[serde(default)] by_dir: std::collections::HashMap<String, String>,
    /// ещё папки, которые не сканировать (готовое манги и комиксов, если они внутри выбранной папки)
    #[serde(default)] skip: Vec<String> }

#[tauri::command]
pub async fn comics_scan(h: AppHandle, args: ScanArgs) -> Result<Value, String> {
    let root = PathBuf::from(args.path.trim().trim_matches('"'));
    if !root.is_dir() { return Err("Такой папки нет — проверьте путь.".into()) }
    let mut skip = vec![root.join("_backup"), args.folders.out(&root)];
    skip.extend(args.skip.iter().filter(|x| !x.trim().is_empty()).map(PathBuf::from));
    let plan = tauri::async_runtime::spawn_blocking(move || {
        let mut p = Plan::scan(&root, args.recurse, args.rename, &skip);
        p.set_names_by(&args.template, &args.by_dir, args.rename, true); p
    }).await.map_err(|e| e.to_string())?;
    let v = items_json(&plan);
    let st = app(&h); let mut s = st.comics.lock().unwrap(); s.plan = Some(plan); s.open = None;
    Ok(v)
}

#[tauri::command]
pub fn comics_names(h: AppHandle, template: String, rename: bool, by_dir: Option<std::collections::HashMap<String, String>>) -> Result<Value, String> {
    let st = app(&h); let mut s = st.comics.lock().unwrap();
    let p = s.plan.as_mut().ok_or("Сначала выберите папку.")?;
    p.set_names_by(&template, &by_dir.unwrap_or_default(), rename, true);
    Ok(items_json(p))
}

/// Пример названий для окна шаблонов (на своём файле), состояние не меняется
#[tauri::command]
pub fn comics_name_samples(h: AppHandle, templates: Vec<String>, dir: Option<String>) -> Result<Value, String> {
    let st = app(&h); let s = st.comics.lock().unwrap();
    Ok(match s.plan.as_ref().and_then(|p| p.name_samples_in(&templates, dir.as_deref().unwrap_or(""))) { Some((orig, names)) => json!({ "orig": orig, "names": names }), None => Value::Null })
}

fn data_url(p: &Pix, max_h: u32) -> String {
    let p = if p.h > max_h { ops::resize(p, ((p.w as u64 * max_h as u64 / p.h as u64) as u32).max(1), max_h) } else { p.clone() };
    let j = encode::jpeg(&p, 90).unwrap_or_default();
    format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(j))
}

/// страницы тома: из кэша или чтением архива
fn volume(h: &AppHandle, index: usize) -> Result<(Arc<Vec<Vec<u8>>>, String), String> {
    let st = app(h);
    let path = {
        let s = st.comics.lock().unwrap();
        if let Some((i, pages, kind)) = &s.open { if *i == index { return Ok((pages.clone(), kind.clone())) } }
        s.plan.as_ref().and_then(|p| p.items.get(index)).map(|i| i.path.clone()).ok_or("Том не найден.")?
    };
    let c = archive::read(&path, None)?;
    let pages: Vec<Vec<u8>> = c.pages.into_iter().map(|(_, b)| b).collect();
    let kind = process::comic_kind(&pages.iter().map(|b| b.as_slice()).collect::<Vec<_>>());
    let pages = Arc::new(pages);
    st.comics.lock().unwrap().open = Some((index, pages.clone(), kind.clone()));
    Ok((pages, kind))
}

#[derive(Deserialize)]
pub struct PreviewArgs { index: usize, page: usize, opt: PageOpt, readers: Vec<String> }

/// Одна страница: исходник, результат и как он будет выглядеть на выбранных читалках
#[tauri::command]
pub async fn comics_preview(h: AppHandle, args: PreviewArgs) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (pages, kind) = volume(&h, args.index)?;
        if pages.is_empty() { return Err("В этом томе нет страниц.".into()) }
        let n = args.page.min(pages.len() - 1);
        let src = &pages[n];
        let r = convert(src, &args.opt, &kind, true)?;
        let orig = r.orig.as_ref().ok_or("нет картинки")?;
        let res = r.result.as_ref().unwrap_or(orig);
        let st = app(&h);
        let readers: Vec<pb_core::Reader> = args.readers.iter().filter_map(|id| st.reader(id)).collect();
        let sims: serde_json::Map<String, Value> = readers.iter().map(|rd| {
            (rd.id.clone(), Value::String(data_url(&pb_image::sim::simulate(res, rd), rd.screen.height)))
        }).collect();
        let out_len = r.bytes.as_ref().map(|b| b.len()).unwrap_or(src.len());
        Ok(json!({
            "page": n, "pages": pages.len(), "kind": kind,
            "orig": { "url": data_url(orig, 1400), "w": orig.w, "h": orig.h, "format": pb_image::sniff(src), "bytes": src.len() },
            "result": { "kept": r.bytes.is_none(), "bytes": out_len, "ext": if r.bytes.is_some() { r.ext } else { pb_image::sniff(src) }, "levels": r.levels, "gray": r.gray, "w": r.w, "h": r.h, "why": r.why },
            "sims": sims,
        }))
    }).await.map_err(|e| e.to_string())?
}

fn ratio_on_pages(pages: &[Vec<u8>], kind: &str, opt: &PageOpt, k: usize, cancel: &pb_core::jobs::Cancel) -> Option<f64> {
    use rayon::prelude::*;
    let n = pages.len(); if n == 0 { return Some(1.0) }
    let k = k.min(n);
    let idx: Vec<usize> = (0..k).map(|i| ((i as f64 + 0.5) * n as f64 / k as f64) as usize).collect();
    let (a, b) = idx.par_iter().map(|&i| {
        if cancel.is_cancelled() { return (0.0, 0.0) }
        let s = &pages[i];
        let o = convert(s, opt, kind, false).ok().and_then(|r| r.bytes).map(|b| b.len()).unwrap_or(s.len());
        (s.len() as f64, o as f64)
    }).reduce(|| (0.0, 0.0), |x, y| (x.0 + y.0, x.1 + y.1));
    if cancel.is_cancelled() { None } else { Some(if a > 0.0 { b / a } else { 1.0 }) }
}

#[derive(Deserialize)]
pub struct ModeEstArgs { index: usize, reader: String }

/// вес этого тома в каждом из трёх режимов — для карточек режимов
#[tauri::command]
pub async fn comics_mode_estimates(h: AppHandle, args: ModeEstArgs) -> Result<Value, String> {
    let cancel = app(&h).jobs.start("modes");
    tauri::async_runtime::spawn_blocking(move || {
        let (pages, kind) = volume(&h, args.index)?;
        let rd = app(&h).reader(&args.reader).ok_or("Нет такой читалки.")?;
        let (size, path) = app(&h).comics.lock().unwrap().plan.as_ref().and_then(|p| p.items.get(args.index)).map(|i| (i.size, i.path.clone())).unwrap_or_default();
        let mut out = serde_json::Map::new();
        for m in ["safe", "balanced", "max"] {
            let opt = pb_image::page::mode_opt(&rd, m);
            let key = est_key(&path, size, "vol5", &opt);
            let cached = app(&h).comics.lock().unwrap().est.get(&key).copied();
            let r = match cached { Some(r) => r, None => {
                let r = ratio_on_pages(&pages, &kind, &opt, 5, &cancel).ok_or("отменено")?;
                app(&h).comics.lock().unwrap().est.insert(key, r); r
            } };
            out.insert(m.into(), json!({ "ratio": r, "bytes": (size as f64 * r) as u64 }));
        }
        Ok(json!({ "size": size, "modes": out }))
    }).await.map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
pub struct VolEstArgs { index: usize, opt: PageOpt }

#[tauri::command]
pub async fn comics_volume_estimate(h: AppHandle, args: VolEstArgs) -> Result<Value, String> {
    let cancel = app(&h).jobs.start("volume");
    tauri::async_runtime::spawn_blocking(move || {
        let (pages, kind) = volume(&h, args.index)?;
        let r = ratio_on_pages(&pages, &kind, &args.opt, 6, &cancel).ok_or("отменено")?;
        let (size, total, count) = { let a = app(&h); let s = a.comics.lock().unwrap(); let p = s.plan.as_ref().unwrap();
            (p.items.get(args.index).map(|i| i.size).unwrap_or(0), p.total_size(), p.items.len()) };
        Ok(json!({ "ratio": r, "size": size, "bytes": (size as f64 * r) as u64, "folderTotal": total, "folderGuess": (total as f64 * r) as u64, "count": count }))
    }).await.map_err(|e| e.to_string())?
}

/// Оценка всей папки: по 2–5 страниц из каждого тома, тома — параллельно; уже оценённые берутся из памяти.
/// События «comics://estimate» в порядке готовности.
#[tauri::command]
pub async fn comics_estimate_all(h: AppHandle, opt: PageOpt) -> Result<Value, String> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let cancel = app(&h).jobs.start("estimate");
    let items: Vec<(usize, PathBuf, u64)> = { let a = app(&h); let s = a.comics.lock().unwrap(); s.plan.as_ref().ok_or("Сначала выберите папку.")?.items.iter().enumerate().map(|(i, x)| (i, x.path.clone(), x.size)).collect() };
    tauri::async_runtime::spawn_blocking(move || {
        let k = process::samples_for(items.len());
        let all: u64 = items.iter().map(|x| x.2).sum();
        let done = AtomicUsize::new(0);
        let acc = std::sync::Mutex::new((0f64, 0f64));
        let tag = format!("all{k}");
        // сначала то, что уже считали, — эти тома не трогаем
        let (known, todo): (Vec<_>, Vec<_>) = items.iter().map(|(i, p, size)| {
            let key = est_key(p, *size, &tag, &opt);
            let r = app(&h).comics.lock().unwrap().est.get(&key).copied();
            (*i, p.clone(), *size, key, r)
        }).partition(|x| x.4.is_some());
        let report = |i: usize, size: u64, r: f64| {
            let (was, now) = { let mut a = acc.lock().unwrap(); a.0 += size as f64; a.1 += size as f64 * r; *a };
            let n = done.fetch_add(1, Ordering::SeqCst) + 1;
            let guess = now + (all as f64 - was) * if was > 0.0 { now / was } else { 1.0 };
            let _ = h.emit("comics://estimate", json!({ "index": i, "bytes": (size as f64 * r) as u64, "done": n, "total": items.len(), "all": all, "guess": guess as u64 }));
        };
        for (i, _, size, _, r) in &known { report(*i, *size, r.unwrap()) }
        todo.par_iter().for_each(|(i, p, size, key, _)| {
            if cancel.is_cancelled() { return }
            let r = process::estimate_ratio(p, &opt, k).unwrap_or(1.0);
            app(&h).comics.lock().unwrap().est.insert(key.clone(), r);
            report(*i, *size, r);
            // сохранять по ходу: прерванная оценка или закрытие программы не теряют уже посчитанное
            // не чаще раза в 5 секунд: файл оценок бывает большим, а тома считаются параллельно
            if done.load(Ordering::SeqCst).is_multiple_of(25) && save_due() { save_cache(&h) }
        });
        if !todo.is_empty() { save_cache(&h) }
        app(&h).jobs.finish("estimate");
        let (was, now) = *acc.lock().unwrap();
        let guess = now + (all as f64 - was) * if was > 0.0 { now / was } else { 1.0 };
        Ok(json!({ "all": all, "guess": guess as u64, "samples": k, "cancelled": cancel.is_cancelled(),
            "text": format!("{} → ≈ {}", format_size(all as f64), format_size(guess)) }))
    }).await.map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
pub struct CompareArgs { index: usize, page: usize, reader: String }
/// Одна и та же страница во всех трёх режимах: как будет на экране читалки, сколько весит, в каком формате
#[tauri::command]
pub async fn comics_compare(h: AppHandle, args: CompareArgs) -> Result<Value, String> {
    use rayon::prelude::*;
    tauri::async_runtime::spawn_blocking(move || {
        let (pages, kind) = volume(&h, args.index)?;
        if pages.is_empty() { return Err("В этом томе нет страниц.".into()) }
        let n = args.page.min(pages.len() - 1);
        let src = &pages[n];
        let rd = app(&h).reader(&args.reader).ok_or("Нет такой читалки.")?;
        let modes: Vec<(&str, Result<Value, String>)> = ["safe", "balanced", "max"].par_iter().map(|m| {
            let opt = pb_image::page::mode_opt(&rd, m);
            let r = convert(src, &opt, &kind, true).and_then(|r| {
                let orig = r.orig.as_ref().ok_or("нет картинки")?;
                let res = r.result.as_ref().unwrap_or(orig);
                let bytes = r.bytes.as_ref().map(|b| b.len()).unwrap_or(src.len());
                Ok(json!({ "sim": data_url(&pb_image::sim::simulate(res, &rd), rd.screen.height), "file": data_url(res, rd.screen.height),
                    "bytes": bytes, "kept": r.bytes.is_none(), "ext": if r.bytes.is_some() { r.ext } else { pb_image::sniff(src) },
                    "levels": r.levels, "gray": r.gray, "w": r.w, "h": r.h, "why": r.why, "quality": opt.quality }))
            });
            (*m, r)
        }).collect();
        let orig = pb_image::decode(src).map_err(|e| e.to_string())?;
        let mut out = serde_json::Map::new();
        for (m, r) in modes { out.insert(m.into(), r?); }
        Ok(json!({ "page": n, "pages": pages.len(), "kind": kind,
            "orig": { "url": data_url(&orig, rd.screen.height), "bytes": src.len(), "w": orig.w, "h": orig.h, "format": pb_image::sniff(src) },
            "modes": out }))
    }).await.map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
pub struct RunArgs { mode: RunMode, opt: Option<PageOpt>, folders: Folders,
    /// только эти тома (выбранная подпапка); нет — все
    #[serde(default)] only: Option<Vec<usize>>,
    /// пропускать тома, у которых готовая копия уже есть
    #[serde(default)] skip_done: bool }

/// Сохранить: копии в папку готового, на месте с бэкапом или только переименовать. События «comics://progress».
#[tauri::command]
pub async fn comics_run(h: AppHandle, args: RunArgs) -> Result<Value, String> {
    let cancel = app(&h).jobs.start("run");
    let plan = app(&h).comics.lock().unwrap().plan.clone().ok_or("Сначала выберите папку.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let stamp = pb_core::util::stamp();
        let out = args.folders.out(&plan.root);
        let backup = args.folders.backup(&plan.root, plan.root.join("_backup"), &stamp);
        let h2 = h.clone();
        let only: Option<std::collections::HashSet<usize>> = args.only.map(|v| v.into_iter().collect());
        let (ok, fail, skipped) = process::run_some(&plan, args.mode, if args.mode == RunMode::Rename { None } else { args.opt.as_ref() }, &out, &backup, &cancel, &|p| { let _ = h2.emit("comics://progress", &p); }, only.as_ref(), args.skip_done);
        app(&h).jobs.finish("run");
        Ok(json!({ "ok": ok, "fail": fail, "skipped": skipped, "cancelled": cancel.is_cancelled(), "out": out, "backup": backup }))
    }).await.map_err(|e| e.to_string())?
}


/// Какие тома уже сохранены в папку готового (копия есть и не старше исходника) — индексы
#[tauri::command]
pub async fn comics_done(h: AppHandle, out: String) -> Result<Vec<usize>, String> {
    let items: Vec<(PathBuf, String, String)> = app(&h).comics.lock().unwrap().plan.as_ref()
        .map(|p| p.items.iter().map(|i| (i.path.clone(), i.rel.clone(), i.new_name.clone())).collect()).unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        let out = PathBuf::from(out);
        items.iter().enumerate().filter(|(_, (p, rel, name))| process::is_done(p, &out.join(rel).join(name))).map(|(i, _)| i).collect()
    }).await.map_err(|e| e.to_string())
}
