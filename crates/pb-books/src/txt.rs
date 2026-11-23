//! TXT → FB2: кодировка угадывается, абзацы — по пустым строкам или по строкам, главы — по заголовкам
//! («Глава 1», «ЧАСТЬ ВТОРАЯ», «Пролог», «Chapter 3», римские цифры, «* * *» — разрыв внутри главы).

use crate::meta::Meta;
use crate::text::{esc, today};
use fancy_regex::Regex;

pub fn to_fb2(text: &str, meta: &Meta) -> Vec<u8> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let blank = lines.iter().filter(|l| l.trim().is_empty()).count();
    let head = Regex::new(r"(?i)^(глава|часть|книга|пролог|эпилог|послесловие|предисловие|интерлюдия|chapter|part|prologue|epilogue)\b|^[IVXLC]{1,6}\.?$|^\d{1,3}\.?$").unwrap();
    let is_head = |p: &str| p.chars().count() <= 80 && head.is_match(p).unwrap_or(false);
    let mut paras: Vec<String> = vec![];
    if blank > lines.len() / 8 {
        // абзацы разделены пустыми строками, внутри — перенос по ширине; заголовок в первой строке — отдельно
        for blk in Regex::new(r"\n\s*\n").unwrap().split(&text).flatten() {
            let mut ls: Vec<&str> = blk.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            if ls.len() > 1 && is_head(ls[0]) { paras.push(ls.remove(0).to_string()) }
            let p = ls.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
            if !p.is_empty() { paras.push(p) }
        }
    } else {
        for l in &lines { let p = l.split_whitespace().collect::<Vec<_>>().join(" "); if !p.is_empty() { paras.push(p) } }
    }
    let brk = Regex::new(r"^[*\s]{3,}$|^[-–—\s]{3,}$").unwrap();
    let mut body = String::new();
    let mut open = false;
    for p in &paras {
        if is_head(p) {
            if open { body.push_str("</section>\n") }
            body.push_str(&format!("<section><title><p>{}</p></title>\n", esc(p)));
            open = true;
        } else if brk.is_match(p).unwrap_or(false) {
            if open { body.push_str("<subtitle>* * *</subtitle>\n") }
        } else {
            if !open { body.push_str("<section>\n"); open = true }
            body.push_str(&format!("<p>{}</p>\n", esc(p)));
        }
    }
    if open { body.push_str("</section>\n") } else { body.push_str("<section><p></p></section>\n") }
    let (fname, lname) = match meta.author.trim().rsplit_once(char::is_whitespace) { Some((f, l)) => (f.trim().to_string(), l.to_string()), None => (String::new(), meta.author.trim().to_string()) };
    let seq = if meta.series.is_empty() { String::new() } else {
        let num = if meta.number.is_empty() { String::new() } else { format!(" number=\"{}\"", esc(&meta.number)) };
        format!("<sequence name=\"{}\"{num}/>", esc(&meta.series))
    };
    let id = format!("inkmaiden-{:x}-{:x}", pb_core::devices::now(), text.len());
    format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<FictionBook xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\" xmlns:l=\"http://www.w3.org/1999/xlink\">\n<description>\n<title-info><genre>prose</genre><author><first-name>{}</first-name><last-name>{}</last-name></author><book-title>{}</book-title><lang>ru</lang>{seq}</title-info>\n<document-info><author><nickname>InkMaiden</nickname></author><date>{}</date><id>{id}</id><version>1.0</version></document-info>\n</description>\n<body>\n<title><p>{}</p></title>\n{body}</body>\n</FictionBook>\n",
        esc(&fname), esc(&lname), esc(&meta.title), today(), esc(&meta.title)).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chapters_and_paragraphs() {
        let t = "Глава 1\r\nПервая строка\r\nвторая строка\r\n\r\nНовый абзац.\r\n\r\n* * *\r\n\r\nПосле разрыва.\r\n\r\nII\r\n\r\nТекст второй главы & <тег>.";
        let m = Meta { author: "Иван Петров".into(), title: "Тест".into(), series: "Серия".into(), number: "2".into(), ..Default::default() };
        let x = String::from_utf8(to_fb2(t, &m)).unwrap();
        assert!(x.contains("<section><title><p>Глава 1</p></title>"));
        assert!(x.contains("<p>Первая строка вторая строка</p>"));
        assert!(x.contains("<subtitle>* * *</subtitle>"));
        assert!(x.contains("<section><title><p>II</p></title>"));
        assert!(x.contains("&amp; &lt;тег&gt;"));
        assert!(x.contains("<first-name>Иван</first-name><last-name>Петров</last-name>"));
        assert!(x.contains("<sequence name=\"Серия\" number=\"2\"/>"));
        crate::xml::parse(&x).unwrap();
        let fb = crate::meta::fb2_meta(&x).unwrap();
        assert_eq!(fb.title, "Тест");
    }
}
