//! Мелочи: размер по-русски, естественная сортировка, слова с числами.

pub fn format_size(b: f64) -> String {
    let b = b.max(0.0);
    const KB: f64 = 1024.0; const MB: f64 = KB * 1024.0; const GB: f64 = MB * 1024.0;
    let s = if b >= GB { format!("{:.1} ГБ", b / GB) }
        else if b >= 10.0 * MB { format!("{:.0} МБ", b / MB) }
        else if b >= MB { format!("{:.1} МБ", b / MB) }
        else { format!("{} КБ", ((b / KB).round() as i64).max(1)) };
    s.replace('.', ",")
}

/// ключ для естественной сортировки: «2.jpg» раньше «10.jpg»
pub fn sort_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    let mut num = String::new();
    for ch in s.to_lowercase().chars() {
        if ch.is_ascii_digit() { num.push(ch) } else {
            if !num.is_empty() { out.push_str(&format!("{:0>10}", num)); num.clear() }
            out.push(ch)
        }
    }
    if !num.is_empty() { out.push_str(&format!("{:0>10}", num)) }
    out
}

/// «1 том», «2 тома», «5 томов»
pub fn plural<'a>(n: u64, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let (m, h) = (n % 10, n % 100);
    if m == 1 && h != 11 { one } else if (2..=4).contains(&m) && !(12..=14).contains(&h) { few } else { many }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sizes_and_sort() {
        assert_eq!(format_size(1.5 * 1024.0 * 1024.0), "1,5 МБ");
        assert!(sort_key("p2.jpg") < sort_key("p10.jpg"));
        assert_eq!(plural(22, "том", "тома", "томов"), "тома");
        assert_eq!(plural(12, "том", "тома", "томов"), "томов");
    }
}

/// Дни от 1970-01-01 → (год, месяц, день), без внешних библиотек
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
/// Метка времени для имён папок: 2026-11-21_15-04-09 (UTC)
pub fn stamp() -> String { stamp_at(crate::devices::now()) }
/// То же для заданного времени (секунды от 1970, UTC)
pub fn stamp_at(t: i64) -> String {
    let (y, m, d) = civil(t.div_euclid(86400));
    let s = t.rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}", s / 3600, s % 3600 / 60, s % 60)
}
/// Сегодня: 2026-11-21
pub fn today() -> String { stamp()[..10].to_string() }
