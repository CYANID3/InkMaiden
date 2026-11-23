// «Моё устройство»: текущая читалка целиком.
// Что видно по USB и с диска читалки — показывается как есть; процессор, память, батарея — по справочнику модели
// (по USB их не узнать); версию прошивки можно вписать вручную. Подробности с диска (что занимает память,
// библиотека, настройки, приложения) читаются при открытии раздела и запоминаются — без подключения виден последний снимок.
import { call, on, openPath, pickFiles } from '../api.js';
import { h, fill, icon, size, inch, date, dateTime, dialog, toast, plural, progress, busy, span, live, maidy } from '../ui.js';
import { eject } from './uploads.js';
import * as Q from '../queue.js';

let C, root, off, offQ, D = null, loadingNow = false, forId = null, stage = null, offP = null;
const unk = (t = 'не видно по USB') => h('b', { class: 'unk' }, t);
const val = v => v == null || v === '' ? unk() : h('b', {}, String(v));
const spec = (label, value, extra) => h('div', { class: 'spec' }, h('small', {}, label), value, extra || null);
// яркие и непохожие друг на друга: синий, малиновый, бирюзовый, жёлтый, фиолетовый, графит
const COLORS = { books: '#2F80ED', comics: '#E0367A', audio: '#12B5A0', images: '#F2A900', other: '#8E5CF7', system: '#4A4F57' };

export default {
  id: 'device', title: 'Моё устройство', hint: 'читалка и её память', icon: 'readers',
  mount(main, ctx) {
    C = ctx; root = h('div', { class: 'page' }); main.append(root);
    const d = ctx.cur(); D = d.details || null; forId = d.id;
    off = ctx.onDevices(() => {
      if (!root.isConnected) return;
      const c = ctx.cur();
      if (c && (c.id !== forId || (c.connected && !loadingNow && !fresh()))) { forId = c.id; D = c.details || null; load(); }
      render();
    });
    on('details://progress', p => { stage = p; if (root.isConnected && loadingNow) { const el = root.querySelector('#det-op'); if (el) fill(el, opLine()); } }).then(u => { offP = u; });
    offQ = Q.onChange(() => { if (root.isConnected && D) render(); });
    render(); load();
  },
  unmount() { off && off(); offP && offP(); offQ && offQ(); },
};

const fresh = () => D && D.at && Date.now() / 1000 - D.at < 120;

async function load(force) {
  const d = C.cur(); if (!d || loadingNow) return;
  if (!d.connected) { D = d.details || null; render(); return; }
  if (!force && fresh()) return;
  loadingNow = true; stage = null; render();
  try {
    D = await call('device_details', { id: d.id });
    // отчёт модуля только что забран с читалки — сказать об этом и подсветить
    justGot = !!(D && D.diagNew);
    // убрать модуль с читалки — тоже только через «На читалку»: по настройке пункт добавляется сам
    const qd = justGot && D.diagAutoRemove && D.diagInstalled;
    if (qd) await Q.add(C, [diagItem('diag-remove')], { quiet: true });
    if (justGot) toast(qd ? 'Отчёт получен. Убрать проверку с читалки — добавлено в «На читалку».' : 'Отчёт снят и загружен на компьютер.', 6000);
  } catch (e) { toast(String(e)); }
  loadingNow = false; if (root.isConnected) render();
}
const opLine = () => progress({ stage: (stage && stage.stage) || 'Читаю диск читалки', detail: stage && stage.files ? `просмотрено ${stage.files.toLocaleString('ru-RU')} файлов` : 'книги, настройки, библиотека, диск', compact: true });

