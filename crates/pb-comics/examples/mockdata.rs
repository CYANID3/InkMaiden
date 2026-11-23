//! Данные для проверки интерфейса в обычном браузере (dev/mock.js): настоящие оценки и предпросмотры
//! на проверочной библиотеке. cargo run --release -p pb-comics --example mockdata -- /tmp/polka-demo dev/mock-data.json
use base64::Engine;
use pb_comics::{archive, plan::Plan, process};
use pb_core::Readers;
use pb_image::{encode, ops, page::{convert, mode_opt, modes}, sim, Pix};
use serde_json::{json, Map, Value};
use std::path::Path;

fn url(p: &Pix, max_h: u32) -> String {
    let p = if p.h > max_h { ops::resize(p, (p.w as u64 * max_h as u64 / p.h as u64) as u32, max_h) } else { p.clone() };
    format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(encode::jpeg(&p, 88).unwrap()))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let root = Path::new(&a[1]);
    let readers = Readers::samples();
    let models = pb_core::models::Models::load(None);
    let mut plan = Plan::scan(root, true, true, &[]);
    plan.set_names("", true, true);
    let items: Vec<Value> = plan.items.iter().map(|i| json!({ "rel": i.rel, "name": i.file_name, "newName": i.new_name, "kind": i.kind.label(), "size": i.size })).collect();
    let mut ratios = Map::new();      // reader/mode -> [ratio по томам]
    let mut previews = Map::new();    // reader/mode/page -> предпросмотр первого тома
    let first = archive::read(&plan.items[0].path, None).unwrap();
    let pages: Vec<Vec<u8>> = first.pages.into_iter().map(|(_, b)| b).collect();
    let kind = process::comic_kind(&pages.iter().map(|b| b.as_slice()).collect::<Vec<_>>());
    for r in &readers.list {
        for m in ["safe", "balanced", "max"] {
            let opt = mode_opt(r, m);
            let rs: Vec<f64> = plan.items.iter().map(|i| process::estimate_ratio(&i.path, &opt, 4).unwrap_or(1.0)).collect();
            ratios.insert(format!("{}/{}", r.id, m), json!(rs));
            for n in 0..4 {
                let src = &pages[n];
                let res = convert(src, &opt, &kind, true).unwrap();
                let orig = res.orig.as_ref().unwrap();
                let out = res.result.as_ref().unwrap_or(orig);
                let sims: Map<String, Value> = readers.list.iter().map(|rd| (rd.id.clone(), Value::String(url(&sim::simulate(out, rd), 1100)))).collect();
                previews.insert(format!("{}/{}/{}", r.id, m, n), json!({
                    "page": n, "pages": pages.len(), "kind": kind,
                    "orig": { "url": url(orig, 1100), "w": orig.w, "h": orig.h, "format": pb_image::sniff(src), "bytes": src.len() },
                    "result": { "kept": res.bytes.is_none(), "bytes": res.bytes.as_ref().map(|b| b.len()).unwrap_or(src.len()),
                        "ext": if res.bytes.is_some() { res.ext } else { pb_image::sniff(src) }, "levels": res.levels, "gray": res.gray, "w": res.w, "h": res.h, "why": res.why },
                    "sims": sims,
                }));
            }
            eprintln!("{} {} {:?}", r.id, m, ratios[&format!("{}/{}", r.id, m)]);
        }
    }
    let opts: Map<String, Value> = readers.list.iter().flat_map(|r| ["safe", "balanced", "max"].map(|m| (format!("{}/{}", r.id, m), serde_json::to_value(mode_opt(r, m)).unwrap()))).collect();
    let v = json!({
        "info": { "version": "0.1.0", "readers": readers.list, "readerErrors": [], "readersDir": "C:\\Программы\\Полка\\readers",
            "modes": modes(), "templates": pb_comics::names::templates(), "settings": {}, "settingsPath": "C:\\Users\\me\\AppData\\Roaming\\kg.inkmaiden.app\\settings.json",
            "dataDir": "C:\\Users\\me\\AppData\\Roaming\\kg.inkmaiden.app", "portable": false,
            "defaults": { "wallpapers": "C:\\Users\\me\\Pictures\\InkMaiden\\Обои", "sleep": "C:\\Users\\me\\Pictures\\InkMaiden\\Иконки сна", "readerWallpapers": "system/logo/offlogo" },
            "models": models.list.iter().map(|m| json!({ "code": m.code, "name": m.name, "year": m.year, "display": m.display, "inches": m.inches,
                "width": m.width, "height": m.height, "color": m.color, "ppi": m.ppi(), "colorPpi": m.color_ppi, "body": m.body_mm })).collect::<Vec<_>>(),
            "modelsSource": models.source, "modelErrors": [] },
        "modelReaders": models.list.iter().map(|m| { let mut r = m.to_reader(&format!("model:{}", m.code), &m.name); r.source = "model".into(); (format!("model:{}", m.code), serde_json::to_value(r).unwrap()) }).collect::<Map<String, Value>>(),
        "root": root.display().to_string().replace('/', "\\"), "total": plan.total_size(), "items": items, "ratios": ratios, "previews": previews, "opts": opts,
    });
    std::fs::write(&a[2], serde_json::to_vec(&v).unwrap()).unwrap();
}
