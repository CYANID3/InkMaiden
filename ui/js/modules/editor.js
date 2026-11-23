// Общая основа разделов «Обои» и «Иконка сна»: верхняя панель, встроенный редактор из «Мастерской»,
// каталог «Мои работы» (выдвижная панель) — открыть, отправить «На читалку», удалить.
// Нижняя панель одинакова у обоих: где лежит готовое · название · «Работы ▾» · «Сохранить» · «На читалку».
import { call } from '../api.js';
import { h, fill, icon, toast, dialog, dateTime, loading, plural } from '../ui.js';
import '../bridge.js';
import { outLine } from '../upload.js';
import * as Q from '../queue.js';

const toB64 = blob => new Promise((res, rej) => { const r = new FileReader(); r.onload = () => res(String(r.result).split(',')[1] || ''); r.onerror = () => rej(r.error); r.readAsDataURL(blob); });
const canvasB64 = cv => cv.toDataURL('image/png').split(',')[1];
export const dataUrl = (b64, mime = 'image/png') => b64.startsWith('data:') ? b64 : `data:${mime};base64,${b64}`;
export { toB64, canvasB64 };

/** уменьшенная копия картинки (Blob или canvas) для каталога — PNG base64 */
export async function thumbOf(src, w = 300) {
  let img = src;
  if (typeof src.getContext !== 'function' && 'size' in src) { // Blob (в т. ч. из окна редактора — instanceof там не работает)
    const url = URL.createObjectURL(src);
    img = await new Promise((res, rej) => { const i = new Image(); i.onload = () => res(i); i.onerror = rej; i.src = url; }).finally(() => setTimeout(() => URL.revokeObjectURL(url), 1000));
  }
  const iw = img.naturalWidth || img.width, ih = img.naturalHeight || img.height, k = Math.min(1, w / iw);
  const c = document.createElement('canvas'); c.width = Math.round(iw * k); c.height = Math.round(ih * k);
  c.getContext('2d').drawImage(img, 0, 0, c.width, c.height);
  return canvasB64(c);
}

/**
 * Раздел-редактор. opt: { id, title, hint, icon, kind ('wallpapers'|'sleep'), src (html), what (подпись работ),
 *   extra(api) → элементы в заголовок, tabs(api) → [{ id, label, render(body, D) }] — вкладки панели «Мои работы»,
 *   single — на читалку можно отправить только одну работу (иконка сна),
 *   save(api) → сохранить в «Мои работы», apply(api) → сохранить и добавить в «На читалку» (false — остаться),
 *   queueWork(api, work) → добавить сохранённую работу в «На читалку», open(api, work, loaded), drop(win, file),
 *   bulk: [{ label, primary, run(api, works) }] — действия над отмеченными работами }
 */
