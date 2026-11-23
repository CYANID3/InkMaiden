//! Названия: чистка («Коллекция_ужасов_-_1_-_1_Томиэ» → «Коллекция ужасов - 1 - 0001 Томиэ»),
//! разбор на серию/том/главу/название и сборка по шаблону. Перенесено из pb-tools.

use fancy_regex::{Captures, Regex};
use std::collections::HashMap;
use std::sync::OnceLock;

fn rx(s: &str) -> Regex { Regex::new(s).unwrap() }
fn vol_rx() -> &'static Regex { static R: OnceLock<Regex> = OnceLock::new(); R.get_or_init(|| rx(r"(?i)(?<!\p{L})(?<kw>Том|Tom|Volume|Vol\.?|Книга|Часть|Part|v(?=\d))\s*(?<num>\d+)(?<let>[A-Za-z](?![A-Za-zА-Яа-яЁё]))?(?<frac>\.\d+(?!\s*$))?")) }
fn chap_rx() -> &'static Regex { static R: OnceLock<Regex> = OnceLock::new(); R.get_or_init(|| rx(r"(?i)(?<!\p{L})(?<kw>Глава|Гл\.?|Chapter|Ch\.?|Выпуск|Issue|Эпизод|Episode|c(?=\d)|#)\s*(?<num>\d+)(?<let>[A-Za-z](?![A-Za-zА-Яа-яЁё]))?(?<frac>\.\d+(?!\s*$))?")) }

fn g<'a>(c: &'a Captures, n: &str) -> &'a str { c.name(n).map(|m| m.as_str()).unwrap_or("") }

pub fn prepared(s: &str) -> String {
    let s = rx(r"\[[^\]]*\]|\{[^}]*\}").replace_all(s, " ");
    let s = rx(r"_+").replace_all(&s, " ");
    let s = rx(r"\.(?=\s)").replace_all(&s, " ");
    let s = rx(r"\s+-\s+").replace_all(&s, " - ");
    let s = rx(r"\s{2,}").replace_all(&s, " ");
    s.trim().to_string()
}

/// содержимое скобок прячется (длина не меняется), чтобы номера в «(Часть 1)» не трогать
fn masked(s: &str) -> String {
    rx(r"\([^()]*\)").replace_all(s, |c: &Captures| {
        let m = c.get(0).unwrap().as_str(); let n = m.chars().count();
        format!("({})", "·".repeat(n.saturating_sub(2)))
    }).to_string()
}

/// Заменяет совпадения только вне скобок. Работает по символам, т.к. «·» в маске — многобайтный.
fn replace_outside(s: &str, re: &Regex, f: &dyn Fn(&Captures) -> String) -> String {
    let m = masked(s);
    let mchars: Vec<(usize, char)> = m.char_indices().collect();
    let schars: Vec<char> = s.chars().collect();
    let to_ci = |b: usize| mchars.iter().position(|(i, _)| *i == b).unwrap_or(mchars.len());
    let mut out = String::new(); let mut last = 0usize;
    for c in re.captures_iter(&m).flatten() {
        let all = c.get(0).unwrap(); let (a, b) = (to_ci(all.start()), to_ci(all.end()));
        out.extend(&schars[last..a]); out.push_str(&f(&c)); last = b;
    }
    out.extend(&schars[last..]);
    out
}

fn max_width(names: &[String], re: &Regex) -> usize {
    names.iter().flat_map(|n| re.captures_iter(&masked(n)).flatten().map(|c| g(&c, "num").trim_start_matches('0').len()).collect::<Vec<_>>()).max().unwrap_or(0)
}

fn pad(s: &str, w: usize) -> String { format!("{:0>w$}", s, w = w) }

