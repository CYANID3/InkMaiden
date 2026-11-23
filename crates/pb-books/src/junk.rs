//! Чистка от рекламы: страницы магазинов и издателей, рекламные абзацы, внешние ссылки.
//! Каждая находка — отдельный пункт с текстом для проверки. Уверенные отмечены сразу, сомнительные — нет.
//! Удаляются только отмеченные; ссылки превращаются в обычный текст (текст не теряется). Порт part2c_clean.ps1.

use crate::container::{dir_of, file_name, resolve, ZipBook};
use crate::meta::{fb2_in_zip, fb3_body, opf_path, Format};
use crate::text::{decode_xml, html_decode, preview, xml_utf8};
use crate::xml::{attr, child, children, inner_range, is, parse, Edits};
use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

struct Rx { strong: Regex, publisher: Regex, fragment: Regex, chapter: Regex, chapter_start: Regex, call: Regex, domain: Regex, ext_href: Regex, link_a: Regex, para: Regex, script: Regex, body: Regex, br: Regex, tags: Regex, img: Regex }

fn rx() -> &'static Rx {
    static R: OnceLock<Rx> = OnceLock::new();
    R.get_or_init(|| Rx {
        strong: Regex::new(concat!(r"(?i)litres|литрес|лит-рес|ozon\.|wildberries|bookmate|букмейт|mybook|storytel|readli|author\.today|litnet|литнет|ridero|флибуст|flibusta|",
            r"купит[ье]|покупк|приобрести|магазин|подпис(ывайтесь|ка|ку|аться|чик)|промокод|скидк|бесплатн(о|ые|ый|ая)\s+(чит|скач|книг|доступ)|",
            r"скача(ть|йте)|чита(йте|ть)\s+(нас|также|другие|онлайн|полностью)|наш(ем|) (сайт|канал|групп)|мы в соц|соцсет|telegram|телеграм|t\.me/|vk\.com|vk\.cc|вконтакте|",
            r"instagram|youtube|boosty|patreon|донат|реклам|партн[её]р|электронн(ая|ую) (верси|книг)\w* (предостав|подготов)|конвертирова|fb2 ?(создан|подготов)|",
            r"лучшие книги|новинки (недели|месяца)|оформи(те|ть)|переходите|по ссылке|www\.|https?://")).unwrap(),
        publisher: Regex::new(r"(?i)isbn|ббк|удк|все права защищены|охраняется законом|©|\(c\)\s*\d|литературное агентство|тираж|подписано в печать").unwrap(),
        fragment: Regex::new(r"(?i)ознакомительн(ый|ого) фрагмент|конец ознакомительного|полную версию (книги|можно)").unwrap(),
        chapter: Regex::new(r"(?im)^\s*(глава|часть|пролог|эпилог|chapter|part|prologue)\b").unwrap(),
        chapter_start: Regex::new(r"(?i)^\s*(глава|часть|пролог|эпилог|chapter|part|prologue)\b").unwrap(),
        call: Regex::new(r"(?i)читайте|подписывайтесь|скачивайте|покупайте|переходите|заходите|присоединяйтесь|поддержите").unwrap(),
        // адрес сайта или магазин по имени — без этого «купить» и «магазин» в обычном тексте рекламой не считаются
        domain: Regex::new(r"(?i)https?://|www\.|\b[\w-]+\.(ru|com|net|org|me|io|su|рф|by|kz|ua)\b|litres|литрес|bookmate|t\.me/|vk\.com|boosty|patreon").unwrap(),
        ext_href: Regex::new(r"(?i)^\s*(https?:|www\.|mailto:|ftp:)").unwrap(),
        link_a: Regex::new(r#"(?is)<a\b([^>]*?)\bhref\s*=\s*(["'])([^"']*)\2([^>]*)>(.*?)</a\s*>"#).unwrap(),
        para: Regex::new(r"(?is)<(p|div)\b[^>]*>((?:(?!<(?:p|div)\b).)*?)</\1\s*>").unwrap(),
        script: Regex::new(r"(?is)<(script|style)\b.*?</\1>").unwrap(),
        body: Regex::new(r"(?is)<body\b[^>]*>(.*)</body>").unwrap(),
        br: Regex::new(r"(?i)<(br|/p|/div|/h\d|/li|/tr)\b[^>]*>").unwrap(),
        tags: Regex::new(r"<[^>]+>").unwrap(),
        img: Regex::new(r"(?i)[\w\-. %]+\.(jpe?g|png|gif|svg|webp)").unwrap(),
    })
}

