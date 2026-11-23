// Мелкие помощники интерфейса InkMaiden: создание элементов, размеры по-русски, уведомления, окна.
export const $ = (s, r = document) => r.querySelector(s);
export const $$ = (s, r = document) => [...r.querySelectorAll(s)];

// разметка из строк (значки SVG) разбирается один раз, дальше — копия готового (в длинных списках их сотни)
const TPL = new Map();
function fromHtml(el, v) {
  let t = TPL.get(v);
  if (!t) { t = document.createElement('template'); t.innerHTML = v; if (TPL.size < 400 && v.length < 4000) TPL.set(v, t); }
  el.append(t.content.cloneNode(true));
}
export function h(tag, attrs = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k === 'html') fromHtml(el, v);
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k === 'style' && typeof v === 'object') for (const [sk, sv] of Object.entries(v)) sk.startsWith('--') ? el.style.setProperty(sk, sv) : (el.style[sk] = sv);
    else el.setAttribute(k, v === true ? (k.startsWith('aria-') ? 'true' : '') : v);
  }
  for (const c of kids.flat()) if (c != null && c !== false) el.append(c.nodeType ? c : document.createTextNode(c));
  return el;
}

// заменить содержимое, пропуская пустые части (null/false)
// при перерисовке страницы списки и панели остаются прокрученными там же, где были
const SCROLLERS = '.blist, .ready-list, .inspector, .split > .side, .sbody, .snav, .up-list, .page-body';
export const fill = (el, ...kids) => {
  const page = el.classList && el.classList.contains('page');
  const keep = page ? [...el.querySelectorAll(SCROLLERS)].map(x => [x.className, x.scrollTop]) : null;
  el.replaceChildren(...kids.flat().filter(c => c != null && c !== false));
  if (keep && keep.some(([, t]) => t)) { const now = [...el.querySelectorAll(SCROLLERS)];
    keep.forEach(([c, t], i) => { const x = now[i]; if (t && x && x.className.split(' ')[0] === c.split(' ')[0]) x.scrollTop = t; }); }
};
export const inch = x => `${String(Math.round(x * 10) / 10).replace('.', ',')}″`;

export function size(b) {
  b = Math.max(0, b || 0); if (!b) return '0 Б'; const K = 1024, M = K * K, G = M * K;
  const s = b >= G ? (b / G).toFixed(1) + ' ГБ' : b >= 10 * M ? Math.round(b / M) + ' МБ' : b >= M ? (b / M).toFixed(1) + ' МБ' : Math.max(1, Math.round(b / K)) + ' КБ';
  return s.replace('.', ',');
}
export const plural = (n, a, b, c) => { const m = n % 10, d = n % 100; return m === 1 && d !== 11 ? a : m >= 2 && m <= 4 && (d < 12 || d > 14) ? b : c; };
export const pct = (a, b) => { const p = Math.round(100 * (1 - b / Math.max(1, a))); return p >= 0 ? `−${p} %` : `+${-p} %`; };

let tt = 0;
/* ---------- Мэйди — помощница InkMaiden. Позы: sit, hug, read, ask, sleep, cheer, peek, usb (ui/img/maidy/*.webp).
   Появляется только на пустых и «ждущих» экранах; движение — mz-bob / mz-busy / mz-breathe / mz-pop, выключается
   системной настройкой «уменьшить движение». */
export const maidy = (pose, cls = '', style = null) => h('div', { class: 'maidy ' + cls, style, 'aria-hidden': 'true' },
  h('img', { src: `img/maidy/${pose}.webp`, alt: '', draggable: 'false' }));
/** пустое место с Мэйди: картинка слева, заголовок, пояснение и кнопки справа */
export const mzEmpty = (pose, title, text, buttons = [], cls = '') => h('div', { class: 'mz-empty ' + cls },
  maidy(pose, pose === 'sleep' ? 'mz-breathe' : 'mz-bob'),
  h('div', { class: 'mz-text' }, h('b', {}, title), text ? h('p', {}, text) : null, buttons.filter(Boolean).length ? h('div', { class: 'mz-btns' }, ...buttons.filter(Boolean)) : null));
