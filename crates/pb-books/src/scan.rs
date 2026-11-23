//! Папка с книгами → список книг; одна книга → готовый файл (обложка, чистка, TXT → FB2).

use crate::container::ZipBook;
use crate::junk::{self, Finding, Junk};
use crate::meta::{self, fb2_in_zip, Format, Meta};
use crate::text::{decode_text, decode_xml, xml_utf8};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Book {
    pub path: PathBuf,
    /// путь от корня папки: «Автор/Серия/03. Название.fb2»
    pub rel: String,
    pub format: Format,
    pub size: u64,
    pub meta: Meta,
    /// не читается — что случилось
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Scan { pub root: PathBuf, pub books: Vec<Book>, pub skipped: BTreeMap<String, usize> }

fn skip_dir(name: &str) -> bool {
    let l = name.to_lowercase();
    matches!(l.as_str(), "system" | "applications" | "_backup" | "$recycle.bin" | "system volume information") || l.starts_with('.')
}

pub fn scan(root: &Path, with_txt: bool) -> Scan { scan_with(root, with_txt, &|_, _| {}) }

/// То же с прогрессом: progress(прочитано книг, всего книг). Книги читаются параллельно, порядок — как в проводнике.
pub fn scan_with(root: &Path, with_txt: bool, progress: &(dyn Fn(usize, usize) + Sync)) -> Scan {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut s = Scan { root: root.to_path_buf(), ..Default::default() };
    let mut stack = vec![root.to_path_buf()];
    let mut files = vec![];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() { if !skip_dir(&name) { stack.push(p) } continue }
            files.push(p);
        }
    }
    files.sort_by_key(|p| pb_core::util::sort_key(&p.to_string_lossy()));
    let mut todo = vec![];
    for p in files {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(fmt) = Format::of(&name) else {
            if Format::unsupported(&name) { *s.skipped.entry(name.rsplit('.').next().unwrap().to_lowercase()).or_default() += 1 }
            continue;
        };
        if fmt == Format::Txt && !with_txt { continue }
        todo.push((p, fmt));
    }
    let total = todo.len();
    progress(0, total);
    let done = AtomicUsize::new(0);
    s.books = todo.into_par_iter().map(|(p, fmt)| {
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        let pm = meta::path_meta(&rel);
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        let (m, error) = match std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| std::panic::catch_unwind(|| meta::read(&b, fmt)).unwrap_or_else(|_| Err("внутренняя ошибка разбора".into()))) {
            Ok(m) => (m, None),
            Err(e) => (Meta::default(), Some(format!("не читается: {e}"))),
        };
        let pick = |a: &str, b: &str| if a.trim().is_empty() { b.to_string() } else { a.trim().to_string() };
        let meta = Meta { author: pick(&m.author, &pm.author), title: pick(&m.title, &pm.title), series: pick(&m.series, &pm.series), number: pick(&m.number, &pm.number), has_cover: m.has_cover, genres: m.genres };
        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
        if n.is_multiple_of(10) || n == total { progress(n, total) }
        Book { path: p, rel, format: fmt, size, meta, error }
    }).collect();
    s
}

/// Что сделать с книгой
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Job {
    /// новая обложка (JPEG) или ничего
    #[serde(default)]
    pub cover: Option<Vec<u8>>,
    /// куда убрать прежнюю версию готовой книги перед заменой (папка «Бэкапы\Книги»); нет — просто заменить
    #[serde(skip)]
    pub backup: Option<std::path::PathBuf>,
    /// находки чистки (учитываются только отмеченные); в интерфейсе с 0.8.1 не используется
    #[serde(default)]
    pub junk: Vec<Finding>,
    /// внешние ссылки (http, www, mailto…) превратить в обычный текст
    #[serde(default)]
    pub unlink: bool,
    /// новые название, автор, серия
    #[serde(default)]
    pub meta: crate::edit::MetaEdit,
    /// поправленные части книги
    #[serde(default)]
    pub parts: Vec<crate::edit::PartEdit>,
    /// своё имя готового файла (только имя, без папок): «Автор — Название.epub»; нет — как у исходного (TXT → .fb2)
    #[serde(default)]
    pub out_name: Option<String>,
}

