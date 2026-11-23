//! Как страница будет выглядеть на экране читалки — по параметрам из профиля (simulation).

use crate::{ops, Pix};
use pb_core::Reader;

#[inline] fn cl(v: f32) -> u8 { v.round().clamp(0.0, 255.0) as u8 }

/// Результат — RGB размера экрана читалки
pub fn simulate(p: &Pix, r: &Reader) -> Pix {
    let s = &r.simulation; let lv = r.screen.gray_levels.max(2) as f32 - 1.0;
    let paper = [s.paper as f32 + s.warm as f32 * 0.25, s.paper as f32, (s.paper - s.warm) as f32];
    let ink = [s.ink as f32, s.ink as f32, (s.ink + if r.screen.color { 4 } else { 2 }) as f32];
    let q = |v: f32| (v.clamp(0.0, 255.0) / 255.0 * lv).round() / lv;
    if !r.screen.color {
        let g = ops::to_gray(&ops::fit(p, r.screen.width, r.screen.height, 255));
        let mut o = Pix::new(g.w, g.h, 3);
        for (i, v) in g.d.iter().enumerate() { let t = q(*v as f32); for ch in 0..3 { o.d[i * 3 + ch] = cl(ink[ch] + (paper[ch] - ink[ch]) * t) } }
        return o;
    }
    // цветной (Kaleido): яркость в полном разрешении, цвет — с половинным и приглушённый
    let f = ops::to_rgb(&ops::fit(p, r.screen.width, r.screen.height, 255));
    let (w, h) = (f.w as usize, f.h as usize); let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
    let mut cc = vec![0f32; hw * hh * 3];
    for y in 0..h { for x in 0..w { let i = (y * w + x) * 3; let j = ((y >> 1) * hw + (x >> 1)) * 3; for ch in 0..3 { cc[j + ch] += f.d[i + ch] as f32 * 0.25 } } }
    let cf = s.chroma as f32 / 100.0; let dkf = 0.06 * (cf / 0.36).min(1.5);
    let mut o = Pix::new(f.w, f.h, 3);
    for y in 0..h { for x in 0..w {
        let i = (y * w + x) * 3; let j = ((y >> 1) * hw + (x >> 1)) * 3;
        let l = 0.299 * f.d[i] as f32 + 0.587 * f.d[i + 1] as f32 + 0.114 * f.d[i + 2] as f32;
        let cl0 = 0.299 * cc[j] + 0.587 * cc[j + 1] + 0.114 * cc[j + 2];
        let ch3 = [cc[j] - cl0, cc[j + 1] - cl0, cc[j + 2] - cl0];
        let dk = (ch3[0].abs() + ch3[1].abs() + ch3[2].abs()) * dkf;
        for ch in 0..3 { let t = q(l - dk + ch3[ch] * cf); o.d[i + ch] = cl(ink[ch] + (paper[ch] - ink[ch]) * t) }
    } }
    o
}