/// Что именно убрать
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Target { Page { file: String }, Sec { i: usize }, Para { file: String, raw: String }, P { i: usize }, Links }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub kind: String,
    pub label: String,
    pub preview: String,
    pub sure: bool,
    pub on: bool,
    pub chars: usize,
    pub target: Target,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Junk { pub findings: Vec<Finding>, pub total: usize, pub fragment: bool }

impl Junk {
    /// краткая подпись: «2 стр., 1 абз., ссылок 14»
    pub fn summary(&self) -> String {
        let f = &self.findings;
        let pg = f.iter().filter(|x| x.kind == "page" && x.sure).count();
        let pq = f.iter().filter(|x| x.kind == "page" && !x.sure).count();
        let pa = f.iter().filter(|x| x.kind == "para").count();
        let mut parts = vec![];
        if pg > 0 { parts.push(format!("{pg} стр.")) }
        if pa > 0 { parts.push(format!("{pa} абз.")) }
        if pq > 0 { parts.push(format!("{pq} под вопросом")) }
        if let Some(l) = f.iter().find(|x| x.kind == "links") { parts.push(format!("ссылок {}", l.chars)) }
        parts.join(", ")
    }
    /// есть что убирать (кроме одних ссылок-сомнений)
    pub fn worth(&self) -> bool { self.findings.iter().any(|x| x.kind != "links" || x.sure) }
}

fn hits(t: &str, r: &Regex) -> usize {
    let mut seen = HashSet::new();
    for m in r.find_iter(t).flatten() { seen.insert(m.as_str().to_lowercase()); }
    seen.len()
}
fn m(r: &Regex, t: &str) -> bool { r.is_match(t).unwrap_or(false) }