/// Готовая книга: куда легла и сколько знаков рекламы убрано
#[derive(Clone, Debug, Serialize)]
pub struct Done { pub path: PathBuf, pub rel: String, pub removed: usize, pub links: usize, pub bytes: u64 }

/// Путь готовой книги внутри папки готового: та же папка, что у исходной; имя — своё (если задано и годится) или исходное (TXT → .fb2)
pub fn out_rel(b: &Book, name: Option<&str>) -> String {
    let legacy = if b.format == Format::Txt { format!("{}.fb2", b.rel.strip_suffix(".txt").or(b.rel.strip_suffix(".TXT")).unwrap_or(&b.rel)) } else { b.rel.clone() };
    match name.filter(|n| safe_name(n)) {
        Some(n) => { let r = legacy.replace('\\', "/"); match r.rsplit_once('/') { Some((dir, _)) => format!("{dir}/{n}"), None => n.to_string() } }
        None => legacy,
    }
}
/// имя файла без папок и запрещённых в Windows знаков
pub fn safe_name(n: &str) -> bool {
    let t = n.trim();
    !t.is_empty() && t.len() <= 240 && t == n && !t.ends_with('.') && t != "." && t != ".."
        && !t.chars().any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || (c as u32) < 32)
}
/// Куда ляжет готовая книга — чтобы пропускать уже готовые
pub fn out_path(b: &Book, out: &Path) -> PathBuf { out.join(out_rel(b, None)) }
/// Готовая книга есть и не старше оригинала
pub fn is_done(b: &Book, out: &Path) -> bool { newer_or_same(&b.path, &out_path(b, out)) }
/// то же для своего имени готового файла
pub fn is_done_as(b: &Book, out: &Path, name: Option<&str>) -> bool { newer_or_same(&b.path, &out.join(out_rel(b, name))) }
pub fn newer_or_same(src: &Path, dst: &Path) -> bool {
    match (std::fs::metadata(src).and_then(|m| m.modified()), std::fs::metadata(dst).and_then(|m| m.modified())) { (Ok(a), Ok(b)) => b >= a, _ => false }
}

