// «Рабочий стол»: замена приложения «Рабочий стол» на самой читалке. Что видно в «Приложениях» и на главном экране:
// показать или скрыть встроенные, добавить свои (из applications), названия и значки своих, группы, панель главного экрана.
// Читается с читалки (или последний снимок на компьютере); правки — черновик (config «desktop» по читалкам),
// записываются одним пунктом в «На читалку» (desktop_apply накладывает их на файл, который лежит на читалке в момент записи).
import { call, pickFiles, openPath } from '../api.js';
import { h, fill, icon, toast, dialog, menuButton, plural, dateTime, busy } from '../ui.js';
import * as Q from '../queue.js';
import { rowEl } from '../upload.js';
import { tip } from '../help.js';

let C, root, S, offs = [];

// встроенные приложения прошивки 6.x: название; на каких моделях их нет
const STOCK = {
  PB_Library: 'Библиотека', PB_Notes: 'Заметки', PB_Bookstore: 'Магазин', PB_Browser: 'Браузер', PB_Calc: 'Калькулятор',
  PB_Calendar: 'Часы-календарь', PB_Dictionary: 'Словарь', PB_Gallery: 'Галерея', PB_Digital_Frame: 'Фоторамка', PB_RSSNews: 'RSS Новости',
  PB_Overdrive: 'PocketBook × Libby', PB_Settings: 'Настройки', PB_Dropbox: 'Dropbox', PB_Cloud: 'PocketBook Cloud', PB_SendToPB: 'Книги по email',
  PB_Chess: 'Шахматы', PB_Painter: 'Раскраска', PB_Klondike: 'Косынка', PB_Scribble: 'Scribble', PB_Sudoku: 'Судоку',
  PB_AudioBooks: 'Аудиокниги', PB_Player: 'Аудиоплеер', PB_Snake: 'Змейка', SYS_Apps: 'Приложения',
};
// партнёрские приложения других стран — на наших читалках их нет; Змейки нет ни на Color 3, ни на Verse
// (это запасной список — пока нет точного каталога по проверке читалки)
const ABSENT = new Set(['PB_Empik', 'PB_Onleihe', 'PB_Enotes', 'PB_Marschpat', 'PB_Bookstore2empikgo', 'PB_Snake']);
const PARTNER = /empik|onleihe|enotes|marschpat|legimi|storytel|saxo|bookbeat|nextory/i;
// без звука (Verse) — нет аудио
const NO_SOUND = new Set(['PB_AudioBooks', 'PB_Player', 'PB_Snake']);
// не советуем прятать: без них читалкой не пользоваться
const KEEP = new Set(['PB_Library', 'PB_Settings']);
const GROUP = { '@General': 'Общие', '@Services': 'Сервисы', '@Games': 'Игры' };
// вкладка в программе (не группа на читалке): все сторонние приложения вместе + папка Apps на компьютере
const OWN = '#own';
const gName = t => GROUP[t] || String(t || '').replace(/^@/, '');

/** каталог встроенных из отчёта модуля (apps_db.json прошивки): ключ → { path, title } — формат бывает разный */
function dbEntries(db) {
  const m = new Map(); if (!db) return m;
  const src = (db && typeof db === 'object') ? (db.applications || db.apps || db) : null;
  if (Array.isArray(src)) src.forEach(x => { const k = x && typeof x === 'object' ? (x.id || x.key || x.name) : (typeof x === 'string' ? x : null); if (k) m.set(String(k), x && typeof x === 'object' ? x : {}); });
  else if (src && typeof src === 'object') for (const [k, x] of Object.entries(src)) if (!String(k).startsWith('_') && x && typeof x === 'object' && !Array.isArray(x)) m.set(String(k), x);
  return m;
}
/* Значения в заводском каталоге (apps_db.json) бывают условными: { "partner": { "all": "1", "vivlio": "0" } },
   с вложением по device / partner / has_audio / has_cloud / localization / language / globalcfg — как описано в его _comment.
   Выбираем ветку по этой читалке; неизвестное условие — ветка "all" (как у прошивки по умолчанию). */
