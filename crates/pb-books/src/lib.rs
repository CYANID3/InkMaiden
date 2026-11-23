//! Книги: обложки, ссылки → текст, TXT → FB2. Перенос логики pb-tools (PowerShell) на Rust.
//! EPUB, FB2, FB2.ZIP, FB3 правятся «по месту»: меняются только нужные куски текста, остальное остаётся байт в байт.

pub mod container;
pub mod cover;
pub mod edit;
pub mod junk;
pub mod links;
pub mod meta;
pub mod scan;
pub mod text;
pub mod txt;
pub mod xml;

pub use meta::{Format, Meta};
pub use scan::{process, scan, scan_with, Book, Job, Scan};
