// Раздел «Комиксы» (и манга): папка → режим сжатия → сколько освободится → сохранить в папку читалки → загрузить на неё.
// Манга и комиксы — в одном списке, по сериям. Что серия — манга или комикс, видно по её папке («Манга», «Comics»…)
// или по отметке; готовое ложится в папку читалки Manga или Comics (PB743K3\Manga), оттуда — на читалку в Manga/Comics.
import { call, on, pickFolder, openPath } from '../api.js';
import { $, $$, h, fill, inch, size, plural, pct, toast, dialog, icon, busy, lock, menuButton, progress, loading, mzEmpty } from '../ui.js';
import { flowBar, drop, toQueue, scopeSelect, inScope, freshBar, readyView, readerPath, listTools, selBar, rowEl, stateChip } from '../upload.js';
import { whereDrawer } from '../where.js';
import { optRow, ctlRow } from '../help.js';
import * as Queue from '../queue.js';

const DEF = { root: '', scope: '', recurse: true, rename: true, template: '', myTemplates: [], tplByDir: {}, skipDone: true, mode: 'balanced', opt: null, defKind: 'manga', kinds: {} };
const KINDS = ['manga', 'comics'];
const KIND_NAME = { manga: 'Манга', comics: 'Комикс' };
let S, C, root, unsub = [];

function save() {
  const v = Object.fromEntries(Object.keys(DEF).map(k => [k, S[k]]));
  if (S.wsOld) v.ws = S.wsOld; // прежние папки «Манги» и «Комиксов» — не теряются
  C.set('comics', v).catch(() => {});
}
// профиль читалки: текущая или другая сохранённая (для сравнения в предпросмотре)
const reader = id => { const d = C.device(id || S.reader); return d && d.reader; };
// шаблон названий для выбранной подпапки: свой (из tplByDir, у ближайшей папки) или общий
const nrm = d => String(d || '').replace(/\\/g, '/').replace(/^\/+|\/+$/g, '');
function tplNow(dir = S.scope) {
  const m = S.tplByDir || {}, d = nrm(dir);
  const k = Object.keys(m).filter(x => d === nrm(x) || d.startsWith(nrm(x) + '/')).sort((a, b) => b.length - a.length)[0];
  return k != null ? m[k] : S.template;
}
const inScopeIdx = () => S.items.map((it, i) => [it, i]).filter(([it]) => inScope(it.rel, S.scope)).map(([, i]) => i);
function setScope(v) { S.scope = v; const ids = inScopeIdx(); if (!ids.includes(S.sel)) S.sel = ids[0] ?? 0; save(); render(); }
// готовое — в папку читалки (Comics или Manga), оригиналы при замене на месте — в папку из настроек или «_backup»
const folders = (k = 'manga') => ({ out_dir: C.cur().dirs[k], backup_dir: ((C.config.paths || {}).comicsBackup || '') });

/* ---------- серии: тома одной папки с одним названием серии */
const stem = n => String(n || '').replace(/\.[^.]+$/, '');
const serKey = it => `${nrm(it.rel).toLowerCase()}|${String(it.series || stem(it.newName)).toLowerCase()}`;
const serTitle = it => it.series || nrm(it.rel).split('/').pop() || stem(it.newName);
// манга или комикс — по названию папок; непонятно — как в настройках
function guessKind(rel) {
  const p = nrm(rel).toLowerCase();
  if (/манг|manga|манхв|manhwa|маньхуа|manhua/.test(p)) return 'manga';
  if (/комикс|comic/.test(p)) return 'comics';
  return S.defKind === 'comics' ? 'comics' : 'manga';
}
const kindOfKey = (key, rel) => (S.kinds || {})[key] || guessKind(rel);
const kindOf = it => kindOfKey(serKey(it), it.rel);
/** серии в порядке томов: [{ key, title, rel, kind, idx, size }] */
function seriesList() {
  if (S._ser && S._serOf === S.items && S._serK === S.kindsV) return S._ser;
  const m = new Map();
  S.items.forEach((it, i) => { const k = serKey(it); let g = m.get(k);
    if (!g) m.set(k, g = { key: k, title: serTitle(it), rel: nrm(it.rel), kind: kindOf(it), idx: [], size: 0 });
    g.idx.push(i); g.size += it.size || 0; });
  S._serOf = S.items; S._serK = S.kindsV; return (S._ser = [...m.values()]);
}
function setKind(g, k) {
  const kinds = Object.assign({}, S.kinds);
  if (k === guessKind(g.rel)) delete kinds[g.key]; else kinds[g.key] = k;
  const had = S.ready.filter(f => f.kind === g.kind && g.idx.some(i => relOf(S.items[i]) === nrm(f.rel).toLowerCase())).length;
  S.kinds = kinds; S.kindsV = (S.kindsV || 0) + 1; S.status = {}; save(); render(); refreshReady();
  if (had) toast(`Серия «${g.title}» теперь ${k === 'manga' ? 'манга' : 'комикс'}. Сохраните её заново; прежние готовые тома (${had}) можно убрать в «Готовом» — на читалку они не уйдут.`);
}

export default {
  id: 'comics', title: 'Комиксы', hint: 'манга и комиксы: сжать и навести порядок', icon: 'comics',
  // вернулись в раздел — обновить «Готовое» (могли сохранить или удалить файлы)
  show() { if (S && S.items.length && !S.busy) refreshReady(); },
  async mount(main, ctx) {
    C = ctx;
    const cfg = ctx.config.comics || {};
    S = Object.assign(structuredClone(DEF), cfg, { reader: ctx.cur().id, busy: false, view: 'src', done: new Set(), ready: [], items: [], total: 0, sel: 0, est: null, perItem: {},
      modeEst: null, status: {}, stale: false, staleModes: false, open: new Set(), kfilter: 'all', kindsV: 0 });
    S.kinds = Object.assign({}, cfg.kinds || {});
    S.wsOld = cfg.ws || null;
    // раньше «Манга» и «Комиксы» были двумя папками: берём ту, что была открыта; другая — в «Папка → Недавние»
    if (!cfg.root && cfg.ws) {
      const k = cfg.kind === 'comics' ? 'comics' : 'manga', o = k === 'manga' ? 'comics' : 'manga';
      const a = cfg.ws[k] || {}, b = cfg.ws[o] || {}, w = a.root ? a : b;
      S.root = w.root || ''; S.scope = w.scope || ''; S.mode = w.mode || 'balanced'; S.opt = w.opt || null;
      if (cfg.defKind == null) S.defKind = a.root ? k : o;
      if (a.root && b.root && a.root !== b.root) { await ctx.remember('comics', b.root).catch(() => {}); S.migrated = b.root; }
      save();
    }
    await call('comics_use', { kind: 'manga' }).catch(() => null);
    if (!S.opt || S.opt.reader !== S.reader) S.opt = await call('comics_mode_opt', { reader: S.reader, mode: S.mode === 'custom' ? 'balanced' : S.mode });
    // сменилась текущая читалка — пересчитать под неё; подключили/отключили — обновить кнопки
    unsub.push(ctx.onDevices(async () => { if (!root || !root.isConnected) return; if (ctx.cur() && ctx.cur().id !== S.reader) await setReader(ctx.cur().id); else { render(); if (S.items.length && !S.busy) refreshReady(); } }));
    unsub.push(Queue.onChange(() => { if (root && root.isConnected && S.view !== 'src') render(); }));
    root = h('div', { class: 'page' }); main.append(root);
    unsub.push(await on('comics://estimate', onEstimate), await on('comics://progress', onProgress));
    render();
    if (S.root) scan(S.root, true);
    if (S.migrated) toast(`Манга и комиксы теперь в одном списке. Вторая папка (${S.migrated.split(/[\\/]/).filter(Boolean).pop()}) — в «Папка → Недавние».`);
  },
  // перетащили папку в окно
  drop(paths) { if (root && root.isConnected) scan(paths[0]) },
  // из приветствия: сразу окно выбора папки
  pick() { if (S && !S.busy) choose(); },
  hover(on_) { const d = root && $('.drop', root); if (d) d.classList.toggle('hot', on_) },
  unmount() { unsub.forEach(u => typeof u === 'function' && u()); unsub = []; call('job_cancel', { id: 'estimate' }); },
};

