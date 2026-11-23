//! Читалки, подключённые к компьютеру, и запомненные устройства.
//! Здесь — то, что не зависит от Windows: что прочитать в корне диска читалки и как хранить профиль.
//! Перебор дисков, метку тома и свободное место даёт программа (app/src/usb.rs).

use crate::models::{norm_code, Models};
use crate::readers::Reader;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Файл-метка, который InkMaiden кладёт на читалку при «Запомнить»: по нему отличаются две одинаковые модели.
pub const MARK: &str = "system/config/inkmaiden.json";

/// Что видно на подключённой читалке. Неизвестное — None, ничего не придумывается.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Seen {
    /// корень диска: "E:\\"
    pub root: String,
    /// метка тома: PB743K3
    pub label: String,
    /// код модели: 743K3
    pub code: String,
    /// название из справочника, если модель известна
    pub model: Option<String>,
    pub total: Option<u64>,
    pub free: Option<u64>,
    /// как читалка представляется по USB: изготовитель, изделие, ревизия (= ядро Linux, 0409 → 4.9)
    pub usb_vendor: Option<String>,
    pub usb_product: Option<String>,
    pub usb_revision: Option<String>,
    pub kernel: Option<String>,
    /// путь к файлу с фото устройства (system/device<код>.ico)
    pub photo: Option<String>,
    pub language: Option<String>,
    pub timezone: Option<String>,
    pub night_mode: Option<bool>,
    pub theme: Option<String>,
    /// время последнего обновления прошивки (unix, секунды)
    pub last_update: Option<i64>,
    /// заставки: сколько картинок для сна, включения, выключения и стоят ли свои
    pub logos: BTreeMap<String, Value>,
    /// id из файла-метки, если читалку уже запоминали
    pub mark: Option<String>,
}

pub fn read_kv(path: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(path).unwrap_or_default().lines()
        .filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect()
}

/// «0409» → «4.9», «0310» → «3.10»
pub fn kernel_from_rev(rev: &str) -> Option<String> {
    let r = rev.trim();
    if r.len() != 4 || !r.chars().all(|c| c.is_ascii_digit()) { return None }
    let (a, b) = (r[..2].parse::<u32>().ok()?, r[2..].parse::<u32>().ok()?);
    if a == 0 { return None }
    Some(format!("{a}.{b}"))
}

/// Это читалка PocketBook? Признак — файл system/.pocketbook (на Verse и Color 3 он пустой).
pub fn is_pocketbook(root: &Path) -> bool { root.join("system/.pocketbook").exists() }

/// Прочитать всё, что известно, из корня диска. `label` — метка тома (её даёт Windows).
pub fn scan_root(root: &Path, label: &str, models: &Models) -> Seen {
    let sys = root.join("system");
    let mut s = Seen { root: root.display().to_string(), label: label.to_string(), ..Default::default() };
    // код модели: из метки тома, иначе из имени значка device<код>.ico
    let icos: Vec<String> = std::fs::read_dir(&sys).map(|rd| rd.flatten().filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| { let l = n.to_lowercase(); l.starts_with("device") && l.ends_with(".ico") }).collect()).unwrap_or_default();
    let from_label = norm_code(label);
    s.code = if label.to_uppercase().starts_with("PB") && from_label.len() >= 3 { from_label } else {
        icos.iter().map(|n| norm_code(&n[6..n.len() - 4])).max_by_key(|c| c.len()).unwrap_or_default()
    };
    s.model = models.get(&s.code).map(|m| m.full_name());
    let want = format!("device{}.ico", s.code.to_lowercase());
    s.photo = icos.iter().find(|n| n.to_lowercase() == want).or(icos.first()).map(|n| sys.join(n).display().to_string());
    let g = read_kv(&sys.join("config/global.cfg"));
    s.language = g.get("language").cloned();
    s.timezone = g.get("timezone").cloned();
    s.theme = g.get("theme").cloned();
    s.night_mode = g.get("night_mode").map(|v| v == "1");
    s.last_update = std::fs::read_to_string(sys.join("last_update")).ok().and_then(|t| t.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok());
    if let Some(v) = std::fs::read_to_string(sys.join("config/logos_customizer/logos.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        if let Some(o) = v.get("logos").and_then(|x| x.as_object()) {
            for (k, x) in o {
                let n = x.get("random").and_then(|r| r.as_array()).map(|a| a.len()).unwrap_or(0);
                s.logos.insert(k.clone(), serde_json::json!({ "random": n, "custom": x.get("custom").is_some() }));
            }
        }
    }
    for k in ["offlogo", "bootlogo"] { if let Some(v) = g.get(k) { s.logos.insert(format!("{k}_setting"), Value::String(v.clone())); } }
    s.mark = std::fs::read_to_string(root.join(MARK)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("id").and_then(|x| x.as_str()).map(String::from));
    s
}

