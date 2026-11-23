//! Сборка CBZ, оценка экономии и запуск по всему плану.

use crate::archive::{self, Kind};
use crate::comicinfo::{self, Meta};
use crate::plan::{Item, Plan};
use pb_core::jobs::Cancel;
use pb_image::page::{convert, PageOpt};
use rayon::prelude::*;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

/// ч/б это том или цветной: по 8 страницам из разных мест
pub fn comic_kind(pages: &[&[u8]]) -> String {
    let n = pages.len(); if n == 0 { return "color".into() }
    let k = n.min(8);
    let picks: Vec<&[u8]> = (0..k).map(|i| pages[((i as f64 + 0.5) * n as f64 / k as f64) as usize]).collect();
    let res: Vec<bool> = picks.par_iter().filter_map(|b| pb_image::decode(b).ok()).map(|p| pb_image::ops::is_gray(&p)).collect();
    if !res.is_empty() && res.iter().filter(|g| **g).count() as f64 / res.len() as f64 >= 0.6 { "bw".into() } else { "color".into() }
}

fn sample_idx(n: usize, k: usize) -> Vec<usize> { let k = k.min(n); (0..k).map(|i| ((i as f64 + 0.5) * n as f64 / k as f64) as usize).collect() }

#[derive(Clone, Debug, Default, Serialize)]
pub struct Built { pub pages: usize, pub gray: usize, pub in_bytes: u64, pub out_bytes: u64 }

/// Пересобрать архив в CBZ по настройкам (`opt` = None — без обработки картинок, только порядок и названия)
pub fn build_cbz(src: &Path, dst: &Path, opt: Option<&PageOpt>, meta: &Meta, cancel: &Cancel) -> Result<Built, String> {
    let content = archive::read(src, None)?;
    if content.pages.is_empty() { return Err("внутри нет картинок".into()) }
    let kind = match opt { Some(o) if o.gray == "auto" && o.keep_color_pages => comic_kind(&content.pages.iter().map(|(_, b)| b.as_slice()).collect::<Vec<_>>()), _ => String::new() };
    let results: Vec<Result<(Vec<u8>, &'static str, bool), String>> = content.pages.par_iter().map(|(name, bytes)| {
        if cancel.is_cancelled() { return Err("остановлено".into()) }
        let orig_ext: &'static str = match pb_image::sniff(bytes) { "jpg" => "jpg", "png" => "png", "gif" => "gif", "bmp" => "bmp", "tiff" => "tif", _ => "" };
        match opt {
            Some(o) => {
                let r = convert(bytes, o, &kind, false).map_err(|e| format!("{name}: {e}"))?;
                match r.bytes { Some(b) => Ok((b, r.ext, r.gray)), None => Ok((bytes.clone(), if orig_ext.is_empty() { "jpg" } else { orig_ext }, false)) }
            }
            None => {
                if orig_ext.is_empty() { let r = convert(bytes, &PageOpt { max_height: 0, squeeze: false, crop: false, clean: 0, sharpen: 0, color_fix: false, gray: "off".into(), ..PageOpt::default() }, "", false).map_err(|e| format!("{name}: {e}"))?; return Ok((r.bytes.unwrap_or_default(), r.ext, false)) }
                Ok((bytes.clone(), orig_ext, false))
            }
        }
    }).collect();
    if cancel.is_cancelled() { return Err("остановлено".into()) }
    let tmp = dst.with_extension("cbz.tmp");
    if let Some(p) = dst.parent() { std::fs::create_dir_all(p).map_err(|e| e.to_string())? }
    let mut built = Built { pages: content.pages.len(), in_bytes: src.metadata().map(|m| m.len()).unwrap_or(0), ..Default::default() };
    let wr = (|| -> Result<(), String> {
        let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut z = zip::ZipWriter::new(std::io::BufWriter::new(f));
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (i, r) in results.into_iter().enumerate() {
            let (b, ext, g) = r?; if g { built.gray += 1 }
            z.start_file(format!("{:04}.{}", i + 1, ext), o).map_err(|e| e.to_string())?; z.write_all(&b).map_err(|e| e.to_string())?;
        }
        z.start_file("ComicInfo.xml", o).map_err(|e| e.to_string())?;
        z.write_all(&comicinfo::build(content.comic_info.as_deref(), meta, built.pages)).map_err(|e| e.to_string())?;
        z.finish().map_err(|e| e.to_string())?; Ok(())
    })();
    if let Err(e) = wr { let _ = std::fs::remove_file(&tmp); return Err(e) }
    std::fs::rename(&tmp, dst).map_err(|e| e.to_string())?;
    built.out_bytes = dst.metadata().map(|m| m.len()).unwrap_or(0);
    Ok(built)
}