const DIMS = ['device', 'partner', 'has_audio', 'has_cloud', 'localization', 'language', 'globalcfg'];
function pick(v, cx, depth = 0) {
  if (!v || typeof v !== 'object' || Array.isArray(v) || depth > 16) return v;
  const d = DIMS.find(x => x in v); if (!d) return v;            // объект-путь { "path": … } и т.п.
  const br = v[d]; if (!br || typeof br !== 'object') return undefined;
  if (d === 'globalcfg') {
    const key = Object.keys(br)[0], m = (key && br[key] && typeof br[key] === 'object') ? br[key] : {};
    const val = cx.globalcfg && cx.globalcfg[key];
    return pick(val != null && String(val) in m ? m[String(val)] : ('default' in m ? m.default : m.all), cx, depth + 1);
  }
  for (const x of cx[d] || []) if (x !== '_comment' && x !== 'all' && Object.prototype.hasOwnProperty.call(br, x)) return pick(br[x], cx, depth + 1);
  return pick(br.all, cx, depth + 1);
}
function context(sys) {
  const d = C.cur() || {}, r = d.reader || {}, det = d.details || {}, flags = (sys.flags && typeof sys.flags === 'object') ? sys.flags : {};
  const code = String(d.code || r.code || '').trim(), num = (code.match(/\d+/) || [''])[0];
  const partner = (S.data && S.data.panel && typeof S.data.panel.partner === 'string' && S.data.panel.partner) || (typeof flags.partner === 'string' ? flags.partner : '');
  const lang = String(det.language || '').toLowerCase().slice(0, 2);
  const cloud = flags.have_pbcloud === '1' ? 'true' : flags.have_pbcloud === '0' ? 'false' : null;
  return {
    device: [code, num, 'touch'].filter(Boolean),                 // обе читалки сенсорные — не "notouch"
    partner: partner ? [partner.toLowerCase()] : [],
    has_audio: [hasSound() ? 'true' : 'false'],
    has_cloud: cloud ? [cloud] : [],
    language: lang ? [lang] : [],
    localization: [],
    globalcfg: det.globalcfg && typeof det.globalcfg === 'object' ? det.globalcfg : null,
  };
}
/** точно ли известен набор встроенных (по отчёту модуля диагностики) и какие есть на этой читалке */
function catalog() {
  const sys = (S.data && S.data.sys) || {};
  if (!sys.known) return null;
  const paths = sys.paths && typeof sys.paths === 'object' && Object.keys(sys.paths).length ? sys.paths : null;
  const db = dbEntries(sys.db), bins = new Set((Array.isArray(sys.bins) ? sys.bins : []).map(b => String(b).toLowerCase()));
  const groupsOf = v => (v && v.view && Array.isArray(v.view.groups)) ? v.view.groups : [];
  const inSysView = new Set(groupsOf(sys.view).flatMap(g => Array.isArray(g && g.apps) ? g.apps.map(String) : []));
  const cx = context(sys);
  const has = k => {
    if (k === 'SYS_Apps') return true;
    const e = db.get(k);
    if (e) {
      const vis = pick(e.visible, cx);
      if (vis === '0' || vis === 0 || vis === false) return false;
      const pp = pick(e.path, cx), p = typeof pp === 'string' ? pp : '';
      // модуль проверил путь на самой читалке — это точнее всего
      if (p && paths && p in paths) return !!paths[p];
      return !p || !bins.size || !p.startsWith('/ebrmain/bin/') || bins.has(p.split('/').pop().toLowerCase());
    }
    return !db.size && inSysView.has(k);
  };
  const title = k => { const e = db.get(k); const t = e && pick(e.title, cx); if (typeof t !== 'string' || !t) return null;
    const lang = sys.lang && typeof sys.lang === 'object' ? sys.lang : {}; return t.startsWith('@') ? (lang[t] || STOCK[k] || t.slice(1)) : t; };
  return { has, title, at: sys.at };
}
const blankDraft = () => ({ ops: {}, panel: null });
const draftAll = () => Object.assign({}, C.config.desktop);
const saveDraft = () => { const all = draftAll(); all[C.cur().id] = S.draft; C.set('desktop', all).catch(() => {}); };

export default {
  id: 'desktop', title: 'Рабочий стол', hint: 'приложения на читалке', icon: 'grid',
  async mount(main, ctx) {
    C = ctx; root = h('div', { class: 'page' }); main.append(root);
    S = { data: null, loading: true, group: '@General', draft: structuredClone(draftAll()[ctx.cur().id] || blankDraft()) };
    offs.push(ctx.onDevices(() => { if (root.isConnected) load(); }), Q.onChange(() => { if (root.isConnected) render(); }));
    render(); load();
  },
  show() { load(); },
  unmount() { offs.forEach(f => f && f()); offs = []; },
};

/* ================================================================ данные */
async function load() {
  if (S) S.lib = null;
  S.loading = true; render();
  try {
    S.data = await Promise.race([call('desktop_read', { device: C.cur().id }),
      new Promise((_, rej) => setTimeout(() => rej(new Error('Читалка долго не отвечает — нажмите «Обновить» ещё раз или переподключите её.')), 20000))]);
    S.err = null;
  } catch (e) { S.err = String(e.message || e); if (!S.data) S.data = { source: 'none' }; }
  S.loading = false;
  try { prune(); } catch (e) { console.error(e); }
  render();
}
/** файл с читалки, приведённый к ожидаемому виду (на случай необычного формата у прошивки) */
function base() {
  const v = (S.data && S.data.view && typeof S.data.view === 'object') ? S.data.view : {};
  const apps = v.applications && typeof v.applications === 'object' && !Array.isArray(v.applications) ? v.applications : {};
  const groups = (v.view && Array.isArray(v.view.groups) ? v.view.groups : []).filter(g => g && typeof g === 'object')
    .map(g => ({ ...g, title: typeof g.title === 'string' ? g.title : String(g.title || ''), apps: Array.isArray(g.apps) ? g.apps.filter(k => typeof k === 'string') : [] }));
  return { ...v, applications: apps, view: { ...(v.view || {}), groups } };
}
const hasSound = () => { const r = C.cur().reader || {}; return !/629|Verse/i.test(`${r.model || ''} ${r.name || ''} ${C.cur().id}`); };
const iconSize = () => ((C.cur().reader || {}).screen || {}).width >= 1200 ? 116 : 64;

