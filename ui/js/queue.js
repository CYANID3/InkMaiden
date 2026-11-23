// «На читалку»: единственное место, откуда что-либо записывается на читалку. Общий для всех разделов список,
// свой у каждой читалки, хранится в Config.json (ключ «queue»; раздел в коде по-прежнему называется uploads).
// Пункт — файл или папка на компьютере и папка на читалке (путь от корня), или действие с читалкой:
//   sleep — поставить иконку сна из «Моих работ», sleep-restore — вернуть иконку из копии или стандартную,
//   diag — поставить модуль диагностики, diag-remove — убрать его,
//   desktop — изменения рабочего стола (plan), desktop-restore — рабочий стол из копии или заводской (stamp).
// Разделы готовят данные и добавляют их сюда одной кнопкой «На читалку»; переносит и применяет — только этот раздел.
import { h, icon, toast } from './ui.js';

export const ACTIONS = new Set(['sleep', 'sleep-restore', 'diag', 'diag-remove', 'desktop', 'desktop-restore']);
const subs = new Set();
const all = ctx => Object.assign({}, ctx.config.queue);
export const list = (ctx, id = (ctx.cur() || {}).id) => (all(ctx)[id] || []);
export const onChange = f => { subs.add(f); return () => subs.delete(f); };
const save = async (ctx, id, items) => { const q = all(ctx); q[id] = items; await ctx.set('queue', q); subs.forEach(f => { try { f(); } catch (e) { console.error(e); } }); };

/** добавить: [{ src, dst, kind, label, … }] — повторы (тот же исходник в ту же папку, то же действие) не дублируются */
export async function add(ctx, items, { quiet = false } = {}) {
  const d = ctx.cur(); if (!d) return 0;
  const cur = list(ctx, d.id);
  const key = x => ACTIONS.has(x.kind) ? `${x.kind}|${x.work || x.stamp || ''}` : `${String(x.src).toLowerCase()}|${String(x.dst).toLowerCase()}`;
  // одно действие одного вида с иконкой сна: новое заменяет прежнее (поставится то, что выбрано последним)
  const one = new Set(items.filter(x => x.kind === 'sleep' || x.kind === 'sleep-restore').length ? ['sleep', 'sleep-restore'] : []);
  const diag = new Set(items.filter(x => x.kind === 'diag' || x.kind === 'diag-remove').length ? ['diag', 'diag-remove'] : []);
  // рабочий стол: последние изменения или возврат заменяют прежние (в плане — все правки сразу)
  const desk = new Set(items.filter(x => x.kind === 'desktop' || x.kind === 'desktop-restore').length ? ['desktop', 'desktop-restore'] : []);
  const kept = cur.filter(x => !one.has(x.kind) && !diag.has(x.kind) && !desk.has(x.kind));
  const have = new Set(kept.map(key));
  const fresh = items.filter(x => (x.src || ACTIONS.has(x.kind)) && !have.has(key(x))).map(x => ({ ...x, id: `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`, added: Date.now() / 1000 | 0 }));
  const changed = fresh.length || kept.length !== cur.length;
  if (changed) await save(ctx, d.id, [...kept, ...fresh]);
  if (!quiet) toast(fresh.length ? `Добавлено в «На читалку»: ${fresh.length}` : 'Это уже есть в «На читалку»');
  return fresh.length;
}
export const remove = (ctx, ids) => save(ctx, ctx.cur().id, list(ctx).filter(x => !ids.includes(x.id)));
export const updateMany = (ctx, ids, patch) => save(ctx, ctx.cur().id, list(ctx).map(x => ids.includes(x.id) ? { ...x, ...patch } : x));
export const clear = ctx => save(ctx, ctx.cur().id, []);

/** добавить и сразу открыть «На читалку» */
export async function addAndGo(ctx, items) { await add(ctx, items, { quiet: true }); ctx.show('uploads'); }

/**
 * Главная кнопка раздела — одна и та же везде, справа в нижней панели: «На читалку».
 * prepare() добавляет готовое этого раздела в список (вернуть false — остаться в разделе); без prepare — просто переход.
 * Число на кнопке — сколько пунктов уже ждёт.
 */
export function applyButton(ctx, prepare, { title } = {}) {
  const n = h('i', { class: 'n' });
  const draw = () => { const k = ctx.cur() ? list(ctx).length : 0; n.textContent = k || ''; n.hidden = !k; };
  const btn = h('button', { class: 'btn apply', title: title || 'Добавить готовое и открыть «На читалку» — запись на читалку только оттуда',
    onclick: async () => { btn.disabled = true; try { if (prepare && (await prepare()) === false) return; ctx.show('uploads'); } catch (e) { toast(String(e.message || e)); } finally { btn.disabled = false; } } },
    h('span', { html: icon('apply') }), 'На читалку', n);
  draw(); const off = onChange(draw); btn._off = off;
  return btn;
}

/** скорость прошлых загрузок (байт/с) — для оценки времени; по умолчанию ~8 МБ/с (USB 2.0 на флеш-память читалки) */
export const speed = ctx => ctx.config.uploadSpeed || 8 * 1024 * 1024;
export async function learnSpeed(ctx, bytes, secs) {
  if (bytes < 2 * 1024 * 1024 || secs < 1) return;
  const v = bytes / secs, old = ctx.config.uploadSpeed;
  await ctx.set('uploadSpeed', old ? Math.round(old * 0.6 + v * 0.4) : Math.round(v));
}
