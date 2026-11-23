//! План: какие архивы есть в папке и как они будут называться.

use crate::archive::{kind_of, Kind, ARCHIVE_EXTS};
use crate::names::{clean_names, format_template, split_parts, Parts};
use pb_core::util::sort_key;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
pub struct Item {
    pub path: PathBuf,
    /// папка относительно корня ('' — сам корень)
    pub rel: String,
    pub file_name: String,
    pub kind: Kind,
    pub size: u64,
    /// исправленное название без расширения
    pub base: String,
    pub parts: Parts,
    /// номер по порядку в своей папке
    pub n: String,
    pub folder: String,
    pub new_name: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Plan { pub root: PathBuf, pub items: Vec<Item> }

fn walk(dir: &Path, recurse: bool, skip: &[PathBuf], out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if skip.iter().any(|s| p.starts_with(s)) { continue }
        if p.is_dir() { if recurse { walk(&p, true, skip, out) } }
        else if p.extension().map(|x| ARCHIVE_EXTS.contains(&x.to_string_lossy().to_lowercase().as_str())).unwrap_or(false) { out.push(p) }
    }
}

impl Plan {
    /// Найти архивы. `skip` — папки, которые не трогать (бэкапы, результат).
    pub fn scan(root: &Path, recurse: bool, rename: bool, skip: &[PathBuf]) -> Plan {
        let mut files = vec![]; walk(root, recurse, skip, &mut files);
        let mut groups: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        for f in files { groups.entry(f.parent().unwrap().to_path_buf()).or_default().push(f) }
        let mut dirs: Vec<_> = groups.keys().cloned().collect();
        dirs.sort_by_key(|d| sort_key(&d.to_string_lossy()));
        let mut items = vec![];
        for d in dirs {
            let mut list = groups.remove(&d).unwrap();
            list.sort_by_key(|p| sort_key(&p.file_name().unwrap().to_string_lossy()));
            let bases: Vec<String> = list.iter().map(|p| p.file_stem().unwrap().to_string_lossy().to_string()).collect();
            let clean = if rename { clean_names(&bases) } else { bases.clone() };
            let rel = d.strip_prefix(root).map(|r| r.to_string_lossy().to_string()).unwrap_or_default();
            let nw = list.len().to_string().len().max(2);
            for (k, p) in list.iter().enumerate() {
                let base = if clean[k].is_empty() { bases[k].clone() } else { clean[k].clone() };
                let mut parts = split_parts(&base);
                for v in [&mut parts.vol, &mut parts.ch] { let t = v.trim_start_matches('0'); if v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) { *v = if t.is_empty() || !t.starts_with(|c: char| c.is_ascii_digit()) { format!("0{t}") } else { t.to_string() } } }
                items.push(Item {
                    path: p.clone(), rel: rel.clone(), file_name: p.file_name().unwrap().to_string_lossy().to_string(),
                    kind: kind_of(p), size: p.metadata().map(|m| m.len()).unwrap_or(0), base, parts,
                    n: format!("{:0>w$}", k + 1, w = nw), folder: d.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                    new_name: p.file_name().unwrap().to_string_lossy().to_string(),
                });
            }
        }
        Plan { root: root.to_path_buf(), items }
    }

    /// Итоговые имена по шаблону; повторы в одной папке получают « (2)»
    pub fn set_names(&mut self, template: &str, rename: bool, to_cbz: bool) {
        self.set_names_by(template, &std::collections::HashMap::new(), rename, to_cbz)
    }

    /// Названия по шаблонам: у подпапки может быть свой шаблон (`by_dir`: папка от корня → шаблон, действует и на вложенные;
    /// берётся самая глубокая подходящая), остальным — общий `template`.
    pub fn set_names_by(&mut self, template: &str, by_dir: &std::collections::HashMap<String, String>, rename: bool, to_cbz: bool) {
        let mut used = HashSet::new();
        for it in self.items.iter_mut() {
            let t = template_for(&it.rel, template, by_dir);
            let base = base_name(it, t, rename);
            let ext = if to_cbz { "cbz".to_string() } else { it.path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default() };
            let mut name = format!("{base}.{ext}"); let mut j = 2;
            while !used.insert(format!("{}|{}", it.rel, name.to_lowercase())) { name = format!("{base} ({j}).{ext}"); j += 1 }
            it.new_name = name;
        }
    }

    /// Пример для окна шаблонов: исходное имя тома (лучше с номером главы) и что из него сделает каждый шаблон.
    /// Состояние плана не меняется.
    pub fn name_samples(&self, templates: &[String]) -> Option<(String, Vec<String>)> { self.name_samples_in(templates, "") }

    /// Пример по тому из папки `dir` (и её вложенных); пустая — из всех
    pub fn name_samples_in(&self, templates: &[String], dir: &str) -> Option<(String, Vec<String>)> {
        let norm = |x: &str| x.replace('\\', "/").trim_matches('/').to_string();
        let d = norm(dir);
        let inside: Vec<&Item> = self.items.iter().filter(|i| { let r = norm(&i.rel); d.is_empty() || r == d || r.starts_with(&format!("{d}/")) }).collect();
        let it = *inside.iter().find(|i| !i.parts.ch.is_empty() && !i.parts.series.is_empty())
            .or_else(|| inside.iter().find(|i| !i.parts.ch.is_empty())).or_else(|| inside.first())?;
        let orig = it.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        Some((orig, templates.iter().map(|t| format!("{}.cbz", base_name(it, t, true))).collect()))
    }

    pub fn total_size(&self) -> u64 { self.items.iter().map(|i| i.size).sum() }
}

