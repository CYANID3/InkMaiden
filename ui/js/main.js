// Оболочка InkMaiden: разделы слева, текущая читалка, общие данные программы.
// Раздел = модуль с { id, title, hint, icon, mount(main, ctx), unmount?, drop?, hover?, full? }.
// Чтобы добавить раздел: создать js/modules/<имя>.js и вписать его в список ниже.
//
// Текущая читалка одна на все разделы. Подключённая выбирается сама; если не подключено ничего,
// а сохранённые есть — при запуске окно выбора; если читалок нет совсем — экран «Подключите читалку».
// Сменить текущую можно только в «Настройках».
import { call, on, onDrop } from './api.js';
import { $, $$, h, icon, fill, toast, dialog, date, dateTime, span, live, maidy } from './ui.js';
import device from './modules/device.js';
import books from './modules/books.js';
import comics from './modules/comics.js';
import wallpapers from './modules/wallpapers.js';
import sleep from './modules/sleep.js';
import desktop from './modules/desktop.js';
import settings from './modules/settings.js';
import uploads from './modules/uploads.js';
import * as Q from './queue.js';
import { welcome } from './welcome.js';
const OFF_BY_DEFAULT = new Set(['openFolderAfterSave']);

// «Моё устройство» открывается карточкой читалки над меню — отдельного пункта в меню нет
// меню: разделы, где готовят данные, — группой; ниже отдельно «На читалку» — единственный, откуда пишется на читалку
const MODULES = [device, books, comics, wallpapers, sleep, desktop, uploads];
const NAV = MODULES.filter(m => m.id !== 'device' && m.id !== 'uploads');
const FOOT = [settings];
const ALL = [...MODULES, ...FOOT];

// подписчик с ошибкой не должен ломать остальных
const notify = () => ctx.subs.forEach(f => { try { f(ctx.dev); } catch (e) { console.error(e); } });

const ctx = {
  info: null, dev: { devices: [], unknown: [] }, subs: new Set(),
  get config() { return ctx.info.config; },
  async refresh() { ctx.info = await call('app_info'); return ctx.info; },
  async refreshDevices() {
    const had = !!ctx.cur();
    ctx.dev = await call('devices_state');
    claimAsk();
    pickCurrent();
    if (!had && ctx.cur() && current && current.id !== 'settings') { notify(); chip(); show(current.id); return ctx.dev; }
    notify();
    chip(); gate();
    return ctx.dev;
  },
  onDevices(f) { ctx.subs.add(f); return () => ctx.subs.delete(f); },
  /** текущая читалка (сохранённая) или null */
  cur() { return ctx.dev.devices.find(d => d.id === ctx.config.current) || null; },
  device: id => ctx.dev.devices.find(d => d.id === id) || null,
  async setCurrent(id) { await ctx.set('current', id); notify(); chip(); gate(); if (current && current.id !== 'settings') show(current.id); },
  async set(key, value) { ctx.config[key] = value; await call('config_set', { key, value }); },
  // по умолчанию всё включено, кроме перечисленного: папку после сохранения открывать не нужно — программа сама показывает «Готово для читалки»
  behavior: key => { const v = (ctx.config.behavior || {})[key]; return OFF_BY_DEFAULT.has(key) ? v === true : v !== false; },
  /** недавние папки: kind — «source», «books», «comics» … */
  history: kind => ((ctx.config.history || {})[kind] || []),
  async remember(kind, path) {
    if (!path) return;
    const all = Object.assign({}, ctx.config.history);
    all[kind] = [path, ...(all[kind] || []).filter(p => p.toLowerCase() !== path.toLowerCase())].slice(0, 12);
    await ctx.set('history', all);
  },
  async forgetHistory() { await ctx.set('history', {}); },
  /** общая папка-источник из настроек */
  source: () => ((ctx.config.paths || {}).source || ''),
  /** откуда брать исходники вида kind: у текущей читалки → общие по видам → общая папка-источник */
  srcFor: kind => { const d = ctx.cur(), p = ctx.config.paths || {}; return (d && d.paths.src && d.paths.src[kind]) || (p.src || {})[kind] || p.source || ''; },
  /** свои пути под названиями: общие и текущей читалки */
  savedPaths: () => [...((ctx.config.paths || {}).saved || []), ...(((ctx.cur() || {}).paths || {}).saved || []).map(x => ({ ...x, own: true }))],
  show: id => show(id),
  setTheme: t => setTheme(t),
  theme: () => document.documentElement.dataset.theme,
};
window.inkmaidenCtx = ctx; // для встроенных редакторов (обои, иконка сна)