// настроение короткого сообщения → лицо Мэйди рядом
function mood(text) {
  const t = String(text).toLowerCase();
  if (/ошибк|не удалось|не получилось|не открыл|не найден|нет такой|не читается|неверн|не хватит|error/.test(t)) return 'ask';
  if (/отключ|подождите|позже|остановлено|ничего не|нечего/.test(t)) return 'sleep';
  if (/готово|сохранено|записано|скопировано|возвращен|добавлено|убрано|объединено|узнал|перенесено|^готов/.test(t)) return 'cheer';
  return 'hug';
}
export function toast(text, ms = 4200) {
  const t = $('#toast'); const m = mood(text);
  t.replaceChildren(h('span', { class: 'mz-face', 'aria-hidden': 'true' }, h('img', { src: `img/maidy/${m}.webp`, alt: '' })), h('span', { class: 'mz-msg' }, text));
  t.classList.add('show'); clearTimeout(tt); tt = setTimeout(() => t.classList.remove('show'), ms);
}

export function dialog(build) {
  const d = h('dialog'); document.body.append(d);
  return new Promise(res => {
    const close = v => { d.close(); d.remove(); res(v); };
    d.append(build(close)); d.addEventListener('cancel', e => { e.preventDefault(); close(null); }); d.showModal();
  });
}

