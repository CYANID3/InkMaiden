//! Диагностика с самой читалки. Что знает только система читалки (версия прошивки, процессор, ОЗУ, батарея,
//! износ встроенной памяти eMMC), по USB не видно. Поэтому есть модуль — shell-скрипт data/diag.app: пользователь
//! запускает его на читалке, он только читает /proc, /sys и /ebrmain и пишет отчёт system/inkmaiden/diag.txt.
//! Здесь отчёт разбирается в понятные пункты; у каждого — откуда он взят.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Сам модуль: ставится в applications/InkMaiden_Diag.app
pub const SCRIPT: &str = include_str!("../data/diag.app");
pub const APP: &str = "applications/InkMaiden_Diag.app";
pub const REPORT: &str = "system/inkmaiden/diag.txt";

#[derive(Debug, Serialize, PartialEq)]
pub struct Item { pub cat: &'static str, pub label: String, pub value: String, pub source: String,
    /// ok / warn / bad — для показателей здоровья
    #[serde(skip_serializing_if = "Option::is_none")] pub level: Option<&'static str> }

fn sections(t: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let mut cur: Option<String> = None;
    for line in t.lines() {
        if let Some(n) = line.strip_prefix("@@@ ") { cur = Some(n.trim().to_string()); m.entry(n.trim().to_string()).or_insert_with(String::new); continue }
        if let Some(c) = &cur { let e = m.get_mut(c).unwrap(); e.push_str(line); e.push('\n') }
    }
    m
}
fn kv(s: &str, sep: char) -> BTreeMap<String, String> {
    s.lines().filter_map(|l| l.split_once(sep)).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect()
}
fn gb(bytes: f64) -> String { if bytes >= 1073741824.0 { format!("{:.1} ГБ", bytes / 1073741824.0).replace('.', ",") } else { format!("{:.0} МБ", bytes / 1048576.0) } }
fn hex(s: &str) -> Option<u32> { u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok() }

/// производители eMMC по коду JEDEC (manfid)
fn maker(id: u32) -> Option<&'static str> {
    Some(match id { 0x02 | 0x45 => "SanDisk / WD", 0x11 => "Toshiba / Kioxia", 0x13 | 0xFE => "Micron", 0x15 => "Samsung", 0x70 => "Kingston", 0x88 => "Foresee (Longsys)",
        0x90 => "SK hynix", 0x9B => "YMTC", 0xD6 => "Foresee", 0x32 => "Phison", 0x89 => "Intel", 0xF4 => "Biwin", _ => return None })
}

/// износ eMMC: life_time «0x01 0x02» — доли ресурса по типам ячеек, шагами по 10 %
fn life(v: u32) -> Option<String> {
    match v { 0 => None, 1..=10 => Some(format!("израсходовано {}–{} %", (v - 1) * 10, v * 10)), 11 => Some("ресурс исчерпан".into()), _ => None }
}