/// Куда класть файлы этой читалки: на компьютере (абсолютные пути) и на самой читалке (от корня).
/// Ключи: books, comics, manga, wallpapers, sleep. Пусто — по умолчанию.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DevicePaths {
    /// куда сохранять готовое на компьютере (по видам: books, comics, manga, wallpapers, sleep, other)
    #[serde(default)]
    pub pc: BTreeMap<String, String>,
    /// куда загружать на читалке (путь от её корня)
    #[serde(default)]
    pub reader: BTreeMap<String, String>,
    /// откуда брать исходники для этой читалки
    #[serde(default)]
    pub src: BTreeMap<String, String>,
    /// свои пути под своими названиями
    #[serde(default)]
    pub saved: Vec<NamedPath>,
    /// версия умолчаний: 1 — обои на читалке по умолчанию в «Wallpapers» (было system/logo/offlogo)
    #[serde(default)]
    pub v: u32,
}

impl DevicePaths {
    /// Перевести старые умолчания на новые (один раз). Возвращает true, если что-то поменялось.
    pub fn migrate(&mut self) -> bool {
        if self.v >= 1 { return false }
        if self.reader.get("wallpapers").map(|w| w == "system/logo/offlogo").unwrap_or(false) { self.reader.insert("wallpapers".into(), "Wallpapers".into()); }
        self.v = 1;
        true
    }
}

/// Путь под своим названием, например «Скачанное» и папка загрузок браузера
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct NamedPath { pub name: String, pub path: String }

/// Иконка сна на этой читалке: что стоит и что ждёт подключения
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SleepState {
    /// работа из каталога, которая встанет при следующем подключении
    #[serde(default)]
    pub pending: Option<String>,
    /// что стоит сейчас и когда поставлено
    #[serde(default)]
    pub installed: Option<String>,
    #[serde(default)]
    pub installed_at: Option<i64>,
}

/// Сохранённая читалка (Readers/PB743K3.json): профиль для обработки, сведения с последнего подключения, папки.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    /// PB743K3, для второй такой же — PB743K3-2 (это и имя файла)
    pub id: String,
    pub name: String,
    pub code: String,
    /// модель определена по самой читалке
    pub verified: bool,
    pub remembered: i64,
    pub last_seen: i64,
    pub seen: Seen,
    pub reader: Reader,
    #[serde(default)]
    pub paths: DevicePaths,
    /// версия прошивки, если её вписали вручную (по USB её не видно)
    #[serde(default)]
    pub firmware: Option<String>,
    #[serde(default)]
    pub sleep: SleepState,
    /// подробности с диска читалки (память по видам файлов, настройки, библиотека) — последний снимок
    #[serde(default)]
    pub details: Option<Value>,
    /// когда отключили в последний раз
    #[serde(default)]
    pub last_off: Option<i64>,
}

pub fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }

/// Папки читалки по умолчанию: на компьютере — подпапки папки читалки, на читалке — существующие папки, если есть
pub const KINDS: [(&str, &str); 6] = [("books", "Books"), ("comics", "Comics"), ("manga", "Manga"), ("wallpapers", "Wallpapers"), ("sleep", "Sleep_icons"), ("other", "Other")];

pub fn default_reader_paths(root: &Path) -> BTreeMap<String, String> {
    let pick = |c: &[&str]| c.iter().find(|n| root.join(n).is_dir()).unwrap_or(&c[0]).to_string();
    let mut m = BTreeMap::new();
    m.insert("books".into(), pick(&["Books", "Библиотека", "Книги"]));
    m.insert("comics".into(), pick(&["Comics", "Комиксы"]));
    m.insert("manga".into(), pick(&["Manga", "Манга"]));
    m.insert("wallpapers".into(), pick(&["Wallpapers", "Обои"]));
    m.insert("other".into(), pick(&["Download", "Other"]));
    m
}

impl Device {
    pub fn from_seen(seen: &Seen, models: &Models, id: &str, name: Option<&str>) -> Option<Device> {
        let m = models.get(&seen.code)?;
        let name = name.map(String::from).unwrap_or_else(|| m.name.clone());
        let mut reader = m.to_reader(id, &name);
        reader.source = "device".into();
        let paths = DevicePaths { reader: default_reader_paths(Path::new(&seen.root)), v: 1, ..Default::default() };
        Some(Device { id: id.into(), name, code: m.code.clone(), verified: true, remembered: now(), last_seen: now(), seen: seen.clone(), reader, paths, firmware: None, sleep: SleepState::default(), details: None, last_off: None })
    }
    /// путь на читалке для вида файлов (от корня)
    pub fn reader_path(&self, kind: &str) -> String {
        self.paths.reader.get(kind).filter(|s| !s.trim().is_empty()).cloned().unwrap_or_else(|| match kind {
            "books" => "Books".into(), "comics" => "Comics".into(), "manga" => "Manga".into(), "wallpapers" => "Wallpapers".into(), "other" => "Download".into(), _ => String::new() })
    }
}

/// Папка Readers: <id>.json и <id>.png (фото с читалки)
pub struct Store { pub dir: PathBuf }

