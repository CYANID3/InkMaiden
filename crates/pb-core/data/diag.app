#!/bin/sh
# InkMaiden — модуль диагностики для PocketBook.
# Только читает сведения о системе (/proc, /sys, /ebrmain) и пишет отчёт:
#   /mnt/ext1/system/inkmaiden/diag.txt и копии заводского рабочего стола в system/inkmaiden/desktop
# Ничего не меняет в настройках и системе. Удалить модуль — удалить этот файл (или кнопкой в InkMaiden).
OUT="${INKMAIDEN_OUT:-/mnt/ext1/system/inkmaiden}"
mkdir -p "$OUT" || exit 1
TMP="$OUT/diag.tmp"
LOG="$OUT/diag.log"
step() { echo "$(date +%H:%M:%S) $1" >> "$LOG" 2>/dev/null; }
step "старт"
sec() { echo "@@@ $1"; }
show() { [ -r "$1" ] && cat "$1" 2>/dev/null; }
kv() { for f in "$@"; do [ -f "$f" ] && [ -r "$f" ] && echo "$(basename "$f")=$(head -n 1 "$f" 2>/dev/null)"; done; }
{
  sec report; echo "version=1"; echo "time=$(date +%s)"
  sec firmware; show /ebrmain/version.txt; show /ebrmain/config/version.txt
  sec ebrmain; ls /ebrmain 2>/dev/null
  sec fwfiles; ls /mnt/ext1/system/config/firmware /ebrmain/config 2>/dev/null
  # номер прошивки (U743K3.6.10.2433): только небольшие текстовые файлы (до 256 КБ) — программы прошивки не читаются:
  # grep по большим двоичным файлам на читалке может работать минутами и съесть память
  P="[A-Za-z][0-9]{3}[A-Za-z0-9]*[.][0-9]+[.][0-9]+([.][0-9]+)?"
  sec fwgrep
  for f in /ebrmain/version* /ebrmain/config/*.cfg /ebrmain/config/*.txt /ebrmain/fsimage.md5 /ebrmain/pocketbook/*.txt /ebrmain/pocketbook/*.cfg /mnt/secure/*.cfg /mnt/secure/*.txt /proc/cmdline; do
    [ -f "$f" ] || continue
    n=$(wc -c < "$f" 2>/dev/null); [ -n "$n" ] && [ "$n" -lt 262144 ] 2>/dev/null || continue
    grep -o -E "$P" "$f" 2>/dev/null | head -n 2
  done
  sec fwhints
  for f in /ebrmain/config/device.cfg /ebrmain/config/global.cfg; do [ -f "$f" ] && grep -i -E "version|firmware|build|swver" "$f" 2>/dev/null | head -n 4; done
  ls /ebrmain/pocketbook 2>/dev/null | head -n 40
  step "прошивка: готово"
  sec uname; uname -a 2>/dev/null
  sec proc_version; show /proc/version
  sec cpuinfo; show /proc/cpuinfo
  sec cpufreq; kv /sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq /sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_min_freq /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
  sec cpus; show /sys/devices/system/cpu/online
  sec meminfo; show /proc/meminfo
  sec uptime; show /proc/uptime
  sec loadavg; show /proc/loadavg
  sec mounts; show /proc/mounts
  sec partitions; show /proc/partitions
  sec df; df -k 2>/dev/null
  sec modules; show /proc/modules
  for d in /sys/class/power_supply/*; do
    [ -d "$d" ] || continue
    sec "power:$(basename "$d")"
    for f in "$d"/*; do [ -f "$f" ] && [ -r "$f" ] && echo "$(basename "$f")=$(head -n 1 "$f" 2>/dev/null)"; done
  done
  for d in /sys/block/mmcblk*; do
    [ -d "$d" ] || continue
    sec "block:$(basename "$d")"
    echo "size=$(show "$d/size")"
    kv "$d/device/name" "$d/device/type" "$d/device/manfid" "$d/device/oemid" "$d/device/date" "$d/device/fwrev" "$d/device/hwrev" "$d/device/rev" "$d/device/life_time" "$d/device/pre_eol_info" "$d/device/cid"
  done
  for d in /sys/class/thermal/thermal_zone*; do
    [ -d "$d" ] || continue
    sec "thermal:$(basename "$d")"; kv "$d/type" "$d/temp"
  done
  sec fb; kv /sys/class/graphics/fb0/name /sys/class/graphics/fb0/virtual_size /sys/class/graphics/fb0/bits_per_pixel
  sec net; for d in /sys/class/net/*; do [ -d "$d" ] && echo "$(basename "$d")=$(show "$d/operstate")"; done
  sec dmesg; dmesg 2>/dev/null | grep -i -E "mmc|i/o error|fat-fs|ext4|vfat|ecc|crc" | tail -n 80
} > "$TMP" 2>/dev/null
step "сбор: готово"
# рабочий стол: точный заводской список приложений (по USB /ebrmain не виден) — копии в $OUT/desktop
D="$OUT/desktop"; mkdir -p "$D"
for f in /ebrmain/config/desktop/* /ebrmain/config/desktop_apps.cfg; do
  [ -f "$f" ] || continue
  n=$(wc -c < "$f" 2>/dev/null); [ -n "$n" ] && [ "$n" -lt 1048576 ] 2>/dev/null && cp -f "$f" "$D/" 2>/dev/null
done
{ ls /ebrmain/bin 2>/dev/null; ls /ebrmain/applications 2>/dev/null; } > "$D/bin.txt"
# какие программы из каталога есть на самом деле: каждый "path" из каталога проверяется на читалке (есть — 1, нет — 0)
: > "$D/paths.txt"
for f in /ebrmain/config/desktop/*.json; do
  [ -f "$f" ] || continue
  grep -o '"path"[[:space:]]*:[[:space:]]*"[^"]*"' "$f" 2>/dev/null | sed 's/.*"\([^"]*\)"$/\1/' | while read -r p; do
    case "$p" in /*) [ -e "$p" ] && echo "$p	1" || echo "$p	0" ;; esac
  done >> "$D/paths.txt"
done
# названия приложений на языке читалки: только строки @Ключ=… (небольшая выборка, не весь файл)
# папка apps рядом с каталогом — копии файлов (плоско, с приставкой apps__)
for f in /ebrmain/config/desktop/apps/*; do
  [ -f "$f" ] || continue
  n=$(wc -c < "$f" 2>/dev/null); [ -n "$n" ] && [ "$n" -lt 1048576 ] 2>/dev/null && cp -f "$f" "$D/apps__$(basename "$f")" 2>/dev/null
  grep -o '"path"[[:space:]]*:[[:space:]]*"[^"]*"' "$f" 2>/dev/null | sed 's/.*"\([^"]*\)"$/\1/' | while read -r p; do
    case "$p" in /*) [ -e "$p" ] && echo "$p	1" || echo "$p	0" ;; esac
  done >> "$D/paths.txt"
done
# правила desktop_apps.cfg (show/hide по ключам вроде have_pbcloud=0): значения этих ключей из настроек читалки
: > "$D/flags.txt"
for k in partner $(sed 's/^[^=]*=//' /ebrmain/config/desktop_apps.cfg 2>/dev/null | tr ',' '\n' | grep '=' | cut -d= -f1 | sort -u); do
  v=$(grep -h "^$k=" /ebrmain/config/*.cfg /ebrmain/config/*.cfg.gen /mnt/ext1/system/config/*.cfg /mnt/secure/*.cfg 2>/dev/null | tail -n 1)
  [ -n "$v" ] && echo "$v" >> "$D/flags.txt"
done
L=/ebrmain/language/ru.txt; [ -r "$L" ] || L=/ebrmain/language/en.txt
[ -r "$L" ] && grep -E "^@[A-Za-z_]+=" "$L" 2>/dev/null | head -n 4000 > "$D/lang.txt"
# коротко — и в сам отчёт: что за файлы и какого вида (чтобы InkMaiden знал формат этой прошивки)
{
  echo "@@@ desktop_files"; ls -l /ebrmain/config/desktop /ebrmain/config/desktop_apps.cfg 2>/dev/null
  echo "@@@ desktop_apps"; ls /ebrmain/applications 2>/dev/null | head -n 80
  echo "@@@ desktop_appsdir"; ls -l /ebrmain/config/desktop/apps 2>/dev/null | head -n 60
  echo "@@@ desktop_flags"; cat "$D/flags.txt" 2>/dev/null
  echo "@@@ desktop_paths"; cat "$D/paths.txt" 2>/dev/null | head -n 80
  echo "@@@ desktop_cfg"; head -n 60 /ebrmain/config/desktop_apps.cfg 2>/dev/null
  for f in /ebrmain/config/desktop/*.json; do [ -f "$f" ] && { echo "@@@ desktop_json:$(basename "$f")"; head -c 1500 "$f" 2>/dev/null; echo; }; done
} >> "$TMP" 2>/dev/null
step "рабочий стол: готово"
mv -f "$TMP" "$OUT/diag.txt" && step "отчёт записан"
sync
[ -x /ebrmain/bin/dialog ] && /ebrmain/bin/dialog 2 "" "InkMaiden: отчёт готов. Подключите читалку к компьютеру." "OK" >/dev/null 2>&1 &
exit 0