pub fn parse(t: &str) -> Value {
    let s = sections(t);
    let mut it: Vec<Item> = vec![];
    let mut add = |cat: &'static str, label: &str, value: String, source: &str, level: Option<&'static str>| { if !value.trim().is_empty() { it.push(Item { cat, label: label.into(), value, source: source.into(), level }) } };
    let get = |k: &str| s.get(k).map(|x| x.trim().to_string()).unwrap_or_default();
    let rep = kv(&get("report"), '=');

    // прошивка и система
    // прошивка: из version.txt, иначе — из имени файла в system/config/firmware или строки в /ebrmain
    let fw = fw_token(&get("firmware")).map(|f| (f, "/ebrmain/version.txt"))
        .or_else(|| fw_token(&get("fwfiles")).map(|f| (f, "system/config/firmware")))
        .or_else(|| fw_token(&get("fwgrep")).map(|f| (f, "/ebrmain (строка версии)")));
    if let Some((f, src)) = fw { add("system", "Прошивка", f, src, None) }
    let pv = get("proc_version");
    if let Some(k) = pv.split_whitespace().nth(2) { add("system", "Ядро Linux", k.to_string(), "/proc/version", None) }
    if let Some(gcc) = pv.find("(gcc").map(|i| pv[i + 1..].split(')').next().unwrap_or("").to_string()) { add("system", "Собрано", gcc, "/proc/version", None) }
    if let Some(up) = get("uptime").split_whitespace().next().and_then(|x| x.parse::<f64>().ok()) {
        let (d, h, m) = ((up / 86400.0) as u64, ((up % 86400.0) / 3600.0) as u64, ((up % 3600.0) / 60.0) as u64);
        add("system", "Без перезагрузки", if d > 0 { format!("{d} д {h} ч {m} мин") } else { format!("{h} ч {m} мин") }, "/proc/uptime", None);
    }
    if let Some(l) = get("loadavg").split_whitespace().next() { add("system", "Загрузка процессора", format!("{l} (средняя за минуту)"), "/proc/loadavg", None) }
    let mods: Vec<String> = get("modules").lines().filter_map(|l| l.split_whitespace().next().map(String::from)).collect();
    if !mods.is_empty() { add("system", "Модули ядра", format!("{}: {}", mods.len(), mods.join(", ")), "/proc/modules", None) }

    // процессор
    let cpu = get("cpuinfo");
    let ck = kv(&cpu, ':');
    let cores = cpu.lines().filter(|l| l.starts_with("processor")).count();
    let name = ck.get("Hardware").or(ck.get("model name")).or(ck.get("Processor")).cloned().unwrap_or_default();
    add("cpu", "Процессор", name, "/proc/cpuinfo", None);
    if cores > 0 { add("cpu", "Ядер", cores.to_string(), "/proc/cpuinfo", None) }
    if let Some(m) = ck.get("model name").or(ck.get("Processor")) { if ck.contains_key("Hardware") { add("cpu", "Ядро ARM", m.clone(), "/proc/cpuinfo", None) } }
    if let Some(r) = ck.get("Revision") { add("cpu", "Ревизия платы", r.clone(), "/proc/cpuinfo", None) }
    let fq = kv(&get("cpufreq"), '=');
    let mhz = |k: &str| fq.get(k).and_then(|x| x.parse::<u64>().ok()).map(|k| format!("{} МГц", k / 1000));
    if let (Some(a), Some(b)) = (mhz("cpuinfo_min_freq"), mhz("cpuinfo_max_freq")) { add("cpu", "Частота", format!("{a} – {b}"), "/sys/devices/system/cpu/cpu0/cpufreq", None) }
    if let Some(g) = fq.get("scaling_governor") { add("cpu", "Режим частоты", g.clone(), "/sys/devices/system/cpu/cpu0/cpufreq", None) }
    for (k, v) in s.iter().filter(|(k, _)| k.starts_with("thermal:")) {
        let t = kv(v, '=');
        if let Some(c) = t.get("temp").and_then(|x| x.parse::<f64>().ok()) { add("cpu", &format!("Температура ({})", t.get("type").cloned().unwrap_or(k[8..].into())), format!("{:.0} °C", if c > 1000.0 { c / 1000.0 } else { c }), &format!("/sys/class/{}", k.replace(':', "/")), None) }
    }

    // оперативная память
    let mem = kv(&get("meminfo"), ':');
    let kb = |k: &str| mem.get(k).and_then(|x| x.split_whitespace().next()).and_then(|x| x.parse::<f64>().ok()).map(|x| x * 1024.0);
    if let Some(t) = kb("MemTotal") { add("ram", "Всего", gb(t), "/proc/meminfo", None) }
    if let Some(a) = kb("MemAvailable").or(kb("MemFree")) { add("ram", "Свободно сейчас", gb(a), "/proc/meminfo", None) }
    if let Some(sw) = kb("SwapTotal") { add("ram", "Подкачка", if sw > 0.0 { gb(sw) } else { "нет".into() }, "/proc/meminfo", None) }

    // батарея
    for (k, v) in s.iter().filter(|(k, _)| k.starts_with("power:")) {
        let p = kv(v, '=');
        if p.get("type").map(|t| t != "Battery").unwrap_or(false) && !p.contains_key("capacity") { continue }
        let src = format!("/sys/class/power_supply/{}", &k[6..]);
        if let Some(c) = p.get("capacity") { add("battery", "Заряд", format!("{c} %"), &src, None) }
        if let Some(st) = p.get("status") { add("battery", "Состояние", match st.as_str() { "Charging" => "заряжается", "Discharging" => "разряжается", "Full" => "заряжена", "Not charging" => "не заряжается", x => x }.to_string(), &src, None) }
        if let Some(hh) = p.get("health") { add("battery", "Здоровье (по контроллеру)", match hh.as_str() { "Good" => "хорошее", "Overheat" => "перегрев", "Dead" => "неисправна", "Over voltage" => "перенапряжение", "Cold" => "переохлаждение", x => x }.to_string(), &src, Some(if hh == "Good" { "ok" } else { "bad" })) }
        if let Some(v) = p.get("voltage_now").and_then(|x| x.parse::<f64>().ok()) { add("battery", "Напряжение", format!("{:.2} В", if v > 100.0 { v / 1e6 } else { v }).replace('.', ","), &src, None) }
        if let Some(t) = p.get("temp").and_then(|x| x.parse::<f64>().ok()) { add("battery", "Температура", format!("{:.0} °C", t / 10.0), &src, None) }
        if let Some(t) = p.get("technology") { add("battery", "Тип", t.clone(), &src, None) }
        if let Some(c) = p.get("cycle_count") { add("battery", "Циклов заряда", c.clone(), &src, None) }
        let num = |k: &str| p.get(k).and_then(|x| x.parse::<f64>().ok()).filter(|x| *x > 0.0);
        if let (Some(full), Some(design)) = (num("charge_full").or(num("energy_full")), num("charge_full_design").or(num("energy_full_design"))) {
            let pct = 100.0 * full / design;
            add("battery", "Остаточная ёмкость", format!("{pct:.0} % от новой"), &src, Some(if pct >= 80.0 { "ok" } else if pct >= 60.0 { "warn" } else { "bad" }));
        }
        if let Some(d) = num("charge_full_design") { add("battery", "Ёмкость по паспорту", format!("{:.0} мА·ч", if d > 100000.0 { d / 1000.0 } else { d }), &src, None) }
    }

    // встроенная память (eMMC)
    let mut errors = None;
    // служебные области чипа (boot0, boot1, rpmb) — одной строкой, а не повтором «Объём чипа»
    let aux: Vec<String> = s.iter().filter(|(k, _)| k.starts_with("block:") && (k.contains("boot") || k.contains("rpmb")))
        .filter_map(|(k, v)| { let name = if k.contains("rpmb") { "RPMB".to_string() } else { k[k.find("boot").unwrap()..].to_string() };
            kv(v, '=').get("size").and_then(|x| x.parse::<f64>().ok()).map(|sz| format!("{name} {}", gb(sz * 512.0))) }).collect();
    for (k, v) in s.iter().filter(|(k, _)| k.starts_with("block:") && !k.contains("boot") && !k.contains("rpmb")) {
        let b = kv(v, '=');
        if b.get("type").map(|t| t != "MMC").unwrap_or(false) { continue }
        let src = format!("/sys/block/{}/device", &k[6..]);
        if let Some(sz) = b.get("size").and_then(|x| x.parse::<f64>().ok()) { add("storage", "Объём чипа", gb(sz * 512.0), &format!("/sys/block/{}/size", &k[6..]), None) }
        if let Some(n) = b.get("name") { add("storage", "Модель чипа", n.clone(), &src, None) }
        if let Some(m) = b.get("manfid").and_then(|x| hex(x)) { add("storage", "Производитель", maker(m).map(|x| format!("{x} (0x{m:02X})")).unwrap_or(format!("код 0x{m:02X}")), &src, None) }
        if let Some(d) = b.get("date") { add("storage", "Дата выпуска чипа", d.clone(), &src, None) }
        if let Some(f) = b.get("fwrev") { add("storage", "Прошивка контроллера", f.clone(), &src, None) }
        if let Some(r) = b.get("rev") { add("storage", "Версия стандарта eMMC", r.clone(), &src, None) }
        if let Some(lt) = b.get("life_time") {
            let v: Vec<u32> = lt.split_whitespace().filter_map(hex).collect();
            for (i, x) in v.iter().enumerate() {
                if let Some(t) = life(*x) { add("health", &format!("Износ ячеек, тип {}", if i == 0 { "A (SLC)" } else { "B (MLC)" }), t, &src, Some(if *x <= 7 { "ok" } else if *x <= 9 { "warn" } else { "bad" })) }
            }
        }
        if let Some(e) = b.get("pre_eol_info").and_then(|x| hex(x)) {
            let (t, l) = match e { 1 => ("в норме", "ok"), 2 => ("предупреждение: израсходовано 80 % запасных блоков", "warn"), 3 => ("срочно: запасные блоки почти кончились", "bad"), _ => ("не сообщает", "ok") };
            add("health", "Запасные блоки", t.into(), &src, Some(l));
        }
        errors = Some(0);
    }
    if !aux.is_empty() { add("storage", "Служебные области", aux.join(", "), "/sys/block/mmcblk0boot*, rpmb", None) }
    // журнал ядра: ошибки самой памяти (mmc0/mmcblk0, ввод-вывод, файловая система) отдельно от сообщений Wi-Fi-модуля —
    // он тоже на шине SD (sdc1/mmc1, SDIO) и при включении/выключении Wi-Fi пишет «err», это не ошибки памяти
    let dm = get("dmesg");
    let wifi = |l: &str| { let l = l.to_lowercase(); ["sdc1", "mmc1", "sdio", "aicwf", "aicbsp", "eth0", "wlan"].iter().any(|k| l.contains(k)) };
    let storage_err = |l: &str| { let x = l.to_lowercase(); !wifi(l) && !x.contains("not a recommended")
        && (x.contains("i/o error") || x.contains("crc") || x.contains("timeout") || (x.contains("error") && (x.contains("mmc0") || x.contains("mmcblk0") || x.contains("fat-fs") || x.contains("ext4") || x.contains("ext2")))) };
    let errs: Vec<String> = dm.lines().filter(|l| storage_err(l)).map(String::from).collect();
    let wifi_lines: Vec<String> = dm.lines().filter(|l| wifi(l) && { let x = l.to_lowercase(); x.contains("err") || x.contains("rto") || x.contains("remove") }).map(String::from).collect();
    if errors.is_some() || !dm.is_empty() {
        add("health", "Ошибки памяти в журнале ядра", if errs.is_empty() { "нет".into() } else { format!("{} — подробнее", errs.len()) }, "dmesg", Some(if errs.is_empty() { "ok" } else { "warn" }));
    }
    if !wifi_lines.is_empty() {
        add("net", "Wi-Fi модуль в журнале", format!("{} сообщений — подробнее", wifi_lines.len()), "dmesg (sdc1 / SDIO)", None);
    }

    // экран, разделы, сеть
    let fb = kv(&get("fb"), '=');
    if let Some(v) = fb.get("virtual_size") { add("screen", "Буфер экрана", v.replace(',', "×"), "/sys/class/graphics/fb0", None) }
    if let Some(b) = fb.get("bits_per_pixel") { add("screen", "Бит на точку", b.clone(), "/sys/class/graphics/fb0", None) }
    if let Some(n) = fb.get("name") { add("screen", "Драйвер экрана", n.clone(), "/sys/class/graphics/fb0", None) }
    for l in get("mounts").lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() >= 3 && ["/mnt/ext1", "/mnt/ext2", "/ebrmain", "/mnt/secure"].contains(&f[1]) { add("disk", &format!("Раздел {}", f[1]), format!("{} ({})", f[2], f[0]), "/proc/mounts", None) }
    }
    for l in get("df").lines().skip(1) {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() >= 6 && ["/", "/ebrmain", "/mnt/secure"].contains(&f[5]) {
            if let (Ok(t), Ok(u)) = (f[1].parse::<f64>(), f[2].parse::<f64>()) { add("disk", &format!("Занято в {}", f[5]), format!("{} из {}", gb(u * 1024.0), gb(t * 1024.0)), "df", None) }
        }
    }
    let net: Vec<String> = kv(&get("net"), '=').into_iter().filter(|(k, _)| k != "lo").map(|(k, v)| format!("{k} ({v})")).collect();
    if !net.is_empty() { add("net", "Сетевые интерфейсы", net.join(", "), "/sys/class/net", None) }

    json!({ "time": rep.get("time").and_then(|x| x.parse::<i64>().ok()), "items": it,
        "dmesg": errs.iter().take(40).collect::<Vec<_>>(), "wifiLog": wifi_lines.iter().take(40).collect::<Vec<_>>() })
}

