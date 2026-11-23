//! Операции над пикселями (перенесены из pb-tools, C#-ядро PbImg).

use crate::Pix;

#[inline] fn clamp(v: f32) -> u8 { if v <= 0.0 { 0 } else if v >= 255.0 { 255 } else { (v + 0.5) as u8 } }

/// Веса усреднения площади: честное уменьшение без муара на скринтонах
fn weights(src: u32, dst: u32) -> Vec<(usize, Vec<f32>)> {
    let s = src as f64 / dst as f64;
    (0..dst).map(|i| {
        let a = i as f64 * s; let b = (i as f64 + 1.0) * s;
        let i0 = a.floor() as usize; let i1 = (b.ceil() as usize).min(src as usize);
        let mut w: Vec<f32> = (i0..i1).map(|k| (b.min(k as f64 + 1.0) - a.max(k as f64)).max(0.0) as f32).collect();
        let sum: f32 = w.iter().sum(); for x in w.iter_mut() { *x /= sum }
        (i0, w)
    }).collect()
}

pub fn resize(p: &Pix, nw: u32, nh: u32) -> Pix {
    let (nw, nh) = (nw.max(1), nh.max(1));
    if nw >= p.w && nh >= p.h { return resize_up(p, nw, nh) }
    let c = p.c as usize; let (w, h) = (p.w as usize, p.h as usize);
    let xw = weights(p.w, nw); let yw = weights(p.h, nh);
    let mut tmp = vec![0f32; nw as usize * h * c];
    for y in 0..h {
        let so = y * w * c; let to = y * nw as usize * c;
        for (x, (x0, ws)) in xw.iter().enumerate() {
            for ch in 0..c {
                let mut acc = 0f32;
                for (k, wt) in ws.iter().enumerate() { acc += p.d[so + (x0 + k) * c + ch] as f32 * wt }
                tmp[to + x * c + ch] = acc;
            }
        }
    }
    let mut o = Pix::new(nw, nh, p.c);
    let nwu = nw as usize;
    for (y, (y0, ws)) in yw.iter().enumerate() {
        for x in 0..nwu { for ch in 0..c {
            let mut acc = 0f32;
            for (k, wt) in ws.iter().enumerate() { acc += tmp[((y0 + k) * nwu + x) * c + ch] * wt }
            o.d[(y * nwu + x) * c + ch] = clamp(acc);
        } }
    }
    o
}

fn resize_up(p: &Pix, nw: u32, nh: u32) -> Pix {
    let c = p.c as usize; let mut o = Pix::new(nw, nh, p.c);
    let (w, h) = (p.w as usize, p.h as usize);
    for y in 0..nh as usize {
        let fy = ((y as f32 + 0.5) * h as f32 / nh as f32 - 0.5).max(0.0); let y0 = (fy as usize).min(h - 1); let y1 = (y0 + 1).min(h - 1); let ty = fy - y0 as f32;
        for x in 0..nw as usize {
            let fx = ((x as f32 + 0.5) * w as f32 / nw as f32 - 0.5).max(0.0); let x0 = (fx as usize).min(w - 1); let x1 = (x0 + 1).min(w - 1); let tx = fx - x0 as f32;
            for ch in 0..c {
                let g = |yy: usize, xx: usize| p.d[(yy * w + xx) * c + ch] as f32;
                let a = g(y0, x0) * (1.0 - tx) + g(y0, x1) * tx; let b = g(y1, x0) * (1.0 - tx) + g(y1, x1) * tx;
                o.d[(y * nw as usize + x) * c + ch] = clamp(a * (1.0 - ty) + b * ty);
            }
        }
    }
    o
}

/// Доля «цветных» точек по уменьшенной копии (шум JPEG и желтоватая бумага цветом не считаются)
pub fn color_share(p: &Pix, lim: i32) -> f64 {
    if p.c == 1 { return 0.0 }
    let s; let q = if p.w > 200 { s = resize(p, 200, ((p.h as u64 * 200 / p.w as u64) as u32).max(1)); &s } else { p };
    let mut col = 0usize;
    for px in q.d.chunks_exact(3) {
        let (r, g, b) = (px[0] as i32, px[1] as i32, px[2] as i32);
        if r.max(g).max(b) - r.min(g).min(b) > lim { col += 1 }
    }
    col as f64 / q.len_px().max(1) as f64
}
pub fn is_gray(p: &Pix) -> bool { color_share(p, 36) <= 0.004 }

pub fn to_gray(p: &Pix) -> Pix {
    if p.c == 1 { return p.clone() }
    let mut o = Pix::new(p.w, p.h, 1);
    for (i, px) in p.d.chunks_exact(3).enumerate() { o.d[i] = ((px[0] as u32 * 299 + px[1] as u32 * 587 + px[2] as u32 * 114 + 500) / 1000) as u8 }
    o
}
pub fn to_rgb(p: &Pix) -> Pix {
    if p.c == 3 { return p.clone() }
    let mut o = Pix::new(p.w, p.h, 3);
    for (i, v) in p.d.iter().enumerate() { o.d[i * 3] = *v; o.d[i * 3 + 1] = *v; o.d[i * 3 + 2] = *v }
    o
}

fn luma(p: &Pix, i: usize) -> i32 {
    if p.c == 1 { p.d[i] as i32 } else { (p.d[i * 3] as i32 * 299 + p.d[i * 3 + 1] as i32 * 587 + p.d[i * 3 + 2] as i32 * 114) / 1000 }
}