/** то, что получится: файл с читалки + черновик (зеркало pb_core::desktop::apply_plan) */
function effective() {
  const v = structuredClone(base());
  v.applications = v.applications || {}; v.view = v.view || {}; v.view.groups = v.view.groups || [];
  for (const [k, op] of Object.entries(S.draft.ops)) {
    if (op.uninstall) continue; // показываем до записи — с пометкой «будет удалено»
    if (op.group || op.remove) v.view.groups.forEach(g => { g.apps = (g.apps || []).filter(x => x !== k); });
    if (op.group && !op.remove) { let g = v.view.groups.find(g => g.title === op.group); if (!g) v.view.groups.push(g = { title: op.group, sort: 'title', apps: [] }); g.apps.push(k); }
    if (op.remove) { delete v.applications[k]; continue; }
    const e = v.applications[k] = Object.assign({}, v.applications[k]);
    if (op.path) e.path = op.path;
    if (op.title != null) e.title = op.title;
    if (op.icon) { e.icon = { path: op.icon }; e.focused_icon = { path: op.focused }; }
    if (op.hidden === true) e.visible = '0'; else if (op.hidden === false) delete e.visible;
  }
  return v;
}
/** список для показа: { key, title, own, hidden, group, file, iconUrl, missing } */
function apps() {
  const cat = catalog();
  const v = effective(), out = [], seen = new Set(), files = new Set((S.data.files || []).map(f => f.toLowerCase()));
  const add = (k, g) => {
    if (seen.has(k) || k.startsWith('_')) return;
    // встроенные: точно по каталогу читалки, если он известен; иначе — по известным спискам моделей
    // партнёрские приложения других стран — никогда (их «видимость» в каталоге не значит, что они есть на модели)
    if (!k.startsWith('U_') && !cat && (ABSENT.has(k) || PARTNER.test(k))) return;
    if (!k.startsWith('U_')) { if (cat ? !cat.has(k) : (!hasSound() && NO_SOUND.has(k))) return; }
    seen.add(k);
    const e = (v.applications[k] && typeof v.applications[k] === 'object') ? v.applications[k] : {}, own = k.startsWith('U_');
    const file = own && typeof e.path === 'string' ? e.path.split('/').pop() : null;
    const op = S.draft.ops[k] || {};
    // своё приложение, которого на читалке уже нет, не показываем (запись уберётся при следующей записи)
    if (own && (!file || !files.has(file.toLowerCase())) && !op.path) return;
    out.push({ key: k, own, group: g, file, title: (typeof e.title === 'string' && e.title) || (cat && cat.title(k)) || STOCK[k] || (file ? file.replace(/\.app$/i, '') : k.replace(/^U_/, '')),
      hidden: e.visible === '0', uninstall: !!op.uninstall,
      iconUrl: op.iconUrl || (e.icon && typeof e.icon === 'object' && S.data.icons && S.data.icons[e.icon.path]) || null, changed: !!S.draft.ops[k] });
  };
  for (const g of v.view.groups) for (const k of g.apps || []) add(k, g.title);
  // записи без группы (бывают у своих приложений) — в «Общие»
  for (const k of Object.keys(v.applications)) if (k.startsWith('U_') && v.applications[k] && typeof v.applications[k] === 'object') add(k, '@General');
  return out;
}
/** свои приложения на читалке, которых нет на рабочем столе */
function found() {
  const v = effective(), have = new Set(Object.values(v.applications).map(e => String((e && typeof e === 'object' && e.path) || '').split('/').pop().toLowerCase()));
  return (S.data.files || []).filter(f => !have.has(f.toLowerCase()) && !/^inkmaiden_diag\.app$/i.test(f));
}
const keyFor = f => 'U_' + f.replace(/\.app$/i, '').replace(/[^A-Za-z0-9_]/g, '_');
const groupsList = () => { const t = effective().view.groups.map(g => g.title); for (const k of Object.keys(GROUP)) if (!t.includes(k)) t.push(k); return t; };
const panelKeys = () => S.draft.panel || ((S.data.panel && Array.isArray(S.data.panel.applications)) ? S.data.panel.applications : []).map(x => x && typeof x === 'object' ? x.id : x).filter(x => typeof x === 'string');

/* ---------------------------------------------------------------- черновик */
function setOp(key, patch) { S.draft.ops[key] = Object.assign({}, S.draft.ops[key], patch); prune(); saveDraft(); render(); }
/** убрать из черновика то, что на читалке уже так и есть (например, после записи) */
function prune() {
  if (!S.data || !S.data.view) return;
  const b = base(), gOf = k => (b.view.groups || []).find(g => (g.apps || []).includes(k));
  for (const [k, op] of Object.entries(S.draft.ops)) {
    const e = b.applications[k];
    const file = op.path ? String(op.path).split('/').pop().toLowerCase() : null;
    const ok = op.uninstall ? !e && !gOf(k) && !(S.data.files || []).some(f => f.toLowerCase() === file)
      : op.remove ? !e && !gOf(k)
      : (op.hidden == null || (op.hidden === ((e || {}).visible === '0'))) && (op.title == null || (e && e.title === op.title))
        && (!op.icon || (e && e.icon && e.icon.path === op.icon && !op.iconFile)) && (!op.path || (e && e.path === op.path))
        && (!op.group || (gOf(k) && gOf(k).title === op.group))
        && (!op.install || (S.data.files || []).some(f => f.toLowerCase() === file));
    if (ok) delete S.draft.ops[k];
  }
  if (S.draft.panel && S.data.panel && Array.isArray(S.data.panel.applications) && JSON.stringify(S.draft.panel) === JSON.stringify(S.data.panel.applications.map(x => x && x.id))) S.draft.panel = null;
}
const changes = () => Object.keys(S.draft.ops).length + (S.draft.panel ? 1 : 0);

