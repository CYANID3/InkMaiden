//! Подробности о подключённой читалке, которые можно прочитать с её диска: настройки из global.cfg,
//! что занимает память (книги, комиксы, звук, картинки, система), приложения, шрифты, словари.
//! Медленно на больших дисках — вызывается в отдельном потоке.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

const BOOKS: &[&str] = &["fb2", "epub", "fb3", "pdf", "djvu", "djv", "mobi", "azw", "azw3", "txt", "rtf", "doc", "docx", "odt", "chm", "html", "htm", "prc"];
const COMICS: &[&str] = &["cbz", "cbr", "cb7", "cbt"];
const AUDIO: &[&str] = &["mp3", "m4a", "m4b", "ogg", "oga", "flac", "wav", "aac", "opus"];
const IMAGES: &[&str] = &["jpg", "jpeg", "png", "bmp", "gif", "webp", "tif", "tiff"];

/// Вид файла по имени (zip с «fb2» внутри имени — книга)
pub fn kind_of(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.ends_with(".fb2.zip") { return "books" }
    let ext = n.rsplit('.').next().unwrap_or("");
    if BOOKS.contains(&ext) { "books" } else if COMICS.contains(&ext) { "comics" } else if AUDIO.contains(&ext) { "audio" } else if IMAGES.contains(&ext) { "images" } else { "other" }
}

#[derive(Default, Clone, Copy)]
struct Sum { count: u64, bytes: u64 }

fn walk(dir: &Path, depth: usize, f: &mut dyn FnMut(&Path, &str, u64)) {
    if depth > 24 { return }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(m) = e.metadata() else { continue };
        let name = e.file_name().to_string_lossy().to_string();
        if m.is_dir() { walk(&e.path(), depth + 1, f) } else { f(&e.path(), &name, m.len()) }
    }
}

fn names(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| keep(n)).collect()).unwrap_or_default();
    v.sort_by_key(|n| n.to_lowercase());
    v
}

/// Настройки, которые интересно показать: шрифт, подсветка, обновления, словарь, раскладки
fn settings(root: &Path) -> Value {
    let g = crate::devices::read_kv(&root.join("system/config/global.cfg"));
    let get = |k: &str| g.get(k).cloned().filter(|v| !v.is_empty());
    let num = |k: &str| get(k).and_then(|v| v.parse::<i64>().ok());
    let kbd: Vec<String> = (1..=6).filter_map(|i| get(&format!("kbdlayout.{i}"))).filter(|v| !v.starts_with('@')).collect();
    let partner = std::fs::read_to_string(root.join("system/config/desktop/apps.json")).ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.get("partner").and_then(|p| p.as_str()).map(String::from));
    let check = std::fs::read_to_string(root.join("system/last_update_check")).ok().and_then(|t| t.trim().parse::<i64>().ok());
    json!({
        "font": get("font").map(|f| f.trim_end_matches(".ttf").trim_end_matches("-Regular").to_string()),
        "readerFont": get("fbreader.font.name.default"), "readerFontSize": num("reader.font.size"),
        "frontlight": num("FrontLightLastState"), "warmth": num("FrontLightLastColor"),
        "autoUpdate": get("auto_check_update").map(|v| v == "1"), "updateInterval": get("update_interval").map(|v| v.trim_start_matches('@').to_string()),
        "lastUpdateCheck": check, "dictionary": get("dictionary"), "keyboards": kbd,
        "batteryPercent": get("show_battery_percentage").map(|v| v == "1"), "statistics": get("statistics_enabled").map(|v| v == "1"),
        "partner": partner,
    })
}

/// Всё о читалке, что видно с её диска (кроме библиотеки — она в базе данных, см. программу)
pub fn read(root: &Path) -> Value { read_with(root, &|_| {}) }

