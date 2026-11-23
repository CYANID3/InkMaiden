//! Подробное окно книги: своя обложка (показать), части книги (посмотреть и поправить), данные (название, автор, серия).
//! Части: в EPUB — файлы глав по порядку чтения, в FB2 — разделы основного текста. Правка части заменяет её целиком;
//! перед записью проверяется, что книга после правки читается как XML.

use crate::container::{dir_of, file_name, resolve, ZipBook};
use crate::meta::{fb2_in_zip, opf_path, Format};
use crate::text::{decode_xml, esc, xml_utf8};
use crate::xml::{attr, child, children, inner_range, is, parse, path, prefix_of, text, Edits};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Новые данные книги: что задано — то и меняется
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MetaEdit {
    pub title: Option<String>,
    pub author: Option<String>,
    pub series: Option<String>,
    pub number: Option<String>,
}
impl MetaEdit { pub fn is_empty(&self) -> bool { self.title.is_none() && self.author.is_none() && self.series.is_none() && self.number.is_none() } }

/// Правка части книги: id части и её новый исходный текст (XHTML или XML раздела)
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PartEdit { pub id: String, #[serde(default)] pub source: String, #[serde(default)] pub delete: bool }

#[derive(Clone, Debug, Serialize)]
pub struct Part { pub id: String, pub title: String, pub chars: usize }

/// Часть для просмотра: HTML для показа (картинки — внутри) и исходник для правки
#[derive(Clone, Debug, Serialize)]
pub struct PartView { pub id: String, pub html: String, pub source: String, pub kind: &'static str }

/* ---------------------------------------------------------------- обложка */

fn mime_of(b: &[u8]) -> &'static str {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else if b.starts_with(b"GIF") { "image/gif" } else if b.len() > 12 && &b[8..12] == b"WEBP" { "image/webp" } else { "image/jpeg" }
}

/// Обложка, которая уже есть в книге: байты картинки
pub fn cover(bytes: &[u8], fmt: Format) -> Option<Vec<u8>> {
    match fmt {
        Format::Fb2 => fb2_cover(&decode_xml(bytes)),
        Format::Fb2Zip => { let z = ZipBook::read(bytes).ok()?; let n = fb2_in_zip(&z).ok()?; fb2_cover(&decode_xml(z.get(&n)?)) }
        Format::Epub => { let z = ZipBook::read(bytes).ok()?; epub_cover(&z) }
        Format::Fb3 => {
            let z = ZipBook::read(bytes).ok()?;
            let rels = decode_xml(z.get("_rels/.rels")?);
            let rd = parse(&rels).ok()?;
            let t = rd.root_element().children().filter(|n| is(n, "Relationship")).find(|r| r.attribute("Type") == Some(crate::meta::THUMB))?;
            z.get(&resolve("", t.attribute("Target")?)).map(|b| b.to_vec())
        }
        Format::Txt => None,
    }
}

fn binary(root: roxmltree::Node, id: &str) -> Option<Vec<u8>> {
    let b = children(root, "binary").find(|b| b.attribute("id") == Some(id))?;
    let t: String = b.text().unwrap_or("").chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD.decode(t).ok()
}

fn fb2_cover(src: &str) -> Option<Vec<u8>> {
    let doc = parse(src).ok()?;
    let root = doc.root_element();
    let img = path(root, &["description", "title-info", "coverpage", "image"])?;
    binary(root, attr(&img, "href")?.trim_start_matches('#'))
}

fn epub_cover(z: &ZipBook) -> Option<Vec<u8>> {
    let opf = opf_path(z).ok()?;
    let s = decode_xml(z.get(&opf)?);
    let doc = parse(&s).ok()?;
    let pkg = doc.root_element();
    let items: Vec<_> = child(pkg, "manifest").map(|m| children(m, "item").collect()).unwrap_or_default();
    let by_prop = items.iter().find(|i| i.attribute("properties").unwrap_or("").split_whitespace().any(|p| p == "cover-image"));
    let by_meta = child(pkg, "metadata").and_then(|md| children(md, "meta").find(|m| m.attribute("name") == Some("cover")))
        .and_then(|m| m.attribute("content")).and_then(|id| items.iter().find(|i| i.attribute("id") == Some(id)));
    let it = by_prop.or(by_meta)?;
    z.get(&resolve(&dir_of(&opf), it.attribute("href")?)).map(|b| b.to_vec())
}

/* ---------------------------------------------------------------- части */

/// Части FB2: разделы основного текста; если разделов нет (текст прямо в <body>) — весь основной текст одной частью
fn units<'a, 'i>(doc: &'a roxmltree::Document<'i>, fmt: Format) -> Vec<roxmltree::Node<'a, 'i>> {
    let s = crate::junk::main_sections(doc, fmt);
    if !s.is_empty() { return s }
    children(doc.root_element(), "body").find(|b| b.attribute("name").is_none()).into_iter().collect()
}

fn epub_spine(z: &ZipBook) -> Result<Vec<String>, String> {
    let opf = opf_path(z)?;
    let s = decode_xml(z.get(&opf).ok_or("нет OPF")?);
    let doc = parse(&s)?;
    let pkg = doc.root_element();
    let man: HashMap<&str, roxmltree::Node> = child(pkg, "manifest").map(|mf| children(mf, "item").filter_map(|i| Some((i.attribute("id")?, i))).collect()).unwrap_or_default();
    let mut out = vec![];
    for ir in child(pkg, "spine").map(|sp| children(sp, "itemref").collect::<Vec<_>>()).unwrap_or_default() {
        let Some(it) = ir.attribute("idref").and_then(|id| man.get(id)) else { continue };
        if let Some(e) = z.find(&resolve(&dir_of(&opf), it.attribute("href").unwrap_or(""))) { out.push(e.name.clone()) }
    }
    Ok(out)
}

fn html_title(src: &str) -> Option<String> {
    let doc = parse(src).ok()?;
    let h = doc.descendants().find(|n| n.is_element() && matches!(n.tag_name().name(), "h1" | "h2" | "h3" | "h4"))
        .or_else(|| doc.descendants().find(|n| is(n, "title")))?;
    let t = text(&h);
    (!t.is_empty()).then_some(t)
}

fn short(t: &str, n: usize) -> String { if t.chars().count() > n { format!("{}…", t.chars().take(n).collect::<String>().trim_end()) } else { t.to_string() } }

fn fb2_src(bytes: &[u8], fmt: Format) -> Result<(String, Option<(ZipBook, String)>), String> {
    match fmt {
        Format::Fb2 => Ok((decode_xml(bytes), None)),
        Format::Fb2Zip => { let z = ZipBook::read(bytes)?; let n = fb2_in_zip(&z)?; let s = decode_xml(z.get(&n).unwrap()); Ok((s, Some((z, n)))) }
        _ => Err("в этом формате части не правятся".into()),
    }
}

/// Части книги по порядку
pub fn parts(bytes: &[u8], fmt: Format) -> Result<Vec<Part>, String> {
    match fmt {
        Format::Epub => {
            let z = ZipBook::read(bytes)?;
            Ok(epub_spine(&z)?.into_iter().map(|n| {
                let s = decode_xml(z.get(&n).unwrap_or_default());
                let chars = parse(&s).map(|d| text(&d.root_element()).chars().count()).unwrap_or(0);
                Part { title: html_title(&s).map(|t| short(&t, 80)).unwrap_or_else(|| file_name(&n).to_string()), id: n, chars }
            }).collect())
        }
        Format::Fb2 | Format::Fb2Zip => {
            let (src, _) = fb2_src(bytes, fmt)?;
            let doc = parse(&src)?;
            Ok(units(&doc, fmt).iter().enumerate().map(|(i, s)| {
                let all = text(s);
                let title = child(*s, "title").map(|t| text(&t)).filter(|t| !t.is_empty()).unwrap_or_else(|| short(&all, 60));
                Part { id: format!("s{i}"), title: if title.is_empty() { format!("Раздел {}", i + 1) } else { short(&title, 80) }, chars: all.chars().count() }
            }).collect())
        }
        _ => Ok(vec![]),
    }
}

fn data_url(b: &[u8]) -> String { format!("data:{};base64,{}", mime_of(b), base64::engine::general_purpose::STANDARD.encode(b)) }

/// Одна часть: HTML для просмотра и исходник
pub fn part(bytes: &[u8], fmt: Format, id: &str) -> Result<PartView, String> {
    match fmt {
        Format::Epub => {
            let z = ZipBook::read(bytes)?;
            let source = decode_xml(z.get(id).ok_or("нет такой части")?);
            Ok(PartView { id: id.into(), html: epub_html(&z, id, &source), source, kind: "xhtml" })
        }
        Format::Fb2 | Format::Fb2Zip => {
            let (src, _) = fb2_src(bytes, fmt)?;
            let doc = parse(&src)?;
            let i: usize = id.strip_prefix('s').and_then(|x| x.parse().ok()).ok_or("нет такой части")?;
            let secs = units(&doc, fmt);
            let s = secs.get(i).ok_or("нет такой части")?;
            let mut html = String::new();
            fb2_html(*s, doc.root_element(), &src, &mut html);
            Ok(PartView { id: id.into(), html, source: src[s.range()].to_string(), kind: "fb2" })
        }
        _ => Err("в этом формате части не показываются".into()),
    }
}

/// HTML для показа и правки текстом — по исходнику части (в том числе уже поправленному)
pub fn render(bytes: &[u8], fmt: Format, id: &str, source: &str) -> Result<String, String> {
    match fmt {
        Format::Epub => { let z = ZipBook::read(bytes)?; Ok(epub_html(&z, id, source)) }
        Format::Fb2 | Format::Fb2Zip => {
            let (src, _) = fb2_src(bytes, fmt)?;
            let e = PartEdit { id: id.into(), source: source.into(), delete: false };
            let full = replace_sections(&src, fmt, std::slice::from_ref(&e), &[])?;
            let doc = parse(&full)?;
            let i: usize = id.strip_prefix('s').and_then(|x| x.parse().ok()).ok_or("нет такой части")?;
            let s = *units(&doc, fmt).get(i).ok_or("нет такой части")?;
            let mut html = String::new();
            fb2_html(s, doc.root_element(), &full, &mut html);
            Ok(html)
        }
        _ => Err("в этом формате части не показываются".into()),
    }
}

/// тело XHTML без скриптов; картинки из книги — прямо в тексте
fn epub_html(z: &ZipBook, name: &str, src: &str) -> String {
    let body = parse(src).ok().and_then(|d| d.descendants().find(|n| is(n, "body")).map(|b| src[inner_range(src, &b)].to_string())).unwrap_or_else(|| src.to_string());
    let script = fancy_regex::Regex::new(r#"(?is)<script\b.*?</script>|<script\b[^>]*/>|\son\w+\s*=\s*("[^"]*"|'[^']*')"#).unwrap();
    let body = script.replace_all(&body, "").into_owned();
    let img = fancy_regex::Regex::new(r#"(?i)(<img\b[^>]*?\ssrc\s*=\s*|<image\b[^>]*?\shref\s*=\s*|<image\b[^>]*?xlink:href\s*=\s*)(["'])([^"']+)\2"#).unwrap();
    let dir = dir_of(name);
    img.replace_all(&body, |c: &fancy_regex::Captures| {
        let p = resolve(&dir, &crate::text::url_decode(&c[3]));
        // исходная ссылка — в data-pb-src: правка текстом вернёт её на место
        match z.get(&p).filter(|b| b.len() < 4 << 20) { Some(b) => format!("{}{}{}{} data-pb-src={}{}{}", &c[1], &c[2], data_url(b), &c[2], &c[2], &c[3], &c[2]), None => c[0].to_string() }
    }).into_owned()
}

/// раздел FB2 → простой HTML для показа
fn fb2_html(n: roxmltree::Node, root: roxmltree::Node, src: &str, out: &mut String) {
    for c in n.children() {
        if c.is_text() { out.push_str(&esc(c.text().unwrap_or(""))); continue }
        if !c.is_element() { continue }
        let (open, close) = match c.tag_name().name() {
            "section" => ("<section>", "</section>"), "title" => ("<h3>", "</h3>"), "subtitle" => ("<h4>", "</h4>"),
            "p" => ("<p>", "</p>"), "emphasis" => ("<em>", "</em>"), "strong" => ("<strong>", "</strong>"), "strikethrough" => ("<s>", "</s>"),
            "sub" => ("<sub>", "</sub>"), "sup" => ("<sup>", "</sup>"), "code" => ("<code>", "</code>"),
            "epigraph" => ("<blockquote class=\"ep\">", "</blockquote>"), "cite" => ("<blockquote>", "</blockquote>"),
            "poem" => ("<div class=\"poem\">", "</div>"), "stanza" => ("<div class=\"stanza\">", "</div>"), "v" => ("<p class=\"v\">", "</p>"),
            "text-author" => ("<p class=\"ta\">", "</p>"),
            // ссылка: исходный открывающий тег — в data-pb-open, чтобы правка текстом вернула его как был
            "a" => {
                let r = c.range(); let open_end = src[r.start..r.end].find('>').map(|i| r.start + i + 1).unwrap_or(r.end);
                out.push_str(&format!("<u data-pb-open=\"{}\" data-pb-close=\"{}\">", esc(&src[r.start..open_end]), esc(&format!("</{}a>", prefix_of(src, &c)))));
                fb2_html(c, root, src, out); out.push_str("</u>"); continue
            }
            "table" => ("<table>", "</table>"), "tr" => ("<tr>", "</tr>"), "td" => ("<td>", "</td>"), "th" => ("<th>", "</th>"),
            "empty-line" => { out.push_str("<br>"); continue }
            "image" => {
                let raw = esc(&src[c.range()]);
                match attr(&c, "href").and_then(|h| binary(root, h.trim_start_matches('#'))) {
                    Some(b) => out.push_str(&format!("<img src=\"{}\" data-pb-raw=\"{raw}\">", data_url(&b))),
                    None => out.push_str(&format!("<img alt=\"картинка\" data-pb-raw=\"{raw}\">")),
                }
                continue;
            }
            _ => ("<span>", "</span>"),
        };
        out.push_str(open); fb2_html(c, root, src, out); out.push_str(close);
    }
}

/* ---------------------------------------------------------------- проверка и применение правок */

/// Проверить правку части: книга после неё должна читаться
pub fn check_part(bytes: &[u8], fmt: Format, e: &PartEdit) -> Result<(), String> {
    match fmt {
        Format::Epub => parse(&e.source).map(|_| ()).map_err(|x| format!("Глава не читается как XHTML: {}", x.trim_start_matches("XML повреждён: "))),
        Format::Fb2 | Format::Fb2Zip => {
            let (src, _) = fb2_src(bytes, fmt)?;
            let out = replace_sections(&src, fmt, std::slice::from_ref(e), &[])?;
            let doc = parse(&out).map_err(|x| format!("Раздел не читается как FB2: {}", x.trim_start_matches("XML повреждён: ")))?;
            if units(&doc, fmt).len() != units(&parse(&src)?, fmt).len() {
                return Err("Число разделов не должно меняться: в правке должен остаться ровно один раздел (или весь <body>, если разделов нет).".into())
            }
            Ok(())
        }
        _ => Err("в этом формате части не правятся".into()),
    }
}

/// Заменить разделы FB2. dropped — номера разделов, которые уже удалены чисткой (номера считаются по исходной книге).
fn replace_sections(src: &str, fmt: Format, edits: &[PartEdit], dropped: &[usize]) -> Result<String, String> {
    let doc = parse(src)?;
    let secs = units(&doc, fmt);
    let mut ed = Edits::default();
    for e in edits {
        let Some(i) = e.id.strip_prefix('s').and_then(|x| x.parse::<usize>().ok()) else { continue };
        if dropped.contains(&i) { continue }
        let j = i - dropped.iter().filter(|d| **d < i).count();
        if let Some(s) = secs.get(j) { ed.replace(s.range(), if e.delete { String::new() } else { e.source.clone() }) }
    }
    Ok(ed.apply(src))
}

/// Применить правки частей к книге (после чистки — поэтому номера удалённых разделов передаются)
pub fn apply_parts(bytes: Vec<u8>, fmt: Format, edits: &[PartEdit], dropped: &[usize]) -> Result<Vec<u8>, String> {
    if edits.is_empty() { return Ok(bytes) }
    match fmt {
        Format::Epub => {
            let mut z = ZipBook::read(&bytes)?;
            for e in edits.iter().filter(|e| !e.delete) { if z.find(&e.id).is_some() { z.set(&e.id, xml_utf8(&e.source)) } }
            for e in edits.iter().filter(|e| e.delete) { epub_drop(&mut z, &e.id)?; }
            z.write()
        }
        Format::Fb2 => Ok(xml_utf8(&replace_sections(&decode_xml(&bytes), fmt, edits, dropped)?)),
        Format::Fb2Zip => {
            let (src, z) = fb2_src(&bytes, fmt)?;
            let (mut z, n) = z.unwrap();
            z.set(&n, xml_utf8(&replace_sections(&src, fmt, edits, dropped)?));
            z.write()
        }
        _ => Ok(bytes),
    }
}

/// Убрать главу из EPUB: из порядка чтения (spine), списка файлов (manifest), оглавлений (NCX и nav) и сам файл.
/// Последнюю главу убрать нельзя.
fn epub_drop(z: &mut ZipBook, name: &str) -> Result<(), String> {
    let opf = opf_path(z)?;
    let base = dir_of(&opf);
    let src = decode_xml(z.get(&opf).ok_or("нет OPF")?);
    let same = |href: &str, from: &str| resolve(from, &crate::text::url_decode(href.split('#').next().unwrap_or(""))).eq_ignore_ascii_case(name);
    let (new_opf, tocs) = {
        let doc = parse(&src)?;
        let pkg = doc.root_element();
        let man = child(pkg, "manifest").ok_or("OPF без manifest")?;
        let item = children(man, "item").find(|i| same(i.attribute("href").unwrap_or(""), &base)).ok_or("глава не найдена в книге")?;
        let id = item.attribute("id").unwrap_or("");
        let spine = child(pkg, "spine").ok_or("OPF без spine")?;
        let refs: Vec<_> = children(spine, "itemref").collect();
        if refs.len() <= 1 { return Err("Нельзя убрать единственную главу книги".into()) }
        let mut ed = Edits::default();
        ed.delete(item.range());
        for r in refs.iter().filter(|r| r.attribute("idref") == Some(id)) { ed.delete(r.range()) }
        if let Some(g) = child(pkg, "guide") { for r in children(g, "reference").filter(|r| same(r.attribute("href").unwrap_or(""), &base)) { ed.delete(r.range()) } }
        // оглавления: NCX и nav.xhtml — поправим отдельно
        let tocs: Vec<String> = children(man, "item").filter(|i| i.attribute("media-type") == Some("application/x-dtbncx+xml")
            || i.attribute("properties").map(|p| p.split_whitespace().any(|x| x == "nav")).unwrap_or(false))
            .map(|i| resolve(&base, i.attribute("href").unwrap_or(""))).collect();
        (ed.apply(&src), tocs)
    };
    z.set(&opf, xml_utf8(&new_opf));
    for t in tocs {
        let Some(b) = z.get(&t) else { continue };
        let s = decode_xml(b);
        let Ok(doc) = parse(&s) else { continue };
        let from = dir_of(&t);
        // пункты, ведущие на убранную главу (вложенные внутрь уже убираемых не трогаем отдельно)
        let hit = |n: &roxmltree::Node| match n.tag_name().name() {
            "navPoint" => child(*n, "content").and_then(|c| c.attribute("src")).map(|h| same(h, &from)).unwrap_or(false),
            "li" => child(*n, "a").and_then(|a| a.attribute("href")).map(|h| same(h, &from)).unwrap_or(false),
            _ => false,
        };
        let mut ed = Edits::default();
        for n in doc.descendants().filter(|n| n.is_element() && hit(n)) {
            if n.ancestors().skip(1).any(|a| a.is_element() && hit(&a)) { continue }
            ed.delete(n.range());
        }
        if !ed.is_empty() { z.set(&t, xml_utf8(&ed.apply(&s))) }
    }
    z.remove(name);
    Ok(())
}

/* ---------------------------------------------------------------- данные книги */

/// «Имя Отчество Фамилия» → (имя и отчество, фамилия)
fn split_author(a: &str) -> (String, String) {
    let a = a.split_whitespace().collect::<Vec<_>>();
    match a.len() { 0 => (String::new(), String::new()), 1 => (String::new(), a[0].into()), n => (a[..n - 1].join(" "), a[n - 1].into()) }
}

/// Записать новые данные в книгу
pub fn apply_meta(bytes: Vec<u8>, fmt: Format, m: &MetaEdit) -> Result<Vec<u8>, String> {
    if m.is_empty() { return Ok(bytes) }
    match fmt {
        Format::Fb2 => Ok(xml_utf8(&fb2_meta(&decode_xml(&bytes), m)?)),
        Format::Fb2Zip => {
            let (src, z) = fb2_src(&bytes, fmt)?;
            let (mut z, n) = z.unwrap();
            z.set(&n, xml_utf8(&fb2_meta(&src, m)?));
            z.write()
        }
        Format::Epub => {
            let mut z = ZipBook::read(&bytes)?;
            let opf = opf_path(&z)?;
            let s = epub_meta(&decode_xml(z.get(&opf).ok_or("нет OPF")?), m)?;
            z.set(&opf, xml_utf8(&s));
            z.write()
        }
        _ => Err("в этом формате данные книги не меняются".into()),
    }
}

fn fb2_meta(src: &str, m: &MetaEdit) -> Result<String, String> {
    let doc = parse(src)?;
    let ti = path(doc.root_element(), &["description", "title-info"]).ok_or("в книге нет блока title-info")?;
    let p = prefix_of(src, &ti);
    let mut ed = Edits::default();
    let end = inner_range(src, &ti).end;
    if let Some(t) = &m.title {
        let tag = format!("<{p}book-title>{}</{p}book-title>", esc(t.trim()));
        match child(ti, "book-title") { Some(n) => ed.replace(n.range(), tag), None => ed.insert(end, tag) }
    }
    if let Some(a) = &m.author {
        let (f, l) = split_author(a);
        let tag = format!("<{p}author><{p}first-name>{}</{p}first-name><{p}last-name>{}</{p}last-name></{p}author>", esc(&f), esc(&l));
        match child(ti, "author") {
            Some(n) => ed.replace(n.range(), tag),
            None => match child(ti, "book-title") { Some(b) => ed.insert(b.range().start, tag), None => ed.insert(end, tag) },
        }
    }
    if m.series.is_some() || m.number.is_some() {
        let old = child(ti, "sequence");
        let name = m.series.clone().unwrap_or_else(|| old.and_then(|o| o.attribute("name")).unwrap_or("").to_string());
        let num = m.number.clone().unwrap_or_else(|| old.and_then(|o| o.attribute("number")).unwrap_or("").to_string());
        let tag = if name.trim().is_empty() { String::new() } else if num.trim().is_empty() { format!("<{p}sequence name=\"{}\"/>", esc(name.trim())) }
            else { format!("<{p}sequence name=\"{}\" number=\"{}\"/>", esc(name.trim()), esc(num.trim())) };
        match old { Some(o) => ed.replace(o.range(), tag), None => if !tag.is_empty() { ed.insert(end, tag) } }
    }
    Ok(ed.apply(src))
}

fn epub_meta(src: &str, m: &MetaEdit) -> Result<String, String> {
    let doc = parse(src)?;
    let pkg = doc.root_element();
    let md = child(pkg, "metadata").ok_or("OPF без metadata")?;
    let mut ed = Edits::default();
    let end = inner_range(src, &md).end;
    // префикс как у найденного элемента; для нового — dc:
    let dc = |n: Option<roxmltree::Node>| n.map(|n| prefix_of(src, &n)).unwrap_or_else(|| "dc:".into());
    if let Some(t) = &m.title {
        let n = child(md, "title");
        let p = dc(n);
        match n { Some(n) => ed.replace(inner_range(src, &n), esc(t.trim())), None => ed.insert(end, format!("<{p}title>{}</{p}title>", esc(t.trim()))) }
    }
    if let Some(a) = &m.author {
        let n = child(md, "creator");
        let p = dc(n);
        match n { Some(n) => ed.replace(inner_range(src, &n), esc(a.trim())), None => ed.insert(end, format!("<{p}creator>{}</{p}creator>", esc(a.trim()))) }
    }
    if m.series.is_some() || m.number.is_some() {
        let mp = prefix_of(src, &md);
        let named = |n: &str| children(md, "meta").find(|x| x.attribute("name") == Some(n));
        let coll = children(md, "meta").find(|x| x.attribute("property") == Some("belongs-to-collection"));
        if let Some(c) = coll {
            if let Some(s) = &m.series { ed.replace(inner_range(src, &c), esc(s.trim())) }
            if let (Some(nu), Some(cid)) = (&m.number, c.attribute("id")) {
                let r = format!("#{cid}");
                if let Some(gp) = children(md, "meta").find(|x| x.attribute("refines") == Some(r.as_str()) && x.attribute("property") == Some("group-position")) { ed.replace(inner_range(src, &gp), esc(nu.trim())) }
            }
        }
        // calibre:series — его читает и PocketBook
        let set = |ed: &mut Edits, key: &str, val: &str| match named(key) {
            Some(x) => { if val.is_empty() { ed.delete(x.range()) } else { ed.replace(x.range(), format!("<{mp}meta name=\"{key}\" content=\"{}\"/>", esc(val))) } }
            None => if !val.is_empty() { ed.insert(end, format!("<{mp}meta name=\"{key}\" content=\"{}\"/>", esc(val))) },
        };
        if let Some(s) = &m.series { set(&mut ed, "calibre:series", s.trim()) }
        if let Some(n) = &m.number { set(&mut ed, "calibre:series_index", n.trim()) }
    }
    Ok(ed.apply(src))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn book() -> Vec<u8> {
        let mut buf = std::io::Cursor::new(vec![]);
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o: zip::write::SimpleFileOptions = Default::default();
            let mut put = |n: &str, t: &str| { w.start_file(n, o).unwrap(); w.write_all(t.as_bytes()).unwrap(); };
            put("mimetype", "application/epub+zip");
            put("META-INF/container.xml", r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#);
            put("OEBPS/content.opf", r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="a" href="Text/a.xhtml" media-type="application/xhtml+xml"/><item id="b" href="Text/b.xhtml" media-type="application/xhtml+xml"/></manifest><spine toc="ncx"><itemref idref="a"/><itemref idref="b"/></spine></package>"#);
            put("OEBPS/toc.ncx", r#"<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/"><navMap><navPoint id="n1"><navLabel><text>A</text></navLabel><content src="Text/a.xhtml"/></navPoint><navPoint id="n2"><navLabel><text>B</text></navLabel><content src="Text/b.xhtml#x"/></navPoint></navMap></ncx>"#);
            put("OEBPS/Text/a.xhtml", r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>A</p></body></html>"#);
            put("OEBPS/Text/b.xhtml", r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>B</p></body></html>"#);
            w.finish().unwrap();
        }
        buf.into_inner()
    }
    #[test]
    fn epub_chapter_deleted() {
        let out = apply_parts(book(), Format::Epub, &[PartEdit { id: "OEBPS/Text/b.xhtml".into(), source: String::new(), delete: true }], &[]).unwrap();
        let z = ZipBook::read(&out).unwrap();
        assert!(z.get("OEBPS/Text/b.xhtml").is_none());
        let opf = String::from_utf8_lossy(z.get("OEBPS/content.opf").unwrap()).to_string();
        assert!(!opf.contains("b.xhtml") && !opf.contains(r#"idref="b""#) && opf.contains(r#"idref="a""#), "{opf}");
        let ncx = String::from_utf8_lossy(z.get("OEBPS/toc.ncx").unwrap()).to_string();
        assert!(!ncx.contains("b.xhtml") && ncx.contains("a.xhtml"), "{ncx}");
        assert_eq!(parts(&out, Format::Epub).unwrap().len(), 1);
        // последнюю главу убрать нельзя
        assert!(apply_parts(out, Format::Epub, &[PartEdit { id: "OEBPS/Text/a.xhtml".into(), source: String::new(), delete: true }], &[]).is_err());
    }
    #[test]
    fn fb2_section_deleted() {
        let src = r#"<?xml version="1.0" encoding="utf-8"?><FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0"><description><title-info><book-title>T</book-title></title-info></description><body><section><p>one</p></section><section><p>two</p></section><section><p>three</p></section></body></FictionBook>"#;
        let out = apply_parts(src.as_bytes().to_vec(), Format::Fb2, &[PartEdit { id: "s1".into(), source: String::new(), delete: true },
            PartEdit { id: "s2".into(), source: "<section><p>3!</p></section>".into(), delete: false }], &[]).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("one") && !s.contains("two") && s.contains("3!"), "{s}");
    }
    #[test]
    fn fb2_html_keeps_links_and_images() {
        let src = r##"<?xml version="1.0" encoding="utf-8"?><FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink"><body><section><p>см. <a l:href="#n1" type="note">1</a></p><image l:href="#pic"/></section><section><p>2</p></section></body><binary id="pic" content-type="image/png">iVBORw0KGgo=</binary></FictionBook>"##;
        let v = part(src.as_bytes(), Format::Fb2, "s0").unwrap();
        assert!(v.html.contains("data-pb-open=\"&lt;a l:href=&quot;#n1&quot; type=&quot;note&quot;&gt;\""), "{}", v.html);
        assert!(v.html.contains("data-pb-raw=\"&lt;image l:href=&quot;#pic&quot;/&gt;\""), "{}", v.html);
        let r = render(src.as_bytes(), Format::Fb2, "s1", "<section><p>два</p></section>").unwrap();
        assert!(r.contains("<p>два</p>"), "{r}");
    }
}