pub fn clean_names(base: &[String]) -> Vec<String> {
    let prep: Vec<String> = base.iter().map(|s| prepared(s)).collect();
    let vw = max_width(&prep, vol_rx()).max(2); let cw = max_width(&prep, chap_rx()).max(3);
    let has_kw = prep.iter().any(|s| { let m = masked(s); vol_rx().is_match(&m).unwrap_or(false) || chap_rx().is_match(&m).unwrap_or(false) });
    let last_num = rx(r"\d+(?!.*\d)");
    let tw = if has_kw { 0 } else { prep.iter().filter_map(|n| last_num.find(&masked(n)).ok().flatten().map(|m| m.as_str().trim_start_matches('0').len())).max().unwrap_or(0).max(2) };
    let dash = rx(r"(?i)((?:Глава|Гл\.?|Chapter|Ch\.?|Выпуск|Issue|Эпизод|Episode) [0-9]+[A-Z]?(?:\.\d+)?|#[0-9]+[A-Z]?(?:\.\d+)?) (?!- )(?=\S)");
    prep.into_iter().map(|mut s| {
        if has_kw {
            s = replace_outside(&s, vol_rx(), &|c| {
                let kw = g(c, "kw");
                if kw.eq_ignore_ascii_case("v") { return format!("v{}{}", pad(g(c, "num"), vw), g(c, "let").to_uppercase()) }
                let kw = if kw.to_lowercase().starts_with("tom") { "Том".to_string() } else if kw.to_lowercase().starts_with("vol") { "Vol".to_string() } else { kw.to_string() };
                format!("{} {}{}{}", kw, pad(g(c, "num"), vw), g(c, "let").to_uppercase(), g(c, "frac"))
            });
            s = replace_outside(&s, chap_rx(), &|c| {
                let kw = g(c, "kw"); let let_ = g(c, "let").to_uppercase();
                let mut num = g(c, "num").to_string(); if !let_.is_empty() { num = num.trim_start_matches('0').to_string() }
                let sep = if kw == "#" { "#".to_string() } else if kw.eq_ignore_ascii_case("c") { "c".to_string() } else { format!("{kw} ") };
                format!("{}{}{}{}", sep, pad(&num, cw), let_, g(c, "frac"))
            });
            s = replace_outside(&s, &dash, &|c| format!("{} - ", c.get(1).unwrap().as_str()));
        } else if let Ok(Some(m)) = last_num.find(&masked(&s)) {
            let chars: Vec<char> = s.chars().collect(); let mc: Vec<(usize, char)> = masked(&s).char_indices().collect();
            let a = mc.iter().position(|(i, _)| *i == m.start()).unwrap(); let b = mc.iter().position(|(i, _)| *i == m.end()).unwrap_or(mc.len());
            s = format!("{}{}{}", chars[..a].iter().collect::<String>(), pad(m.as_str(), tw), chars[b..].iter().collect::<String>());
        }
        let s = rx(r"\s{2,}").replace_all(&s, " ");
        s.trim().trim_end_matches(['.', ' ']).to_string()
    }).collect()
}

/// Части названия: серия, том, глава, название главы
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Parts { pub series: String, pub vol: String, pub ch: String, pub title: String }

pub fn split_parts(s: &str) -> Parts {
    let mk = masked(s);
    let mut r = Parts::default();
    // позиции — в символах (маска и строка совпадают посимвольно)
    let ci = |b: usize| mk[..b].chars().count();
    let val = |c: &Captures| format!("{}{}{}", g(c, "num"), g(c, "let").to_uppercase(), g(c, "frac"));
    let (mut first, mut end): (Option<usize>, usize) = (None, 0);
    let vm = vol_rx().captures(&mk).ok().flatten(); let cm = chap_rx().captures(&mk).ok().flatten();
    if let Some(c) = &vm { r.vol = val(c); let m = c.get(0).unwrap(); first = Some(ci(m.start())); end = ci(m.end()) }
    if let Some(c) = &cm {
        r.ch = val(c); let m = c.get(0).unwrap();
        if first.map(|f| ci(m.start()) < f).unwrap_or(true) { first = Some(ci(m.start())) }
        end = end.max(ci(m.end()));
    }
    if vm.is_none() && cm.is_none() {
        let nums: Vec<(usize, usize, String)> = rx(r"(?<![\p{L}\d.])\d+[A-Z]?(?:\.\d+)?(?![\p{L}\d])").find_iter(&mk).flatten().map(|m| (ci(m.start()), ci(m.end()), m.as_str().to_string())).collect();
        let dash = mk.find(" - ").map(ci);
        let mut c: Vec<_> = nums.iter().filter(|n| dash.map(|d| n.0 > d).unwrap_or(true)).cloned().collect();
        if c.is_empty() { c = nums }
        if c.len() >= 2 { r.vol = c[0].2.clone(); r.ch = c[1].2.clone(); first = Some(c[0].0); end = c[1].1 }
        else if c.len() == 1 { r.ch = c[0].2.clone(); first = Some(c[0].0); end = c[0].1 }
    }
    let chars: Vec<char> = s.chars().collect();
    match first {
        Some(f) => { r.series = chars[..f].iter().collect(); r.title = chars[end.min(chars.len())..].iter().collect() }
        None => r.title = s.to_string(),
    }
    r.series = rx(r"[\s\-–—_.,:#]+$").replace(&r.series, "").trim().to_string();
    r.title = rx(r"^[\s\-–—_.,:]+").replace(&r.title, "").trim().to_string();
    if r.series.is_empty() && r.vol.is_empty() && r.ch.is_empty() { r.series = std::mem::take(&mut r.title) }
    r
}

