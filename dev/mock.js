// Подмена программы на Rust для проверки интерфейса в обычном браузере (без сборки).
// Комиксы — настоящие расчёты на проверочной библиотеке (dev/mock-data.json из examples/mockdata.rs).
// Читалки: InkPad Color 3 подключена на F:\, Verse сохранена и не подключена.
// Сценарии через адрес: ?scene=none — читалок нет; ?scene=offline — сохранены, но не подключены (окно выбора).
const D = await (await fetch(new URL('./mock-data.json', import.meta.url))).json();
// серии: как в Rust (pb-comics split_parts) — название без номера тома
D.items.forEach(it => { const m = it.newName.replace(/\.[^.]+$/, '').match(/^(.*?)[\s_]*v?(\d+)$/i); it.series = m ? m[1].trim() : it.newName; it.vol = m ? m[2] : ''; });
// настоящий заводской каталог InkPad Color 3 (из отчёта модуля 22.11)
const DB743 = await (await fetch(new URL('./apps_db-743K3.json', import.meta.url))).json();
const PH = (await (await fetch(new URL('./mock-photos.json', import.meta.url))).json()).photos;
// справочник — тот же, что встроен в программу
const MODELS = (await (await fetch(new URL('../crates/pb-core/data/models.json', import.meta.url))).json()).models
  .map(m => ({ ...m, colorPpi: m.color_ppi, body: m.body_mm }));
let scene = new URLSearchParams(location.search).get('scene') || '';
// ?scene=big — большая библиотека для проверки скорости: 704 книги, 640 томов в 40 сериях
if (scene === 'big') { const base = D.items; D.items = []; for (let k = 0; k < 40; k++) for (let v = 1; v <= 16; v++) { const b = base[(k + v) % base.length];
  D.items.push({ kind: 'ZIP', name: `Seriya_${k}_v${String(v).padStart(2, '0')}_[scan].cbz`, newName: `Серия ${k} v${String(v).padStart(2, '0')}.cbz`, rel: `${k % 2 ? 'Манга' : 'Комиксы'}/Серия ${k}`, size: b.size }); }
  D.total = D.items.reduce((a, x) => a + x.size, 0);
  D.items.forEach(it => { const m = it.newName.replace(/\.[^.]+$/, '').match(/^(.*?)[\s_]*v?(\d+)$/i); it.series = m ? m[1].trim() : it.newName; it.vol = m ? m[2] : ''; }); }
const subs = {};
const emit = (ev, p) => (subs[ev] || []).forEach(cb => cb(p));
const wait = ms => new Promise(r => setTimeout(r, ms));
const now = () => Date.now() / 1000 | 0;
// приветствие Мэйди в проверках не мешает; ?scene=welcome — показать, как при первом запуске
let config = scene === 'welcome' ? {} : { welcomed: true };
const cancelled = {};
const MODE = ['safe', 'balanced', 'max'];
const famKey = id => /629|verse/i.test(id) ? 'verse' : 'color3';
const optKey = (reader, mode) => `${famKey(reader)}/${MODE.includes(mode) ? mode : 'balanced'}`;
const ratio = (opt, i) => { const r = D.ratios[optKey(opt.reader, opt.mode)]; return r[i % r.length]; };
const ROOT = 'D:\\InkMaiden';

/* ---------------------------------------------------------------- читалки */
const seenC3 = { root: 'F:\\', label: 'PB743K3', code: '743K3', model: 'PocketBook InkPad Color 3', total: 29685035520, free: 13421772800,
  usb_vendor: 'USB-FS', usb_product: 'PocketBook', usb_revision: '0409', kernel: '4.9', photo: 'F:\\system\\device743k3.ico', language: 'ru', timezone: 'Asia/Bishkek',
  night_mode: false, theme: 'Line', last_update: 1795101188, logos: {}, mark: 'PB743K3' };
const seenV = { root: 'G:\\', label: 'PB629', code: '629', model: 'PocketBook Verse', total: 7516192768, free: 2147483648, usb_vendor: 'USB-FS', usb_product: 'PocketBook',
  usb_revision: '0310', kernel: '3.10', photo: null, language: 'ru', timezone: 'Asia/Bishkek', night_mode: false, theme: 'Line', last_update: null, logos: {}, mark: 'PB629' };
const reader = (code, id, name) => Object.assign(structuredClone(D.modelReaders['model:' + code]), { id, name, source: 'device' });
const model = code => { const m = MODELS.find(x => x.code === code); return { name: 'PocketBook ' + m.name, display: m.display, ppi: m.ppi, colorPpi: m.colorPpi, year: m.year, body: m.body,
  cpu: m.cpu, ramMb: m.ram_mb, storageGb: m.storage_gb, batteryMah: m.battery_mah, wifi: m.wifi, bluetooth: m.bluetooth, audio: m.audio, water: m.water }; };
