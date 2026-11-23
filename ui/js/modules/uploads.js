// «На читалку»: единственное место, откуда InkMaiden что-либо записывает на читалку.
// Разделы готовят данные на компьютере и добавляют сюда пункты кнопкой «На читалку»: файлы и папки, установку иконки сна,
// её возврат из копии, модуль диагностики и его удаление. Здесь — проверка (что, куда, сколько места и времени) и запись.
// Файлы переносятся по одному с проверкой размера; отличающиеся заменяются только с согласия.
import { call, on, pickFolder, pickFiles } from '../api.js';
import { $, h, fill, icon, size, plural, toast, progress, busy, dialog, lock, menuButton, maidy, mzEmpty, dateTime } from '../ui.js';
import * as Q from '../queue.js';
import { confirm } from './editor.js';

let C, root, offs = [], P = null, R = null; // P — проверка, R — идёт запись
const open = new Set(); // группы, показанные целиком (не только первые SHOW строк)
const shut = new Set(), known = new Set(); // свёрнутые группы; группы, которые уже видели (большие сворачиваются при первом показе)
const BIG = 12;
const KIND = { books: 'books', comics: 'comics', manga: 'comics', wallpapers: 'wallpaper', sleep: 'moon', 'sleep-restore': 'moon', diag: 'settings', 'diag-remove': 'settings', desktop: 'grid', 'desktop-restore': 'grid', other: 'folder' };
const time = s => s < 60 ? `${Math.max(1, Math.round(s))} с` : s < 3600 ? `${Math.floor(s / 60)} мин ${Math.round(s % 60)} с` : `${Math.floor(s / 3600)} ч ${Math.round((s % 3600) / 60)} мин`;
const base = p => p.split(/[\\/]/).filter(Boolean).pop() || p;
const SHOW = 6; // сколько пунктов группы видно сразу

export default {
  id: 'uploads', title: 'На читалку', hint: 'всё, что запишется', icon: 'apply',
  mount(main, ctx) {
    C = ctx; root = h('div', { class: 'page' }); main.append(root);
    offs.push(ctx.onDevices(() => { if (root.isConnected && !R) { P = null; render(); } }), Q.onChange(() => { if (root.isConnected && !R) { P = null; render(); } }));
    on('queue://progress', p => { if (R) { R.p = p; drawRun(); } }).then(u => offs.push(u));
    render();
  },
  show() { if (!R) { P = null; render(); } },
  unmount() { offs.forEach(f => typeof f === 'function' && f()); offs = []; },
  drop(paths) { if (root && root.isConnected) addPaths(paths); },
};