/// имя тома без расширения по шаблону (пустой шаблон — исправленное исходное название)
fn base_name(it: &Item, template: &str, rename: bool) -> String {
    let mut base = it.base.clone();
    if rename && !template.is_empty() {
        let mut v: HashMap<&str, String> = HashMap::new();
        v.insert("series", it.parts.series.clone()); v.insert("vol", it.parts.vol.clone()); v.insert("ch", it.parts.ch.clone());
        v.insert("title", it.parts.title.clone()); v.insert("n", it.n.clone()); v.insert("folder", it.folder.clone());
        v.insert("name", it.base.clone()); v.insert("orig", it.path.file_stem().unwrap().to_string_lossy().to_string());
        let t = format_template(template, &v); if !t.is_empty() { base = t }
    } else if !rename { base = it.path.file_stem().unwrap().to_string_lossy().to_string() }
    base.chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).collect()
}

/// Шаблон для папки `rel`: самый глубокий из `by_dir`, чья папка — она сама или её родитель; иначе общий
pub fn template_for<'a>(rel: &str, general: &'a str, by_dir: &'a std::collections::HashMap<String, String>) -> &'a str {
    let norm = |x: &str| x.replace('\\', "/").trim_matches('/').to_string();
    let r = norm(rel);
    by_dir.iter().filter(|(k, _)| { let k = norm(k); !k.is_empty() && (r == k || r.starts_with(&format!("{k}/"))) })
        .max_by_key(|(k, _)| k.len()).map(|(_, v)| v.as_str()).unwrap_or(general)
}

#[cfg(test)]
mod by_dir_tests {
    #[test]
    fn deepest_folder_template_wins() {
        let mut m = std::collections::HashMap::new();
        m.insert("Манга".to_string(), "{series} {ch:3}".to_string());
        m.insert("Манга\\Берсерк".to_string(), "{title}".to_string());
        assert_eq!(super::template_for("Манга/Берсерк/Том 1", "", &m), "{title}");
        assert_eq!(super::template_for("Манга/Другое", "", &m), "{series} {ch:3}");
        assert_eq!(super::template_for("Комиксы", "общий", &m), "общий");
        assert_eq!(super::template_for("МангаX", "общий", &m), "общий");
    }
}
