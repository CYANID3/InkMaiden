//! Справочник моделей PocketBook: то, чего не узнать с самой читалки (диагональ, корпус, тип экрана).
//! Встроенный файл data/models.json; models.json рядом с программой дополняет или заменяет записи по коду.

use crate::readers::{Body, ComicsDefaults, Reader, Screen, Simulation};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    /// код модели = метка тома без «PB»: 629, 743K3
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub year: u32,
    #[serde(default)]
    pub display: String,
    pub inches: f64,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub color: bool,
    #[serde(default)]
    pub color_ppi: Option<u32>,
    /// ширина, высота, толщина корпуса, мм
    pub body_mm: [f64; 3],
    #[serde(default)]
    pub ppi: u32,
    /// по таблице источника: процессор, ОЗУ, память, батарея, связь, звук, защита от воды (по USB этого не видно)
    #[serde(default)]
    pub cpu: Option<String>,
    #[serde(default)]
    pub ram_mb: Option<u32>,
    #[serde(default)]
    pub storage_gb: Option<u32>,
    #[serde(default)]
    pub battery_mah: Option<u32>,
    #[serde(default)]
    pub wifi: Option<String>,
    #[serde(default)]
    pub bluetooth: Option<bool>,
    #[serde(default)]
    pub audio: Option<bool>,
    #[serde(default)]
    pub water: Option<String>,
}

#[derive(Deserialize)]
struct File { models: Vec<Model>, #[serde(default)] source: String }

const BUILTIN: &str = include_str!("../data/models.json");

#[derive(Clone, Debug, Default, Serialize)]
pub struct Models { pub list: Vec<Model>, pub source: String, pub errors: Vec<String> }

/// «PB743K3», «pb743k3», «743K3» → «743K3»
pub fn norm_code(s: &str) -> String {
    let s = s.trim().to_uppercase();
    s.strip_prefix("PB").unwrap_or(&s).to_string()
}

impl Models {
    pub fn load(extra: Option<&Path>) -> Models {
        let f: File = serde_json::from_str(BUILTIN).expect("встроенный справочник");
        let mut m = Models { list: f.models, source: f.source, errors: vec![] };
        if let Some(p) = extra.filter(|p| p.exists()) {
            match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<File>(&t).map_err(|e| e.to_string())) {
                Ok(x) => for md in x.models {
                    match m.list.iter_mut().find(|o| norm_code(&o.code) == norm_code(&md.code)) { Some(o) => *o = md, None => m.list.push(md) }
                },
                Err(e) => m.errors.push(format!("{}: {e}", p.display())),
            }
        }
        m
    }
    pub fn get(&self, code: &str) -> Option<&Model> { let c = norm_code(code); self.list.iter().find(|m| norm_code(&m.code) == c) }
}

impl Model {
    pub fn full_name(&self) -> String { format!("PocketBook {}", self.name) }
    pub fn ppi(&self) -> u32 { if self.ppi > 0 { self.ppi } else { ((self.width as f64).hypot(self.height as f64) / self.inches).round() as u32 } }

    /// Профиль для обработки и предпросмотра. Размер экрана в мм — из диагонали и пропорций,
    /// место экрана на корпусе — примерно (сверху ~29 % свободного поля, как у Verse и Color 3).
    pub fn to_reader(&self, id: &str, name: &str) -> Reader {
        let diag = (self.width as f64).hypot(self.height as f64);
        let mm = self.inches * 25.4 / diag;
        let (sw, sh) = (self.width as f64 * mm, self.height as f64 * mm);
        let [bw, bh, _] = self.body_mm;
        let (cw, ch) = match self.color_ppi { Some(c) if self.color => {
            let k = c as f64 / self.ppi() as f64; ((self.width as f64 * k).round() as u32, (self.height as f64 * k).round() as u32) } _ => (0, 0) };
        let r1 = |x: f64| (x * 10.0).round() / 10.0;
        Reader {
            id: id.into(), name: name.into(), model: format!("{} (PB{})", self.full_name(), self.code), order: 0,
            screen: Screen { width: self.width, height: self.height, inches: self.inches, color: self.color, gray_levels: 16, color_width: cw, color_height: ch },
            body: Body { width_mm: bw, height_mm: bh, screen_width_mm: r1(sw), screen_height_mm: r1(sh), screen_top_mm: r1(((bh - sh) * 0.29).max(3.0)),
                color: if self.color { "#2B3036".into() } else { "#2E2E2D".into() } },
            simulation: if self.color { Simulation { paper: 205, ink: 40, warm: 8, chroma: 36 } } else { Simulation { paper: 229, ink: 22, warm: 6, chroma: 0 } },
            comics: if self.color { ComicsDefaults { max_height: self.height, gray: "auto".into(), keep_color_pages: true, color_fix: true } }
                    else { ComicsDefaults { max_height: self.height, gray: "all".into(), keep_color_pages: false, color_fix: false } },
            source: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_and_reader() {
        let m = Models::load(None);
        assert!(m.list.len() >= 25);
        let c3 = m.get("PB743K3").unwrap();
        assert_eq!(c3.name, "InkPad Color 3");
        let r = c3.to_reader("x", "Моя");
        assert_eq!((r.screen.color_width, r.screen.color_height), (702, 936));
        assert!((r.body.screen_width_mm - 119.0).abs() < 1.5 && (r.body.screen_height_mm - 158.5).abs() < 1.5, "{:?}", r.body);
        let v = m.get("629").unwrap().to_reader("v", "Verse");
        assert!(!v.screen.color && v.comics.gray == "all" && (v.body.screen_height_mm - 121.9).abs() < 1.0);
        assert!(m.get("743c").is_some() && m.get("743").is_none());
    }
}