function render() {
  const d = C.cur(); if (!d) return;
  const top = root.querySelector('.page-body') ? root.querySelector('.page-body').scrollTop : 0;
  fill(root,
    h('header', { class: 'page-head' }, h('div', {}, h('h1', {}, 'Моё устройство'),
      h('p', {}, 'Память, книги и настройки вашей читалки.'))),
    // заголовок и нижняя строка на месте — листается только содержимое
    h('div', { class: 'page-body' },
    d.connected ? null : h('div', { class: 'off-banner mz-off', role: 'status' }, maidy(d.ejected ? 'cheer' : 'usb', 'mz-bob'),
      h('div', {}, h('b', {}, d.ejected ? `${d.name} безопасно извлечена — можно вынуть кабель` : `${d.name} не подключена`), h('small', {}, `Показано, как было${d.lastOff ? ` ${dateTime(d.lastOff)}` : D && D.at ? ` ${dateTime(D.at)}` : ' при последнем подключении'}. Подключите кабелем и выберите на читалке «Связь с ПК».`))),
    hero(d),
    details(d),
    C.dev.unknown.length ? h('section', { class: 'panel' }, h('h2', {}, 'Подключены другие устройства'),
      C.dev.unknown.map(u => h('p', { class: 'hint', style: { margin: '4px 0' } },
        `${u.seen.root} — ${u.model || u.seen.label}: ${u.known ? 'сохранение новых читалок выключено в настройках' : 'модели нет в справочнике, работать с ней пока нельзя'}.`))) : null,
    h('section', {},
      h('h2', { style: { margin: '10px 0 6px' } }, 'Сохранённые читалки'),
      h('p', { class: 'hint' }, 'Нажмите на читалку, чтобы работать с ней.'),
      h('div', { class: 'saved' }, C.dev.devices.map(o => h('button', { class: 'dcard', 'aria-current': o.id === d.id, title: o.id === d.id ? 'Текущая читалка' : 'Сделать текущей', onclick: () => { if (o.id !== d.id) C.setCurrent(o.id); } },
        h('span', {}, o.photo ? h('img', { src: o.photo, alt: '' }) : h('span', { html: icon('readers') })),
        h('b', {}, o.name),
        h('small', {}, o.model ? o.model.name : o.code),
        h('small', {}, o.connected ? h('span', { class: 'chip ok' }, 'подключена') : o.lastSeen ? `была ${date(o.lastSeen)}` : '')))))),
    // как во всех разделах: действия — нижней строкой, главное справа
    h('div', { class: 'actions' },
      h('div', { class: 'where' }, d.connected ? ['Подключена — ', h('b', {}, d.since ? live(t => `${span(t)}, диск ${d.connected.replace(/\\$/, '')}`, d.since) : `диск ${d.connected.replace(/\\$/, '')}`)]
        : d.lastOff ? `${d.ejected ? 'Безопасно извлечена' : 'Отключена'} ${dateTime(d.lastOff)}` : 'Не подключена'),
      d.connected ? h('button', { class: 'btn quiet', disabled: loadingNow, onclick: () => load(true) }, h('span', { html: icon('refresh') }), 'Обновить') : null,
      d.connected ? h('button', { class: 'btn', title: 'Сбросить записи на диск и отключить читалку', disabled: loadingNow, onclick: () => eject(C) }, 'Безопасно извлечь') : null,
      Q.applyButton(C, null, { title: 'Открыть «На читалку» — всё, что ждёт записи' })));
  const w = root.querySelector('.page-body'); if (w && top) w.scrollTop = top;
}

