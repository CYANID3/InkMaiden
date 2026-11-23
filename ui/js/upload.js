// Общее для «Книг» и «Комиксов»: цепочка «Скачанное → Готовое → На читалке», списки, «Готовое — …», «На читалку».
import { call, pickFolder, openPath } from './api.js';
import { h, icon, toast, lock, menuButton, dialog, size, plural, maidy, mzEmpty } from './ui.js';
import * as Q from './queue.js';

/**
 * Цепочка «Скачанное → Готовое → На читалке» над списком раздела: три места, где живут файлы, и действия между ними.
 * Шаг — это и вкладка списка: «Скачанное» — ваши файлы, «Готовое» — папка готового, «На читалке» — готовое, что уже там.
 * opts: { kind (недавние папки), title (окно выбора), src: { path, count, fresh, tip }, ready: { path, count, wait },
 *   reader: { where, count }, view, onView(id), onPick(path), onMap(), scope, settings, locked, progress }
 */
export function flowBar(ctx, o) {
  const d = ctx.cur(), short = p => String(p || '').split(/[\\/]/).filter(Boolean).pop() || p;
  const choose = async () => { const p = await pickFolder(o.title, o.src.path || ctx.srcFor(o.kind) || undefined); if (p) o.onPick(p); };
  const folderMenu = menuButton('Папка', () => {
    const list = ctx.history(o.kind).filter(p => p !== o.src.path), saved = ctx.savedPaths(), src = ctx.srcFor(o.kind);
    return [
      o.src.path ? { label: 'Обновить', hint: 'перечитать папку — найти новые файлы', onClick: () => o.onPick(o.src.path) } : null,
      o.src.path ? { label: 'Открыть в проводнике', onClick: () => openPath(o.src.path).catch(e => toast(String(e))) } : null,
      { label: 'Другая папка…', onClick: choose },
      src && src !== o.src.path ? [{ sep: 'Основная' }, { label: short(src), hint: src, title: src, onClick: () => o.onPick(src) }] : null,
      saved.length ? [{ sep: 'Мои папки' }, ...saved.map(x => ({ label: x.name, hint: x.path, title: x.path, onClick: () => o.onPick(x.path) }))] : null,
      list.length ? [{ sep: 'Недавние' }, ...list.slice(0, 8).map(p => ({ label: short(p), hint: p, title: p, onClick: () => o.onPick(p) }))] : null,
    ].flat().filter(Boolean);
  }, { icon: 'folder', title: 'Обновить, открыть или сменить папку со скачанным' });
  const step = (id, ic, title, path, badge, tip, disabled) => h('button', { type: 'button', class: 'fstep' + (o.view === id ? ' on' : ''), 'aria-pressed': String(o.view === id),
      disabled: !!disabled, title: tip, onclick: () => { if (o.view !== id) o.onView(id); } },
    h('span', { class: 'fic', html: icon(ic) }),
    h('span', { class: 'ftx' }, h('b', {}, title), h('small', {}, path)),
    badge ? h('span', { class: 'fn' + (badge[1] ? ' w' : '') }, badge[0]) : null);
  const arrow = label => h('span', { class: 'farr', 'aria-hidden': 'true' }, h('span', { html: icon('arrow') }), h('small', {}, label));
  const sp = o.src, rp = o.ready, dp = o.reader;
  return h('section', { class: 'flow' },
    o.progress != null ? h('div', { class: 'progress src-prog', role: 'progressbar', 'aria-valuenow': Math.round(Math.max(0, o.progress) * 100), 'aria-valuemin': 0, 'aria-valuemax': 100 },
      h('i', { class: o.progress < 0 ? 'indet' : '', style: { width: o.progress < 0 ? '30%' : `${Math.max(2, o.progress * 100)}%` } })) : null,
    h('div', { class: 'flow-steps', role: 'tablist', 'aria-label': 'Где файлы' },
      step('src', 'inbox', 'Скачанное', sp.path || 'папка не выбрана', sp.fresh ? [`${sp.fresh} ${plural(sp.fresh, 'новый', 'новых', 'новых')}`, true] : sp.count != null ? [String(sp.count)] : null,
        sp.tip || `Ваша папка со скачанным: ${sp.path}. Программа её не меняет.`),
      arrow('Сохранить в папку'),
      step('ready', 'ready', 'Готовое', rp.path, rp.wait ? [`${rp.wait} ${plural(rp.wait, 'ждёт', 'ждут', 'ждут')}`, true] : [String(rp.count || 0)],
        `Готовое для читалки на компьютере: ${rp.path}. Сюда кладёт «Сохранить в папку», отсюда файлы уходят на читалку.`),
      arrow('На читалку'),
      step('reader', 'readers', 'На читалке', d.connected ? `${d.name} › ${dp.where}` : `${d.name} не подключена`, d.connected && dp.count != null ? [String(dp.count)] : null,
        d.connected ? `Что из готового уже лежит на читалке (${dp.where})` : 'Подключите читалку — покажу, что из готового уже на ней', !d.connected)),
    lock(h('div', { class: 'flow-tools' }, o.scope || null,
      o.onMap ? h('button', { class: 'btn', title: 'Где лежат файлы и что во что превращается', onclick: o.onMap }, h('span', { html: icon('map') }), h('span', { class: 'bl' }, 'Что и куда')) : null,
      o.settings ? settingsButton(o.settings) : null, folderMenu), !!o.locked));
}