/// Пустые поля по краям (белые или чёрные). Возвращает (x, y, w, h) или None.
pub fn find_crop(p: &Pix, tol: i32) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = (p.w as usize, p.h as usize);
    let corners = [luma(p, 0), luma(p, w - 1), luma(p, (h - 1) * w), luma(p, h * w - 1)];
    let bg = if corners.iter().filter(|&&c| c > 128).count() >= 2 { 255 } else { 0 };
    let ink = |x: usize, y: usize| (luma(p, y * w + x) - bg).abs() > tol;
    let need_w = (w / 300).max(2); let need_h = (h / 300).max(2);
    let row = |y: usize| (0..w).filter(|&x| ink(x, y)).take(need_w).count() >= need_w;
    let col = |x: usize| (0..h).filter(|&y| ink(x, y)).take(need_h).count() >= need_h;
    let (mx, my) = (w / 4, h / 4);
    let top = (0..my).find(|&y| row(y)).unwrap_or(my);
    let bottom = (0..my).find(|&k| row(h - 1 - k)).unwrap_or(my);
    let left = (0..mx).find(|&x| col(x)).unwrap_or(mx);
    let right = (0..mx).find(|&k| col(w - 1 - k)).unwrap_or(mx);
    let pad = (w.min(h) / 100).max(4);
    let (top, bottom, left, right) = (top.saturating_sub(pad), bottom.saturating_sub(pad), left.saturating_sub(pad), right.saturating_sub(pad));
    let (nw, nh) = (w - left - right, h - top - bottom);
    if nw < w / 2 || nh < h / 2 { return None }
    if (nw * nh) as u64 > (w * h) as u64 * 97 / 100 { return None }
    Some((left as u32, top as u32, nw as u32, nh as u32))
}

pub fn crop(p: &Pix, x0: u32, y0: u32, w: u32, h: u32) -> Pix {
    let c = p.c as usize; let mut o = Pix::new(w, h, p.c);
    for y in 0..h as usize {
        let s = ((y0 as usize + y) * p.w as usize + x0 as usize) * c;
        o.d[y * w as usize * c..(y + 1) * w as usize * c].copy_from_slice(&p.d[s..s + w as usize * c]);
    }
    o
}

/// Уровни для ч/б: чистый белый фон, плотный чёрный
pub fn levels(p: &mut Pix, black: i32, white: i32, gamma: f32) {
    if white <= black + 10 { return }
    let lut: Vec<u8> = (0..256).map(|v| { let t = ((v - black) as f32 / (white - black) as f32).clamp(0.0, 1.0); clamp(t.powf(gamma) * 255.0) }).collect();
    for v in p.d.iter_mut() { *v = lut[*v as usize] }
}

/// Цветокор для цветных экранов: насыщенность, контраст, яркость (%), тени светлее (0–100)
pub fn color_fix(p: &mut Pix, sat: i32, contrast: i32, bright: i32, shadows: i32) {
    if p.c != 3 { return }
    let s = 1.0 + sat as f32 / 100.0; let k = 1.0 + contrast as f32 / 100.0; let b = bright as f32 * 2.55;
    let lut: Vec<u8> = (0..256).map(|v| { let t = v as f32 / 255.0; clamp((t + shadows as f32 / 100.0 * (1.0 - t).powi(3) * 0.35) * 255.0) }).collect();
    for px in p.d.chunks_exact_mut(3) {
        let (r, g, bl) = (px[0] as f32, px[1] as f32, px[2] as f32);
        let y = 0.299 * r + 0.587 * g + 0.114 * bl;
        for (i, v) in [r, g, bl].iter().enumerate() {
            let x = (y + (v - y) * s - 128.0) * k + 128.0 + b;
            px[i] = lut[clamp(x) as usize];
        }
    }
}

/// Резкость (нерезкая маска 3×3), amount в процентах
pub fn sharpen(p: &mut Pix, amount: i32) {
    if amount <= 0 || p.w < 3 || p.h < 3 { return }
    let a = amount as f32 / 100.0; let c = p.c as usize; let (w, h) = (p.w as usize, p.h as usize);
    let src = p.d.clone();
    for y in 1..h - 1 { for x in 1..w - 1 { for ch in 0..c {
        let o = (y * w + x) * c + ch;
        let mut sum = 0u32;
        for dy in [-1isize, 0, 1] { for dx in [-1isize, 0, 1] { sum += src[(o as isize + (dy * w as isize + dx) * c as isize) as usize] as u32 } }
        p.d[o] = clamp(src[o] as f32 + a * (src[o] as f32 - sum as f32 / 9.0));
    } } }
}

/// Вписать в экран (как просмотрщик читалки), поля — цвета бумаги
pub fn fit(p: &Pix, sw: u32, sh: u32, bg: u8) -> Pix {
    let k = (sw as f32 / p.w as f32).min(sh as f32 / p.h as f32);
    let (w, h) = (((p.w as f32 * k).round() as u32).max(1), ((p.h as f32 * k).round() as u32).max(1));
    let r = resize(p, w, h); let c = p.c as usize;
    let mut o = Pix { w: sw, h: sh, c: p.c, d: vec![bg; (sw * sh) as usize * c] };
    let (ox, oy) = (((sw - w) / 2) as usize, ((sh - h) / 2) as usize);
    for y in 0..h as usize {
        let d = ((oy + y) * sw as usize + ox) * c;
        o.d[d..d + w as usize * c].copy_from_slice(&r.d[y * w as usize * c..(y + 1) * w as usize * c]);
    }
    o
}