const DETAILS = { at: now(), settings: { font: 'Roboto', readerFont: 'PTSerif', readerFontSize: 36, frontlight: 60, warmth: 40, autoUpdate: true, updateInterval: '1day',
  lastUpdateCheck: 1795182435, dictionary: 'PocketBook Dictionary (En-Ru)', keyboards: ['RU: Russian', 'EN: English'], batteryPercent: true, statistics: true, partner: 'bookland' },
  content: [{ key: 'books', label: 'Книги', count: 1284, bytes: 4.2e9 }, { key: 'comics', label: 'Комиксы и манга', count: 93, bytes: 3.9e9 }, { key: 'audio', label: 'Звук и аудиокниги', count: 12, bytes: 0.8e9 },
    { key: 'images', label: 'Картинки', count: 40, bytes: 0.12e9 }, { key: 'other', label: 'Другое', count: 210, bytes: 0.3e9 }, { key: 'system', label: 'Система', count: 4800, bytes: 2.1e9 }],
  apps: ['Bloknot', 'Dela', 'Kontakty', 'Chasy'], fonts: 14, dictionaries: ['PocketBook Dictionary (En-Ru)'],
  library: { total: 1284, reading: 7, finished: 213, recent: [{ title: 'Пикник на обочине', author: 'Стругацкие', page: 112, pages: 240, opened: now() - 3600 },
    { title: 'Солярис', author: 'Станислав Лем', page: 300, pages: 300, opened: now() - 86400 * 3 }, { title: 'Дюна', author: 'Фрэнк Герберт', page: 40, pages: 700, opened: now() - 86400 * 9 }] } };
const KINDS = { books: 'Books', comics: 'Comics', manga: 'Manga', wallpapers: 'Wallpapers', sleep: 'Sleep_icons' };
const RDEF = { books: 'Books', comics: 'Comics', manga: 'Manga', wallpapers: 'Wallpapers', other: 'Download' };
const mk = (id, name, code, seen, last, connected) => ({ id, name, code, verified: true, remembered: 1795010000, lastSeen: last, seen, reader: reader(code, id, name),
  photo: PH[code] || null, on: connected, paths: { pc: {}, reader: {} }, firmware: code === '629' ? 'U629.6.11.2433' : null, sleep: { pending: null, installed: null, installed_at: null }, model: model(code), details: null });
let devices = scene === 'none' ? [] : [mk('PB743K3', 'InkPad Color 3', '743K3', seenC3, now(), scene !== 'offline'), mk('PB629', 'Verse', '629', seenV, 1795180000, false)];
const T0 = now() - 754; // подключена ~12 минут назад
const json = d => Object.assign({}, d, {
  connected: d.on ? d.seen.root : null, since: d.on ? T0 : null, lastOff: d.on ? null : d.offAt || null,
  dirs: Object.fromEntries(Object.entries(KINDS).map(([k, v]) => [k, d.paths.pc[k] || `${ROOT}\\${d.id}\\${v}`])),
  readerDirs: Object.fromEntries(Object.entries(RDEF).map(([k, v]) => [k, d.paths.reader[k] || v])) });