/* ---------- «Настройки ▾»: все настройки раздела в одной панели, пояснение у каждой. Открытая панель переживает
   перерисовку раздела (переключили — панель на месте), закрывается щелчком мимо или Esc */
let openSettings = null;
const closeSettings = () => { openSettings = null; document.querySelectorAll('.opts-pop').forEach(p => { p.hidden = true; }); document.querySelectorAll('.opts-btn').forEach(b => b.setAttribute('aria-expanded', 'false')); };
addEventListener('pointerdown', e => { if (openSettings && !e.target.closest('.opts-wrap') && !e.target.closest('dialog')) closeSettings(); });
addEventListener('keydown', e => { if (e.key === 'Escape' && openSettings && !document.querySelector('dialog[open]')) closeSettings(); });
/** opts: { key, summary, groups: [[заголовок, [строки]]] } */
export function settingsButton({ key, summary, groups }) {
  const open = openSettings === key;
  const pop = h('div', { class: 'opts-pop', role: 'dialog', 'aria-label': 'Настройки', hidden: !open },
    ...groups.filter(g => g && g[1].filter(Boolean).length).flatMap(([title, rows]) => [title ? h('h4', {}, title) : null, ...rows.filter(Boolean)]));
  const btn = h('button', { class: 'btn opts-btn', 'aria-haspopup': 'dialog', 'aria-expanded': String(open), title: summary ? `Сейчас: ${summary}. Все настройки раздела — с пояснениями` : 'Все настройки раздела с пояснениями',
    onclick: () => { const was = openSettings === key; closeSettings(); if (!was) { openSettings = key; pop.hidden = false; btn.setAttribute('aria-expanded', 'true'); } } },
    h('span', { html: icon('settings') }), h('span', { class: 'ob' }, 'Настройки', summary ? h('small', {}, summary) : null),
    h('span', { class: 'caret', html: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7 10l5 5 5-5"/></svg>' }));
  return h('div', { class: 'opts-wrap' }, btn, pop);
}

/** строка «Готовое — в папке …» с кнопкой открыть */
export function outLine(path) {
  return h('div', { class: 'where' },
    h('button', { class: 'btn quiet', title: `Открыть папку с готовым: ${path}`, onclick: () => openPath(path).catch(e => toast(String(e))) }, h('span', { html: icon('folder') }), 'Папка с готовым'));
}

/** пустой раздел: перетащить папку, выбрать или сразу открыть папку-источник из настроек */
// с Мэйди (pose) — картинка слева, текст и кнопки справа
export const drop = (title, text, onChoose, src, onOpen, pose = 'hug') => h('section', { class: 'drop' + (pose ? ' with-maidy' : '') },
  pose ? maidy(pose, 'mz-bob') : h('span', { html: icon('folder') }),
  h('div', { class: 'mz-text' }, h('h2', {}, title), h('p', {}, text),
    h('div', { style: { display: 'flex', gap: '10px', flexWrap: 'wrap' } },
      src && onOpen ? h('button', { class: 'btn primary', title: src, onclick: () => onOpen(src) }, `Открыть «${src.split(/[\\/]/).filter(Boolean).pop()}»`) : null,
      h('button', { class: 'btn' + (src && onOpen ? '' : ' primary'), onclick: onChoose }, 'Выбрать папку'))));

/**
 * «На читалку» для «Книг» и «Комиксов»: готовое из папки на компьютере → в список «На читалку» и туда же переход.
 * Пока в папке готового пусто — окно: сначала сохранить (кнопкой раздела), иначе переносить нечего.
 * opts: { label, onSave — «Сохранить в папку» этого раздела, saveLabel }
 */
export function toQueue(ctx, kind, opts = {}) {
  if (typeof opts === 'string') opts = { label: opts };
  // у «Комиксов» готовое лежит в двух папках (Manga и Comics) — opts.kinds
  const d = ctx.cur(), kinds = opts.kinds || [kind], dir = d.dirs[kinds[0]];
  return Q.applyButton(ctx, async () => {
    const has = await Promise.all(kinds.map(k => call('dir_has_files', { path: d.dirs[k] }).catch(() => true)));
    if (!has.some(Boolean)) {
      const go = await dialog(close => h('div', { class: 'dlg' }, h('h2', {}, 'Сначала сохраните в папку'),
        h('p', { style: { margin: 0 } }, `На читалку переносится уже готовое — из папки ${dir.split(/[\\/]/).slice(-2).join('\\')}. Сейчас она пустая: нажмите «${opts.saveLabel || 'Сохранить в папку'}», а потом «На читалку».`),
        h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close('open') }, 'Всё равно открыть «На читалку»'),
          opts.onSave ? h('button', { class: 'btn primary', onclick: () => close('save') }, opts.saveLabel || 'Сохранить в папку') : h('button', { class: 'btn primary', onclick: () => close(false) }, 'Понятно'))));
      if (go === 'save' && opts.onSave) opts.onSave();
      return go === 'open';
    }
    // читалка подключена и готовое сверено — отправляем только то, чего на ней нет; иначе — папку целиком
    // (при проверке в «На читалку» одинаковые файлы всё равно пропустятся)
    const miss = opts.missing ? opts.missing() : null;
    if (miss && !miss.length) { toast('Всё готовое уже на читалке'); return; }
    await Q.add(ctx, miss ? miss.map(f => { const r = String(f.rel).replace(/\\/g, '/'), dr = r.split('/').slice(0, -1).join('/'), k = f.kind || kind;
      return { src: f.path, dst: [d.readerDirs[k], dr].filter(Boolean).join('/'), kind: k, label: r.split('/').pop() }; })
      : kinds.filter((k, i) => has[i]).map(k => ({ src: d.dirs[k], dst: d.readerDirs[k], kind: k, label: opts.label || `Готовое: ${d.dirs[k].split(/[\\/]/).filter(Boolean).pop()}` })), { quiet: true });
  }, { title: `Добавить готовое из ${kinds.map(k => d.dirs[k]).join(' и ')} и открыть «На читалку»` });
}