/* ================================================================ вид */
// ошибка при разборе того, что прочитано с читалки, — не «вечное чтение», а понятное сообщение
function render() {
  if (!root) return;
  try { draw(); } catch (e) {
    console.error(e);
    fill(root, head(), h('section', { class: 'panel' }, h('h2', {}, 'Не получилось показать рабочий стол'),
      h('p', { class: 'hint' }, 'Файлы рабочего стола на читалке в непривычном виде. Ничего не записывалось. Пришлите, пожалуйста, текст ниже — поправлю.'),
      h('pre', { class: 'dk-err' }, String(e && e.stack || e)),
      h('button', { class: 'btn', onclick: load }, h('span', { html: icon('refresh') }), 'Обновить')));
  }
}
function draw() {
  if (S.err) return fill(root, head(), h('section', { class: 'panel' }, h('h2', {}, 'Не получилось прочитать рабочий стол'),
    h('pre', { class: 'dk-err' }, S.err), h('button', { class: 'btn', onclick: load }, h('span', { html: icon('refresh') }), 'Обновить')));
  const d = C.cur();
  if (S.loading && !S.data) return fill(root, head(), h('section', { class: 'panel' }, h('p', { class: 'hint', style: { margin: 0 } }, 'Читаю рабочий стол…')));
  if (!S.data || S.data.source === 'none') return fill(root, head(),
    h('section', { class: 'drop' }, h('span', { html: icon('grid') }), h('h2', {}, `Подключите «${d.name}»`),
      h('p', {}, 'Программа прочитает, какие приложения сейчас на рабочем столе. Потом их можно менять и без читалки — по последнему снимку.')),
    actions());
  const list = apps(), groups = [...groupsList(), OWN], cnt = g => g === OWN ? list.filter(a => a.own).length : list.filter(a => !a.own && a.group === g).length;
  const inG = S.group === OWN ? list.filter(a => a.own) : list.filter(a => !a.own && a.group === S.group);
  const nf = found();
  fill(root, head(),
    h('section', { class: 'panel src' },
      h('div', { class: 'src-row' },
        h('span', { class: 'src-ic', html: icon('readers') }),
        h('div', { class: 'path' }, h('b', {}, d.name),
          h('small', {}, S.data.source === 'reader' ? 'подключена' : `не подключена — показано, как было${d.seen && d.seen.at ? ` ${dateTime(d.seen.at)}` : ''}`)),
        h('div', { class: 'src-ctl' },
          h('button', { class: 'btn', title: 'Прочитать рабочий стол заново', onclick: load }, h('span', { html: icon('refresh') }), 'Обновить'),
          menuButton('Вернуть', backupsMenu, { title: 'Вернуть прежний рабочий стол или заводской' })))),
    !catalog() ? h('div', { class: 'panel fresh-bar dk-exact' }, h('span', { html: icon('info') }),
      h('div', {}, h('b', {}, 'Список может быть неточным'),
        h('small', {}, 'Нажмите «Узнать точно» — программа спросит у самой читалки.')),
      h('span', { class: 'grow' }), h('button', { class: 'btn primary', onclick: exactList }, 'Узнать точно')) : null,
    nf.length ? h('div', { class: 'panel fresh-bar dk-found' }, h('span', { html: icon('plus') }),
      h('div', {}, h('b', {}, `${nf.slice(0, 3).join(', ')}${nf.length > 3 ? ' …' : ''} — ${nf.length > 1 ? 'есть на читалке, но их нет на рабочем столе' : 'есть на читалке, но его нет на рабочем столе'}`)),
      h('span', { class: 'grow' }), h('button', { class: 'btn primary', onclick: () => addFound(nf) }, nf.length > 1 ? 'Добавить все' : 'Добавить')) : null,
    busy(h('div', { class: 'split' },
      h('section', { class: 'panel list-panel' },
        h('div', { class: 'list-head' },
          h('div', { class: 'vtabs', role: 'tablist', 'aria-label': 'Группа' }, groups.map(g => h('button', { class: 'vtab', role: 'tab', 'aria-selected': String(S.group === g), onclick: () => { S.group = g; render(); } },
            g === OWN ? 'Сторонние' : gName(g), h('span', { class: 'count' }, String(cnt(g))))), tip('desktop')),
          h('span', { class: 'grow' }), h('small', {}, `скрыто: ${list.filter(a => a.hidden).length}`)),
        h('div', { class: 'blist dk-list' }, S.group === OWN ? secHead('На читалке', inG.length, 'Ваши приложения в папке applications на читалке. Переключатель — видно ли его в «Приложениях».') : null, inG.length ? inG.map(row) : h('p', { class: 'empty-note', style: { padding: '20px' } }, S.group === OWN ? 'На читалке пока нет сторонних приложений.' : 'В этой группе пусто.'),
          S.group === OWN ? libBox() : null)),
      h('div', { class: 'stack side' }, panelBox(list))), S.loading ? 'Читаю…' : null),
    actions());
}
const head = () => h('header', { class: 'page-head' }, h('div', {}, h('h1', {}, 'Рабочий стол'),
  h('p', {}, 'Какие приложения видны на читалке.')));

function tile(a) {
  return a.iconUrl ? h('img', { class: 'dk-ic', src: a.iconUrl, alt: '' }) : h('span', { class: 'dk-ic letter' + (a.own ? ' own' : '') }, (a.title || '?').trim().charAt(0).toUpperCase());
}
function row(a) {
  const keep = KEEP.has(a.key);
  if (a.uninstall) return h('div', { class: 'item dk-row gone' }, tile(a),
    h('div', { class: 'dk-name' }, h('b', {}, h('s', {}, a.title), h('span', { class: 'chip warn' }, 'удалится')), h('small', {}, `${a.file} — будет удалено с читалки, копия останется на компьютере`)),
    h('span'), h('button', { class: 'btn quiet', onclick: () => { delete S.draft.ops[a.key]; saveDraft(); render(); } }, 'Не удалять'));
  const menu = [
    a.own ? { label: 'Переименовать…', onClick: () => rename(a) } : null,
    a.own ? { label: 'Значок…', hint: 'картинка с компьютера — сделается BMP нужного размера', onClick: () => pickIcon(a) } : null,
    { sep: 'Группа' },
    ...groupsList().filter(g => g !== a.group).map(g => ({ label: `В «${gName(g)}»`, onClick: () => setOp(a.key, { group: g }) })),
    a.own && a.file ? { sep: '' } : null,
    a.own && a.file ? { label: 'Удалить с читалки…', hint: 'файл приложения и значки; копия — на компьютер', onClick: () => uninstall(a) } : null,
    S.draft.ops[a.key] ? { sep: '' } : null,
    S.draft.ops[a.key] ? { label: 'Отменить изменения', onClick: () => { delete S.draft.ops[a.key]; saveDraft(); render(); } } : null,
  ].filter(Boolean);
  // та же строка, что в списках книг и комиксов (rowEl): значок, название, метки справа, «⋯»
  return rowEl({ cls: 'dk-row' + (a.hidden ? ' off' : ''), cover: tile(a), title: a.title, sub: a.own && a.file ? a.file : null,
    tags: [a.changed ? h('span', { class: 'chip new' }, 'изменено') : null,
      keep
        // без Библиотеки и Настроек читалкой не пользоваться: переключателя нет совсем
        ? h('span', { class: 'dk-vis dk-lock', title: 'Без этого приложения читалкой не пользоваться — его не скрыть' }, 'всегда')
        : h('label', { class: 'check dk-vis', title: a.hidden ? 'Скрыто — нажмите, чтобы показать' : 'Видно — нажмите, чтобы скрыть', onclick: e => e.stopPropagation() },
          h('input', { type: 'checkbox', checked: !a.hidden, onchange: e => setOp(a.key, { hidden: !e.target.checked }) }), a.hidden ? 'скрыто' : '')],
    menu });
}

