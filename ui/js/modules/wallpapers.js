// «Обои»: редактор заставок из «Мастерской» (tools/wallpaper.html) под текущую читалку.
// «Сохранить» кладёт обои в каталог «Мои работы» (PB743K3\Wallpapers) вместе с проектом; «На читалку» — сохраняет
// и добавляет файл в общий список «На читалку» (папка обоев на читалке, по умолчанию Wallpapers). Прямой записи на читалку нет.
import { call, pickFolder } from '../api.js';
import { h, fill, toast, icon, plural, size, loading } from '../ui.js';
import { editorModule, tileView, thumbOf, canvasB64, toB64, dataUrl } from './editor.js';
import * as Q from '../queue.js';

const B = () => window.inkmaidenBridge;

// вкладка «Из папки»: картинки из папки обоев. Отмеченные — «На читалку» как есть; подогнать под экран — открыть в редакторе
function folderTab(api) { return (body, D) => {
  let dir = api.C.srcFor('wallpapers');
  const picked = new Set(); let list = [];
  const choose = async () => { const p = await pickFolder('Папка с обоями', dir || undefined); if (!p) return;
    dir = p; const c = api.C.config.paths || {}; await api.C.set('paths', { ...c, src: { ...(c.src || {}), wallpapers: p } }); load(); };
  const toQ = async items => { await Q.addAndGo(api.C, items.map(x => ({ src: x.path, dst: api.cur().readerDirs.wallpapers, kind: 'wallpapers', label: `Обои «${x.name}»` }))); D.close(); };
  const edit = async x => { D.close(); await api.module.drop([x.path]); };
  const where = () => h('p', { class: 'hint wnote' }, 'Папка: ', h('b', {}, dir || 'не указана'));
  const other = () => h('button', { class: 'btn quiet', title: dir, onclick: choose }, h('span', { html: icon('folder') }), 'Другая папка');
  const draw = () => {
    const sel = list.filter(x => picked.has(x.path));
    fill(body, where(), tileView({ items: list, key: x => x.path, picked, onChange: draw, word: ['картинка', 'картинки', 'картинок'],
      thumb: x => x.thumb, title: x => x.name.replace(/\.[^.]+$/, ''), sub: x => `${x.name.split('.').pop().toUpperCase()}, ${size(x.size || 0)}`,
      onOpen: edit, openTip: 'Открыть в редакторе', top: [other()],
      note: 'Отмеченные уходят на читалку как есть. Подогнать под экран — двойной щелчок по картинке.',
      actions: [h('button', { class: 'btn apply', disabled: !sel.length, onclick: () => toQ(sel) }, h('span', { html: icon('apply') }), 'На читалку', sel.length > 1 ? h('i', { class: 'n' }, String(sel.length)) : null)] }));
  };
  const load = async () => {
    if (!dir) { fill(body, h('div', { class: 'drop', style: { padding: '36px' } }, h('h2', {}, 'Папка обоев не указана'), h('p', {}, 'Выберите папку с картинками — она запомнится в «Настройках».'), h('button', { class: 'btn primary', onclick: choose }, 'Выбрать папку'))); return; }
    fill(body, where(), loading('Загрузка', 'читаю картинки'));
    try { list = await call('list_images', { dir }); } catch (e) { fill(body, where(), h('p', { class: 'hint' }, String(e)), h('div', {}, other())); return; }
    if (!list.length) { fill(body, where(), h('p', { class: 'hint' }, 'В этой папке нет картинок (JPG, PNG, BMP, WebP).'), h('div', {}, other())); return; }
    draw();
  };
  load();
}; }

const fileOf = (d, f) => `${d.dirs.wallpapers}${d.dirs.wallpapers.includes('\\') ? '\\' : '/'}${f}`;
const items = (api, works) => { const d = api.cur(); return works.flatMap(w => (w.files || []).map(f => ({ src: fileOf(d, f), dst: d.readerDirs.wallpapers, kind: 'wallpapers', label: `Обои «${w.name}»` }))); };