/* ================= отрисовка */
function render() {
  const keep = ['.blist', '.side'].map(sel => [sel, $(sel, root) ? $(sel, root).scrollTop : 0]);
  fill(root,
    S.items.length || S.root ? flow() : drop('Перетащите папку с мангой и комиксами', 'или выберите её — вложенные папки тоже найдутся. Манга и комиксы различаются по папкам', choose, C.srcFor('comics') || C.srcFor('manga'), p => scan(p)),
    S.busy === 'scan' && !S.items.length ? h('section', { class: 'panel' }, progress({ stage: 'Сканирование', detail: S.root || '' })) : null,
    // при сканировании — одна плашка на весь блок вместе с кнопками
    S.items.length ? busy(h('div', { class: 'stack busy-block' }, work(), S.busy === 'scan' ? actions() : null),
      S.busy === 'scan' ? 'Сканирование…' : null) : null,
    S.items.length && S.busy !== 'scan' ? actions() : null,
  );
  for (const [sel, top] of keep) { const el = $(sel, root); if (el) el.scrollTop = top; }
}

async function choose() { const p = await pickFolder('Папка с мангой и комиксами', C.srcFor('comics') || C.srcFor('manga') || undefined); if (p) scan(p); }

/** «Скачанное → Готовое → На читалке»; готовое манги и комиксов — в двух папках, на читалке — в Manga и Comics */
function flow() {
  const d = C.cur(), conn = !!d.connected, checked = conn && S.ready.length && S.ready.every(f => f.onReader != null);
  const on = S.ready.filter(f => f.onReader === 'same').length;
  const t = tplNow();
  const tpl = h('button', { class: 'btn', disabled: !S.rename, title: `Как называть файлы${S.scope ? ` в «${S.scope}»` : ''}: ${tplHuman(t)}${t ? ` (${t})` : ''}`, onclick: templateDialog },
    (S.scope && S.tplByDir && S.tplByDir[S.scope] != null ? 'своё: ' : '') + (t ? tplHuman(t) : 'как в файле'), '…');
  const n = seriesList().length;
  const parent = p => String(p || '').split(/[\\/]/).slice(0, -1).join('\\');
  return flowBar(C, { kind: 'comics', title: 'Папка с мангой и комиксами', view: S.view, onView: v => { S.view = v; S.readyCur = null; render(); },
    src: { path: S.root, count: S.items.length, fresh: S.ready.length ? freshIdx().length : 0,
      tip: S.items.length ? `${S.root}\n${n} ${plural(n, 'серия', 'серии', 'серий')}, ${S.items.length} ${plural(S.items.length, 'том', 'тома', 'томов')}, ${size(S.total)}${renamedCount() ? `, у ${renamedCount()} поменяется название` : ''}` : S.root },
    // обе папки готового рядом (обычно PB743K3\\Manga и PB743K3\\Comics) — показать короче
    ready: { path: parent(d.dirs.manga) === parent(d.dirs.comics) ? `${parent(d.dirs.manga)}\\${d.dirs.manga.split(/[\\/]/).pop()}, ${d.dirs.comics.split(/[\\/]/).pop()}` : `${d.dirs.manga} · ${d.dirs.comics}`,
      count: S.ready.length, wait: checked ? S.ready.length - on : 0 },
    reader: { where: `${d.readerDirs.manga}, ${d.readerDirs.comics}`, count: conn ? on : null },
    onPick: p => scan(p), onMap: () => whereDrawer(C, 'comics'),
    scope: S.items.length ? scopeSelect(S.items.map(x => x.rel), S.scope, setScope, S.items.length) : null,
    locked: !!S.busy || !!S.estimating, progress: S.busy === 'scan' ? -1 : null,
    settings: { key: 'comics', summary: [S.skipDone ? 'только новые' : 'все заново', S.rename ? (t ? tplHuman(t) : 'имена как в файле') : 'имена как есть'].join(', '), groups: [
      ['Сохранение', [optRow('skipDone', S.skipDone, v => { S.skipDone = v; save(); render(); }),
        optRow('recurse', S.recurse, v => { S.recurse = v; scan(S.root); })]],
      ['Манга или комиксы', [ctlRow('defKind', h('div', { class: 'seg' }, KINDS.map(k => h('button', { 'aria-pressed': S.defKind === k,
        onclick: () => { S.defKind = k; S.kindsV++; save(); render(); refreshReady(); } }, k === 'manga' ? 'Манга' : 'Комиксы'))))]],
      ['Названия файлов', [optRow('rename', S.rename, v => { S.rename = v; scan(S.root); }),
        ctlRow('names', tpl, { disabled: !S.rename })]]] } });
}
const renamedCount = () => S.items.filter(i => i.newName !== i.name).length;

const quiet = el => { el.classList.add('quiet'); return el; };
function work() {
  const est = S.estimating && !S.busy;
  // во время поиска старый список остаётся, но размыт и недоступен; во время сохранения блокируются настройки
  // как у книг: список — слева и во всю высоту, справа — итог (сколько освободится) и режимы сжатия
  // при пересчёте размыты режимы и список (плашка «Пересчёт» — на списке), а итог с цифрами виден и растёт
  return h('div', { class: 'split' },
    est ? busy(listPanel(), `Пересчёт… ${S.est ? S.est.done : 0} из ${S.items.length}`) : lock(listPanel(), S.busy === 'run'),
    h('div', { class: 'stack side' }, serCard(), shelfPanel(),
      est ? quiet(busy(h('div', { class: 'stack' }, modePanel()), 'Пересчёт')) : busy(h('div', { class: 'stack' }, modePanel()), S.busy === 'run' ? 'Сохранение…' : null)));
}

function modePanel() {
  const est = S.modeEst && S.modeEst.modes;
  return h('section', { class: 'panel' },
    h('h2', {}, 'Насколько сжимать'),
    h('p', { class: 'hint' }, S.staleModes ? 'Вес по режимам посчитается после «Пересчитать».' : est ? `Вес тома «${shortName(S.items[S.sel])}» в каждом режиме.` : 'Считаю вес выбранного тома…'),
    h('div', { class: 'modes', role: 'group', 'aria-label': 'Режим' }, C.info.modes.map(m =>
      h('button', { class: 'mode', 'aria-pressed': S.opt.mode === m.id, onclick: () => setMode(m.id) },
        h('div', { class: 'mode-top' }, h('b', {}, m.name),
          h('span', { class: 'est' }, est && est[m.id] ? `${size(est[m.id].bytes)} · ${pct(S.modeEst.size, est[m.id].bytes)}` : ''), h('small', {}, m.note)),
        // подробности — только у выбранного режима: остальные компактной строкой
        S.opt.mode === m.id ? h('ul', {}, m.plus.map(t => h('li', { class: 'plus' }, t)), m.minus.map(t => h('li', { class: 'minus' }, t))) : null))),
    S.opt.mode === 'custom' ? h('p', { class: 'hint', style: { margin: '12px 0 0', color: 'var(--olive-strong)' } }, 'Своя настройка. Выберите режим, чтобы вернуться к готовому.') : null,
    h('button', { class: 'btn', style: { marginTop: '10px', width: '100%' }, onclick: () => openCompare() }, h('span', { html: icon('compare') }), 'Сравнить режимы'),
    fine());
}

function fine() {
  const o = S.opt, rd = reader();
  const set = (k, v) => { S.opt[k] = v; S.opt.mode = 'custom'; S.mode = 'custom'; changed(); };
  const seg = (k, items) => h('div', { class: 'seg' }, items.map(([v, t]) => h('button', { 'aria-pressed': o[k] === v, onclick: () => set(k, v) }, t)));
  const range = (k, min, max, unit = '') => h('div', { class: 'range' },
    h('input', { type: 'range', min, max, value: o[k], oninput: e => { e.target.nextSibling.value = e.target.value + unit }, onchange: e => set(k, +e.target.value) }),
    h('output', {}, o[k] + unit));
  const d = h('details', { class: 'fine', open: S.fineOpen },
    h('summary', {}, 'Тонкая настройка'),
    h('div', { class: 'fine-grid' },
      h('div', { class: 'field' }, h('span', {}, 'Высота страницы, px'), h('input', { class: 'input', type: 'number', min: 0, max: 8000, step: 100, value: o.max_height, onchange: e => set('max_height', +e.target.value || 0) }),
        h('small', {}, `Экран «${rd.name}» — ${rd.screen.height} px. 0 — как в исходнике.`)),
      h('div', { class: 'field' }, h('span', {}, 'Качество JPEG'), range('quality', 30, 100), h('small', {}, '80–85 на e-ink неотличимо от оригинала.')),
      h('div', { class: 'field' }, h('span', {}, 'Чёрно-белые страницы'), seg('gray', [['off', 'Как есть'], ['auto', 'Серым'], ['all', 'Всё серым']])),
      h('div', { class: 'field' }, h('span', {}, 'Сохранять ч/б как'), seg('bw_format', [['auto', 'Что легче'], ['auto4', 'Ещё легче'], ['png16', 'PNG 16'], ['jpeg', 'JPEG']]),
        h('small', {}, '«Ещё легче» разрешает PNG в 4 оттенка — серые заливки станут ступеньками.')),
      h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: o.keep_color_pages, onchange: e => set('keep_color_pages', e.target.checked) }), 'Цветные вклейки в ч/б томе не трогать'),
      h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: o.crop, onchange: e => set('crop', e.target.checked) }), 'Обрезать пустые поля'),
      h('div', { class: 'field' }, h('span', {}, 'Белый фон, плотный чёрный'), seg('clean', [[0, 'Нет'], [1, 'Слабо'], [2, 'Средне'], [3, 'Сильно']])),
      h('div', { class: 'field' }, h('span', {}, 'Резкость'), seg('sharpen', [[0, 'Нет'], [1, 'Слабо'], [2, 'Средне'], [3, 'Сильно']])),
      rd.screen.color ? h('div', { class: 'field' }, h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: o.color_fix, onchange: e => set('color_fix', e.target.checked) }), 'Сочнее цвета на цветном экране'),
        h('span', {}, 'Насыщенность'), range('sat', -50, 100, ' %')) : null,
      h('div', { class: 'field' }, h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: o.squeeze, onchange: e => set('squeeze', e.target.checked) }), 'Пережимать страницы, которые и так подходят'),
        h('span', {}, 'если легче хотя бы на'), range('min_gain', 0, 50, ' %'))));
  d.addEventListener('toggle', () => S.fineOpen = d.open);
  return d;
}

