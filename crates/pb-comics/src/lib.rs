//! Раздел «Комиксы»: CBZ/CBR/CB7/ZIP/RAR/7Z → правильный CBZ с понятными названиями и сжатыми страницами.

pub mod archive;
pub mod comicinfo;
pub mod names;
pub mod plan;
pub mod process;

pub use plan::{Item, Plan};
