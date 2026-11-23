// Мост для встроенных редакторов (tools/wallpaper.html, tools/sleep.html): они живут в iframe того же
// происхождения и берут отсюда текущую читалку и её папки. Сохранение в каталог работ делают разделы
// «Обои» и «Иконка сна» (js/modules/editor.js) — редактор только отдаёт готовую картинку и свои настройки.
import { toast } from './ui.js';

const ctx = () => window.inkmaidenCtx;
const cur = () => ctx().cur();

window.inkmaidenBridge = {
  toast,
  theme: () => document.documentElement.dataset.theme,
  // подписки редактора снимаются, когда раздел закрывается (iframe удалён) — см. drop()
  subs: [],
  onDevices(f) { const off = ctx().onDevices(d => { if (cur()) f(d); }); window.inkmaidenBridge.subs.push(off); return off; },
  drop() { window.inkmaidenBridge.subs.splice(0).forEach(off => off()); },
  connected: id => { const d = ctx().device(id); return !!(d && d.connected); },
  /** папка: wallpapers / sleep — на компьютере, readerWallpapers — на читалке */
  folder: kind => { const d = cur(); return kind === 'readerWallpapers' ? d.readerDirs.wallpapers : d.dirs[kind]; },
  /** сохранить обои: раздел «Обои» подставляет сюда свою функцию */
  saveWall: (name, blob, up) => window.inkmaidenBridge.onWallSave ? window.inkmaidenBridge.onWallSave(name, blob, up) : Promise.reject(new Error('Раздел «Обои» не открыт')),

  /* ---- иконка сна: размер экрана текущей читалки ---- */
  sleepModels() { const d = cur(), s = d.reader.screen; return { [d.id]: [s.width, s.height, d.name, Math.round(s.inches * 10) / 10] }; },

  /* ---- обои: текущая читалка для рисунка корпуса и имитации экрана ---- */
  wallDevices() {
    const d = cur(), r = d.reader, b = r.body, carta = !r.screen.color, k = b.width_mm / 108;
    return { [d.id]: { name: d.name, file: (d.name || 'reader').toLowerCase().replace(/[^a-z0-9а-яё]+/gi, '-'), W: r.screen.width, H: r.screen.height,
      ppi: Math.round(Math.hypot(r.screen.width, r.screen.height) / r.screen.inches), bodyW: b.width_mm, bodyH: b.height_mm, depth: 7.8, chamfer: 4.6 * k, round: 1.6,
      scrTop: b.screen_top_mm, mode: carta ? 'carta' : 'kaleido', recess: carta ? .75 : 1.25, metal: !carta,
      keys: [{ x: 13 * k, w: 10 * k }, { x: 28 * k, w: 22 * k }, { x: 58 * k, w: 22 * k }, { x: 85 * k, w: 10 * k }], keyY: b.height_mm - 12.8 * (b.height_mm / 156), keyH: 2.4,
      colors: { body: shades(b.color) }, color: 'body', sim: r.simulation } };
  },
};

// оттенки корпуса для рисунка читалки в обоях
function shades(hex) {
  const n = parseInt((hex || '#2E2E2D').slice(1), 16), c = [n >> 16, (n >> 8) & 255, n & 255];
  const t = (k, d) => '#' + c.map(v => Math.max(0, Math.min(255, Math.round(v * k + d))).toString(16).padStart(2, '0')).join('');
  return { n: 'Корпус', base: t(1, 0), hi: t(1.25, 14), lo: t(.78, 0), key: t(.92, 0) };
}
