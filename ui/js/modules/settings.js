// «Настройки»: общие, папки и история, данные и сброс, и по разделу на каждую сохранённую читалку
// (название, папки на компьютере и на читалке, профиль, «Сделать текущей», «Забыть»).
import { HELP, GROUPS } from '../help.js';
import { call, openPath, pickFolder } from '../api.js';
import { h, fill, toast, size, inch, date, icon, dialog, menuButton, plural, maidy } from '../ui.js';
import { confirm } from './editor.js';

let C, root, tab = 'general', off;
const KINDS = [['books', 'Книги', 'Books'], ['comics', 'Комиксы', 'Comics'], ['manga', 'Манга', 'Manga'], ['wallpapers', 'Обои', 'Wallpapers'], ['sleep', 'Иконки сна', 'Sleep_icons'], ['other', 'Другое', 'Other']];
const READER = { books: true, comics: true, manga: true, wallpapers: true, other: true };
const sep = () => (C.info.root.includes('\\') ? '\\' : '/');

export default {
  id: 'settings', title: 'Настройки', icon: 'settings',
  mount(main, ctx) {
    C = ctx; root = h('div', { class: 'page' }); main.append(root);
    off = ctx.onDevices(() => root.isConnected && render());
    render();
  },
  unmount() { off && off(); },
  /** открыть подраздел: general | paths | data | dev:<id> */
  open(t) { tab = t; if (root && root.isConnected) render(); },
};

function render() {
  if (tab.startsWith('dev:') && !C.device(tab.slice(4))) tab = 'general';
  const nav = (id, label, extra) => h('button', { 'aria-current': tab === id, onclick: () => { tab = id; render(); } }, extra || null, label);
  fill(root,
    h('header', { class: 'page-head' }, h('div', {}, h('h1', {}, 'Настройки'), h('p', {}, 'Тема, папки и ваши читалки.'))),
    h('div', { class: 'settings' },
      h('nav', { class: 'snav', 'aria-label': 'Подразделы настроек' },
        nav('general', 'Общие'), nav('paths', 'Папки'), nav('data', 'Сброс'), nav('help', 'Справка'),
        C.dev.devices.length ? h('div', { class: 'sep' }, 'Читалки') : null,
        C.dev.devices.map(d => nav(`dev:${d.id}`, C.dev.devices.filter(o => o.name === d.name).length > 1 ? `${d.name} · ${d.id}` : d.name, d.photo ? h('img', { src: d.photo, alt: '' }) : h('span', { html: icon('readers'), style: { width: '18px' } })))),
      h('div', { class: 'sbody' }, tab === 'paths' ? paths() : tab === 'data' ? data() : tab === 'help' ? help() : tab.startsWith('dev:') ? device(C.device(tab.slice(4))) : general())));
}