function setTheme(t) {
  try { localStorage.setItem('inkmaiden-theme', t); } catch (e) {}
  document.documentElement.dataset.theme = t === 'auto' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : t;
  notify();
}

/* ---------- читалка без метки, а такая модель уже сохранена: после сброса или форматирования метка пропадает.
   Спросить, это прежняя (папки, готовое и очередь остаются её) или другая такая же — вместо «новой» PB…-2 */
const claiming = new Set();
async function claimAsk() {
  for (const u of ctx.dev.unknown || []) {
    if (!u.candidates || !u.candidates.length || claiming.has(u.seen.root)) continue;
    claiming.add(u.seen.root);
    const one = u.candidates.length === 1 ? u.candidates[0] : null;
    let pick = one ? one.id : u.candidates[0].id;
    const ans = await dialog(close => h('div', { class: 'dlg' }, h('h2', {}, `Подключена ${u.model || u.seen.label}`),
      h('p', { style: { margin: 0 } }, one ? `Это ваша «${one.name}»? На ней нет метки InkMaiden — так бывает после сброса настроек или форматирования памяти. Если это она, всё готовое, папки и список «На читалку» останутся её — заново ничего обрабатывать не нужно.`
        : 'На ней нет метки InkMaiden — так бывает после сброса настроек или форматирования. Если это одна из ваших, выберите её — папки, готовое и список «На читалку» останутся её.'),
      one ? null : h('select', { class: 'input', onchange: e => { pick = e.target.value; } }, u.candidates.map(c => h('option', { value: c.id }, `${c.name} (${c.id})`))),
      h('div', { class: 'dlg-foot' },
        h('button', { class: 'btn', onclick: () => close('new') }, 'Нет, другая читалка'),
        h('button', { class: 'btn primary', onclick: () => close('same') }, one ? `Да, это «${one.name}»` : 'Да, это она'))));
    if (!ans) { claiming.delete(u.seen.root); gate(); continue; } // закрыли окно — спросим снова (кнопка на экране и при подключении)
    try {
      const id = await call('device_claim', { root: u.seen.root, id: ans === 'same' ? pick : null });
      await ctx.setCurrent(id); toast(ans === 'same' ? 'Узнал: прежние папки и готовое на месте' : 'Сохранена как новая читалка');
    } catch (e) { toast(String(e)); claiming.delete(u.seen.root); }
    ctx.refreshDevices();
  }
}

/* ---------- выбор текущей читалки */
let chooserOpen = false, asked = false;
function pickCurrent() {
  // сначала ответ на «это прежняя читалка?» — выбор текущей подождёт
  if ((ctx.dev.unknown || []).some(u => u.candidates && u.candidates.length)) return;
  const devs = ctx.dev.devices, cur = ctx.cur();
  const connected = devs.filter(d => d.connected);
  // текущая подключена или подключённых нет — ничего не меняем (несколько подключённых: остаётся прежняя)
  if (cur && (cur.connected || !connected.length)) return;
  if (connected.length) {
    const next = connected[0];
    const was = cur;
    ctx.set('current', next.id).catch(() => {});
    if (was && asked) toast(`Текущая читалка — ${next.name}`);
    return;
  }
  if (!devs.length) { if (ctx.config.current) ctx.set('current', null).catch(() => {}); return; }
  // ничего не подключено, но читалки есть: один раз за запуск спросить, с какой работать (после первого просмотра дисков)
  if (ctx.dev.scanned && !asked && !chooserOpen) chooser();
}