export function editorModule(opt) {
  const api = { C: null, frame: null, name: null, bar: null, off: [] };
  api.win = () => api.frame.contentWindow;
  api.cur = () => api.C.cur();
  api.workName = () => (api.name.value || '').trim();
  api.setName = n => { api.name.value = n || ''; };
  api.refreshBar = () => drawBar();
  api.list = () => call('works_list', { device: api.cur().id, kind: opt.kind });

  // как во всех разделах: заголовок с описанием сверху, все кнопки — в нижней строке, главная — справа
  function drawBar() {
    if (!api.bar) return;
    fill(api.head, h('div', {}, h('h1', {}, opt.title), h('p', {}, opt.desc)), ...(opt.extra ? opt.extra(api) : []));
    if (api.applyBtn && api.applyBtn._off) api.applyBtn._off();
    api.applyBtn = Q.applyButton(api.C, () => opt.apply(api), { title: opt.applyTitle });
    fill(api.bar,
      outLine(api.cur().dirs[opt.kind]),
      api.name,
      h('button', { class: 'btn', title: `Сохранённые ${opt.what}${opt.tabs ? ', ' + opt.tabs(api).map(t => t.label.toLowerCase()).join(', ') : ''}`, onclick: () => works() }, h('span', { html: icon('folder') }), 'Мои работы'),
      h('button', { class: 'btn primary', onclick: () => opt.save(api) }, 'Сохранить'),
      api.applyBtn);
  }

  // одна выдвижная панель на раздел: «Мои работы» и вкладки раздела (у обоев — «Из папки», у иконки — «Резервные копии»)
  function works(tabId = 'works') {
    const tabs = [{ id: 'works', label: 'Мои работы', render: worksTab }, ...(opt.tabs ? opt.tabs(api) : [])];
    const D = drawer(opt.title, [], tabs);
    D.show(tabId);
  }

  function worksTab(body, D) {
    const picked = new Set(); let list = [];
    const open = async w => { try { const r = await call('works_load', { device: api.cur().id, kind: opt.kind, name: w.name }); await opt.open(api, w, r); api.setName(w.name); D.close(); } catch (e) { toast(String(e)); } };
    const remove = async ws => {
      const one = ws.length === 1;
      const ok = await confirm(one ? `Удалить «${ws[0].name}»?` : `Удалить выбранные (${ws.length})?`,
        `${one ? 'Работа удалится' : 'Работы удалятся'} с компьютера вместе с файлами. На читалке ничего не изменится.`, 'Удалить');
      if (!ok) return;
      for (const w of ws) { try { await call('works_delete', { device: api.cur().id, kind: opt.kind, name: w.name }); } catch (e) { toast(String(e)); } }
      load();
    };
    const toReader = async ws => { for (const w of ws) await opt.queueWork(api, w); D.close(); api.C.show('uploads'); };
    const draw = () => {
      const sel = list.filter(w => picked.has(w.name));
      fill(body, tileView({
        items: list, key: w => w.name, picked, onChange: draw, word: ['работа', 'работы', 'работ'],
        thumb: w => w.thumb, title: w => w.name, sub: w => dateTime(w.saved), onOpen: open, openTip: 'Открыть в редакторе',
        note: 'Щелчок — выбрать, двойной щелчок — открыть в редакторе.',
        actions: [
          h('button', { class: 'btn quiet', disabled: !sel.length, onclick: () => remove(sel) }, 'Удалить'),
          ...(opt.bulk || []).map(b => h('button', { class: 'btn quiet', disabled: !sel.length, onclick: () => b.run(api, sel) }, b.label)),
          opt.single && sel.length > 1 ? null : h('button', { class: 'btn apply', disabled: !sel.length, onclick: () => toReader(sel) }, h('span', { html: icon('apply') }), 'На читалку', sel.length > 1 ? h('i', { class: 'n' }, String(sel.length)) : null)],
        empty: h('div', { class: 'drop', style: { padding: '36px' } }, h('span', { html: icon(opt.icon) }), h('h2', {}, 'Пока пусто'), h('p', {}, `Сохранённые ${opt.what} появятся здесь — их можно открыть и поправить.`)) }));
    };
    const load = async () => {
      try { list = await api.list(); } catch (e) { toast(String(e)); }
      for (const n of [...picked]) if (!list.some(w => w.name === n)) picked.delete(n);
      draw();
    };
    fill(body, loading()); load();
  }

  const mod = {
    id: opt.id, title: opt.title, hint: opt.hint, icon: opt.icon, full: true,
    mount(main, ctx) {
      api.C = ctx;
      api.name = h('input', { class: 'input', placeholder: 'Название работы', 'aria-label': 'Название работы', style: { width: '200px' } });
      api.frame = h('iframe', { class: 'tool-frame', src: opt.src, title: opt.title });
      api.head = h('header', { class: 'page-head tool-head' });
      api.bar = h('div', { class: 'actions tool-actions' });
      main.append(h('div', { class: 'tool-page' }, api.head, api.frame, api.bar));
      drawBar();
      api.off.push(ctx.onDevices(() => { drawBar(); try { api.frame.contentDocument.documentElement.dataset.theme = ctx.theme(); } catch (e) {} }));
      if (opt.mount) opt.mount(api);
    },
    async drop(paths) {
      try {
        const r = await call('read_image', { path: paths[0] });
        const b = Uint8Array.from(atob(r.data), c => c.charCodeAt(0)), w = api.win();
        opt.drop(w, new w.File([b], r.name, { type: r.mime }));
        if (!api.workName()) api.setName(r.name.replace(/\.[^.]+$/, ''));
      } catch (e) { toast(String(e)); }
    },
    unmount() { api.off.forEach(f => f && f()); api.off = []; window.inkmaidenBridge.drop(); if (opt.unmount) opt.unmount(api); },
  };
  api.module = mod; // для «Открыть в редакторе» из папки обоев
  api.works = t => works(t);
  return mod;
}

