//! Вшить обложку (JPEG) в книгу. Логика как в pb-tools: старая обложка заменяется, в EPUB ставятся обе пометки
//! (EPUB 3 properties и EPUB 2 meta name="cover"), страница-обложка добавляется, только если своей не было.

use crate::container::{dir_of, resolve, ZipBook};
use crate::meta::{opf_path, THUMB};
use crate::text::{decode_xml, xml_utf8};
use crate::xml::{attr, child, children, inner_range, is, parse, path, prefix_of, Edits};
use base64::Engine;

const XLINK: &str = "http://www.w3.org/1999/xlink";

/// FB2: текст книги → текст с новой обложкой
pub fn fb2(src: &str, jpeg: &[u8]) -> Result<String, String> {
    let doc = parse(src)?;
    let root = doc.root_element();
    let ti = path(root, &["description", "title-info"]).ok_or("в книге нет блока title-info")?;
    let mut ed = Edits::default();
    // старая обложка и её картинки
    for cp in children(ti, "coverpage") {
        for img in children(cp, "image") {
            let id = attr(&img, "href").unwrap_or("").trim_start_matches('#').to_string();
            for b in children(root, "binary").filter(|b| b.attribute("id") == Some(id.as_str())) { ed.delete(b.range()); }
        }
        ed.delete(cp.range());
    }
    for b in children(root, "binary").filter(|b| b.attribute("id") == Some("pbcover.jpg")) { ed.delete(b.range()); }
    // префикс xlink в корне (обычно l: или xlink:)
    let xl = root.namespaces().find(|n| n.uri() == XLINK && n.name().is_some()).and_then(|n| n.name()).map(String::from);
    let xl = match xl { Some(p) => p, None => {
        let r = root.range(); let tag_end = r.start + 1 + root.tag_name().name().len() + prefix_of(src, &root).len();
        ed.insert(tag_end, format!(" xmlns:l=\"{XLINK}\""));
        "l".into()
    } };
    let pfx = prefix_of(src, &ti);
    let cover = format!("<{pfx}coverpage><{pfx}image {xl}:href=\"#pbcover.jpg\"/></{pfx}coverpage>");
    let before = ti.children().find(|c| c.is_element() && matches!(c.tag_name().name(), "lang" | "src-lang" | "translator" | "sequence"));
    match before { Some(b) => ed.insert(b.range().start, cover), None => ed.insert(inner_range(src, &ti).end, cover) }
    let bin = format!("\n<{pfx}binary id=\"pbcover.jpg\" content-type=\"image/jpeg\">{}</{pfx}binary>\n", base64::engine::general_purpose::STANDARD.encode(jpeg));
    ed.insert(inner_range(src, &root).end, bin);
    Ok(ed.apply(src))
}