function shelfPanel() {
  if (S.busy === 'run') {
    const n = S.runTotal || S.items.length, done = Object.values(S.status).filter(x => x.done).length;
    const inB = Object.values(S.status).reduce((a, x) => a + (x.in || 0), 0), outB = Object.values(S.status).reduce((a, x) => a + (x.out || 0), 0);
    return h('section', { class: 'panel shelf', 'aria-live': 'polite' },
      h('div', { class: 'saving' }, h('div', { class: 'big' }, `${done} из ${n}`),
        h('div', { class: 'cap' }, S.runMode === 'rename' ? 'переименовано' : inB ? `сохранено · освобождено ${size(inB - outB)}` : 'сохранено')),
      h('div', { class: 'progress' }, h('i', { style: { width: `${100 * done / n}%` } })),
      h('div', { class: 'shelf-actions' }, h('button', { class: 'btn', onclick: () => call('job_cancel', { id: 'run' }) }, 'Остановить')));
  }
  const e = S.est;
  const all = S.total, guess = e ? e.guess : null;
  // настройки изменились: прежняя цифра приглушена, одна главная кнопка — «Пересчитать»
  if (S.stale) return h('section', { class: 'panel shelf stale', 'aria-live': 'polite' },
    h('div', { class: 'saving' }, h('div', { class: 'big pending' }, guess != null && all - guess > 0 ? `−${size(all - guess)}` : '?'),
      h('div', { class: 'cap' }, guess != null ? 'было при прежних настройках' : 'ещё не посчитано')),
    h('div', { class: 'shelf-actions' },
      h('button', { class: 'btn primary', onclick: recalc, title: 'Оценить все тома с новыми настройками' }, h('span', { html: icon('refresh') }), 'Пересчитать'),
      h('button', { class: 'btn quiet', onclick: () => openPreview() }, 'Посмотреть на читалке'),
      h('span', { class: 'empty-note', style: { fontSize: 'var(--fs-s)' } }, 'Сам пересчёт не запускается — он долгий.')));
  const done = e && e.done >= e.total;
  const free = guess != null ? Math.max(0, all - guess) : null;
  return h('section', { class: 'panel shelf', 'aria-live': 'polite' },
    h('div', { class: 'saving' },
      h('div', { class: 'big' + (done ? '' : ' pending') }, free != null ? (free > 0 ? `−${size(free)}` : '±0') : '…'),
      h('div', { class: 'cap' }, free == null ? 'Считаю, сколько освободится…' : done ? (free > 0 ? 'освободится на диске' : 'места не освободится — попробуйте режим посильнее') : `пока примерно — проверено ${e.done} из ${e.total}`)),
    h('div', { class: 'bars' },
      bar('Сейчас', all, all, ''),
      bar('Станет', guess == null ? 0 : guess, all, 'after')),
    !done && e ? h('div', { class: 'progress' }, h('i', { style: { width: `${100 * e.done / e.total}%` } })) : null,
    h('div', { class: 'shelf-actions' },
      h('button', { class: 'btn primary', onclick: () => openPreview() }, h('span', { html: icon('eye') }), 'Посмотреть на читалке'),
      h('span', { class: 'empty-note', style: { fontSize: 'var(--fs-s)' } }, done ? `Оценка по ${e.samples || 'нескольким'} страницам каждого тома. Файлы пока не менялись.` : '')));
}
function bar(label, v, max, cls) {
  return h('div', { class: 'bar ' + cls }, h('span', {}, label), h('div', { class: 'track' }, h('div', { class: 'fill', style: { width: `${max ? Math.max(1.5, 100 * v / max) : 0}%` } })), h('b', {}, v ? size(v) : '—'));
}

/* ---------- готовое по томам: сохранён ли том и что с ним на читалке */
const relOf = it => `${it.rel ? nrm(it.rel) + '/' : ''}${it.newName}`.toLowerCase();
function readyOf(i) {
  if (!S.done.has(i)) return null;
  if (S._rmapOf !== S.ready) { S._rmapOf = S.ready; S._rmap = new Map(S.ready.map(f => [`${f.kind}|${nrm(f.rel).toLowerCase()}`, f])); }
  return S._rmap.get(`${kindOf(S.items[i])}|${relOf(S.items[i])}`) || { mtime: 0, onReader: null };
}
const isSaved = i => S.done.has(i) || !!(S.status[i] && S.status[i].done && S.status[i].ok && S.runMode === 'copies');
/** сводка серии: сколько готово, сколько на читалке, одна метка состояния */
function serState(g) {
  const n = g.idx.length, conn = !!C.cur().connected;
  const saving = g.idx.some(i => S.status[i] && !S.status[i].done), err = g.idx.filter(i => S.status[i] && S.status[i].done && !S.status[i].ok);
  const saved = g.idx.filter(isSaved).length, onR = g.idx.filter(i => (readyOf(i) || {}).onReader === 'same').length;
  const chip = (cls, text, title) => h('span', { class: 'chip st ' + cls, title }, text);
  const st = saving ? chip('wait', 'сохраняю…', 'Идёт сохранение в папку готового')
    : err.length ? chip('warn', 'ошибка', S.status[err[0]].text)
    : saved < n ? (S.ready.length || S.done.size ? chip('new', saved ? `${n - saved} не сохр.` : 'не сохранено', `Ещё не сохранено в готовое: ${n - saved} из ${n}`) : null)
    : conn && onR === n ? chip('ok', 'на читалке', 'Все тома уже на читалке')
    : conn && onR < n ? chip('done', `${n - onR} ${plural(n - onR, 'ждёт', 'ждут', 'ждут')}`, `Сохранено, на читалку ещё не отправлено: ${n - onR}`)
    : chip('done', 'сохранено', 'Все тома сохранены в готовое');
  const prog = h('span', { class: 'sprog', title: `Готово ${saved} из ${n}${conn ? `, на читалке ${onR}` : ''}` },
    h('span', {}, conn ? `готово ${saved} · на читалке ${onR}` : `готово ${saved} из ${n}`),
    h('span', { class: 'bar' }, h('i', { style: { width: `${100 * onR / n}%` } }), h('u', { style: { width: `${100 * Math.max(0, saved - onR) / n}%` } })));
  return { st, prog, saved, onR };
}
const twisty = (g, open) => h('button', { class: 'tw', type: 'button', title: open ? 'Свернуть тома' : 'Показать тома', 'aria-expanded': String(open), html: icon('right'),
  onclick: e => { e.stopPropagation(); open ? S.open.delete(g.key) : S.open.add(g.key); render(); }, ondblclick: e => e.stopPropagation() });
function selectSeries(g) { if (!g.idx.includes(S.sel)) select(g.idx[0]); }