/* ---------------------------------------------------------------- «Исходные» и «Готовое»
 * Список раздела показывает либо исходную папку (куда вы скачиваете), либо папку готового (что уйдёт на читалку).
 * После сохранения раздел сам переключается на «Готовое». Новые исходные файлы, ещё не сохранённые, видны и там:
 * полоса «Новых: N» с кнопкой «Сохранить новые».
 * S: { view: 'src'|'ready', done: Set индексов сохранённых, ready: [{ rel, path, size, mtime }] }
 */
/** короткая дата: «сегодня 14:05», «вчера», «21 нояб.» */
export function shortDate(sec) {
  if (!sec) return '';
  const d = new Date(sec * 1000), now = new Date(), day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(now) - day(d)) / 864e5);
  if (diff === 0) return `сегодня в ${d.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })}`;
  if (diff === 1) return 'вчера';
  return d.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short', year: d.getFullYear() === now.getFullYear() ? undefined : 'numeric' });
}
/**
 * Одна метка состояния файла — одинаковая в «Книгах», «Комиксах» и «Готово для читалки».
 * s: { saving, err, savedNow, note, edited, ready (файл в готовом: { mtime, onReader }), queued, connected, fresh }
 */
export function stateChip(s) {
  // коротко: «сохранено» / «не сохранено»; когда читалка подключена — ещё «на читалке». Подробности (дата и т. п.) — в подсказке
  const chip = (cls, text, title) => h('span', { class: 'chip st ' + cls, title }, text);
  if (s.saving) return chip('wait', 'сохраняю…', 'Идёт сохранение в папку готового');
  if (s.err) return chip('warn', 'ошибка', s.err);
  if (s.edited) return chip('edit', 'есть правки', 'Вы что-то изменили — нажмите «Сохранить в папку», чтобы правки попали в готовое');
  if (s.savedNow) return chip('done', 'сохранено', ['Только что сохранено в папку готового', s.note].filter(Boolean).join('. '));
  const r = s.ready;
  if (r) {
    const when = r.mtime ? ` ${shortDate(r.mtime)}` : '';
    if (r.onReader === 'same') return chip('ok', 'на читалке', `Сохранено${when} и уже лежит на читалке`);
    if (s.queued) return chip('wait', 'ждёт отправки', 'Добавлено в «На читалку» — запишется там');
    if (r.onReader === 'diff') return chip('warn', 'на читалке другое', 'На читалке файл с тем же именем, но другого размера');
    return chip('done', 'сохранено', `Сохранено${when} в «Готовое»${s.connected ? ', на читалку ещё не отправлено' : ''}`);
  }
  if (s.fresh) return chip('new', 'не сохранено', 'Ещё не сохранено в папку готового');
  return null;
}
/** та же папка на подключённой читалке (для сверки «уже на читалке») или null */
export function readerPath(ctx, kind) {
  const d = ctx.cur(); if (!d || !d.connected) return null;
  return `${d.connected.replace(/[\\/]+$/, '')}\\${String(d.readerDirs[kind] || '').replace(/\//g, '\\')}`;
}
/** полоса о новых, ещё не сохранённых файлах — над списком «Готовое» */
export function freshBar(n, word, { onShow, onSave, busy }) {
  if (!n) return null;
  return h('div', { class: 'fresh-bar' }, h('span', { html: icon('plus') }),
    h('div', {}, h('b', {}, `Новых в «Скачанном»: ${n}`), h('small', {}, `${plural(n, word[0], word[1], word[2])} ещё не ${n === 1 ? 'сохранён' : 'сохранены'} в готовое`)),
    h('span', { class: 'grow' }),
    h('button', { class: 'btn quiet', onclick: onShow }, 'Показать'),
    h('button', { class: 'btn primary', disabled: !!busy, onclick: onSave }, 'Сохранить новые'));
}
/**
 * «Готовое для читалки»: что лежит в папке готового и что из этого уже на читалке.
 * Сверху — сводка (сколько готово, сколько уже на читалке, сколько ещё нет) и одна кнопка: отправить недостающее;
 * ниже — фильтр и файлы со статусом. Двойного щелчка нет: «Показать в папке» и «На читалку» — в меню «⋯» у файла.
 * opts: { ctx, kind, dir (папка готового), scope, icon, thumb(f) → элемент обложки или null, rerender }
 */