/// То же с прогрессом: progress(просмотрено файлов) примерно раз в 300 файлов
pub fn read_with(root: &Path, progress: &dyn Fn(u64)) -> Value {
    let seen = std::cell::Cell::new(0u64);
    let tick = || { let n = seen.get() + 1; seen.set(n); if n.is_multiple_of(300) { progress(n) } };
    let mut sums: BTreeMap<&'static str, Sum> = BTreeMap::new();
    let mut system = Sum::default();
    let sys = root.join("system");
    let Ok(rd) = std::fs::read_dir(root) else { return json!({ "error": "диск не читается" }) };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(m) = e.metadata() else { continue };
        if p == sys || name.starts_with('.') || name.eq_ignore_ascii_case("System Volume Information") {
            walk(&p, 0, &mut |_, _, len| { tick(); system.count += 1; system.bytes += len });
            if !m.is_dir() { system.count += 1; system.bytes += m.len() }
            continue;
        }
        if m.is_dir() {
            walk(&p, 0, &mut |_, n, len| { tick(); let s = sums.entry(kind_of(n)).or_default(); s.count += 1; s.bytes += len });
        } else { let s = sums.entry(kind_of(&name)).or_default(); s.count += 1; s.bytes += m.len() }
    }
    let label = |k: &str| match k { "books" => "Книги", "comics" => "Комиксы и манга", "audio" => "Звук и аудиокниги", "images" => "Картинки", _ => "Другое" };
    let mut content: Vec<Value> = ["books", "comics", "audio", "images", "other"].iter().map(|k| {
        let s = sums.get(k).copied().unwrap_or_default();
        json!({ "key": k, "label": label(k), "count": s.count, "bytes": s.bytes })
    }).collect();
    content.push(json!({ "key": "system", "label": "Система", "count": system.count, "bytes": system.bytes }));
    let apps = names(&root.join("applications"), |n| n.to_lowercase().ends_with(".app"))
        .into_iter().map(|n| n[..n.len() - 4].to_string()).collect::<Vec<_>>();
    let fonts = names(&sys.join("fonts"), |n| { let l = n.to_lowercase(); l.ends_with(".ttf") || l.ends_with(".otf") });
    let dicts = names(&sys.join("dictionaries"), |n| !n.starts_with('.'))
        .into_iter().map(|n| n.trim_end_matches(".dic").to_string()).collect::<Vec<_>>();
    // версия прошивки: прошивка кладёт system/config/firmware/<версия>.json — его видно и по USB
    let firmware = std::fs::read_dir(root.join("system/config/firmware")).ok().and_then(|rd| {
        let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort(); names.iter().rev().find_map(|n| crate::diag::fw_token(n))
    });
    json!({ "at": crate::devices::now(), "settings": settings(root), "content": content, "apps": apps, "fonts": fonts.len(), "dictionaries": dicts, "firmware": firmware })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_disk() {
        let r = std::env::temp_dir().join(format!("pbdet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        let w = |p: &str, n: usize| { let f = r.join(p); std::fs::create_dir_all(f.parent().unwrap()).unwrap(); std::fs::write(f, vec![b'x'; n]).unwrap(); };
        w("Books/a.fb2", 10); w("Books/b.fb2.zip", 5); w("Manga/c.cbz", 100); w("Music/d.mp3", 7); w("x.jpg", 3);
        w("system/fonts/A.ttf", 50); w("system/config/global.cfg", 0);
        std::fs::write(r.join("system/config/global.cfg"), "font=Roboto-Regular.ttf\nFrontLightLastState=60\nkbdlayout.1=RU: Russian\nkbdlayout.2=@KA_none\n").unwrap();
        w("applications/Bloknot.app", 1);
        let v = read(&r);
        let c = |k: &str| v["content"].as_array().unwrap().iter().find(|x| x["key"] == k).unwrap().clone();
        assert_eq!(c("books")["count"], 2); assert_eq!(c("books")["bytes"], 15);
        assert_eq!(c("comics")["bytes"], 100); assert_eq!(c("audio")["count"], 1); assert_eq!(c("images")["count"], 1);
        assert_eq!(c("system")["count"], 2);
        assert_eq!(v["fonts"], 1); assert_eq!(v["apps"][0], "Bloknot");
        assert_eq!(v["settings"]["font"], "Roboto"); assert_eq!(v["settings"]["frontlight"], 60);
        assert_eq!(v["settings"]["keyboards"].as_array().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(r);
    }
}