/* ---------------------------------------------------------------- общие */
function general() {
  const theme = C.config.theme || (() => { try { return localStorage.getItem('inkmaiden-theme'); } catch (e) { return null; } })() || 'light';
  const toggle = (key, label, hint) => h('label', { class: 'check', style: { alignItems: 'flex-start' } },
    h('input', { type: 'checkbox', checked: C.behavior(key), onchange: async e => { await C.set('behavior', Object.assign({}, C.config.behavior, { [key]: e.target.checked })); } }),
    h('span', {}, label, h('small', { class: 'hint', style: { display: 'block', margin: 0 } }, hint)));
  const cur = C.cur();
  return [
    h('section', { class: 'panel' }, h('h2', {}, 'Текущая читалка'),
      cur ? h('p', { class: 'hint', style: { margin: 0 } }, `Сейчас: ${cur.name}. Программа готовит файлы под неё. Подключите другую — она выберется сама.`)
        : h('p', { class: 'hint', style: { margin: 0 } }, 'Читалок пока нет — подключите PocketBook по USB.')),
    h('section', { class: 'panel' }, h('h2', {}, 'Тема'),
      h('div', { class: 'seg', role: 'group', 'aria-label': 'Тема' },
        [['light', 'Светлая'], ['dark', 'Тёмная'], ['auto', 'Как в Windows']].map(([v, t]) => h('button', { 'aria-pressed': theme === v, onclick: async () => { C.setTheme(v); await C.set('theme', v); render(); } }, t)))),
    h('section', { class: 'panel' }, h('h2', {}, 'Удобства'),
      toggle('autoSave', 'Запоминать новые читалки', 'Подключили новую читалку — программа её запомнит и узнает в следующий раз.'),
      toggle('backup', 'Сохранять прежнюю иконку сна', 'Перед заменой старая иконка копируется на компьютер — её можно вернуть.'),
      toggle('openFolderAfterSave', 'Открывать папку с готовым', 'После сохранения книг и комиксов открывать её в проводнике. Обычно не нужно — готовое и так видно в разделе: «Готовое» в строке сверху.'),
      toggle('diagAutoRemove', 'Убирать проверку с читалки после отчёта', 'Когда отчёт о читалке получен, программа сама уберёт с неё свою проверку.')),
    h('section', { class: 'panel' }, h('div', { class: 'mz-about' }, h('div', {},
      h('div', { class: 'brandline' }, h('img', { src: 'img/icon.png', alt: '' }), h('b', {}, 'InkMaiden')),
      h('p', { class: 'hint', style: { margin: '8px 0 0' } }, `Версия ${C.info.version} · знает ${C.info.models.length} ${plural(C.info.models.length, 'модель', 'модели', 'моделей')} PocketBook`),
      h('p', { style: { margin: '14px 0 0', maxWidth: '52ch' } }, 'Мэйди — помощница InkMaiden. Готовит книги, мангу и обои под вашу читалку и следит, чтобы на неё не попало лишнее.'),
      h('div', { class: 'mz-btns' }, h('button', { class: 'btn', onclick: () => C.welcome && C.welcome() }, 'Показать приветствие'))),
      maidy('sit', 'mz-breathe')),
      C.info.modelErrors && C.info.modelErrors.length ? h('p', { class: 'hint', style: { color: 'var(--err)', margin: 0 } }, C.info.modelErrors.join('; ')) : null,
      h('details', { class: 'fine' }, h('summary', {}, 'Какие модели'),
        h('table', { class: 'models-table' }, h('thead', {}, h('tr', {}, h('th', {}, 'Модель'), h('th', {}, 'Код'), h('th', {}, 'Экран'), h('th', {}, 'Год'))),
          h('tbody', {}, C.info.models.map(m => h('tr', {}, h('td', {}, m.name), h('td', {}, m.code), h('td', {}, `${inch(m.inches)}, ${m.width}×${m.height}${m.color ? ', цвет' : ''}`), h('td', {}, m.year || '')))))))];
}

/* ---------------------------------------------------------------- папки и история */
/**
 * Поле пути: ввести, выбрать папку, взять из «Моих путей» или недавних, сохранить под своим названием, открыть.
 * opts.reader — путь на читалке (без выбора папки на компьютере)
 */
function pathRow(value, placeholder, onSave, opts = {}) {
  const inp = h('input', { class: 'input', value: value || '', placeholder, 'aria-label': opts.label || placeholder, onchange: e => onSave(e.target.value.trim()) });
  if (opts.reader) return h('div', { class: 'prow reader' }, inp);
  const set = p => { inp.value = p; onSave(p); };
  // одна кнопка-меню вместо трёх: выбрать на диске, открыть, мои пути, недавние, сохранить под названием
  const menu = menuButton('', () => {
    const saved = C.savedPaths(), recent = [...new Set(Object.values(C.config.history || {}).flat())].slice(0, 8);
    const short = p => p.split(/[\\/]/).filter(Boolean).pop() || p;
    return [
      { label: 'Выбрать на диске…', onClick: async () => { const p = await pickFolder(opts.label || 'Папка', inp.value || placeholder); if (p) set(p); } },
      { label: 'Открыть в проводнике', onClick: () => openPath(inp.value || placeholder).catch(() => toast('Папки ещё нет — она появится при первом сохранении.')) },
      inp.value ? { label: '★ Сохранить под названием…', onClick: () => saveNamed(inp.value, opts.device) } : null,
      saved.length ? { sep: 'Мои папки' } : null, ...saved.map(x => ({ label: x.name, hint: x.path, title: x.path, onClick: () => set(x.path) })),
      recent.length ? { sep: 'Недавние' } : null, ...recent.map(p => ({ label: short(p), hint: p, title: p, onClick: () => set(p) })),
    ];
  }, { icon: 'folder', title: 'Выбрать, открыть, мои пути', cls: 'icon-only' });
  return h('div', { class: 'prow' }, inp, menu);
}