export const icon = name => {
  const p = {
    comics: '<rect x="3" y="4" width="8" height="16" rx="1.5"/><rect x="13" y="4" width="8" height="7" rx="1.5"/><rect x="13" y="13" width="8" height="7" rx="1.5"/>',
    books: '<path d="M4 5.5A1.5 1.5 0 0 1 5.5 4H10a2 2 0 0 1 2 2v14a2 2 0 0 0-2-2H4z"/><path d="M20 5.5A1.5 1.5 0 0 0 18.5 4H14a2 2 0 0 0-2 2v14a2 2 0 0 1 2-2h6z"/>',
    readers: '<rect x="6" y="2.5" width="12" height="19" rx="2.5"/><rect x="8.5" y="5.5" width="7" height="10" rx=".8"/><path d="M11 18.5h2"/>',
    folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    eye: '<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    wallpaper: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="9.5" r="1.8"/><path d="M4 18l5.5-5.5 3.5 3.5 2.5-2.5L20 18"/>',
    moon: '<path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/>',
    settings: '<path d="M4 7h10M18 7h2M4 17h4M12 17h8"/><circle cx="16" cy="7" r="2"/><circle cx="10" cy="17" r="2"/>',
    usb: '<path d="M12 3v14M8 7l4-4 4 4M7 13v2l5 3 5-3v-2"/><circle cx="12" cy="20" r="1.5"/>',
    upload: '<path d="M12 3.5v10.5M7.5 9.5L12 14l4.5-4.5"/><path d="M4 13.5v4A2.5 2.5 0 0 0 6.5 20h11a2.5 2.5 0 0 0 2.5-2.5v-4"/>',
    plug: '<rect x="3" y="2.5" width="10" height="14" rx="2"/><path d="M5.5 5.5h5M5.5 8h5M5.5 10.5h3.5" opacity=".55"/><path d="M8 16.5v1.8c0 1.5 1 2.7 2.6 2.7h5c1.6 0 2.4-1 2.4-2.6V14"/><rect x="16" y="9.5" width="4" height="4.5" rx="1"/><path d="M17.3 9.5V7.5h1.4v2"/>',
    refresh: '<path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7"/>',
    edit: '<path d="M4 20h4L19 9l-4-4L4 16z"/><path d="M14 6l4 4"/>',
    image: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="M21 16l-5-5-9 9"/>',
    compare: '<rect x="3" y="4" width="7" height="16" rx="1.5"/><rect x="14" y="4" width="7" height="16" rx="1.5"/>',
    // «На читалку»: стрелка внутрь читалки
    apply: '<rect x="6.5" y="2.5" width="11" height="19" rx="2.5"/><path d="M12 7v7M9 11.5l3 3 3-3"/><path d="M10.5 18.5h3"/>',
    // «Рабочий стол»: сетка значков
    grid: '<rect x="4" y="4" width="6" height="6" rx="1.5"/><rect x="14" y="4" width="6" height="6" rx="1.5"/><rect x="4" y="14" width="6" height="6" rx="1.5"/><rect x="14" y="14" width="6" height="6" rx="1.5"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.5"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
    search: '<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>',
    more: '<circle cx="5.5" cy="12" r="1.3"/><circle cx="12" cy="12" r="1.3"/><circle cx="18.5" cy="12" r="1.3"/>',
    inbox: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M12 10v6M9.5 13.5 12 16l2.5-2.5"/>',
    ready: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="m9 13 2 2 4-4"/>',
    map: '<path d="M9 4 3 6.5v13L9 17l6 2.5 6-2.5v-13L15 6.5 9 4z"/><path d="M9 4v13M15 6.5v13"/>',
    arrow: '<path d="M5 12h13M13 6l6 6-6 6"/>',
    left: '<path d="M15 5l-7 7 7 7"/>', right: '<path d="M9 5l7 7-7 7"/>', close: '<path d="M6 6l12 12M18 6L6 18"/>',
  }[name] || '';
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${p}</svg>`;
};

export const date = sec => sec ? new Date(sec * 1000).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric' }) : '';
export const dateTime = sec => sec ? new Date(sec * 1000).toLocaleString('ru-RU', { day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' }) : '';
/** сколько длится: «меньше минуты», «12 мин», «2 ч 05 мин», «3 дн 4 ч» */
export function span(sec) {
  const s = Math.max(0, Math.floor(sec)); if (s < 60) return 'меньше минуты';
  const m = Math.floor(s / 60) % 60, hh = Math.floor(s / 3600) % 24, d = Math.floor(s / 86400);
  return d ? `${d} дн ${hh} ч` : hh ? `${hh} ч ${String(m).padStart(2, '0')} мин` : `${m} мин`;
}
/** элемент, текст которого сам обновляется раз в 30 с: live(t => `подключена ${span(t)}`, since) */
export function live(fmt, since) {
  const el = h('span', { class: 'live' }); el._fmt = () => fmt(Date.now() / 1000 - since); el.textContent = el._fmt(); return el;
}
setInterval(() => document.querySelectorAll('.live').forEach(el => { if (el._fmt) el.textContent = el._fmt(); }), 30000);

/* ---------------------------------------------------------------- единая индикация долгих операций */

/**
 * Строка прогресса: этап, проценты, полоса. value — 0…1, или < 0 / null — «идёт, сколько осталось неизвестно».
 * progress({ stage: 'Читаю книги', value: .4, detail: '200 из 500' })
 */
export function progress({ stage, value = -1, detail = '', compact = false } = {}) {
  const known = value != null && value >= 0;
  return h('div', { class: 'op' + (compact ? ' compact' : ''), role: 'status', 'aria-live': 'polite' },
    h('div', { class: 'op-line' }, h('span', { class: 'op-spin', 'aria-hidden': 'true' }), h('b', {}, stage || 'Загрузка'),
      detail ? h('small', {}, detail) : null, known ? h('span', { class: 'op-pct' }, `${Math.floor(value * 100)} %`) : null),
    h('div', { class: 'progress', role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': 100, 'aria-valuenow': known ? Math.round(value * 100) : null },
      h('i', { class: known ? '' : 'indet', style: { width: known ? `${Math.max(1, value * 100)}%` : '30%' } })));
}

/** заглушка «Загрузка» там, где результата ещё нет */
export const loading = (text = 'Загрузка', detail, pose) => h('div', { class: 'loading-box' + (pose ? ' with-maidy' : '') }, pose ? maidy(pose, 'mz-breathe') : h('span', { class: 'op-spin', 'aria-hidden': 'true' }), h('b', {}, text), detail ? h('small', {}, detail) : null);

/**
 * Заблокировать часть окна на время операции: размыть, выключить, поверх — что происходит.
 * busy(el, 'Сохраняю книги…') — включить; busy(el, null) — снять.
 */
/**
 * Кнопка с выпадающим меню: вместо ряда редких кнопок — одна.
 * items: [{ label, hint, onClick } | { sep: 'Заголовок' } | элемент] ; opts: { cls, icon, title, align }
 */
export function menuButton(label, items, opts = {}) {
  const menu = h('div', { class: 'hist-menu' + (opts.align === 'left' ? ' left' : '') + (opts.up ? ' up' : ''), hidden: true, role: 'menu' });
  const btn = h('button', { class: 'btn ' + (opts.cls || ''), 'aria-haspopup': 'menu', 'aria-expanded': 'false', title: opts.title || null,
    onclick: e => { e.stopPropagation(); toggle(e.detail === 0); } }, opts.icon ? h('span', { html: icon(opts.icon) }) : null, label, h('span', { class: 'caret', html: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7 10l5 5 5-5"/></svg>' }));
  // меню открывается поверх всего окна (не обрезается краем списка) и само выбирает сторону: вниз или вверх
  const close = () => { menu.hidden = true; btn.setAttribute('aria-expanded', 'false'); menu.remove(); wrap.append(menu);
    removeEventListener('scroll', onScroll, true); removeEventListener('resize', close); };
  const onScroll = e => { if (!menu.contains(e.target)) close(); };
  const place = () => {
    const r = btn.getBoundingClientRect(), vw = innerWidth, vh = innerHeight, gap = 6, pad = 8;
    const w = Math.min(opts.width || (label ? 400 : 340), vw - pad * 2);
    Object.assign(menu.style, { position: 'fixed', width: `${w}px`, right: 'auto', bottom: 'auto', maxHeight: '' });
    const below = vh - r.bottom - gap - pad, above = r.top - gap - pad, hNeed = Math.min(menu.scrollHeight, 420);
    const up = opts.up ? above > 120 || above > below : hNeed > below && above > below;
    const room = Math.max(120, Math.min(420, up ? above : below));
    menu.style.maxHeight = `${room}px`;
    const left = opts.align === 'left' ? r.left : r.right - w;
    menu.style.left = `${Math.max(pad, Math.min(left, vw - w - pad))}px`;
    menu.style.top = up ? `${Math.max(pad, r.top - gap - Math.min(menu.scrollHeight, room))}px` : `${r.bottom + gap}px`;
  };
  const toggle = async kb => {
    if (!menu.hidden) return close();
    // пункты могут собираться не сразу (например, список копий с диска)
    const list = await (typeof items === 'function' ? items() : items);
    fill(menu, list.filter(Boolean).map(it => it instanceof Node ? it : it.sep != null ? h('div', { class: 'sep' }, it.sep)
      : h('button', { role: 'menuitem', title: it.title || null, disabled: !!it.disabled, onclick: () => { close(); it.onClick(); } }, it.label, it.hint ? h('small', {}, it.hint) : null)));
    (btn.closest('dialog') || document.body).append(menu); menu.hidden = false; btn.setAttribute('aria-expanded', 'true'); place();
    // с клавиатуры — фокус на первый пункт; мышью — без рамки
    const f = menu.querySelector('button'); if (f && kb) f.focus({ preventScroll: true });
    setTimeout(() => { addEventListener('click', close, { once: true }); addEventListener('scroll', onScroll, true); addEventListener('resize', close); });
  };
  menu.addEventListener('keydown', e => { if (e.key === 'Escape') { close(); btn.focus(); } });
  const wrap = h('div', { class: 'hist' }, btn, menu);
  return wrap;
}

/** мягкая блокировка: без размытия и подписи — элемент виден (например, ход сохранения), но нажать ничего нельзя */
export function lock(el, on = true) {
  if (!el) return el;
  el.classList.toggle('is-locked', !!on);
  if (el.tagName === 'BUTTON') el.disabled = !!on;
  if (on) el.setAttribute('inert', ''); else el.removeAttribute('inert');
  return el;
}
export function busy(el, text) {
  if (!el) return el;
  el.classList.toggle('is-busy', !!text);
  if (text) el.setAttribute('inert', ''); else el.removeAttribute('inert');
  el.setAttribute('aria-busy', text ? 'true' : 'false');
  if (text) el.dataset.busy = text; else delete el.dataset.busy;
  return el;
}