const DISK = { fs: 'FAT32', cluster: 32768, dirty: false, partition: 29.1e9, diskSize: 31.2e9, bus: 'USB', removable: true, serial: '0123456789ABCDEF', volumeSerial: '4A1F-09C2', smart: false };
const DIAG = await (await fetch(new URL('./diag_sample.json', import.meta.url))).json();
let diagInstalled = location.hash.includes('diag');
// рабочий стол: как у пользователя на Color 3 (Табло и «Рабочий стол» свои, часть стоковых скрыта)
// превью картинок папки обоев: тёмные и светлые, как настоящие обои
const wthumb = i => { const dark = i % 3 !== 1, bg = dark ? '#111' : '#f2f2ee', fg = dark ? '#e8e8e2' : '#222';
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='200' height='260'><rect width='200' height='260' fill='${bg}'/><circle cx='100' cy='${110 + (i % 4) * 8}' r='${46 + i * 4}' fill='none' stroke='${fg}' stroke-width='${3 + i % 3}'/><path d='M40 ${200 - i * 3} Q100 ${150 + i * 6} 160 ${200 - i * 2}' stroke='${fg}' stroke-width='4' fill='none'/><text x='100' y='240' font-family='serif' font-size='16' fill='${fg}' text-anchor='middle'>N ${i + 1}</text></svg>`;
  return 'data:image/svg+xml;base64,' + btoa(svg); };
const onReader = new Set();
const appLib = [{ file: 'Tablo.app', path: 'D:\\InkMaiden\\Apps\\PB743K3\\Tablo.app', size: 412000, title: 'Табло', icon: null, only: true }, { file: 'Markdown.app', path: 'D:\\InkMaiden\\Apps\\Markdown.app', size: 288000, title: 'Markdown', icon: null }, { file: 'Kvesty.app', path: 'D:\\InkMaiden\\Apps\\Kvesty.app', size: 199000, title: null, icon: null }];
const kept = {};
let deskSnap = false; const deskBk = [], deskFiles = ['Desktop.app', 'Tablo.app', 'Bloknot.app', 'InkMaiden_Diag.app'];
const desk = { panel: { applications: [{ id: 'PB_Library' }, { id: 'U_Tablo' }, { id: 'SYS_Apps' }], partner: 'bookland', version: 1 },
  view: { version: '8', applications: { U_Desktop: { path: '/mnt/ext1/applications/Desktop.app', title: 'Рабочий стол' }, U_Tablo: { path: '/mnt/ext1/applications/Tablo.app', title: 'Табло' }, U_Kontakty: { path: '/mnt/ext1/applications/Kontakty.app', title: 'Контакты' },
    PB_Cloud: { visible: '0' }, PB_Dropbox: { visible: '0' }, PB_SendToPB: { visible: '0' }, PB_Bookstore: { visible: '0' }, PB_Overdrive: { visible: '0' }, PB_Snake: { visible: '0' } },
  view: { groups: [{ title: '@General', sort: 'title', apps: ['U_Desktop', 'U_Tablo', 'U_Kontakty', 'PB_AudioBooks', 'PB_Browser', 'PB_Bookstore', 'PB_Empik', 'PB_Calendar', 'PB_Calc', 'PB_Dictionary', 'PB_Digital_Frame', 'PB_Gallery', 'PB_Library', 'PB_Notes', 'PB_Player', 'PB_RSSNews', 'PB_Overdrive', 'PB_Settings'] },
    { title: '@Services', sort: 'title', apps: ['PB_Dropbox', 'PB_Cloud', 'PB_SendToPB'] }, { title: '@Games', sort: 'title', apps: ['PB_Chess', 'PB_Painter', 'PB_Klondike', 'PB_Scribble', 'PB_Sudoku', 'PB_Snake'] }] } } };
function deskApply(plan) {
  const v = desk.view;
  for (const op of plan.apps) { const k = op.key;
    if (op.group || op.remove) v.view.groups.forEach(g => { g.apps = g.apps.filter(x => x !== k); });
    if (op.group && !op.remove) { let g = v.view.groups.find(g => g.title === op.group); if (!g) v.view.groups.push(g = { title: op.group, apps: [] }); g.apps.push(k); }
    if (op.uninstall) { const f = String((v.applications[k] || {}).path || op.path || '').split('/').pop(); const i = deskFiles.indexOf(f); if (i >= 0) deskFiles.splice(i, 1); }
    if (op.remove || op.uninstall) { v.view.groups.forEach(g => { g.apps = g.apps.filter(x => x !== k); }); delete v.applications[k]; continue; }
    const e = v.applications[k] = Object.assign({}, v.applications[k]);
    if (op.path) e.path = op.path; if (op.title != null) e.title = op.title; if (op.icon) { e.icon = { path: op.icon }; e.focused_icon = { path: op.focused }; }
    if (op.hidden === true) e.visible = '0'; else if (op.hidden === false) delete e.visible;
    if (!Object.keys(e).length && !k.startsWith('U_')) delete v.applications[k];
  }
  if (plan.panel) desk.panel.applications = plan.panel.map(id => ({ id }));
}
const bak = new Map(); // «Бэкапы»: путь книги → когда сохранена прежняя версия
let cplans = {}, ckind = 'manga'; const saved = new Set(), savedIdx = { books: new Set() };
let scanned = false; setTimeout(() => { scanned = true; emit('devices://changed'); }, 700); // как наблюдатель USB: первый просмотр дисков
// сцены «reset» (читалка после сброса — без метки) и «twin» (уже записана дважды: PB743K3 и PB743K3-2)
let unknown = [];
if (scene === 'reset') { devices.forEach(d => { d.on = false; }); unknown = [{ seen: Object.assign({}, seenC3, { mark: null }), known: true, model: 'PocketBook InkPad Color 3', candidates: [{ id: 'PB743K3', name: 'InkPad Color 3', lastSeen: now() - 9000 }] }]; }
if (scene === 'orphan') { devices = devices.filter(d => d.id !== 'PB743K3'); devices.unshift(mk('PB743K3-2', 'InkPad Color 3', '743K3', seenC3, now(), true)); }
if (scene === 'twin') { devices.forEach(d => { d.on = false; }); devices.push(mk('PB743K3-2', 'InkPad Color 3', '743K3', seenC3, now(), true)); }
const state = () => ({ devices: devices.map(json), unknown, scanned });
const dev = id => { const d = devices.find(x => x.id === id); if (!d) throw new Error('Такой читалки нет'); return d; };

/* ---------------------------------------------------------------- книги */
const BOOKS = [
  ['Фантастика/Лем/Солярис.fb2', 'FB2', 'Станислав Лем', 'Солярис', '', null, false, ['sf']],
  ['Фантастика/Стругацкие/Пикник на обочине.epub', 'EPUB', 'Аркадий и Борис Стругацкие', 'Пикник на обочине', 'Мир Полудня', 7, true, ['sf_social']],
  ['Фэнтези/Пратчетт/Цвет волшебства.fb2.zip', 'FB2.ZIP', 'Терри Пратчетт', 'Цвет волшебства', 'Плоский мир', 1, false, ['sf_fantasy']],
  ['Фэнтези/Пратчетт/Безумная звезда.fb2', 'FB2', 'Терри Пратчетт', 'Безумная звезда', 'Плоский мир', 2, false, ['sf_fantasy', 'humor']],
  ['Детективы/Кристи/Убийство в Восточном экспрессе.epub', 'EPUB', 'Агата Кристи', 'Убийство в «Восточном экспрессе»', 'Эркюль Пуаро', 10, false, ['det_classic']],
  ['Классика/Булгаков/Мастер и Маргарита.fb2', 'FB2', 'Михаил Булгаков', 'Мастер и Маргарита', '', null, true, ['prose_classic']],
  ['Классика/Чехов/Рассказы.txt', 'TXT', 'Чехов', 'Рассказы', '', null, false, []],
  ['Нон-фикшн/Харари/Sapiens.epub', 'EPUB', 'Юваль Ной Харари', 'Sapiens. Краткая история человечества', '', null, false, ['sci_history']],
  ['Детское/Носов/Незнайка на Луне.fb3', 'FB3', 'Николай Носов', 'Незнайка на Луне', 'Незнайка', 3, false, ['child_tale']],
  ['Разное/Без названия.fb2', 'FB2', '', '', '', null, false, []],
  ['Разное/Сломанная.epub', 'EPUB', '', '', '', null, false, []],
];
let books = [];
const savedRel = new Map(); // номер книги → путь в готовом (своё имя «Автор — Название»)
const bookOut = i => savedRel.get(i) || books[i].rel.replace(/\.txt$/i, '.fb2');
const findings = i => i % 3 === 1 ? [
  { kind: 'page', label: 'Конец ознакомительного фрагмента — реклама полной версии · стр. 5 из 6', preview: 'Конец ознакомительного фрагмента. Текст предоставлен ООО «ЛитРес». Прочитайте эту книгу целиком, купив полную легальную версию на ЛитРес. Безопасно оплатить книгу можно банковской картой Visa, MasterCard, Maestro, со счета мобильного телефона, с платежного терминала, в салоне МТС или Связной, через PayPal, WebMoney, Яндекс.Деньги, QIWI Кошелек, бонусными картами или другим удобным Вам способом.', sure: true, on: true, chars: 397, target: { t: 'Sec', i: 4 } },
  { kind: 'para', label: 'Рекламный абзац', preview: 'Скачано с сайта knigi-free.example — лучшие книги бесплатно', sure: true, on: true, chars: 61, target: { t: 'P', i: 3 } },
  { kind: 'links', label: 'Внешние ссылки: 8 — станут обычным текстом', preview: 'https://www.litres.ru/book/123\nhttps://t.me/books\nhttp://example.com', sure: true, on: true, chars: 8, target: { t: 'Links' } }] : [];

/* ---------------------------------------------------------------- каталог работ и иконка сна */
const works = { wallpapers: [], sleep: [] };
const backups = [{ stamp: '2026-11-02_18-22-40', files: ['taskmgr_lock_background.bmp', 'taskmgr_lock_background_landscape.bmp'] }];

export function on(ev, cb) { (subs[ev] = subs[ev] || []).push(cb); return () => { subs[ev] = subs[ev].filter(x => x !== cb); }; }
export async function pickFolder() { await wait(150); return window.__mockPick || D.root; }

export async function call(cmd, a = {}) {
  (globalThis.__mockCalls ||= []).push(cmd); // для проверок: какие команды вызывались
  await wait(30);
  switch (cmd) {
    case 'app_info': return { version: '0.18.0', models: MODELS, modelsSource: 'встроенный справочник', modelErrors: [], modes: D.info.modes, templates: D.info.templates,
      config: structuredClone(config), root: ROOT, beside: true };
    case 'config_set': config[a.key] = structuredClone(a.value); return null;
    case 'config_reset': config = {}; return null;
    case 'config_delete_all': config = {}; devices = []; return null;
    case 'job_cancel': cancelled[a.id] = true; return null;

    case 'devices_state': return structuredClone(state());
    case 'device_update': { const d = dev(a.id); if (a.name) { d.name = a.name; d.reader.name = a.name; } if (a.paths) d.paths = a.paths; if (a.firmware != null) d.firmware = a.firmware.trim() || null; return null; }
    case 'device_details': { const d = dev(a.id);
      if (d.on) { for (let n = 0; n <= 1500; n += 300) { emit('details://progress', { stage: 'Смотрю файлы на читалке', files: n }); await wait(90); }
        emit('details://progress', { stage: 'Читаю библиотеку', files: 0 }); await wait(150); emit('details://progress', { stage: 'Спрашиваю Windows о диске', files: 0 }); await wait(150);
        const rep = diagInstalled ? { diag: structuredClone(DIAG), diagFile: `D:\\InkMaiden\\${d.id}\\Diagnostics\\diag-2026-11-21_15-04-09.txt`, diagNew: true, diagAutoRemove: true } : {};
        // отчёт забран; модуль остаётся на читалке — убрать его можно только через «На читалку»
        d.details = Object.assign(structuredClone(DETAILS), { disk: DISK, diagInstalled }, rep.diag ? rep : { diag: d.details && d.details.diag, diagFile: d.details && d.details.diagFile }); }
      return d.details || null; }
    case 'device_forget': devices = devices.filter(d => d.id !== a.id); return null;
    case 'devices_forget_all': devices = []; return null;
    case 'books_cover_keep': { const path = `D:\\InkMaiden\\${a.device}\\Covers\\${a.name}.jpg`; kept[path] = a.data; return path; }
    case 'read_image': if (kept[a.path]) return { name: 'c.jpg', mime: 'image/jpeg', data: kept[a.path] }; throw new Error('mock: перетаскивание файлов не проверяется');

    case 'works_save': works[a.kind] = [{ name: a.name, saved: now(), files: a.files.map(f => f.name), thumb: a.thumb ? 'data:image/png;base64,' + a.thumb : null, state: a.project, source: a.source },
      ...works[a.kind].filter(w => w.name !== a.name)]; return { name: a.name, files: a.files.map(f => f.name), dir: json(dev(a.device)).dirs[a.kind] };
    case 'works_list': return works[a.kind].map(w => ({ name: w.name, saved: w.saved, files: w.files, thumb: w.thumb }));
    case 'works_load': { const w = works[a.kind].find(x => x.name === a.name); return { state: w.state, source: w.source, name: w.name }; }
    case 'works_delete': works[a.kind] = works[a.kind].filter(x => x.name !== a.name); return null;
    case 'sleep_unpend': dev(a.device).sleep.pending = null; return null;
    case 'sleep_install': { const d = dev(a.device); if (!d.on) throw new Error('Читалка не подключена.');
      backups.unshift({ stamp: new Date().toISOString().slice(0, 19).replace('T', '_').replace(/:/g, '-'), files: ['taskmgr_lock_background.bmp', 'taskmgr_lock_background_landscape.bmp'] });
      d.sleep = { pending: null, installed: a.name, installed_at: now() };
      return { installed: true, log: ['прежняя иконка → system/resources/Line/old/' + backups[0].stamp, 'записан system/resources/Line/taskmgr_lock_background.bmp', 'записан system/resources/Line/taskmgr_lock_background_landscape.bmp'] }; }
    case 'sleep_plan': if (!dev(a.device).on) throw new Error('Читалка не подключена.'); await wait(150);
      return ['taskmgr_lock_background.bmp', 'taskmgr_lock_background_landscape.bmp'].map((f, i) => ({ src: `D:\\InkMaiden\\${a.device}\\Sleep_icons\\${a.name}\\${f}`, dst: `system/resources/Line/${f}`, size: 2808054 + i * 10, state: 'changed' }));
    case 'sleep_backups': return structuredClone(backups).map((b, i) => Object.assign(b, { thumb: i ? null : PH['743K3'] || null }));
    case 'sleep_restore': { const d = dev(a.device); if (!d.on) throw new Error('Читалка не подключена.'); d.sleep = { pending: null, installed: a.stamp === 'default' ? null : `резервная копия ${a.stamp}`, installed_at: now() }; return []; }

    case 'queue_check': { const d = json(dev(a.device)); if (!d.connected) throw new Error('Читалка не подключена.');
      const files = []; a.items.forEach((it, k) => { const n = /\.[a-z0-9]+$/i.test(it.src) ? 1 : 4;
        for (let i = 0; i < n; i++) { const name = n === 1 ? it.src.split(/[\\/]/).pop() : `${it.src.split(/[\\/]/).pop()}\\том ${i + 1}.cbz`;
          files.push({ item: k, src: n === 1 ? it.src : `${it.src}\\том ${i + 1}.cbz`, dst: `${it.dst.replace(/\\/g, '/').replace(/^\/+|\/+$/g, '')}/${name.split('\\').pop()}`, size: 1.5e6 + ((k * 7 + i * 3) % 9) * 2.1e6, state: i === 1 ? 'changed' : (k + i) % 5 === 4 ? 'same' : 'new' }); } });
      await wait(300); return { files, free: d.seen.free, root: d.connected }; }
    case 'queue_run': {
      cancelled.queue = false; const n = a.files.length, all = a.files.reduce((x, f) => x + (f.size || 2e6), 0); let bytes = 0;
      for (let i = 0; i < n; i++) { if (cancelled.queue) break; bytes += a.files[i].size || 2e6; emit('queue://progress', { done: i, total: n, name: a.files[i].dst, bytes: bytes - (a.files[i].size || 2e6), all, secs: (i + 1) * 0.2 }); await wait(200); }
      a.files.slice(0, cancelled.queue ? 3 : n).forEach(f => onReader.add(String(f.dst).split('/').pop()));
      return { copied: a.files.slice(0, cancelled.queue ? 3 : n).map(f => f.dst), bytes, secs: n * 0.2, errors: [], cancelled: !!cancelled.queue };
    }
    case 'device_eject': { const d = dev(a.id); if (!d.on) throw new Error('Читалка не подключена.'); await wait(500); d.on = false; d.offAt = now(); d.ejected = true; emit('devices://changed'); return null; }
    case 'diag_install': if (!dev(a.id).on) throw new Error('Читалка не подключена.'); diagInstalled = true; return null;
    case 'diag_remove': if (!dev(a.id).on) throw new Error('Читалка не подключена.'); diagInstalled = false; return null;

    case 'books_scan': for (let i = 0; i <= 10; i++) { emit('books://scan', { done: i * 50, total: 500 }); await wait(60); }
      books = (scene === 'big' ? Array.from({ length: 64 }, (_, k) => BOOKS.map(b => [`Полка ${k}/${b[0]}`, ...b.slice(1)])).flat() : BOOKS).map(([rel, format, author, title, series, number, hasCover, genres], index) => ({ index, rel, name: rel.split('/').pop(), format, fmt: format.toLowerCase(),
        size: 180000 + index * 97000, author, title, series, number, hasCover, genres, error: rel.includes('Сломанная') ? 'Не удалось прочитать OPF: файл повреждён' : null }));
      return { root: a.args.path, items: books, skipped: { mobi: 1, pdf: 1 }, total: books.reduce((s, b) => s + b.size, 0) };
    case 'books_cover': { const b = books[a.index]; if (!b.hasCover) return null;
      const c = document.createElement('canvas'); c.width = 300; c.height = 420; const g = c.getContext('2d');
      const grd = g.createLinearGradient(0, 0, 300, 420); grd.addColorStop(0, '#2d4a6b'); grd.addColorStop(1, '#c9a46a'); g.fillStyle = grd; g.fillRect(0, 0, 300, 420);
      g.fillStyle = '#fff'; g.font = '600 30px Georgia'; g.fillText(b.title.slice(0, 14), 20, 330); g.font = '18px Georgia'; g.fillText(b.author, 20, 370);
      return c.toDataURL('image/jpeg', .85); }
    case 'books_parts': await wait(200); return ['Обложка', 'Глава 1. Посещение', 'Глава 2. Зона', 'Глава 3. Рэдрик', 'Реклама полной версии'].map((t, i) => ({ id: i ? `s${i}` : 's0', title: t, chars: 400 + i * 5200 }));
    case 'books_part': await wait(150); return { id: a.id, kind: 'fb2', source: `<section><title><p>Глава</p></title><p>Текст части ${a.id}.</p></section>`,
      html: `<section><h3><p>Глава</p></h3><p>${'Рэдрик шёл через Зону, стараясь не думать ни о чём. '.repeat(30)}</p><p><em>Курсив</em> и <strong>жирный</strong>.</p></section>` };
    case 'books_file_info': { await wait(120); const n = a.path.split(/[\\/]/).pop().replace(/\.[^.]+$/, '');
      const c = document.createElement('canvas'); c.width = 300; c.height = 420; const g = c.getContext('2d'); g.fillStyle = '#8a2b22'; g.fillRect(0, 0, 300, 420); g.fillStyle = '#fff'; g.font = '600 28px Georgia'; g.fillText(n.slice(0, 16), 20, 330);
      return { title: n, author: 'Автор', series: '', number: '', format: 'FB2', size: 100000, cover: c.toDataURL('image/jpeg', .85), parts: true }; }
    case 'books_file_parts': await wait(150); return ['Обложка', 'Глава 1'].map((t, i) => ({ id: 's' + i, title: t, chars: 300 + i * 4000 }));
    case 'books_file_part': await wait(100); return { id: a.id, kind: 'fb2', source: '', html: `<p>Готовая книга, часть ${a.id}. ${'Текст. '.repeat(40)}</p>` };
    case 'backups_list': return { dir: 'C:\\InkMaiden\\PB743K3\\Бэкапы', books: [...bak].map(([rel, mtime]) => ({ rel, size: 1000, mtime })), sleep: 2 };
    case 'backups_restore_book': await wait(60); bak.set(a.rel.replace(/\\/g, '/'), Date.now() / 1000 | 0); return null;
    case 'backups_forget_book': bak.delete(a.rel); return null;
    case 'books_render_part': await wait(80); return a.source.replace(/^\s*<section[^>]*>|<\/section>\s*$/g, '').replace(/<title>/g, '<h3>').replace(/<\/title>/g, '</h3>').replace(/<emphasis>/g, '<em>').replace(/<\/emphasis>/g, '</em>').replace(/<empty-line\/>/g, '<br>');
    case 'books_check_part': { const d = new DOMParser().parseFromString(`<r>${a.source}</r>`, 'application/xml'); if (d.querySelector('parsererror')) throw new Error('Раздел не читается как FB2: ' + d.querySelector('parsererror').textContent.split('\n')[0]); return null; }
    // рабочий стол читалки
    case 'desktop_read': { const d = dev(a.device); if (!d.on && !deskSnap) return { source: 'none' }; deskSnap = true;
      // scene=deskexact — каталог из отчёта модуля известен; scene=desknone — своей настройки нет и каталога нет
      if (scene === 'deskweird') { const v = structuredClone(desk.view); v.view.groups[0].apps.push(42, null); v.applications.PB_Calc = 'странно';
        return { source: 'reader', custom: true, exact: true, view: v, panel: { applications: ['PB_Library', { id: 'SYS_Apps' }, 7] }, files: [...deskFiles], icons: {},
          sys: { known: true, bins: 'не массив', lang: null, view: { view: { groups: 'нет' } },
            db: [{ id: 'PB_Calc', title: { ru: 'Калькулятор' }, path: 5 }, 'PB_Chess', { name: 'PB_Gallery', title: '@Gallery' }, null, 17] } }; }
      const sysKnown = scene === 'deskexact', custom = scene !== 'desknone';
      const sys = sysKnown ? { known: true, at: now() - 3600, view: null, bins: ['calc.app', 'browser.app', 'gallery.app', 'notes.app', 'settings.app', 'dictionary.app', 'rss.app', 'audiobooks.app', 'player.app', 'calendar.app', 'frame.app', 'library.app', 'chess.app', 'painter.app', 'klondike.app', 'scribble.app', 'sudoku.app'],
        db: structuredClone(DB743),
        paths: { '/ebrmain/bin/audio_books.app': true, '/ebrmain/bin/webbrowser.sh': true, '/ebrmain/bin/calc.app': true, '/ebrmain/bin/calendar.app': true, '/ebrmain/bin/chess.app': true, '/ebrmain/applications/pbcloud.app': true, '/ebrmain/bin/Dictionary.app': true, '/ebrmain/bin/dropboxctl.app': true, '/ebrmain/bin/empik_store.app': true, '/ebrmain/bin/mediaexplorer.app': true, '/ebrmain/bin/kosynka.app': true, '/ebrmain/bin/explorer.app': true, '/ebrmain/bin/notes.app': true, '/ebrmain/bin/pbonleihe_library.app': true, '/ebrmain/bin/music_player.app': true, '/ebrmain/bin/pocketnews.app': true, '/ebrmain/bin/scribble.app': true, '/ebrmain/bin/s2pb_wizard.app': true, '/ebrmain/bin/settings.app': true, '/ebrmain/bin/snake.app': false, '/ebrmain/bin/coloring.app': true, '/ebrmain/bin/sudoku.app': true, '/ebrmain/bin/pb_notes_app.app': false, '/ebrmain/bin/digital_frame.app': true, '/ebrmain/bin/libby.app': true },
        flags: { have_pbcloud: '1', use_audio_player: '1', use_calendar: '0' },
        lang: { '@Calc': 'Калькулятор', '@Browser': 'Браузер', '@Gallery': 'Галерея', '@Notes': 'Заметки', '@Settings': 'Настройки', '@Dictionary': 'Словарь', '@RSSNews': 'RSS Новости', '@AudioBooks': 'Аудиокниги',
          '@Player': 'Аудиоплеер', '@Calendar': 'Часы-календарь', '@Digital_Frame': 'Фоторамка', '@Library': 'Библиотека', '@Chess': 'Шахматы', '@Painter': 'Раскраска', '@Klondike': 'Косынка', '@Sudoku': 'Судоку' } } : { known: false };
      return { source: d.on ? 'reader' : 'snapshot', custom, exact: custom || sysKnown, view: structuredClone(desk.view), panel: structuredClone(desk.panel), files: [...deskFiles], icons: {}, sys }; }
    case 'desktop_apply': { if (!dev(a.device).on) throw new Error('Читалка не подключена.'); deskApply(a.plan); deskBk.unshift({ stamp: '2026-11-22_14-10-00', files: ['system/config/desktop/view.json'], factory: false });
      return ['прежние файлы — копия на компьютере (2026-11-22_14-10-00)', `system/config/desktop/view.json: изменений ${a.plan.apps.length}`]; }
    case 'desktop_backups': return deskBk;
    case 'desktop_restore': return ['текущие файлы — копия на компьютере', 'рабочий стол возвращён'];
    case 'apps_list': return { dir: 'D:\\InkMaiden\\Apps', devDir: a.code ? 'D:\\InkMaiden\\Apps\\PB' + a.code : null, apps: structuredClone(appLib) };
    case 'apps_import': { const out = []; for (const p of a.paths) { const f = p.split(/[\\/]/).pop(); if (/\.app$/i.test(f)) { if (!appLib.some(x => x.file === f)) appLib.push({ file: f, path: 'D:\\InkMaiden\\Apps\\' + f, size: 100000, title: null, icon: null }); out.push(f); } } return out; }
    case 'apps_forget': { const i = appLib.findIndex(x => x.path === a.path); if (i >= 0) appLib.splice(i, 1); return 1; }
    case 'desktop_icon_save': return [`D:\\InkMaiden\\${a.device}\\Desktop\\icons\\${a.name}.bmp`, `D:\\InkMaiden\\${a.device}\\Desktop\\icons\\${a.name}_f.bmp`];
    case 'ready_remove': { let n = 0; for (const p of a.paths) { const name = p.split(/[\\/]/).pop();
        const bi = books.findIndex((b, i) => bookOut(i).split('/').pop() === name); if (bi >= 0 && savedIdx.books.delete(bi)) n++; } return n; }
    case 'books_done': return [...savedIdx.books];
    case 'comics_done': { const k = /Manga$/i.test(a.out) ? 'manga' : /Comics$/i.test(a.out) ? 'comics' : ckind; return [...(savedIdx[k] || [])]; }
    case 'ready_list': {
      // что лежит в папке готового: сохранённые книги и тома; onReader — по тому, что записано через «На читалку»
      const mark = list => a.reader ? list.map((f, i) => ({ ...f, onReader: onReader.has(f.rel.split('/').pop()) || (i < 3) ? 'same' : 'none' })) : list;
      return mark((() => {
      const kb = /Books$/i.test(a.dir), km = /Manga$/i.test(a.dir) ? 'manga' : /Comics$/i.test(a.dir) ? 'comics' : null;
      if (kb) return [...savedIdx.books].sort((x, y) => x - y).map(i => ({ rel: bookOut(i), path: a.dir + '\\' + bookOut(i).replace(/\//g, '\\'), size: Math.round(books[i].size * 0.9), mtime: now() - i * 60 }));
      if (km) return [...(savedIdx[km] || [])].sort((x, y) => x - y).map(i => ({ rel: (D.items[i].rel ? D.items[i].rel + '/' : '') + D.items[i].newName, path: a.dir + '\\' + D.items[i].newName, size: Math.round(D.items[i].size * 0.4), mtime: now() - i * 60 }));
      return [];
      })());
    }
    case 'books_process': if (a.args.skip_done && savedIdx.books.has(a.args.index)) return { skipped: true }; if (savedIdx.books.has(a.args.index)) bak.set(bookOut(a.args.index), Date.now() / 1000 | 0); saved.add(a.args.out); savedIdx.books.add(a.args.index); { const b = books[a.args.index], dir = b.rel.includes('/') ? b.rel.slice(0, b.rel.lastIndexOf('/') + 1) : ''; savedRel.set(a.args.index, a.args.name ? dir + a.args.name : b.rel.replace(/\.txt$/i, '.fb2')); } await wait(180); return { path: 'x', rel: bookOut(a.args.index), links: a.args.unlink ? (a.args.index % 3 === 1 ? 4 : 0) : 0, bytes: 1 };

    case 'comics_mode_opt': { const o = structuredClone(D.opts[optKey(a.reader, a.mode)]); o.reader = a.reader; return o; }
    case 'comics_scan': await wait(400); cplans[ckind] = { root: a.args.path, total: D.total, items: D.items }; return cplans[ckind];
    case 'device_claim': { unknown = []; const d = a.id ? dev(a.id) : (devices.push(mk('PB743K3-2', 'InkPad Color 3', '743K3', seenC3, now(), true)), devices[devices.length - 1]); d.on = true; return d.id; }
    case 'device_merge': { const i = devices.findIndex(x => x.id === a.from); const was = devices[i]; if (i >= 0) devices.splice(i, 1); dev(a.into).on = !!(was && was.on); emit('devices://changed'); return { moved: 12, kept: 1 }; }
    case 'device_orphans': return scene === 'orphan' && a.id === 'PB743K3-2' ? [{ id: 'PB743K3', files: 1416 }] : [];
    case 'device_rename': { const d = dev(a.id); d.id = a.to; d.reader.id = a.to; scene = 'none-orphan'; emit('devices://changed'); return { id: a.to, moved: 3, kept: 0 }; }
    case 'open_folder': return null;
    case 'list_images': await wait(300); return Array.from({ length: 7 }, (_, i) => ({ path: `${a.dir}\\фото ${i + 1}.jpg`, name: `фото ${i + 1}.jpg`, rel: `фото ${i + 1}.jpg`, size: 1.2e6 + i * 3e5, thumb: wthumb(i) }));
    case 'copy_files': return a.files.length;
    case 'device_diag_load': return Object.assign({}, dev(a.id).details || {}, { diag: structuredClone(DIAG), diagFile: a.path });
    case 'dir_has_files': return saved.has(a.path) || /Wallpapers|Sleep_icons/.test(a.path);
    case 'comics_use': ckind = a.kind; return cplans[a.kind] || null;
    case 'comics_name_samples': {
      const v = { series: 'Nochnoy gorod', vol: '1', ch: '12', title: 'Ночной патруль', n: '2', folder: 'Манга', name: 'Nochnoy gorod v01 Глава 012 Ночной патруль' };
      const fmt = t => { if (!t) return v.name; let x = t.replace(/\[([^\[\]]*)\]/g, (m, inner) => [...inner.matchAll(/\{(\w+)(?::(\d+))?\}/g)].some(f => !v[f[1]]) ? '' : inner);
        x = x.replace(/\{(\w+)(?::(\d+))?\}/g, (m, k, w) => { const val = v[k] || ''; return w && /^\d+$/.test(val) ? val.padStart(+w, '0') : val; });
        return x.replace(/\s{2,}/g, ' ').trim().replace(/^[-_. ]+|[-_. ]+$/g, '') || v.name; };
      return { orig: 'Nochnoy_gorod_v01_ch12_[scan].cbr', names: a.templates.map(t => fmt(t) + '.cbz') };
    }
    case 'comics_names': return { root: D.root, total: D.total, items: D.items };
    case 'comics_mode_estimates': {
      await wait(500); const i = a.args.index, s = D.items[i].size; const modes = {};
      for (const m of MODE) { const rr = D.ratios[optKey(a.args.reader, m)], r = rr[i % rr.length]; modes[m] = { ratio: r, bytes: Math.round(s * r) }; }
      return { size: s, modes };
    }
    case 'comics_estimate_all': {
      cancelled.estimate = false; let guess = 0;
      for (let i = 0; i < D.items.length; i++) {
        await wait(300); if (cancelled.estimate) return { cancelled: true };
        const b = Math.round(D.items[i].size * ratio(a.opt, i)); guess += b;
        const done = i + 1, all = D.items.slice(0, done).reduce((x, y) => x + y.size, 0);
        emit('comics://estimate', { index: i, bytes: b, done, total: D.items.length, all, guess: Math.round(guess * D.total / all) });
      }
      return { all: D.total, guess, samples: 4, cancelled: false };
    }
    case 'comics_volume_estimate': {
      await wait(300); const i = a.args.index, r = ratio(a.args.opt, i), s = D.items[i].size;
      return { ratio: r, size: s, bytes: Math.round(s * r), folderTotal: D.total, folderGuess: Math.round(D.total * r), count: D.items.length };
    }
    case 'comics_preview': {
      await wait(250); const o = a.args.opt; const p = Math.min(a.args.page, 3);
      const d = structuredClone(D.previews[`${optKey(o.reader, o.mode)}/${p}`]);
      const sims = {}; for (const id of a.args.readers) sims[id] = d.sims[famKey(id)];
      d.sims = sims; return d;
    }
    case 'comics_compare': {
      await wait(400); const p = Math.min(a.args.page, 3), f = famKey(a.args.reader), modes = {};
      let base;
      for (const m of MODE) { const d = D.previews[`${f}/${m}/${p}`]; base = d; modes[m] = Object.assign({}, d.result, { sim: d.sims[f], file: d.sims[f], quality: m === 'max' ? 75 : m === 'safe' ? 92 : 85 }); }
      return { page: base.page, pages: base.pages, kind: base.kind, orig: base.orig, modes };
    }
    case 'comics_run': {
      if (a.args.mode !== 'rename' && a.args.folders) saved.add(a.args.folders.out_dir);
      cancelled.run = false; let ok = 0;
      for (let i = 0; i < D.items.length; i++) {
        if (cancelled.run) break;
        if (a.args.only && !a.args.only.includes(i)) continue;
        const it = D.items[i];
        emit('comics://progress', { index: i, total: D.items.length, name: it.newName, status: '…', ok: true, in_bytes: 0, out_bytes: 0 });
        await wait(500);
        const out = a.args.mode === 'rename' ? 0 : Math.round(it.size * ratio(a.args.opt, i));
        emit('comics://progress', { index: i, total: D.items.length, name: it.newName, status: a.args.mode === 'rename' ? 'переименован' : '12 стр.', ok: true, in_bytes: out ? it.size : 0, out_bytes: out });
        ok++; if (a.args.mode === 'copies') { const k = /Comics$/i.test(a.args.folders.out_dir) ? 'comics' : 'manga'; (savedIdx[k] = savedIdx[k] || new Set()).add(i); }
      }
      return { ok, fail: 0, cancelled: !!cancelled.run, out: a.args.folders.out_dir, backup: D.root + '\\_backup' };
    }
  }
  throw new Error('mock: нет команды ' + cmd);
}