/// Обработать книгу и положить результат в out (с той же вложенностью). Оригинал не меняется.
/// TXT становится FB2 (с обложкой, если она есть).
pub fn process(b: &Book, out: &Path, job: &Job) -> Result<Done, String> {
    use crate::edit::{apply_meta, apply_parts};
    use crate::junk::Target;
    let src = std::fs::read(&b.path).map_err(|e| format!("не читается: {e}"))?;
    let mut rel = b.rel.clone();
    let fmt = b.format;
    let named = job.out_name.as_deref().filter(|n| safe_name(n)).map(|n| out_rel(b, Some(n)));
    let mut data = src;
    let mut removed = 0;
    let mut links = 0;
    if fmt == Format::Txt {
        let mut meta = b.meta.clone();
        if let Some(t) = &job.meta.title { meta.title = t.clone() }
        if let Some(a) = &job.meta.author { meta.author = a.clone() }
        if let Some(s) = &job.meta.series { meta.series = s.clone() }
        if let Some(n) = &job.meta.number { meta.number = n.clone() }
        let fb = crate::txt::to_fb2(&decode_text(&data), &meta);
        data = match &job.cover { Some(j) => xml_utf8(&crate::cover::fb2(&String::from_utf8_lossy(&fb), j)?), None => fb };
        rel = format!("{}.fb2", rel.strip_suffix(".txt").or(rel.strip_suffix(".TXT")).unwrap_or(&rel));
    } else {
        // 1) чистка по исходной книге (номера находок считаны с неё); разделы, которые правили вручную, не трогаем
        let edited: Vec<usize> = job.parts.iter().filter_map(|p| p.id.strip_prefix('s')?.parse().ok()).collect();
        let junk: Vec<Finding> = job.junk.iter().cloned().map(|mut f| { if let Target::Sec { i } = f.target { if edited.contains(&i) { f.on = false } } f }).collect();
        let dropped: Vec<usize> = junk.iter().filter(|f| f.on).filter_map(|f| if let Target::Sec { i } = f.target { Some(i) } else { None }).collect();
        if junk.iter().any(|f| f.on) { let (d, n) = junk::remove(&data, fmt, &junk)?; data = d; removed = n; }
        // 2) правки частей, 3) данные книги, 4) обложка
        data = apply_parts(data, fmt, &job.parts, &dropped)?;
        data = apply_meta(data, fmt, &job.meta)?;
        if job.unlink { let (d, n) = crate::links::unlink(&data, fmt)?; data = d; links = n; }
        if let Some(j) = &job.cover {
            data = match fmt {
                Format::Fb2 => xml_utf8(&crate::cover::fb2(&decode_xml(&data), j)?),
                Format::Fb2Zip => { let mut z = ZipBook::read(&data)?; let n = fb2_in_zip(&z)?; let s = crate::cover::fb2(&decode_xml(z.get(&n).unwrap()), j)?; z.set(&n, xml_utf8(&s)); z.write()? }
                Format::Epub => { let mut z = ZipBook::read(&data)?; crate::cover::epub(&mut z, j)?; z.write()? }
                Format::Fb3 => { let mut z = ZipBook::read(&data)?; crate::cover::fb3(&mut z, j)?; z.write()? }
                Format::Txt => unreachable!(),
            };
        }
    }
    if let Some(r) = named { rel = r }
    let dst = out.join(&rel);
    // та же книга уже лежит в готовом — не переписывать (меньше записи на диск, прежняя копия не теряется)
    if std::fs::metadata(&dst).map(|m| m.len() == data.len() as u64).unwrap_or(false) && std::fs::read(&dst).map(|b| b == data).unwrap_or(false) {
        // отметить свежей, чтобы «Только новые» считала её готовой
        if let Ok(f) = std::fs::File::options().write(true).open(&dst) { let _ = f.set_modified(std::time::SystemTime::now()); }
        return Ok(Done { path: dst, rel, removed, links, bytes: data.len() as u64 })
    }
    if let Some(d) = dst.parent() { std::fs::create_dir_all(d).map_err(|e| format!("не создать папку {}: {e}", d.display()))? }
    // сначала новая версия целиком во временный файл: при сбое или нехватке места готовая книга остаётся прежней
    let tmp = dst.with_extension("inkmaiden-tmp");
    if let Err(e) = std::fs::write(&tmp, &data) { let _ = std::fs::remove_file(&tmp); return Err(format!("не записать {}: {e}", dst.display())) }
    // прежняя версия — в «Бэкапы» (одна на книгу: новая копия заменяет старую)
    if let (Some(b), true) = (&job.backup, dst.exists()) {
        let bp = b.join(&rel);
        let moved = bp.parent().map(|d| std::fs::create_dir_all(d).is_ok()).unwrap_or(false)
            && (std::fs::rename(&dst, &bp).is_ok() || (std::fs::copy(&dst, &bp).is_ok()));
        if !moved { let _ = std::fs::remove_file(&tmp); return Err(format!("не сохранить прежнюю версию в {}", b.display())) }
    }
    std::fs::rename(&tmp, &dst).map_err(|e| { let _ = std::fs::remove_file(&tmp); format!("не записать {}: {e}", dst.display()) })?;
    Ok(Done { path: dst, rel, removed, links, bytes: data.len() as u64 })
}

/// Найти рекламу в книге
pub fn analyze(b: &Book) -> Result<Junk, String> {
    if b.format == Format::Txt { return Ok(Junk::default()) }
    junk::find(&std::fs::read(&b.path).map_err(|e| e.to_string())?, b.format)
}
