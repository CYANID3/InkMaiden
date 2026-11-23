//! Рабочий стол читалки: view.json (группы и приложения) и apps.json (панель главного экрана).
//! План правок накладывается на файл, который лежит на читалке в момент записи; чужие поля не трогаются.
use serde::Deserialize;
use serde_json::{json, Value};

/// Заводской набор групп (как в /ebrmain/config/desktop/view.json прошивки 6.x) — когда пользовательской копии нет
pub fn default_view() -> Value {
    json!({
        "version": "8",
        "applications": {},
        "view": { "groups": [
            { "title": "@General", "sort": "title", "apps": ["PB_AudioBooks", "PB_Browser", "PB_Bookstore", "PB_Calendar", "PB_Calc", "PB_Dictionary",
                "PB_Digital_Frame", "PB_Gallery", "PB_Library", "PB_Notes", "PB_Player", "PB_RSSNews", "PB_Overdrive", "PB_Settings"] },
            { "title": "@Services", "sort": "title", "apps": ["PB_Dropbox", "PB_Cloud", "PB_SendToPB"] },
            { "title": "@Games", "sort": "title", "apps": ["PB_Chess", "PB_Painter", "PB_Klondike", "PB_Scribble", "PB_Sudoku", "PB_Snake"] }
        ] }
    })
}

/// встроенные, которые нельзя скрыть
pub const KEEP: [&str; 2] = ["PB_Library", "PB_Settings"];

#[derive(Deserialize, Clone, Debug, Default)]
pub struct AppOp {
    pub key: String,
    /// своё приложение: путь на читалке (/mnt/ext1/applications/X.app)
    #[serde(default)] pub path: Option<String>,
    #[serde(default)] pub title: Option<String>,
    /// значок и значок в фокусе — пути от корня читалки (applications/icons/x.bmp)
    #[serde(default)] pub icon: Option<String>,
    #[serde(default)] pub focused: Option<String>,
    /// Some(true) — скрыть, Some(false) — показать, None — не менять
    #[serde(default)] pub hidden: Option<bool>,
    /// заголовок группы (@General, @Services, @Games или свой)
    #[serde(default)] pub group: Option<String>,
    /// убрать с рабочего стола совсем (свои приложения)
    #[serde(default)] pub remove: bool,
    /// удалить своё приложение с читалки: файл .app и его значки (копия — на компьютер), запись — тоже
    #[serde(default)] pub uninstall: bool,
}
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Plan {
    #[serde(default)] pub apps: Vec<AppOp>,
    /// панель главного экрана: ключи по порядку (None — не менять)
    #[serde(default)] pub panel: Option<Vec<String>>,
}

/// Файлы своего приложения на читалке (пути от корня): сам .app и его значки
pub fn app_files(view: &Value, key: &str, path: Option<&str>) -> Vec<String> {
    let e = view.get("applications").and_then(|a| a.get(key));
    let rel = |p: &str| p.trim_start_matches("/mnt/ext1/").trim_start_matches('/').to_string();
    let mut out = vec![];
    if let Some(p) = path.or_else(|| e.and_then(|e| e.get("path")).and_then(|p| p.as_str())) { if p.starts_with("/mnt/ext1/applications/") { out.push(rel(p)) } }
    for k in ["icon", "focused_icon"] {
        if let Some(p) = e.and_then(|e| e.get(k)).and_then(|i| i.get("path")).and_then(|p| p.as_str()) {
            let r = rel(p); if r.starts_with("applications/") && !out.contains(&r) { out.push(r) }
        }
    }
    out
}