function render() {
  const d = C.cur(), items = Q.list(C);
  const acts = items.filter(x => Q.ACTIONS.has(x.kind)), files = items.filter(x => !Q.ACTIONS.has(x.kind));
  fill(root,
    h('header', { class: 'page-head' }, h('div', {}, h('h1', {}, 'На читалку'),
      h('p', {}, `Всё, что нужно записать на «${d.name}». Проверьте список и нажмите «Проверить и записать».`))),
    h('section', { class: 'panel up-bar' + (d.connected ? '' : ' off') },
      h('div', { class: 'up-dev' }, d.photo ? h('img', { src: d.photo, alt: '' }) : h('span', { html: icon('readers') }),
        h('div', {}, h('b', {}, d.name), h('small', {}, h('i', { class: 'dot' + (d.connected ? ' on' : '') }), d.connected ? `подключена, диск ${d.connected.replace(/\\$/, '')}${d.seen.free != null ? ` · свободно ${size(d.seen.free)}` : ''}` : 'не подключена — список сохранится, запись после подключения'))),
      h('span', { class: 'grow' }),
      groupKeys(items).length > 1 ? h('button', { class: 'btn quiet', onclick: () => { const all = groupKeys(items), was = all.every(k => shut.has(k)); all.forEach(k => was ? shut.delete(k) : shut.add(k)); render(); } },
        groupKeys(items).every(k => shut.has(k)) ? 'Развернуть все' : 'Свернуть все') : null,
      lock(menuButton('Добавить', [{ label: 'Файлы…', onClick: addFiles }, { label: 'Папку…', onClick: addFolder }], { icon: 'folder' }), !!R),
      d.connected ? h('button', { class: 'btn quiet', disabled: !!R, title: 'Отключить читалку от компьютера безопасно', onclick: () => eject(C) }, 'Безопасно извлечь') : null),
    // читалка не подключена — Мэйди с кабелем: как подключить (список ниже сохраняется и запишется после подключения)
    !d.connected && !R ? connectPanel(d, !items.length) : null,
    items.length ? listView(acts, files) : d.connected ? h('section', { class: 'drop up-empty' }, mzEmpty('sleep', 'Пока переносить нечего',
      'Нажмите «На читалку» в любом разделе — готовое появится здесь. Или перетащите сюда свои файлы.'), h('span', { class: 'mz-zz', style: { left: 'calc(50% - 250px)', top: '24%' } }, 'z')) : null,
    R ? (R.el || (R.el = h('section', { class: 'panel xf-panel', id: 'run', role: 'progressbar', 'aria-label': 'Запись на читалку', 'aria-valuemin': 0, 'aria-valuemax': 100 }))) : P ? checkPanel() : null,
    h('div', { class: 'actions' },
      h('div', { class: 'where' }, items.length ? `Ждёт: ${items.length} ${plural(items.length, 'пункт', 'пункта', 'пунктов')}` : 'Список пуст'),
      items.length ? h('button', { class: 'btn quiet', disabled: !!R, onclick: async () => { if (await confirm('Очистить список?', 'Файлы на компьютере не удаляются — только список того, что ждёт записи.', 'Очистить')) { await Q.clear(C); P = null; render(); } } }, 'Очистить') : null,
      R ? h('button', { class: 'btn', onclick: () => call('job_cancel', { id: 'queue' }) }, 'Остановить')
        : P && P !== 'checking'
          ? [h('button', { class: 'btn quiet', onclick: check }, 'Проверить заново'),
            h('button', { class: 'btn apply', disabled: !d.connected || !todo().length || P.short, onclick: run }, h('span', { html: icon('apply') }), `Записать: ${todo().length}`)]
          : h('button', { class: 'btn apply', disabled: !items.length || !d.connected || P === 'checking', title: d.connected ? 'Проверить, что и куда запишется' : `${d.name} не подключена`, onclick: check }, h('span', { html: icon('apply') }), 'Проверить и записать')));
  if (R) drawRun();
}

/** «Подключите читалку» с Мэйди: большой — когда список пуст, узкой полосой — над списком */
function connectPanel(d, big) {
  const when = d.lastOff || d.lastSeen, last = when ? `Последний раз ${d.name} была подключена ${dateTime(when)}.` : null;
  return h('section', { class: 'panel mz-connect' + (big ? '' : ' small') }, maidy('usb', 'mz-bob'),
    h('div', {}, h('h2', {}, `Подключите ${d.name}`),
      big ? h('ol', { class: 'mz-steps' }, h('li', {}, h('i', {}, '1'), h('span', {}, 'Соедините читалку с компьютером кабелем USB.')),
        h('li', {}, h('i', {}, '2'), h('span', {}, 'На читалке выберите «Связь с ПК».')), h('li', {}, h('i', {}, '3'), h('span', {}, 'Подождите пару секунд — я её узнаю сама.')))
        : h('p', { class: 'hint', style: { margin: 0 } }, 'Кабель USB → на читалке «Связь с ПК». Список ниже сохранится и запишется, когда читалка подключится.'),
      big && last ? h('p', { class: 'hint', style: { margin: '6px 0 0' } }, last) : null));
}
/* ---------------------------------------------------------------- список */
function dstOptions(cur) {
  const r = C.cur().readerDirs;
  return [...new Set([r.books, r.comics, r.manga, r.wallpapers, r.other, cur].filter(Boolean))];
}
// куда положить: меню вместо выпадающего списка в каждой строке
const moveMenu = (ids, cur) => [{ sep: 'Папка на читалке' },
  ...dstOptions(cur).map(o => ({ label: o === cur ? `${o}  ✓` : o, disabled: o === cur, onClick: () => Q.updateMany(C, ids, { dst: o }) })),
  { label: 'Другая папка…', onClick: async () => { const v = await askDir(cur); Q.updateMany(C, ids, { dst: v.replace(/\\/g, '/').replace(/^\/+|\/+$/g, '') }); } }];