/** сохранить путь под названием: общий (в Config.json) или для читалки (в её файле) */
async function saveNamed(path, deviceId, oldName) {
  const name = await dialog(close => h('form', { class: 'dlg', onsubmit: e => { e.preventDefault(); close(e.target.n.value.trim()) } },
    h('h2', {}, oldName ? 'Новое название' : 'Сохранить путь'),
    h('p', { class: 'hint', style: { margin: 0 } }, path),
    h('input', { class: 'input', name: 'n', value: oldName || path.split(/[\\/]/).filter(Boolean).pop() || '', placeholder: 'Например: Скачанное', autofocus: true }),
    h('div', { class: 'dlg-foot' }, h('button', { class: 'btn', type: 'button', onclick: () => close(null) }, 'Отмена'), h('button', { class: 'btn primary' }, 'Сохранить'))));
  if (!name) return;
  if (deviceId) {
    const d = C.device(deviceId), list = (d.paths.saved || []).filter(x => x.name !== (oldName || name)).concat([{ name, path }]);
    await call('device_update', { id: d.id, paths: { ...d.paths, saved: list } }); await C.refreshDevices();
  } else {
    const p = C.config.paths || {}, list = (p.saved || []).filter(x => x.name !== (oldName || name)).concat([{ name, path }]);
    await C.set('paths', { ...p, saved: list });
  }
  toast(`Путь «${name}» сохранён`); render();
}
async function dropNamed(name, deviceId) {
  if (deviceId) { const d = C.device(deviceId); await call('device_update', { id: d.id, paths: { ...d.paths, saved: (d.paths.saved || []).filter(x => x.name !== name) } }); await C.refreshDevices(); }
  else { const p = C.config.paths || {}; await C.set('paths', { ...p, saved: (p.saved || []).filter(x => x.name !== name) }); }
  render();
}
/** список своих путей с «Открыть», «Переименовать», «Удалить» и «Добавить» */
function namedList(list, deviceId) {
  return h('div', { class: 'named' },
    list.length ? list.map(x => h('div', { class: 'named-row' }, h('span', { html: icon('folder') }), h('b', {}, x.name), h('small', { title: x.path }, x.path),
      h('button', { class: 'btn quiet', onclick: () => openPath(x.path).catch(() => toast('Папки нет')) }, 'Открыть'),
      h('button', { class: 'btn quiet', onclick: () => saveNamed(x.path, deviceId, x.name) }, 'Переименовать'),
      h('button', { class: 'btn quiet', 'aria-label': `Удалить «${x.name}»`, html: icon('close'), onclick: () => dropNamed(x.name, deviceId) })))
      : h('p', { class: 'hint', style: { margin: 0 } }, 'Пока нет. Сохраните любой путь кнопкой ★ в его поле или добавьте здесь.'),
    h('div', {}, h('button', { class: 'btn', onclick: async () => { const p = await pickFolder('Папка для «Моих путей»'); if (p) saveNamed(p, deviceId); } }, '+ Добавить путь')));
}
const setPath = async (k, v) => { await C.set('paths', Object.assign({}, C.config.paths, { [k]: v })); toast('Сохранено'); };
const setKindPath = async (group, k, v) => { const p = C.config.paths || {}; await C.set('paths', { ...p, [group]: { ...(p[group] || {}), [k]: v } }); await C.refreshDevices(); toast('Сохранено'); };

