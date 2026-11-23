// «Книги»: обложки, ссылки → текст, TXT → FB2, правка каждой книги (данные, обложка, текст).
// Оригиналы не меняются: готовые книги ложатся в папку книг текущей читалки (PB743K3\Books), оттуда — на читалку.
// Разбор книг, чистка и правка — в Rust (pb-books), обложки рисуются здесь, на canvas (js/covers.js).
//
// Обложка книги: своя (если есть) / нарисованная (стиль и цвет) / своя картинка с компьютера.
// По умолчанию рисуется только книгам без обложки (или всем, если включено «Заменять свои»);
// выбор для отдельной книги или серии сильнее общих настроек.
import { call, on, pickFolder, openPath } from '../api.js';
import { $, h, fill, icon, size, plural, toast, dialog, busy, lock, progress as opProgress, loading, menuButton, mzEmpty } from '../ui.js';
import * as CV from '../covers.js';
import { flowBar, outLine, drop, toQueue, scopeSelect, inScope, freshBar, readyView, readerPath, listTools, selBar, rowEl, stateChip, shortDate } from '../upload.js';
import { whereDrawer } from '../where.js';
import { optRow, ctlRow } from '../help.js';
import { epubFromPage, fb2FromPage, makeEditable } from '../textedit.js';
import * as Q from '../queue.js';

const DEF = { root: '', scope: '', skipDone: true, txt: true, niceNames: true, outNames: {}, covers: true, replace: false, unlink: true, style: 'Табло', bySeries: true, categories: null, overrides: {}, images: {}, looks: {} };
let C, S, root, unsub = [];

const cfg = () => ({ style: S.style, bySeries: S.bySeries, categories: S.categories, overrides: S.overrides });
const screen = () => { const sc = C.cur().reader.screen; return { W: sc.width, H: sc.height, gray: !sc.color }; };
const outDir = () => C.cur().dirs.books;
const ed = i => { const e = S.ed[i] || (S.ed[i] = { meta: {}, parts: {}, image: null }); if (!e.drop) e.drop = {}; return e; };
// своя картинка-обложка: файл на компьютере (переживает «Обновить» и перезапуск) — ключ по папке и пути книги
const imgKey = it => `${S.root}|${String(it.rel).replace(/\\/g, '/')}`;
const imgPath = it => (S.images || {})[imgKey(it)] || null;
const hash = t => { let a = 5381, b = 52711; for (let i = 0; i < t.length; i++) { const c = t.charCodeAt(i); a = (a * 33) ^ c; b = (b * 33) ^ c; } return (a >>> 0).toString(36) + (b >>> 0).toString(36); };
/** какой обложкой книга сохранена в последний раз: своя картинка или своя настройка вида — чтобы «Только новые» не пропустил смену */
function lookSig(it) {
  const p = plan(it), o = S.overrides[CV.bookKey(book(it))] || (book(it).series && S.overrides[CV.seriesKey(book(it))]) || null;
  if (p === 'image') return `img:${imgPath(it) || 'новая'}`;
  if (!o) return null; // как у всех — без подписи, такие книги пересохранять незачем
  return `ov:${JSON.stringify(o)}`;
}
const lookChanged = it => { const sg = lookSig(it); return sg != null && (S.looks || {})[imgKey(it)] !== sg; };
function forgetImage(it) { if (imgPath(it)) { const m = Object.assign({}, S.images); delete m[imgKey(it)]; S.images = m; } ed(it.index).image = null; }
/** картинка из файла — в data URL (по одному разу) */
function imageUrl(path) {
  S.imgUrl = S.imgUrl || {};
  if (S.imgUrl[path] === undefined) { S.imgUrl[path] = null; call('read_image', { path }).then(r => { S.imgUrl[path] = `data:${r.mime};base64,${r.data}`; later(); }).catch(() => { S.imgUrl[path] = ''; }); }
  return S.imgUrl[path] || null;
}
// правки этой сессии: данные, текст, картинка или смена обложки — такие книги сохраняются даже при «Только новые»
// правки книги, уже записанные в готовую книгу: подпись правок на момент сохранения (по пути книги)
const edSig = i => { const e = S.ed[i]; return e ? hash(JSON.stringify([e.meta, e.parts, e.drop || {}, e.image || null, e.cover || null])) : ''; };
const edKey = i => `${S.root}|${S.items[i] ? S.items[i].rel : i}`;
/** есть правки, которых ещё нет в готовой книге */
const pending = i => edited(i) && (S.edSaved || {})[edKey(i)] !== edSig(i);
const edited = i => { const e = S.ed[i]; return !!e && (Object.keys(e.meta).length > 0 || Object.keys(e.parts).length > 0 || Object.keys(e.drop || {}).length > 0 || !!e.image || !!e.cover); };
/** книга с учётом правок: для обложки, списка и правил категорий */
const book = it => {
  const m = (S.ed[it.index] || {}).meta || {};
  return { title: m.title ?? (it.title || it.name.replace(/\.[^.]+$/, '')), author: m.author ?? it.author, series: m.series ?? it.series, number: m.number ?? it.number, genres: it.genres, rel: it.rel };
};
// it.fmt приходит из Rust: fb2, fb2.zip, epub, fb3, txt
const canMeta = it => !it.error && ['fb2', 'fb2.zip', 'epub', 'txt'].includes(it.fmt);
const canParts = it => !it.error && ['fb2', 'fb2.zip', 'epub'].includes(it.fmt);

/** у книги есть обложка: своя в файле или картинка, выбранная в программе (файл книги при этом не меняется —
 *  картинка хранится отдельно и вшивается в готовую книгу, поэтому смотреть только на файл нельзя) */
const hasCover = it => !it.error && (it.hasCover || !!(S.ed[it.index] || {}).image || !!imgPath(it));
/** что будет с обложкой: own — своя остаётся, draw — нарисовать, image — своя картинка, none — без обложки */
function plan(it) {
  if (it.error) return 'none';
  if ((S.ed[it.index] || {}).image || imgPath(it)) return 'image';
  const o = S.overrides[CV.bookKey(book(it))] || {};
  if (o.keep) return it.hasCover ? 'own' : 'none';
  if (o.style || o.color) return 'draw';
  if (!S.covers) return it.hasCover ? 'own' : 'none';
  if (!it.hasCover || S.replace || it.fmt === 'txt') return 'draw';
  return 'own';
}

function save() {
  const v = {}; for (const k of Object.keys(DEF)) v[k] = S[k];
  C.set('books', v).catch(() => {});
}

export default {
  id: 'books', title: 'Книги', hint: 'обложки, ссылки, TXT', icon: 'books',
  // вернулись в раздел — обновить «Готовое» (могли сохранить или удалить файлы)
  show() { if (S && S.items.length && !S.busy) refreshReady(); },
  async mount(main, ctx) {
    C = ctx;
    S = Object.assign(structuredClone(DEF), ctx.config.books || {}, { items: [], skipped: 0, total: 0, sel: -1, ed: {}, filter: 'all', q: '', busy: false, status: {}, view: 'src', done: new Set(), ready: [],
      thumbs: new Map(), own: {}, ownQ: [], ownBusy: 0 });
    if (!S.categories) S.categories = structuredClone(CV.DEFAULT_CATEGORIES);
    root = h('div', { class: 'page' }); main.append(root);
    unsub.push(await on('books://scan', p => { S.scanP = p; later(); }));
    // другая текущая читалка — другой размер экрана, обложки перерисовать
    unsub.push(ctx.onDevices(() => { if (root.isConnected) { S.thumbs.clear(); render(); if (S.items.length && !S.busy) refreshReady(); } }));
    // «ждёт отправки» у готовых — по списку «На читалку»
    unsub.push(Q.onChange(() => { if (root.isConnected && S.view !== 'src') render(); }));
    await CV.ready();
    render();
    if (S.root) scan(S.root, true);
  },
  drop(paths) { if (root && root.isConnected && !S.busy) scan(paths[0]); },
  // из приветствия: сразу окно выбора папки
  pick() { if (S && !S.busy) choose(); },
  hover(v) { const d = root && $('.drop', root); if (d) d.classList.toggle('hot', v); },
  unmount() { unsub.forEach(u => typeof u === 'function' && u()); unsub = []; if (S) S.stop = true; closeBook(); },
};

/* ================================================================ отрисовка */
let rT = 0;
// перерисовка не чаще кадра; во время сохранения — не чаще раза в 300 мс: строк сотни, а ход приходит после каждой книги
let lastR = 0, lT = 0;
const later = () => {
  if (rT || lT) return;
  const go = () => { rT = requestAnimationFrame(() => { rT = 0; lastR = performance.now(); if (root.isConnected) render(); if (B && B.editing !== 'text') drawBook(); }); };
  const wait = S && S.busy === 'run' ? 300 - (performance.now() - lastR) : 0;
  if (wait > 0) lT = setTimeout(() => { lT = 0; go(); }, wait); else go();
};

function render() {
  S._plan = null;
  // прокрутка списка и правой панели не должна прыгать при перерисовке
  const keep = ['.blist', '.inspector'].map(sel => [sel, $(sel, root) ? $(sel, root).scrollTop : 0]);
  fill(root,
    h('header', { class: 'page-head' }, h('div', {}, h('h1', {}, 'Книги'),
      h('p', {}, 'Обложки и названия для книг. Ваши файлы не меняются.'))),
    S.root ? flow()
      : drop('Перетащите папку с книгами', 'FB2, FB2.ZIP, EPUB, FB3 и TXT, вложенные папки тоже', choose, C.srcFor('books'), p => scan(p)),
    S.busy === 'scan' && !S.items.length ? h('section', { class: 'panel' }, opProgress({ stage: 'Сканирование', value: progress(), detail: S.scanP && S.scanP.total ? `${S.scanP.done} из ${S.scanP.total}` : S.root })) : null,
    // одна плашка на весь блок: при сканировании размыто всё, при сохранении — всё, кроме нижней строки с ходом и «Остановить»
    S.items.length ? busy(h('div', { class: 'stack busy-block' }, h('div', { class: 'split' }, listPanel(), S.view !== 'src' && S.readyCur ? readyInspector() : inspector()), S.busy === 'scan' ? actions() : null),
      scanMsg() || (S.busy === 'run' ? runMsg() : null)) : null,
    S.items.length && S.busy !== 'scan' ? actions() : null);
  for (const [sel, top] of keep) { const el = $(sel, root); if (el) el.scrollTop = top; }
}

