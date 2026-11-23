//! Куда класть готовое и бэкапы. Одинаково для всех разделов.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Folders {
    /// папка для готовых файлов (обычно папка читалки рядом с программой: PB743K3\Manga);
    /// внутри повторяются вложенные папки источника. Пусто — рядом с источником: «Манга» → «Манга (PocketBook)»
    #[serde(default)]
    pub out_dir: String,
    /// своя папка для оригиналов при изменении на месте; пусто — по умолчанию раздела
    #[serde(default)]
    pub backup_dir: String,
}

fn leaf(root: &Path) -> String { root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Папка".into()) }

impl Folders {
    pub fn out(&self, root: &Path) -> PathBuf {
        if self.out_dir.trim().is_empty() {
            let mut p = root.to_path_buf(); let name = format!("{} (PocketBook)", leaf(root)); p.set_file_name(name); p
        } else { PathBuf::from(self.out_dir.trim()) }
    }
    /// `default` — папка бэкапов раздела по умолчанию; `stamp` — время запуска
    pub fn backup(&self, root: &Path, default: PathBuf, stamp: &str) -> PathBuf {
        if self.backup_dir.trim().is_empty() { default } else { PathBuf::from(self.backup_dir.trim()).join(format!("{} {}", leaf(root), stamp)) }
    }
}