pub fn epub(z: &mut ZipBook, jpeg: &[u8]) -> Result<(), String> {
    let opf = opf_path(z)?;
    let src = decode_xml(z.get(&opf).ok_or("нет OPF")?);
    let doc = parse(&src)?;
    let pkg = doc.root_element();
    let dir = dir_of(&opf);
    let (md, mf, sp) = (child(pkg, "metadata"), child(pkg, "manifest"), child(pkg, "spine"));
    let (Some(md), Some(mf), Some(sp)) = (md, mf, sp) else { return Err("OPF без metadata/manifest/spine".into()) };
    // новые элементы — с тем же префиксом, что у соседей (<opf:item>): простые разборщики ищут теги по имени
    let (pm, pf, ps) = (prefix_of(&src, &md), prefix_of(&src, &mf), prefix_of(&src, &sp));
    let mut ed = Edits::default();
    let items: Vec<_> = children(mf, "item").collect();
    let had_ours = items.iter().any(|i| i.attribute("id") == Some("pbcover-img"));
    let had_page = items.iter().any(|i| i.attribute("id") == Some("pbcover-page"));
    let mut had_foreign = false;
    // прежняя картинка обложки (по cover-image и по <meta name="cover">) — страницы книги, которые её показывают, переведём на новую
    let cover_ids: Vec<&str> = children(md, "meta").filter(|m| m.attribute("name") == Some("cover")).filter_map(|m| m.attribute("content")).collect();
    let old_imgs: Vec<String> = items.iter()
        .filter(|i| i.attribute("id") != Some("pbcover-img"))
        .filter(|i| i.attribute("properties").unwrap_or("").split_whitespace().any(|p| p == "cover-image")
            || i.attribute("id").map(|id| cover_ids.contains(&id)).unwrap_or(false)
            // бывает <meta name="cover" content="Images/cover.jpg"> — путь вместо id
            || i.attribute("href").map(|h| cover_ids.contains(&h)).unwrap_or(false))
        .filter_map(|i| i.attribute("href")).map(|h| resolve(&dir, h)).collect();
    let mut old_imgs = old_imgs;
    // страница-обложка книги (по <guide type="cover"> или первая страница с «cover» в имени): картинки на ней — тоже прежняя обложка
    let by_id = |id: &str| items.iter().find(|i| i.attribute("id") == Some(id)).and_then(|i| i.attribute("href")).map(|h| resolve(&dir, h));
    let mut cover_pages: Vec<String> = child(pkg, "guide").map(|g| children(g, "reference").filter(|x| x.attribute("type") == Some("cover")).filter_map(|x| x.attribute("href")).map(|h| resolve(&dir, h)).collect()).unwrap_or_default();
    if let Some(first) = children(sp, "itemref").next().and_then(|r| r.attribute("idref")).and_then(by_id) {
        if first.to_lowercase().contains("cover") || first.to_lowercase().contains("titlepage") { cover_pages.push(first); }
    }
    let img_re = fancy_regex::Regex::new(r#"(?:xlink:href|src|href)\s*=\s*["']([^"']+\.(?:jpe?g|png|gif|webp))["']"#).unwrap();
    for cp in &cover_pages {
        if cp.ends_with("pbcover.xhtml") { continue }
        if let Some(raw) = z.get(cp) {
            let t = decode_xml(raw);
            let pdir = dir_of(cp);
            for c in img_re.captures_iter(&t).flatten() { if let Some(m) = c.get(1) { let r = resolve(&pdir, m.as_str()); if !r.ends_with("pbcover.jpg") && !old_imgs.contains(&r) { old_imgs.push(r); } } }
        }
    }
    if !cover_pages.is_empty() && !old_imgs.is_empty() { had_foreign = true; }
    let pages: Vec<String> = items.iter()
        .filter(|i| matches!(i.attribute("media-type"), Some("application/xhtml+xml") | Some("text/html")))
        .filter_map(|i| i.attribute("href")).map(|h| resolve(&dir, h)).collect();
    for it in &items {
        let props = it.attribute("properties").unwrap_or("");
        if props.split_whitespace().any(|p| p == "cover-image") && it.attribute("id") != Some("pbcover-img") {
            had_foreign = true;
            // убрать cover-image из properties: элемент переписывается целиком
            let rest: Vec<&str> = props.split_whitespace().filter(|p| *p != "cover-image").collect();
            let raw = &src[it.range()];
            let re = fancy_regex::Regex::new(r#"\s+properties\s*=\s*(["'])[^"']*\1"#).unwrap();
            let new = if rest.is_empty() { re.replace(raw, "").to_string() } else { re.replace(raw, format!(" properties=\"{}\"", rest.join(" ")).as_str()).to_string() };
            ed.replace(it.range(), new);
        }
    }
    for cm in children(md, "meta").filter(|m| m.attribute("name") == Some("cover")) {
        if cm.attribute("content") != Some("pbcover-img") { had_foreign = true; }
        ed.delete(cm.range());
    }
    // картинка в той же папке, что и OPF
    z.set(&format!("{dir}pbcover.jpg"), jpeg.to_vec());
    if let Some(it) = items.iter().find(|i| i.attribute("id") == Some("pbcover-img")) { ed.delete(it.range()); }
    let mut add_items = format!("<{pf}item id=\"pbcover-img\" href=\"pbcover.jpg\" media-type=\"image/jpeg\" properties=\"cover-image\"/>");
    let meta = format!("<{pm}meta name=\"cover\" content=\"pbcover-img\"/>");
    match children(md, "meta").next() { Some(m) => ed.insert(m.range().start, meta), None => ed.insert(inner_range(&src, &md).end, meta) }
    let mut page = had_page;
    if !had_foreign && !had_page && !had_ours {
        let xhtml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\">\n<head><title>Обложка</title>\n<style type=\"text/css\">html,body{margin:0;padding:0;height:100%;text-align:center}img{max-width:100%;max-height:100%}</style>\n</head>\n<body><div><img src=\"pbcover.jpg\" alt=\"Обложка\"/></div></body>\n</html>\n";
        z.set(&format!("{dir}pbcover.xhtml"), xhtml.as_bytes().to_vec());
        add_items.push_str(&format!("<{pf}item id=\"pbcover-page\" href=\"pbcover.xhtml\" media-type=\"application/xhtml+xml\"/>"));
        let ir = format!("<{ps}itemref idref=\"pbcover-page\"/>");
        match sp.children().find(|c| c.is_element()) { Some(f) => ed.insert(f.range().start, ir), None => ed.insert(inner_range(&src, &sp).end, ir) }
        page = true;
    }
    ed.insert(inner_range(&src, &mf).end, add_items);
    // <guide><reference type="cover"> — старая пометка EPUB 2, по ней часть читалок ищет обложку
    if page {
        let r = format!("<{ps}reference type=\"cover\" title=\"Обложка\" href=\"pbcover.xhtml\"/>");
        match child(pkg, "guide") {
            Some(g) => {
                for x in children(g, "reference").filter(|x| x.attribute("type") == Some("cover")) { ed.delete(x.range()); }
                ed.insert(inner_range(&src, &g).start, r);
            }
            None => ed.insert(sp.range().end, format!("<{ps}guide>{r}</{ps}guide>")),
        }
    }
    let out = ed.apply(&src);
    parse(&out).map_err(|e| format!("после правки OPF испорчен: {e}"))?;
    z.set(&opf, xml_utf8(&out));
    // страница-обложка самой книги (cover.xhtml и т. п.) показывала старую картинку — теперь новую
    if !old_imgs.is_empty() { retarget(z, &pages, &old_imgs, &format!("{dir}pbcover.jpg"), jpeg_dims(jpeg)); }
    Ok(())
}

/// ширина и высота JPEG (по заголовку SOF)
fn jpeg_dims(b: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xFF { i += 1; continue }
        let m = b[i + 1];
        if (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 && m != 0xCC {
            let hh = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
            return (w > 0 && hh > 0).then_some((w, hh));
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        i += 2 + len;
    }
    None
}
fn svg_fit(t: &str, w: u32, hh: u32) -> String {
    let vb = fancy_regex::Regex::new(r#"viewBox\s*=\s*(["'])[^"']*\1"#).unwrap();
    let t = vb.replace_all(t, format!("viewBox=\"0 0 {w} {hh}\"").as_str()).to_string();
    let par = fancy_regex::Regex::new(r#"preserveAspectRatio\s*=\s*(["'])[^"']*\1"#).unwrap();
    let t = par.replace_all(&t, "preserveAspectRatio=\"xMidYMid meet\"").to_string();
    let img = fancy_regex::Regex::new(r#"<(\w+:)?image\b[^>]*>"#).unwrap();
    img.replace_all(&t, |c: &fancy_regex::Captures| {
        let e = &c[0];
        let wr = fancy_regex::Regex::new(r#"\swidth\s*=\s*(["'])[^"']*\1"#).unwrap();
        let hr = fancy_regex::Regex::new(r#"\sheight\s*=\s*(["'])[^"']*\1"#).unwrap();
        let e = wr.replace(e, format!(" width=\"{w}\"").as_str()).to_string();
        hr.replace(&e, format!(" height=\"{hh}\"").as_str()).to_string()
    }).to_string()
}

/// путь к файлу `to` относительно папки `from_dir` (оба — внутри архива)
fn relative(from_dir: &str, to: &str) -> String {
    let a: Vec<&str> = from_dir.split('/').filter(|x| !x.is_empty()).collect();
    let b: Vec<&str> = to.split('/').filter(|x| !x.is_empty()).collect();
    let mut i = 0;
    while i < a.len() && i + 1 < b.len() && a[i] == b[i] { i += 1; }
    let mut out: Vec<String> = vec!["..".into(); a.len() - i];
    out.extend(b[i..].iter().map(|x| x.to_string()));
    out.join("/")
}

/// в страницах книги заменить ссылки (src / href / xlink:href) на прежнюю картинку обложки ссылкой на новую
fn retarget(z: &mut ZipBook, pages: &[String], old: &[String], new_img: &str, dims: Option<(u32, u32)>) {
    let re = fancy_regex::Regex::new(r#"((?:xlink:)?href|src)\s*=\s*(["'])([^"']+)\2"#).unwrap();
    for page in pages {
        let Some(raw) = z.get(page) else { continue };
        let text = decode_xml(raw);
        let pdir = dir_of(page);
        let mut changed = false;
        let out = re.replace_all(&text, |c: &fancy_regex::Captures| {
            let v = &c[3];
            if old.iter().any(|o| o.eq_ignore_ascii_case(&resolve(&pdir, v))) {
                changed = true;
                format!("{}={}{}{}", &c[1], &c[2], relative(&pdir, new_img), &c[2])
            } else { c[0].to_string() }
        }).to_string();
        if !changed { continue }
        // SVG-обложка подогнана под размер прежней картинки (viewBox, width/height у <image>) — под новую, без растяжения
        let out = match dims { Some((w, hh)) if out.contains("<svg") => svg_fit(&out, w, hh), _ => out };
        z.set(page, xml_utf8(&out));
    }
}

pub fn fb3(z: &mut ZipBook, jpeg: &[u8]) -> Result<(), String> {
    let rels = decode_xml(z.get("_rels/.rels").ok_or("нет _rels/.rels")?);
    let rd = parse(&rels)?;
    let root = rd.root_element();
    let mut ed = Edits::default();
    for r in root.children().filter(|n| is(n, "Relationship") && n.attribute("Type") == Some(THUMB)) { ed.delete(r.range()); }
    ed.insert(inner_range(&rels, &root).end, format!("<Relationship Id=\"rIdPbCover\" Type=\"{THUMB}\" Target=\"fb3/img/pbcover.jpg\"/>"));
    let new = ed.apply(&rels);
    z.set("fb3/img/pbcover.jpg", jpeg.to_vec());
    z.set("_rels/.rels", xml_utf8(&new));
    if let Some(ct) = z.get("[Content_Types].xml").map(decode_xml) {
        let cd = parse(&ct)?;
        let r = cd.root_element();
        if !r.children().any(|d| is(&d, "Default") && d.attribute("Extension").map(|e| e.eq_ignore_ascii_case("jpg")).unwrap_or(false)) {
            let mut e = Edits::default();
            e.insert(inner_range(&ct, &r).start, "<Default Extension=\"jpg\" ContentType=\"image/jpeg\"/>");
            let out = e.apply(&ct);
            z.set("[Content_Types].xml", xml_utf8(&out));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn mini() -> ZipBook {
        let mut buf = std::io::Cursor::new(vec![]);
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o: zip::write::SimpleFileOptions = Default::default();
            let mut put = |n: &str, t: &str| { w.start_file(n, o).unwrap(); w.write_all(t.as_bytes()).unwrap(); };
            put("mimetype", "application/epub+zip");
            put("META-INF/container.xml", r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#);
            put("OEBPS/content.opf", r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title><meta name="cover" content="img"/></metadata><manifest><item id="img" href="Images/cover.png" media-type="image/png" properties="cover-image"/><item id="cp" href="Text/cover.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="cp"/></spine></package>"#);
            put("OEBPS/Text/cover.xhtml", r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><img src="../Images/cover.png"/></body></html>"#);
            put("OEBPS/Images/cover.png", "x");
            w.finish().unwrap();
        }
        ZipBook::read(&buf.into_inner()).unwrap()
    }
    #[test]
    fn cover_page_shows_new_image() {
        let mut z = mini();
        epub(&mut z, b"JPEG").unwrap();
        let page = String::from_utf8_lossy(z.get("OEBPS/Text/cover.xhtml").unwrap()).to_string();
        assert!(page.contains(r#"src="../pbcover.jpg""#), "{page}");
        assert_eq!(z.get("OEBPS/pbcover.jpg").unwrap(), b"JPEG");
        let opf = String::from_utf8_lossy(z.get("OEBPS/content.opf").unwrap()).to_string();
        assert!(opf.contains("pbcover-img") && !opf.contains(r#"properties="cover-image"/><item id="cp""#));
    }
    #[test]
    fn cover_page_without_marks() {
        // обложка не помечена в OPF вовсе — только страница cover.xhtml первой в книге (SVG-картинка)
        let mut buf = std::io::Cursor::new(vec![]);
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o: zip::write::SimpleFileOptions = Default::default();
            let mut put = |n: &str, t: &str| { w.start_file(n, o).unwrap(); w.write_all(t.as_bytes()).unwrap(); };
            put("mimetype", "application/epub+zip");
            put("META-INF/container.xml", r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#);
            put("content.opf", r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="2.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata><manifest><item id="c" href="cover.xhtml" media-type="application/xhtml+xml"/><item id="i" href="img/c.jpeg" media-type="image/jpeg"/></manifest><spine><itemref idref="c"/></spine></package>"#);
            put("cover.xhtml", r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:xlink="http://www.w3.org/1999/xlink"><body><svg><image xlink:href="img/c.jpeg"/></svg></body></html>"#);
            put("img/c.jpeg", "x");
            w.finish().unwrap();
        }
        let mut z = ZipBook::read(&buf.into_inner()).unwrap();
        epub(&mut z, b"JPEG").unwrap();
        let page = String::from_utf8_lossy(z.get("cover.xhtml").unwrap()).to_string();
        assert!(page.contains(r#"xlink:href="pbcover.jpg""#), "{page}");
        // своей лишней страницы-обложки не добавили: первой остаётся страница книги
        let opf = String::from_utf8_lossy(z.get("content.opf").unwrap()).to_string();
        assert!(!opf.contains("pbcover-page"), "{opf}");
    }
    #[test]
    fn svg_cover_resized() {
        let t = r#"<svg viewBox="0 0 600 800" preserveAspectRatio="none"><image width="600" height="800" xlink:href="a.jpg"/></svg>"#;
        let o = svg_fit(t, 1200, 1700);
        assert!(o.contains(r#"viewBox="0 0 1200 1700""#) && o.contains(r#"width="1200""#) && o.contains(r#"height="1700""#) && o.contains("xMidYMid meet"), "{o}");
        // SOF0: FFD8, FFC0 len, precision, h=0x0010, w=0x0020
        assert_eq!(jpeg_dims(&[0xFF, 0xD8, 0xFF, 0xC0, 0, 17, 8, 0, 0x10, 0, 0x20, 3, 0, 0, 0, 0]), Some((32, 16)));
    }
    #[test]
    fn rel_paths() {
        assert_eq!(relative("OEBPS/Text/", "OEBPS/pbcover.jpg"), "../pbcover.jpg");
        assert_eq!(relative("OEBPS/", "OEBPS/pbcover.jpg"), "pbcover.jpg");
        assert_eq!(relative("", "pbcover.jpg"), "pbcover.jpg");
    }
}