const scanMsg = () => S.busy === 'scan' ? (S.scanP && S.scanP.total ? `Сканирование… ${S.scanP.done} из ${S.scanP.total}` : 'Сканирование…') : null;
const runMsg = () => `Сохранение… ${Object.values(S.status).filter(x => x.ok != null).length} из ${S.runTotal}`;
// пропущенные форматы приходят как { mobi: 3, pdf: 2 }
const skipped = () => { const e = Object.entries(S.skipped || {}); const n = e.reduce((a, [, v]) => a + v, 0);
  return n ? `${n} ${plural(n, 'файл', 'файла', 'файлов')} не трогаю (${e.map(([k]) => k.toUpperCase()).join(', ')})` : ''; };
/** полоса над строкой папки: чтение книг, сохранение */
function progress() {
  if (S.busy === 'scan') return S.scanP && S.scanP.total ? S.scanP.done / S.scanP.total : -1;
  if (S.busy === 'run') { const n = Object.values(S.status).filter(x => x.ok != null).length; return n / Math.max(1, S.runTotal || 1); }
  return null;
}
function summary() {
  if (S.busy === 'scan') return S.scanP && S.scanP.total ? `Читаю книги: ${S.scanP.done} из ${S.scanP.total}` : 'Ищу книги…';
  if (S.busy === 'run') { const n = Object.values(S.status).filter(x => x.ok != null).length; return `Сохраняю: ${n} из ${S.runTotal}`; }
  if (!S.items.length) return skipped() ? `Книг для обработки нет, ${skipped()}` : 'Книг не найдено';
  const noCover = S.items.filter(i => !i.error && !hasCover(i)).length;
  return [`${S.items.length} ${plural(S.items.length, 'книга', 'книги', 'книг')}, ${size(S.total)}`,
    noCover ? `без обложки ${noCover}` : 'у всех есть обложки',
    skipped() || null].filter(Boolean).join(', ');
}

/** «Скачанное → Готовое → На читалке»: три места книг; шаг — это и вкладка списка */
function flow() {
  const d = C.cur(), conn = !!d.connected, checked = conn && S.ready.length && S.ready.every(f => f.onReader != null);
  const on = S.ready.filter(f => f.onReader === 'same').length;
  return flowBar(C, { kind: 'books', title: 'Папка с книгами', view: S.view, onView: v => { S.view = v; S.readyCur = null; render(); },
    src: { path: S.root, count: scoped().length, fresh: S.ready.length ? fresh().length : 0, tip: `${S.root}\n${summary()}` },
    ready: { path: outDir(), count: S.ready.length, wait: checked ? S.ready.length - on : 0 },
    reader: { where: d.readerDirs.books, count: conn ? on : null },
    onPick: p => scan(p), onMap: () => whereDrawer(C, 'books'),
    // подпапка: список и сохранение — только в ней (остальное не трогается)
    scope: S.items.length ? scopeSelect(S.items.map(dirOf), S.scope, setScope, S.items.length) : null,
    settings: bookSettings(), locked: !!S.busy, progress: progress() });
}

async function choose() { const p = await pickFolder('Папка с книгами', C.srcFor('books') || undefined); if (p) scan(p); }

// все настройки «Книг» — в одной панели «Настройки ▾», у каждой пояснение
function bookSettings() {
  const set = (k, v) => { S[k] = v; save(); S.thumbs.clear(); render(); };
  const on = [S.skipDone ? 'только новые' : 'все заново', S.covers ? `обложки: ${S.style}` : 'без обложек'];
  return { key: 'books', summary: on.join(', '), groups: [
    ['Сохранение', [
      optRow('skipDone', S.skipDone, v => { S.skipDone = v; save(); render(); }),
      optRow('txt', S.txt, v => { S.txt = v; save(); scan(S.root); }),
      optRow('niceNames', S.niceNames !== false, v => { S.niceNames = v; save(); refreshReady(); }),
      optRow('unlink', S.unlink, v => { S.unlink = v; save(); render(); })]],
    ['Обложки', [
      optRow('covers', S.covers, v => set('covers', v)),
      ctlRow('style', h('select', { 'aria-label': 'Вид обложки', disabled: !S.covers, onchange: e => set('style', e.target.value) },
        CV.STYLES.map(([n, note]) => h('option', { value: n, title: note, selected: n === S.style }, n))), { disabled: !S.covers }),
      optRow('replace', S.replace, v => set('replace', v), { disabled: !S.covers }),
      optRow('bySeries', S.bySeries, v => set('bySeries', v), { disabled: !S.covers }),
      ctlRow('categories', h('button', { class: 'btn', disabled: !S.covers, onclick: categories }, 'Настроить…'), { disabled: !S.covers })]]] };
}

const dirOf = i => i.rel.replace(/\\/g, '/').split('/').slice(0, -1).join('/');
// книги выбранной подпапки — одна выборка на список книг и подпапку (её спрашивают много раз за перерисовку)
const scoped = () => { if (S._scOf !== S.items || S._scS !== S.scope) { S._scOf = S.items; S._scS = S.scope; S._sc = S.items.filter(i => inScope(dirOf(i), S.scope)); } return S._sc; };
function setScope(v) {
  S.scope = v; S.sel = -1; save(); render();
}
function visible() {
  const q = S.q.trim().toLowerCase();
  return scoped().filter(i =>
    (S.filter === 'all' || (S.filter === 'cover' && !i.error && !hasCover(i)) || (S.filter === 'edit' && pending(i.index)) || (S.filter === 'err' && i.error)) &&
    (!q || [book(i).title, book(i).author, book(i).series, i.rel].some(x => x && String(x).toLowerCase().includes(q))));
}

/* ---------------------------------------------------------------- обложки: своя (с диска), нарисованная, картинка */
function ownCover(it) {
  const v = S.own[it.index];
  if (v !== undefined) return v;
  if (S.busy !== 'scan' && !S.ownQ.includes(it.index)) { S.ownQ.push(it.index); pumpOwn(); }
  return undefined;
}
// свои обложки подгружаются по три за раз
async function pumpOwn() {
  while (S.ownBusy < 3 && S.ownQ.length) {
    const i = S.ownQ.shift(), gen = S.gen; S.ownBusy++;
    // ответ от прошлого поиска не должен попасть к книге с тем же номером в новом
    call('books_cover', { index: i, height: 600 }).then(u => { if (gen === S.gen) S.own[i] = u; }).catch(() => { if (gen === S.gen) S.own[i] = null; })
      .finally(() => { S.ownBusy--; later(); pumpOwn(); });
  }
}
/** картинка обложки для показа: { url } или { canvas } */
/** вид обложки книги (категория, серия, свой выбор) — считается один раз, пока не поменялись книга или настройки обложек */
function lookOf(it, b, gray) {
  const sig = `${S.style}|${S.bySeries}|${gray}|${b.title}|${b.author}|${b.series}|${b.number}`;
  if (S._lookOv !== S.overrides || S._lookCat !== S.categories || S._lookItems !== S.items) { S._lookOv = S.overrides; S._lookCat = S.categories; S._lookItems = S.items; S._looks = new Map(); }
  const c = S._looks.get(it.index);
  if (c && c.sig === sig) return c.look;
  const look = CV.resolve(b, cfg(), gray); S._looks.set(it.index, { sig, look }); return look;
}
function coverView(it, w) {
  const p = plan(it), { W, H, gray } = screen();
  if (p === 'image') { const u = ed(it.index).image || imageUrl(imgPath(it)); return u ? { url: u } : { note: '…' }; }
  if (p === 'own') { const u = ownCover(it); return u ? { url: u } : { note: u === undefined ? '…' : 'своя' }; }
  if (p === 'none') return { note: it.error ? '—' : 'нет' };
  const b = book(it), look = lookOf(it, b, gray), key = `${it.index}|${w}|${look.style}|${look.color}|${b.title}|${b.author}|${b.series}|${b.number}`;
  let url = S.thumbs.get(key);
  if (!url) { url = CV.draw(b, look.style, look.color, w, Math.round(w * H / W), gray).toDataURL(); S.thumbs.set(key, url); }
  return { url, look };
}
const coverEl = (it, w, cls) => { const v = coverView(it, w); return v.url ? h('span', { class: cls + ' has' }, h('img', { src: v.url, alt: '' })) : h('span', { class: cls }, v.note); };

// готовый файл этой книги (txt сохраняется как fb2)
/* ---------- имя готового файла: «Автор — Название.epub» вместо скачанного «Strugackiy_Trudno-byt-bogom.434934.fb2.epub».
   Только тем книгам, которых ещё нет в «Готовом»: сохранённые раньше остаются под своим именем (на читалке не будет двух копий).
   Папка — та же, что у исходной книги; одинаковые имена в одной папке получают « (2)». Ваш файл не меняется. */
