//! Чтение архивов комиксов: ZIP/CBZ, RAR/CBR, 7Z/CB7. Страницы — картинки в естественном порядке.

use pb_core::util::sort_key;
use std::io::Read;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Kind { Zip, Rar, SevenZ, Pdf, Unknown }

impl Kind {
    pub fn label(&self) -> &'static str { match self { Kind::Zip => "ZIP", Kind::Rar => "RAR", Kind::SevenZ => "7Z", Kind::Pdf => "PDF", Kind::Unknown => "?" } }
}

pub const ARCHIVE_EXTS: &[&str] = &["cbz", "cbr", "cb7", "zip", "rar", "7z"];
pub const PAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp", "avif", "tif", "tiff", "jxl", "heic"];

pub fn kind_of(path: &Path) -> Kind {
    let mut h = [0u8; 8];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut h)).unwrap_or(0);
    let h = &h[..n];
    if h.starts_with(b"PK") { Kind::Zip } else if h.starts_with(b"Rar!") { Kind::Rar }
    else if h.starts_with(&[0x37, 0x7A, 0xBC, 0xAF]) { Kind::SevenZ } else if h.starts_with(b"%PDF") { Kind::Pdf } else { Kind::Unknown }
}

pub fn is_page(name: &str) -> bool {
    let n = name.replace('\\', "/"); let base = n.rsplit('/').next().unwrap_or("");
    if base.starts_with("._") || n.contains("__MACOSX/") { return false }
    base.rsplit_once('.').map(|(_, e)| PAGE_EXTS.contains(&e.to_lowercase().as_str())).unwrap_or(false)
}

/// Содержимое архива: страницы по порядку + ComicInfo.xml (если был)
pub struct Content { pub pages: Vec<(String, Vec<u8>)>, pub comic_info: Option<Vec<u8>> }

/// Прочитать всё. `want` — если задан, читаются только эти номера страниц (по порядку), остальные пропускаются.
pub fn read(path: &Path, want: Option<&[usize]>) -> Result<Content, String> {
    let kind = kind_of(path);
    let mut all: Vec<(String, Option<Vec<u8>>)> = Vec::new();
    let mut ci = None;
    match kind {
        Kind::Zip => {
            let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
            let mut z = zip::ZipArchive::new(f).map_err(|e| format!("ZIP повреждён: {e}"))?;
            let mut names: Vec<(usize, String)> = (0..z.len()).filter_map(|i| z.by_index_raw(i).ok().map(|e| (i, e.name().to_string()))).collect();
            if let Some((i, _)) = names.iter().find(|(_, n)| n.to_lowercase().ends_with("comicinfo.xml")) {
                let mut b = vec![]; if let Ok(mut e) = z.by_index(*i) { let _ = e.read_to_end(&mut b); ci = Some(b) }
            }
            names.retain(|(_, n)| is_page(n));
            names.sort_by_key(|(_, n)| sort_key(n));
            for (k, (i, n)) in names.iter().enumerate() {
                let need = want.map(|w| w.contains(&k)).unwrap_or(true);
                let data = if need { let mut e = z.by_index(*i).map_err(|e| e.to_string())?; let mut b = Vec::with_capacity(e.size() as usize); e.read_to_end(&mut b).map_err(|e| format!("ZIP повреждён: {e}"))?; Some(b) } else { None };
                all.push((n.clone(), data));
            }
            return Ok(finish(all, ci));
        }
        Kind::SevenZ => {
            let mut r = sevenz_rust2::SevenZReader::open(path, sevenz_rust2::Password::empty()).map_err(|e| format!("7Z не открылся: {e}"))?;
            let mut got: Vec<(String, Vec<u8>)> = vec![];
            r.for_each_entries(|e, rd| {
                if e.is_directory() { return Ok(true) }
                let n = e.name().to_string();
                if is_page(&n) || n.to_lowercase().ends_with("comicinfo.xml") { let mut b = vec![]; rd.read_to_end(&mut b)?; got.push((n, b)) }
                else { std::io::copy(rd, &mut std::io::sink())?; }
                Ok(true)
            }).map_err(|e| format!("7Z повреждён: {e}"))?;
            for (n, b) in got { if n.to_lowercase().ends_with("comicinfo.xml") { ci = Some(b) } else { all.push((n, Some(b))) } }
        }
        Kind::Rar => {
            let mut a = unrar::Archive::new(path).open_for_processing().map_err(|e| format!("RAR не открылся: {e}"))?;
            while let Some(h) = a.read_header().map_err(|e| format!("RAR повреждён: {e}"))? {
                let n = h.entry().filename.to_string_lossy().to_string();
                a = if h.entry().is_file() && (is_page(&n) || n.to_lowercase().ends_with("comicinfo.xml")) {
                    let (b, rest) = h.read().map_err(|e| format!("RAR повреждён: {e}"))?;
                    if n.to_lowercase().ends_with("comicinfo.xml") { ci = Some(b) } else { all.push((n, Some(b))) }
                    rest
                } else { h.skip().map_err(|e| e.to_string())? };
            }
        }
        Kind::Pdf => return Err("это PDF, а не архив".into()),
        Kind::Unknown => return Err("неизвестный формат или файл повреждён".into()),
    }
    all.sort_by_key(|(n, _)| sort_key(n));
    if let Some(w) = want { for (k, x) in all.iter_mut().enumerate() { if !w.contains(&k) { x.1 = None } } }
    Ok(finish(all, ci))
}

fn finish(all: Vec<(String, Option<Vec<u8>>)>, ci: Option<Vec<u8>>) -> Content {
    Content { pages: all.into_iter().map(|(n, b)| (n, b.unwrap_or_default())).collect(), comic_info: ci }
}

/// Сколько страниц и их общий вес — быстро для ZIP, иначе через чтение
pub fn page_count(path: &Path) -> usize {
    if kind_of(path) == Kind::Zip {
        if let Ok(z) = std::fs::File::open(path).map_err(|_| ()).and_then(|f| zip::ZipArchive::new(f).map_err(|_| ())) { return z.file_names().filter(|n| is_page(n)).count() }
    }
    read(path, Some(&[])).map(|c| c.pages.len()).unwrap_or(0)
}