pub fn plain_text(html: &str) -> String {
    let r = rx();
    let s = r.script.replace_all(html, " ");
    let s = match r.body.captures(&s) { Ok(Some(c)) => c[1].to_string(), _ => s.to_string() };
    let s = r.br.replace_all(&s, "\n");
    let s = r.tags.replace_all(&s, " ");
    let s = html_decode(&s);
    s.split('\n').map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Оценка «страницы» (файл EPUB или раздел FB2): (уверенно?, подпись, конец фрагмента?)
fn judge_page(text: &str, ext: usize) -> Option<(bool, &'static str, bool)> {
    let r = rx();
    let len = text.chars().count();
    if len > 2500 { return None }
    let chapter = m(&r.chapter, text) && len > 400;
    let strong = hits(text, &r.strong);
    let pubh = hits(text, &r.publisher);
    let dom = m(&r.domain, text);
    if m(&r.fragment, text) { return Some((dom || ext >= 1, "Конец ознакомительного фрагмента — реклама полной версии", true)) }
    if chapter { return None }
    // начинается с «Глава …» и без адресов — это текст книги, даже если короткий
    if m(&r.chapter_start, text) && !dom && ext == 0 { return None }
    if len < 40 && ext >= 1 { return Some((true, "Баннер со ссылкой", false)) }
    if (strong >= 2 && (dom || ext >= 1)) || (strong >= 1 && ext >= 1) || (ext >= 2 && len < 600) { return Some((true, "Реклама магазина или издателя", false)) }
    if strong >= 2 || ext >= 1 || (strong >= 1 && dom) { return Some((false, "Возможно, реклама", false)) }
    if pubh >= 2 { return Some((false, "Служебная страница издателя (©, ISBN) — обычно не мешает", false)) }
    None
}

/// Абзац внутри текста: только с адресом/ссылкой — обычные фразы героев не трогаем
fn judge_para(text: &str, link: bool) -> Option<bool> {
    let r = rx();
    let len = text.chars().count();
    if !(8..=500).contains(&len) { return None }
    let strong = hits(text, &r.strong);
    let dom = m(&r.domain, text);
    if (strong >= 1 && link) || (strong >= 2 && dom) { return Some(true) }
    if strong >= 1 && dom && m(&r.call, text) { return Some(false) }
    None
}

fn finding(kind: &str, label: String, pv: String, sure: bool, chars: usize, target: Target) -> Finding {
    Finding { kind: kind.into(), label, preview: pv, sure, on: sure, chars, target }
}
/// внешние ссылки: в предпросмотре — сами адреса (по одному в строке, без повторов)
fn links_finding(urls: Vec<String>) -> Finding {
    let n = urls.len();
    let mut uniq: Vec<String> = vec![];
    for u in urls { if !uniq.contains(&u) { uniq.push(u) } }
    let more = uniq.len().saturating_sub(200);
    uniq.truncate(200);
    let mut pv = uniq.join("\n");
    if more > 0 { pv.push_str(&format!("\n… и ещё {more}")) }
    finding("links", format!("Внешние ссылки: {n} — станут обычным текстом"), pv, true, n, Target::Links)
}

/* ---------------------------------------------------------------- EPUB */

struct EpubDocs { opf: String, dir: String, docs: Vec<String> }

fn epub_docs(z: &ZipBook) -> Result<EpubDocs, String> {
    let opf = opf_path(z)?;
    let s = decode_xml(z.get(&opf).ok_or("нет OPF")?);
    let doc = parse(&s)?;
    let pkg = doc.root_element();
    let dir = dir_of(&opf);
    let man: HashMap<&str, roxmltree::Node> = child(pkg, "manifest").map(|mf| children(mf, "item").filter_map(|i| Some((i.attribute("id")?, i))).collect()).unwrap_or_default();
    let mut docs = vec![];
    if let Some(sp) = child(pkg, "spine") {
        for ir in children(sp, "itemref") {
            let Some(it) = ir.attribute("idref").and_then(|id| man.get(id)) else { continue };
            if it.attribute("properties").unwrap_or("").split_whitespace().any(|p| p == "nav") { continue }
            let p = resolve(&dir, it.attribute("href").unwrap_or(""));
            if let Some(e) = z.find(&p) { docs.push(e.name.clone()) }
        }
    }
    Ok(EpubDocs { opf, dir, docs })
}

fn html_of(z: &ZipBook, name: &str) -> String { z.get(name).map(|b| String::from_utf8_lossy(b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(b)).into_owned()).unwrap_or_default() }

fn ext_links(html: &str) -> Vec<String> {
    let r = rx();
    r.link_a.captures_iter(html).flatten().filter(|c| m(&r.ext_href, &c[3])).map(|c| crate::text::html_decode(c[3].trim())).collect()
}

fn find_epub(z: &ZipBook) -> Result<Junk, String> {
    let r = rx();
    let info = epub_docs(z)?;
    let n = info.docs.len();
    let mut res = Junk::default();
    let mut dropped = HashSet::new();
    for (k, d) in info.docs.iter().enumerate() {
        let html = html_of(z, d);
        let text = plain_text(&html);
        res.total += text.chars().count();
        let ext = ext_links(&html).len();
        if let Some((sure, label, frag)) = judge_page(&text, ext) {
            res.fragment |= frag;
            let pv = if text.is_empty() { "(только картинка)".into() } else { preview(&text, 1500) };
            res.findings.push(finding("page", format!("{label} · стр. {} из {n}", k + 1), pv, sure, text.chars().count(), Target::Page { file: d.clone() }));
            dropped.insert(d.clone());
            continue;
        }
        for c in r.para.find_iter(&html).flatten() {
            let raw = c.as_str();
            let pt = plain_text(raw);
            let link = Regex::new(r#"(?i)href\s*=\s*["']\s*(https?:|www\.)"#).unwrap().is_match(raw).unwrap_or(false);
            if let Some(sure) = judge_para(&pt, link) {
                res.findings.push(finding("para", "Рекламный абзац".into(), preview(&pt, 800), sure, pt.chars().count(), Target::Para { file: d.clone(), raw: raw.to_string() }));
            }
        }
    }
    let links: Vec<String> = info.docs.iter().filter(|d| !dropped.contains(*d)).flat_map(|d| ext_links(&html_of(z, d))).collect();
    if !links.is_empty() { res.findings.push(links_finding(links)) }
    Ok(res)
}

fn remove_epub(z: &mut ZipBook, f: &[Finding]) -> Result<usize, String> {
    let r = rx();
    let info = epub_docs(z)?;
    let drop: HashSet<String> = f.iter().filter_map(|x| if let Target::Page { file } = &x.target { Some(file.clone()) } else { None }).collect();
    let unlink = f.iter().any(|x| x.target == Target::Links);
    let mut removed = 0;
    // абзацы и ссылки в оставшихся файлах
    for d in &info.docs {
        if drop.contains(d) { continue }
        let orig = html_of(z, d);
        let mut html = orig.clone();
        for x in f {
            if let Target::Para { file, raw } = &x.target {
                if file == d { if let Some(i) = html.find(raw.as_str()) { html.replace_range(i..i + raw.len(), ""); removed += x.chars; } }
            }
        }
        if unlink {
            html = r.link_a.replace_all(&html, |c: &fancy_regex::Captures| if m(&r.ext_href, &c[3]) { c[5].to_string() } else { c[0].to_string() }).into_owned();
        }
        if html != orig { z.set(d, html.into_bytes()) }
    }
    if drop.is_empty() { return Ok(removed) }
    // картинки, на которые ссылались только удаляемые страницы (OPF и NCX перечисляют всё — по ним не судим)
    let mut in_drop = HashSet::new(); let mut in_keep = HashSet::new();
    for e in &z.entries {
        let l = e.name.to_lowercase();
        if !(l.ends_with(".html") || l.ends_with(".xhtml") || l.ends_with(".htm") || l.ends_with(".css") || l.ends_with(".svg")) { continue }
        let t = String::from_utf8_lossy(&e.data);
        let bag = if drop.contains(&e.name) { &mut in_drop } else { &mut in_keep };
        for mm in r.img.find_iter(&t).flatten() { bag.insert(crate::text::url_decode(file_name(mm.as_str())).to_lowercase()); }
    }
    let src = decode_xml(z.get(&info.opf).unwrap());
    let doc = parse(&src)?;
    let pkg = doc.root_element();
    let mut orphan: HashSet<String> = in_drop.difference(&in_keep).cloned().collect();
    // обложку не трогаем никогда
    if let Some(mf) = child(pkg, "manifest") {
        for it in children(mf, "item").filter(|i| i.attribute("properties").unwrap_or("").split_whitespace().any(|p| p == "cover-image")) {
            orphan.remove(&file_name(it.attribute("href").unwrap_or("")).to_lowercase());
        }
    }
    let mut ed = Edits::default();
    let mut drop_ids = HashSet::new();
    let mut gone = vec![];
    if let Some(mf) = child(pkg, "manifest") {
        for it in children(mf, "item") {
            let p = resolve(&info.dir, it.attribute("href").unwrap_or(""));
            let img = it.attribute("media-type").unwrap_or("").starts_with("image/") && orphan.contains(&file_name(&p).to_lowercase());
            if drop.contains(&p) || img { drop_ids.insert(it.attribute("id").unwrap_or("").to_string()); ed.delete(it.range()); gone.push(p); }
        }
    }
    if let Some(sp) = child(pkg, "spine") { for ir in children(sp, "itemref") { if drop_ids.contains(ir.attribute("idref").unwrap_or("")) { ed.delete(ir.range()); } } }
    if let Some(g) = child(pkg, "guide") { for rf in children(g, "reference") { if drop.contains(&resolve(&info.dir, rf.attribute("href").unwrap_or(""))) { ed.delete(rf.range()); } } }
    let out = ed.apply(&src);
    z.set(&info.opf, xml_utf8(&out));
    for p in gone { z.remove(&p) }
    removed += f.iter().filter(|x| x.kind == "page").map(|x| x.chars).sum::<usize>();
    // оглавления: toc.ncx и nav.xhtml
    let names: HashSet<String> = drop.iter().map(|d| file_name(d).to_lowercase()).collect();
    let hits_name = |href: &str| names.contains(&file_name(&crate::text::url_decode(href.split('#').next().unwrap_or(""))).to_lowercase());
    let tocs: Vec<String> = z.entries.iter().map(|e| e.name.clone()).filter(|n| { let l = n.to_lowercase(); l.ends_with(".ncx") || l.ends_with(".xhtml") || l.ends_with(".html") || l.ends_with(".htm") }).collect();
    for name in tocs {
        let t = decode_xml(z.get(&name).unwrap());
        if name.to_lowercase().ends_with(".ncx") {
            let Ok(nd) = parse(&t) else { continue };
            let mut e = Edits::default();
            for np in nd.descendants().filter(|n| is(n, "navPoint")) {
                if child(np, "content").and_then(|c| c.attribute("src")).map(hits_name).unwrap_or(false) { e.delete(np.range()); }
            }
            if e.is_empty() { continue }
            let mut out = e.apply(&t);
            // playOrder по порядку
            let po = Regex::new(r#"playOrder\s*=\s*["']\d+["']"#).unwrap();
            let mut k = 0;
            out = po.replace_all(&out, |_: &fancy_regex::Captures| { k += 1; format!("playOrder=\"{k}\"") }).into_owned();
            z.set(&name, xml_utf8(&out));
        } else if Regex::new(r#"(?i)epub:type\s*=\s*["']toc"#).unwrap().is_match(&t).unwrap_or(false) {
            let li = Regex::new(r#"(?is)<li\b[^>]*>\s*<a\b[^>]*href\s*=\s*["']([^"']*)["'][^>]*>.*?</a>\s*</li>"#).unwrap();
            let out = li.replace_all(&t, |c: &fancy_regex::Captures| if hits_name(&c[1]) { String::new() } else { c[0].to_string() }).into_owned();
            if out != t { z.set(&name, out.into_bytes()) }
        }
    }
    Ok(removed)
}

/* ---------------------------------------------------------------- FB2 / FB3 */

pub(crate) fn main_sections<'a, 'i>(doc: &'a roxmltree::Document<'i>, fmt: Format) -> Vec<roxmltree::Node<'a, 'i>> {
    let root = doc.root_element();
    if fmt == Format::Fb3 { return children(root, "section").collect() }
    children(root, "body").find(|b| b.attribute("name").is_none()).map(|b| children(b, "section").collect()).unwrap_or_default()
}
fn ext_a<'a, 'i>(n: roxmltree::Node<'a, 'i>) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> {
    n.descendants().filter(|a| is(a, "a") && m(&rx().ext_href, attr(a, "href").unwrap_or("")))
}

fn find_xml(src: &str, fmt: Format) -> Result<Junk, String> {
    let doc = parse(src)?;
    let secs = main_sections(&doc, fmt);
    let n = secs.len();
    let mut res = Junk { total: crate::xml::text(&doc.root_element()).chars().count(), ..Default::default() };
    let mut dropped = vec![];
    for (i, s) in secs.iter().enumerate() {
        let text = crate::xml::text(s);
        if let Some((sure, label, frag)) = judge_page(&text, ext_a(*s).count()) {
            res.fragment |= frag;
            res.findings.push(finding("page", format!("{label} · раздел {} из {n}", i + 1), preview(&text, 1500), sure, text.chars().count(), Target::Sec { i }));
            dropped.push(s.range());
        }
    }
    // абзацы: в аннотации и в тексте, кроме разделов, которые уже предложено убрать
    for (k, p) in doc.descendants().filter(|x| is(x, "p")).enumerate() {
        if dropped.iter().any(|r| r.contains(&p.range().start)) { continue }
        let pt = crate::xml::text(&p);
        if let Some(sure) = judge_para(&pt, ext_a(p).next().is_some()) {
            let label = if p.ancestors().any(|a| is(&a, "annotation")) { "Реклама в аннотации" } else { "Рекламный абзац" };
            res.findings.push(finding("para", label.into(), preview(&pt, 800), sure, pt.chars().count(), Target::P { i: k }));
        }
    }
    let links: Vec<String> = ext_a(doc.root_element()).filter_map(|a| attr(&a, "href").map(|h| h.trim().to_string())).collect();
    if !links.is_empty() { res.findings.push(links_finding(links)) }
    Ok(res)
}

fn remove_xml(src: &str, fmt: Format, f: &[Finding]) -> Result<(String, usize), String> {
    let doc = parse(src)?;
    let secs = main_sections(&doc, fmt);
    let ps: Vec<_> = doc.descendants().filter(|x| is(x, "p")).collect();
    let refs = |d: &roxmltree::Document| -> HashSet<String> {
        d.descendants().flat_map(|n| n.attributes().filter(|a| a.name() == "href").map(|a| a.value().to_string()).collect::<Vec<_>>())
            .filter_map(|v| v.strip_prefix('#').map(String::from)).collect()
    };
    let before = refs(&doc);
    let mut ed = Edits::default();
    let mut removed = 0;
    for x in f {
        match &x.target {
            Target::Sec { i } if *i < secs.len() => { ed.delete(secs[*i].range()); removed += x.chars }
            Target::P { i } if *i < ps.len() => { ed.delete(ps[*i].range()); removed += x.chars }
            _ => {}
        }
    }
    if f.iter().any(|x| x.target == Target::Links) {
        for a in ext_a(doc.root_element()) { let inner = inner_range(src, &a); ed.replace(a.range(), src[inner].to_string()); }
    }
    let mut out = ed.apply(src);
    // FB2: картинки, которые остались без ссылок после удаления
    if fmt != Format::Fb3 {
        let d2 = parse(&out)?;
        let after = refs(&d2);
        let mut e2 = Edits::default();
        for b in children(d2.root_element(), "binary") {
            let id = b.attribute("id").unwrap_or("");
            if before.contains(id) && !after.contains(id) { e2.delete(b.range()); }
        }
        if !e2.is_empty() { out = e2.apply(&out); }
    }
    Ok((out, removed))
}

/* ---------------------------------------------------------------- общее */

pub fn find(bytes: &[u8], fmt: Format) -> Result<Junk, String> {
    match fmt {
        Format::Fb2 => find_xml(&decode_xml(bytes), fmt),
        Format::Fb2Zip => { let z = ZipBook::read(bytes)?; find_xml(&decode_xml(z.get(&fb2_in_zip(&z)?).unwrap()), Format::Fb2) }
        Format::Fb3 => { let z = ZipBook::read(bytes)?; find_xml(&decode_xml(z.get(&fb3_body(&z)?).unwrap()), Format::Fb3) }
        Format::Epub => find_epub(&ZipBook::read(bytes)?),
        Format::Txt => Ok(Junk::default()),
    }
}

/// Убрать отмеченное (on). Возвращает новую книгу и число убранных знаков.
pub fn remove(bytes: &[u8], fmt: Format, findings: &[Finding]) -> Result<(Vec<u8>, usize), String> {
    let f: Vec<Finding> = findings.iter().filter(|x| x.on).cloned().collect();
    if f.is_empty() { return Ok((bytes.to_vec(), 0)) }
    match fmt {
        Format::Fb2 => { let (s, n) = remove_xml(&decode_xml(bytes), fmt, &f)?; Ok((xml_utf8(&s), n)) }
        Format::Fb2Zip | Format::Fb3 => {
            let mut z = ZipBook::read(bytes)?;
            let name = if fmt == Format::Fb3 { fb3_body(&z)? } else { fb2_in_zip(&z)? };
            let (s, n) = remove_xml(&decode_xml(z.get(&name).unwrap()), if fmt == Format::Fb3 { Format::Fb3 } else { Format::Fb2 }, &f)?;
            z.set(&name, xml_utf8(&s));
            Ok((z.write()?, n))
        }
        Format::Epub => { let mut z = ZipBook::read(bytes)?; let n = remove_epub(&mut z, &f)?; Ok((z.write()?, n)) }
        Format::Txt => Ok((bytes.to_vec(), 0)),
    }
}