function paths() {
  const hist = C.config.history || {};
  const kinds = Object.entries(hist).filter(([, l]) => l && l.length);
  const names = { books: 'Книги', comics: 'Комиксы', manga: 'Манга' };
  return [
    h('section', { class: 'panel' }, h('h2', {}, 'Папки для каждого вида файлов'),
      h('p', { class: 'hint', style: { margin: 0 } }, 'Для всех читалок. «Откуда брать» — папка, куда вы скачиваете. «Куда сохранять готовое» — пусто: папка читалки рядом с программой. Для одной читалки можно задать свои — в её подразделе слева.'),
      h('div', { class: 'ptable' }, h('div', { class: 'ph' }, h('span'), h('span', {}, 'Откуда брать'), h('span', {}, 'Куда сохранять готовое')),
        KINDS.map(([k, label, dir]) => h('div', { class: 'pr' }, h('b', {}, label),
          pathRow(((C.config.paths || {}).src || {})[k], C.source() || 'как общая папка', v => setKindPath('src', k, v), { label: `${label}: откуда` }),
          k === 'other' ? h('span', { class: 'hint', style: { margin: 0 } }, 'только «На читалку»') : pathRow(((C.config.paths || {}).out || {})[k], `${C.info.root}${sep()}<читалка>${sep()}${dir}`, v => setKindPath('out', k, v), { label: `${label}: куда` }))),
        h('div', { class: 'pr' }, h('b', {}, 'Приложения'),
          pathRow(((C.config.paths || {}).src || {}).apps, `${C.info.root}${sep()}Apps`, v => setKindPath('src', 'apps', v), { label: 'Приложения: откуда' }),
          h('span', { class: 'hint', style: { margin: 0 } }, 'не нужно — ставятся как есть')))),
    h('section', { class: 'panel' }, h('h2', {}, 'Общая папка для скачанного'),
      h('p', { class: 'hint', style: { margin: 0 } }, 'Если для вида файлов «Откуда» не задано — берётся она. Например, папка, куда вы скачиваете книги и комиксы.'),
      pathRow(C.source(), 'не задана', v => setPath('source', v), { label: 'Общая папка для скачанного' })),
    h('section', { class: 'panel' }, h('h2', {}, 'Мои папки'),
      h('p', { class: 'hint', style: { margin: 0 } }, 'Папки под своими названиями — их можно выбрать в любом поле и в разделах (кнопка «Другая папка»).'),
      namedList((C.config.paths || {}).saved || [])),
    h('section', { class: 'panel' }, h('h2', {}, 'Куда убирать оригиналы при «Изменить на месте»'),
      pathRow((C.config.paths || {}).comicsBackup, 'папка «_backup» рядом с комиксами', v => setPath('comicsBackup', v), { label: 'Куда класть оригиналы' })),
    h('section', { class: 'panel' }, h('h2', {}, 'Недавние папки'),
      kinds.length ? kinds.map(([k, list]) => h('div', {}, h('b', {}, names[k] || k), h('div', { class: 'tree', style: { marginTop: '6px' } }, list.join('\n'))))
        : h('p', { class: 'hint', style: { margin: 0 } }, 'Пока пусто.'),
      kinds.length ? h('div', {}, h('button', { class: 'btn', onclick: async () => { await C.forgetHistory(); render(); toast('История папок очищена'); } }, 'Очистить историю')) : null)];
}

/* ---------------------------------------------------------------- данные и сброс */
// справка: что значит каждая настройка разделов (те же тексты, что у значков «?»), и главное правило записи
function help() {
  return [h('section', { class: 'panel' }, h('h2', {}, 'Как всё устроено'),
      h('p', { class: 'hint', style: { margin: 0 } }, '1. В разделах «Книги», «Комиксы», «Обои», «Иконка сна» вы готовите файлы — они сохраняются на компьютер. 2. Кнопка «На читалку» добавляет их в список. 3. В разделе «На читалку» нажмите «Проверить и записать» — файлы попадут на читалку. 4. Перезагрузите читалку.')),
    ...GROUPS.map(([title, keys]) => h('section', { class: 'panel' }, h('h2', {}, title),
      h('dl', { class: 'help-list' }, keys.flatMap(k => [h('dt', {}, HELP[k][0]), h('dd', {}, HELP[k][1])]))))];
}