export function readyView(S, opts) {
  const { ctx, kind, dir, scope } = opts, d = ctx.cur();
  const nrm = x => String(x || '').replace(/\\/g, '/');
  const all = S.ready.filter(f => !scope || inScope(nrm(f.rel).split('/').slice(0, -1).join('/'), scope));
  const short = dir.split(/[\\/]/).filter(Boolean).slice(-2).join('\\');
  if (opts.only === 'on' && !all.some(f => f.onReader === 'same')) return d.connected
    ? mzEmpty('peek', 'На читалке пока пусто', all.length ? `${all.length} ${plural(all.length, 'файл готов', 'файла готовы', 'файлов готовы')} и ждут. Нажмите «На читалку» — перенесу только то, чего там ещё нет.` : 'Сначала «Сохранить в папку», потом «На читалку» — файлы появятся здесь.')
    : mzEmpty('usb', 'Читалка не подключена', 'Подключите её кабелем и выберите на читалке «Связь с ПК» — покажу, что из готового уже на ней.');
  if (!all.length) return h('div', { class: 'ready-empty' }, h('span', { html: icon('folder') }),
    h('b', {}, 'Готового пока нет'),
    h('p', {}, `Нажмите «Сохранить в папку» — обработанные файлы появятся здесь (папка ${short}). Отсюда они уходят на читалку.`));
  // где каждый файл: на читалке / ждёт в «На читалку» / ещё нет
  const dirs = (opts.dirs || [dir]).map(x => nrm(x).toLowerCase());
  const q = Q.list(ctx), dirQueued = q.some(x => dirs.includes(nrm(x.src).toLowerCase()));
  const queued = new Set(q.map(x => nrm(x.src).toLowerCase()));
  const where = f => f.onReader === 'same' ? 'on' : (dirQueued || queued.has(nrm(f.path).toLowerCase())) ? 'wait' : f.onReader == null ? 'unknown' : 'off';
  const st = all.map(f => [f, where(f)]);
  const n = k => st.filter(([, w]) => w === k).length;
  // opts.skip(f) — готовое, которое не отправлять (например, старая копия после смены вида серии)
  const missing = st.filter(([f, w]) => w === 'off' && !(opts.skip && opts.skip(f))).map(([f]) => f);
  const total = all.reduce((a, f) => a + (f.size || 0), 0), missBytes = missing.reduce((a, f) => a + (f.size || 0), 0);
  const F = opts.only || S.readyFilter || 'all';
  const shown = st.filter(([, w]) => F === 'all' || (F === 'off' ? w === 'off' || w === 'unknown' : F === w));
  const item = f => { const dirRel = nrm(f.rel).split('/').slice(0, -1).join('/'), k = f.kind || kind;
    return { src: f.path, dst: [d.readerDirs[k], dirRel].filter(Boolean).join('/'), kind: k, label: nrm(f.rel).split('/').pop() }; };
  const send = list => Q.addAndGo(ctx, list.map(item));
  // выбор: щелчок по строке; действия с выбранным — в полосе над списком
  const sel = S.readySel = S.readySel || new Set();
  for (const p of [...sel]) if (!all.some(f => f.path === p)) sel.delete(p);
  const picked = all.filter(f => sel.has(f.path));
  const toggle = f => { sel.has(f.path) ? sel.delete(f.path) : sel.add(f.path); opts.rerender(); };
  const remove = async list => {
    const ok = await dialog(close => h('div', { class: 'dlg' }, h('h2', {}, list.length === 1 ? `Убрать «${nrm(list[0].rel).split('/').pop()}» из готового?` : `Убрать из готового: ${list.length}?`),
      h('p', { style: { margin: 0 } }, 'Файлы уйдут в корзину Windows — их можно вернуть оттуда. Ваши файлы не трогаются, на читалке ничего не меняется. Убранное снова станет «новым» — его можно сохранить заново.'),
      h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close(false) }, 'Отмена'), h('button', { class: 'btn primary', onclick: () => close(true) }, 'Убрать в корзину'))));
    if (!ok) return;
    try { const n = await call('ready_remove', { paths: list.map(f => f.path) }); toast(`Убрано в корзину: ${n}`); list.forEach(f => sel.delete(f.path)); if (opts.onRemoved) opts.onRemoved(); }
    catch (e) { toast(String(e)); }
  };
  const QQ = (S.readyQ || '').trim().toLowerCase();
  const found = QQ ? shown.filter(([f]) => `${nrm(f.rel)} ${(opts.title && opts.title(f)) || ''} ${(opts.sub && opts.sub(f)) || ''}`.toLowerCase().includes(QQ)) : shown;
  const multi = sel.size > 0, pick = opts.onPick;
  return h('div', { class: 'ready', title: `Папка ${short}: сюда «Сохранить в папку» кладёт обработанное, отсюда оно уходит на читалку.` },
    // строка над списком — та же, что в «Ваших файлах»: фильтр, поиск, «Выбрать несколько»
    listTools({ key: 'ready',
      filters: d.connected && !opts.only ? [['all', `Все ${all.length}`], n('off') || F === 'off' ? ['off', `Не на читалке ${n('off')}`] : null, n('on') || F === 'on' ? ['on', `На читалке ${n('on')}`] : null, n('wait') ? ['wait', `Ждут ${n('wait')}`] : null].filter(Boolean) : null,
      value: F, onFilter: id => { S.readyFilter = id; opts.rerender(); },
      note: d.connected ? null : h('small', { class: 'hint', style: { margin: 0 }, title: 'Подключите читалку — покажу, что уже на ней' }, 'читалка не подключена'),
      q: S.readyQ || '', onQ: v => { S.readyQ = v; opts.rerender(); },
      all: { keys: () => found.map(([f]) => f.path), sel, onChange: () => opts.rerender() } }),
    // чего нет на читалке — одной строкой над списком, с кнопкой отправить всё недостающее
    !picked.length && missing.length && !opts.only ? h('div', { class: 'fresh-bar miss-bar' }, h('span', { html: icon('apply') }),
      h('div', {}, h('b', {}, `Ещё не на читалке: ${missing.length}`), h('small', {}, `${size(missBytes)} — остальное (${all.length - missing.length}) там уже есть`)),
      h('span', { class: 'grow' }), h('button', { class: 'btn apply', onclick: () => send(missing) }, h('span', { html: icon('apply') }), 'Отправить недостающие')) : null,
    picked.length ? selBar({ count: picked.length, bytes: picked.reduce((a, f) => a + (f.size || 0), 0), onClear: () => { sel.clear(); opts.rerender(); },
      actions: [h('button', { class: 'btn quiet', onclick: () => remove(picked) }, 'Убрать из готового…'),
        h('button', { class: 'btn apply', onclick: () => send(picked) }, h('span', { html: icon('apply') }), `На читалку: ${picked.length}`)] }) : null,
    h('div', { class: 'ready-list' }, found.length ? found.slice(0, 2000).map(([f, w]) => { const parts = nrm(f.rel).split('/'), name = parts.pop();
      return rowEl({ multi, checked: sel.has(f.path), picked: pick && opts.cur === f.path, cls: w, tip: f.rel,
        cover: (opts.thumb && opts.thumb(f)) || h('span', { class: 'ic', html: icon(opts.icon || 'books') }),
        title: (opts.title && opts.title(f)) || name,
        sub: [(opts.sub && opts.sub(f)) || parts.join(' / '), size(f.size)].filter(Boolean).join(' · '),
        tags: [opts.fmt ? h('span', { class: 'chip mute' }, (name.match(/\.(fb2\.zip|[a-z0-9]+)$/i) || ['', ''])[1].toUpperCase()) : null,
          stateChip({ ready: f, queued: w === 'wait', connected: !!d.connected })],
        menu: [
          opts.onOpen ? { label: 'Открыть книгу', onClick: () => opts.onOpen(f) } : null,
          opts.onEdit ? { label: 'Изменить', hint: 'обложку, название, текст — в «Скачанном»', onClick: () => opts.onEdit(f) } : null,
          { label: 'Показать в папке', hint: 'откроется проводник с этим файлом', onClick: () => openPath(f.path).catch(e => toast(String(e))) },
          w !== 'on' && w !== 'wait' ? { label: 'На читалку', hint: 'только этот файл', onClick: () => send([f]) } : null,
          ...(opts.extraMenu ? opts.extraMenu(f) : []),
          { label: 'Убрать из готового…', hint: 'в корзину Windows', onClick: () => remove([f]) },
        ].filter(Boolean),
        onToggle: () => toggle(f), onClick: pick ? () => pick(f) : null, onOpen: opts.onOpen ? () => opts.onOpen(f) : null }); })
      : QQ ? mzEmpty('ask', `По «${S.readyQ.trim()}» ничего нет`, 'Поиск идёт по имени файла, названию и автору.', [h('button', { class: 'btn', onclick: () => { S.readyQ = ''; opts.rerender(); } }, 'Очистить поиск')])
      : h('p', { class: 'empty-note', style: { padding: '20px' } }, F === 'off' ? 'Всё готовое уже на читалке.' : 'Здесь пусто.')));
}

