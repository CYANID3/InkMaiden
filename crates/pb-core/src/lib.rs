//! Общее для всех разделов InkMaiden: справочник моделей, устройства, профили читалок, настройки, папки, фоновые задачи, мелочи.
//! Разделы (комиксы, книги, …) — отдельные крейты, которые используют только это ядро.

pub mod details;
pub mod desktop;
pub mod diag;
pub mod files;
pub mod folders;
pub mod devices;
pub mod jobs;
pub mod library;
pub mod models;
pub mod readers;
pub mod settings;
pub mod util;

pub use readers::{Reader, Readers};