/// Номер прошивки PocketBook в тексте: U743K3.6.10.2433, U629.6.11 (буква, 3 цифры, буквы/цифры, затем .число.число…)
pub fn fw_token(t: &str) -> Option<String> {
    t.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.')).find_map(|w| {
        let w = w.trim_matches('.');
        let (head, rest) = w.split_once('.')?;
        let b = head.as_bytes();
        let ok_head = b.len() >= 4 && b[0].is_ascii_alphabetic() && b[1..4].iter().all(u8::is_ascii_digit) && b.iter().all(u8::is_ascii_alphanumeric);
        // числовые части подряд; хвост вроде «.json» отбрасывается
        let parts: Vec<&str> = rest.split('.').take_while(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit())).collect();
        (ok_head && parts.len() >= 2).then(|| format!("{head}.{}", parts.join(".")))
    })
}

/// Журнал шагов модуля на читалке: если отчёта нет, а журнал есть — модуль запускался, но не дошёл до конца
pub const LOG: &str = "system/inkmaiden/diag.log";
/// Незаконченный запуск: журнал без отчёта (что успел сделать модуль)
pub fn unfinished(root: &std::path::Path) -> Option<String> {
    if root.join(REPORT).exists() { return None }
    std::fs::read_to_string(root.join(LOG)).ok().filter(|l| !l.trim().is_empty())
}