// что именно сделает действие — заранее и подробно
function note(it) {
  const th = (C.cur().seen || {}).theme || 'Line', dir = `system/resources/${th}`;
  // простыми словами; точные пути — во всплывающей подсказке
  const note = (title, ...lines) => h('div', { class: 'up-note', title }, ...lines.map(l => h('div', {}, l)));
  if (it.kind === 'sleep') return note(`${dir}/taskmgr_lock_background.bmp`, 'Заменит иконку сна на читалке.', 'Прежняя иконка сохранится на компьютере.');
  if (it.kind === 'sleep-restore') return note(dir, it.stamp === 'default' ? 'Вернёт стандартную иконку сна.' : 'Вернёт прежнюю иконку сна.', 'Текущая иконка тоже сохранится на компьютере.');
  if (it.kind === 'diag') return note('applications/InkMaiden_Diag.app', 'Запишет маленькое приложение-проверку. Оно только читает сведения о читалке и ничего не меняет.');
  if (it.kind === 'desktop') return note('system/config/desktop/view.json', 'Изменит рабочий стол читалки. После записи читалку нужно перезагрузить.', 'Прежний рабочий стол сохранится на компьютере.');
  if (it.kind === 'desktop-restore') return note('system/config/desktop/view.json', it.stamp === 'factory' ? 'Вернёт заводской рабочий стол.' : 'Вернёт прежний рабочий стол.', 'Текущий тоже сохранится на компьютере.');
  if (it.kind === 'diag-remove') return h('div', { class: 'up-note' },
    h('div', {}, 'Удалит ', h('code', {}, 'applications/InkMaiden_Diag.app'), ' и папку ', h('code', {}, 'system/inkmaiden'), '. Ещё не забранный отчёт сначала скопируется на компьютер.'));
  return null;
}
const row = (it, menu) => h('div', { class: 'up-item' },
  h('span', { class: 'up-ic', html: icon(KIND[it.kind] || 'folder') }),
  h('div', { class: 'up-name' }, h('b', { title: it.src || it.label }, it.label || base(it.src)), it.src && !Q.ACTIONS.has(it.kind) ? h('small', { title: it.src }, `с компьютера: …\\${String(it.src).split(/[\\/]/).filter(Boolean).slice(-3, -1).join('\\')}`) : null, note(it)),
  lock(menuButton('', menu, { cls: 'quiet icon-only', icon: 'more', title: 'Действия' }), !!R));

// ключи групп: '*' — изменения на читалке, иначе папка на читалке
const ACTS = '*';
const groupKeys = items => [...new Set(items.map(x => Q.ACTIONS.has(x.kind) ? ACTS : x.dst))];
const CHEV = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l6 6-6 6"/></svg>';
// группа: заголовок сворачивает и разворачивает её (щелчок по всей строке), справа — «⋯» для всей группы
function group(key, ic, title, list, menu, rows) {
  if (!known.has(key)) { known.add(key); if (list.length > BIG) shut.add(key); }
  const closed = shut.has(key);
  const toggle = () => { closed ? shut.delete(key) : shut.add(key); render(); };
  return h('section', { class: 'up-group' + (closed ? ' closed' : '') },
    h('div', { class: 'up-gh' },
      h('button', { class: 'up-tg', 'aria-expanded': String(!closed), title: closed ? 'Развернуть' : 'Свернуть', onclick: toggle },
        h('span', { class: 'chev', html: CHEV }), h('span', { html: icon(ic) }), h('b', {}, title),
        h('small', {}, `${list.length} ${plural(list.length, 'пункт', 'пункта', 'пунктов')}`),
        closed ? h('em', {}, list.slice(0, 3).map(x => x.label || base(x.src)).join(', ') + (list.length > 3 ? ' …' : '')) : null),
      menu ? lock(menuButton('', menu, { cls: 'quiet icon-only', icon: 'more', title: 'Вся группа' }), !!R) : null),
    closed ? null : rows);
}
function listView(acts, files) {
  const groups = {};
  for (const it of files) (groups[it.dst] = groups[it.dst] || []).push(it);
  // один общий список (как «Ваши файлы» в разделах): группы по папкам на читалке — заголовками внутри
  const body = h('div', { class: 'panel up-list' },
    acts.length ? group(ACTS, 'readers', 'Изменения на читалке', acts, [{ label: 'Убрать все изменения', onClick: () => Q.remove(C, acts.map(x => x.id)) }],
      acts.map(it => row(it, [{ label: 'Убрать из списка', onClick: () => Q.remove(C, [it.id]) }]))) : null,
    Object.entries(groups).map(([dst, list]) => {
      const all = open.has(dst) || list.length <= SHOW + 1, shown = all ? list : list.slice(0, SHOW);
      return group(dst, 'folder', dst || 'корень читалки', list,
        [...moveMenu(list.map(x => x.id), dst), { sep: '' }, { label: 'Убрать всю группу', onClick: () => Q.remove(C, list.map(x => x.id)) }],
        [...shown.map(it => row(it, [...moveMenu([it.id], it.dst), { sep: '' }, { label: 'Убрать из списка', onClick: () => Q.remove(C, [it.id]) }])),
          all ? null : h('button', { class: 'btn quiet up-more', onclick: () => { open.add(dst); render(); } }, `Показать ещё ${list.length - SHOW}`)]);
    }));
  return R ? busy(body, 'Идёт запись на читалку…') : P === 'checking' ? busy(body, 'Проверяю…') : body;
}