function data() {
  const s = sep(), r = C.info.root;
  const open = p => openPath(p).catch(() => toast('Папки ещё нет — она появится при первом сохранении.'));
  const row = (ic, name, note, path, kids, cls = '') => h('div', { class: 'ftree-node ' + cls },
    h('div', { class: 'ftree-row' },
      h('span', { class: 'ftree-ic', html: icon(ic) }),
      h('span', { class: 'ftree-name' }, name),
      h('small', {}, note),
      path ? h('button', { class: 'btn quiet', onclick: () => open(path) }, 'Открыть') : h('span')),
    kids && kids.length ? h('div', { class: 'ftree-kids' }, kids) : null);
  const KIND = { books: 'книги', comics: 'комиксы', manga: 'манга', wallpapers: 'обои', sleep: 'иконки сна' };
  return [
    h('section', { class: 'panel' }, h('h2', {}, 'Где лежат данные'),
      h('p', { class: 'hint', style: { margin: 0 } }, C.info.beside ? 'Всё рядом с программой: перенесите папку — переедут и настройки, и готовые файлы.' : 'Папка программы закрыта для записи, поэтому данные лежат в папке пользователя.'),
      h('div', { class: 'ftree' },
        row('folder', r.split(/[\\/]/).filter(Boolean).pop() || r, r, r, [
          row('settings', 'Config.json', 'общие настройки: тема, папки, история, разделы'),
          row('readers', `Readers${s}`, 'сохранённые читалки — по файлу JSON и фото на каждую', `${r}${s}Readers`,
            C.dev.devices.map(d => row('readers', `${d.id}.json`, d.name + (d.id === C.config.current ? ', текущая' : '')))),
          ...C.dev.devices.map(d => row('folder', `${d.id}${s}`, `готовое для «${d.name}»`, `${r}${s}${d.id}`,
            KINDS.map(([k, , dir]) => { const own = d.paths.pc[k]; return row(k === 'books' ? 'books' : k === 'wallpapers' ? 'wallpaper' : k === 'sleep' ? 'moon' : 'comics',
              own ? own : `${dir}${s}`, own ? `${KIND[k]} — своя папка` : KIND[k], d.dirs[k], null, own ? 'own' : ''); }))),
        ], 'root')),
      h('p', { class: 'hint', style: { margin: 0 } }, 'Ещё программа держит WebView2Loader.dll в %LOCALAPPDATA%\\InkMaiden\\runtime — его можно удалить, он появится снова.')),
    h('section', { class: 'panel' }, h('h2', {}, 'Сброс'),
      h('div', { class: 'grid3' },
        danger('Сбросить настройки', 'Тема, папки, история и настройки разделов вернутся к исходным. Читалки и готовые файлы останутся.', 'Сбросить', async () => {
          await call('config_reset'); clearLocal(); await reload(); toast('Настройки сброшены');
        }),
        danger('Забыть все читалки', 'Удалятся файлы в Readers. Готовые файлы останутся; подключённая читалка сохранится снова.', 'Забыть все', async () => {
          await call('devices_forget_all'); await C.refreshDevices(); toast('Читалки забыты');
        }),
        danger('Удалить все файлы настроек', 'Config.json и папка Readers удаляются целиком — как при первом запуске. Готовые книги, комиксы и работы не трогаются.', 'Удалить', async () => {
          await call('config_delete_all'); clearLocal(); await reload(); toast('Файлы настроек удалены');
        })))];
}
function danger(title, text, yes, act) {
  return h('div', { class: 'spec', style: { display: 'grid', gap: '8px', alignContent: 'space-between' } }, h('b', {}, title), h('small', {}, text),
    h('div', {}, h('button', { class: 'btn danger', onclick: async () => { if (await confirm(`${title}?`, text, yes)) { try { await act(); } catch (e) { toast(String(e)); } } } }, yes)));
}
const clearLocal = () => { try { localStorage.removeItem('inkmaiden-theme'); localStorage.removeItem('inkmaiden-page'); } catch (e) {} };
async function reload() { await C.refresh(); C.setTheme('light'); await C.refreshDevices(); render(); }