/// Папка модуля на читалке (отчёт и его временный файл) — только наша, системное не трогаем
pub const DIR: &str = "system/inkmaiden";

/// Забрать отчёт с читалки: копия в `dest` с датой в имени (diag-2026-11-21_15-04-09.txt), затем разбор.
/// None — отчёта на читалке нет.
pub fn collect(root: &std::path::Path, dest: &std::path::Path) -> std::io::Result<Option<(std::path::PathBuf, Value)>> {
    let src = root.join(REPORT);
    let text = match std::fs::read_to_string(&src) { Ok(t) => t, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None), Err(e) => return Err(e) };
    // журнал шагов модуля — в ту же копию (разбор его пропускает), чтобы было видно, как прошёл запуск
    let text = match std::fs::read_to_string(root.join(LOG)) { Ok(l) if !l.trim().is_empty() => format!("{text}\n@@@ log\n{l}"), _ => text };
    let v = parse(&text);
    let t = v["time"].as_i64().filter(|t| *t > 1_000_000_000).unwrap_or_else(crate::devices::now);
    std::fs::create_dir_all(dest)?;
    let mut out = dest.join(format!("diag-{}.txt", crate::util::stamp_at(t)));
    let mut j = 2; while out.exists() && std::fs::read_to_string(&out).ok().as_deref() != Some(text.as_str()) { out = dest.join(format!("diag-{}-{j}.txt", crate::util::stamp_at(t))); j += 1 }
    std::fs::write(&out, &text)?;
    Ok(Some((out, v)))
}