const nrmRel = r => String(r || '').replace(/\\/g, '/');
const legacyRel = it => nrmRel(it.rel).replace(/\.txt$/i, '.fb2');
const extOf = it => it.fmt === 'txt' ? '.fb2' : it.fmt === 'fb2.zip' ? '.fb2.zip' : '.' + String(it.fmt || '').toLowerCase();
const cleanName = s => String(s || '').replace(/[\\/:*?"<>|\u0000-\u001f]/g, ' ').replace(/\s+/g, ' ').trim();
function niceName(it) {
  const b = book(it), t = cleanName(b.title).replace(/[. ]+$/, '');
  if (!t) return null;
  const a = cleanName(b.author);
  let n = a ? `${a} — ${t}` : t;
  if (n.length > 150) n = n.slice(0, 150).replace(/[. ]+$/, '').trim();
  return n + extOf(it);
}
function outPlan() {
  if (S._plan) return S._plan;
  const ready = new Set(S.ready.map(f => nrmRel(f.rel).toLowerCase())), used = new Set(ready), plan = new Map(), todo = [];
  for (const it of S.items) {
    const prev = (S.outNames || {})[imgKey(it)], leg = legacyRel(it);
    if (prev && ready.has(prev.toLowerCase())) plan.set(it.index, prev);
    else if (ready.has(leg.toLowerCase()) || S.niceNames === false || it.error) plan.set(it.index, leg);
    else todo.push(it);
  }
  for (const it of todo) {
    const nn = niceName(it), leg = legacyRel(it);
    if (!nn) { plan.set(it.index, leg); used.add(leg.toLowerCase()); continue; }
    const dir = leg.includes('/') ? leg.slice(0, leg.lastIndexOf('/') + 1) : '', ext = extOf(it), stem = nn.slice(0, -ext.length);
    let r = dir + nn, k = 2;
    while (used.has(r.toLowerCase())) r = `${dir}${stem} (${k++})${ext}`;
    used.add(r.toLowerCase()); plan.set(it.index, r);
  }
  return (S._plan = plan);
}
/** путь готовой книги в «Готовом» (через /) */
const outRel = it => outPlan().get(it.index) || legacyRel(it);
const readyKey = it => outRel(it).toLowerCase();
function readyOf(it) {
  if (!S.done.has(it.index)) return null;
  if (!S._rmap || S._rmapOf !== S.ready) { S._rmapOf = S.ready; S._rmap = new Map(S.ready.map(f => [String(f.rel).replace(/\\/g, '/').toLowerCase(), f])); }
  return S._rmap.get(readyKey(it)) || { mtime: 0, onReader: null };
}
/** что будет с книгой в готовом: «TXT → FB2», «FB2 · + обложка» */
function destEl(it) {
  const f = String(it.format || it.fmt || '').toUpperCase();
  if (it.error) return h('span', { class: 'dest' }, f);
  if (it.fmt === 'txt') return h('span', { class: 'dest', title: 'Текстовый файл станет книгой FB2 с обложкой' }, 'TXT → ', h('i', {}, 'FB2'));
  const p = plan(it);
  const add = p === 'draw' ? (it.hasCover ? 'новая обложка' : '+ обложка') : p === 'image' ? 'своя картинка' : null;
  const nm = outRel(it).split('/').pop(), renamed = nm !== legacyRel(it).split('/').pop();
  return h('span', { class: 'dest', title: [add ? `В готовой книге: ${add}` : 'Формат не меняется', renamed ? `Имя в «Готовом»: ${nm}` : null].filter(Boolean).join('\n') }, f, add ? [' · ', h('i', {}, add)] : null);
}
function tags(it) {
  const st0 = S.status[it.index], st = st0 && st0.skipped ? null : st0;
  return h('div', { class: 'tags' },
    destEl(it),
    it.error && !st ? h('span', { class: 'chip warn', title: it.error }, 'не читается') : null,
    it.error && !st ? null : stateChip({ saving: st && st.ok == null, err: st && st.ok === false ? st.text : null, savedNow: st && st.ok && st.now, note: st && st.note,
      edited: !st && pending(it.index), ready: readyOf(it), connected: !!C.cur().connected, fresh: S.ready.length > 0 || S.done.size > 0 }));
}

// список: шапка (название, сколько, отбор, поиск) и книги под ней — как «Тома» у комиксов
function listPanel() {
  const rows = visible(), lim = S.limit || 300;
  const n = f => scoped().filter(f).length;
  const F = [['all', S.scope ? 'В подпапке' : 'Все', () => true], ['cover', 'Без обложки', i => !i.error && !hasCover(i)],
    ['edit', 'Есть правки', i => pending(i.index)], ['err', 'Не читаются', i => !!i.error]].filter(([id, , f]) => id === 'all' || id === S.filter || n(f));
  // «новых» — только когда что-то уже сохранено: до первого сохранения новое всё
  const nf = S.ready.length ? fresh().length : 0, RV = S.view !== 'src';
  if (RV) return h('section', { class: 'panel list-panel' },
    S.view === 'ready' ? freshBar(nf, ['книга', 'книги', 'книг'], { busy: S.busy, onShow: () => { S.view = 'src'; render(); }, onSave: () => run({ onlyNew: true }) }) : null,
    h('div', { class: 'blist' }, readyView(S, { ctx: C, kind: 'books', dir: outDir(), scope: S.scope, icon: 'books', rerender: render, onRemoved: refreshReady, extraMenu: backupMenu, only: S.view === 'reader' ? 'on' : null,
      fmt: true, cur: S.readyCur, onPick: f => { S.readyCur = f.path; render(); }, onEdit: f => editFromReady(f), onOpen: f => { S.readyCur = f.path; render(); readyInfo(f.path); openReady(f); },
      title: f => { const it = byOut().get(String(f.rel).replace(/\\/g, '/').toLowerCase()); return it ? book(it).title : null; },
      sub: f => { const it = byOut().get(String(f.rel).replace(/\\/g, '/').toLowerCase()); if (!it) return null; const b = book(it); return [b.author, b.series ? `${b.series}${b.number ? ' #' + b.number : ''}` : null].filter(Boolean).join(', '); },
      // обложка — как у исходной книги (имя готовой = её путь, у TXT — .fb2)
      thumb: f => { const it = byOut().get(String(f.rel).replace(/\\/g, '/').toLowerCase()); return it ? coverEl(it, 120, 'cov') : null; } })));
  const sel = S.srcSel = S.srcSel || new Set(), multi = sel.size > 0;
  const picked = scoped().filter(it => sel.has(it.index));
  return h('section', { class: 'panel list-panel' },
    // та же строка, что в «Готово для читалки»: фильтр, поиск, «Выбрать несколько»
    listTools({ key: 'src', filters: F.map(([id, t, f]) => [id, id === 'all' ? t : `${t} ${n(f)}`]), value: S.filter, onFilter: id => { S.filter = id; render(); },
      note: skipped() ? h('small', { class: 'hint skip-note', style: { margin: 0 }, title: 'Эти форматы программа не меняет и в готовое не кладёт' }, skipped()) : null,
      q: S.q, onQ: v => { S.q = v; render(); }, all: { keys: () => visible().filter(i => !i.error).map(i => i.index), sel, onChange: render } }),
    picked.length ? selBar({ count: picked.length, bytes: picked.reduce((a, it) => a + (it.size || 0), 0), onClear: () => { sel.clear(); render(); },
      actions: [h('button', { class: 'btn quiet', disabled: !!S.busy, title: 'Сохранить в папку только выбранные (даже если они уже готовы)', onclick: () => run({ list: picked }) }, 'Сохранить выбранные'),
        h('button', { class: 'btn apply', disabled: !!S.busy, title: 'Сохранить выбранные и добавить их в «На читалку»', onclick: () => sendSrc(picked) }, h('span', { html: icon('apply') }), `На читалку: ${picked.length}`)] }) : null,
    h('div', { class: 'blist ready-list', role: 'listbox', 'aria-label': 'Книги' },
      rows.length ? rows.slice(0, lim).map(it => { const b = book(it);
        return rowEl({ multi, checked: sel.has(it.index), picked: it.index === S.sel, tip: it.rel,
          cover: coverEl(it, 120, 'cov'), title: b.title,
          sub: [[b.author, b.series ? `${b.series}${b.number ? ' #' + b.number : ''}` : null].filter(Boolean).join(', '), size(it.size)].filter(Boolean).join(' · '),
          tags: [...tags(it).childNodes],
          menu: [{ label: 'Открыть книгу', onClick: () => openBook(it.index) },
            { label: 'Показать в папке', hint: 'ваш файл в проводнике', onClick: () => openPath(`${S.root}/${it.rel}`.replace(/\//g, '\\')).catch(e => toast(String(e))) },
            it.error ? null : { label: 'Сохранить только её', hint: 'в папку готового, даже если уже готова', onClick: () => run({ list: [it] }) }].filter(Boolean),
          onToggle: () => { sel.has(it.index) ? sel.delete(it.index) : sel.add(it.index); render(); },
          onClick: () => select(it.index), onOpen: () => openBook(it.index) }); })
        : S.q.trim() ? mzEmpty('ask', `По «${S.q.trim()}» ничего нет`, `Поиск идёт по названию, автору и серии${S.scope ? ` в подпапке «${S.scope.split('/').pop()}»` : ''}. Попробуйте другое слово${S.scope ? ' или покажите всю папку' : ''}.`,
            [S.scope ? h('button', { class: 'btn', onclick: () => setScope('') }, 'Искать во всей папке') : null, h('button', { class: 'btn quiet', onclick: () => { S.q = ''; render(); } }, 'Очистить поиск')])
          : mzEmpty('ask', 'Под этот отбор книг нет', 'Покажите все книги — или выберите другой отбор над списком.', [h('button', { class: 'btn', onclick: () => { S.filter = 'all'; render(); } }, 'Показать все')]),
      rows.length > lim ? h('button', { class: 'btn quiet', style: { margin: '8px auto', display: 'flex' }, onclick: () => { S.limit = lim + 300; render(); } }, `Показать ещё ${Math.min(300, rows.length - lim)}`) : null));
}
/** из «Готово для читалки» — к исходной книге в окне правки */
function editFromReady(f) {
  const it = byOut().get(String(f.rel).replace(/\\/g, '/').toLowerCase());
  if (!it) { toast('Исходной книги в «Скачанном» нет — изменить можно только её.'); return; }
  S.view = 'src'; S.readyCur = null; S.sel = it.index; render(); openBook(it.index);
}
/** выбранные из «Ваших файлов» → сохранить и сразу в «На читалку» */
async function sendSrc(list) {
  await run({ list });
  await refreshReady();
  const want = new Set(list.map(it => readyKey(it)));
  const files = S.ready.filter(f => want.has(String(f.rel).replace(/\\/g, '/').toLowerCase()));
  if (!files.length) return;
  const dirOf2 = r => String(r).replace(/\\/g, '/').split('/').slice(0, -1).join('/');
  await Q.addAndGo(C, files.map(f => ({ src: f.path, dst: [C.cur().readerDirs.books, dirOf2(f.rel)].filter(Boolean).join('/'), kind: 'books', label: String(f.rel).replace(/\\/g, '/').split('/').pop() })));
}

/* ---------------------------------------------------------------- выбор обложки (справа и в окне книги) */
function coverPicker(it) {
  const b = book(it), { W, H, gray } = screen(), p = plan(it);
  const look = CV.resolve(b, cfg(), gray);
  const key = it.series && S.forSeries ? CV.seriesKey(b) : CV.bookKey(b);
  const setOv = patch => {
    const o = Object.assign({}, S.overrides[key], patch); delete o.keep;
    const all = Object.assign({}, S.overrides, { [key]: o });
    if (key !== CV.bookKey(b)) { const bo = Object.assign({}, all[CV.bookKey(b)]); delete bo.keep; all[CV.bookKey(b)] = bo; }
    S.overrides = all; forgetImage(it); ed(it.index).cover = true; save(); S.thumbs.clear(); later();
  };
  const keepOwn = () => { S.overrides = Object.assign({}, S.overrides, { [CV.bookKey(b)]: { keep: true } }); forgetImage(it); ed(it.index).cover = true; save(); later(); };
  const reset = () => { const o = Object.assign({}, S.overrides); delete o[CV.bookKey(b)]; if (b.series) delete o[CV.seriesKey(b)]; S.overrides = o; forgetImage(it); ed(it.index).cover = true; save(); S.thumbs.clear(); later(); };
  const pickImage = () => {
    const inp = h('input', { type: 'file', accept: 'image/*', style: { display: 'none' } });
    inp.addEventListener('change', () => { const f = inp.files[0]; if (!f) return; const r = new FileReader(); r.onload = async () => {
      ed(it.index).image = r.result; later();
      // сразу сохранить на компьютер под экран читалки — выбор не пропадёт после «Обновить»
      try {
        const jpg = await imageJpeg(r.result, W, H, gray);
        const path = await call('books_cover_keep', { device: C.cur().id, name: hash(imgKey(it)), data: jpg });
        S.images = Object.assign({}, S.images, { [imgKey(it)]: path }); if (S.imgUrl) delete S.imgUrl[path]; save();
      } catch (e) { toast(`Картинку не удалось запомнить: ${e.message || e}`); }
    }; r.readAsDataURL(f); });
    document.body.append(inp); inp.click(); setTimeout(() => inp.remove(), 60000);
  };
  const own = S.own[it.index];
  if (it.hasCover && own === undefined) ownCover(it);
  const source = p === 'image' ? 'своя картинка' : p === 'own' ? 'своя обложка книги' : p === 'none' ? 'без обложки' : look.source;
  const tileKey = `${it.index}|${look.color}|${b.title}|${b.author}|${b.series}|${b.number}|${gray}`;
  if (S.tilesKey !== tileKey) { S.tilesKey = tileKey; S.tiles = {}; }
  const tile = n => S.tiles[n] || (S.tiles[n] = CV.draw(b, n, look.color, 150, Math.round(150 * H / W), gray).toDataURL());
  const custom = S.overrides[CV.bookKey(b)] || (b.series && S.overrides[CV.seriesKey(b)]);
  return h('div', { class: 'cpick' },
    h('div', { class: 'cpick-head' }, h('b', {}, 'Обложка'), h('small', { class: 'hint', style: { margin: 0 } }, source),
      custom || ed(it.index).image ? h('button', { class: 'btn quiet', style: { minHeight: '28px', marginLeft: 'auto' }, onclick: reset }, 'Как у всех') : null),
    it.series ? h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: !!S.forSeries, onchange: e => { S.forSeries = e.target.checked; later(); } }), `Стиль и цвет — для всей серии «${it.series}»`) : null,
    h('div', { class: 'covers', role: 'group', 'aria-label': 'Обложка' },
      it.hasCover ? h('button', { 'aria-pressed': p === 'own', title: 'Оставить обложку, которая есть в книге', onclick: keepOwn },
        own ? h('img', { src: own, alt: '' }) : h('i', { class: 'ph' }, own === undefined ? '…' : 'не читается'), h('span', {}, 'Своя')) : null,
      CV.STYLES.map(([n, note]) => h('button', { 'aria-pressed': p === 'draw' && n === look.style, title: note, onclick: () => setOv({ style: n }) }, h('img', { src: tile(n), alt: '' }), h('span', {}, n))),
      h('button', { 'aria-pressed': p === 'image', title: 'Взять картинку с компьютера', onclick: pickImage },
        p === 'image' ? h('img', { src: ed(it.index).image, alt: '' }) : h('i', { class: 'ph', html: icon('image') }), h('span', {}, 'Картинка…'))),
    p === 'draw' ? h('div', { class: 'swatches', role: 'group', 'aria-label': 'Цвет' }, CV.SWATCHES.map(c => h('button', { style: { background: c }, title: c, 'aria-pressed': (S.overrides[key] || {}).color === c, onclick: () => setOv({ color: c }) }))) : null);
}

/** путь книги: скачанное → готовое → на читалке, у каждого места — состояние */
const tail = (p, n = 3) => { const a = String(p).replace(/\//g, '\\').split('\\').filter(Boolean); return (a.length > n ? '…\\' : '') + a.slice(-n).join('\\'); };
function trail(it) {
  const d = C.cur(), r = readyOf(it), rel = String(it.rel).replace(/\//g, '\\'), out = outRel(it).replace(/\//g, '\\');
  const readyState = pending(it.index) ? 'есть правки' : r ? 'сохранено' : 'ещё нет';
  const readerState = !d.connected ? 'не подключена' : !r ? null : r.onReader === 'same' ? 'такая же' : r.onReader === 'diff' ? 'другая' : 'ещё нет';
  const line = (label, state, path) => h('div', {}, h('span', {}, label, state ? h('em', {}, state) : null), h('code', { title: path }, tail(path)));
  return h('div', { class: 'trail' },
    line('Скачанное', null, `${S.root}\\${rel}`),
    line('Готовое', readyState, `${outDir()}\\${out}`),
    line('На читалке', readerState, `${d.readerDirs.books}\\${out}`.replace(/\//g, '\\')));
}
function inspector() {
  const it = S.items[S.sel];
  if (!it) return h('aside', { class: 'panel inspector' }, h('h2', {}, 'Книга'), h('p', { class: 'hint', style: { margin: 0 } }, 'Выберите книгу в списке. Двойной щелчок — подробное окно: данные, обложка, текст.'));
  const b = book(it);
  const big = coverEl(it, 330, 'big-cover');
  return h('aside', { class: 'panel inspector' },
    h('div', {}, h('h2', { style: { margin: 0 } }, b.title),
      h('p', { class: 'hint', style: { margin: '2px 0 0' } }, [b.author, b.series ? `${b.series}${b.number ? ', книга ' + b.number : ''}` : null, `${it.format}, ${size(it.size)}`].filter(Boolean).join(' · '))),
    it.error ? h('p', { class: 'hint', style: { color: 'var(--err)', margin: 0 } }, it.error) : null,
    // кнопки — одинаково с «Готово для читалки»: «Открыть книгу» и «⋯»
    it.error ? null : trail(it),
    h('div', { class: 'insp-btns' }, h('button', { class: 'btn grow', onclick: () => openBook(it.index) }, h('span', { html: icon('books') }), 'Открыть книгу'),
      menuButton('', [{ label: 'Показать в папке', hint: 'ваш файл в проводнике', onClick: () => openPath(`${S.root}/${it.rel}`.replace(/\//g, '\\')).catch(e => toast(String(e))) },
        it.error ? null : { label: 'Сохранить только её', onClick: () => run({ list: [it] }) }].filter(Boolean), { cls: 'quiet icon-only', icon: 'more', title: 'Ещё' })),
    big,
    it.error ? null : coverPicker(it));
}

/* ---------------------------------------------------------------- готовая книга: как она записана для читалки */
function readyInfo(path) {
  S.rinfo = S.rinfo || {};
  if (S.rinfo[path] === undefined) { S.rinfo[path] = null; call('books_file_info', { path }).then(v => { S.rinfo[path] = v; render(); }).catch(e => { S.rinfo[path] = { err: String(e.message || e) }; render(); }); }
  return S.rinfo[path];
}
function readyInspector() {
  const f = (S.ready || []).find(x => x.path === S.readyCur);
  if (!f) { S.readyCur = null; return inspector(); }
  const v = readyInfo(f.path), src = byOut().get(String(f.rel).replace(/\\/g, '/').toLowerCase());
  return h('aside', { class: 'panel inspector' },
    h('div', {}, h('h2', { style: { margin: 0 } }, (v && v.title) || f.rel.split(/[\\/]/).pop()),
      h('p', { class: 'hint', style: { margin: '2px 0 0' } }, [v && v.author, v && v.series ? `${v.series}${v.number ? ', книга ' + v.number : ''}` : null, v && v.format, size(f.size)].filter(Boolean).join(' · '))),
    v && v.err ? h('p', { class: 'hint', style: { color: 'var(--err)', margin: 0 } }, v.err) : null,
    h('div', { class: 'insp-btns' }, h('button', { class: 'btn grow', disabled: !(v && v.parts), onclick: () => openReady(f) }, h('span', { html: icon('books') }), 'Открыть книгу'),
      menuButton('', [src ? { label: 'Изменить', hint: 'обложку, название, текст — в «Скачанном»', onClick: () => editFromReady(f) } : null,
        { label: 'Показать в папке', onClick: () => openPath(f.path).catch(e => toast(String(e))) }].filter(Boolean), { cls: 'quiet icon-only', icon: 'more', title: 'Ещё' })),
    !v ? h('div', { class: 'big-cover' }, '…') : v.cover ? h('span', { class: 'big-cover has' }, h('img', { src: v.cover, alt: '' })) : h('span', { class: 'big-cover' }, 'без обложки'),
    h('p', { class: 'hint', style: { margin: 0 } }, 'Готовая книга — так она уйдёт на читалку. Изменить обложку и данные — «⋯ → Изменить».'));
}
/** просмотр готовой книги (только чтение) — те же части, что в окне книги */
async function openReady(f) {
  const back = h('div', { class: 'sheet-back' }), sheet = h('section', { class: 'sheet book-sheet ready-sheet', role: 'dialog', 'aria-label': 'Готовая книга' });
  const R = { parts: null, part: null, view: null, err: '' };
  const close = () => { removeEventListener('keydown', key); back.classList.remove('open'); sheet.classList.remove('open'); setTimeout(() => { back.remove(); sheet.remove(); }, 280); };
  const key = e => { if (e.key === 'Escape') close(); };
  back.onclick = close; document.body.append(back, sheet); addEventListener('keydown', key);
  requestAnimationFrame(() => { back.classList.add('open'); sheet.classList.add('open'); });
  S.rinfo = S.rinfo || {};
  if (!S.rinfo[f.path]) { try { S.rinfo[f.path] = await call('books_file_info', { path: f.path }); } catch (e) { S.rinfo[f.path] = { err: String(e.message || e) }; } }
  const v = S.rinfo[f.path] || {};
  const draw = () => fill(sheet,
    h('header', { class: 'sheet-head' }, h('h2', {}, v.title || f.rel.split(/[\\/]/).pop()), h('span', { class: 'chip ok' }, 'готовая книга'), h('span', { class: 'grow' }),
      h('button', { class: 'btn quiet', 'aria-label': 'Закрыть', onclick: close, html: icon('close') })),
    h('div', { class: 'book-body' }, R.err ? h('div', { class: 'book-pad' }, h('p', { class: 'hint', style: { color: 'var(--err)' } }, R.err))
      : !R.parts ? loading('Читаю книгу…', 'разбираю на разделы', 'read')
      : h('div', { class: 'book-text' },
        h('nav', { class: 'parts' }, R.parts.map((p, k) => h('button', { 'aria-current': R.part === p.id, onclick: () => show(p.id) }, h('b', {}, p.title || `Часть ${k + 1}`), h('small', {}, `${p.chars} ${plural(p.chars, 'знак', 'знака', 'знаков')}`)))),
        h('div', { class: 'part-view' }, !R.view ? loading('Читаю раздел…', '') : h('iframe', { class: 'part-frame', sandbox: '', title: 'Текст части', srcdoc: page(R.view.html, false) })))),
    h('footer', { class: 'sheet-foot' }, h('div', { class: 'sub-line', title: f.path }, `Готовая книга: ${f.rel} · ${size(f.size)}`),
      h('div', { class: 'row-btns' }, h('button', { class: 'btn primary', onclick: close }, 'Закрыть'))));
  const show = async id => { R.part = id; R.view = null; draw(); try { R.view = await call('books_file_part', { path: f.path, id }); } catch (e) { R.err = String(e.message || e); } draw(); };
  draw();
  try { R.parts = await call('books_file_parts', { path: f.path }); draw(); if (R.parts.length) show(R.parts[0].id); } catch (e) { R.err = String(e.message || e); draw(); }
}

function actions() {
  return h('div', { class: 'actions' },
    outLine(outDir()),
    S.busy === 'run' ? h('div', { class: 'run-prog' }, h('small', {}, `${Object.values(S.status).filter(x => x.ok != null).length} из ${S.runTotal}`),
      h('div', { class: 'progress' }, h('i', { style: { width: `${100 * progress()}%` } }))) : null,
    S.busy === 'run' ? h('button', { class: 'btn', onclick: () => { S.stop = true; } }, 'Остановить')
      : h('button', { class: 'btn primary', disabled: !!S.busy, onclick: run, title: S.scope ? `Только книги из подпапки ${S.scope}` : null }, S.scope ? `Сохранить «${S.scope.split('/').pop()}»` : 'Сохранить в папку'),
    lock(toQueue(C, 'books', { onSave: run, missing: () => missingNow() }), !!S.busy));
}

/* ================================================================ подробное окно книги */
let B = null; // { index, tab, parts, part, view, editing, draft, err }

// Рабочая область книги: занимает раздел целиком (не всплывающее окно). Слева — список книг для переключения,
// справа — данные и обложка, текст, версии. Живёт рядом со страницей раздела: уходите в другой раздел — она ждёт
// там же, где вы были (вместе с недописанной правкой).
function openBook(index, tab) {
  const it = S.items[index]; if (!it) return;
  S.sel = index; render();
  if (!B) {
    const sheet = h('section', { class: 'book-ws', role: 'region', 'aria-label': 'Книга' });
    (root.parentElement || document.body).append(sheet);
    requestAnimationFrame(() => sheet.classList.add('open'));
    B = { sheet, wsQ: '', key: e => { if (e.key === 'Escape' && B && !B.editing && root.parentElement && !root.parentElement.hidden && !document.querySelector('dialog[open], body > .hist-menu:not([hidden])')) closeBook(); } };
    addEventListener('keydown', B.key);
  }
  Object.assign(B, { index, tab: tab || B.tab || 'about', parts: null, part: null, view: null, editing: false, draft: '', err: '', shownHtml: null, textHtml: null });
  drawBook();
  if (canParts(it)) call('books_parts', { index }).then(p => { if (B && B.index === index) { B.parts = p; drawBook(); } }).catch(e => { if (B) { B.parts = []; B.err = String(e); drawBook(); } });
}
function closeBook() {
  if (!B) return; removeEventListener('keydown', B.key);
  const s = B.sheet; s.classList.remove('open'); setTimeout(() => s.remove(), 200); B = null; render();
}

// список книг слева: те же, что в списке раздела (фильтр, подпапка), плюс свой быстрый поиск
function wsList() {
  const q = (B.wsQ || '').trim().toLowerCase();
  const list = visible().filter(i => !q || [book(i).title, book(i).author].some(x => x && String(x).toLowerCase().includes(q)));
  let t;
  return h('aside', { class: 'ws-list' },
    h('div', { class: 'ws-list-head' },
      h('button', { class: 'btn quiet ws-back', title: 'Вернуться к списку книг (Esc)', onclick: closeBook }, h('span', { html: icon('left') }), 'Все книги'),
      h('input', { class: 'input', type: 'search', placeholder: 'Найти книгу', 'aria-label': 'Найти книгу', value: B.wsQ || '',
        oninput: e => { const v = e.target.value; clearTimeout(t); t = setTimeout(() => { B.wsQ = v; drawBook(); const i = $('.ws-list .input', B.sheet); if (i) { i.focus(); i.setSelectionRange(v.length, v.length); } }, 150); } })),
    h('div', { class: 'ws-items', role: 'listbox', 'aria-label': 'Книги' }, list.slice(0, 600).map(i => h('button', { class: 'ws-item', role: 'option', 'aria-selected': String(i.index === B.index),
      onclick: () => { if (B.editing) { toast('Сначала примите или отмените правку раздела'); return; } openBook(i.index, B.tab); } },
      coverEl(i, 60, 'cov'), h('span', { class: 'nm' }, h('b', {}, book(i).title), h('small', {}, book(i).author || i.format)),
      pending(i.index) ? h('i', { class: 'dot', title: 'есть несохранённые правки' }) : null)),
      list.length ? null : h('p', { class: 'hint', style: { padding: '12px' } }, 'Ничего не нашлось.')));
}

function drawBook() {
  if (!B) return;
  // правка текстом идёт прямо в странице: перед перерисовкой запомнить, что уже набрано
  { const fr = B.sheet && B.sheet.querySelector('.part-frame.editing'); if (B.editing === 'text' && fr && fr.contentDocument && fr.contentDocument.body && fr.contentDocument.body.childNodes.length) B.textHtml = fr.contentDocument.body.innerHTML; }
  const it = S.items[B.index], b = book(it), keep = $('.book-body', B.sheet) ? $('.book-body', B.sheet).scrollTop : 0;
  const lkeep = $('.ws-items', B.sheet) ? $('.ws-items', B.sheet).scrollTop : null;
  const tabs = [['about', 'Данные и обложка'], ['text', 'Текст'], ['versions', 'Версии']];
  fill(B.sheet,
    wsList(),
    h('div', { class: 'ws-main' },
      h('header', { class: 'sheet-head' },
        h('div', { class: 'ws-title' }, h('h2', { title: b.title }, b.title), h('small', {}, [b.author, b.series ? `${b.series}${b.number ? ' #' + b.number : ''}` : null, it.format].filter(Boolean).join(' · '))),
        h('div', { class: 'seg', role: 'tablist' }, tabs.map(([id, t]) => h('button', { role: 'tab', 'aria-pressed': B.tab === id, onclick: () => { if (B.editing) { toast('Сначала примите или отмените правку раздела'); return; } B.tab = id; drawBook(); } }, t))),
        h('div', { class: 'pager' },
          h('button', { class: 'btn', 'aria-label': 'Предыдущая книга', title: 'Предыдущая книга', disabled: B.index <= 0, onclick: () => openBook(B.index - 1, B.tab), html: icon('left') }),
          h('button', { class: 'btn', 'aria-label': 'Следующая книга', title: 'Следующая книга', disabled: B.index >= S.items.length - 1, onclick: () => openBook(B.index + 1, B.tab), html: icon('right') }))),
      h('div', { class: 'book-body' }, B.tab === 'text' ? textTab(it) : B.tab === 'versions' ? versionsTab(it) : aboutTab(it)),
      h('footer', { class: 'sheet-foot' },
        h('div', { class: 'sub-line' }, pending(it.index) ? h('span', { class: 'chip st edit' }, 'есть правки') : edited(it.index) ? h('span', { class: 'chip st done' }, 'правки сохранены') : null,
          h('span', { title: 'Ваш файл не меняется. Правки и обложка попадут в готовую книгу после «Сохранить».' }, `Ваш файл: ${it.rel} · ${size(it.size)}`)),
        h('div', { class: 'row-btns' },
          h('button', { class: 'btn primary book-save', disabled: !!S.busy, title: 'Записать книгу со всеми правками в папку готового (прежняя версия уйдёт в «Бэкапы»)', onclick: saveBook }, 'Сохранить')))));
  const body = $('.book-body', B.sheet); if (body) body.scrollTop = keep;
  const li = $('.ws-items', B.sheet);
  if (li) { if (lkeep != null) li.scrollTop = lkeep; const cur = $('.ws-item[aria-selected="true"]', li); if (cur && (lkeep == null || cur.offsetTop < li.scrollTop || cur.offsetTop > li.scrollTop + li.clientHeight - 60)) cur.scrollIntoView({ block: 'nearest' }); }
}

// «Версии»: путь книги (ваш файл → готовая → читалка) и история версий с действиями
function versionsTab(it) {
  const f = readyOf(it), out = f && f.path ? f : null, d = C.cur();
  const bak = out && S.bak ? S.bak.get(String(out.rel).replace(/\\/g, '/').toLowerCase()) : null;
  const show = p => openPath(p).catch(e => toast(String(e.message || e)));
  const dirty = pending(it.index), onR = out && out.onReader;
  // три шага пути: состояние каждого — готово / нужно действие / ещё нет
  const step = (ic, title, state, note, act) => h('div', { class: 'vp-step ' + state },
    h('span', { class: 'vp-dot', html: icon(state === 'done' ? 'check' : ic) }),
    h('div', { class: 'vp-t' }, h('b', {}, title), h('small', {}, note)), act || null);
  const restore = async () => {
    try { await call('backups_restore_book', { device: d.id, out: outDir(), rel: out.rel }); toast('Прежняя версия возвращена'); await refreshReady(); drawBook(); }
    catch (e) { toast(String(e.message || e)); }
  };
  const ev = (cls, when, title, sub, btns) => h('li', { class: 'vh-item vh-' + cls }, h('span', { class: 'vh-mark' }),
    h('div', { class: 'vh-body' }, h('div', { class: 'vh-head' }, h('b', {}, title), when ? h('time', {}, when) : null), sub ? h('small', {}, sub) : null),
    h('div', { class: 'vh-act' }, ...btns.filter(Boolean)));
  return h('div', { class: 'book-pad vers' },
    h('div', { class: 'vpath' },
      step('books', 'Ваш файл', 'done', `${it.format}, ${size(it.size)} · не меняется`),
      h('span', { class: 'vp-line' + (out ? ' on' : '') }),
      step('folder', 'Готовая книга', out ? (dirty ? 'todo' : 'done') : 'todo', out ? (dirty ? 'есть правки — сохраните заново' : `сохранена ${shortDate(out.mtime)}, ${size(out.size || 0)}`) : 'ещё не сохранена',
        dirty || !out ? h('button', { class: 'btn primary', disabled: !!S.busy, onclick: saveBook }, 'Сохранить') : null),
      h('span', { class: 'vp-line' + (onR === 'same' ? ' on' : '') }),
      step('apply', 'На читалке', onR === 'same' ? 'done' : onR === 'diff' ? 'todo' : 'off',
        !d.connected ? 'читалка не подключена' : onR === 'same' ? 'та же версия, что в готовом' : onR === 'diff' ? 'там другая версия' : out ? 'ещё не отправлена' : '—',
        out && d.connected && onR !== 'same' ? h('button', { class: 'btn apply', onclick: () => { const r = String(out.rel).replace(/\\/g, '/'); Q.addAndGo(C, [{ src: out.path, dst: [d.readerDirs.books, r.split('/').slice(0, -1).join('/')].filter(Boolean).join('/'), kind: 'books', label: r.split('/').pop() }]); } }, 'На читалку') : null)),
    h('h3', { class: 'vh-title' }, 'История'),
    h('ol', { class: 'vh' },
      dirty ? ev('now', 'сейчас', 'Ваши правки', 'ещё не в готовой книге — нажмите «Сохранить»', []) : null,
      out ? ev('cur', shortDate(out.mtime), 'Готовая книга', `${size(out.size || 0)} · так она уйдёт на читалку`,
        [h('button', { class: 'btn quiet', onclick: () => openReady(out) }, 'Открыть'), h('button', { class: 'btn quiet', onclick: () => show(out.path) }, 'В папке')]) : null,
      bak ? ev('old', shortDate(bak.mtime), 'Прежняя версия', `${size(bak.size)} · лежит в «Бэкапах»; вернуть её — а нынешняя станет копией`,
        [h('button', { class: 'btn', onclick: restore }, 'Вернуть'), h('button', { class: 'btn quiet', onclick: () => S.bakDir && show(S.bakDir) }, 'В папке')]) : null,
      ev('src', null, 'Ваш файл', `${it.rel} · ${size(it.size)} · оригинал, программа его не меняет`, [h('button', { class: 'btn quiet', onclick: () => show(it.path) }, 'В папке')])),
    !bak && out ? h('p', { class: 'hint vh-note' }, 'Прежняя версия появится, когда вы сохраните книгу ещё раз с изменениями — одна на книгу, в «Бэкапах».') : null);
}

function aboutTab(it) {
  const b = book(it), e = ed(it.index);
  const field = (k, label, hint) => h('label', { class: 'field' }, h('span', {}, label),
    h('input', { class: 'input', value: b[k] ?? '', disabled: !canMeta(it), oninput: ev => {
      const v = ev.target.value; const orig = k === 'title' ? (it.title || '') : (it[k] ?? '');
      if (String(v) === String(orig ?? '')) delete e.meta[k]; else e.meta[k] = v;
      S.thumbs.clear(); clearTimeout(S.mT); S.mT = setTimeout(() => { render(); const c = $('.book-cover', B && B.sheet); if (c) fill(c, coverEl(it, 330, 'big-cover')); }, 250);
    } }), hint ? h('small', {}, hint) : null);
  return h('div', { class: 'book-about' },
    h('div', { class: 'book-cover' }, coverEl(it, 330, 'big-cover')),
    h('div', { class: 'book-fields' },
      h('div', { class: 'fine-grid' },
        field('title', 'Название'), field('author', 'Автор', 'Как на обложке: имя и фамилия.'),
        field('series', 'Серия'), field('number', 'Номер в серии')),
      !canMeta(it) ? h('p', { class: 'hint', style: { margin: 0 } }, it.error ? 'Книга не читается.' : `В ${it.format} данные книги не меняются — только обложка.`) : null,
      Object.keys(e.meta).length ? h('div', {}, h('button', { class: 'btn quiet', onclick: () => { e.meta = {}; S.thumbs.clear(); render(); drawBook(); } }, 'Вернуть данные как было')) : null,
      it.genres && it.genres.length ? h('p', { class: 'hint', style: { margin: 0 }, title: `Жанры в файле: ${it.genres.join(', ')}` }, `Жанр в файле: ${it.genres.join(', ')}`) : null,
      it.error ? null : coverPicker(it)));
}

function textTab(it) {
  if (!canParts(it)) return h('div', { class: 'book-pad' }, h('p', { class: 'hint' }, it.fmt === 'txt' ? 'TXT превращается в FB2 при сохранении — текст можно будет поправить в готовой книге.' : `Текст ${it.format} здесь не показывается.`));
  if (B.err) return h('div', { class: 'book-pad' }, h('p', { class: 'hint', style: { color: 'var(--err)' } }, B.err));
  if (!B.parts) return loading('Читаю книгу…', 'разбираю на разделы', 'read');
  if (!B.parts.length) return h('div', { class: 'book-pad' }, h('p', { class: 'hint' }, 'В книге не нашлось основного текста.'));
  const e = ed(it.index);
  if (!B.part && B.parts.length) setTimeout(() => openPart(B.parts[0].id));
  const v = B.view, dropped = id => !!e.drop[id], cur = B.part, left = B.parts.filter(p => !dropped(p.id)).length;
  const toggleDrop = id => { if (dropped(id)) delete e.drop[id]; else { if (left <= 1) { toast('Нельзя убрать все разделы книги'); return; } e.drop[id] = true; }
    B.editing = false; render(); drawBook(); };
  const partMenu = (p) => [
    { label: 'Править текст', hint: 'как в обычном текстовом редакторе', disabled: dropped(p.id), onClick: () => openPart(p.id, 'text') },
    { label: 'Править код', hint: v && v.kind === 'fb2' ? 'XML раздела FB2 — для опытных' : 'XHTML главы — для опытных', disabled: dropped(p.id), onClick: () => openPart(p.id, 'code') },
    e.parts[p.id] != null ? { label: 'Вернуть текст как был', onClick: () => { delete e.parts[p.id]; render(); openPart(p.id); } } : null,
    { sep: '' },
    dropped(p.id) ? { label: 'Не удалять раздел', onClick: () => toggleDrop(p.id) } : { label: 'Удалить раздел', hint: 'уберётся из книги при сохранении', onClick: () => toggleDrop(p.id) }];
  const mode = B.editing; // false | 'text' | 'code'
  return h('div', { class: 'book-text' },
    h('nav', { class: 'parts' }, B.parts.map((p, k) => h('div', { class: 'part-row' + (cur === p.id ? ' cur' : '') + (dropped(p.id) ? ' gone' : '') },
      h('button', { class: 'part-pick', 'aria-current': cur === p.id, onclick: () => openPart(p.id) },
        h('b', {}, p.title || `Часть ${k + 1}`),
        h('small', {}, dropped(p.id) ? 'будет удалён' : `${p.chars} ${plural(p.chars, 'знак', 'знака', 'знаков')}${e.parts[p.id] != null ? ', изменён' : ''}`)),
      menuButton('', partMenu(p), { cls: 'quiet icon-only', icon: 'more', title: 'Действия с разделом' })))),
    h('div', { class: 'part-view' },
      !v ? loading('Читаю раздел…', '', 'read') : h('div', { class: 'part-bar' },
        mode ? [
          h('span', { class: 'hint', style: { margin: 0, marginRight: 'auto' } }, mode === 'text' ? 'Правьте прямо в тексте: Enter — новый абзац, Ctrl+B — жирный, Ctrl+I — курсив, Ctrl+Z — отменить.'
            : v.kind === 'fb2' ? 'Код раздела FB2: <section>, <p>, <emphasis>, <strong>…' : 'Код главы XHTML: должен остаться правильным XML'),
          h('button', { class: 'btn', onclick: () => { B.editing = false; B.partErr = ''; drawBook(); } }, 'Отмена'),
          h('button', { class: 'btn primary', onclick: acceptPart }, 'Принять правку')]
        : dropped(cur) ? [
          h('span', { class: 'chip st edit' }, 'будет удалён'), h('span', { style: { marginRight: 'auto' } }),
          h('button', { class: 'btn', onclick: () => toggleDrop(cur) }, 'Не удалять')]
        : [
          e.parts[cur] != null ? h('span', { class: 'chip st edit', title: 'Правка попадёт в книгу при сохранении' }, 'изменён') : null,
          h('span', { style: { marginRight: 'auto' } }),
          h('button', { class: 'btn', onclick: () => openPart(cur, 'text') }, h('span', { html: icon('edit') }), 'Править текст'),
          menuButton('', partMenu(B.parts.find(p => p.id === cur) || { id: cur }), { cls: 'quiet icon-only', icon: 'more', title: 'Ещё: код, удалить раздел' })]),
      v && B.partErr ? h('p', { class: 'hint', style: { color: 'var(--err)', margin: 0 } }, B.partErr) : null,
      !v ? null
        : mode === 'code' ? h('textarea', { class: 'input src-edit', spellcheck: 'false', oninput: ev => { B.draft = ev.target.value; } }, B.draft)
        : mode === 'text' ? (B.textHtml == null ? loading('Готовлю правку', '') : (() => {
            const f = h('iframe', { class: 'part-frame editing', sandbox: 'allow-same-origin', title: 'Правка текста', srcdoc: page(B.textHtml, false) });
            B.frame = f; makeEditable(f); return f; })())
        : h('iframe', { class: 'part-frame' + (dropped(cur) ? ' gone' : ''), sandbox: '', title: 'Текст части', srcdoc: page(B.shownHtml != null ? B.shownHtml : v.html, false) })));
}
// страница для просмотра части: без скриптов (iframe sandbox), шрифт и поля как у читалки
const page = (html, changed) => `<!doctype html><meta charset="utf-8"><style>body{font:17px/1.55 Georgia,serif;max-width:40em;margin:24px auto;padding:0 24px;color:#1b1b1a;background:#fbfaf6;outline:none}
img{max-width:100%;height:auto}h1,h2,h3,h4{font-family:"Segoe UI",sans-serif;line-height:1.25}blockquote.ep{font-style:italic;margin-left:30%}.v{margin:0}.stanza{margin:1em 0}.ta{text-align:right;font-style:italic}u[data-pb-open]{color:#2b5ea8}
.pb-note{font:13px "Segoe UI",sans-serif;background:#efeee6;border-radius:8px;padding:8px 12px;color:#55552a}</style>${changed ? '<p class="pb-note">Показан исходный текст. Правка примется при сохранении книги — её видно в окне правки.</p>' : ''}${html}`;

// открыть раздел: просто посмотреть, или сразу править — текстом ('text') или кодом ('code')
async function openPart(id, mode = false) {
  if (!B) return;
  const index = B.index;
  if (B.part !== id || !B.view) {
    Object.assign(B, { part: id, view: null, editing: false, partErr: '', shownHtml: null, textHtml: null }); drawBook();
    try { const v = await call('books_part', { index, id }); if (!B || B.index !== index || B.part !== id) return; B.view = v; }
    catch (e) { if (B) { B.partErr = String(e); drawBook(); } return; }
  }
  const e = ed(index), src = e.parts[id];
  // уже поправленный раздел показываем таким, каким он станет
  B.shownHtml = null;
  if (src != null) { try { B.shownHtml = await call('books_render_part', { index, id, source: src }); } catch (er) { B.shownHtml = null; } }
  B.editing = mode; B.partErr = '';
  if (mode === 'code') B.draft = src ?? B.view.source;
  if (mode === 'text') { B.textHtml = B.shownHtml != null ? B.shownHtml : B.view.html; }
  drawBook();
}
async function acceptPart() {
  const index = B.index, id = B.part, e = ed(index), base = e.parts[id] ?? B.view.source;
  let src;
  try {
    if (B.editing === 'text') {
      const fr = B.sheet.querySelector('.part-frame.editing') || B.frame, d = fr && fr.contentDocument; if (!d || !d.body) throw new Error('Правка не открылась');
      src = B.view.kind === 'fb2' ? fb2FromPage(d, base) : epubFromPage(d, base);
    } else src = B.draft;
    await call('books_check_part', { index, id, source: src });
    if (src === B.view.source) delete e.parts[id]; else e.parts[id] = src;
    B.editing = false; B.partErr = ''; render(); await openPart(id); toast('Правка принята — попадёт в книгу при сохранении');
  } catch (er) { B.partErr = String(er.message || er); drawBook(); }
}

/* ================================================================ действия */
async function scan(path, quiet) {
  closeBook();
  // правки книг (название, автор, текст) переживают «Обновить» той же папки — переносятся по пути книги
  const keep = path === S.root ? new Map((S.items || []).filter(i => S.ed[i.index]).map(i => [i.rel, S.ed[i.index]])) : new Map();
  const selRel = path === S.root && S.items && S.items[S.sel] ? S.items[S.sel].rel : null;
  Object.assign(S, { root: path, busy: 'scan', gen: (S.gen || 0) + 1, scanP: null, ed: {}, status: {}, sel: -1, own: {}, ownQ: [], limit: 300 });
  S.thumbs.clear(); render();
  try {
    const r = await call('books_scan', { args: { path, txt: S.txt } });
    S.items = r.items; S.skipped = r.skipped; S.total = r.total; S.sel = r.items.length ? 0 : -1;
    for (const it of S.items) { const e = keep.get(it.rel); if (e) S.ed[it.index] = e; if (selRel && it.rel === selRel) S.sel = it.index; }
    save(); C.remember('books', path);
  } catch (e) { if (!quiet) toast(String(e)); S.items = []; S.sel = -1; }
  if (S.scope && !S.items.some(i => inScope(dirOf(i), S.scope))) S.scope = '';
  S.busy = false; render();
  refreshReady();
}
/** что уже сохранено и что лежит в папке готового */
async function refreshReady() {
  // сначала что лежит в готовом — от этого зависят имена готовых файлов, по ним — что уже сохранено
  const out = outDir(), items = S.items;
  const [ready] = await Promise.all([call('ready_list', { dir: out, exts: [], reader: readerPath(C, 'books') }).catch(() => []), loadBackups()]);
  if (items !== S.items) return;
  S.ready = ready; S._plan = null;
  const done = await call('books_done', { out, names: S.items.map(it => outRel(it).split('/').pop()) }).catch(() => []);
  if (items !== S.items) return;
  S.done = new Set(done); S._plan = null;
  if (root && root.isConnected) render();
}
// «Бэкапы»: прежние версии готовых книг (путь в готовом → копия)
async function loadBackups() {
  try { const r = await call('backups_list', { device: C.cur().id }); S.bak = new Map((r.books || []).map(b => [String(b.rel).replace(/\\/g, '/').toLowerCase(), b])); S.bakDir = r.dir; }
  catch (e) { S.bak = new Map(); }
}
function backupMenu(f) {
  const b = S.bak && S.bak.get(String(f.rel).replace(/\\/g, '/').toLowerCase());
  if (!b) return [];
  return [{ label: 'Вернуть прежнюю версию', hint: `сохранённую ${shortDate(b.mtime)}; эта станет копией — можно вернуть обратно`, onClick: async () => {
    try { await call('backups_restore_book', { device: C.cur().id, out: outDir(), rel: f.rel }); toast('Прежняя версия возвращена'); await refreshReady(); }
    catch (e) { toast(String(e.message || e)); } } }];
}
// готовая книга → исходная (по пути в папке готового)
// готовая книга → исходная (по пути; у TXT — .fb2). Одна таблица на список книг: в «Готовом» её спрашивает каждая строка
const byOut = () => { const p = outPlan(); if (S._outOf !== p) { S._outOf = p; S._out = new Map(S.items.map(it => [readyKey(it), it])); } return S._out; };
// готовое, которого нет на читалке (только если читалка подключена и сверка была)
const missingNow = () => C.cur().connected && S.ready.length && S.ready.every(f => f.onReader != null) ? S.ready.filter(f => f.onReader !== 'same' && (!S.scope || inScope(String(f.rel).replace(/\\/g, '/').split('/').slice(0, -1).join('/'), S.scope))) : null;
const fresh = () => scoped().filter(i => !i.error && !S.done.has(i.index));

function select(i) { if (S.sel === i) return; S.sel = i; render(); }

/** своя картинка → JPEG ровно того размера, в каком загружена (без обрезки и уменьшения); JPEG без изменений — как есть */
async function imageJpeg(url, W, H, gray) {
  if (!gray && /^data:image\/jpe?g;base64,/i.test(url)) return url.split(',')[1];
  const img = await new Promise((res, rej) => { const i = new Image(); i.onload = () => res(i); i.onerror = rej; i.src = url; });
  const c = document.createElement('canvas'); c.width = img.naturalWidth; c.height = img.naturalHeight;
  const g = c.getContext('2d');
  g.fillStyle = '#fff'; g.fillRect(0, 0, c.width, c.height); // прозрачное (PNG) — на белом
  g.drawImage(img, 0, 0);
  if (gray) { const d = g.getImageData(0, 0, c.width, c.height), p = d.data; for (let i = 0; i < p.length; i += 4) p[i] = p[i + 1] = p[i + 2] = 0.299 * p[i] + 0.587 * p[i + 1] + 0.114 * p[i + 2]; g.putImageData(d, 0, 0); }
  return c.toDataURL('image/jpeg', 0.92).split(',')[1];
}

/** одна книга со всеми правками (данные, текст, обложка, ссылки → текст) → в папку готового */
async function processOne(it, out, cv) {
  const { W, H, gray } = screen();
  const e = S.ed[it.index] || { meta: {}, parts: {} }, p = plan(it);
  const args = { index: it.index, device: C.cur().id, out, unlink: !!S.unlink, cover: null, name: outRel(it).split('/').pop(),
    meta: e.meta, parts: [...Object.entries(e.parts).filter(([id]) => !(e.drop || {})[id]).map(([id, source]) => ({ id, source })),
      ...Object.keys(e.drop || {}).map(id => ({ id, delete: true }))],
    // книги с правками сохраняются всегда; без правок — пропуск, если готовая уже есть
    skip_done: S.skipDone && !pending(it.index) && !lookChanged(it) && !(S.force && S.force.has(it.index)) };
  if (p === 'draw') { const b = book(it), look = CV.resolve(b, cfg(), gray); args.cover = CV.jpeg(CV.draw(b, look.style, look.color, W, H, gray, cv || document.createElement('canvas'))); }
  if (p === 'image') { const src = e.image || await call('read_image', { path: imgPath(it) }).then(r => `data:${r.mime};base64,${r.data}`); args.cover = await imageJpeg(src, W, H, gray); }
  const r = await call('books_process', { args });
  // под каким именем книга лежит в готовом — дальше это имя за ней и остаётся
  if (r.rel) { const k = imgKey(it), rel = nrmRel(r.rel); if ((S.outNames || {})[k] !== rel) { S.outNames = Object.assign({}, S.outNames, { [k]: rel }); save(); } }
  // запомнить, с какой обложкой сохранено (для «Только новые»)
  if (!r.skipped && edited(it.index)) S.edSaved = Object.assign({}, S.edSaved, { [edKey(it.index)]: edSig(it.index) });
  if (!r.skipped) { const sg = lookSig(it); const m = Object.assign({}, S.looks); if (sg) m[imgKey(it)] = sg; else delete m[imgKey(it)]; S.looks = m; save(); }
  return r;
}
/** «Сохранить» в окне книги: правки сразу записываются в готовую книгу */
async function saveBook() {
  if (!B) return;
  if (B.editing) { toast('Сначала примените или отмените правку раздела'); return; }
  const it = S.items[B.index], btn = $('.book-save', B.sheet);
  if (btn) { btn.disabled = true; btn.textContent = 'Сохраняю…'; }
  try {
    const r = await processOne(it, outDir());
    S.status[it.index] = { ok: true, now: true, note: r.links ? `ссылок стало текстом: ${r.links}` : '' };
    toast(`Сохранено: «${book(it).title || it.name}» → ${outDir().split(/[\\/]/).slice(-2).join('\\')}`);
    closeBook(); render(); refreshReady();
  } catch (e) { toast(String(e)); if (btn) { btn.disabled = false; btn.textContent = 'Сохранить'; } }
}

async function run({ onlyNew = false, list = null } = {}) {
  // list — только выбранные: их сохраняем всегда (даже если уже готовы)
  const todo = list ? list.filter(it => !it.error) : (onlyNew ? fresh() : scoped().filter(it => !it.error));
  S.force = list ? new Set(todo.map(it => it.index)) : null;
  if (!todo.length) return;
  S.busy = 'run'; S.stop = false; S.status = {}; S.runTotal = todo.length; render();
  const out = outDir();
  const cv = document.createElement('canvas');
  let ok = 0, fail = 0, links = 0, skip = 0;
  for (const it of todo) {
    if (S.stop) break;
    S.status[it.index] = { text: 'сохраняю…' }; later();
    try {
      const r = await processOne(it, out, cv);
      if (r.skipped) { skip++; S.status[it.index] = { ok: true, skipped: true }; later(); continue; } // уже было сохранено — метка покажет, где оно
      links += r.links || 0; ok++;
      S.status[it.index] = { ok: true, now: true, note: r.links ? `ссылок стало текстом: ${r.links}` : '' };
    } catch (e) { fail++; S.status[it.index] = { ok: false, text: String(e).slice(0, 60) }; }
    later();
    await new Promise(r => setTimeout(r)); // дать окну перерисоваться
  }
  const stopped = S.stop;
  // сохранили — показать, что теперь в папке готового
  if (ok && !list) S.view = 'ready';
  if (list && S.srcSel) { S.srcSel.clear(); S.srcMulti = false; }
  S.busy = false; render(); refreshReady();
  toast(!ok && skip && !fail ? `Сохранять нечего: ${skip} ${plural(skip, 'книга уже сохранена', 'книги уже сохранены', 'книг уже сохранены')}, правок и новых нет` : `Сохранено ${ok} ${plural(ok, 'книга', 'книги', 'книг')}${skip ? `, уже готовых пропущено ${skip}` : ''}${fail ? `, с ошибками ${fail}` : ''}${links ? `, ссылок стало текстом: ${links}` : ''}${stopped ? ', остановлено' : ''}`);
  if (ok && C.behavior('openFolderAfterSave')) openPath(out).catch(() => {});
}

/* ================================================================ категории */
async function categories() {
  let list = structuredClone(S.categories);
  const r = await dialog(close => {
    const body = h('div', { class: 'cat-list' });
    const draw = () => fill(body, list.map((c, i) => h('div', { class: 'cat-row' },
      h('input', { type: 'color', class: 'cat-color', value: c.color || '#3A3A37', 'aria-label': `Цвет: ${c.name}`, title: 'Цвет обложки', oninput: e => { c.color = e.target.value.toUpperCase(); } }),
      h('input', { class: 'input', value: c.name, 'aria-label': 'Название', placeholder: 'Название', oninput: e => { c.name = e.target.value; } }),
      h('select', { 'aria-label': 'Вид обложки', title: 'Вид обложки', onchange: e => { c.style = e.target.value; } }, h('option', { value: '' }, 'вид как у всех'), CV.STYLES.map(([n]) => h('option', { value: n, selected: c.style === n }, n))),
      h('button', { class: 'btn quiet icon-only', 'aria-label': `Удалить «${c.name}»`, title: 'Удалить категорию', html: icon('close'), onclick: () => { list.splice(i, 1); draw(); } }),
      h('textarea', { class: 'input', rows: 2, 'aria-label': `Какие книги: ${c.name}`, placeholder: 'жанр: фэнтези*', oninput: e => { c.rules = e.target.value; } }, c.rules))));
    draw();
    return h('div', { class: 'dlg dlg-wide' },
      h('h2', {}, 'Цвета обложек по жанрам'),
      h('p', { class: 'hint', style: { margin: 0 } }, 'Книга берёт цвет и вид первой подходящей строки сверху. Какие книги подходят — по одному правилу в строке: «жанр: фэнтези*», «папка: Детективы», «автор: Пратчетт», «серия: Плоский мир». Звёздочка — любые буквы.'),
      body,
      h('div', { class: 'dlg-foot' },
        h('button', { class: 'btn quiet', style: { marginRight: 'auto' }, title: 'Вернуть категории, которые были в программе изначально', onclick: () => { list = structuredClone(CV.DEFAULT_CATEGORIES); draw(); } }, 'Как было'),
        h('button', { class: 'btn', onclick: () => { list.push({ name: 'Новая', color: '#3A3A37', style: '', rules: 'жанр: ' }); draw(); body.scrollTop = 1e6; } }, h('span', { html: icon('plus') }), 'Добавить'),
        h('button', { class: 'btn', onclick: () => close(null) }, 'Отмена'),
        h('button', { class: 'btn primary', onclick: () => close(list) }, 'Сохранить')));
  });
  if (r) { S.categories = r.filter(c => c.name.trim()); save(); S.thumbs.clear(); render(); }
}