/* ---------------------------------------------------------------- шапка: фото, имя, память, характеристики */
function memory(d) {
  const s = d.seen || {}, total = s.total, free = s.free;
  if (!total) return h('div', { class: 'mem' }, h('div', { class: 'mem-head' }, h('b', {}, 'Память'), h('span', { class: 'hint', style: { margin: 0 } }, 'станет видно, когда читалка подключится')), h('div', { class: 'bar2' }));
  const parts = (D && D.content || []).filter(c => c.bytes > 0);
  const known = parts.reduce((a, c) => a + c.bytes, 0), used = total - free;
  // что не посчитано по файлам (служебное место файловой системы) — к «Системе»
  const segs = parts.length ? parts.map(c => ({ ...c, bytes: c.key === 'system' ? c.bytes + Math.max(0, used - known) : c.bytes })) : [{ key: 'other', label: 'Занято', bytes: used }];
  return h('div', { class: 'mem' },
    // отключённая читалка: это прошлый снимок — серым и с подписью, чтобы не путать с тем, что есть сейчас
    h('div', { class: 'mem-head' }, h('b', {}, d.connected ? 'Память' : 'Память — прошлый снимок'),
      h('span', { class: 'hint', style: { margin: 0 } }, d.connected ? `${size(free)} свободно из ${size(total)}` : `было ${size(free)} свободно из ${size(total)} · ${dateTime(d.lastOff || d.lastSeen)}`)),
    // ровная полоса: у каждой части не меньше 6 px (иначе мелкие пропадают точкой), между частями одинаковый зазор,
    // свободное место — своей частью до конца полосы
    h('div', { class: 'bar2', role: 'img', 'aria-label': segs.map(c => `${c.label} ${size(c.bytes)}`).join(', ') + `, свободно ${size(free)}` },
      segs.map(c => h('i', { title: `${c.label}: ${size(c.bytes)}`, style: { flexBasis: `${100 * c.bytes / total}%`, background: COLORS[c.key] || COLORS.other } })),
      h('i', { class: 'free', title: `Свободно: ${size(free)}` })),
    h('div', { class: 'legend' },
      segs.map(c => h('span', { style: { '--c': COLORS[c.key] || COLORS.other } }, h('em', {}, c.label), h('b', {}, size(c.bytes)),
        c.count && c.key !== 'system' ? h('small', {}, `${c.count} ${plural(c.count, 'файл', 'файла', 'файлов')}`) : null)),
      h('span', { class: 'free', style: { '--c': 'var(--paper)' } }, h('em', {}, 'Свободно'), h('b', {}, size(free)))));
}