/* ---------------------------------------------------------------- общие части списков («Ваши файлы» и «Готово для читалки») */
/** строка над списком: фильтр слева; справа — поиск и «Выбрать несколько» */
export function listTools({ key, filters, value, onFilter, note, extra, q, onQ, all }) {
  let t;
  // флажок «выбрать все» стоит над колонкой флажков строк: пусто / часть / все показанные
  let selAll = null;
  if (all) {
    const keys = all.keys(), n = keys.filter(k => all.sel.has(k)).length;
    const box = h('input', { type: 'checkbox', checked: n > 0 && n === keys.length, 'aria-label': n ? 'Снять выбор' : 'Выбрать все',
      onchange: () => { if (n) all.sel.clear(); else keys.forEach(k => all.sel.add(k)); all.onChange(); } });
    box.indeterminate = n > 0 && n < keys.length;
    selAll = h('label', { class: 'sel-all', title: n ? 'Снять выбор' : 'Выбрать все показанные — или отмечайте строки по одной (флажок слева у строки, Ctrl+щелчок)' }, box, h('span', { class: 'box', html: CHECK }));
  }
  return h('div', { class: 'list-tools' },
    selAll,
    filters && filters.length > 1 ? h('div', { class: 'seg', role: 'group', 'aria-label': 'Показать' }, filters.map(([id, label]) => h('button', { 'aria-pressed': value === id, onclick: () => onFilter(id) }, label))) : null,
    note || null,
    h('span', { class: 'grow' }),
    extra || null,
    onQ ? h('label', { class: 'search' }, h('span', { html: icon('search') }), h('input', { type: 'search', placeholder: 'Поиск', 'aria-label': 'Поиск', value: q || '', 'data-k': key,
      oninput: e => { const v = e.target.value; clearTimeout(t); t = setTimeout(() => { onQ(v); const i = document.querySelector(`.list-tools input[data-k="${key}"]`); if (i) { i.focus(); i.setSelectionRange(i.value.length, i.value.length); } }, 200); } })) : null);
}
/** полоса «Выбрано: N» с действиями */
export function selBar({ count, bytes, onClear, actions }) {
  return h('div', { class: 'ready-sel' }, h('b', {}, `Выбрано: ${count}`), bytes ? h('span', {}, size(bytes)) : null,
    h('button', { class: 'link', onclick: onClear }, 'снять выбор'), h('span', { class: 'grow' }), ...actions);
}
const CHECK = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>';
/** строка списка: кружок (в режиме выбора), обложка, название, подпись, метки, «⋯». Щелчок — выбрать/посмотреть, двойной — открыть */
export function rowEl({ multi, checked, picked, cls, tip, cover, title, sub, tags, menu, onToggle, onClick, onOpen }) {
  return h('div', { class: 'ready-row' + (cls ? ' ' + cls : '') + (multi ? ' multi' : '') + (checked ? ' sel' : '') + (picked ? ' picked' : ''), role: 'option',
      'aria-selected': String(multi ? !!checked : !!picked), tabindex: 0, title: tip || null,
      onclick: e => { if (e.target.closest('.hist')) return; if (onToggle && (multi || e.ctrlKey || e.metaKey)) onToggle(); else if (onClick) onClick(); },
      ondblclick: e => { if (!multi && onOpen && !e.target.closest('.hist, .rck')) onOpen(); },
      onkeydown: e => { if (e.key === ' ' && onToggle) { e.preventDefault(); onToggle(); } else if (e.key === 'Enter') { if (multi) onToggle(); else if (onOpen) onOpen(); } } },
    // флажок строки: виден при наведении и когда что-то выбрано; место под него есть всегда — строки не прыгают
    onToggle ? h('span', { class: 'rck', role: 'checkbox', 'aria-checked': String(!!checked), 'aria-label': 'Отметить', html: CHECK,
      onclick: e => { e.stopPropagation(); onToggle(); }, ondblclick: e => e.stopPropagation() }) : h('span', { class: 'rck none' }),
    cover,
    h('div', { class: 'nm' }, h('b', {}, title), sub ? h('small', {}, sub) : null),
    h('div', { class: 'tags' }, ...(tags || []).filter(Boolean)),
    menu && menu.length ? menuButton('', menu, { cls: 'quiet icon-only', icon: 'more', title: 'Ещё' }) : h('span'));
}