/// Доля «станет / было» для архива по `k` страницам (ничего не меняет)
pub fn estimate_ratio(src: &Path, opt: &PageOpt, k: usize) -> Result<f64, String> {
    let n = archive::page_count(src); if n == 0 { return Ok(1.0) }
    let idx = sample_idx(n, k);
    let c = archive::read(src, Some(&idx))?;
    let picked: Vec<&[u8]> = idx.iter().filter_map(|&i| c.pages.get(i)).map(|(_, b)| b.as_slice()).filter(|b| !b.is_empty()).collect();
    let kind = if opt.gray == "auto" && opt.keep_color_pages { comic_kind(&picked) } else { String::new() };
    let (was, now) = picked.par_iter().map(|b| {
        let r = convert(b, opt, &kind, false).ok().and_then(|r| r.bytes).map(|x| x.len()).unwrap_or(b.len());
        (b.len() as f64, r as f64)
    }).reduce(|| (0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
    Ok(if was > 0.0 { now / was } else { 1.0 })
}

pub fn samples_for(archives: usize) -> usize { (600 / archives.max(1)).clamp(2, 5) }

fn meta_of(it: &Item) -> Meta {
    Meta { title: Path::new(&it.new_name).file_stem().unwrap().to_string_lossy().to_string(), series: it.parts.series.clone(), volume: it.parts.vol.clone(), number: it.parts.ch.clone() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode { Copies, InPlace, Rename }

#[derive(Clone, Debug, Serialize)]
pub struct Progress { pub index: usize, pub total: usize, pub name: String, pub status: String, pub ok: bool, pub in_bytes: u64, pub out_bytes: u64 }

/// Обработать весь план. `out` — папка готовых копий, `backup` — папка оригиналов (для «на месте»).
pub fn run(plan: &Plan, mode: RunMode, opt: Option<&PageOpt>, out: &Path, backup: &Path, cancel: &Cancel, report: &dyn Fn(Progress)) -> (usize, usize) {
    let (ok, fail, _) = run_some(plan, mode, opt, out, backup, cancel, report, None, false); (ok, fail)
}

/// Как `run`, но только тома `only` (номера в плане) и, если `skip_done`, без томов, у которых готовая копия уже есть
/// и не старше исходника. Возвращает (сделано, ошибок, пропущено как готовые).
#[allow(clippy::too_many_arguments)]
pub fn run_some(plan: &Plan, mode: RunMode, opt: Option<&PageOpt>, out: &Path, backup: &Path, cancel: &Cancel, report: &dyn Fn(Progress),
    only: Option<&std::collections::HashSet<usize>>, skip_done: bool) -> (usize, usize, usize) {
    let (mut ok, mut fail, mut skipped) = (0, 0, 0);
    let total = plan.items.len();
    let mut renames: Vec<(PathBuf, PathBuf)> = vec![];
    for (i, it) in plan.items.iter().enumerate() {
        if cancel.is_cancelled() { break }
        if only.map(|o| !o.contains(&i)).unwrap_or(false) { continue }
        if skip_done && mode == RunMode::Copies && is_done(&it.path, &out.join(&it.rel).join(&it.new_name)) {
            skipped += 1; report(Progress { index: i, total, name: it.new_name.clone(), status: "уже готово".into(), ok: true, in_bytes: 0, out_bytes: 0 }); continue
        }
        report(Progress { index: i, total, name: it.new_name.clone(), status: "…".into(), ok: true, in_bytes: 0, out_bytes: 0 });
        let res: Result<(String, u64, u64), String> = (|| match mode {
            RunMode::Copies => {
                let dst = out.join(&it.rel).join(&it.new_name);
                let b = build_cbz(&it.path, &dst, opt, &meta_of(it), cancel)?;
                Ok((format!("{} стр.", b.pages), b.in_bytes, b.out_bytes))
            }
            RunMode::InPlace => {
                let dir = it.path.parent().unwrap();
                let tmp = dir.join(format!("{}.polka-tmp", it.new_name));
                let b = build_cbz(&it.path, &tmp, opt, &meta_of(it), cancel)?;
                let bak = backup.join(&it.rel); std::fs::create_dir_all(&bak).map_err(|e| e.to_string())?;
                move_file(&it.path, &bak.join(&it.file_name))?;
                std::fs::rename(&tmp, dir.join(&it.new_name)).map_err(|e| e.to_string())?;
                Ok((format!("{} стр.", b.pages), b.in_bytes, b.out_bytes))
            }
            RunMode::Rename => {
                if it.new_name == it.file_name { return Ok(("без изменений".into(), it.size, it.size)) }
                let tmp = it.path.with_file_name(format!("polka-{}.tmp", i));
                std::fs::rename(&it.path, &tmp).map_err(|e| e.to_string())?;
                renames.push((tmp, it.path.with_file_name(&it.new_name)));
                Ok(("переименовано".into(), it.size, it.size))
            }
        })();
        match res {
            Ok((s, a, b)) => { ok += 1; report(Progress { index: i, total, name: it.new_name.clone(), status: s, ok: true, in_bytes: a, out_bytes: b }) }
            Err(e) => { fail += 1; report(Progress { index: i, total, name: it.new_name.clone(), status: e, ok: false, in_bytes: 0, out_bytes: 0 }) }
        }
    }
    // второй шаг переименования: новые имена не сталкиваются со старыми
    for (tmp, dst) in renames { let _ = std::fs::rename(&tmp, &dst); if archive::kind_of(&dst) == Kind::Zip { let _ = set_zip_comicinfo(&dst, plan); } }
    (ok, fail, skipped)
}

/// Готовая копия есть и не старше исходника
pub fn is_done(src: &Path, dst: &Path) -> bool {
    match (std::fs::metadata(src).and_then(|m| m.modified()), std::fs::metadata(dst).and_then(|m| m.modified())) { (Ok(a), Ok(b)) => b >= a, _ => false }
}

fn move_file(a: &Path, b: &Path) -> Result<(), String> {
    if std::fs::rename(a, b).is_ok() { return Ok(()) }
    std::fs::copy(a, b).map_err(|e| e.to_string())?; std::fs::remove_file(a).map_err(|e| e.to_string())
}

/// Название внутри ZIP после переименования: архив переписывается с той же упаковкой страниц
fn set_zip_comicinfo(path: &Path, plan: &Plan) -> Result<(), String> {
    let it = plan.items.iter().find(|i| i.path.with_file_name(&i.new_name) == path).ok_or("нет в плане")?;
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut old_ci = None; let mut pages = 0;
    for i in 0..z.len() { let e = z.by_index(i).map_err(|e| e.to_string())?; let n = e.name().to_lowercase(); if n.ends_with("comicinfo.xml") { let mut b = vec![]; let mut e = e; std::io::Read::read_to_end(&mut e, &mut b).ok(); old_ci = Some(b) } else if archive::is_page(&n) { pages += 1 } }
    let tmp = path.with_extension("ci.tmp");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&tmp).map_err(|e| e.to_string())?);
    for i in 0..z.len() { let e = z.by_index_raw(i).map_err(|e| e.to_string())?; if e.name().to_lowercase().ends_with("comicinfo.xml") { continue } w.raw_copy_file(e).map_err(|e| e.to_string())? }
    w.start_file("ComicInfo.xml", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).map_err(|e| e.to_string())?;
    w.write_all(&comicinfo::build(old_ci.as_deref(), &meta_of(it), pages)).map_err(|e| e.to_string())?;
    w.finish().map_err(|e| e.to_string())?; drop(z);
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}
