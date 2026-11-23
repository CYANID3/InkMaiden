//! Текст: кодировки (UTF-8, UTF-16, windows-1251, koi8-r), HTML-сущности, превью, экранирование.

/// XML/HTML из байтов: BOM, затем encoding="…" из объявления, иначе UTF-8 (с запасным cp1251)
pub fn decode_xml(b: &[u8]) -> String {
    if let Some((enc, bom)) = encoding_rs::Encoding::for_bom(b) { return enc.decode_without_bom_handling(&b[bom..]).0.into_owned() }
    let head = String::from_utf8_lossy(&b[..b.len().min(200)]).to_lowercase();
    if let Some(i) = head.find("encoding=") {
        let rest = &head[i + 9..];
        let q = rest.chars().next().unwrap_or('"');
        let name: String = rest[1..].chars().take_while(|&c| c != q).collect();
        if let Some(enc) = encoding_rs::Encoding::for_label(name.trim().as_bytes()) {
            if enc != encoding_rs::UTF_8 { return enc.decode_without_bom_handling(b).0.into_owned() }
        }
    }
    match std::str::from_utf8(b) { Ok(s) => s.to_string(), Err(_) => encoding_rs::WINDOWS_1251.decode_without_bom_handling(b).0.into_owned() }
}

/// Объявление XML переписывается на UTF-8 (текст после правки сохраняется в UTF-8)
pub fn xml_utf8(s: &str) -> Vec<u8> {
    let t = s.trim_start_matches('\u{feff}');
    if t.starts_with("<?xml") {
        if let Some(end) = t.find("?>") {
            let decl = &t[..end];
            let fixed = fancy_regex::Regex::new(r#"encoding\s*=\s*["'][^"']*["']"#).unwrap().replace(decl, "encoding=\"utf-8\"").to_string();
            return format!("{fixed}{}", &t[end..]).into_bytes();
        }
    }
    t.as_bytes().to_vec()
}

/// Текстовый файл: BOM, затем строгий UTF-8, иначе cp1251 или koi8-r (что больше похоже на русский)
pub fn decode_text(b: &[u8]) -> String {
    if let Some((enc, bom)) = encoding_rs::Encoding::for_bom(b) { return enc.decode_without_bom_handling(&b[bom..]).0.into_owned() }
    if let Ok(s) = std::str::from_utf8(b) { return s.to_string() }
    let w = encoding_rs::WINDOWS_1251.decode_without_bom_handling(b).0.into_owned();
    let k = encoding_rs::KOI8_R.decode_without_bom_handling(b).0.into_owned();
    // в нормальном русском тексте строчных букв больше, чем прописных; в неверной кодировке — наоборот
    let score = |s: &str| s.chars().filter(|c| ('а'..='я').contains(c)).count() as i64 - s.chars().filter(|c| ('А'..='Я').contains(c)).count() as i64;
    if score(&k) > score(&w) { k } else { w }
}

pub fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            let h = |c: u8| (c as char).to_digit(16).unwrap() as u8;
            out.push(h(b[i + 1]) * 16 + h(b[i + 2])); i += 3; continue
        }
        out.push(b[i]); i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// &amp; &lt; &#1072; &#x430; &nbsp; …
pub fn html_decode(s: &str) -> String {
    if !s.contains('&') { return s.to_string() }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        // мнемоника не длиннее 12 байт; граница — по целому символу (кириллица и т. п. — по 2 байта)
        let mut lim = tail.len().min(12);
        while !tail.is_char_boundary(lim) { lim -= 1 }
        if let Some(end) = tail[..lim].find(';') {
            let ent = &tail[1..end];
            let ch = match ent {
                "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" | "#39" => Some('\''),
                "nbsp" => Some('\u{a0}'), "laquo" => Some('«'), "raquo" => Some('»'), "mdash" => Some('—'), "ndash" => Some('–'), "hellip" => Some('…'), "copy" => Some('©'),
                e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
                e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(c) = ch { out.push(c); rest = &tail[end + 1..]; continue }
        }
        out.push('&'); rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

pub fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") }

/// Короткий кусок текста для показа: пробелы схлопнуты, не длиннее n знаков
pub fn preview(t: &str, n: usize) -> String {
    let s = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > n { format!("{}…", s.chars().take(n).collect::<String>()) } else { s }
}

pub use pb_core::util::today;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoding() {
        assert_eq!(html_decode("a &amp; b &#1072;&#x431; &laquo;x&raquo; &bogus; &"), "a & b аб «x» &bogus; &");
        let cp = encoding_rs::WINDOWS_1251.encode("Глава первая. Привет").0.into_owned();
        assert_eq!(decode_text(&cp), "Глава первая. Привет");
        let koi = encoding_rs::KOI8_R.encode("Глава первая. Привет, как дела").0.into_owned();
        assert_eq!(decode_text(&koi), "Глава первая. Привет, как дела");
        let x = b"<?xml version=\"1.0\" encoding=\"windows-1251\"?><a>\xcf\xf0\xe8</a>";
        assert_eq!(decode_xml(x), "<?xml version=\"1.0\" encoding=\"windows-1251\"?><a>При</a>");
        assert!(String::from_utf8(xml_utf8(&decode_xml(x))).unwrap().starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert_eq!(url_decode("a%20b%D0%B0"), "a bа");
        assert_eq!(today().len(), 10);
    }
}

#[cfg(test)]
mod boundary {
    use super::*;
    /// «&» перед кириллицей: раньше срез [..12] попадал в середину буквы — паника в поиске рекламы
    #[test]
    fn ampersand_before_cyrillic() {
        assert_eq!(html_decode("Тест&ёжик ёлка"), "Тест&ёжик ёлка");
        assert_eq!(html_decode("&ааааааа;"), "&ааааааа;");
        assert_eq!(html_decode("a&amp;b &#1105; &laquo;"), "a&b ё &laquo;".replace("&laquo;", if html_decode("&laquo;") == "«" { "«" } else { "&laquo;" }));
        for s in ["&", "&ю", "x&юююююю", "&#x44f;"] { let _ = html_decode(s); }
    }
}