function hero(d) {
  const s = d.seen || {}, m = d.model || {}, sc = d.reader.screen;
  const ref = v => v == null ? unk('нет в справочнике') : h('b', {}, v);
  // что сообщила сама читалка (модуль диагностики) — точнее справочника
  const dv = l => { const x = D && D.diag && D.diag.items.find(i => i.label === l); return x && x.value; };
  const yes = v => v == null ? null : v ? 'есть' : 'нет';
  return h('section', { class: 'panel hero' + (d.connected ? '' : ' is-off') },
    h('div', { class: 'pic' }, d.photo ? h('img', { src: d.photo, alt: d.name }) : h('span', { html: icon('readers'), style: { width: '90px', color: 'var(--muted)' } })),
    h('div', { style: { minWidth: 0 } },
      h('div', { class: 'tags' },
        d.connected ? h('span', { class: 'chip ok', title: d.since ? `подключена с ${dateTime(d.since)}` : null }, `подключена, диск ${d.connected.replace(/\\$/, '')}`, d.since ? live(t => ` · ${span(t)}`, d.since) : null)
          : h('span', { class: 'chip mute' }, d.lastOff ? `отключена ${dateTime(d.lastOff)}` : d.lastSeen ? `не подключена, была ${dateTime(d.lastSeen)}` : 'не подключена'),
        
        loadingNow ? h('span', { class: 'chip mute' }, 'читаю диск…') : null),
      h('h2', {}, d.name),
      h('p', { class: 'sub' }, [m.name || `PocketBook ${d.code}`, m.year ? `${m.year} год` : null].filter(Boolean).join(', ')),
      memory(d),
      h('h3', { class: 'spec-h' }, 'Экран и корпус'),
      h('div', { class: 'specs' },
        spec('Экран', h('b', {}, `${inch(sc.inches)} ${m.display || ''}`.trim())),
        spec('Разрешение', h('b', {}, `${sc.width}×${sc.height}${m.ppi ? `, ${m.ppi} ppi` : ''}`)),
        spec('Цвет', h('b', {}, sc.color ? `цветной${m.colorPpi ? `, ${m.colorPpi} ppi в цвете` : ''}` : `${sc.gray_levels} оттенков серого`)),
        spec('Корпус', h('b', {}, m.body ? `${m.body.map(x => String(x).replace('.', ',')).join('×')} мм` : `${d.reader.body.width_mm}×${d.reader.body.height_mm} мм`)),
        spec('Защита от воды', ref(m.water || (m.cpu ? 'нет' : null))),
        spec('Год выхода', ref(m.year || null))),
      h('h3', { class: 'spec-h' }, 'Внутри ', h('small', {}, D && D.diag ? `по проверке от ${dateTime(D.diag.time)}` : 'по справочнику модели — по USB её не видно')),
      h('div', { class: 'specs' },
        spec('Процессор', ref(D && D.diag && dv('Ядер') ? [dv('Процессор'), `${dv('Ядер')} ${plural(+dv('Ядер'), 'ядро', 'ядра', 'ядер')}`, (dv('Частота') || '').split('–').pop().trim() || null].filter(Boolean).join(', ') : [m.cpu, dv('Процессор')].filter(Boolean).join(', ') || null)), spec('Оперативная память', ref(dv('Всего') || (m.ramMb ? (m.ramMb >= 1000 ? `${String(m.ramMb / 1000).replace('.', ',')} ГБ` : `${m.ramMb} МБ`) : null))),
        spec('Батарея', ref([m.batteryMah ? `${m.batteryMah} мА·ч` : null, dv('Заряд') ? `заряд ${dv('Заряд')} на момент отчёта` : null, dv('Здоровье (по контроллеру)') ? `состояние ${dv('Здоровье (по контроллеру)')}` : null].filter(Boolean).join(', ') || null)), spec('Встроенная память', ref(dv('Объём чипа') ? [dv('Объём чипа'), dv('Производитель') && dv('Производитель').replace(/ \(.*\)$/, ''), dv('Модель чипа')].filter(Boolean).join(', ') : m.storageGb ? `${m.storageGb} ГБ` : null)),
        spec('Связь', ref(m.wifi ? [m.wifi.replace('Wi-Fi', 'Wi‑Fi'), m.bluetooth ? 'Bluetooth' : null].filter(Boolean).join(', ') : null)),
        spec('Звук и аудиокниги', ref(yes(m.audio)))),
      h('h3', { class: 'spec-h' }, 'Система'),
      h('div', { class: 'specs' },
        spec('Прошивка', dv('Прошивка') ? h('b', { title: 'по проверке читалки' }, dv('Прошивка')) : d.firmware ? h('b', {}, d.firmware)
          : D && D.diag ? unk('в отчёте не нашлась') : unk(), h('button', { class: 'btn quiet', style: { minHeight: '28px', padding: '0 8px' }, onclick: () => firmware(d) }, d.firmware ? 'Изменить' : 'Указать вручную')),
        spec('Ядро Linux', dv('Ядро Linux') ? h('b', { title: 'по проверке читалки' }, dv('Ядро Linux')) : val(s.kernel)),
        spec('Прошивка обновлялась', s.last_update ? h('b', {}, date(s.last_update)) : unk()),
        spec('Язык', val(s.language)), spec('Часовой пояс', val(s.timezone)), spec('Тема оформления', val(s.theme)),
        spec('Иконка сна', d.sleep && d.sleep.installed ? h('b', {}, d.sleep.installed) : h('b', { class: 'unk' }, d.sleep && d.sleep.pending ? 'ждёт подключения' : 'стандартная или не менялась')),
        spec('Ночной режим', s.night_mode == null ? unk() : h('b', {}, s.night_mode ? 'включён' : 'выключен')),
        spec('USB', s.usb_product ? h('b', {}, [s.usb_vendor, s.usb_product].filter(Boolean).join(' ')) : unk('не подключалась')))));
}