/// Собрать название по шаблону: {series} {vol} {ch} {title} {n} {folder} {name} {orig}; {ch:3} — с нулями;
/// [ … ] — кусок пропадает, если поле внутри пустое.
pub fn format_template(tpl: &str, v: &HashMap<&str, String>) -> String {
    let field = rx(r"\{(\w+)(?::(\d+))?\}");
    let get = |c: &Captures| -> String {
        let x = v.get(c.get(1).unwrap().as_str().to_lowercase().as_str()).cloned().unwrap_or_default();
        if let (false, Some(w)) = (x.is_empty(), c.get(2)) {
            if let Ok(Some(m)) = rx(r"^(\d+)(.*)$").captures(&x) { return format!("{}{}", pad(m.get(1).unwrap().as_str(), w.as_str().parse().unwrap_or(0)), m.get(2).unwrap().as_str()) }
        }
        x
    };
    let s = rx(r"\[([^\[\]]*)\]").replace_all(tpl, |c: &Captures| {
        let inner = c.get(1).unwrap().as_str();
        if field.captures_iter(inner).flatten().any(|fc| get(&fc).is_empty()) { return String::new() }
        field.replace_all(inner, |fc: &Captures| get(fc)).to_string()
    });
    let s = field.replace_all(&s, |fc: &Captures| get(fc));
    let s = rx(r"\(\s*\)|\[\s*\]").replace_all(&s, "");
    let s = rx(r"\s{2,}").replace_all(&s, " ");
    let s = rx(r"(\s-)+\s*$").replace(&s, "");
    let s = rx(r"^\s*(-\s)+").replace(&s, "");
    s.trim().trim_matches(['-', '_', '.', ' ']).to_string()
}

/// Готовые шаблоны: (шаблон, человеческое название)
pub fn templates() -> Vec<(&'static str, &'static str)> {
    vec![
        ("", "Навести порядок в исходном названии"),
        ("{series}[ - Том {vol}][ - Глава {ch:3}][ - {title}]", "Серия — Том — Глава — Название"),
        ("{series} {ch:3}[ {title}]", "Серия и номер главы"),
        ("[{vol} - ]{ch}[ - {title}]", "Только номера и название"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleans_like_pb_tools() {
        let v = clean_names(&["Коллекция_ужасов_от_Дзюндзи_Ито_-_1_-_1_Томиэ".into(), "Коллекция_ужасов_от_Дзюндзи_Ито_-_16_-_1_Франкенштейн_(Часть_1)".into()]);
        assert_eq!(v[0], "Коллекция ужасов от Дзюндзи Ито - 1 - 01 Томиэ");
        assert_eq!(v[1], "Коллекция ужасов от Дзюндзи Ито - 16 - 01 Франкенштейн (Часть 1)");
        let v = clean_names(&["[Mangalib] Берсерк Том 1 Глава 5 Чёрный мечник".into(), "Берсерк Том 12 Глава 101.5".into()]);
        assert_eq!(v[0], "Берсерк Том 01 Глава 005 - Чёрный мечник");
        assert_eq!(v[1], "Берсерк Том 12 Глава 101.5");
        let p = split_parts(&v[0]);
        assert_eq!((p.series.as_str(), p.vol.as_str(), p.ch.as_str(), p.title.as_str()), ("Берсерк", "01", "005", "Чёрный мечник"));
        let p = split_parts("Коллекция ужасов от Дзюндзи Ито - 16 - 1 Франкенштейн (Часть 1)");
        assert_eq!((p.vol.as_str(), p.ch.as_str(), p.title.as_str()), ("16", "1", "Франкенштейн (Часть 1)"));
        let mut m = HashMap::new(); m.insert("series", "Берсерк".to_string()); m.insert("vol", "1".into()); m.insert("ch", "5".into()); m.insert("title", String::new());
        assert_eq!(format_template("{series}[ - Том {vol}][ - Глава {ch:3}][ - {title}]", &m), "Берсерк - Том 1 - Глава 005");
    }
}
