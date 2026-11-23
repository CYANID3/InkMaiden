//! «Ссылки → текст»: внешние ссылки в книге (http, https, ftp, mailto, www) становятся обычным текстом.
//! Тег `<a …>` убирается, его текст остаётся на месте. Внутренние ссылки (сноски, оглавление: `#…`, файлы книги)
//! не трогаются — без них не работают сноски. Работает по байтам: теги ASCII, поэтому кодировка файла
//! (UTF-8, Windows-1251) не важна и остальной текст остаётся байт в байт.

use crate::container::ZipBook;
use crate::meta::Format;

/// Внешний ли адрес
fn external(href: &[u8]) -> bool {
    let h: Vec<u8> = href.iter().skip_while(|c| c.is_ascii_whitespace()).take(8).map(|c| c.to_ascii_lowercase()).collect();
    ["http:", "https:", "ftp:", "mailto:", "www."].iter().any(|p| h.starts_with(p.as_bytes()))
}

/// Значение атрибута href (в том числе l:href, xlink:href) внутри открывающего тега
fn href_of(tag: &[u8]) -> Option<&[u8]> {
    let low: Vec<u8> = tag.iter().map(|c| c.to_ascii_lowercase()).collect();
    let mut from = 0;
    while let Some(p) = find(&low[from..], b"href") {
        let i = from + p;
        let before = if i == 0 { b' ' } else { low[i - 1] };
        let mut j = i + 4;
        while j < low.len() && low[j].is_ascii_whitespace() { j += 1 }
        if (before.is_ascii_whitespace() || before == b':') && j < low.len() && low[j] == b'=' {
            j += 1;
            while j < low.len() && low[j].is_ascii_whitespace() { j += 1 }
            if j < low.len() && (low[j] == b'"' || low[j] == b'\'') {
                let q = low[j];
                let end = low[j + 1..].iter().position(|&c| c == q).map(|e| j + 1 + e)?;
                return Some(&tag[j + 1..end]);
            }
            let end = low[j..].iter().position(|c| c.is_ascii_whitespace() || *c == b'>' || *c == b'/').map(|e| j + e).unwrap_or(low.len());
            return Some(&tag[j..end]);
        }
        from = i + 4;
    }
    None
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> { hay.windows(needle.len()).position(|w| w == needle) }

/// конец тега: первый «>» вне кавычек
fn tag_end(d: &[u8], from: usize) -> Option<usize> {
    let mut q = 0u8;
    for (k, &c) in d[from..].iter().enumerate() {
        if q != 0 { if c == q { q = 0 } continue }
        match c { b'"' | b'\'' => q = c, b'>' => return Some(from + k), _ => {} }
    }
    None
}

/// начинается ли в позиции i тег a (`<a` или `<xx:a`), и где кончается его имя
fn a_open(d: &[u8], i: usize) -> Option<usize> {
    if d.get(i) != Some(&b'<') { return None }
    let mut j = i + 1;
    // префикс пространства имён: <xhtml:a
    let name_start = j;
    while j < d.len() && (d[j].is_ascii_alphanumeric() || d[j] == b'_' || d[j] == b'-') { j += 1 }
    let (s, e) = if d.get(j) == Some(&b':') { let s = j + 1; let mut e = s; while e < d.len() && d[e].is_ascii_alphanumeric() { e += 1 } (s, e) } else { (name_start, j) };
    if e - s == 1 && (d[s] == b'a' || d[s] == b'A') && d.get(e).map(|c| c.is_ascii_whitespace() || *c == b'>' || *c == b'/').unwrap_or(false) { Some(e) } else { None }
}

/// закрывающий `</a>` (или `</xx:a>`) начиная с from: (начало, конец+1)
fn a_close(d: &[u8], from: usize) -> Option<(usize, usize)> {
    let mut i = from;
    while let Some(p) = d[i..].iter().position(|&c| c == b'<') {
        let s = i + p;
        if d.get(s + 1) == Some(&b'/') {
            let mut j = s + 2;
            while j < d.len() && (d[j].is_ascii_alphanumeric() || d[j] == b':' || d[j] == b'_' || d[j] == b'-') { j += 1 }
            let name = &d[s + 2..j];
            let local = name.rsplit(|&c| c == b':').next().unwrap_or(name);
            if local.eq_ignore_ascii_case(b"a") { let e = tag_end(d, j)?; return Some((s, e + 1)) }
        }
        i = s + 1;
    }
    None
}

/// Убрать внешние ссылки в одном файле разметки: (новые данные, сколько ссылок стало текстом)
pub fn unlink_markup(d: &[u8]) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(d.len());
    let (mut i, mut n) = (0, 0);
    while i < d.len() {
        if d[i] == b'<' {
            if let Some(ne) = a_open(d, i) {
                if let Some(te) = tag_end(d, ne) {
                    let tag = &d[i..=te];
                    if href_of(tag).map(external).unwrap_or(false) {
                        n += 1;
                        if d[te - 1] == b'/' { i = te + 1; continue } // <a href="…"/> — пустая ссылка, просто убрать
                        if let Some((cs, ce)) = a_close(d, te + 1) {
                            // текст ссылки остаётся (вложенные теги оформления — тоже)
                            out.extend_from_slice(&d[te + 1..cs]);
                            i = ce; continue;
                        }
                        i = te + 1; continue; // незакрытая — убрать только открывающий тег
                    }
                }
            }
        }
        out.push(d[i]); i += 1;
    }
    (out, n)
}