function listPanel() {
  if (S.view !== 'src') {
    const nf = S.ready.length ? freshIdx().length : 0;
    // готовый файл → исходный том: по пути; вид не важен (серию могли переотметить, а копия осталась в прежней папке)
    const byRel = () => new Map(S.items.map((it, i) => [relOf(it), i]));
    const idxOf = f => byRel().get(nrm(f.rel).toLowerCase());
    return h('section', { class: 'panel list-panel' },
      S.view === 'ready' ? freshBar(nf, ['том', 'тома', 'томов'], { busy: S.busy, onShow: () => { S.view = 'src'; render(); }, onSave: () => run('copies', { onlyNew: true }) }) : null,
      h('div', { class: 'blist' }, readyView(S, { ctx: C, kind: 'manga', dir: folders('manga').out_dir, dirs: KINDS.map(k => folders(k).out_dir), scope: S.scope, icon: 'comics', rerender: render, onRemoved: refreshReady, fmt: true,
        only: S.view === 'reader' ? 'on' : null, skip: stale,
        sub: f => [stale(f) ? `старая копия: серия теперь ${f.kind === 'manga' ? 'комикс' : 'манга'}` : f.kind === 'manga' ? 'манга' : 'комикс', nrm(f.rel).split('/').slice(0, -1).join(' / ')].filter(Boolean).join(' · '),
        // щелчок — тот же том выбран в «Скачанном» (оценки справа), двойной — как будет на читалке
        cur: S.readyCur, onPick: f => { S.readyCur = f.path; const i = idxOf(f); if (i != null) select(i); else render(); },
        onOpen: f => { const i = idxOf(f); if (i != null) openPreview(i); else toast('Исходного тома в «Скачанном» нет — показать нечего.'); } })));
  }
  const q = (S.q || '').trim().toLowerCase();
  const inSc = new Set(inScopeIdx());
  const all = seriesList().map(g => Object.assign({}, g, { idx: g.idx.filter(i => inSc.has(i)) })).filter(g => g.idx.length);
  const cnt = f => all.filter(f).length;
  const unsaved = g => g.idx.some(i => !isSaved(i));
  const FK = { all: () => true, manga: g => g.kind === 'manga', comics: g => g.kind === 'comics', wait: unsaved };
  if (!FK[S.kfilter]) S.kfilter = 'all';
  const filters = [['all', `Все ${all.length}`], ['manga', `Манга ${cnt(FK.manga)}`], ['comics', `Комиксы ${cnt(FK.comics)}`],
    S.ready.length || S.done.size ? ['wait', `Не сохранено ${cnt(unsaved)}`] : null].filter(f => f && (f[0] === 'all' || f[0] === S.kfilter || cnt(FK[f[0]])));
  const shown = all.filter(FK[S.kfilter]).map(g => {
    if (!q) return g;
    if (`${g.title} ${g.rel}`.toLowerCase().includes(q)) return g;
    const idx = g.idx.filter(i => `${S.items[i].newName} ${S.items[i].name}`.toLowerCase().includes(q));
    return idx.length ? Object.assign({}, g, { idx, hit: true }) : null;
  }).filter(Boolean);
  const vols = shown.flatMap(g => g.idx);
  const sel = S.srcSel = S.srcSel || new Set(), multi = sel.size > 0;
  const picked = inScopeIdx().filter(i => sel.has(i));
  const conn = !!C.cur().connected;
  const volRow = i => { const it = S.items[i], st = S.status[i], est = S.perItem[i];
    const state = stateChip({ saving: st && !st.done, err: st && st.done && !st.ok ? st.text : null, savedNow: st && st.done && st.ok,
      note: st && st.done && st.ok ? (st.out ? `теперь ${size(st.out)}` : st.text) : '', ready: readyOf(i), connected: conn, fresh: S.ready.length > 0 || S.done.size > 0 });
    return rowEl({ multi, checked: sel.has(i), picked: i === S.sel, cls: 'vol', tip: it.rel ? `${it.rel}/${it.name}` : it.name,
      cover: h('span', { class: 'ic', html: icon('comics') }), title: it.newName,
      sub: [it.newName !== it.name ? `было: ${it.name}` : null, est != null ? `${size(it.size)} → ≈ ${size(est)}${S.stale ? ' (по прежним настройкам)' : ''}` : size(it.size)].filter(Boolean).join(' · '),
      tags: [h('span', { class: 'chip mute' }, it.kind), state],
      menu: [{ label: 'Как будет на читалке', onClick: () => openPreview(i) }, { label: 'Сравнить режимы', onClick: () => openCompare(i) },
        { label: 'Показать в папке', hint: 'ваш файл в проводнике', onClick: () => openPath(`${S.root}/${it.rel ? it.rel + '/' : ''}${it.name}`.replace(/\//g, '\\')).catch(e => toast(String(e))) },
        { label: 'Сохранить только этот', hint: 'в папку готового, даже если уже готов', onClick: () => run('copies', { list: [i] }) }],
      onToggle: () => { sel.has(i) ? sel.delete(i) : sel.add(i); render(); }, onClick: () => select(i), onOpen: () => openPreview(i) }); };
  const serRow = g => { const open = S.open.has(g.key) || !!g.hit, n = g.idx.length, ss = serState(g);
    const est = g.idx.every(i => S.perItem[i] != null) ? g.idx.reduce((a, i) => a + S.perItem[i], 0) : null;
    const allSel = g.idx.every(i => sel.has(i)), other = g.kind === 'manga' ? 'comics' : 'manga';
    return rowEl({ multi, checked: allSel, picked: !!S.items[S.sel] && serKey(S.items[S.sel]) === g.key, cls: 'series' + (open ? ' open' : ''), tip: g.rel || g.title,
      cover: twisty(g, open), title: g.title,
      sub: [`${n} ${plural(n, 'том', 'тома', 'томов')}`, est != null ? `${size(g.size)} → ≈ ${size(est)}` : size(g.size), g.rel && g.rel.split('/').pop() !== g.title ? g.rel : null].filter(Boolean).join(' · '),
      tags: [h('span', { class: 'kchip ' + g.kind }, KIND_NAME[g.kind]), ss.prog, ss.st],
      menu: [{ label: `Это ${other === 'manga' ? 'манга' : 'комикс'}`, hint: `готовое уйдёт в ${C.cur().readerDirs[other]}`, onClick: () => setKind(g, other) },
        { label: 'Сохранить серию', hint: `${n} ${plural(n, 'том', 'тома', 'томов')} в папку готового`, onClick: () => run('copies', { list: g.idx }) },
        { label: 'На читалку', hint: 'сохранить и добавить в «На читалку»', onClick: () => sendSel(g.idx) },
        { label: 'Как будет на читалке', onClick: () => openPreview(g.idx[0]) },
        { label: 'Показать в папке', onClick: () => openPath(`${S.root}/${g.rel}`.replace(/\//g, '\\')).catch(e => toast(String(e))) }],
      onToggle: () => { if (allSel) g.idx.forEach(i => sel.delete(i)); else g.idx.forEach(i => sel.add(i)); render(); },
      onClick: () => selectSeries(g), onOpen: () => { open ? S.open.delete(g.key) : S.open.add(g.key); render(); } }); };
  const lim = S.limit || 200;
  return h('section', { class: 'panel list-panel' },
    listTools({ key: 'csrc', filters, value: S.kfilter, onFilter: id => { S.kfilter = id; render(); },
      q: S.q || '', onQ: v => { S.q = v; render(); }, all: { keys: () => vols, sel, onChange: render } }),
    picked.length ? selBar({ count: picked.length, bytes: picked.reduce((a, i) => a + (S.items[i].size || 0), 0), onClear: () => { sel.clear(); render(); },
      actions: [h('button', { class: 'btn quiet', disabled: !!S.busy, title: 'Сохранить в папку только выбранные (даже если уже готовы)', onclick: () => run('copies', { list: picked }) }, 'Сохранить выбранные'),
        h('button', { class: 'btn apply', disabled: !!S.busy, onclick: () => sendSel(picked) }, h('span', { html: icon('apply') }), `На читалку: ${picked.length}`)] }) : null,
    h('div', { class: 'blist ready-list', role: 'listbox', 'aria-label': 'Серии' }, shown.length
      ? shown.slice(0, lim).flatMap(g => [serRow(g), ...(S.open.has(g.key) || g.hit ? g.idx.map(volRow) : [])])
      : q ? mzEmpty('ask', `По «${(S.q || '').trim()}» ничего нет`, 'Поиск идёт по названию серии, папке и именам томов.', [h('button', { class: 'btn quiet', onclick: () => { S.q = ''; render(); } }, 'Очистить поиск')])
        : mzEmpty('ask', 'Под этот отбор серий нет', null, [h('button', { class: 'btn', onclick: () => { S.kfilter = 'all'; render(); } }, 'Показать все')]),
      shown.length > lim ? h('button', { class: 'btn quiet', style: { margin: '8px auto', display: 'flex' }, onclick: () => { S.limit = lim + 200; render(); } }, `Показать ещё ${Math.min(200, shown.length - lim)}`) : null));
}
/** справа сверху: выбранная серия — манга или комикс, куда ляжет, быстрые действия */
function serCard() {
  const it = S.items[S.sel]; if (!it) return null;
  const g = seriesList().find(x => x.key === serKey(it)); if (!g) return null;
  const d = C.cur(), n = g.idx.length, ss = serState(g), k = g.kind;
  const tail = (p, m = 3) => { const a = String(p).replace(/\//g, '\\').split('\\').filter(Boolean); return (a.length > m ? '…\\' : '') + a.slice(-m).join('\\'); };
  const line = (label, state, path) => h('div', {}, h('span', {}, label, state ? h('em', {}, state) : null), h('code', { title: path }, tail(path)));
  const rel = g.rel ? '\\' + g.rel.replace(/\//g, '\\') : '';
  const todo = g.idx.filter(i => !isSaved(i));
  return h('section', { class: 'panel ser-card' },
    h('h2', {}, g.title),
    h('p', { class: 'sub' }, `${n} ${plural(n, 'том', 'тома', 'томов')} · ${size(g.size)}`),
    h('div', { class: 'krow' }, h('span', {}, 'Это'), lock(h('div', { class: 'seg', role: 'group', 'aria-label': 'Манга или комикс' }, KINDS.map(x =>
      h('button', { 'aria-pressed': k === x, title: `Готовое — в папку ${d.readerDirs[x]} на читалке`, onclick: () => { if (k !== x) setKind(g, x); } }, x === 'manga' ? 'Манга' : 'Комикс'))), !!S.busy)),
    h('div', { class: 'trail' },
      line('Скачанное', null, `${S.root}${rel}`),
      line('Готовое', `${ss.saved} из ${n}`, `${d.dirs[k]}${rel}`),
      line('На читалке', d.connected ? `${ss.onR} из ${n}` : 'не подключена', `${d.readerDirs[k]}${rel}`.replace(/\//g, '\\'))),
    h('div', { class: 'ser-btns' },
      h('button', { class: 'btn grow', disabled: !!S.busy, style: { flex: 1 }, title: todo.length ? 'Сохранить несохранённые тома серии' : 'Сохранить серию заново', onclick: () => run('copies', { list: todo.length ? todo : g.idx }) },
        todo.length ? `Сохранить ${todo.length} ${plural(todo.length, 'том', 'тома', 'томов')}` : 'Сохранить заново'),
      h('button', { class: 'btn apply', disabled: !!S.busy, onclick: () => sendSel(g.idx) }, h('span', { html: icon('apply') }), 'На читалку')));
}
/** выбранные тома → сохранить и сразу в «На читалку» (каждый — в Manga или Comics) */
async function sendSel(list) {
  const todo = list.filter(i => !isSaved(i));
  if (todo.length) await run('copies', { list: todo });
  await refreshReady();
  const want = new Set(list.map(i => `${kindOf(S.items[i])}|${relOf(S.items[i])}`));
  const files = S.ready.filter(f => want.has(`${f.kind}|${nrm(f.rel).toLowerCase()}`));
  if (!files.length) return;
  const dirOf2 = r => nrm(r).split('/').slice(0, -1).join('/');
  await Queue.addAndGo(C, files.map(f => ({ src: f.path, dst: [C.cur().readerDirs[f.kind], dirOf2(f.rel)].filter(Boolean).join('/'), kind: f.kind, label: nrm(f.rel).split('/').pop() })));
}

/** «Папка с готовым»: у манги и комиксов — своя */
const outMenu = () => menuButton('Папка с готовым', KINDS.map(k => ({ label: k === 'manga' ? 'Манга' : 'Комиксы', hint: folders(k).out_dir, onClick: () => openPath(folders(k).out_dir).catch(() => toast('Папки ещё нет — она появится при первом сохранении.')) })), { cls: 'quiet', icon: 'folder', up: true });
const sendBtn = () => toQueue(C, 'manga', { kinds: KINDS, onSave: () => run('copies'), missing: () => missingNow() });
function actions() {
  const est = S.estimating && !S.busy;
  if (est) return h('div', { class: 'actions' },
    h('div', { class: 'where' }, outMenu()),
    h('div', { class: 'run-prog' }, h('small', {}, `пересчёт: ${S.est ? S.est.done : 0} из ${S.items.length}`),
      h('div', { class: 'progress' }, h('i', { style: { width: `${100 * (S.est ? S.est.done : 0) / Math.max(1, S.items.length)}%` } }))),
    h('button', { class: 'btn', title: 'Прервать оценку — её можно запустить снова кнопкой «Пересчитать»', onclick: () => call('job_cancel', { id: 'estimate' }) }, 'Остановить'),
    sendBtn());
  return h('div', { class: 'actions' },
    h('div', { class: 'where' }, outMenu()),
    // редкие действия — в меню, на виду только главное
    lock(menuButton('Ещё', [
      { label: 'Только переименовать', hint: 'файлы не пересобираются', onClick: () => run('rename') },
      { label: 'Изменить на месте', hint: 'оригиналы уйдут в резервную папку', onClick: () => run('inplace') },
    ], { up: true }), !!S.busy),
    h('button', { class: 'btn primary', disabled: !!S.busy, onclick: () => run('copies'), title: S.scope ? `Только тома из подпапки ${S.scope}` : null }, S.scope ? `Сохранить «${S.scope.split('/').pop()}»` : 'Сохранить в папку'),
    lock(sendBtn(), !!S.busy));
}

const shortName = it => it ? (it.newName.length > 44 ? it.newName.slice(0, 43) + '…' : it.newName) : '';

/* ================= действия */
async function scan(path, quiet) {
  S.root = path; S.busy = 'scan'; S.est = null; S.perItem = {}; S.status = {}; S.modeEst = null; render();
  try {
    // готовое манги и комиксов может лежать внутри выбранной папки — его не сканировать
    const r = await call('comics_scan', { args: { path, recurse: S.recurse, rename: S.rename, template: S.template, folders: folders('manga'), skip: [folders('comics').out_dir], by_dir: S.tplByDir || {} } });
    S.items = r.items; S.total = r.total; S.sel = 0; S.perItem = {}; S.stale = false; S.staleModes = false; S.modeEst = null; S.done = new Set(); save(); C.remember('comics', path);
  } catch (e) { if (!quiet) toast(String(e)); S.items = [] }
  S.busy = false; render();
  refreshReady();
  if (S.items.length) { modeEstimates(); estimateAll(); }
}
async function names() { const r = await call('comics_names', { template: S.template, rename: S.rename, by_dir: S.tplByDir || {} }); S.items = r.items; save(); render(); }
/* ---------- шаблоны названий (как в «Мастерской»): готовые + до 4 своих, пример на своём файле */
const TPL_MAP = { series: 'Серия', vol: 'Том', ch: 'Глава', title: 'Название', n: '№ по порядку', folder: 'Папка', name: 'Исходное имя', orig: 'Исходное имя' };
function tplHuman(t) {
  if (!t) return 'навести порядок в исходном';
  return [...new Set([...t.matchAll(/\{(\w+)(?::\d+)?\}/g)].map(m => TPL_MAP[m[1]] || m[1]))].join(' · ');
}
async function templateDialog() {
  const presets = C.info.templates.map(([t]) => t);
  // у подпапки — свой шаблон или «как у всей папки»
  const INH = '\u0000inherit', target = S.scope;
  let sel = target ? ((S.tplByDir || {})[target] ?? INH) : S.template, mine = (S.myTemplates || []).filter(t => t && !presets.includes(t)).slice(0, 4);
  if (sel && sel !== INH && !presets.includes(sel) && !mine.includes(sel)) mine = [sel, ...mine].slice(0, 4);
  const sample = async list => { try { return await call('comics_name_samples', { templates: list.map(t => t === INH ? S.template : t), dir: target || null }); } catch (e) { return null; } };
  const list = h('div', { class: 'tpl-list', role: 'radiogroup', 'aria-label': 'Шаблон' });
  const was = h('p', { class: 'hint', style: { margin: 0 } });
  const draw = async () => {
    const all = [...(target ? [{ t: INH, mine: false }] : []), ...presets.map(t => ({ t, mine: false })), ...mine.map(t => ({ t, mine: true }))];
    const smp = await sample(all.map(x => x.t));
    fill(was, smp ? ['было: ', h('b', {}, smp.orig)] : 'Пример появится, когда в папке будут тома.');
    fill(list, all.map((x, i) => h('div', { class: 'tpl-row' },
      h('button', { type: 'button', class: 'tpl', role: 'radio', 'aria-checked': x.t === sel, onclick: () => { sel = x.t; draw(); } },
        h('b', {}, smp ? smp.names[i] : '—'),
        h('small', {}, x.t === INH ? `как у всей папки: ${S.template ? tplHuman(S.template) : 'без шаблона'}` : x.t ? h('span', { title: x.t }, tplHuman(x.t)) : 'как в файле, только аккуратнее')),
      x.mine ? h('button', { type: 'button', class: 'icon-btn', title: 'Убрать шаблон', 'aria-label': 'Убрать шаблон', html: icon('close'),
        onclick: () => { mine = mine.filter(m => m !== x.t); if (sel === x.t) sel = ''; draw(); } }) : null)));
  };
  const inp = h('input', { class: 'input mono', placeholder: '{series} - {ch:3}', 'aria-label': 'Свой шаблон' });
  const newSmp = h('small', { class: 'hint' });
  let tT = 0;
  inp.addEventListener('input', () => { clearTimeout(tT); tT = setTimeout(async () => { const t = inp.value.trim(); const r = t ? await sample([t]) : null; fill(newSmp, r ? `станет: ${r.names[0]}` : ''); }, 200); });
  const add = () => { const t = inp.value.trim(); if (!t) return; if (!presets.includes(t)) mine = [t, ...mine.filter(m => m !== t)].slice(0, 4); sel = t; inp.value = ''; fill(newSmp, ''); draw(); };
  inp.addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); add(); } });
  draw();
  const ok = await dialog(close => h('div', { class: 'dlg dlg-wide' }, h('h2', {}, target ? `Как называть файлы в «${target.split('/').pop()}»` : 'Как называть файлы'),
    target ? h('p', { class: 'hint', style: { margin: 0 } }, `Шаблон только для подпапки ${target} и вложенных в неё. Другие папки — по своим шаблонам или по общему. Пример — по тому из этой подпапки.`) : null,
    !target && Object.keys(S.tplByDir || {}).length ? h('p', { class: 'hint', style: { margin: 0 } }, `Свои шаблоны у подпапок: ${Object.keys(S.tplByDir).join(', ')}. Общий шаблон к ним не применяется.`) : null,
    was, list,
    h('div', { class: 'field' }, h('span', {}, 'Свой вариант'), h('div', { class: 'row' }, inp, h('button', { type: 'button', class: 'btn', onclick: add }, 'Добавить')), newSmp),
    // как писать свой шаблон — свёрнуто, это нужно редко
    h('details', { class: 'fine' }, h('summary', {}, 'Как написать свой шаблон'), h('p', { class: 'hint', style: { margin: 0 } }, 'Поля: {series} серия · {vol} том · {ch} глава · {title} название главы · {n} номер по порядку · {folder} папка · {name} исправленное название. {ch:3} — номер с нулями (004). Кусок в [ … ] пропадает, если поле внутри пустое.')),
    h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', type: 'button', onclick: () => close(false) }, 'Отмена'), h('button', { class: 'btn primary', type: 'button', onclick: () => close(true) }, 'Готово'))));
  if (!ok) return;
  S.myTemplates = mine;
  if (target) { S.tplByDir = { ...(S.tplByDir || {}) }; if (sel === INH) delete S.tplByDir[target]; else S.tplByDir[target] = sel; }
  else S.template = sel;
  save(); names();
}
async function setReader(id) {
  S.reader = id;
  if (S.opt.mode === 'custom') S.opt.reader = id; else S.opt = await call('comics_mode_opt', { reader: id, mode: S.opt.mode });
  changed(true);
}
async function setMode(m) { S.mode = m; S.opt = await call('comics_mode_opt', { reader: S.reader, mode: m }); changed(); }
function select(i) { if (S.sel === i) return; S.sel = i; S.modeEst = null; render(); modeEstimates(); }

// смена режима, настроек или читалки сама ничего не пересчитывает: оценка всех томов долгая и грузит процессор.
// Прежние цифры остаются приглушёнными, пересчёт — по кнопке «Пересчитать».
function changed(readerChanged) {
  save();
  if (S.est && S.est.done < S.est.total) call('job_cancel', { id: 'estimate' }).catch(() => {});
  S.stale = true; if (readerChanged) S.staleModes = true;
  render();
}
function recalc() {
  S.stale = false; S.est = null; S.perItem = {};
  if (S.staleModes) { S.staleModes = false; S.modeEst = null; modeEstimates(); }
  render(); estimateAll();
}

async function modeEstimates() {
  if (!S.items.length) return;
  const sel = S.sel, rd = S.reader;
  try { const r = await call('comics_mode_estimates', { args: { index: sel, reader: rd } }); if (sel === S.sel && rd === S.reader) { S.modeEst = r; render() } } catch (e) {}
}
async function estimateAll() {
  if (!S.items.length || S.busy === 'run') return;
  // пересчёт — как сканирование: всё ниже строки папки размыто, в нижней строке ход и «Остановить»
  S.est = { done: 0, total: S.items.length, guess: null }; S.estimating = true; render();
  try { const r = await call('comics_estimate_all', { opt: S.opt }); if (!r.cancelled) S.est = Object.assign(S.est, { done: S.items.length, guess: r.guess, samples: r.samples }); else S.stale = true; } catch (e) { S.stale = true; }
  S.estimating = false; if (root && root.isConnected) render();
}
function onEstimate(p) {
  if (!S.est) return;
  S.perItem[p.index] = p.bytes; S.est.done = p.done; S.est.total = p.total; S.est.guess = p.guess;
  scheduleRender();
}
// не чаще кадра; во время сохранения и пересчёта — не чаще раза в 250 мс (события идут после каждого тома)
let rT = 0, lT = 0, lastR = 0;
function scheduleRender() {
  if (rT || lT) return;
  const go = () => { rT = requestAnimationFrame(() => { rT = 0; lastR = performance.now(); if (root && root.isConnected) render(); }); };
  const wait = S.busy === 'run' || S.estimating ? 250 - (performance.now() - lastR) : 0;
  if (wait > 0) lT = setTimeout(() => { lT = 0; go(); }, wait); else go();
}

/** что уже сохранено и что лежит в папке готового (у манги и комиксов — свои папки) */
async function refreshReady() {
  const items = S.items;
  const res = await Promise.all(KINDS.map(k => Promise.all([call('comics_done', { out: folders(k).out_dir }).catch(() => []),
    call('ready_list', { dir: folders(k).out_dir, exts: [], reader: readerPath(C, k) }).catch(() => [])])));
  if (items !== S.items) return; // пока считали, папку пересканировали
  const done = new Set(), ready = [];
  // том сохранён, если копия есть в папке его вида (манга — в Manga, комикс — в Comics)
  KINDS.forEach((k, j) => { const [dn, rl] = res[j]; dn.forEach(i => { if (S.items[i] && kindOf(S.items[i]) === k) done.add(i); }); rl.forEach(f => ready.push(Object.assign({}, f, { kind: k }))); });
  S.done = done; S.ready = ready;
  if (root && root.isConnected) render();
}
// готовое, которого нет на читалке (только если читалка подключена и сверка была)
const missingNow = () => C.cur().connected && S.ready.length && S.ready.every(f => f.onReader != null) ? S.ready.filter(f => f.onReader !== 'same' && !stale(f) && (!S.scope || inScope(String(f.rel).replace(/\\/g, '/').split('/').slice(0, -1).join('/'), S.scope))) : null;
/** копия в папке другого вида: серию переотметили (манга ↔ комикс) — на читалку такую не отправлять */
function stale(f) {
  if (S._srcOf !== S.items || S._srcK !== S.kindsV) { S._srcOf = S.items; S._srcK = S.kindsV; S._src = new Map(S.items.map(it => [relOf(it), kindOf(it)])); }
  const k = S._src.get(nrm(f.rel).toLowerCase()); return !!k && k !== f.kind;
}
const freshIdx = () => inScopeIdx().filter(i => !S.done.has(i));

async function run(mode, { onlyNew = false, list = null } = {}) {
  if (mode === 'inplace') {
    const ok = await dialog(close => h('div', { class: 'dlg' }, h('h2', {}, 'Изменить файлы на месте?'),
      h('p', { style: { margin: 0 } }, `${S.items.length} ${plural(S.items.length, 'том', 'тома', 'томов')} будут заменены новыми. Оригиналы перенесутся в ${folders().backup_dir || 'папку «_backup» внутри папки с мангой и комиксами'}.`),
      h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close(false) }, 'Отмена'), h('button', { class: 'btn primary', onclick: () => close(true) }, 'Изменить'))));
    if (!ok) return;
  }
  call('job_cancel', { id: 'estimate' });
  const base = list || (onlyNew ? freshIdx() : S.scope ? inScopeIdx() : S.items.map((_, i) => i));
  S.busy = 'run'; S.runMode = mode; S.status = {}; S.runTotal = base.length; render();
  try {
    // копии: манга — в папку Manga, комиксы — в Comics; переименование и «на месте» — одним проходом
    const groups = mode === 'copies' ? KINDS.map(k => [k, base.filter(i => kindOf(S.items[i]) === k)]).filter(([, l]) => l.length) : [['manga', base]];
    const r = { ok: 0, fail: 0, skipped: 0, cancelled: false, out: folders(groups.length ? groups[0][0] : 'manga').out_dir };
    for (const [k, only] of groups) {
      const x = await call('comics_run', { args: { mode, opt: S.opt, folders: folders(k), only, skip_done: list ? false : S.skipDone } });
      r.ok += x.ok; r.fail += x.fail; r.skipped += x.skipped || 0; if (x.cancelled) { r.cancelled = true; break; }
    }
    const inB = Object.values(S.status).reduce((a, s) => a + (s.in || 0), 0), outB = Object.values(S.status).reduce((a, s) => a + (s.out || 0), 0);
    // сохранили — показать, что теперь в папке готового
    if (mode === 'copies' && r.ok && !list) S.view = 'ready';
    if (list && S.srcSel) { S.srcSel.clear(); S.srcMulti = false; }
    S.busy = false; render(); refreshReady();
    const msg = `${mode === 'rename' ? 'Переименовано' : 'Готово'}: ${r.ok}${r.skipped ? `, уже готовых пропущено: ${r.skipped}` : ''}${r.fail ? `, с ошибками: ${r.fail}` : ''}${r.cancelled ? ', остановлено' : ''}${mode !== 'rename' && inB ? ` · было ${size(inB)} → стало ${size(outB)}` : ''}`;
    toast(msg);
    if (mode === 'copies' && r.ok && C.behavior('openFolderAfterSave')) openPath(r.out).catch(() => {});
    if (mode !== 'copies') scan(S.root, true);
  } catch (e) { S.busy = false; render(); toast(String(e)) }
}
function onProgress(p) {
  S.status[p.index] = p.status === '…' ? { text: 'обрабатываю…' } : { done: true, ok: p.ok, text: p.ok ? (p.out_bytes && p.in_bytes ? `${p.status} · ${pct(p.in_bytes, p.out_bytes)}` : p.status) : p.status, in: p.in_bytes, out: p.out_bytes };
  scheduleRender();
}

/* ================= предпросмотр на читалке */
let P = null;
function openPreview(index = S.sel) {
  if (!S.items.length) return;
  P = { index, page: 2, others: false };
  const back = h('div', { class: 'sheet-back', onclick: closePreview });
  const sheet = h('section', { class: 'sheet', role: 'dialog', 'aria-label': 'Как будет на читалке' });
  document.body.append(back, sheet);
  P.back = back; P.sheet = sheet;
  requestAnimationFrame(() => { back.classList.add('open'); sheet.classList.add('open') });
  P.key = e => { if (e.key === 'Escape') closePreview(); if (e.key === 'ArrowRight' || e.key === 'PageDown') turn(1); if (e.key === 'ArrowLeft' || e.key === 'PageUp') turn(-1) };
  addEventListener('keydown', P.key);
  drawSheet();
  P.ro = new ResizeObserver(() => layoutStage()); P.ro.observe($('#stage', P.sheet));
  loadPage();
}
function closePreview() {
  if (!P) return; removeEventListener('keydown', P.key); P.ro.disconnect();
  P.back.classList.remove('open'); P.sheet.classList.remove('open');
  const b = P.back, s = P.sheet; setTimeout(() => { b.remove(); s.remove() }, 280); P = null;
}
function turn(d) { if (!P || !P.data) return; const n = P.page + d; if (n < 0 || n >= P.data.pages) return; P.page = n; loadPage() }

function drawSheet() {
  const rd = reader();
  const others = C.dev.devices.filter(d => d.id !== S.reader);
  fill(P.sheet, 
    h('header', { class: 'sheet-head' },
      h('h2', {}, `Как будет на «${rd.name}»`),
      others.length ? h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: P.others, onchange: e => { P.others = e.target.checked; loadPage() } }), 'И на других читалках') : null,
      h('select', { 'aria-label': 'Том', onchange: e => { P.index = +e.target.value; P.page = 2; loadPage() } }, S.items.map((it, i) => h('option', { value: i, selected: i === P.index }, it.newName))),
      h('div', { class: 'pager' },
        h('button', { class: 'btn', 'aria-label': 'Предыдущая страница', onclick: () => turn(-1), html: icon('left') }),
        h('span', { id: 'pg' }, '…'),
        h('button', { class: 'btn', 'aria-label': 'Следующая страница', onclick: () => turn(1), html: icon('right') })),
      h('button', { class: 'btn quiet', 'aria-label': 'Закрыть', onclick: closePreview, html: icon('close') })),
    h('div', { class: 'stage', id: 'stage' }, loading('Готовлю страницу…', '')),
    h('footer', { class: 'sheet-foot', id: 'foot' }, h('div', { class: 'sub-line' }, '…')));
}

async function loadPage() {
  // «Загрузка» — по центру поверх прежней страницы и только если ответ не мгновенный; одна, сколько бы ни листали
  const stage = $('#stage', P.sheet); stage.querySelectorAll('.loading-box').forEach(x => x.remove());
  const ld = loading('Готовлю страницу…', ''); ld.hidden = !!stage.querySelector('figure'); stage.append(ld);
  const slow = setTimeout(() => { ld.hidden = false; stage.classList.add('dim'); }, 150);
  const ids = P.others ? [S.reader, ...C.dev.devices.map(d => d.id).filter(x => x !== S.reader)] : [S.reader];
  const ask = { index: P.index, page: P.page, opt: S.opt, readers: ids };
  try {
    const d = await call('comics_preview', { args: ask });
    if (!P || P.index !== ask.index || P.page !== ask.page) return;
    clearTimeout(slow); stage.classList.remove('dim');
    P.data = d; drawStage(d, ids); drawFoot(d);
    call('comics_volume_estimate', { args: { index: P.index, opt: S.opt } }).then(v => { if (P && P.index === ask.index) { P.vol = v; drawFoot(P.data) } }).catch(() => {});
  } catch (e) { clearTimeout(slow); stage.classList.remove('dim'); ld.hidden = false; fill(ld, h('b', {}, 'Страница не открылась'), h('small', {}, String(e))); }
}

function drawStage(d, ids) {
  const stage = $('#stage', P.sheet);
  const figs = [h('figure', { 'data-kind': 'orig' }, h('img', { class: 'page-orig', src: d.orig.url, alt: 'Исходная страница', onclick: e => zoom(e.target.src) }),
    h('figcaption', { title: `${d.orig.w}×${d.orig.h} · ${d.orig.format.toUpperCase()}` }, h('b', {}, 'Сейчас'), h('br'), size(d.orig.bytes)))];
  for (const id of ids) {
    const r = reader(id); if (!r || !d.sims[id]) continue;
    const dev = h('div', { class: 'device', 'data-id': id, style: { '--body': r.body.color }, onclick: () => zoom(d.sims[id]) }, h('img', { src: d.sims[id], alt: `Страница на ${r.name}` }));
    figs.push(h('figure', { 'data-kind': 'device', 'data-id': id }, dev,
      h('figcaption', {}, h('b', {}, r.name), h('br'), `${inch(r.screen.inches)} · ${r.screen.color ? 'цветной экран' : `${r.screen.gray_levels} оттенков серого`}`)));
  }
  fill(stage, ...figs);
  $('#pg', P.sheet).textContent = `стр. ${d.page + 1} из ${d.pages}`;
  layoutStage();
}

// раскладка по настоящим размерам: корпуса в миллиметрах, все стоят на одной линии
function layoutStage() {
  if (!P || !P.data) return;
  const stage = $('#stage', P.sheet); const H = stage.clientHeight - 28 - 18 - 56; const W = stage.clientWidth - 56;
  const figs = $$('figure', stage); const devs = figs.filter(f => f.dataset.kind === 'device').map(f => reader(f.dataset.id));
  const maxMm = Math.max(...devs.map(r => r.body.height_mm), 1);
  const items = figs.map(f => {
    if (f.dataset.kind === 'orig') { const ar = P.data.orig.w / P.data.orig.h; const hh = (devs.length ? Math.min(...devs.map(r => r.body.height_mm)) / maxMm : 1) * H * 0.86; return { f, w: hh * ar, h: hh } }
    const r = reader(f.dataset.id); const hh = H * r.body.height_mm / maxMm; return { f, r, w: hh * r.body.width_mm / r.body.height_mm, h: hh };
  });
  const gap = 40; const tot = items.reduce((a, i) => a + i.w, 0) + gap * (items.length - 1);
  const k = Math.min(1, W / tot);
  for (const it of items) {
    const w = Math.round(it.w * k), hh = Math.round(it.h * k);
    if (!it.r) { const img = $('img', it.f); img.style.width = w + 'px'; img.style.height = hh + 'px'; continue }
    const dev = $('.device', it.f); const mm = w / it.r.body.width_mm;
    dev.style.width = w + 'px'; dev.style.height = hh + 'px'; dev.style.setProperty('--k', mm);
    const img = $('img', dev); const b = it.r.body;
    Object.assign(img.style, { left: ((b.width_mm - b.screen_width_mm) / 2 * mm) + 'px', top: (b.screen_top_mm * mm) + 'px', width: (b.screen_width_mm * mm) + 'px', height: (b.screen_height_mm * mm) + 'px' });
  }
  stage.style.gap = Math.round(gap * k) + 'px';
}

function drawFoot(d) {
  const r = d.result; const rd = reader();
  const fmt = r.kept ? 'без изменений' : r.gray ? 'чёрно-белая' : 'цветная';
  const tech = r.kept ? 'исходный файл' : r.ext === 'png' ? `PNG, ${r.levels} оттенков серого, ${r.w}×${r.h}` : `JPEG ${S.opt.quality}${r.gray ? ', серый' : ''}, ${r.w}×${r.h}`;
  const heavier = r.bytes > d.orig.bytes;
  const notes = [];
  if (r.why) notes.push(r.why.charAt(0).toUpperCase() + r.why.slice(1) + '.');
  if (rd.screen.height > S.opt.max_height && S.opt.max_height > 0) notes.push(`Страница меньше экрана «${rd.name}» — на читалке она растянется и станет чуть мягче.`);
  if (r.levels === 4) notes.push('Мало оттенков серого — проверьте серые заливки. Щелчок по читалке — увидеть в настоящую величину.');
  if (S.opt.color_fix && d.kind === 'bw' && rd.screen.color) notes.push('«Сочнее цвета» меняет только цветные страницы.');
  const v = P.vol;
  fill($('#foot', P.sheet), 
    h('div', {},
      h('div', { class: 'result-line' + (heavier ? ' warn' : '') }, v ? `Весь том: ${size(v.size)} → ≈ ${size(v.bytes)} (${pct(v.size, v.bytes)})` : 'Весь том: считаю…'),
      h('div', { class: 'sub-line', title: tech }, `Эта страница: ${size(d.orig.bytes)} → ${size(r.bytes)} (${pct(d.orig.bytes, r.bytes)}) · ${fmt} · том ${d.kind === 'bw' ? 'чёрно-белый' : 'цветной'}`),
      v && v.count > 1 ? h('div', { class: 'sub-line' }, `Вся папка примерно: ${size(v.folderTotal)} → ≈ ${size(v.folderGuess)}`) : null),
    h('button', { class: 'btn', onclick: closePreview }, 'Готово'),
    notes.length ? h('ul', { class: 'notes' }, notes.map(n => h('li', {}, n))) : null);
}

function zoom(src) {
  const z = h('div', { class: 'zoom', onclick: () => z.remove(), title: 'Щелчок — закрыть' }, h('img', { src, alt: '' }));
  document.body.append(z);
}

/* ================= сравнение трёх режимов на одной странице */
const MODE_NAMES = { safe: 'Бережно', balanced: 'Сбалансированно', max: 'Максимум места' };
let Q = null;
function openCompare(index = S.sel) {
  if (!S.items.length) return;
  const back = h('div', { class: 'sheet-back', onclick: closeCompare });
  const sheet = h('section', { class: 'sheet cmp-sheet', role: 'dialog', 'aria-label': 'Сравнение режимов' });
  document.body.append(back, sheet);
  Q = { index, page: 2, view: 'sim', back, sheet, data: null };
  Q.key = e => { if (e.key === 'Escape') closeCompare(); if (e.key === 'ArrowRight') qturn(1); if (e.key === 'ArrowLeft') qturn(-1) };
  addEventListener('keydown', Q.key);
  requestAnimationFrame(() => { back.classList.add('open'); sheet.classList.add('open') });
  drawCompare(); loadCompare();
}
function closeCompare() {
  if (!Q) return; removeEventListener('keydown', Q.key);
  Q.back.classList.remove('open'); Q.sheet.classList.remove('open');
  const b = Q.back, s = Q.sheet; setTimeout(() => { b.remove(); s.remove() }, 280); Q = null;
}
function qturn(d) { if (!Q || !Q.data) return; const n = Q.page + d; if (n < 0 || n >= Q.data.pages) return; Q.page = n; loadCompare() }
async function loadCompare() {
  const ask = { index: Q.index, page: Q.page, reader: S.reader };
  // заглушка «Загрузка» — только если ответ не мгновенный (из кэша приходит сразу, тогда без мигания)
  Q.loading = true; Q.slow = false; clearTimeout(Q.slowT); Q.slowT = setTimeout(() => { if (Q && Q.loading) { Q.slow = true; drawCompare(); } }, 150);
  drawCompare();
  try { const d = await call('comics_compare', { args: ask }); if (Q && Q.index === ask.index && Q.page === ask.page) { Q.data = d; Q.page = d.page; Q.dataIndex = ask.index; } }
  catch (e) { toast(String(e)) }
  if (Q) { Q.loading = false; Q.slow = false; clearTimeout(Q.slowT); drawCompare(); }
}
function drawCompare() {
  const d = Q.data, rd = reader();
  const fig = (title, url, lines, mode) => h('figure', { class: 'cmp' + (mode && S.opt.mode === mode ? ' on' : '') },
    h('figcaption', {}, h('b', {}, title), mode && S.opt.mode === mode ? h('span', { class: 'chip' }, 'выбран') : null),
    h('div', { class: 'cmp-img', onclick: () => url && zoom(url) }, url ? h('img', { src: url, alt: title }) : null),
    h('div', { class: 'cmp-info' }, lines.filter(Boolean).map(l => h('div', {}, l))),
    mode ? h('button', { class: 'btn' + (S.opt.mode === mode ? '' : ' primary'), disabled: S.opt.mode === mode, onclick: async () => { await setMode(mode); if (Q) drawCompare(); } }, S.opt.mode === mode ? 'Этот режим выбран' : 'Выбрать этот') : h('div'));
  const fmt = r => r.kept ? 'как в исходнике' : r.ext === 'png' ? `PNG, ${r.levels} оттенков` : `${r.gray ? 'серый' : 'цветной'} JPEG ${r.quality || ''}`.trim();
  fill(Q.sheet,
    h('header', { class: 'sheet-head' },
      h('h2', {}, `Три режима на «${rd.name}»`),
      h('div', { class: 'seg', role: 'group', 'aria-label': 'Что показывать' },
        [['sim', 'Как на экране'], ['file', 'Сам файл']].map(([v, t]) => h('button', { 'aria-pressed': Q.view === v, onclick: () => { Q.view = v; drawCompare() } }, t))),
      h('select', { 'aria-label': 'Том', onchange: e => { Q.index = +e.target.value; Q.page = 2; loadCompare() } }, S.items.map((it, i) => h('option', { value: i, selected: i === Q.index }, it.newName))),
      h('div', { class: 'pager' },
        h('button', { class: 'btn', 'aria-label': 'Предыдущая страница', onclick: () => qturn(-1), html: icon('left') }),
        h('span', {}, d ? `стр. ${d.page + 1} из ${d.pages}` : '…'),
        h('button', { class: 'btn', 'aria-label': 'Следующая страница', onclick: () => qturn(1), html: icon('right') })),
      h('button', { class: 'btn quiet', 'aria-label': 'Закрыть', onclick: closeCompare, html: icon('close') })),
    !d ? h('div', { class: 'cmp-grid cmp-empty' }, loading('Готовлю страницу…', 'сразу в трёх вариантах'))
    : busy(h('div', { class: 'cmp-grid' }, [
      fig('Оригинал', d.orig.url, [`${size(d.orig.bytes)}, ${String(d.orig.format).toUpperCase()}`, `${d.orig.w}×${d.orig.h}`]),
      ...['safe', 'balanced', 'max'].map(m => { const r = d.modes[m]; return fig(MODE_NAMES[m], Q.view === 'sim' ? r.sim : r.file,
        [h('b', {}, `${size(r.bytes)} · ${pct(d.orig.bytes, r.bytes)}`), `${fmt(r)}${r.kept ? '' : `, ${r.w}×${r.h}`}`, r.why ? r.why : null], m); })]),
      Q.slow ? `Загрузка: ${Q.index !== Q.dataIndex ? S.items[Q.index].newName : `стр. ${Q.page + 1}`}` : null),
    h('footer', { class: 'sheet-foot' },
      h('div', { class: 'sub-line' }, 'Щелчок по странице — в натуральную величину. «Как на экране» — с бумагой и контрастом e-ink этой читалки; «Сам файл» — что ляжет в архив.'),
      h('button', { class: 'btn', onclick: closeCompare }, 'Готово')));
}