/* ---------------------------------------------------------------- панель главного экрана */
function panelBox(list) {
  const keys = panelKeys(), byKey = Object.fromEntries(list.map(a => [a.key, a]));
  const name = k => (byKey[k] && byKey[k].title) || STOCK[k] || k.replace(/^U_/, '');
  const set = arr => { S.draft.panel = arr; prune(); saveDraft(); render(); };
  const can = list.filter(a => !a.hidden && !keys.includes(a.key));
  return h('section', { class: 'panel dk-panel' },
    h('div', { class: 'dk-ph' }, h('h2', {}, 'Главный экран'), tip('panel')),
    !keys.length ? h('p', { class: 'hint', style: { margin: 0 } }, 'Как на читалке по умолчанию. Можно добавить свои значки.') : null,
    
    h('ol', { class: 'dk-pl' }, keys.map((k, i) => h('li', { class: (S.draft.ops[k] || {}).uninstall ? 'gone' : null },
      h('span', { class: 'n' }, String(i + 1)), h('b', {}, name(k)), (S.draft.ops[k] || {}).uninstall ? h('small', {}, 'удалится') : null,
      h('span', { class: 'grow' }),
      h('button', { class: 'btn quiet icon-only', title: 'Левее', disabled: i === 0, onclick: () => { const a = [...keys]; [a[i - 1], a[i]] = [a[i], a[i - 1]]; set(a); } }, '↑'),
      h('button', { class: 'btn quiet icon-only', title: 'Правее', disabled: i === keys.length - 1, onclick: () => { const a = [...keys]; [a[i + 1], a[i]] = [a[i], a[i + 1]]; set(a); } }, '↓'),
      h('button', { class: 'btn quiet icon-only', title: k === 'SYS_Apps' ? '«Приложения» убирать нельзя — иначе до них не добраться' : 'Убрать с главного экрана', disabled: k === 'SYS_Apps', onclick: () => set(keys.filter(x => x !== k)) }, '×')))),
    keys.length >= 5 ? h('p', { class: 'hint', style: { color: 'var(--warn, var(--olive-strong))' } }, 'Больше 4–5 значков может не поместиться.') : null,
    can.length ? menuButton('Добавить', can.map(a => ({ label: a.title, hint: gName(a.group), onClick: () => set([...keys.filter(k => k !== 'SYS_Apps'), a.key, ...(keys.includes('SYS_Apps') ? ['SYS_Apps'] : [])]) })), { icon: 'plus' }) : null);
}
/** что поменяется — строками (подсказка у «Изменений: N») */
function changeLines() {
  const list = apps(), byKey = Object.fromEntries(list.map(a => [a.key, a]));
  const lines = Object.entries(S.draft.ops).map(([k, op]) => {
    const t = (byKey[k] && byKey[k].title) || op.title || STOCK[k] || k;
    return [op.uninstall ? 'удалить с читалки' : null, op.remove ? 'убрать' : null, op.install ? 'установить' : op.path && !op.remove && !op.uninstall ? 'добавить' : null, op.hidden === true ? 'скрыть' : op.hidden === false ? 'показать' : null,
      op.title != null && !op.path && !op.uninstall ? `название «${op.title}»` : null, op.iconFile ? 'новый значок' : null, op.group && !op.path ? `в «${gName(op.group)}»` : null]
      .filter(Boolean).map((x, i) => i ? x : `${t}: ${x}`).join(', ');
  });
  if (S.draft.panel) lines.push('главный экран: новый порядок');
  return lines;
}

