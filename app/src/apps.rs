//! Сторонние приложения для читалки: папка Apps рядом с программой.
//! В ней лежат *.app; рядом по желанию — значок (<имя>.png/.jpg/.bmp) и описание <имя>.desk (title=…).
//! Установка и удаление на читалке идут через «На читалку» (копия файла + запись рабочего стола).

use crate::App;
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

fn dir(app: &App) -> PathBuf { app.root.join("Apps") }
/// папка «только для этой читалки»: Apps\PB<код модели> (как метка диска читалки, например PB743K3)
fn dev_dir(app: &App, code: &Option<String>) -> Option<PathBuf> {
    let c = code.as_deref()?.trim().trim_start_matches("PB").to_string();
    if c.is_empty() || !c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') { return None }
    Some(dir(app).join(format!("PB{c}")))
}
fn scan(d: &Path, only: bool, out: &mut Vec<Value>) {
    let Ok(rd) = std::fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if !p.is_file() || !name.to_lowercase().ends_with(".app") { continue }
        // своя версия для этой читалки важнее общей с тем же именем
        if let Some(i) = out.iter().position(|v| v["file"].as_str().map(|f| f.eq_ignore_ascii_case(&name)).unwrap_or(false)) {
            if only { out.remove(i); } else { continue }
        }
        let s = stem(&name).to_string();
        let icon = ["png", "jpg", "jpeg", "bmp", "webp"].iter().map(|x| d.join(format!("{s}.{x}"))).find(|q| q.is_file());
        let title = std::fs::read_to_string(d.join(format!("{s}.desk"))).ok().and_then(|t| desk_title(&t));
        let m = e.metadata().ok();
        out.push(json!({
            "file": name, "path": p.to_string_lossy(), "only": only, "size": m.as_ref().map(|m| m.len()).unwrap_or(0),
            "modified": m.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()),
            "title": title, "icon": icon.map(|q| q.to_string_lossy().to_string()),
        }));
    }
}

/// безопасное имя файла: только последняя часть пути, без управляющих символов
fn clean(name: &str) -> Option<String> {
    let n = name.rsplit(['/', '\\']).next()?.trim();
    if n.is_empty() || n.starts_with('.') || n.chars().any(|c| c.is_control() || "<>:\"|?*".contains(c)) { return None }
    Some(n.to_string())
}
fn stem(f: &str) -> &str { f.rsplit_once('.').map(|(s, _)| s).unwrap_or(f) }

fn desk_title(t: &str) -> Option<String> {
    t.lines().find_map(|l| l.trim().strip_prefix("title=").map(|v| v.trim().to_string())).filter(|v| !v.is_empty())
}

/// Приложения для читалки: общие (Apps) + только для неё (Apps\PB<код>). Обе папки создаются, чтобы их было видно.
#[tauri::command]
pub fn apps_list(app: tauri::State<App>, code: Option<String>, dir: Option<String>) -> Result<Value, String> {
    // своя папка приложений этой читалки (из «Путей») — вместо Apps и Apps\PB<код>
    if let Some(own) = dir.filter(|x| !x.trim().is_empty()).map(PathBuf::from) {
        std::fs::create_dir_all(&own).map_err(|e| format!("{}: {e}", own.display()))?;
        let mut out = vec![];
        scan(&own, true, &mut out);
        out.sort_by_key(|v| v["file"].as_str().unwrap_or("").to_lowercase());
        return Ok(json!({ "dir": own.to_string_lossy(), "devDir": null, "own": true, "apps": out }));
    }
    let d = self::dir(&app);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let dd = dev_dir(&app, &code);
    if let Some(x) = &dd { let _ = std::fs::create_dir_all(x); }
    let mut out = vec![];
    scan(&d, false, &mut out);
    if let Some(x) = &dd { scan(x, true, &mut out); }
    out.sort_by_key(|v| v["file"].as_str().unwrap_or("").to_lowercase());
    Ok(json!({ "dir": d.to_string_lossy(), "devDir": dd.map(|x| x.to_string_lossy().to_string()), "apps": out }))
}

/// Добавить в папку: файлы *.app, картинки-значки и zip-комплекты
/// (из zip берутся *.app, значки icons/<имя>.bmp и описания desk/<имя>.desk). Возвращает имена добавленных приложений.
#[tauri::command]
pub fn apps_import(app: tauri::State<App>, paths: Vec<String>, code: Option<String>, dir: Option<String>) -> Result<Vec<String>, String> {
    // dir — своя папка из «Путей»; code — только для этой читалки (Apps\\PB<код>); без них — для всех
    let d = match (dir.filter(|x| !x.trim().is_empty()), &code) {
        (Some(x), _) => PathBuf::from(x),
        (None, Some(_)) => dev_dir(&app, &code).ok_or("Неизвестная модель читалки.")?,
        (None, None) => self::dir(&app),
    };
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let mut added = vec![];
    for src in paths {
        let p = PathBuf::from(&src);
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let low = name.to_lowercase();
        if low.ends_with(".app") {
            std::fs::copy(&p, d.join(&name)).map_err(|e| format!("{name}: {e}"))?;
            added.push(name);
        } else if low.ends_with(".zip") {
            added.extend(from_zip(&p, &d).map_err(|e| format!("{name}: {e}"))?);
        } else if [".png", ".jpg", ".jpeg", ".bmp", ".webp", ".desk"].iter().any(|x| low.ends_with(x)) {
            std::fs::copy(&p, d.join(&name)).map_err(|e| format!("{name}: {e}"))?;
        } else {
            return Err(format!("{name}: это не приложение — нужен файл .app или zip-архив с ним."));
        }
    }
    Ok(added)
}