impl Store {
    pub fn list(&self) -> Vec<Device> {
        let mut v: Vec<Device> = std::fs::read_dir(&self.dir).map(|rd| rd.flatten().map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
            .filter_map(|p| std::fs::read_to_string(p).ok()).filter_map(|t| serde_json::from_str(&t).ok()).collect()).unwrap_or_default();
        // старые умолчания путей — один раз на новые, с сохранением
        for d in v.iter_mut() { if d.paths.migrate() { let _ = self.save(d); } }
        v.sort_by_key(|d| d.remembered);
        v
    }
    pub fn get(&self, id: &str) -> Option<Device> {
        let mut d: Device = std::fs::read_to_string(self.dir.join(format!("{id}.json"))).ok().and_then(|t| serde_json::from_str(&t).ok())?;
        if d.paths.migrate() { let _ = self.save(&d); }
        Some(d)
    }
    pub fn save(&self, d: &Device) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let p = self.dir.join(format!("{}.json", d.id));
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(d).unwrap())?;
        std::fs::rename(tmp, p)
    }
    pub fn photo_path(&self, id: &str) -> PathBuf { self.dir.join(format!("{id}.png")) }
    pub fn forget(&self, id: &str) -> std::io::Result<()> {
        for ext in ["json", "png"] { let p = self.dir.join(format!("{id}.{ext}")); if p.exists() { std::fs::remove_file(p)? } }
        Ok(())
    }
    pub fn forget_all(&self) -> std::io::Result<()> { if self.dir.exists() { std::fs::remove_dir_all(&self.dir)? } Ok(()) }
    /// свободное имя для новой читалки: PB743K3, потом PB743K3-2, -3 …
    pub fn new_id(&self, code: &str) -> String {
        let base = format!("PB{}", code.to_uppercase().replace(|c: char| !c.is_ascii_alphanumeric(), ""));
        if !self.dir.join(format!("{base}.json")).exists() { return base }
        (2..).map(|n| format!("{base}-{n}")).find(|id| !self.dir.join(format!("{id}.json")).exists()).unwrap()
    }
    /// какая сохранённая читалка подключена — по файлу-метке на ней
    pub fn match_seen(&self, s: &Seen) -> Option<Device> {
        if let Some(m) = &s.mark { return self.get(m) }
        None
    }
}

/// Записать на читалку файл-метку (при сохранении): по нему отличаются две одинаковые модели
pub fn write_mark(root: &Path, d: &Device) -> std::io::Result<()> {
    let p = root.join(MARK);
    if let Some(dir) = p.parent() { std::fs::create_dir_all(dir)? }
    std::fs::write(p, serde_json::to_string_pretty(&serde_json::json!({ "id": d.id, "name": d.name, "by": "InkMaiden" })).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fake(code: &str, label: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("pbdev-{}-{code}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(r.join("system/config/logos_customizer")).unwrap();
        std::fs::write(r.join("system/.pocketbook"), "").unwrap();
        std::fs::write(r.join(format!("system/device{}.ico", code.to_lowercase())), "x").unwrap();
        std::fs::write(r.join("system/device743c.ico"), "x").unwrap();
        std::fs::write(r.join("system/last_update"), "1795101188\0\0").unwrap();
        std::fs::write(r.join("system/config/global.cfg"), "language=ru\ntimezone=Bishkek\nnight_mode=0\noffloga=x\noffloge=@user_defined\nofflogo=@user_defined\n").unwrap();
        std::fs::write(r.join("system/config/logos_customizer/logos.json"), r#"{"logos":{"SleepLogo":{"random":[{},{}]},"OffLogo":{"custom":{},"random":[{}]}}}"#).unwrap();
        let _ = label; r
    }
    #[test]
    fn scan_remember_match() {
        let models = Models::load(None);
        let root = fake("743K3", "PB743K3");
        assert!(is_pocketbook(&root));
        let s = scan_root(&root, "PB743K3", &models);
        assert_eq!(s.code, "743K3");
        assert_eq!(s.model.as_deref(), Some("PocketBook InkPad Color 3"));
        assert!(s.photo.as_ref().unwrap().ends_with("device743k3.ico"));
        assert_eq!((s.language.as_deref(), s.last_update, s.night_mode), (Some("ru"), Some(1795101188), Some(false)));
        assert_eq!(s.logos["SleepLogo"]["random"], 2);
        assert_eq!(s.logos["OffLogo"]["custom"], true);
        // метка тома переименована — код берётся из значка
        let s2 = scan_root(&root, "МОЯ КНИГА", &models);
        assert_eq!(s2.code, "743K3");
        // запомнить и узнать снова
        let store = Store { dir: root.join("store") };
        let id = store.new_id(&s.code);
        assert_eq!(id, "PB743K3");
        let d = Device::from_seen(&s, &models, &id, None).unwrap();
        store.save(&d).unwrap();
        write_mark(&root, &d).unwrap();
        let again = scan_root(&root, "PB743K3", &models);
        assert_eq!(store.match_seen(&again).unwrap().id, d.id);
        assert_eq!(kernel_from_rev("0409").as_deref(), Some("4.9"));
        assert_eq!(store.new_id("743K3"), "PB743K3-2");
        assert_eq!(d.reader_path("books"), "Books");
        store.forget(&d.id).unwrap();
        assert!(store.list().is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
