// «Иконка сна»: редактор из «Мастерской» (tools/sleep.html) под текущую читалку.
// «Сохранить» кладёт работу в каталог (PB743K3\Sleep_icons\<имя>); «На читалку» — сохраняет и добавляет установку
// в раздел «На читалку». Ставится иконка только оттуда; прежняя перед заменой копируется на компьютер (Бэкапы\\Иконка сна).
// Вернуть прежнюю или стандартную — тоже пунктом в «На читалку».
import { call, openPath } from '../api.js';
import { h, fill, toast, dateTime, plural, loading } from '../ui.js';
import * as Q from '../queue.js';
import { editorModule, thumbOf, canvasB64, toB64, dataUrl } from './editor.js';

const sleepItem = (d, name) => ({ src: `${d.dirs.sleep}\\${name}`, dst: 'system/resources', kind: 'sleep', work: name, label: `Иконка сна «${name}»` });

// «2026-11-21_14-05-09» (UTC) → дата по-русски
const when = s => { const m = s.match(/^(\d{4})-(\d\d)-(\d\d)_(\d\d)-(\d\d)-(\d\d)$/); return m ? dateTime(Date.UTC(+m[1], m[2] - 1, +m[3], +m[4], +m[5], +m[6]) / 1000) : s; };

const backups = api => (body, D) => {
  const d = api.cur(), close = D.close;
  const load = async () => {
    let list = [];
    try { list = await call('sleep_backups', { device: d.id }); } catch (e) { fill(body, h('p', { class: 'hint' }, String(e))); return; }
    // возврат — тоже через «На читалку»: там видно, что заменится, и там же он выполняется
    const restore = async (stamp, label) => {
      await Q.addAndGo(api.C, [{ kind: 'sleep-restore', stamp, src: '', dst: 'system/resources', label: stamp === 'default' ? 'Стандартная иконка сна' : `Иконка сна из копии от ${label}` }]);
      close();
    };
    fill(body,
      h('section', { class: 'panel', style: { display: 'grid', gap: '6px' } },
        h('b', {}, d.sleep && d.sleep.installed ? `Сейчас: ${d.sleep.installed}` : 'Сейчас стоит стандартная иконка или поставленная не из InkMaiden'),
        d.sleep && d.sleep.installed_at ? h('small', { class: 'hint', style: { margin: 0 } }, dateTime(d.sleep.installed_at)) : null,
        h('div', {}, h('button', { class: 'btn', onclick: () => restore('default') }, 'Вернуть стандартную'))),
      h('p', { class: 'hint', style: { margin: 0 } }, 'Копии хранятся на компьютере. «Вернуть» добавляет возврат в «На читалку» — там он и выполняется; текущая иконка перед этим тоже уйдёт в копию.'),
      h('h3', { style: { margin: '6px 0 0' } }, 'Резервные копии на компьютере'),
      h('div', {}, h('button', { class: 'btn quiet', onclick: () => call('backups_list', { device: d.id }).then(r => openPath(`${r.dir}\\Иконка сна`)).catch(() => toast('Копий пока нет')) }, 'Открыть папку с копиями')),
      // копии плитками с предпросмотром иконки
      list.length ? h('div', { class: 'bk-grid' }, list.map(b => h('div', { class: 'bk' },
        h('div', { class: 'bk-th' }, b.thumb ? h('img', { src: b.thumb, alt: `Иконка от ${when(b.stamp)}` }) : h('small', { class: 'hint' }, 'нет картинки')),
        h('b', {}, when(b.stamp)), h('small', { class: 'hint', style: { margin: 0 } }, `${b.files.length} ${plural(b.files.length, 'файл', 'файла', 'файлов')}`),
        h('button', { class: 'btn', onclick: () => restore(b.stamp, when(b.stamp)) }, 'Вернуть'))))
        : h('p', { class: 'hint' }, 'Копий пока нет: они появляются, когда иконка заменяется.'));
  };
  fill(body, loading()); load();
};

export default editorModule({
  id: 'sleep', title: 'Иконка сна', hint: 'картинка на экране сна', icon: 'moon', kind: 'sleep', what: 'иконки',
  desc: 'Картинка, которую читалка показывает, когда спит. Прежняя сохранится на компьютере.',
  src: 'tools/sleep.html',
  applyTitle: 'Сохранить иконку и добавить установку в «На читалку»',
  extra: api => {
    const s = api.cur().sleep || {};
    const q = Q.list(api.C).find(x => x.kind === 'sleep' || x.kind === 'sleep-restore');
    return [h('span', { class: 'sleep-state' }, q ? ['Ждёт в «На читалку»: ', h('b', {}, q.label)] : s.installed ? ['Стоит: ', h('b', {}, s.installed)] : 'Стоит стандартная')];
  },
  tabs: api => [{ id: 'backups', label: 'Резервные копии', render: backups(api) }],
  single: true,
  save: api => save(api, false),
  apply: async api => { const w = api.win().__pbs; if (!w || !w.has()) return true; return save(api, true); },
  queueWork: (api, w) => Q.add(api.C, [sleepItem(api.cur(), w.name)], { quiet: true }),
  async open(api, work, r) { api.win().__pbs.open(dataUrl(r.source), work.name, r.state || {}); },
  mount(api) { api.off.push(Q.onChange(() => api.refreshBar())); },
  drop: (win, file) => win.__pbs.loadFile(file),
});

async function save(api, up) {
  const w = api.win().__pbs;
  if (!w || !w.has()) { toast('Сначала откройте картинку'); return false; }
  let name = api.workName();
  if (!name) { name = `Иконка ${new Date().toLocaleDateString('ru-RU')}`; api.setName(name); }
  try {
    const files = w.files();
    await call('works_save', { device: api.cur().id, kind: 'sleep', name,
      files: await Promise.all(files.map(async ([n, b]) => ({ name: n, data: await toB64(b) }))),
      project: await w.state(), source: canvasB64(w.source()), thumb: await thumbOf(files[0][1], 240) });
    if (up) { await Q.add(api.C, [sleepItem(api.cur(), name)], { quiet: true }); return true; }
    toast(`Сохранено в «Мои работы»: ${name}`);
  } catch (e) { toast(String(e.message || e)); }
  return false;
}