/* ---------------------------------------------------------------- проверка */
const todo = () => P && P.files ? P.files.filter(f => f.state === 'new' || f.state === 'icon' || f.state === 'act' || (f.state === 'changed' && P.replace.has(f.dst))) : [];
async function check() {
  P = 'checking'; render();
  try {
    const items = Q.list(C), d = C.cur(), dev = d.id, th = (d.seen || {}).theme || 'Line';
    // файлы — копирование; действия — что изменится на читалке (иконка сна — от sleep_plan)
    const plain = items.map((x, i) => [x, i]).filter(([x]) => !Q.ACTIONS.has(x.kind));
    const r = await call('queue_check', { device: dev, items: plain.map(([x]) => ({ src: x.src, dst: x.dst })) });
    r.files.forEach(f => { f.item = plain[f.item][1]; });
    for (const [x, i] of items.map((x, i) => [x, i]).filter(([x]) => Q.ACTIONS.has(x.kind))) {
      const act = { item: i, act: x.kind, work: x.work, stamp: x.stamp, src: x.label };
      if (x.kind === 'sleep') {
        try { (await call('sleep_plan', { device: dev, name: x.work })).forEach(f => r.files.push({ ...f, ...act, src: f.src, state: f.state === 'same' ? 'same' : 'icon' })); }
        catch (e) { r.files.push({ ...act, dst: '—', size: 0, state: 'missing' }); }
      } else if (x.kind === 'sleep-restore') r.files.push({ ...act, dst: `system/resources/${th}/taskmgr_lock_background*.bmp`, size: 0, state: 'act' });
      else if (x.kind === 'diag') r.files.push({ ...act, dst: 'applications/InkMaiden_Diag.app', size: 12 * 1024, state: 'act' });
      else if (x.kind === 'diag-remove') r.files.push({ ...act, dst: 'applications/InkMaiden_Diag.app, system/inkmaiden', size: 0, state: 'act' });
      else if (x.kind === 'desktop' || x.kind === 'desktop-restore') r.files.push({ ...act, plan: x.plan, dst: 'system/config/desktop/view.json', size: 0, state: 'act' });
    }
    P = { ...r, items, replace: new Set(), open: false };
  } catch (e) { P = null; toast(String(e)); }
  render();
}
function checkPanel() {
  if (P === 'checking') return h('section', { class: 'panel' }, progress({ stage: 'Проверяю список', detail: 'смотрю файлы на компьютере и что уже есть на читалке' }));
  const f = P.files, by = s => f.filter(x => x.state === s);
  const t = todo(), bytes = t.reduce((a, x) => a + (x.size || 0), 0);
  P.short = P.free != null && bytes > P.free;
  const eta = bytes / Q.speed(C);
  const stateName = { new: 'новый', same: 'уже есть', changed: 'другая версия', missing: 'нет на компьютере — сначала «Сохранить» в разделе', icon: 'заменит текущую (прежняя сохранится на компьютере)', act: 'изменение' };
  return h('section', { class: 'panel up-check' },
    h('div', { class: 'nums' },
      h('div', {}, h('b', {}, t.length), h('small', {}, `${plural(t.length, 'запись', 'записи', 'записей')} на читалку`)),
      h('div', {}, h('b', {}, size(bytes)), h('small', {}, 'нужно места')),
      h('div', {}, h('b', { style: { color: P.short ? 'var(--err)' : '' } }, P.free != null ? size(P.free) : '—'), h('small', {}, 'свободно на читалке')),
      h('div', {}, h('b', {}, bytes ? `≈ ${time(eta)}` : '—'), h('small', {}, C.config.uploadSpeed ? `при ${size(Q.speed(C))}/с, как в прошлый раз` : 'по обычной скорости USB'))),
    P.short ? h('p', { class: 'hint', style: { color: 'var(--err)', margin: 0 } }, 'Места не хватит — уберите часть пунктов или освободите память на читалке.') : null,
    h('p', { class: 'hint', style: { margin: 0 } }, [by('new').length ? `новых файлов: ${by('new').length}` : null, by('icon').length ? `иконка сна: ${by('icon').length} ${plural(by('icon').length, 'файл', 'файла', 'файлов')}, прежняя — копией на компьютер` : null,
      by('act').length ? `изменений: ${by('act').length}` : null, by('same').length ? `уже на читалке: ${by('same').length} — их не записываю` : null,
      by('changed').length ? `на читалке другая версия: ${by('changed').length} — заменятся только отмеченные` : null, by('missing').length ? `не найдено на компьютере: ${by('missing').length} — сначала нажмите «Сохранить» в разделе` : null].filter(Boolean).join(', ') + '.'),
    by('changed').length ? h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: P.replace.size === by('changed').length, onchange: e => { P.replace = new Set(e.target.checked ? by('changed').map(x => x.dst) : []); render(); } }), 'Заменить все другие версии') : null,
    h('details', { class: 'fine', open: P.open, ontoggle: e => { P.open = e.target.open; } }, h('summary', {}, `Подробно: ${f.length}`),
      h('div', { class: 'up-files' }, h('table', { class: 'models-table' },
        h('thead', {}, h('tr', {}, h('th', {}, 'Что'), h('th', {}, 'Куда на читалке'), h('th', {}, 'Размер'), h('th', {}, 'Что сейчас на читалке'))),
        h('tbody', {}, f.slice(0, 2000).map(x => h('tr', { class: 'st-' + x.state },
          h('td', { title: x.src }, x.act && x.act !== 'sleep' ? x.src : base(String(x.src))), h('td', {}, x.dst), h('td', {}, x.state === 'missing' || !x.size ? '—' : size(x.size)),
          h('td', {}, x.state === 'changed' ? h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: P.replace.has(x.dst), onchange: e => { e.target.checked ? P.replace.add(x.dst) : P.replace.delete(x.dst); render(); } }), 'заменить') : stateName[x.state]))))))));
}