/// Заводской рабочий стол из отчёта модуля (system/inkmaiden/desktop: view.json, apps_db.json, desktop_apps.cfg, bin.txt, lang.txt) —
/// на компьютер в dest (папка читалки\Desktop\system). Возвращает, скопировано ли что-нибудь.
pub fn collect_desktop(root: &std::path::Path, dest: &std::path::Path) -> std::io::Result<bool> {
    let src = root.join(DIR).join("desktop");
    if !src.is_dir() { return Ok(false) }
    std::fs::create_dir_all(dest)?;
    let mut any = false;
    // всё, что модуль положил (view.json, apps_db.json, desktop_apps.cfg, bin.txt, lang.txt и др.), — только файлы
    for e in std::fs::read_dir(&src)?.flatten() {
        if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            if let Ok(b) = std::fs::read(e.path()) { std::fs::write(dest.join(e.file_name()), b)?; any = true; }
        }
    }
    Ok(any)
}

/// Убрать модуль с читалки целиком: applications/InkMaiden_Diag.app и папку system/inkmaiden.
/// Удаляется только то, что ставит InkMaiden; возвращает, что удалено.
pub fn remove(root: &std::path::Path) -> std::io::Result<Vec<String>> {
    let mut gone = vec![];
    let app = root.join(APP);
    if app.exists() { std::fs::remove_file(&app)?; gone.push(APP.to_string()) }
    let dir = root.join(DIR);
    if dir.is_dir() { std::fs::remove_dir_all(&dir)?; gone.push(DIR.to_string()) }
    Ok(gone)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = "@@@ report\nversion=1\ntime=1795262000\n@@@ firmware\nU743K3.6.10.2433\n@@@ proc_version\nLinux version 4.9.191 (builder@host) (gcc version 6.3.0 (crosstool-NG)) #1 SMP PREEMPT\n\
@@@ cpuinfo\nprocessor\t: 0\nmodel name\t: ARMv7 Processor rev 5 (v7l)\nprocessor\t: 1\nmodel name\t: ARMv7 Processor rev 5 (v7l)\nHardware\t: sun8iw15\nRevision\t: 0000\n\
@@@ cpufreq\ncpuinfo_max_freq=1800000\ncpuinfo_min_freq=480000\nscaling_governor=interactive\n@@@ meminfo\nMemTotal:        1013452 kB\nMemFree:  100000 kB\nMemAvailable:     612000 kB\nSwapTotal: 0 kB\n\
@@@ uptime\n93784.12 1000.0\n@@@ power:battery\ntype=Battery\ncapacity=76\nstatus=Discharging\nhealth=Good\nvoltage_now=3912000\ntemp=243\ncharge_full=2610000\ncharge_full_design=2900000\n\
@@@ block:mmcblk0\nsize=61071360\nname=8GTF4R\ntype=MMC\nmanfid=0x000015\ndate=06/2023\nlife_time=0x01 0x02\npre_eol_info=0x01\n@@@ fb\nvirtual_size=1404,3744\nbits_per_pixel=32\n\
@@@ mounts\n/dev/mmcblk0p1 /mnt/ext1 vfat rw 0 0\n@@@ dmesg\n[ 1.0] mmc0: new HS200 MMC card\n";
    #[test]
    fn parses_report() {
        let v = parse(SAMPLE);
        let f = |l: &str| v["items"].as_array().unwrap().iter().find(|x| x["label"] == l).map(|x| x["value"].as_str().unwrap().to_string());
        assert_eq!(f("Прошивка").unwrap(), "U743K3.6.10.2433");
        assert_eq!(f("Ядро Linux").unwrap(), "4.9.191");
        assert_eq!(f("Процессор").unwrap(), "sun8iw15");
        assert_eq!(f("Ядер").unwrap(), "2");
        assert_eq!(f("Частота").unwrap(), "480 МГц – 1800 МГц");
        assert_eq!(f("Всего").unwrap(), "990 МБ");
        assert_eq!(f("Заряд").unwrap(), "76 %");
        assert_eq!(f("Остаточная ёмкость").unwrap(), "90 % от новой");
        assert_eq!(f("Напряжение").unwrap(), "3,91 В");
        assert_eq!(f("Производитель").unwrap(), "Samsung (0x15)");
        assert_eq!(f("Объём чипа").unwrap(), "29,1 ГБ");
        assert_eq!(f("Износ ячеек, тип A (SLC)").unwrap(), "израсходовано 0–10 %");
        assert_eq!(f("Запасные блоки").unwrap(), "в норме");
        assert_eq!(f("Ошибки памяти в журнале ядра").unwrap(), "нет");
        assert_eq!(f("Без перезагрузки").unwrap(), "1 д 2 ч 3 мин");
        assert_eq!(v["time"], 1795262000);
    }
    /// скрипт запускается в обычном Linux (на читалке тот же sh из busybox) и его отчёт разбирается
    #[cfg(unix)]
    #[test]
    fn script_runs_and_parses() {
        let dir = std::env::temp_dir().join(format!("pbdiag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let st = std::process::Command::new("sh").arg("-c").arg(SCRIPT).env("INKMAIDEN_OUT", &dir).status().unwrap();
        assert!(st.success());
        let t = std::fs::read_to_string(dir.join("diag.txt")).unwrap();
        assert!(!dir.join("diag.tmp").exists());
        let v = parse(&t);
        let labels: Vec<&str> = v["items"].as_array().unwrap().iter().map(|x| x["label"].as_str().unwrap()).collect();
        assert!(labels.contains(&"Ядро Linux") && labels.contains(&"Всего"), "{labels:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn script_is_unix() { assert!(!SCRIPT.contains('\r') && SCRIPT.starts_with("#!/bin/sh")) }

    /// настоящий отчёт с InkPad Color 3 (21.11.2026): ошибок памяти нет, сообщения Wi-Fi — отдельно
    #[test]
    fn real_color3_report() {
        let v = parse(include_str!("../tests/data/diag-color3.txt"));
        let f = |l: &str| v["items"].as_array().unwrap().iter().find(|x| x["label"] == l).map(|x| x["value"].as_str().unwrap().to_string());
        assert_eq!(f("Ошибки памяти в журнале ядра").unwrap(), "нет");
        assert!(f("Wi-Fi модуль в журнале").is_some());
        assert_eq!(f("Ядер").unwrap(), "4");
        assert_eq!(f("Ядро Linux").unwrap(), "4.9.56+");
        assert_eq!(f("Износ ячеек, тип B (MLC)").unwrap(), "израсходовано 0–10 %");
        assert_eq!(f("Служебные области").unwrap(), "boot0 4 МБ, boot1 4 МБ, RPMB 4 МБ");
    }

    #[test]
    fn wifi_noise_is_not_storage_error() {
        let t = "@@@ dmesg\n[ 27.45] sunxi-mmc sdc1: smc 1 p1 err, cmd 7, RE !!\n[ 28.48] mmc1: error -22 whilst initialising SDIO card\n[ 424.7] FAT-fs (mmcblk0p1): utf8 is not a recommended IO charset for FAT filesystems\n";
        let v = parse(t);
        let f = |l: &str| v["items"].as_array().unwrap().iter().find(|x| x["label"] == l).map(|x| x["value"].as_str().unwrap().to_string());
        assert_eq!(f("Ошибки памяти в журнале ядра").unwrap(), "нет");
        assert!(f("Wi-Fi модуль в журнале").unwrap().starts_with("2 "));
        let v = parse("@@@ dmesg\n[ 5.0] mmcblk0: error -110 transferring data\n[ 6.0] Buffer I/O error on dev mmcblk0p1\n");
        assert_eq!(v["dmesg"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn finds_firmware() {
        assert_eq!(fw_token("U743K3.6.10.2433").as_deref(), Some("U743K3.6.10.2433"));
        assert_eq!(fw_token("U629.6.11.json\nother.json").as_deref(), Some("U629.6.11"));
        assert_eq!(fw_token("version 1.2.3 build"), None);
        assert_eq!(fw_token("4.9.56+"), None);
        let v = parse("@@@ fwfiles\nU743K3.6.10.2433.json\n");
        assert!(v["items"].as_array().unwrap().iter().any(|x| x["label"] == "Прошивка" && x["value"] == "U743K3.6.10.2433"));
    }

    #[test]
    fn collect_and_remove() {
        let r = std::env::temp_dir().join(format!("diag-cr-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(r.join("reader/system/inkmaiden")).unwrap();
        std::fs::create_dir_all(r.join("reader/applications")).unwrap();
        std::fs::create_dir_all(r.join("reader/system/config")).unwrap();
        std::fs::write(r.join("reader/system/config/global.cfg"), "x").unwrap();
        std::fs::write(r.join("reader").join(APP), SCRIPT).unwrap();
        std::fs::write(r.join("reader").join(REPORT), SAMPLE).unwrap();
        let (f, v) = collect(&r.join("reader"), &r.join("pc")).unwrap().unwrap();
        assert!(f.file_name().unwrap().to_string_lossy().starts_with("diag-20"));
        assert_eq!(std::fs::read_to_string(&f).unwrap(), SAMPLE);
        assert!(v["items"].as_array().unwrap().len() > 10);
        // повторный забор того же отчёта не плодит копии
        let (f2, _) = collect(&r.join("reader"), &r.join("pc")).unwrap().unwrap();
        assert_eq!(f, f2);
        let gone = remove(&r.join("reader")).unwrap();
        assert_eq!(gone.len(), 2);
        assert!(!r.join("reader").join(APP).exists() && !r.join("reader").join(DIR).exists());
        assert!(r.join("reader/system/config/global.cfg").exists(), "системное не трогаем");
        assert!(collect(&r.join("reader"), &r.join("pc")).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&r);
    }
}