/* ---------------------------------------------------------------- с диска читалки */
function details(d) {
  if (!D) return d.connected ? h('section', { class: 'panel', id: 'det-op' }, loadingNow ? opLine() : h('p', { class: 'hint', style: { margin: 0 } }, 'Подробностей пока нет.'))
    : h('section', { class: 'panel' }, h('p', { class: 'hint', style: { margin: 0 } }, 'Подключите читалку — программа прочитает с неё библиотеку, настройки и что занимает память.'));
  const st = D.settings || {}, lib = D.library;
  const head = h('div', { class: 'det-head' },
    h('h2', {}, 'На читалке сейчас'),
    h('small', { class: 'hint', style: { margin: 0 } }, d.connected ? `прочитано ${dateTime(D.at)}` : `последний снимок — ${dateTime(D.at)}`),
);
  const kv = (k, v) => v == null || v === '' || (Array.isArray(v) && !v.length) ? null : h('div', { class: 'kv' }, h('span', {}, k), h('b', {}, Array.isArray(v) ? v.join(', ') : String(v)));
  const bool = v => v == null ? null : v ? 'да' : 'нет';
  const libBox = lib && !lib.error ? h('div', { class: 'panel det' },
    h('h3', {}, 'Библиотека'),
    h('div', { class: 'nums' },
      h('div', {}, h('b', {}, lib.total ?? '—'), h('small', {}, plural(lib.total || 0, 'книга', 'книги', 'книг'))),
      lib.reading != null ? h('div', {}, h('b', {}, lib.reading), h('small', {}, 'читается')) : null,
      lib.finished != null ? h('div', {}, h('b', {}, lib.finished), h('small', {}, 'прочитано')) : null),
    lib.recent && lib.recent.length ? h('div', { class: 'recent' }, h('small', { class: 'hint', style: { margin: 0 } }, 'Последние открытые'),
      lib.recent.map(r => h('div', { class: 'rbook' },
        h('div', {}, h('b', {}, r.title || 'Без названия'), h('small', {}, [r.author, r.opened ? date(r.opened) : null].filter(Boolean).join(', '))),
        r.pages ? h('div', { class: 'rprog', title: `стр. ${r.page} из ${r.pages}` }, h('i', { style: { width: `${Math.min(100, 100 * r.page / r.pages)}%` } }), h('small', {}, `${Math.round(100 * r.page / r.pages)} %`)) : null))) : null)
    : h('div', { class: 'panel det' }, h('h3', {}, 'Библиотека'), h('p', { class: 'hint', style: { margin: 0 } }, lib && lib.error ? `Базу библиотеки не удалось прочитать: ${lib.error}` : 'Базы библиотеки на читалке нет — откройте «Библиотеку» на самой читалке хотя бы раз.'));
  const grid = h('div', { class: 'det-grid' },
      libBox,
      h('div', { class: 'panel det' }, h('h3', {}, 'Настройки на читалке'),
        kv('Шрифт интерфейса', st.font), kv('Шрифт книг', st.readerFont && `${st.readerFont}${st.readerFontSize ? `, ${st.readerFontSize}` : ''}`),
        kv('Подсветка', st.frontlight != null ? `${st.frontlight} %` : null), kv('Тёплый свет', st.warmth != null ? `${st.warmth} %` : null),
        kv('Словарь', st.dictionary), kv('Раскладки', st.keyboards),
        kv('Проверять обновления', st.autoUpdate == null ? null : st.autoUpdate ? `да${st.updateInterval ? `, раз в ${st.updateInterval.replace('1day', 'день').replace('1week', 'неделю')}` : ''}` : 'нет'),
        kv('Последняя проверка', st.lastUpdateCheck ? dateTime(st.lastUpdateCheck) : null),
        kv('Заряд в процентах', bool(st.batteryPercent)), kv('Статистика чтения', bool(st.statistics)), kv('Магазин', st.partner)),
      h('div', { class: 'panel det' }, h('h3', {}, 'Установлено'),
        kv('Свои приложения', D.apps && D.apps.length ? D.apps : 'нет'),
        kv('Словари', D.dictionaries && D.dictionaries.length ? D.dictionaries : 'нет'),
        kv('Шрифты в system/fonts', D.fonts ? `${D.fonts}` : 'нет'),
        kv('Заставки сна', d.seen && d.seen.logos && d.seen.logos.SleepLogo ? `${d.seen.logos.SleepLogo.random} шт.` : null)));
  return h('section', { class: 'details' }, head, loadingNow ? h('div', { class: 'panel', id: 'det-op' }, opLine()) : null,
    loadingNow ? busy(grid, 'Обновляю…') : grid, diagnostics(d));
}