/* ---------------------------------------------------------------- запись */
// действие с читалкой → вызов (и что записать в итог)
const ACT = {
  sleep: x => call('sleep_install', { device: C.cur().id, name: x.work }).then(r => r.log || []),
  'sleep-restore': x => call('sleep_restore', { device: C.cur().id, stamp: x.stamp }),
  diag: () => call('diag_install', { id: C.cur().id }).then(() => ['записан applications/InkMaiden_Diag.app — на читалке: «Приложения» → «@InkMaiden_Diag»']),
  'diag-remove': () => call('diag_remove', { id: C.cur().id }).then(() => ['проверка убрана с читалки; отчёт — на компьютере']),
  desktop: x => call('desktop_apply', { device: C.cur().id, plan: x.plan }),
  'desktop-restore': x => call('desktop_restore', { device: C.cur().id, stamp: x.stamp }),
};
async function run() {
  const all = todo(), files = all.filter(x => !x.act).map(x => ({ src: x.src, dst: x.dst }));
  const acts = [...new Map(all.filter(x => x.act).map(x => [x.item, x])).values()];
  if (!files.length && !acts.length) return;
  const plan = P;
  R = { p: null, total: files.length + acts.length, line: null }; render();
  let r = files.length ? null : { copied: [], bytes: 0, secs: 0, errors: [], cancelled: false };
  if (files.length) { try { r = await call('queue_run', { device: C.cur().id, files }); } catch (e) { toast(String(e)); } }
  // действия — после файлов; журнал — в итоговом окне
  const log = [], doneActs = new Set();
  for (const [k, x] of acts.entries()) {
    if (!r || r.cancelled) break;
    R.p = { done: files.length + k, total: R.total, name: plan.items[x.item].label, bytes: r.bytes, all: 0, secs: 0 }; drawRun();
    try { log.push(...((await ACT[x.act](x)) || [])); doneActs.add(x.item); all.filter(f => f.item === x.item).forEach(f => { r.copied.push(f.dst); r.bytes += f.size || 0; }); }
    catch (e) { r.errors.push(`${plan.items[x.item].label}: ${e}`); }
  }
  R = null; P = null;
  if (r) {
    await Q.learnSpeed(C, r.bytes, r.secs);
    // пункт уходит из списка, когда с ним всё решено: записан, уже был таким же или его решили не заменять;
    // остаётся, если что-то не записалось или файла нет на компьютере
    const done = new Set(r.copied), want = new Set(all.map(f => f.dst));
    const gone = plan.items.filter((it, i) => { const fs = plan.files.filter(f => f.item === i);
      if (Q.ACTIONS.has(it.kind)) return doneActs.has(i) || (fs.length && fs.every(f => f.state === 'same'));
      return fs.length && fs.every(f => f.state !== 'missing' && (want.has(f.dst) ? done.has(f.dst) : true)); }).map(it => it.id);
    if (gone.length) await Q.remove(C, gone);
    report(plan, r, log);
  }
  await C.refreshDevices(); render();
}
// итог — окном: что записано и куда, что пропущено и почему, ошибки; журнал действий
function report(plan, r, log) {
  const done = new Set(r.copied), errs = r.errors || [];
  const wrote = plan.files.filter(f => done.has(f.dst));
  const skipped = plan.files.filter(f => !done.has(f.dst) && !errs.some(e => e.startsWith(f.dst)));
  const why = f => f.state === 'same' ? 'уже есть на читалке' : f.state === 'missing' ? 'файла нет на компьютере' : f.state === 'changed' ? 'на читалке другая версия — не заменял' : r.cancelled ? 'остановлено' : '';
  const ok = wrote.length && !errs.length;
  const reboot = wrote.some(f => !f.act || f.act === 'sleep' || f.act === 'sleep-restore' || f.act === 'desktop' || f.act === 'desktop-restore');
  // Мэйди по итогу: радуется, если всё записано; спрашивает, если были ошибки; дремлет, если записывать было нечего
  const pose = r.cancelled || !wrote.length ? 'sleep' : errs.length ? 'ask' : 'cheer';
  return dialog(close => h('div', { class: 'dlg dlg-wide' },
    h('div', { class: 'mz-head' }, maidy(pose, pose === 'cheer' ? 'mz-pop' : 'mz-breathe'), h('div', {},
    h('h2', {}, r.cancelled ? 'Запись остановлена' : !wrote.length ? 'На читалку ничего не записано' : errs.length ? 'Записано, но с ошибками' : 'Готово! Всё на читалке'),
    h('p', { style: { margin: 0 } }, [
      wrote.length ? `Записано ${wrote.length} ${plural(wrote.length, 'файл', 'файла', 'файлов')} (${size(r.bytes)}) за ${time(r.secs || 1)}.` : null,
      skipped.length ? `Не записано ${skipped.length} — подробности ниже.` : null,
      errs.length ? `Не получилось: ${errs.length}.` : null,
      wrote.some(f => f.act === 'sleep' || f.act === 'sleep-restore') ? 'Прежняя иконка сна сохранена на компьютере: «Иконка сна» → «Мои работы» → «Резервные копии».' : null].filter(Boolean).join(' ')))),
    // после записи читалку нужно перезагрузить — иначе прошивка может не увидеть новую иконку сна, обои и книги
    reboot ? h('div', { class: 'restart-note', role: 'note' }, h('span', { html: icon('refresh') }),
      h('div', {}, h('b', {}, 'Обязательно перезагрузите читалку'),
        h('small', {}, `${wrote.some(f => f.act === 'desktop' || f.act === 'desktop-restore') ? 'Рабочий стол' : wrote.some(f => f.act === 'sleep' || f.act === 'sleep-restore') ? 'Иконка сна' : wrote.some(f => /wallpaper|Wallpapers|logo/i.test(f.dst)) ? 'Новые обои' : 'Новые файлы'} появятся только после перезагрузки. ` +
          'Нажмите «Безопасно извлечь», отключите кабель, затем на читалке: удерживайте кнопку питания → «Перезагрузка».'))) : null,
    wrote.length ? h('details', { class: 'log', open: wrote.length <= 12 }, h('summary', {}, 'Что записано'), h('pre', {}, wrote.map(f => `${f.dst}${f.size ? `  ·  ${size(f.size)}` : ''}`).join('\n'))) : null,
    skipped.length ? h('details', { class: 'log', open: !wrote.length }, h('summary', {}, 'Что не записано и почему'), h('pre', {}, skipped.map(f => `${f.dst}  —  ${why(f)}`).join('\n'))) : null,
    errs.length ? h('details', { class: 'log', open: true }, h('summary', {}, 'Что не получилось'), h('pre', {}, errs.join('\n'))) : null,
    log.length ? h('details', { class: 'log' }, h('summary', {}, 'Подробный журнал'), h('pre', {}, log.join('\n'))) : null,
    h('div', { class: 'dlg-foot' },
      wrote.length && C.cur().connected ? h('button', { class: 'btn', onclick: () => { close(true); eject(C); } }, 'Безопасно извлечь') : null,
      h('button', { class: 'btn primary', onclick: () => close(true) }, ok ? 'Готово' : 'Понятно'))));
}
// ход записи: читалка справа, экран которой заполняется строками по мере записи (как страница на e-ink),
// по «кабелю» к ней плывут листы; заполнилась строка — мягкая перерисовка экрана (не чаще раза в 2 с).
// Каркас строится один раз, дальше меняются только значения — анимации не перезапускаются.
const LINES = 16;
function runView() {
  return h('div', { class: 'xfer with-maidy' }, maidy('hug', 'mz-busy'),
    h('div', { class: 'xf-info' },
      h('div', { class: 'xf-pct' }, h('b', { class: 'xf-num' }, '0'), h('span', {}, '%')),
      h('div', { class: 'xf-stage' }), h('div', { class: 'xf-name' }), h('div', { class: 'xf-meta' }),
      h('div', { class: 'xf-bar' }, h('i'))),
    h('div', { class: 'xf-flow', 'aria-hidden': 'true' }, [0, 1, 2, 3].map(k => h('span', { class: 'sheet', style: { animationDelay: `${k * 0.45}s` } }))),
    h('div', { class: 'xf-reader', 'aria-hidden': 'true' },
      h('div', { class: 'xf-screen' }, Array.from({ length: LINES }, (_, k) => h('i', { class: 'ln', style: { '--w': `${k % 5 === 4 ? 55 : 82 + ((k * 37) % 17)}%` } }))),
      h('div', { class: 'xf-btns' }, h('i'), h('i'))));
}
function drawRun() {
  const el = $('#run', root); if (!el || !R) return;
  if (!el.firstChild) el.append(runView());
  const p = R.p;
  const v = p && p.all ? p.bytes / p.all : p ? p.done / Math.max(1, p.total) : 0;
  const sp = p && p.secs > 1 ? p.bytes / p.secs : Q.speed(C);
  const left = p && p.all ? (p.all - p.bytes) / sp : null;
  const pct = Math.min(100, Math.floor(v * 100));
  $('.xf-num', el).textContent = pct;
  $('.xf-stage', el).textContent = p && p.name ? `Записываю ${Math.min(p.done + 1, p.total)} из ${p.total}` : 'Начинаю запись';
  $('.xf-name', el).textContent = p && p.name ? p.name : '';
  $('.xf-meta', el).textContent = p ? [p.all ? `${size(p.bytes)} из ${size(p.all)}` : null, p.secs > 1 ? `${size(sp)}/с` : null, left != null && p.secs > 1 ? `осталось ≈ ${time(left)}` : null].filter(Boolean).join(' · ') : '';
  $('.xf-bar i', el).style.transform = `scaleX(${Math.max(0.005, v)})`;
  // строки экрана: каждая заполняется своей долей
  el.querySelectorAll('.ln').forEach((ln, k) => ln.style.setProperty('--f', Math.max(0, Math.min(1, v * LINES - k)).toFixed(3)));
  el.querySelector('.xfer').setAttribute('aria-valuenow', pct);
  // заполнилась строка — мягкая перерисовка экрана, как на e-ink; не чаще раза в 2 с (файлов бывают сотни)
  const line = Math.floor(v * LINES), now = Date.now();
  if (R.line != null && line > R.line && now - (R.flashAt || 0) > 2000) {
    const sc = $('.xf-screen', el); sc.classList.remove('flash'); void sc.offsetWidth; sc.classList.add('flash'); R.flashAt = now; }
  R.line = line;
}

