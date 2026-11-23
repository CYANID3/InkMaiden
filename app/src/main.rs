//! InkMaiden — порядок в библиотеке и на читалке PocketBook. Здесь только связка: состояние и команды для окна.
//! Логика — в крейтах: pb-books (книги), pb-comics (комиксы), pb-image (страницы), pb-core (читалки, настройки).
//!
//! Всё хранится рядом с программой (если туда можно писать, иначе в %APPDATA%\InkMaiden):
//!   Config.json — общие настройки; Readers\PB743K3.json (+ .png) — читалки; PB743K3\Books|Comics|Manga|Wallpapers|Sleep_icons.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod books;
mod backups;
mod comics;
mod desktop;
mod apps;
mod devices;
mod files;
mod usb;
mod webview;

use pb_core::devices::{Device, Store, KINDS};
use pb_core::models::Models;
use pb_core::{jobs::Jobs, settings::Settings, Reader};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

pub struct App {
    pub models: Models,
    pub config: Settings,
    /// папка данных: рядом с программой или %APPDATA%\InkMaiden
    pub root: PathBuf,
    pub jobs: Jobs,
    pub comics: Mutex<comics::ComicsState>,
    pub books: Mutex<books::BooksState>,
    pub usb: Mutex<devices::UsbState>,
}

impl App {
    pub fn store(&self) -> Store { Store { dir: self.root.join("Readers") } }
    /// Профиль сохранённой читалки (размер экрана, корпус, имитация e-ink)
    pub fn reader(&self, id: &str) -> Option<Reader> { self.store().get(id).map(|d| d.reader) }
    pub fn device(&self, id: &str) -> Result<Device, String> { self.store().get(id).ok_or_else(|| "Такой читалки нет — подключите её, она сохранится сама.".into()) }
    /// Папка на компьютере для готового: своя у читалки → общая из настроек (paths.out) → PB743K3\Books рядом с программой
    pub fn pc_dir(&self, d: &Device, kind: &str) -> PathBuf {
        let general = self.config.get("paths").get("out").and_then(|o| o.get(kind)).and_then(|v| v.as_str()).map(String::from);
        d.paths.pc.get(kind).cloned().filter(|s| !s.trim().is_empty()).or(general.filter(|s| !s.trim().is_empty())).map(PathBuf::from)
            .unwrap_or_else(|| self.root.join(&d.id).join(KINDS.iter().find(|k| k.0 == kind).map(|k| k.1).unwrap_or(kind)))
    }
    /// Корень подключённой читалки
    pub fn connected_root(&self, id: &str) -> Option<PathBuf> {
        self.usb.lock().unwrap().seen.iter().find(|s| s.mark.as_deref() == Some(id)).map(|s| PathBuf::from(&s.root))
    }
    pub fn behavior(&self, key: &str) -> bool { self.config.get("behavior").get(key).and_then(|v| v.as_bool()).unwrap_or(true) }
}

/// папка рядом с программой
pub fn exe_dir() -> PathBuf { std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| PathBuf::from(".")) }

fn writable(dir: &Path) -> bool {
    let t = dir.join(".inkmaiden-write-test");
    let ok = std::fs::write(&t, b"x").is_ok();
    let _ = std::fs::remove_file(t);
    ok
}

#[tauri::command]
fn app_info(app: tauri::State<App>) -> Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "models": app.models.list.iter().map(|m| json!({ "code": m.code, "name": m.name, "year": m.year, "display": m.display, "inches": m.inches,
            "width": m.width, "height": m.height, "color": m.color, "ppi": m.ppi(), "colorPpi": m.color_ppi, "body": m.body_mm })).collect::<Vec<_>>(),
        "modelsSource": app.models.source, "modelErrors": app.models.errors,
        "modes": pb_image::page::modes(),
        "templates": pb_comics::names::templates(),
        "config": app.config.all(),
        "root": app.root, "beside": app.root == exe_dir(),
    })
}

#[tauri::command]
fn config_set(app: tauri::State<App>, key: String, value: Value) -> Result<(), String> { app.config.set(&key, value).map_err(|e| e.to_string()) }

/// Сбросить общие настройки (Config.json). Читалки и готовые файлы остаются.
#[tauri::command]
fn config_reset(app: tauri::State<App>) -> Result<(), String> { app.config.reset().map_err(|e| e.to_string()) }

/// Удалить все файлы настроек: Config.json и папку Readers. Готовые файлы остаются.
#[tauri::command]
fn config_delete_all(app: tauri::State<App>) -> Result<(), String> {
    app.config.reset().map_err(|e| e.to_string())?;
    app.store().forget_all().map_err(|e| e.to_string())?;
    devices::rescan(&app);
    Ok(())
}

#[tauri::command]
fn job_cancel(app: tauri::State<App>, id: String) { app.jobs.cancel(&id) }

fn main() {
    webview::prepare();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|a| {
            let beside = exe_dir();
            let root = if writable(&beside) { beside } else {
                a.path().data_dir().map(|d| d.join("InkMaiden")).unwrap_or_else(|_| beside.join("InkMaiden-data"))
            };
            let _ = std::fs::create_dir_all(&root);
            let config = Settings::open(root.join("Config.json"));
            // старые настройки комиксов из «Полки» — один раз
            if config.get("comics").is_null() {
                if let Some(old) = a.path().config_dir().ok().map(|d| d.join("kg.polka.pocketbook").join("settings.json")).filter(|p| p.exists()) {
                    if let Some(c) = std::fs::read_to_string(old).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.get("comics").cloned()) {
                        let _ = config.set("comics", c);
                    }
                }
            }
            let est = comics::load_cache(&root);
            a.manage(App {
                models: Models::load(Some(&exe_dir().join("models.json"))),
                config, root,
                jobs: Jobs::default(),
                comics: Mutex::new(comics::ComicsState { est, ..Default::default() }),
                books: Mutex::new(books::BooksState::default()),
                usb: Mutex::new(devices::UsbState::default()),
            });
            devices::watch(a.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info, config_set, config_reset, config_delete_all, job_cancel,
            devices::devices_state, devices::device_update, devices::device_forget, devices::device_claim, devices::device_merge, devices::device_orphans, devices::device_rename, devices::device_diag_load, devices::devices_forget_all, devices::device_details, devices::diag_install, devices::diag_remove, devices::device_eject,
            files::read_image, books::books_cover_keep, books::books_file_info, books::books_file_parts, books::books_file_part, apps::apps_list, apps::apps_import, apps::apps_forget,
            files::works_save, files::works_list, files::works_load, files::works_delete,
            files::sleep_install, files::sleep_unpend, files::sleep_plan, files::sleep_backups, files::sleep_restore,
            files::queue_check, files::queue_run, files::dir_has_files, files::ready_list, files::ready_remove, backups::backups_list, backups::backups_restore_book, backups::backups_forget_book, files::copy_files, files::open_folder, files::list_images,
            books::books_scan, books::books_process, books::books_done,
            desktop::desktop_read, desktop::desktop_apply, desktop::desktop_backups, desktop::desktop_restore, desktop::desktop_icon_save,
            books::books_cover, books::books_parts, books::books_part, books::books_render_part, books::books_check_part,
            comics::comics_scan, comics::comics_mode_opt, comics::comics_names, comics::comics_use, comics::comics_done, comics::comics_name_samples, comics::comics_preview, comics::comics_mode_estimates,
            comics::comics_volume_estimate, comics::comics_estimate_all, comics::comics_run, comics::comics_compare,
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска InkMaiden");
}