async function chooser() {
  chooserOpen = true; asked = true;
  const devs = ctx.dev.devices;
  const id = await dialog(close => h('div', { class: 'dlg' },
    h('h2', {}, 'С какой читалкой работаем?'),
    h('p', { class: 'hint', style: { margin: 0 } }, 'Сейчас ни одна не подключена. Выберите сохранённую — все разделы будут готовить файлы под неё. Сменить её можно в «Настройках».'),
    h('div', { class: 'choices' }, devs.map(d => h('button', { onclick: () => close(d.id) },
      h('span', {}, d.photo ? h('img', { src: d.photo, alt: '' }) : h('span', { html: icon('readers') })),
      h('b', {}, d.name), d.id === ctx.config.current ? h('span', { class: 'chip' }, 'в прошлый раз') : h('span'),
      h('small', {}, [d.model && d.model.name, d.lastSeen ? `подключалась ${date(d.lastSeen)}` : null].filter(Boolean).join(', '))))),
    h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close(ctx.config.current || devs[0].id) }, 'Как в прошлый раз'))));
  chooserOpen = false;
  if (id) await ctx.setCurrent(id);
}

/* ---------- текущая читалка в левой колонке */
// «в 22:06» сегодня, иначе «21 нояб.»
const short = t => { const x = new Date(t * 1000), n = new Date(); return x.toDateString() === n.toDateString() ? `в ${x.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })}` : x.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short' }); };
function chip() {
  const el = $('#cur'); if (!el) return;
  const d = ctx.cur();
  if (!d) { fill(el, h('span', { class: 'plug', html: icon('plug') }), h('b', {}, 'Нет читалки'), h('small', {}, ctx.dev.scanned ? 'подключите по USB' : 'ищу…')); return; }
  el.classList.toggle('off', !d.connected); // отключённая — заметно: пунктир, бледное фото
  fill(el, h('span', {}, d.photo ? h('img', { src: d.photo, alt: '' }) : h('span', { html: icon('readers') })),
    h('b', { title: d.name }, d.name),
    // буква диска — только в подсказке: в карточке «на связи 7 мин»
    h('small', { title: d.connected ? `Подключена как диск ${d.connected.replace(/\\$/, '')}${d.since ? ' с ' + dateTime(d.since) : ''}` : d.lastOff ? `Отключена ${dateTime(d.lastOff)}` : '' }, h('i', { class: 'dot' + (d.connected ? ' on' : '') }),
      d.connected ? (d.since ? live(t => `на связи ${span(t)}`, d.since) : 'подключена')
        : h('em', {}, d.ejected ? 'извлечена' : d.lastOff || d.lastSeen ? `отключена ${short(d.lastOff || d.lastSeen)}` : 'не подключена')));
}

/* ---------- экран «Подключите читалку», если читалок нет совсем */
// Мэйди с кабелем USB вместо рисунка; пока диски не просмотрены — выглядывает («Ищу читалку»)
function gate() {
  const main = $('#main'); let g = $('.gate', main.parentNode);
  // «Подключите читалку» — только когда читалка не подключена и сохранённых нет; пока диски не просмотрены — «Ищу читалку»
  const need = !ctx.dev.devices.length && !(current && current.id === 'settings');
  if (!need) { if (g) g.remove(); return; }
  const unknown = ctx.dev.unknown[0];
  if (!ctx.dev.scanned) {
    if (!g) { g = h('div', { class: 'gate' }); main.parentNode.append(g); }
    fill(g, h('div', { class: 'box' }, maidy('peek', 'mz-bob'), h('h2', {}, 'Ищу читалку…'), h('p', {}, 'Смотрю подключённые диски — это пара секунд.')));
    return;
  }
  const text = unknown
    ? (unknown.candidates && unknown.candidates.length ? `На ${unknown.seen.root} ${unknown.model || unknown.seen.label} без метки InkMaiden — возможно, ваша прежняя после сброса. Ответьте, она это или другая, — тогда можно работать.`
      : unknown.known ? `Нашёл ${unknown.model || unknown.seen.label} на ${unknown.seen.root}. Сохранение выключено в «Настройках» — включите его, чтобы начать.`
      : `На ${unknown.seen.root} читалка ${unknown.seen.label}, но её модели нет в справочнике. Пока с ней работать нельзя.`)
    : 'Подключите PocketBook к компьютеру кабелем и выберите на читалке «Связь с ПК». Программа сама узнает модель, сохранит её и откроет все разделы.';
  if (!g) { g = h('div', { class: 'gate' }); main.parentNode.append(g); }
  fill(g, h('div', { class: 'box' }, maidy(unknown ? 'ask' : 'usb', 'mz-bob'), h('h2', {}, unknown ? 'Читалка подключена' : 'Подключите читалку'),
    unknown ? h('p', {}, text) : h('ol', { class: 'mz-steps' }, h('li', {}, h('i', {}, '1'), h('span', {}, 'Соедините PocketBook с компьютером кабелем USB.')),
      h('li', {}, h('i', {}, '2'), h('span', {}, 'На читалке выберите «Связь с ПК».')), h('li', {}, h('i', {}, '3'), h('span', {}, 'Подождите пару секунд — я узнаю модель, запомню её и открою все разделы.'))),
    unknown && unknown.candidates && unknown.candidates.length ? h('button', { class: 'btn primary', onclick: () => claimAsk() }, 'Ответить')
      : h('button', { class: 'btn quiet', onclick: () => show('settings') }, 'Открыть настройки')));
}

/* ---------- разделы */
let current = null;
// страницы не пересоздаются при переходе между разделами: найденные книги, оценки комиксов, открытый редактор обоев
// остаются как были. Заново строятся только «Настройки» и все страницы при смене текущей читалки.
const pages = new Map(); let pagesFor = null;
function dropPages() { for (const [id, el] of pages) { const m = ALL.find(x => x.id === id); try { m && m.unmount && m.unmount(); } catch (e) { console.error(e); } el.remove(); } pages.clear(); }
function show(id) {
  const m = ALL.find(x => x.id === id) || MODULES[0];
  $$('.nav-item, #cur').forEach(b => b.removeAttribute('aria-current'));
  const navEl = m.id === 'device' ? $('#cur') : $(`.nav-item[data-id="${m.id}"]`); if (navEl) navEl.setAttribute('aria-current', 'page');
  const main = $('#main');
  const cur = ctx.cur() ? ctx.cur().id : null;
  if (pagesFor !== cur) { dropPages(); pagesFor = cur; }
  // уходим со страницы: запомнить прокрутку и спрятать
  if (current && pages.has(current.id)) { const el = pages.get(current.id); el.dataset.scroll = main.scrollTop; el.hidden = true; if (current.hide) current.hide(); }
  if (current && current.id === 'settings' && pages.has('settings')) { if (current.unmount) current.unmount(); pages.get('settings').remove(); pages.delete('settings'); }
  main.classList.toggle('full', !!m.full);
  current = m; gate();
  let el = pages.get(m.id);
  if (el) { el.hidden = false; main.scrollTop = +el.dataset.scroll || 0; if (m.show) m.show(); }
  else if (m.id === 'settings' || ctx.cur()) {
    el = h('div', { class: 'page-host' }); main.append(el); pages.set(m.id, el); main.scrollTop = 0;
    m.mount(el, ctx);
  }
  try { localStorage.setItem('inkmaiden-page', m.id); } catch (e) {}
}
ctx.show = show;

async function start() {
  await ctx.refresh();
  $('#ver').textContent = `Версия ${ctx.info.version}`;
  const item = (m, cls = '') => h('button', { class: 'nav-item ' + cls, 'data-id': m.id, title: m.hint ? `${m.title}: ${m.hint}` : null, onclick: () => show(m.id) },
    h('span', { html: icon(m.icon) }), h('b', {}, m.title, m.id === 'uploads' ? h('i', { class: 'badge', id: 'up-badge', hidden: true }) : null), m.hint ? h('small', {}, m.hint) : null);
  $('#nav').append(h('div', { class: 'nav-cap' }, 'Подготовка'), ...NAV.map(m => item(m)), h('div', { class: 'nav-cap' }, 'Запись'), item(uploads, 'nav-apply'));
  FOOT.forEach(m => $('#nav-foot').append(item(m)));
  // карточка читалки ведёт на «Моё устройство»; её настройки — только в «Настройках»
  $('#cur').addEventListener('click', () => show(ctx.cur() ? 'device' : 'settings'));
  let t = ctx.config.theme; if (!t) { try { t = localStorage.getItem('inkmaiden-theme'); } catch (e) {} }
  setTheme(t || 'light');
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => { if (ctx.config.theme === 'auto') setTheme('auto'); });
  await ctx.refreshDevices();
  // сколько ждёт переноса на текущую читалку — значок у «Загрузок»
  const badge = () => { const b = $('#up-badge'); if (!b) return; const n = ctx.cur() ? Q.list(ctx).length : 0; b.hidden = !n; b.textContent = n; };
  Q.onChange(badge); ctx.onDevices(badge); badge();
  onDrop(paths => current && current.drop && current.drop(paths), hot => current && current.hover && current.hover(hot));
  on('devices://changed', () => ctx.refreshDevices().catch(e => toast(String(e))));
  // иконка сна, ждавшая подключения в прежних версиях, — пунктом в «На читалку» (сама больше ничего не ставится)
  const unpend = async () => { const d = ctx.cur(); if (!d || !d.sleep || !d.sleep.pending) return;
    await Q.add(ctx, [{ src: `${d.dirs.sleep}\\${d.sleep.pending}`, dst: 'system/resources', kind: 'sleep', work: d.sleep.pending, label: `Иконка сна «${d.sleep.pending}»` }], { quiet: true });
    await call('sleep_unpend', { device: d.id }).catch(() => {}); d.sleep.pending = null; };
  ctx.onDevices(() => { unpend().catch(() => {}); }); unpend().catch(() => {});
  let page = MODULES[0].id; try { page = localStorage.getItem('inkmaiden-page') || page; } catch (e) {}
  show(page);
  // первый запуск: ни одной папки ещё не выбрано — приветствие Мэйди
  const cfg = ctx.config;
  if (!cfg.welcomed && !(cfg.books || {}).root && !(cfg.comics || {}).root && !((cfg.comics || {}).ws)) ctx.welcome();
}
/** приветствие; «Выбрать папку…» — открыть раздел и сразу окно выбора папки */
ctx.welcome = () => welcome(ctx, id => { show(id); const m = ALL.find(x => x.id === id); setTimeout(() => { if (m && m.pick) m.pick(); }, 400); });
start().catch(e => { document.body.textContent = `Не удалось запустить InkMaiden: ${e}`; });

// правая кнопка мыши: вместо меню браузера («Назад», «Обновить», «Печать») — меню «⋯» той строки, где щёлкнули.
// В полях ввода остаётся обычное меню (вырезать, копировать, вставить).
addEventListener('contextmenu', e => {
  if (e.target.closest('input, textarea, [contenteditable]')) return;
  e.preventDefault();
  const row = e.target.closest('.ready-row, .part-row, .up-item, .dk-row, .wtile');
  const b = row && row.querySelector('.hist > .btn[aria-haspopup]');
  if (b && b.getAttribute('aria-expanded') !== 'true') b.click();
});
// двойной щелчок по строке открывает её — без выделения слов
addEventListener('mousedown', e => { if (e.detail > 1 && !e.target.closest('input, textarea, [contenteditable], .selectable')) e.preventDefault(); });