/// наложить план на view.json
pub fn apply_plan(view: &mut Value, plan: &Plan) {
    if !view.is_object() { *view = default_view() }
    let o = view.as_object_mut().unwrap();
    if !o.get("applications").map(|a| a.is_object()).unwrap_or(false) { o.insert("applications".into(), json!({})); }
    if !o.get("view").and_then(|v| v.get("groups")).map(|g| g.is_array()).unwrap_or(false) {
        o.insert("view".into(), default_view()["view"].clone());
    }
    for op in &plan.apps {
        let k = op.key.as_str();
        if k.is_empty() || k.starts_with('_') { continue }
        // группы
        let groups = view["view"]["groups"].as_array_mut().unwrap();
        let remove = op.remove || op.uninstall;
        if remove || op.group.is_some() {
            for g in groups.iter_mut() { if let Some(a) = g.get_mut("apps").and_then(|a| a.as_array_mut()) { a.retain(|x| x.as_str() != Some(k)) } }
        }
        if let (false, Some(gt)) = (remove, &op.group) {
            let i = match groups.iter().position(|g| g.get("title").and_then(|t| t.as_str()) == Some(gt.as_str())) {
                Some(i) => i,
                None => { groups.push(json!({ "title": gt, "sort": "title", "apps": [] })); groups.len() - 1 }
            };
            let g = &mut groups[i];
            if !g.get("apps").map(|a| a.is_array()).unwrap_or(false) { g["apps"] = json!([]) }
            g["apps"].as_array_mut().unwrap().push(json!(k));
        }
        let apps = view["applications"].as_object_mut().unwrap();
        if remove { apps.remove(k); continue }
        let e = apps.entry(k.to_string()).or_insert_with(|| json!({}));
        if !e.is_object() { *e = json!({}) }
        let e = e.as_object_mut().unwrap();
        if let Some(p) = &op.path { e.insert("path".into(), json!(p)); }
        if let Some(t) = &op.title { if t.trim().is_empty() { e.remove("title"); } else { e.insert("title".into(), json!(t.trim())); } }
        if let Some(i) = &op.icon { e.insert("icon".into(), json!({ "path": i })); }
        if let Some(f) = &op.focused { e.insert("focused_icon".into(), json!({ "path": f })); }
        // Библиотеку и Настройки не скрываем никогда: без них читалкой не пользоваться
        let hidden = if KEEP.contains(&op.key.as_str()) && op.hidden == Some(true) { None } else { op.hidden };
        match hidden { Some(true) => { e.insert("visible".into(), json!("0")); } Some(false) => { e.remove("visible"); } None => {} }
        // у стоковых пустая запись — то же, что её нет
        if e.is_empty() && !k.starts_with("U_") { apps.remove(k); }
    }
}
pub fn apply_panel(panel: Option<Value>, keys: &[String]) -> Value {
    let mut p = panel.filter(|p| p.is_object()).unwrap_or_else(|| json!({ "version": 1 }));
    p["applications"] = Value::Array(keys.iter().map(|k| json!({ "id": k })).collect());
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plan_hides_adds_moves() {
        let mut v = default_view();
        apply_plan(&mut v, &Plan { apps: vec![
            AppOp { key: "PB_Cloud".into(), hidden: Some(true), ..Default::default() },
            AppOp { key: "U_Tablo".into(), path: Some("/mnt/ext1/applications/Tablo.app".into()), title: Some("Табло".into()),
                icon: Some("applications/icons/tablo.bmp".into()), focused: Some("applications/icons/tablo_f.bmp".into()), group: Some("@General".into()), ..Default::default() },
            AppOp { key: "PB_Chess".into(), group: Some("@General".into()), ..Default::default() },
        ], panel: None });
        assert_eq!(v["applications"]["PB_Cloud"]["visible"], "0");
        assert_eq!(v["applications"]["U_Tablo"]["path"], "/mnt/ext1/applications/Tablo.app");
        let gen: Vec<&str> = v["view"]["groups"][0]["apps"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        assert!(gen.contains(&"U_Tablo") && gen.contains(&"PB_Chess"));
        assert!(!v["view"]["groups"][2]["apps"].as_array().unwrap().iter().any(|x| x == "PB_Chess"));
        // показать обратно — запись у стокового исчезает целиком
        apply_plan(&mut v, &Plan { apps: vec![AppOp { key: "PB_Cloud".into(), hidden: Some(false), ..Default::default() },
            AppOp { key: "U_Tablo".into(), remove: true, ..Default::default() }], panel: None });
        assert!(v["applications"].get("PB_Cloud").is_none());
        assert!(v["applications"].get("U_Tablo").is_none());
        assert!(!v["view"]["groups"][0]["apps"].as_array().unwrap().iter().any(|x| x == "U_Tablo"));
    }
    #[test]
    fn uninstall_lists_files() {
        let v = json!({ "applications": { "U_Tablo": { "path": "/mnt/ext1/applications/Tablo.app", "icon": { "path": "applications/icons/tablo.bmp" }, "focused_icon": { "path": "applications/icons/tablo_f.bmp" } } },
            "view": { "groups": [{ "title": "@General", "apps": ["U_Tablo"] }] } });
        assert_eq!(app_files(&v, "U_Tablo", None), vec!["applications/Tablo.app", "applications/icons/tablo.bmp", "applications/icons/tablo_f.bmp"]);
        // системные пути не трогаем
        assert!(app_files(&json!({ "applications": { "U_X": { "path": "/ebrmain/bin/x.app" } } }), "U_X", None).is_empty());
        let mut v2 = v.clone();
        apply_plan(&mut v2, &Plan { apps: vec![AppOp { key: "U_Tablo".into(), uninstall: true, ..Default::default() }], panel: None });
        assert!(v2["applications"].get("U_Tablo").is_none());
        assert!(v2["view"]["groups"][0]["apps"].as_array().unwrap().is_empty());
    }
    #[test]
    fn plan_keeps_unknown_fields() {
        let mut v = json!({ "version": "8", "applications": { "_comment": ["x"], "U_A": { "path": "/a", "custom": 1 } }, "view": { "_comment": ["y"], "groups": [] } });
        apply_plan(&mut v, &Plan { apps: vec![AppOp { key: "U_A".into(), title: Some("А".into()), ..Default::default() }], panel: None });
        assert_eq!(v["applications"]["U_A"]["custom"], 1);
        assert_eq!(v["applications"]["_comment"][0], "x");
        assert_eq!(v["view"]["_comment"][0], "y");
    }
    #[test]
    fn panel_keeps_partner() {
        let p = apply_panel(Some(json!({ "applications": [{ "id": "PB_Library" }], "partner": "bookland", "version": 1 })), &["PB_Library".into(), "U_Tablo".into(), "SYS_Apps".into()]);
        assert_eq!(p["partner"], "bookland");
        assert_eq!(p["applications"][1]["id"], "U_Tablo");
    }
    #[test]
    fn keep_never_hidden() {
        let mut v = default_view();
        apply_plan(&mut v, &Plan { apps: vec![AppOp { key: "PB_Library".into(), hidden: Some(true), ..Default::default() },
            AppOp { key: "PB_Settings".into(), hidden: Some(true), ..Default::default() }], panel: None });
        for k in KEEP { assert!(v["applications"].get(k).and_then(|e| e.get("visible")).is_none(), "{k}"); }
    }

}