/// Книга целиком: FB2 — сам файл, FB2.ZIP/EPUB/FB3 — файлы разметки внутри архива
pub fn unlink(data: &[u8], fmt: Format) -> Result<(Vec<u8>, usize), String> {
    match fmt {
        Format::Fb2 => Ok(unlink_markup(data)),
        Format::Fb2Zip | Format::Epub | Format::Fb3 => {
            let mut z = ZipBook::read(data)?;
            let mut total = 0;
            for e in z.entries.iter_mut() {
                let l = e.name.to_ascii_lowercase();
                if !(l.ends_with(".fb2") || l.ends_with(".xhtml") || l.ends_with(".html") || l.ends_with(".htm") || (fmt == Format::Fb3 && l.ends_with(".xml"))) { continue }
                let (d, n) = unlink_markup(&e.data);
                if n > 0 { e.data = d; total += n }
            }
            Ok((if total > 0 { z.write()? } else { data.to_vec() }, total))
        }
        Format::Txt => Ok((data.to_vec(), 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(b: &[u8]) -> String { String::from_utf8(b.to_vec()).unwrap() }

    #[test]
    fn external_links_become_text() {
        let src = r##"<p>Скачано с <a l:href="http://example.com/x">сайта <emphasis>Пример</emphasis></a>, сноска<a l:href="#n1" type="note">[1]</a>.</p>"##;
        let (d, n) = unlink_markup(src.as_bytes());
        assert_eq!(n, 1);
        assert_eq!(s(&d), r##"<p>Скачано с сайта <emphasis>Пример</emphasis>, сноска<a l:href="#n1" type="note">[1]</a>.</p>"##);
    }

    #[test]
    fn html_variants() {
        let src = "<p><A HREF='https://x.ru'>x</A> <xhtml:a href=\"mailto:a@b\">почта</xhtml:a> <a href=\"ch2.xhtml#t\">гл. 2</a> <abbr>a</abbr> <a href=\"www.y.ru\"/></p>";
        let (d, n) = unlink_markup(src.as_bytes());
        assert_eq!(n, 3);
        assert_eq!(s(&d), "<p>x почта <a href=\"ch2.xhtml#t\">гл. 2</a> <abbr>a</abbr> </p>");
    }

    #[test]
    fn cp1251_bytes_untouched() {
        let mut src = b"<p>\xcf\xf0\xe8\xe2\xe5\xf2 <a href=\"http://z\">\xf1\xe0\xe9\xf2</a></p>".to_vec();
        let (d, n) = unlink_markup(&src);
        assert_eq!(n, 1);
        src = b"<p>\xcf\xf0\xe8\xe2\xe5\xf2 \xf1\xe0\xe9\xf2</p>".to_vec();
        assert_eq!(d, src);
    }

    #[test]
    fn epub_inside_zip() {
        use crate::container::{Entry, ZipBook};
        let z = ZipBook { entries: vec![
            Entry { name: "mimetype".into(), data: b"application/epub+zip".to_vec(), stored: true },
            Entry { name: "OEBPS/c1.xhtml".into(), data: "<p><a href=\"https://t.me/x\">канал</a> и <a href=\"c2.xhtml\">дальше</a></p>".as_bytes().to_vec(), stored: false },
            Entry { name: "OEBPS/content.opf".into(), data: b"<package/>".to_vec(), stored: false },
        ] };
        let (d, n) = unlink(&z.write().unwrap(), Format::Epub).unwrap();
        assert_eq!(n, 1);
        let r = ZipBook::read(&d).unwrap();
        assert_eq!(s(r.get("OEBPS/c1.xhtml").unwrap()), "<p>канал и <a href=\"c2.xhtml\">дальше</a></p>");
        assert_eq!(r.entries[0].name, "mimetype");
    }
}
