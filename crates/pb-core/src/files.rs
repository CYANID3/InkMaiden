//! Файлы: безопасные имена, запись с проверкой, пути на читалке, установка с резервной копией, сравнение папок для загрузки.

use std::path::{Component, Path, PathBuf};

/// Имя файла или папки без запрещённых в Windows знаков
pub fn clean_name(n: &str) -> String {
    n.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) || c.is_control() { '_' } else { c }).collect::<String>().trim().trim_end_matches('.').to_string()
}

/// Записать файл через временный и проверить размер: на читалке место кончается тихо
pub fn write_checked(p: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() { std::fs::create_dir_all(d).map_err(|e| format!("Не создать папку {}: {e}", d.display()))? }
    let tmp = p.with_extension("inkmaiden-tmp");
    std::fs::write(&tmp, data).map_err(|e| format!("Не записать {}: {e}", p.display()))?;
    if std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0) != data.len() as u64 { let _ = std::fs::remove_file(&tmp); return Err("Файл записался не полностью — проверьте свободное место.".into()) }
    std::fs::rename(&tmp, p).map_err(|e| format!("Не записать {}: {e}", p.display()))
}

/// Путь на читалке от её корня: без «..», дисков и начального слэша
pub fn reader_rel(p: &str) -> Result<PathBuf, String> {
    if p.contains(':') { return Err("Неверный путь на читалке.".into()) }
    let p = PathBuf::from(p.replace('\\', "/").trim_start_matches('/'));
    if p.as_os_str().is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) { return Err("Неверный путь на читалке.".into()) }
    Ok(p)
}

/// Положить файлы в папку; прежние с теми же именами — сначала в old/<stamp>. Возвращает журнал.
/// `backup` — папка резервных копий (на компьютере): прежний файл уходит в <backup>/<stamp>/<имя>; None — без копии.
pub fn install(dir: &Path, files: &[(String, Vec<u8>)], backup: Option<&Path>, stamp: &str) -> Result<Vec<String>, String> {
    let mut log = vec![];
    for (name, data) in files {
        let p = dir.join(name);
        if let (Some(bk), true) = (backup, p.exists()) {
            let b = bk.join(stamp).join(name);
            if let Some(d) = b.parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())? }
            std::fs::copy(&p, &b).map_err(|e| format!("Не удалось сохранить прежний файл: {e}"))?;
            log.push(format!("прежний {name} → копия на компьютере ({stamp})"));
        }
        write_checked(&p, data)?;
        log.push(format!("записан {name}"));
    }
    Ok(log)
}

/// Перенести всё из `from` в `to` (с вложенными; одноимённое в `to` не перезаписывается — тогда файл просто убирается из
/// `from`, если он такой же) и удалить `from`. Возвращает число перенесённых файлов.
pub fn move_tree(from: &Path, to: &Path) -> std::io::Result<usize> {
    if !from.exists() { return Ok(0) }
    let mut n = 0;
    for e in std::fs::read_dir(from)?.flatten() {
        let (s, d) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() { n += move_tree(&s, &d)?; continue }
        std::fs::create_dir_all(to)?;
        if !d.exists() { std::fs::copy(&s, &d)?; n += 1 }
        if std::fs::metadata(&d).map(|m| m.len()).ok() == std::fs::metadata(&s).map(|m| m.len()).ok() { std::fs::remove_file(&s)?; }
    }
    let _ = std::fs::remove_dir(from);
    Ok(n)
}

/// Все файлы в папке (с вложенными): путь от неё и размер. Скрытое и служебное InkMaiden пропускается.
pub fn walk(dir: &Path) -> Vec<(String, u64)> {
    fn go(dir: &Path, base: &Path, out: &mut Vec<(String, u64)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with('.') || n.ends_with(".inkmaiden") || n.ends_with(".inkmaiden-tmp") { continue }
            if p.is_dir() { go(&p, base, out) } else if let (Ok(m), Ok(r)) = (e.metadata(), p.strip_prefix(base)) { out.push((r.to_string_lossy().to_string(), m.len())) }
        }
    }
    let mut out = vec![];
    go(dir, dir, &mut out);
    out.sort();
    out
}

/// Скопировать файл через временный и проверить размер (большие файлы не читаются в память)
pub fn copy_checked(src: &Path, dst: &Path) -> Result<u64, String> {
    if let Some(d) = dst.parent() { std::fs::create_dir_all(d).map_err(|e| format!("Не создать папку {}: {e}", d.display()))? }
    let tmp = dst.with_extension("inkmaiden-tmp");
    let n = std::fs::copy(src, &tmp).map_err(|e| { let _ = std::fs::remove_file(&tmp); format!("Не скопировать {}: {e}", src.display()) })?;
    let want = std::fs::metadata(src).map(|m| m.len()).unwrap_or(n);
    if std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0) != want { let _ = std::fs::remove_file(&tmp); return Err("Файл записался не полностью — проверьте свободное место.".into()) }
    if dst.exists() { std::fs::remove_file(dst).map_err(|e| format!("Не заменить {}: {e}", dst.display()))? }
    std::fs::rename(&tmp, dst).map_err(|e| format!("Не записать {}: {e}", dst.display()))?;
    Ok(n)
}