/* ---------------------------------------------------------------- диагностика: Windows о диске и модуль на читалке */
const CATS = [['health', 'Здоровье памяти'], ['storage', 'Встроенная память (eMMC)'], ['system', 'Прошивка и система'], ['cpu', 'Процессор'], ['ram', 'Оперативная память'],
  ['battery', 'Батарея'], ['screen', 'Экран'], ['disk', 'Разделы'], ['net', 'Сеть']];
function diagnostics(d) {
  const k = D.disk, g = D.diag, items = (g && g.items) || [];
  // строка: название — значение; источник — мелко под значением, только когда включено «Откуда данные»
  const row = (label, value, source, level, more) => h('div', { class: 'drow' + (level ? ' ' + level : ''), title: source ? `Источник: ${source}` : null },
    h('span', {}, label), h('b', {}, more ? h('button', { class: 'link', onclick: more }, value) : value), showSrc && source ? h('small', {}, source) : null);
  // «подробнее» у строк журнала ядра: сами строки и что они значат
  const more = x => x.label === 'Ошибки памяти в журнале ядра' && g.dmesg && g.dmesg.length ? () => logDialog('Ошибки памяти в журнале ядра',
      'Строки журнала ядра, где встроенная память (mmc0 / mmcblk0) или файловая система сообщили об ошибке. Единичные ошибки при сне и пробуждении бывают; если их много и они повторяются — сохраните важное и проверьте память на читалке.', g.dmesg)
    : x.label === 'Wi-Fi модуль в журнале' && g.wifiLog && g.wifiLog.length ? () => logDialog('Wi-Fi модуль в журнале',
      'Это не ошибки памяти. Wi-Fi в этой читалке подключён к той же шине SD (sdc1 / mmc1, SDIO). При включении и выключении Wi-Fi драйвер пишет такие строки («err», «RTO», «card removed») — это обычная работа модуля, на книги и память они не влияют.', g.wifiLog) : null;
  const card = (title, rows, extra) => rows.filter(Boolean).length || extra ? h('div', { class: 'panel dcard2' }, h('h3', {}, title), rows, extra) : null;
  const win = k ? [
    k.fs ? row('Файловая система', `${k.fs}${k.cluster ? `, кластер ${size(k.cluster)}` : ''}`, 'Windows: GetVolumeInformation, GetDiskFreeSpace') : null,
    k.dirty != null ? row('Проверка тома', k.dirty ? 'нужна проверка диска' : 'ошибок нет', 'Windows: FSCTL_IS_VOLUME_DIRTY', k.dirty ? 'bad' : 'ok') : null,
    k.partition ? row('Раздел для файлов', size(k.partition), 'Windows: IOCTL_DISK_GET_LENGTH_INFO') : null,
    k.diskSize ? row('Диск целиком', size(k.diskSize), 'Windows: IOCTL_DISK_GET_DRIVE_GEOMETRY_EX') : null,
    k.bus ? row('Подключение', `${k.bus}${k.removable ? ', съёмный' : ''}`, 'Windows: IOCTL_STORAGE_QUERY_PROPERTY') : null,
    k.serial ? row('Серийный номер USB', k.serial, 'Windows: IOCTL_STORAGE_QUERY_PROPERTY') : null,
    k.volumeSerial ? row('Серийный номер тома', k.volumeSerial, 'Windows: GetVolumeInformation') : null,
    k.smart != null ? row('SMART', k.smart ? 'отвечает' : 'по USB недоступен', 'Windows: SMART_GET_VERSION', k.smart ? null : 'muted') : null,
  ] : [];
  const pick = l => items.find(x => x.label === l);
  const worst = items.filter(x => x.cat === 'health' && x.level && /Износ|Запасные/.test(x.label)).sort((a, b) => ({ bad: 2, warn: 1, ok: 0 }[b.level] - { bad: 2, warn: 1, ok: 0 }[a.level]))[0];
  const tile = (label, x, fallback) => !x && !fallback ? null : h('div', { class: 'dtile' + (x && x.level ? ' ' + x.level : '') }, h('small', {}, label), h('b', {}, x ? x.value : fallback || '—'));
  const temp = items.find(x => x.cat === 'cpu' && /Температура/.test(x.label));
  // состояние модуля: собран отчёт — заметная зелёная плашка; иначе — шаги
  const status = g ? h('div', { class: 'diag-ok' + (justGot ? ' fresh' : '') },
      h('span', { class: 'ok-mark', html: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>' }),
      h('div', {}, h('b', {}, justGot ? 'Отчёт собран и загружен' : 'Отчёт с читалки'),
        h('small', {}, [`снят ${dateTime(g.time)}`, D.diagFile ? 'сохранён на компьютере' : null,
          D.diagInstalled ? (queued('diag-remove') ? 'удаление проверки ждёт в «На читалку»' : 'проверка ещё на читалке') : 'проверки на читалке нет'].filter(Boolean).join(' · '))),
      h('div', { class: 'row-btns' },
        h('button', { class: 'btn quiet', title: 'Открыть другой файл отчёта', onclick: () => diagLoad(d) }, 'Другой отчёт…'),
        D.diagFile ? h('button', { class: 'btn quiet', onclick: () => openPath(D.diagFile.replace(/[\\/][^\\/]+$/, '')).catch(e => toast(String(e))) }, 'Открыть папку') : null,
        D.diagInstalled ? (queued('diag-remove') ? null : h('button', { class: 'btn', onclick: () => diagRemove(d) }, 'Убрать проверку…')) : h('button', { class: 'btn', onclick: () => diagInstall(d) }, 'Снять заново…')))
    : h('div', { class: 'panel dcard2 diag-steps' }, h('h3', {}, 'Узнать больше о читалке'),
      h('p', { class: 'hint', style: { margin: 0 } }, 'Прошивку, заряд, износ батареи и памяти знает только сама читалка. Их узнает маленькое приложение-проверка — оно только читает и ничего не меняет.'),
      h('ol', { class: 'steps' }, h('li', {}, '«Поставить проверку» добавит её в «На читалку»; там она и запишется — один файл.'), h('li', {}, 'Отключите читалку, откройте «Приложения» → «@InkMaiden_Diag». Через секунду появится «отчёт готов».'),
        h('li', {}, C.behavior('diagAutoRemove') ? 'Подключите читалку снова — отчёт скопируется на компьютер, а проверка сама уберётся через «На читалку».' : 'Подключите читалку снова — отчёт скопируется на компьютер. Убрать проверку — «Убрать проверку…».')),
      h('div', { class: 'row-btns' },
        D.diagInstalled ? [h('span', { class: 'chip ok' }, 'проверка на читалке — запустите её'), queued('diag-remove') ? null : h('button', { class: 'btn quiet', onclick: () => diagRemove(d) }, 'Убрать проверку…')]
          : queued('diag') ? h('span', { class: 'chip' }, 'проверка ждёт в «На читалку»') : h('button', { class: 'btn primary', onclick: () => diagInstall(d) }, 'Поставить проверку…'),
        // отчёт уже есть (снят раньше, лежит в Diagnostics или на читалке в system/inkmaiden) — открыть его
        h('button', { class: 'btn', onclick: () => diagLoad(d) }, 'Загрузить готовый отчёт…')));
  const cards = [
    ...CATS.map(([c, title]) => card(title, items.filter(x => x.cat === c).map(x => row(x.label, x.value, x.source, x.level, more(x))),
      c === 'health' && g && !items.some(x => x.cat === 'health') ? h('p', { class: 'hint', style: { margin: 0 } }, 'Контроллер памяти этой читалки не сообщает износ (нет life_time и pre_eol_info). Показателей здоровья нет — оценивать их по свободному месту было бы неправдой.') : null)),
    card('Диск — как его видит Windows', win, !k ? h('p', { class: 'hint', style: { margin: 0 } }, 'Появится после «Обновить» с подключённой читалкой.') : null),
  ].filter(Boolean);
  return h('section', { class: 'diag' },
    h('div', { class: 'det-head' }, h('h2', {}, 'Состояние читалки'), h('small', { class: 'hint', style: { margin: 0 } }, 'только чтение — ничего не меняется')),
    // модуль запускался, но отчёт не дописал — показать, на каком шаге остановился
    D.diagUnfinished ? h('div', { class: 'off-banner' }, h('span', { html: icon('refresh') }),
      h('div', {}, h('b', {}, 'Проверка запускалась, но отчёт не дописан'),
        h('small', {}, 'Похоже, читалка прервала его. Последние шаги: ', D.diagUnfinished.trim().split('\n').slice(-3).join(' → '), '. Запустите «@InkMaiden_Diag» ещё раз; если снова не выйдет — пришлите фото экрана читалки.'))) : null,
    status,
    g ? h('div', { class: 'dtiles' },
      tile('Прошивка', pick('Прошивка'), d.firmware), tile('Износ памяти', worst, 'не сообщается'), tile('Батарея', pick('Здоровье (по контроллеру)') || pick('Остаточная ёмкость')),
      tile('Заряд', pick('Заряд')), tile('Процессор', temp), tile('Свободно ОЗУ', pick('Свободно сейчас'))) : null,
    // подробности — для тех, кому интересно: свёрнуты, чтобы не пугать цифрами
    cards.length ? h('details', { class: 'fine diag-more', open: moreOpen, ontoggle: e => { moreOpen = e.target.open; } },
      h('summary', {}, 'Подробно (для специалистов)'),
      h('label', { class: 'check src-toggle' }, h('input', { type: 'checkbox', checked: showSrc, onchange: e => { showSrc = e.target.checked; render(); } }), 'Показать, откуда взяты данные'),
      h('div', { class: 'dmasonry' }, cards)) : null);
}
let showSrc = false, justGot = false, moreOpen = false;
const logDialog = (title, text, lines) => dialog(close => h('div', { class: 'dlg dlg-wide' }, h('h2', {}, title), h('p', { style: { margin: 0 } }, text),
  h('pre', { class: 'logpre' }, lines.join('\n')),
  h('div', { class: 'dlg-foot' }, h('button', { class: 'btn primary', onclick: () => close(true) }, 'Понятно'))));
async function diagLoad(d) {
  const dir = (D && D.diagFile ? D.diagFile.replace(/[\\/][^\\/]+$/, '') : `${d.dirs.books.replace(/[\\/][^\\/]+$/, '')}\\Diagnostics`);
  const f = (await pickFiles('Отчёт диагностики (diag-….txt или diag.txt)', dir))[0];
  if (!f) return;
  try { D = await call('device_diag_load', { id: d.id, path: f }); toast('Отчёт загружен'); render(); }
  catch (e) { toast(String(e)); }
}
// модуль диагностики ставится и убирается только через «На читалку»
const diagItem = kind => ({ kind, src: '', dst: 'applications', label: kind === 'diag' ? 'Проверка читалки (InkMaiden_Diag)' : 'Убрать проверку с читалки' });
const queued = kind => Q.list(C).some(x => x.kind === kind);
const diagInstall = () => Q.addAndGo(C, [diagItem('diag')]);
const diagRemove = () => Q.addAndGo(C, [diagItem('diag-remove')]);

async function firmware(d) {
  const v = await dialog(close => h('form', { class: 'dlg', onsubmit: e => { e.preventDefault(); close(e.target.f.value) } },
    h('h2', {}, 'Версия прошивки'),
    h('p', { class: 'hint', style: { margin: 0 } }, 'Читалка не пишет её ни в один файл, видимый по USB. Посмотрите на читалке: Настройки → О устройстве → Программное обеспечение — и впишите сюда, например U743K3.6.10.2433.'),
    h('input', { class: 'input', name: 'f', value: d.firmware || '', placeholder: 'U743K3.6.10.2433', autofocus: true }),
    h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', type: 'button', onclick: () => close(null) }, 'Отмена'), h('button', { class: 'btn primary' }, 'Сохранить'))));
  if (v == null) return;
  try { await call('device_update', { id: d.id, firmware: v }); await C.refreshDevices(); toast('Версия прошивки сохранена'); } catch (e) { toast(String(e)); }
}
