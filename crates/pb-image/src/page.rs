//! Настройки страниц, готовые режимы и решение по одной странице.

use crate::{encode, ops, Pix};
use pb_core::Reader;
use serde::{Deserialize, Serialize};

/// Все настройки страниц. Обычно получаются из «читалка × режим», но любую можно поменять вручную.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PageOpt {
    pub reader: String,
    /// "safe" | "balanced" | "max" | "custom"
    pub mode: String,
    /// 0 — размер как в исходнике
    pub max_height: u32,
    pub quality: u8,
    /// "off" | "auto" (ч/б страницы серым) | "all"
    pub gray: String,
    /// в ч/б томе цветные вклейки не трогать
    pub keep_color_pages: bool,
    /// ч/б: "auto" (JPEG или PNG 16), "auto4" (+ PNG 4), "png16", "png4", "jpeg"
    pub bw_format: String,
    pub crop: bool,
    /// чистка фона 0–3
    pub clean: u8,
    /// резкость 0–3
    pub sharpen: u8,
    pub color_fix: bool,
    pub sat: i32,
    pub con: i32,
    pub bright: i32,
    pub shadows: i32,
    /// пережимать страницы, которые и так подходят по размеру
    pub squeeze: bool,
    /// …если выйдет легче хотя бы на столько процентов
    pub min_gain: u8,
}

impl Default for PageOpt {
    fn default() -> Self {
        PageOpt { reader: "color3".into(), mode: "balanced".into(), max_height: 1872, quality: 82, gray: "auto".into(), keep_color_pages: true,
            bw_format: "auto".into(), crop: true, clean: 1, sharpen: 1, color_fix: true, sat: 35, con: 10, bright: 4, shadows: 40, squeeze: true, min_gain: 10 }
    }
}

