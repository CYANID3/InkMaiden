//! Что за книга: формат, автор, название, серия, номер, есть ли обложка, жанры.

use crate::container::{dir_of, resolve, ZipBook};
use crate::text::decode_xml;
use crate::xml::{attr, child, children, is, parse, path, text, text1};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format { Fb2, #[serde(rename = "fb2.zip")] Fb2Zip, Epub, Fb3, Txt }

impl Format {
    pub fn of(name: &str) -> Option<Format> {
        let n = name.to_lowercase();
        if n.ends_with(".fb2.zip") { return Some(Format::Fb2Zip) }
        match n.rsplit('.').next()? { "fb2" => Some(Format::Fb2), "epub" => Some(Format::Epub), "fb3" => Some(Format::Fb3), "txt" => Some(Format::Txt), _ => None }
    }
    /// форматы, которые читалка открывает, но обложку в них не вшить
    pub fn unsupported(name: &str) -> bool { matches!(name.to_lowercase().rsplit('.').next(), Some("doc" | "docx" | "rtf" | "mobi" | "azw" | "azw3" | "odt" | "pdf" | "djvu")) }
    pub fn label(self) -> &'static str { match self { Format::Fb2 => "FB2", Format::Fb2Zip => "FB2.ZIP", Format::Epub => "EPUB", Format::Fb3 => "FB3", Format::Txt => "TXT" } }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Meta {
    pub author: String,
    pub title: String,
    pub series: String,
    pub number: String,
    pub has_cover: bool,
    pub genres: Vec<String>,
}

/// Из пути: «Автор\Серия\03. Название.fb2», «Автор - Название.fb2»
pub fn path_meta(rel: &str) -> Meta {
    let parts: Vec<&str> = rel.split(['\\', '/']).collect();
    let file = parts.last().copied().unwrap_or("");
    let lower = file.to_lowercase();
    let mut name = file.to_string();
    for ext in [".fb2.zip", ".fb2", ".epub", ".fb3", ".txt"] { if lower.ends_with(ext) { name.truncate(file.len() - ext.len()); break } }
    let mut m = Meta { title: name.clone(), ..Default::default() };
    let num = fancy_regex::Regex::new(r"^(\d{1,3})\.\s*(.+)$").unwrap();
    let dash = fancy_regex::Regex::new(r"^(.+?)\s+-\s+(.+)$").unwrap();
    if let Ok(Some(c)) = num.captures(&name) { m.number = c[1].parse::<u32>().map(|n| n.to_string()).unwrap_or_default(); m.title = c[2].to_string(); }
    else if let Ok(Some(c)) = dash.captures(&name) { m.author = c[1].to_string(); m.title = c[2].to_string(); }
    let mut dirs: Vec<&str> = parts[..parts.len().saturating_sub(1)].to_vec();
    if dirs.first().map(|d| matches!(*d, "Библиотека" | "Книги" | "Books")).unwrap_or(false) { dirs.remove(0); }
    if m.author.is_empty() {
        if let Some(a) = dirs.first() {
            if !a.starts_with("Сборники") && !a.starts_with("Фанфики") {
                if let Some((f, rest)) = a.split_once(char::is_whitespace) { m.author = format!("{} {}", rest.trim(), f); }
            }
        }
    }
    if dirs.len() >= 2 { m.series = dirs[dirs.len() - 1].to_string(); }
    m
}

pub fn fb2_meta(src: &str) -> Result<Meta, String> {
    let doc = parse(src)?;
    let root = doc.root_element();
    let mut m = Meta::default();
    let Some(ti) = path(root, &["description", "title-info"]) else { return Ok(m) };
    if let Some(a) = child(ti, "author") {
        m.author = format!("{} {}", text1(child(a, "first-name")), text1(child(a, "last-name"))).trim().to_string();
        if m.author.is_empty() { m.author = text1(child(a, "nickname")); }
    }
    m.title = text1(child(ti, "book-title"));
    m.genres = children(ti, "genre").map(|g| text(&g)).filter(|g| !g.is_empty()).collect();
    if let Some(sq) = child(ti, "sequence") {
        m.series = sq.attribute("name").unwrap_or("").trim().to_string();
        let n = sq.attribute("number").unwrap_or("").trim();
        if !n.is_empty() && n != "0" { m.number = n.to_string(); }
    }
    if let Some(img) = path(ti, &["coverpage", "image"]) {
        let id = attr(&img, "href").unwrap_or("").trim_start_matches('#');
        if !id.is_empty() {
            m.has_cover = children(root, "binary").any(|b| b.attribute("id") == Some(id) && b.text().map(|t| t.trim().len() > 100).unwrap_or(false));
        }
    }
    Ok(m)
}

/// Путь к OPF из META-INF/container.xml
pub fn opf_path(z: &ZipBook) -> Result<String, String> {
    let c = z.get("META-INF/container.xml").ok_or("нет META-INF/container.xml — это не EPUB")?;
    let s = decode_xml(c);
    let doc = parse(&s)?;
    let rf = doc.descendants().find(|n| is(n, "rootfile")).ok_or("в container.xml нет rootfile")?;
    Ok(crate::text::url_decode(rf.attribute("full-path").unwrap_or("")))
}

pub fn epub_meta(z: &ZipBook) -> Result<Meta, String> {
    let opf = opf_path(z)?;
    let s = decode_xml(z.get(&opf).ok_or(format!("нет файла {opf}"))?);
    let doc = parse(&s)?;
    let pkg = doc.root_element();
    let dir = dir_of(&opf);
    let mut m = Meta::default();
    let md = child(pkg, "metadata");
    if let Some(md) = md {
        m.title = text1(child(md, "title"));
        m.author = text1(child(md, "creator"));
        m.genres = children(md, "subject").map(|g| text(&g)).filter(|g| !g.is_empty()).collect();
        let meta_named = |n: &str| children(md, "meta").find(|x| x.attribute("name") == Some(n));
        if let Some(sr) = meta_named("calibre:series") {
            m.series = sr.attribute("content").unwrap_or("").to_string();
            if let Some(si) = meta_named("calibre:series_index") {
                let n = si.attribute("content").unwrap_or("").trim_end_matches(".0").trim_end_matches(".00").to_string();
                if n != "0" { m.number = n; }
            }
        } else if let Some(c) = children(md, "meta").find(|x| x.attribute("property") == Some("belongs-to-collection")) {
            m.series = text(&c);
            if let Some(cid) = c.attribute("id") {
                let r = format!("#{cid}");
                if let Some(gp) = children(md, "meta").find(|x| x.attribute("refines") == Some(r.as_str()) && x.attribute("property") == Some("group-position")) { m.number = text(&gp); }
            }
        }
    }
    let items: Vec<_> = child(pkg, "manifest").map(|mf| children(mf, "item").collect()).unwrap_or_default();
    let mut cover = items.iter().find(|it| it.attribute("properties").unwrap_or("").split_whitespace().any(|p| p == "cover-image")).copied();
    if cover.is_none() {
        if let Some(cid) = md.and_then(|md| children(md, "meta").find(|x| x.attribute("name") == Some("cover"))).and_then(|x| x.attribute("content")) {
            cover = items.iter().find(|it| it.attribute("id") == Some(cid)).copied();
        }
    }
    if let Some(c) = cover {
        if c.attribute("media-type").unwrap_or("").starts_with("image/") && z.find(&resolve(&dir, c.attribute("href").unwrap_or(""))).is_some() { m.has_cover = true; }
    }
    Ok(m)
}

pub const THUMB: &str = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail";

pub fn fb3_meta(z: &ZipBook) -> Result<Meta, String> {
    let rels = decode_xml(z.get("_rels/.rels").ok_or("нет _rels/.rels — это не FB3")?);
    let rd = parse(&rels)?;
    let mut m = Meta::default();
    for r in rd.root_element().children().filter(|n| is(n, "Relationship")) {
        let t = r.attribute("Type").unwrap_or(""); let target = resolve("", r.attribute("Target").unwrap_or(""));
        if t == THUMB && z.find(&target).is_some() { m.has_cover = true; }
        if t.ends_with("FictionBook3/relationships/Book") {
            if let Some(b) = z.get(&target) {
                let ds = decode_xml(b);
                let d = parse(&ds)?;
                let de = d.root_element();
                m.title = text1(path(de, &["title", "main"]));
                if let Some(cl) = child(de, "fb3-classification") { m.genres = children(cl, "subject").map(|g| text(&g)).filter(|g| !g.is_empty()).collect(); }
                if let Some(sq) = child(de, "sequence") { m.series = text1(path(sq, &["title", "main"])); m.number = sq.attribute("number").unwrap_or("").to_string(); }
                if let Some(au) = child(de, "fb3-relations").and_then(|r| children(r, "subject").find(|s| s.attribute("link") == Some("author"))) {
                    m.author = format!("{} {}", text1(child(au, "first-name")), text1(child(au, "last-name"))).trim().to_string();
                    if m.author.is_empty() { m.author = text1(path(au, &["title", "main"])); }
                }
            }
        }
    }
    Ok(m)
}

pub fn fb2_in_zip(z: &ZipBook) -> Result<String, String> {
    z.entries.iter().find(|e| e.name.to_lowercase().ends_with(".fb2")).map(|e| e.name.clone()).ok_or("внутри архива нет .fb2".into())
}
pub fn fb3_body(z: &ZipBook) -> Result<String, String> {
    z.entries.iter().find(|e| { let n = e.name.to_lowercase(); n == "body.xml" || n.ends_with("/body.xml") }).map(|e| e.name.clone()).ok_or("внутри FB3 нет body.xml".into())
}

/// Сведения о книге из самого файла (пустые поля потом добираются из пути)
pub fn read(bytes: &[u8], fmt: Format) -> Result<Meta, String> {
    match fmt {
        Format::Fb2 => fb2_meta(&decode_xml(bytes)),
        Format::Fb2Zip => { let z = ZipBook::read(bytes)?; fb2_meta(&decode_xml(z.get(&fb2_in_zip(&z)?).unwrap())) }
        Format::Epub => epub_meta(&ZipBook::read(bytes)?),
        Format::Fb3 => fb3_meta(&ZipBook::read(bytes)?),
        Format::Txt => Ok(Meta::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_and_fb2() {
        let m = path_meta("Библиотека/Лукьяненко Сергей/Дозоры/03. Сумеречный дозор.fb2");
        assert_eq!((m.author.as_str(), m.series.as_str(), m.number.as_str(), m.title.as_str()), ("Сергей Лукьяненко", "Дозоры", "3", "Сумеречный дозор"));
        let m = path_meta("Иван Петров - Город.epub");
        assert_eq!((m.author.as_str(), m.title.as_str()), ("Иван Петров", "Город"));
        let fb = r##"<?xml version="1.0" encoding="utf-8"?><FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink"><description><title-info><genre>sf_fantasy</genre><author><first-name>Анна</first-name><last-name>Лесная</last-name></author><book-title>Тихий берег</book-title><coverpage><image l:href="#c.jpg"/></coverpage><sequence name="Берега" number="2"/></title-info></description><body><section><p>x</p></section></body><binary id="c.jpg" content-type="image/jpeg">QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB</binary></FictionBook>"##;
        let m = fb2_meta(fb).unwrap();
        assert_eq!((m.author.as_str(), m.title.as_str(), m.series.as_str(), m.number.as_str(), m.has_cover), ("Анна Лесная", "Тихий берег", "Берега", "2", true));
        assert_eq!(m.genres, vec!["sf_fantasy"]);
        assert_eq!(Format::of("a.FB2.zip"), Some(Format::Fb2Zip));
    }
}