/* ---------------------------------------------------------------- добавить свои */
function guessKind(p) {
  const l = p.toLowerCase();
  if (/\.(fb2|epub|fb3|pdf|djvu|mobi|azw3?|txt|rtf|docx?|fb2\.zip)$/.test(l)) return 'books';
  if (/\.(cbz|cbr|cb7|cbt)$/.test(l)) return 'comics';
  if (/\.(bmp|png|jpe?g)$/.test(l)) return 'wallpapers';
  return 'other';
}
async function addPaths(paths) {
  const r = C.cur().readerDirs;
  await Q.add(C, paths.map(p => { const k = guessKind(p); return { src: p, kind: k, dst: r[k] || r.other, label: base(p) }; }));
}
async function addFiles() { const p = await pickFiles('Файлы для читалки', C.srcFor('other') || undefined); if (p.length) addPaths(p); }
async function addFolder() {
  const p = await pickFolder('Папка для читалки', C.srcFor('other') || undefined); if (!p) return;
  await Q.add(C, [{ src: p, kind: 'other', dst: `${C.cur().readerDirs.other}/${base(p)}`, label: `${base(p)} (папка)` }]);
}

/** Безопасно извлечь читалку (общая функция — и из «Моего устройства») */
// извлечение — с понятным окном: идёт, получилось (можно вынимать кабель) или нет и почему
export async function eject(ctx) {
  const d = ctx.cur();
  if (R) { toast('Идёт запись на читалку — дождитесь окончания или остановите её.'); return; }
  let setState;
  const shown = dialog(close => {
    const body = h('div', { class: 'dlg eject-dlg' });
    setState = (kind, title, text) => fill(body,
      h('div', { class: 'eject-ic ' + kind, html: kind === 'run' ? '' : kind === 'ok' ? '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>' : '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M12 7v6M12 17h.01"/></svg>' },
        kind === 'run' ? h('span', { class: 'op-spin' }) : null),
      h('h2', {}, title), h('p', { style: { margin: 0 } }, text),
      kind === 'run' ? null : h('div', { class: 'dlg-foot' }, h('button', { class: 'btn primary', onclick: () => close(true) }, 'Готово')));
    setState('run', `Извлекаю ${d.name}…`, 'Записываю всё на диск читалки и отключаю его от Windows. Не вынимайте кабель.');
    return body;
  });
  // если Windows долго молчит — сказать об этом, не оставляя окно «висеть» без объяснений
  const slow = setTimeout(() => setState('run', `Извлекаю ${d.name}…`, 'Windows отвечает дольше обычного — возможно, файлы на читалке открыты в Проводнике или другой программе. Подождите ещё немного.'), 8000);
  try {
    await call('device_eject', { id: d.id });
    clearTimeout(slow);
    setState('ok', `${d.name} извлечена`, 'Можно отключать кабель. Если что-то записывали — перезагрузите читалку (удерживайте кнопку питания → «Перезагрузка»), иначе новые файлы, иконка сна и обои могут не появиться.');
    await ctx.refreshDevices();
  } catch (e) {
    clearTimeout(slow);
    setState('err', 'Не удалось извлечь', String(e));
  }
  return shown;
}

/** своя папка на читалке */
const askDir = cur => dialog(close => h('form', { class: 'dlg', onsubmit: e => { e.preventDefault(); close(e.target.d.value.trim() || cur) } },
  h('h2', {}, 'Папка на читалке'),
  h('p', { class: 'hint', style: { margin: 0 } }, 'Путь от корня читалки, например Books/Новое. Папки, которой нет, появится.'),
  h('input', { class: 'input', name: 'd', value: cur, autofocus: true }),
  h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', type: 'button', onclick: () => close(cur) }, 'Отмена'), h('button', { class: 'btn primary' }, 'Готово')))).then(v => v || cur);