/* ---------------------------------------------------------------- читалка */
// та же модель, записанная раньше — в неё и объединять (новая запись PB…-2 появляется после сброса)
function sameReader(d) {
  const box = h('section', { class: 'panel', hidden: true });
  call('device_orphans', { id: d.id }).catch(() => []).then(orph => {
    const tw = twins(d);
    if (!tw.length && !orph.length) return;
    box.hidden = false;
    fill(box, h('h2', {}, 'Это та же читалка?'),
      h('p', { class: 'hint', style: { margin: 0 } }, `После полного сброса или форматирования читалка теряет метку InkMaiden и записывается заново — сейчас как ${d.id}. Если у неё уже было прежнее имя, верните его: готовое из папки ${d.id} перейдёт в прежнюю папку (одноимённые файлы не трогаются), список «На читалку» тоже, заново ничего обрабатывать не нужно.`),
      h('div', { class: 'row-btns' },
        orph.map(o => h('button', { class: 'btn primary', onclick: async () => {
          if (!await confirm(`Вернуть имя ${o.id}?`, `«${d.name}» снова станет ${o.id} и получит папку ${o.id} (${o.files} ${plural(o.files, 'файл', 'файла', 'файлов')} готового). Запись ${d.id} исчезнет.`, 'Вернуть')) return;
          try { const r = await call('device_rename', { id: d.id, to: o.id }); await C.setCurrent(r.id); tab = `dev:${r.id}`; await C.refreshDevices();
            toast(`Готово: это снова ${r.id}${r.moved ? `, перенесено ${r.moved} ${plural(r.moved, 'файл', 'файла', 'файлов')}` : ''}${r.kept ? `, ${r.kept} одноимённых осталось в папке ${d.id}` : ''}`); render(); }
          catch (e) { toast(String(e)); }
        } }, `Вернуть имя ${o.id} (${o.files} ${plural(o.files, 'файл', 'файла', 'файлов')})`)),
        tw.map(o => h('button', { class: 'btn', onclick: async () => {
          if (!await confirm(`Объединить ${d.id} с ${o.id}?`, `${d.name} станет «${o.name}» (${o.id}): готовое из ${d.id} перейдёт в ${o.id}, запись ${d.id} удалится.`, 'Объединить')) return;
          try { const r = await call('device_merge', { from: d.id, into: o.id }); await C.setCurrent(o.id); tab = `dev:${o.id}`; await C.refreshDevices();
            toast(`Объединено: перенесено ${r.moved} ${plural(r.moved, 'файл', 'файла', 'файлов')}${r.kept ? `, ${r.kept} с теми же именами остались в папке ${d.id}` : ''}`); render(); }
          catch (e) { toast(String(e)); }
        } }, `Это «${o.name}» (${o.id})`))));
  });
  return box;
}
const twins = d => C.dev.devices.filter(o => o.id !== d.id && o.seen.code === d.seen.code && ((o.remembered || 0) < (d.remembered || 0) || (o.remembered === d.remembered && o.id < d.id)));
function device(d) {
  const s = sep(), isCur = C.config.current === d.id;
  const save = async patch => { try { await call('device_update', Object.assign({ id: d.id }, patch)); await C.refreshDevices(); toast('Сохранено'); } catch (e) { toast(String(e)); } };
  const setPc = (k, v) => save({ paths: { ...d.paths, pc: { ...d.paths.pc, [k]: v } } });
  const setSrc = (k, v) => save({ paths: { ...d.paths, src: { ...(d.paths.src || {}), [k]: v } } });
  const setRd = (k, v) => save({ paths: { ...d.paths, reader: { ...d.paths.reader, [k]: v.replace(/\\/g, '/') } } });
  const sc = d.reader.screen;
  return [
    h('section', { class: 'panel', style: { display: 'grid', gridTemplateColumns: '64px 1fr auto', gap: '14px', alignItems: 'center' } },
      d.photo ? h('img', { src: d.photo, alt: '', style: { width: '56px', height: '72px', objectFit: 'contain' } }) : h('span', { html: icon('readers') }),
      h('div', { style: { minWidth: 0 } },
        h('input', { class: 'input', value: d.name, 'aria-label': 'Название читалки', style: { fontWeight: 600, fontSize: '17px' }, onchange: e => save({ name: e.target.value }) }),
        h('small', { class: 'hint', style: { display: 'block', margin: '4px 0 0' } }, [d.model ? d.model.name : d.code, `файл Readers${s}${d.id}.json`, d.connected ? `подключена, ${d.connected}` : d.lastSeen ? `была ${date(d.lastSeen)}` : null].filter(Boolean).join(', '))),
      isCur ? h('span', { class: 'chip ok' }, 'текущая') : h('button', { class: 'btn primary', onclick: async () => { await C.setCurrent(d.id); render(); toast(`Текущая читалка — ${d.name}`); } }, 'Сделать текущей')),
    h('section', { class: 'panel' }, h('h2', {}, 'Папки этой читалки'),
      h('p', { class: 'hint', style: { margin: 0 } }, 'Пустое поле — берётся общая папка из «Папки». «На читалке» — папка на самой читалке, куда записываются файлы.'),
      h('div', { class: 'ptable three' }, h('div', { class: 'ph' }, h('span'), h('span', {}, 'Откуда брать'), h('span', {}, 'Куда сохранять готовое'), h('span', {}, 'На читалке')),
        KINDS.map(([k, label]) => h('div', { class: 'pr' }, h('b', {}, label),
          pathRow((d.paths.src || {})[k], ((C.config.paths || {}).src || {})[k] || C.source() || 'как в общих', v => setSrc(k, v), { label: `${label}: откуда`, device: d.id }),
          k === 'other' ? h('span', { class: 'hint', style: { margin: 0 } }, 'только «На читалку»') : pathRow(d.paths.pc[k], d.dirs[k], v => setPc(k, v), { label: `${label}: куда`, device: d.id }),
          READER[k] && k !== 'sleep' ? pathRow(d.paths.reader[k], d.readerDirs[k] || '', v => setRd(k, v), { label: `${label}: на читалке`, reader: true }) : h('span', { class: 'hint', style: { margin: 0 } }, 'system/resources'))),
        // приложения: своя папка или Apps рядом с программой; на читалке — всегда applications (так требует прошивка)
        h('div', { class: 'pr' }, h('b', {}, 'Приложения'),
          pathRow((d.paths.src || {}).apps, ((C.config.paths || {}).src || {}).apps || 'Apps рядом с программой', v => setSrc('apps', v), { label: 'Приложения: откуда', device: d.id }),
          h('span', { class: 'hint', style: { margin: 0 } }, 'не нужно — ставятся как есть'),
          h('span', { class: 'hint', style: { margin: 0 }, title: 'Прошивка ищет приложения только там' }, 'applications')),
        // «Бэкапы»: прежние версии готовых книг и иконки сна — отдельно от готового и ваших файлов
        h('div', { class: 'pr' }, h('b', {}, 'Резервные копии'),
          h('span', { class: 'hint', style: { margin: 0 } }, 'прежние версии готовых книг и иконки сна'),
          h('div', {}, h('button', { class: 'btn quiet', onclick: () => call('backups_list', { device: d.id }).then(r => openPath(r.dir)).catch(e => toast(String(e))) }, h('span', { html: icon('folder') }), 'Открыть «Бэкапы»')),
          h('span', { class: 'hint', style: { margin: 0 } }, 'только на компьютере')))),
    h('section', { class: 'panel' }, h('h2', {}, 'Мои папки этой читалки'), namedList(d.paths.saved || [], d.id)),
    h('section', { class: 'panel' }, h('h2', {}, 'Модель и экран'),
      h('div', { class: 'grid3' },
        h('div', { class: 'spec' }, h('small', {}, 'Экран'), h('b', {}, `${inch(sc.inches)}, ${sc.width}×${sc.height}, ${sc.color ? 'цветной' : `${sc.gray_levels} оттенков серого`}`)),
        h('div', { class: 'spec' }, h('small', {}, 'Память'), h('b', {}, d.seen.total ? `${size(d.seen.free)} свободно из ${size(d.seen.total)}` : 'неизвестно')),
        h('div', { class: 'spec' }, h('small', {}, 'Проверка'), h('b', {}, d.verified ? 'подключалась к этому компьютеру' : 'не подключалась'))),
      h('div', { class: 'field' }, h('span', {}, 'Версия прошивки'),
        h('input', { class: 'input', value: d.firmware || '', placeholder: 'по USB не видно — например, U743K3.6.10.2433', onchange: e => save({ firmware: e.target.value }) }),
        h('small', {}, 'На читалке: Настройки → О устройстве → Программное обеспечение.'))),
    // одна и та же читалка записана дважды или её прежняя папка осталась без записи (после сброса / «Забыть»)
    sameReader(d),
    h('section', { class: 'panel' }, h('h2', {}, 'Забыть читалку'),
      h('p', { class: 'hint', style: { margin: 0 } }, `Удалится файл Readers${s}${d.id}.json. Готовые файлы в папке ${d.id} останутся. Если читалка подключена, она сохранится снова.`),
      h('div', {}, h('button', { class: 'btn danger', onclick: async () => {
        if (!await confirm(`Забыть ${d.name}?`, 'Настройки этой читалки удалятся. Готовые файлы останутся.', 'Забыть')) return;
        try { await call('device_forget', { id: d.id }); tab = 'general'; await C.refreshDevices(); toast(`${d.name} забыта`); } catch (e) { toast(String(e)); }
      } }, 'Забыть'))),
  ];
}