/// Описание режима для окна: название, кому подходит, плюсы и минусы
#[derive(Clone, Debug, Serialize)]
pub struct ModeInfo { pub id: &'static str, pub name: &'static str, pub note: &'static str, pub plus: Vec<&'static str>, pub minus: Vec<&'static str> }

pub fn modes() -> Vec<ModeInfo> {
    vec![
        ModeInfo { id: "safe", name: "Бережно", note: "Почти как исходник",
            plus: vec!["Скан не меняется: поля и бумага как были", "Никаких видимых потерь даже при увеличении"],
            minus: vec!["Места освобождается меньше всего"] },
        ModeInfo { id: "balanced", name: "Сбалансированно", note: "Подходит почти всем",
            plus: vec!["На экране читалки неотличимо от исходника", "Поля обрезаны — рисунок крупнее, бумага белее"],
            minus: vec!["На компьютере при сильном увеличении видны следы сжатия"] },
        ModeInfo { id: "max", name: "Максимум места", note: "Для большой библиотеки",
            plus: vec!["Меньше всего весит — в разы легче исходника"],
            minus: vec!["Серые заливки становятся ступеньками: 4 оттенка вместо 16", "На цветных страницах заметнее следы сжатия"] },
    ]
}

/// Настройки для «читалка × режим». Всё, что зависит от модели, — из её профиля.
pub fn mode_opt(r: &Reader, mode: &str) -> PageOpt {
    let c = &r.comics;
    let mut o = PageOpt { reader: r.id.clone(), mode: mode.into(), max_height: c.max_height, gray: c.gray.clone(), keep_color_pages: c.keep_color_pages,
        color_fix: c.color_fix && r.screen.color, ..PageOpt::default() };
    match mode {
        "safe" => { o.quality = 92; o.bw_format = "auto".into(); o.crop = false; o.clean = 0; o.sharpen = 0; o.min_gain = 10; o.sat = 20; o.con = 5; o.bright = 3; o.shadows = 25 }
        "max" => { o.quality = 68; o.bw_format = "auto4".into(); o.crop = true; o.clean = 2; o.sharpen = 1; o.min_gain = 5; o.keep_color_pages = false }
        _ => { o.mode = "balanced".into(); o.quality = 82; o.bw_format = "auto".into(); o.crop = true; o.clean = 1; o.sharpen = 1; o.min_gain = 10 }
    }
    o
}

const CLEAN: [(i32, i32, f32); 4] = [(0, 255, 1.0), (8, 245, 1.0), (16, 232, 1.0), (28, 218, 0.92)];
const SHARP: [i32; 4] = [0, 30, 60, 100];

/// Результат обработки страницы
pub struct PageResult {
    /// None — оставить исходный файл
    pub bytes: Option<Vec<u8>>,
    pub ext: &'static str,
    pub gray: bool,
    pub levels: u32,
    pub w: u32,
    pub h: u32,
    /// почему страница осталась как есть / почему потяжелела
    pub why: String,
    /// для предпросмотра: исходник и то, что получится
    pub orig: Option<Pix>,
    pub result: Option<Pix>,
}

/// Читалка откроет этот формат как есть?
pub fn reader_can_open(ext: &str) -> bool { matches!(ext, "jpg" | "png" | "gif" | "bmp" | "tiff") }

/// Обработать страницу. `kind` — "bw" / "color" / "" (весь том ч/б или цветной).
pub fn convert(bytes: &[u8], opt: &PageOpt, kind: &str, preview: bool) -> Result<PageResult, String> {
    let fmt = crate::sniff(bytes);
    let must = !reader_can_open(fmt);
    let any = must || opt.max_height > 0 || opt.squeeze || opt.crop || opt.clean > 0 || opt.sharpen > 0 || opt.color_fix || opt.gray == "all";
    let mut res = PageResult { bytes: None, ext: "", gray: false, levels: 0, w: 0, h: 0, why: String::new(), orig: None, result: None };
    if !any && !preview { return Ok(res) }
    let pix = crate::decode(bytes)?;
    res.w = pix.w; res.h = pix.h;
    let gray_page = ops::is_gray(&pix);
    let keep = |mut r: PageResult, why: &str, pix: Pix| { r.why = why.into(); if preview { r.result = Some(pix.clone()); r.orig = Some(pix) } r };
    if !gray_page && kind == "bw" && opt.keep_color_pages && !must { return Ok(keep(res, "цветная вклейка в ч/б томе — остаётся как есть", pix)) }
    if !any { return Ok(keep(res, "настройки ничего не меняют", pix)) }

    let mut p = pix.clone(); let mut visual = false; let mut resized = false;
    if opt.crop { if let Some((x, y, w, h)) = ops::find_crop(&p, 24) { p = ops::crop(&p, x, y, w, h); visual = true } }
    if opt.gray == "all" || (opt.gray == "auto" && gray_page) { p = ops::to_gray(&p) }
    if opt.max_height > 0 && p.h > opt.max_height && p.h >= p.w {
        p = ops::resize(&p, ((p.w as f64 * opt.max_height as f64 / p.h as f64).round() as u32).max(1), opt.max_height); resized = true;
    }
    if opt.clean > 0 && p.c == 1 { let (b, w, g) = CLEAN[opt.clean.min(3) as usize]; ops::levels(&mut p, b, w, g); visual = true }
    if opt.color_fix && p.c == 3 { ops::color_fix(&mut p, opt.sat, opt.con, opt.bright, opt.shadows); visual = true }
    if opt.sharpen > 0 { ops::sharpen(&mut p, SHARP[opt.sharpen.min(3) as usize]); visual = true }

    // кодирование: для ч/б при «что легче» пробуем несколько форматов и берём самый лёгкий
    let f = if p.c == 1 { opt.bw_format.as_str() } else { "jpeg" };
    let (mut out, mut ext) = match f {
        "png16" => (encode::png_gray(&p, 4)?, "png"),
        "png4" => (encode::png_gray(&p, 2)?, "png"),
        _ => (encode::jpeg(&p, opt.quality)?, "jpg"),
    };
    if f == "auto" || f == "auto4" {
        let bits: &[u8] = if f == "auto4" { &[4, 2] } else { &[4] };
        for &b in bits { let c = encode::png_gray(&p, b)?; if c.len() < out.len() { out = c; ext = "png" } }
    }
    res.levels = if ext == "png" { encode::png_levels(&out) } else { 0 };
    res.gray = p.c == 1; res.w = p.w; res.h = p.h; res.ext = ext;
    if must && out.len() > bytes.len() {
        res.why = format!("{} читалка в комиксах не открывает — перевод обязателен, а JPEG/PNG тяжелее этого формата", fmt.to_uppercase());
    }
    if !must && !visual {
        let lim = if resized { 1.0 } else { 1.0 - opt.min_gain as f64 / 100.0 };
        if out.len() as f64 >= lim * bytes.len() as f64 {
            return Ok(keep(res, "новый файл не легче — остаётся исходный", pix));
        }
    }
    if preview { res.orig = Some(pix); res.result = Some(crate::decode(&out)?) }
    res.bytes = Some(out);
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manga_page() -> Pix {
        // ч/б страница: рамки панелей, штрихи, скринтон, желтоватая бумага, поля
        let (w, h) = (900u32, 1350u32);
        let mut p = Pix::new(w, h, 3);
        for i in 0..(w * h) as usize { p.d[i * 3] = 244; p.d[i * 3 + 1] = 238; p.d[i * 3 + 2] = 220 }
        let mut set = |x: u32, y: u32, v: u8| { let i = ((y * w + x) * 3) as usize; p.d[i] = v; p.d[i + 1] = v; p.d[i + 2] = v };
        for y in 60..1290 { for x in [60u32, 61, 62, 839, 840, 841] { set(x, y, 10) } }
        for x in 60..842 { for y in [60u32, 61, 62, 700, 701, 1288, 1289] { set(x, y, 10) } }
        for y in (120..640).step_by(6) { for x in (100..500).step_by(6) { set(x, y, 40); set(x + 1, y, 40) } }
        for k in 0..400u32 { let x = 100 + (k * 37) % 700; let y = 760 + (k * 53) % 500; for d in 0..20 { set((x + d).min(839), (y + d / 2).min(1287), 15) } }
        p
    }
    fn reader(color: bool) -> Reader {
        let r = pb_core::Readers::samples();
        r.list[if color { 1 } else { 0 }].clone()
    }
    #[test]
    fn modes_shrink_bw_page() {
        let src = encode::jpeg(&manga_page(), 95).unwrap();
        let mut sizes = vec![];
        for m in ["safe", "balanced", "max"] {
            let o = mode_opt(&reader(true), m);
            let r = convert(&src, &o, "bw", false).unwrap();
            let b = r.bytes.expect("должна пережаться");
            assert!(b.len() < src.len(), "{m}: {} >= {}", b.len(), src.len());
            sizes.push(b.len());
        }
        assert!(sizes[2] <= sizes[1], "максимум места должен быть легче: {sizes:?}");
        // verse: высота 1024 и всё серым
        let r = convert(&src, &mode_opt(&reader(false), "balanced"), "bw", false).unwrap();
        assert!(r.h <= 1024 && r.gray);
    }
    #[test]
    fn jpeg_is_baseline_and_png_bits() {
        let p = manga_page();
        let j = encode::jpeg(&p, 80).unwrap();
        // SOF0 (FFC0) — обычный последовательный JPEG, не SOF2 (прогрессивный)
        assert!(j.windows(2).any(|w| w == [0xFF, 0xC0]) && !j.windows(2).any(|w| w == [0xFF, 0xC2]));
        let p4 = encode::png_gray(&p, 2).unwrap(); assert_eq!(encode::png_levels(&p4), 4);
        let dec = crate::decode(&p4).unwrap(); assert_eq!((dec.w, dec.c), (900, 1));
    }
    #[test]
    fn color_insert_kept_in_bw_volume() {
        let mut p = manga_page();
        for i in 0..(p.w * p.h / 3) as usize { p.d[i * 3] = 200; p.d[i * 3 + 1] = 40; p.d[i * 3 + 2] = 60 }
        let src = encode::jpeg(&p, 90).unwrap();
        let r = convert(&src, &mode_opt(&reader(true), "balanced"), "bw", false).unwrap();
        assert!(r.bytes.is_none() && r.why.contains("цветная"));
    }
}