/* ---------- подпапки: работать с одной, не трогая всю папку */
const norm = d => String(d || '').replace(/\\/g, '/').replace(/^\/+|\/+$/g, '');
/** входит ли папка элемента в выбранную (сама или вложенная); пустая — всё */
export const inScope = (dir, scope) => { const d = norm(dir), s = norm(scope); return !s || d === s || d.startsWith(s + '/'); };
/** список подпапок с числом файлов (вместе с вложенными): [{ dir, name, depth, count }] */
export function subdirs(dirs) {
  const n = new Map();
  for (const d of dirs) { const parts = norm(d).split('/').filter(Boolean); for (let i = 1; i <= parts.length; i++) { const k = parts.slice(0, i).join('/'); n.set(k, (n.get(k) || 0) + 1); } }
  return [...n.entries()].sort((a, b) => a[0].localeCompare(b[0], 'ru')).map(([dir, count]) => ({ dir, count, depth: dir.split('/').length - 1, name: dir.split('/').pop() }));
}
/** выбор «с чем работаем»: вся папка или одна подпапка (со вложенными) */
export function scopeSelect(dirs, scope, onPick, total) {
  const list = subdirs(dirs);
  if (!list.length) return null;
  return h('label', { class: 'scope', title: 'Работать только с одной папкой внутри' }, h('span', {}, 'Показать'),
    h('select', { 'aria-label': 'Какую папку показать', onchange: e => onPick(e.target.value) },
      h('option', { value: '', selected: !scope }, `всё (${total})`),
      list.map(x => h('option', { value: x.dir, selected: norm(scope) === x.dir }, `${' '.repeat(x.depth)}${x.name} (${x.count})`))));
}
