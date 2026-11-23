//! Обработка страниц: чтение, уменьшение, поля, уровни, цветокор, резкость, сжатие (mozjpeg, PNG с малым
//! числом оттенков + oxipng) и имитация экранов читалок по их профилям.

pub mod encode;
pub mod ops;
pub mod page;
pub mod sim;

/// Картинка в памяти: c = 1 (серый) или 3 (R, G, B)
#[derive(Clone)]
pub struct Pix { pub w: u32, pub h: u32, pub c: u32, pub d: Vec<u8> }

impl Pix {
    pub fn new(w: u32, h: u32, c: u32) -> Pix { Pix { w, h, c, d: vec![0; (w * h * c) as usize] } }
    pub fn len_px(&self) -> usize { (self.w * self.h) as usize }
}

/// Прочитать картинку любого понятного формата (JPEG, PNG, GIF, BMP, TIFF, WebP)
pub fn decode(bytes: &[u8]) -> Result<Pix, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("не удалось прочитать картинку: {e}"))?;
    Ok(match img.color() {
        image::ColorType::L8 | image::ColorType::La8 | image::ColorType::L16 | image::ColorType::La16 => {
            let g = img.to_luma8(); Pix { w: g.width(), h: g.height(), c: 1, d: g.into_raw() }
        }
        _ => { let g = img.to_rgb8(); Pix { w: g.width(), h: g.height(), c: 3, d: g.into_raw() } }
    })
}

/// Формат по первым байтам
pub fn sniff(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xFF, 0xD8, ..] => "jpg",
        [0x89, b'P', b'N', b'G', ..] => "png",
        [b'G', b'I', b'F', ..] => "gif",
        [b'B', b'M', ..] => "bmp",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "webp",
        [b'I', b'I', 42, 0, ..] | [b'M', b'M', 0, 42, ..] => "tiff",
        _ if bytes.len() > 12 && &bytes[4..8] == b"ftyp" => "avif",
        _ => "?",
    }
}