fn from_zip(p: &Path, d: &Path) -> Result<Vec<String>, String> {
    let f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| "архив не открылся".to_string())?;
    let mut entries: Vec<(String, Vec<u8>)> = vec![];
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        if e.is_dir() || e.size() > 64 << 20 { continue }
        let full = e.name().replace('\\', "/");
        let low = full.to_lowercase();
        let keep = low.ends_with(".app") || (low.contains("icons/") && low.ends_with(".bmp")) || low.ends_with(".desk");
        if !keep { continue }
        let mut b = vec![];
        e.read_to_end(&mut b).map_err(|e| e.to_string())?;
        entries.push((full, b));
    }
    let apps: Vec<String> = entries.iter().filter(|(n, _)| n.to_lowercase().ends_with(".app")).filter_map(|(n, _)| clean(n)).collect();
    if apps.is_empty() { return Err("в архиве нет ни одного файла .app".into()) }
    let mut added = vec![];
    for (n, b) in &entries {
        let Some(f) = clean(n) else { continue };
        let low = f.to_lowercase();
        if low.ends_with(".app") { std::fs::write(d.join(&f), b).map_err(|e| e.to_string())?; added.push(f); }
        else if low.ends_with(".desk") {
            // описание из комплекта: app=Имя.app → <Имя>.desk рядом с приложением
            let t = String::from_utf8_lossy(b);
            let target = t.lines().find_map(|l| l.trim().strip_prefix("app=").map(|v| v.trim().to_string())).unwrap_or_else(|| f.clone());
            std::fs::write(d.join(format!("{}.desk", stem(&target))), b).map_err(|e| e.to_string())?;
        } else if low.ends_with(".bmp") && !low.ends_with("_f.bmp") {
            // значок icons/<имя>.bmp подходит приложению с тем же именем (без учёта регистра)
            if let Some(a) = apps.iter().find(|a| stem(a).eq_ignore_ascii_case(stem(&f))) {
                let dst = d.join(format!("{}.bmp", stem(a)));
                if !dst.exists() { std::fs::write(dst, b).map_err(|e| e.to_string())?; }
            }
        }
    }
    // значок, названный не как приложение, — по описанию (icon=…)
    for (n, b) in &entries {
        if !n.to_lowercase().ends_with(".desk") { continue }
        let t = String::from_utf8_lossy(b);
        let get = |k: &str| t.lines().find_map(|l| l.trim().strip_prefix(k).map(|v| v.trim().to_string()));
        if let (Some(a), Some(ic)) = (get("app="), get("icon=")) {
            let dst = d.join(format!("{}.bmp", stem(&a)));
            if dst.exists() { continue }
            if let Some((_, b)) = entries.iter().find(|(m, _)| clean(m).map(|x| x.eq_ignore_ascii_case(&ic)).unwrap_or(false)) {
                std::fs::write(dst, b).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(added)
}

/// Убрать приложение из папки (в корзину) — вместе с его значком и описанием. Только внутри Apps.
#[tauri::command]
pub fn apps_forget(app: tauri::State<App>, path: String, dir: Option<String>) -> Result<usize, String> {
    let p = PathBuf::from(&path);
    let base = self::dir(&app);
    let own = dir.filter(|x| !x.trim().is_empty()).map(PathBuf::from);
    let d = p.parent().ok_or("Неверный путь.")?.to_path_buf();
    if !(d == base || d.parent() == Some(base.as_path()) || own.as_ref() == Some(&d)) { return Err("Файл не из папки приложений.".into()) }
    let f = p.file_name().map(|n| n.to_string_lossy().to_string()).ok_or("Неверный путь.")?;
    let s = stem(&f).to_string();
    let mut paths = vec![p.to_string_lossy().to_string()];
    for x in ["png", "jpg", "jpeg", "bmp", "webp", "desk"] { let q = d.join(format!("{s}.{x}")); if q.is_file() { paths.push(q.to_string_lossy().to_string()) } }
    crate::files::ready_remove(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(clean("applications/Bloknot.app").as_deref(), Some("Bloknot.app"));
        assert_eq!(clean("..\\x.app").as_deref(), Some("x.app"));
        assert_eq!(clean("a/"), None);
        assert_eq!(desk_title("key=U_X\ntitle=Блокнот\n").as_deref(), Some("Блокнот"));
    }
}
