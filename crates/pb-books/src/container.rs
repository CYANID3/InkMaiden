//! ZIP-книга в памяти (EPUB, FB2.ZIP, FB3): прочитать, поменять файлы, записать заново.
//! Порядок файлов сохраняется; mimetype в EPUB остаётся первым и без сжатия, как требует стандарт.

use std::io::{Read, Write};

pub struct Entry { pub name: String, pub data: Vec<u8>, pub stored: bool }

pub struct ZipBook { pub entries: Vec<Entry> }

impl ZipBook {
    pub fn read(bytes: &[u8]) -> Result<ZipBook, String> {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("архив повреждён: {e}"))?;
        let mut entries = Vec::with_capacity(z.len());
        for i in 0..z.len() {
            let mut e = z.by_index(i).map_err(|e| format!("архив повреждён: {e}"))?;
            if e.is_dir() { continue }
            let mut data = Vec::with_capacity(e.size() as usize);
            e.read_to_end(&mut data).map_err(|e| format!("архив повреждён: {e}"))?;
            entries.push(Entry { name: e.name().to_string(), stored: e.compression() == zip::CompressionMethod::Stored, data });
        }
        Ok(ZipBook { entries })
    }
    /// файл по имени (без учёта регистра)
    pub fn find(&self, name: &str) -> Option<&Entry> { self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)) }
    pub fn get(&self, name: &str) -> Option<&[u8]> { self.find(name).map(|e| e.data.as_slice()) }
    pub fn set(&mut self, name: &str, data: Vec<u8>) {
        match self.entries.iter_mut().find(|e| e.name.eq_ignore_ascii_case(name)) {
            Some(e) => e.data = data,
            None => self.entries.push(Entry { name: name.to_string(), data, stored: false }),
        }
    }
    pub fn remove(&mut self, name: &str) { self.entries.retain(|e| !e.name.eq_ignore_ascii_case(name)) }
    pub fn write(&self) -> Result<Vec<u8>, String> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let mut order: Vec<&Entry> = self.entries.iter().collect();
        if let Some(i) = order.iter().position(|e| e.name == "mimetype") { let m = order.remove(i); order.insert(0, m); }
        for e in order {
            let stored = e.stored || e.name == "mimetype";
            let o = zip::write::SimpleFileOptions::default()
                .compression_method(if stored { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated });
            w.start_file(e.name.as_str(), o).map_err(|e| e.to_string())?;
            w.write_all(&e.data).map_err(|e| e.to_string())?;
        }
        Ok(w.finish().map_err(|e| e.to_string())?.into_inner())
    }
}

/// Путь внутри архива относительно папки файла: «../Images/a.jpg» от «OEBPS/Text/» → «OEBPS/Images/a.jpg»
pub fn resolve(base_dir: &str, href: &str) -> String {
    let href = crate::text::url_decode(href.split('#').next().unwrap_or(""));
    if let Some(h) = href.strip_prefix('/') { return h.to_string() }
    let mut parts: Vec<&str> = vec![];
    let joined = format!("{base_dir}{href}");
    for p in joined.split('/') {
        match p { ".." => { parts.pop(); } "." | "" => {} x => parts.push(x) }
    }
    parts.join("/")
}

pub fn dir_of(path: &str) -> String { path.rfind('/').map(|i| path[..=i].to_string()).unwrap_or_default() }
pub fn file_name(path: &str) -> &str { path.rsplit('/').next().unwrap_or(path) }