/** выдвижная панель справа; tabs: [{ id, label, render(body, D) }] — вкладки под заголовком */
export function drawer(title, head = [], tabs = null) {
  const back = h('div', { class: 'drawer-back' }), body = h('div', { class: 'body' });
  const close = () => { back.classList.remove('open'); el.classList.remove('open'); removeEventListener('keydown', key); setTimeout(() => { back.remove(); el.remove(); }, 280); };
  const key = e => { if (e.key === 'Escape') close(); };
  const tabBar = tabs && tabs.length > 1 ? h('div', { class: 'dtabs vtabs', role: 'tablist' }) : null;
  const D = { body, close, tab: null };
  D.show = id => {
    const t = (tabs || []).find(x => x.id === id) || tabs[0]; D.tab = t.id;
    if (tabBar) fill(tabBar, tabs.map(x => h('button', { class: 'vtab', role: 'tab', 'aria-selected': String(x.id === t.id), onclick: () => D.show(x.id) }, x.label)));
    fill(body); body.scrollTop = 0; t.render(body, D);
  };
  const el = h('aside', { class: 'drawer' + (tabBar ? ' has-tabs' : ''), role: 'dialog', 'aria-label': title },
    h('header', {}, h('h2', {}, title), ...head, h('button', { class: 'btn quiet', 'aria-label': 'Закрыть', html: icon('close'), onclick: close })), tabBar, body);
  back.addEventListener('click', close); addEventListener('keydown', key);
  document.body.append(back, el);
  requestAnimationFrame(() => { back.classList.add('open'); el.classList.add('open'); });
  return D;
}

const CHECK = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>';
/**
 * Плитки картинок с выбором — одинаково для «Моих работ», «Из папки» и копий.
 * Щелчок — выбрать, двойной щелчок (или Enter, или карандаш) — открыть. Сверху — строка: сколько, «выбрать все», действия.
 */
export function tileView({ items, key, picked, onChange, word, thumb, title, sub, onOpen, openTip, actions = [], top = [], note, empty }) {
  if (!items.length) return [...top, empty || h('p', { class: 'hint' }, 'Пусто')];
  const all = picked.size === items.length;
  const toggle = it => { const k = key(it); picked.has(k) ? picked.delete(k) : picked.add(k); onChange(); };
  const tile = it => { const on = picked.has(key(it));
    return h('div', { class: 'wtile' + (on ? ' picked' : ''), tabindex: 0, role: 'checkbox', 'aria-checked': String(on), 'aria-label': title(it),
        title: onOpen ? `Щелчок — выбрать, двойной щелчок — ${openTip.toLowerCase()}` : 'Щелчок — выбрать', onclick: () => toggle(it), ondblclick: () => onOpen && onOpen(it),
        onkeydown: e => { if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); e.key === 'Enter' && onOpen ? onOpen(it) : toggle(it); } } },
      h('div', { class: 'im' }, thumb(it) ? h('img', { src: thumb(it), alt: '', loading: 'lazy', draggable: false }) : h('span', { class: 'noimg' }, 'нет картинки')),
      h('span', { class: 'ck', html: CHECK }),
      onOpen ? h('button', { class: 'ed', title: openTip, 'aria-label': `${openTip}: «${title(it)}»`, onclick: e => { e.stopPropagation(); onOpen(it); }, html: icon('edit') }) : null,
      h('span', { class: 'cap' }, h('b', { title: title(it) }, title(it)), sub ? h('small', {}, sub(it)) : null)); };
  return [h('div', { class: 'bulk wbar' },
      h('div', { class: 'wsel' }, h('b', {}, picked.size ? `Выбрано: ${picked.size}` : `${items.length} ${plural(items.length, ...word)}`),
        h('button', { class: 'link', onclick: () => { picked.clear(); if (!all) items.forEach(it => picked.add(key(it))); onChange(); } }, all ? 'снять выбор' : 'выбрать все')),
      h('span', { class: 'grow' }), ...top, ...actions.filter(Boolean)),
    note ? h('p', { class: 'hint wnote' }, note) : null,
    h('div', { class: 'wgrid' }, items.map(tile))];
}

export const confirm = (title, text, yes) => dialog(close => h('div', { class: 'dlg' }, h('h2', {}, title), h('p', { style: { margin: 0 } }, text),
  h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', onclick: () => close(false) }, 'Отмена'), h('button', { class: 'btn primary', onclick: () => close(true) }, yes))));

