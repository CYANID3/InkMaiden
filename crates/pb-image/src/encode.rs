//! Сжатие: JPEG через mozjpeg (обычный последовательный, без прогрессивной развёртки — читалки открывают его быстрее)
//! и PNG в оттенках серого с 2/4/8 битами на точку, дожатый oxipng.

use crate::Pix;

pub fn jpeg(p: &Pix, quality: u8) -> Result<Vec<u8>, String> {
    std::panic::catch_unwind(|| {
        let cs = if p.c == 1 { mozjpeg::ColorSpace::JCS_GRAYSCALE } else { mozjpeg::ColorSpace::JCS_RGB };
        let mut c = mozjpeg::Compress::new(cs);
        c.set_size(p.w as usize, p.h as usize);
        c.set_quality(quality as f32);
        c.set_optimize_coding(true);
        c.set_optimize_scans(false);     // без прогрессивной развёртки
        let mut s = c.start_compress(Vec::with_capacity(p.d.len() / 8)).expect("jpeg start");
        s.write_scanlines(&p.d).expect("jpeg write");
        s.finish().expect("jpeg finish")
    }).map_err(|_| "не удалось сжать JPEG".to_string())
}

/// Серый PNG: bits = 2 (4 оттенка), 4 (16 оттенков), 8
pub fn png_gray(p: &Pix, bits: u8) -> Result<Vec<u8>, String> {
    let g = crate::ops::to_gray(p);
    let (w, h) = (g.w as usize, g.h as usize);
    let bits = if [1, 2, 4].contains(&bits) { bits } else { 8 };
    let levels = (1u32 << bits) - 1;
    let row_bytes = (w * bits as usize).div_ceil(8);
    let mut raw = vec![0u8; row_bytes * h];
    for y in 0..h { for x in 0..w {
        let v = g.d[y * w + x] as u32;
        if bits == 8 { raw[y * row_bytes + x] = v as u8; continue }
        let q = (v * levels + 127) / 255;
        let bit = x * bits as usize; let sh = 8 - bits as usize - (bit & 7);
        raw[y * row_bytes + (bit >> 3)] |= (q << sh) as u8;
    } }
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, g.w, g.h);
        e.set_color(png::ColorType::Grayscale);
        e.set_depth(match bits { 1 => png::BitDepth::One, 2 => png::BitDepth::Two, 4 => png::BitDepth::Four, _ => png::BitDepth::Eight });
        e.set_compression(png::Compression::Fast);
        let mut wr = e.write_header().map_err(|e| e.to_string())?;
        wr.write_image_data(&raw).map_err(|e| e.to_string())?;
    }
    // дожать: oxipng подбирает фильтры и сжатие (без потерь)
    let mut o = oxipng::Options::from_preset(2);
    o.bit_depth_reduction = false; o.color_type_reduction = false; o.palette_reduction = false; o.grayscale_reduction = false;
    Ok(oxipng::optimize_from_memory(&out, &o).unwrap_or(out))
}

/// Число оттенков в нашем PNG (по заголовку): 4, 16 или 256
pub fn png_levels(bytes: &[u8]) -> u32 { if bytes.len() > 24 { 1 << bytes[24].min(8) } else { 0 } }