export default editorModule({
  id: 'wallpapers', title: 'Обои', hint: 'заставки под экран', icon: 'wallpaper', kind: 'wallpapers', what: 'обои',
  desc: 'Картинка на экран читалки: обрежьте, поправьте и добавьте текст.',
  src: 'tools/wallpaper.html',
  applyTitle: 'Сохранить открытые обои и добавить в «На читалку»',
  // редактор отдаёт картинку через мост (onWallSave) — ждём, пока сохранится
  save: api => exportNow(api, false),
  apply: async api => { if (!ready(api)) return true; return exportNow(api, true); },
  queueWork: (api, w) => Q.add(api.C, items(api, [w]), { quiet: true }),
  tabs: api => [{ id: 'folder', label: 'Из папки', render: folderTab(api) }],
  mount(api) {
    B().onWallSave = async (fileName, blob, up) => {
      const w = api.win().__pbw, ext = fileName.split('.').pop();
      const name = api.workName() || fileName.replace(/\.[^.]+$/, '');
      api.setName(name);
      try {
        const r = await call('works_save', { device: api.cur().id, kind: 'wallpapers', name,
          files: [{ name: `${name}.${ext}`, data: await toB64(blob) }],
          project: { snap: JSON.parse(w.snap()), fmt: w.fmt() }, source: canvasB64(w.orig()), thumb: await thumbOf(blob) });
        if (up) await Q.add(api.C, [{ src: fileOf(api.cur(), `${name}.${ext}`), dst: api.cur().readerDirs.wallpapers, kind: 'wallpapers', label: `Обои «${r.name}»` }], { quiet: true });
        else toast(`Сохранено в «Мои работы»: ${r.name}`);
        if (api.waiting) api.waiting(true);
        else if (up) api.C.show('uploads'); // «На читалку» из панели самого редактора
      } catch (e) { toast(String(e.message || e)); if (api.waiting) api.waiting(false); }
    };
  },
  unmount() { B().onWallSave = null; },
  async open(api, work, r) {
    const w = api.win().__pbw;
    const img = await new Promise((res, rej) => { const i = new Image(); i.onload = () => res(i); i.onerror = () => rej(new Error('Исходная картинка не открылась')); i.src = dataUrl(r.source); });
    w.setSource(img);
    const st = r.state || {};
    if (st.fmt) w.st.fmt = st.fmt;
    if (st.snap) w.restore(JSON.stringify(Object.assign({}, st.snap, { dev: api.cur().id })));
    toast(`Открыта работа «${work.name}»`);
  },
  // отмеченные обои в «Моих работах» — ещё и копией в любую папку
  bulk: [
    { label: 'Сохранить в папку…', run: async (api, works) => {
      const d = api.cur();
      const dir = await pickFolder('Куда сохранить выбранные обои', d.dirs.wallpapers); if (!dir) return;
      try { const n = await call('copy_files', { files: works.flatMap(w => (w.files || []).map(f => fileOf(d, f))), dir });
        toast(`Сохранено ${n} ${plural(n, 'файл', 'файла', 'файлов')} в ${dir}`); } catch (e) { toast(String(e)); } } },
  ],
  drop: (win, file) => win.__pbw.loadFile(file),
});

const ready = api => api.win().__pbw && api.win().__pbw.st.src;
/** выгрузить картинку из редактора и дождаться, пока она сохранится (и при up — попадёт в «На читалку») */
function exportNow(api, up) {
  if (!ready(api)) { toast('Сначала откройте картинку'); return false; }
  return new Promise(res => { const t = setTimeout(() => { api.waiting = null; res(false); }, 60000);
    api.waiting = ok => { clearTimeout(t); api.waiting = null; res(ok); };
    api.win().__pbw.exportTo(up); }).then(ok => up ? ok : false);
}
