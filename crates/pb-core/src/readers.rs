//! Профиль читалки — всё, что нужно для обработки страниц и предпросмотра: экран, корпус, имитация e-ink, умолчания комиксов.
//! Профили создаются из справочника моделей (models.rs) при «Запомнить устройство» (devices.rs).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub inches: f64,
    #[serde(default)]
    pub color: bool,
    #[serde(default = "d16")]
    pub gray_levels: u32,
    #[serde(default)]
    pub color_width: u32,
    #[serde(default)]
    pub color_height: u32,
}
fn d16() -> u32 { 16 }

/// Корпус — для рисунка читалки в предпросмотре
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Body {
    pub width_mm: f64,
    pub height_mm: f64,
    pub screen_width_mm: f64,
    pub screen_height_mm: f64,
    #[serde(default)]
    pub screen_top_mm: f64,
    #[serde(default = "dbody")]
    pub color: String,
}
fn dbody() -> String { "#2E2E2D".into() }

/// Как экран показывает картинку: цвет бумаги и чернил (0–255), тёплый оттенок, приглушение цвета (0–100)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Simulation {
    pub paper: i32,
    pub ink: i32,
    #[serde(default)]
    pub warm: i32,
    #[serde(default)]
    pub chroma: i32,
}

/// Что лучше всего для комиксов на этой читалке
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComicsDefaults {
    pub max_height: u32,
    /// "all" — всё серым, "auto" — ч/б страницы серым, "off" — цвет как есть
    pub gray: String,
    #[serde(default)]
    pub keep_color_pages: bool,
    #[serde(default)]
    pub color_fix: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reader {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub order: i32,
    pub screen: Screen,
    pub body: Body,
    pub simulation: Simulation,
    pub comics: ComicsDefaults,
    /// откуда профиль: "device" (запомненная читалка) или "model" (из справочника, не проверено)
    #[serde(default)]
    pub source: String,
}

/// Список профилей, с которыми работает программа: запомненные читалки (и модель из справочника, выбранная без подключения).
#[derive(Clone, Debug, Default, Serialize)]
pub struct Readers { pub list: Vec<Reader> }

impl Readers {
    pub fn get(&self, id: &str) -> Option<&Reader> { self.list.iter().find(|r| r.id == id) }
    /// Две читалки для проверок и демо-данных: Verse и InkPad Color 3 из справочника.
    pub fn samples() -> Readers {
        let m = crate::models::Models::load(None);
        Readers { list: vec![m.get("629").unwrap().to_reader("verse", "Verse"), m.get("743K3").unwrap().to_reader("color3", "InkPad Color 3")] }
    }
}