/* ---------------------------------------------------------------- действия */
function actions() {
  const n = S && S.data ? changes() : 0, q = Q.list(C).find(x => x.kind === 'desktop' || x.kind === 'desktop-restore');
  return h('div', { class: 'actions' },
    h('div', { class: 'where' }, q ? `Ждёт в «На читалку»: ${q.label}` : n ? h('span', { title: changeLines().join('\n') }, `Изменений: ${n}`) : 'Изменений нет',
      n && !q ? h('button', { class: 'btn quiet', style: { marginLeft: '8px' }, onclick: () => { S.draft = blankDraft(); saveDraft(); render(); } }, 'Отменить') : null),
    Q.applyButton(C, n ? toQueue : null, { title: n ? 'Добавить изменения рабочего стола в «На читалку»' : 'Открыть «На читалку»' }));
}
// точный список: модуль диагностики на читалку → запустить на ней → подключить; отчёт и каталог заберутся сами
function exactList() {
  return Q.addAndGo(C, [{ kind: 'diag', src: '', dst: 'applications', label: 'Проверка читалки (InkMaiden_Diag)' }]);
}
async function toQueue() {
  // своей настройки на читалке нет, а заводская неизвестна — по догадке не пишем
  if (!S.data.exact) {
    const go = await dialog(close => h('div', { class: 'dlg' }, h('h2', {}, 'Сначала нужен точный список'),
      h('p', { style: { margin: 0 } }, 'На читалке нет своей настройки рабочего стола, а заводскую по USB не видно. Если записать наугад, можно спрятать или потерять приложения. Проверка узнает точный список у самой читалки:'),
      h('ol', { class: 'steps' }, h('li', {}, '«Узнать точно» — проверка добавится в «На читалку», запишите её.'), h('li', {}, 'На читалке: «Приложения» → «@InkMaiden_Diag».'), h('li', {}, 'Подключите читалку — список обновится сам, изменения сохранятся.')),
      h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close(false) }, 'Позже'), h('button', { class: 'btn primary', onclick: () => close(true) }, 'Узнать точно'))));
    if (go) await Q.add(C, [{ kind: 'diag', src: '', dst: 'applications', label: 'Проверка читалки (InkMaiden_Diag)' }], { quiet: true });
    return go ? undefined : false;
  }
  const ops = Object.entries(S.draft.ops).map(([key, op]) => ({ key, path: op.path || null, title: op.uninstall ? null : op.title ?? null, icon: op.uninstall ? null : op.icon || null,
    focused: op.uninstall ? null : op.focused || null, hidden: op.uninstall ? null : KEEP.has(key) ? (op.hidden === false ? false : null) : op.hidden ?? null, group: op.uninstall ? null : op.group || null, remove: !!op.remove, uninstall: !!op.uninstall }));
  // Библиотеку и Настройки, если их когда-то скрыли, при записи возвращаем
  { const apps = effective().applications || {};
    for (const k of KEEP) if (apps[k] && apps[k].visible === '0' && !ops.some(o => o.key === k))
      ops.push({ key: k, path: null, title: null, icon: null, focused: null, hidden: false, group: null, remove: false, uninstall: false }); }
  // записи своих приложений, файлов которых на читалке нет, — убрать заодно
  const files = new Set((S.data.files || []).map(f => f.toLowerCase())), b = base();
  for (const [k, e] of Object.entries(b.applications || {})) {
    const f = k.startsWith('U_') && e && e.path ? String(e.path).split('/').pop().toLowerCase() : null;
    if (f && !files.has(f) && !S.draft.ops[k]) ops.push({ key: k, remove: true });
  }
  // удалённые — и с главного экрана
  const gone = new Set(Object.entries(S.draft.ops).filter(([, op]) => op.uninstall).map(([k]) => k));
  const pk = panelKeys(), panel = pk.some(k => gone.has(k)) ? pk.filter(k => !gone.has(k)) : S.draft.panel;
  const icons = Object.values(S.draft.ops).filter(op => op.iconFile && !op.uninstall).flatMap(op => op.iconFile.map(src => ({ src, dst: 'applications/icons', kind: 'other', label: `Значок ${src.split(/[\\/]/).pop()}` })));
  const inst = Object.values(S.draft.ops).filter(op => op.install && !op.uninstall).map(op => ({ src: op.install, dst: 'applications', kind: 'other', label: `Приложение ${op.install.split(/[\\/]/).pop()}` }));
  const n = changes();
  await Q.add(C, [...inst, ...icons, { kind: 'desktop', src: '', dst: 'system/config', plan: { apps: ops, panel }, label: `Рабочий стол: ${n} ${plural(n, 'изменение', 'изменения', 'изменений')}` }], { quiet: true });
}
function addFound(files) {
  for (const f of files) {
    const k = keyFor(f);
    S.draft.ops[k] = Object.assign({}, S.draft.ops[k], { path: `/mnt/ext1/applications/${f}`, title: f.replace(/\.app$/i, ''), group: S.group || '@General' });
  }
  saveDraft(); render(); toast(`Добавлено на рабочий стол: ${files.length}. Названия и значки — в меню «⋯» у приложения.`);
}
/* ---------------------------------------------------------------- папка сторонних приложений на компьютере */
async function libLoad() {
  try { S.lib = await call('apps_list', { code: devCode(), dir: ownDir() }); } catch (e) { S.lib = { dir: '', apps: [], err: String(e.message || e) }; }
  if (root && root.isConnected) render();
}
// своя папка приложений (Настройки → Пути → «Приложения»), иначе Apps рядом с программой
const ownDir = () => { const d = C.cur() || {}, g = (C.config.paths || {}).src || {}; return ((d.paths && d.paths.src) || {}).apps || g.apps || null; };
const devCode = () => { const d = C.cur() || {}; return String(d.code || (d.seen && d.seen.code) || '').trim() || null; };
const onReader = f => (S.data.files || []).some(x => x.toLowerCase() === f.toLowerCase());
// заголовок части списка: название, сколько, пояснение, справа — действия (одинаково для обеих частей «Сторонних»)
function secHead(title, n, hint, actions = []) {
  return h('div', { class: 'dk-sec' }, h('div', { class: 'dk-sec-t' }, h('b', {}, title), n != null ? h('span', { class: 'count' }, String(n)) : null, hint ? h('small', {}, hint) : null),
    h('span', { class: 'grow' }), ...actions.filter(Boolean));
}
function libBox() {
  const open = p => openPath(p).catch(e => toast(String(e.message || e)));
  if (!S.lib) { if (!S.libBusy) { S.libBusy = true; libLoad().finally(() => { S.libBusy = false; }); } return h('div', { class: 'dk-lib' }, secHead('Можно установить', null, 'Читаю папку с приложениями…')); }
  const L = [...(S.lib.apps || [])].sort((a, b) => onReader(a.file) - onReader(b.file));
  const folder = S.lib.dir.split(/[\\/]/).filter(Boolean).pop();
  const add = h('button', { class: 'btn', title: 'Выбрать файл .app или zip-архив с приложением', onclick: libAdd }, h('span', { html: icon('plus') }), 'Добавить приложение…');
  const more = menuButton('', [
    { label: S.lib.own ? 'Открыть папку с приложениями' : 'Открыть папку для всех читалок', hint: S.lib.dir, onClick: () => open(S.lib.dir) },
    !S.lib.own && S.lib.devDir ? { label: `Открыть папку только для «${C.cur().name}»`, hint: S.lib.devDir, onClick: () => open(S.lib.devDir) } : null].filter(Boolean),
    { cls: 'quiet icon-only', icon: 'more', title: 'Папка с приложениями на компьютере' });
  return h('div', { class: 'dk-lib' },
    secHead('Можно установить', L.length, `Приложения из папки ${folder} на компьютере. «Установить» добавит их в «На читалку».`, L.length ? [add, more] : [more]),
    S.lib.err ? h('p', { class: 'hint', style: { padding: '0 16px' } }, S.lib.err) : null,
    L.length ? L.map(libRow) : h('div', { class: 'dk-empty' }, h('span', { html: icon('plus') }),
      h('b', {}, 'Здесь пока ничего нет'),
      h('p', {}, `Скачали приложение для PocketBook? Добавьте файл .app или zip-архив — он сохранится в папке ${folder} на компьютере, и его можно будет поставить на читалку.`),
      add));
}
function libRow(x) {
  const k = keyFor(x.file), op = S.draft.ops[k] || {}, on = onReader(x.file), pending = !!op.install;
  const title = x.title || x.file.replace(/\.app$/i, '');
  return rowEl({ cls: 'dk-row dk-librow', cover: tile({ title, own: true }), title, sub: x.file,
    tags: [x.only ? h('span', { class: 'chip', title: 'Лежит в папке этой читалки — другим не предлагается' }, 'только эта читалка') : null,
      on ? h('span', { class: 'chip ok' }, 'на читалке') : pending ? h('span', { class: 'chip new' }, 'будет установлено') : null,
      on ? h('button', { class: 'btn quiet', title: 'Заменить файл на читалке этим (например, новой версией)', onclick: e => { e.stopPropagation(); libUpdate(x); } }, 'Обновить')
        : pending ? h('button', { class: 'btn quiet', onclick: e => { e.stopPropagation(); delete S.draft.ops[k]; saveDraft(); render(); } }, 'Не ставить')
        : h('button', { class: 'btn primary', onclick: e => { e.stopPropagation(); libInstall(x); } }, 'Установить')],
    menu: [{ label: 'Убрать из папки…', hint: 'в корзину Windows; на читалке ничего не меняется', onClick: () => libForget(x) }] });
}
async function libAdd() {
  const p = await pickFiles('Приложения для читалки (.app или zip)'); if (!p || !p.length) return;
  const code = devCode();
  const where = ownDir() ? 'own' : code ? await dialog(close => h('div', { class: 'dlg' }, h('h2', {}, 'Для какой читалки?'),
    h('p', { style: { margin: 0 } }, 'Если приложение собрано под одну модель (например, своя версия для Color 3), выберите «Только для этой».'),
    h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close(null) }, 'Отмена'),
      h('button', { class: 'btn', onclick: () => close('one') }, `Только для «${C.cur().name}»`),
      h('button', { class: 'btn primary', onclick: () => close('all') }, 'Для всех читалок')))) : 'all';
  if (!where) return;
  try { const got = await call('apps_import', { paths: p, code: where === 'one' ? code : null, dir: ownDir() }); toast(got.length ? `В папке: ${got.join(', ')}` : 'Приложений в выбранном не нашлось.'); }
  catch (e) { toast(String(e.message || e)); }
  libLoad();
}
async function libInstall(x) {
  const k = keyFor(x.file), title = x.title || x.file.replace(/\.app$/i, '');
  const op = { path: `/mnt/ext1/applications/${x.file}`, title, group: '@General', install: x.path };
  if (x.icon) {
    try {
      const r = await call('read_image', { path: x.icon });
      const img = await new Promise((res, rej) => { const i = new Image(); i.onload = () => res(i); i.onerror = () => rej(new Error('значок не открылся')); i.src = `data:${r.mime};base64,${r.data}`; });
      const [bmp, inv, url] = makeIcon(img, iconSize()), name = x.file.replace(/\.app$/i, '').toLowerCase().replace(/[^a-z0-9_-]/g, '_');
      const files = await call('desktop_icon_save', { device: C.cur().id, name, icon: bmp, focused: inv });
      Object.assign(op, { icon: `applications/icons/${name}.bmp`, focused: `applications/icons/${name}_f.bmp`, iconFile: files, iconUrl: url });
    } catch (e) { toast(`Значок не подошёл (${e.message || e}) — поставлю без него.`); }
  }
  S.draft.ops[k] = Object.assign({}, S.draft.ops[k], op); saveDraft(); render();
  toast(`«${title}» — установится через «На читалку».`);
}
function libUpdate(x) {
  return Q.add(C, [{ src: x.path, dst: 'applications', kind: 'other', label: `Приложение ${x.file} (новая версия)` }]);
}
async function libForget(x) {
  const ok = await dialog(close => h('div', { class: 'dlg' }, h('h2', {}, `Убрать «${x.file}» из папки?`),
    h('p', { style: { margin: 0 } }, 'Файл (и его значок) уйдёт в корзину Windows. На читалке ничего не меняется — чтобы удалить с читалки, используйте «⋯ → Удалить с читалки…» у приложения выше.'),
    h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close(false) }, 'Отмена'), h('button', { class: 'btn primary', onclick: () => close(true) }, 'Убрать'))));
  if (!ok) return;
  try { await call('apps_forget', { path: x.path, dir: ownDir() }); } catch (e) { toast(String(e.message || e)); }
  const k = keyFor(x.file); if (S.draft.ops[k] && S.draft.ops[k].install) { delete S.draft.ops[k]; saveDraft(); }
  libLoad();
}
async function uninstall(a) {
  const ok = await dialog(close => h('div', { class: 'dlg' }, h('h2', {}, `Удалить «${a.title}» с читалки?`),
    h('p', { style: { margin: 0 } }, `Удалится файл applications/${a.file} и его значки, приложение пропадёт с рабочего стола${panelKeys().includes(a.key) ? ' и с главного экрана' : ''}. Перед удалением всё копируется на компьютер — вернуть можно через «Копии».`),
    h('p', { class: 'hint', style: { margin: 0 } }, 'Удаление выполнится в «На читалку», вместе с остальными изменениями. Настройки самого приложения (если есть) остаются.'),
    h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close(false) }, 'Отмена'), h('button', { class: 'btn primary', onclick: () => close(true) }, 'Удалить'))));
  if (!ok) return;
  S.draft.ops[a.key] = { uninstall: true, path: `/mnt/ext1/applications/${a.file}` }; saveDraft(); render();
}
async function rename(a) {
  const v = await dialog(close => { const inp = h('input', { class: 'input', value: a.title, maxlength: 40, onkeydown: e => { if (e.key === 'Enter') close(inp.value); } });
    setTimeout(() => { inp.focus(); inp.select(); }, 30);
    return h('div', { class: 'dlg' }, h('h2', {}, 'Название на рабочем столе'), inp,
      h('p', { class: 'hint', style: { margin: 0 } }, 'Так приложение будет подписано в «Приложениях» и на главном экране.'),
      h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close(null) }, 'Отмена'), h('button', { class: 'btn primary', onclick: () => close(inp.value) }, 'Готово'))); });
  if (v == null || !v.trim() || v.trim() === a.title) return;
  setOp(a.key, { title: v.trim() });
}
async function pickIcon(a) {
  const p = await pickFiles('Картинка для значка'); if (!p || !p.length) return;
  try {
    const r = await call('read_image', { path: p[0] });
    const img = await new Promise((res, rej) => { const i = new Image(); i.onload = () => res(i); i.onerror = () => rej(new Error('Картинка не открылась')); i.src = `data:${r.mime};base64,${r.data}`; });
    const N = iconSize(), [bmp, inv, url] = makeIcon(img, N);
    const name = (a.file || a.key).replace(/\.app$/i, '').replace(/^U_/, '').toLowerCase().replace(/[^a-z0-9_-]/g, '_');
    const files = await call('desktop_icon_save', { device: C.cur().id, name, icon: bmp, focused: inv });
    setOp(a.key, { icon: `applications/icons/${name}.bmp`, focused: `applications/icons/${name}_f.bmp`, iconFile: files, iconUrl: url });
  } catch (e) { toast(String(e.message || e)); }
}
/** значок как у прошивки: BMP 8 бит серый, холст N×N, рисунок ≈ 70 % по центру; в фокусе — инверсия */
function makeIcon(img, N) {
  const c = document.createElement('canvas'); c.width = c.height = N;
  const g = c.getContext('2d'); g.fillStyle = '#fff'; g.fillRect(0, 0, N, N);
  const box = N * 0.7, k = Math.min(box / img.naturalWidth, box / img.naturalHeight), w = img.naturalWidth * k, hh = img.naturalHeight * k;
  g.imageSmoothingQuality = 'high'; g.drawImage(img, (N - w) / 2, (N - hh) / 2, w, hh);
  const px = g.getImageData(0, 0, N, N).data, gray = new Uint8Array(N * N);
  for (let i = 0; i < N * N; i++) gray[i] = Math.round(0.299 * px[i * 4] + 0.587 * px[i * 4 + 1] + 0.114 * px[i * 4 + 2]);
  const url = c.toDataURL('image/png');
  return [bmp8(gray, N), bmp8(gray.map(v => 255 - v), N), url];
}
function bmp8(gray, N) {
  const row = (N + 3) & ~3, off = 14 + 40 + 1024, size = off + row * N, b = new Uint8Array(size), dv = new DataView(b.buffer);
  b[0] = 0x42; b[1] = 0x4D; dv.setUint32(2, size, true); dv.setUint32(10, off, true);
  dv.setUint32(14, 40, true); dv.setInt32(18, N, true); dv.setInt32(22, N, true); dv.setUint16(26, 1, true); dv.setUint16(28, 8, true);
  dv.setUint32(34, row * N, true); dv.setUint32(46, 256, true);
  for (let i = 0; i < 256; i++) { b[54 + i * 4] = b[55 + i * 4] = b[56 + i * 4] = i; }
  for (let y = 0; y < N; y++) b.set(gray.subarray((N - 1 - y) * N, (N - y) * N), off + y * row);
  let s = ''; for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode.apply(null, b.subarray(i, i + 0x8000));
  return btoa(s);
}
async function backupsMenu() {
  const list = await call('desktop_backups', { device: C.cur().id }).catch(() => []);
  const go = (stamp, label) => Q.addAndGo(C, [{ kind: 'desktop-restore', stamp, src: '', dst: 'system/config', label }]);
  return [
    { label: 'Заводской рабочий стол', hint: 'убрать свою настройку — как после покупки', onClick: () => go('factory', 'Рабочий стол: заводской') },
    list.length ? { sep: 'Копии на компьютере' } : null,
    ...list.slice(0, 12).map(b => ({ label: when(b.stamp), hint: b.factory ? 'тогда был заводской' : 'вернуть как было', onClick: () => go(b.stamp, `Рабочий стол из копии от ${when(b.stamp)}`) })),
    !list.length ? { label: 'Копий пока нет', hint: 'появятся при первой записи', disabled: true } : null,
  ].filter(Boolean);
}
const when = st => { const m = /^(\d{4})-(\d\d)-(\d\d)_(\d\d)-(\d\d)/.exec(st); return m ? `${+m[3]}.${m[2]}.${m[1]} ${m[4]}:${m[5]}` : st; };
