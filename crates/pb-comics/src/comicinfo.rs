//! ComicInfo.xml — карточка комикса внутри архива; PocketBook берёт из неё название.
//! Старые поля сохраняются, Title заменяется, серия/том/глава дописываются, если их не было.

fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") }

pub struct Meta { pub title: String, pub series: String, pub volume: String, pub number: String }

pub fn build(existing: Option<&[u8]>, m: &Meta, pages: usize) -> Vec<u8> {
    let mut fields: Vec<(String, String)> = Vec::new();
    if let Some(b) = existing {
        let t = String::from_utf8_lossy(b);
        if let Some(body) = t.split_once("<ComicInfo").and_then(|(_, r)| r.split_once('>')).map(|(_, r)| r) {
            let re = fancy_regex::Regex::new(r"(?s)<(\w+)>(.*?)</\1>").unwrap();
            for c in re.captures_iter(body).flatten() { fields.push((c[1].to_string(), c[2].to_string())) }
        }
    }
    let mut set = |k: &str, v: &str, replace: bool| {
        if v.is_empty() { return }
        if let Some(f) = fields.iter_mut().find(|(n, _)| n == k) { if replace { f.1 = esc(v) } } else { fields.push((k.into(), esc(v))) }
    };
    set("Title", &m.title, true);
    set("Series", &m.series, false);
    set("Volume", m.volume.trim_start_matches('0'), false);
    set("Number", m.number.trim_start_matches('0'), false);
    set("PageCount", &pages.to_string(), true);
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ComicInfo xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">\n");
    for (k, v) in fields { if k == "Pages" { continue } s.push_str(&format!("  <{k}>{v}</{k}>\n")) }
    s.push_str("</ComicInfo>\n");
    s.into_bytes()
}
