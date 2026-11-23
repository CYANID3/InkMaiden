//! Настройки: один JSON-файл (Config.json рядом с программой), у каждого раздела свой ключ ("comics", "books", "paths", …).

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct Settings {
    path: PathBuf,
    data: Mutex<Map<String, Value>>,
}

fn read(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path).ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

impl Settings {
    pub fn open(path: PathBuf) -> Settings { Settings { data: Mutex::new(read(&path)), path } }
    pub fn get(&self, key: &str) -> Value { self.data.lock().unwrap().get(key).cloned().unwrap_or(Value::Null) }
    pub fn all(&self) -> Value { Value::Object(self.data.lock().unwrap().clone()) }
    pub fn set(&self, key: &str, v: Value) -> std::io::Result<()> {
        let mut d = self.data.lock().unwrap();
        d.insert(key.to_string(), v);
        self.write(&d)
    }
    fn write(&self, d: &Map<String, Value>) -> std::io::Result<()> {
        let path = self.path();
        if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&Value::Object(d.clone())).unwrap())?;
        std::fs::rename(tmp, &path)
    }
    pub fn path(&self) -> PathBuf { self.path.clone() }
    /// Сбросить всё: файл удаляется, программа работает с настройками по умолчанию.
    pub fn reset(&self) -> std::io::Result<()> {
        self.data.lock().unwrap().clear();
        let p = self.path();
        if p.exists() { std::fs::remove_file(p)?; }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_reset() {
        let dir = std::env::temp_dir().join(format!("pbset-{}", std::process::id()));
        let a = dir.join("a/Config.json");
        let s = Settings::open(a.clone());
        s.set("theme", serde_json::json!("dark")).unwrap();
        assert!(a.exists());
        assert_eq!(Settings::open(a.clone()).get("theme"), "dark");
        s.reset().unwrap();
        assert!(!a.exists() && s.get("theme").is_null());
        let _ = std::fs::remove_dir_all(dir);
    }
}