/// Те же ли файлы одинакового размера: сравниваются начало, середина и конец (по 64 КБ) — быстро даже через USB
pub fn same_content(a: &Path, b: &Path, size: u64) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    const CH: u64 = 64 * 1024;
    let (Ok(mut fa), Ok(mut fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else { return false };
    let marks = if size <= 3 * CH { vec![0] } else { vec![0, size / 2 - CH / 2, size - CH] };
    for at in marks {
        let n = CH.min(size - at) as usize;
        let (mut x, mut y) = (vec![0u8; n], vec![0u8; n]);
        if fa.seek(SeekFrom::Start(at)).is_err() || fb.seek(SeekFrom::Start(at)).is_err() { return false }
        if fa.read_exact(&mut x).is_err() || fb.read_exact(&mut y).is_err() || x != y { return false }
    }
    true
}

/// Перенести всё из папки `from` в `into` (с вложенными): файл с тем же путём в `into` не перезаписывается — он остаётся в `from`.
/// Опустевшие папки в `from` убираются. Возвращает (перенесено, оставлено из-за совпадения имён).
pub fn merge_dirs(from: &Path, into: &Path) -> std::io::Result<(usize, usize)> {
    fn walk(f: &Path, t: &Path, n: &mut (usize, usize)) -> std::io::Result<()> {
        if !f.is_dir() { return Ok(()) }
        std::fs::create_dir_all(t)?;
        for e in std::fs::read_dir(f)?.flatten() {
            let (src, dst) = (e.path(), t.join(e.file_name()));
            if e.file_type()?.is_dir() { walk(&src, &dst, n)?; let _ = std::fs::remove_dir(&src); continue }
            if dst.exists() { n.1 += 1; continue }
            if std::fs::rename(&src, &dst).is_err() { std::fs::copy(&src, &dst)?; std::fs::remove_file(&src)?; }
            n.0 += 1;
        }
        Ok(())
    }
    let mut n = (0, 0);
    walk(from, into, &mut n)?;
    let _ = std::fs::remove_dir(from);
    Ok(n)
}

/// Файл загрузки: откуда, куда на читалке (путь от корня), размер и что там сейчас
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Planned { pub item: usize, pub src: PathBuf, pub dst: String, pub size: u64, pub state: String }

/// Список загрузок → файлы. Пункт — файл (ляжет в папку dst) или папка (её содержимое ляжет в dst с подпапками).
/// state: new — на читалке нет, same — такой же размер, changed — есть, но другой, missing — исходника нет.
pub fn plan(items: &[(PathBuf, String)], root: &Path) -> Vec<Planned> {
    let mut out = vec![];
    for (i, (src, dir)) in items.iter().enumerate() {
        let dir = dir.replace('\\', "/").trim_matches('/').to_string();
        let join = |r: &str| if dir.is_empty() { r.replace('\\', "/") } else { format!("{dir}/{}", r.replace('\\', "/")) };
        let files: Vec<(PathBuf, String, u64)> = if src.is_dir() {
            walk(src).into_iter().map(|(r, size)| (src.join(&r), join(&r), size)).collect()
        } else if let Ok(m) = std::fs::metadata(src) {
            vec![(src.clone(), join(&src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()), m.len())]
        } else { // нет ни файла, ни папки: для файла — куда бы он лёг, для папки (ещё не создана) — сама папка на читалке
            let dst = if src.extension().is_some() { join(&src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()) } else { dir.clone() };
            out.push(Planned { item: i, src: src.clone(), dst, size: 0, state: "missing".into() }); continue };
        for (f, dst, size) in files {
            // одинаковый размер ещё не значит тот же файл (обои и иконки одного разрешения всегда одного размера) — сверяем куски
            let state = match std::fs::metadata(root.join(&dst)) { Ok(m) if m.len() == size && same_content(&f, &root.join(&dst), size) => "same", Ok(_) => "changed", Err(_) => "new" };
            out.push(Planned { item: i, src: f, dst, size, state: state.into() });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pbfiles-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn names_and_paths() {
        assert_eq!(clean_name(" a:b/c?. "), "a_b_c_");
        assert_eq!(clean_name("Кот. "), "Кот");
        assert_eq!(reader_rel("\\system\\logo\\offlogo").unwrap(), PathBuf::from("system/logo/offlogo"));
        assert!(reader_rel("../x").is_err());
        assert!(reader_rel("C:/x").is_err());
        assert!(reader_rel("").is_err());
    }

    #[test]
    fn install_keeps_backup() {
        let d = tmp("inst");
        let pc = d.join("pc");
        install(&d, &[("a.bmp".into(), b"old".to_vec())], Some(&pc), "s1").unwrap();
        assert!(!pc.exists(), "прежнего файла не было — копии нет");
        let log = install(&d, &[("a.bmp".into(), b"new".to_vec())], Some(&pc), "s2").unwrap();
        assert_eq!(std::fs::read(d.join("a.bmp")).unwrap(), b"new");
        assert_eq!(std::fs::read(pc.join("s2/a.bmp")).unwrap(), b"old");
        assert!(!d.join("old").exists(), "на читалке копий нет");
        assert_eq!(log.len(), 2);
        install(&d, &[("a.bmp".into(), b"newer".to_vec())], None, "s3").unwrap();
        assert!(!pc.join("s3").exists());
        // старые копии на читалке (old/<время>) переезжают на компьютер, папка old убирается
        write_checked(&d.join("old/s0/a.bmp"), b"older").unwrap();
        assert_eq!(move_tree(&d.join("old"), &pc).unwrap(), 1);
        assert!(!d.join("old").exists());
        assert_eq!(std::fs::read(pc.join("s0/a.bmp")).unwrap(), b"older");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn merge_keeps_both() {
        let d = tmp("merge");
        write_checked(&d.join("a/Books/x.fb2"), b"new").unwrap(); write_checked(&d.join("a/Books/same.fb2"), b"from-a").unwrap();
        write_checked(&d.join("b/Books/same.fb2"), b"from-b").unwrap();
        assert_eq!(merge_dirs(&d.join("a"), &d.join("b")).unwrap(), (1, 1));
        assert_eq!(std::fs::read(d.join("b/Books/x.fb2")).unwrap(), b"new");
        assert_eq!(std::fs::read(d.join("b/Books/same.fb2")).unwrap(), b"from-b", "совпадающее не перезаписывается");
        assert_eq!(std::fs::read(d.join("a/Books/same.fb2")).unwrap(), b"from-a", "и не теряется");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn same_size_different_content() {
        let d = tmp("same"); std::fs::create_dir_all(d.join("pc")).unwrap(); std::fs::create_dir_all(d.join("rd/W")).unwrap();
        std::fs::write(d.join("pc/a.bmp"), vec![1u8; 300_000]).unwrap();
        let mut other = vec![1u8; 300_000]; other[150_000] = 9;
        std::fs::write(d.join("rd/W/a.bmp"), &other).unwrap();
        let p = plan(&[(d.join("pc/a.bmp"), "W".into())], &d.join("rd"));
        assert_eq!(p[0].state, "changed");
        std::fs::write(d.join("rd/W/a.bmp"), vec![1u8; 300_000]).unwrap();
        assert_eq!(plan(&[(d.join("pc/a.bmp"), "W".into())], &d.join("rd"))[0].state, "same");
        let m = plan(&[(d.join("pc/нет"), "W".into())], &d.join("rd"));
        assert_eq!((m[0].state.as_str(), m[0].dst.as_str()), ("missing", "W"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn upload_plan_and_copy() {
        let (s, r) = (tmp("qsrc"), tmp("qroot"));
        write_checked(&s.join("Books/a.fb2"), b"aaaa").unwrap();
        write_checked(&s.join("Books/sub/b.fb2"), b"bb").unwrap();
        write_checked(&s.join("one.cbz"), b"ccc").unwrap();
        write_checked(&r.join("Books/a.fb2"), b"aaaa").unwrap();
        write_checked(&r.join("Manga/one.cbz"), b"c").unwrap();
        let p = plan(&[(s.join("Books"), "Books".into()), (s.join("one.cbz"), "\\Manga\\".into()), (s.join("nope.fb2"), "Books".into())], &r);
        let st = |d: &str| p.iter().find(|x| x.dst == d).map(|x| x.state.clone()).unwrap();
        assert_eq!(st("Books/a.fb2"), "same"); assert_eq!(st("Books/sub/b.fb2"), "new"); assert_eq!(st("Manga/one.cbz"), "changed"); assert_eq!(st("Books/nope.fb2"), "missing");
        assert_eq!(copy_checked(&s.join("one.cbz"), &r.join("Manga/one.cbz")).unwrap(), 3);
        assert_eq!(std::fs::read(r.join("Manga/one.cbz")).unwrap(), b"ccc");
        assert!(!r.join("Manga/one.inkmaiden-tmp").exists());
        let _ = (std::fs::remove_dir_all(s), std::fs::remove_dir_all(r));
    }
}
