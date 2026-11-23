// «Что и куда»: где лежат файлы каждого вида (скачанное → готовое → на читалке) и что программа делает с каждым форматом.
// Только показывает: сами настройки — в «Настройках» разделов.
import { h, icon } from './ui.js';
import { drawer } from './modules/editor.js';

const ARROW = () => h('span', { class: 'wr-ar', html: icon('arrow') });
const chips = list => h('span', { class: 'wr-f' }, list.map(x => h('i', {}, x)));

/** focus: 'books' | 'comics' — какой раздел показать первым */
export function whereDrawer(ctx, focus = 'books') {
  const d = ctx.cur(), cfg = ctx.config, b = cfg.books || {}, c = cfg.comics || {};
  const modes = { safe: 'бережно', balanced: 'сбалансированно', max: 'максимум места', custom: 'своя настройка' };
  const srcBooks = b.root || ctx.srcFor('books') || '';
  const srcComics = c.root || ((c.ws || {}).manga || {}).root || ((c.ws || {}).comics || {}).root || ctx.srcFor('comics') || '';
  const cell = (v, title) => h('code', { title: title || v || '' }, v || '—');
  const row = (name, src, ready, reader) => [h('b', {}, name), cell(src), cell(ready), cell(reader)];
  const rule = (from, to, title, note, off) => h('div', { class: 'wr-rule' + (off ? ' off' : '') },
    chips(from), ARROW(),
    h('span', { class: 'wr-d' }, h('b', {}, to ? h('span', { class: 'wr-to' }, to) : null, title), h('small', {}, note)));
  const where = () => h('section', { class: 'wr-sec' },
    h('h3', {}, `Где лежат файлы — ${d.name}`),
    h('div', { class: 'wr-grid' },
      h('span', { class: 'hd' }), h('span', { class: 'hd' }, 'Скачанное'), h('span', { class: 'hd' }, 'Готовое (на компьютере)'), h('span', { class: 'hd' }, 'На читалке'),
      ...row('Книги', srcBooks, d.dirs.books, d.readerDirs.books),
      ...row('Манга', srcComics, d.dirs.manga, d.readerDirs.manga),
      ...row('Комиксы', srcComics, d.dirs.comics, d.readerDirs.comics),
      ...row('Обои', '', d.dirs.wallpapers, d.readerDirs.wallpapers),
      ...row('Иконка сна', '', d.dirs.sleep, 'system/resources')),
    h('p', { class: 'hint' }, 'Папки внутри «Скачанного» сохраняются: Фантастика\\Лем\\Солярис.fb2 ляжет на читалку в Books\\Фантастика\\Лем\\. Скачанное программа не меняет — пишет только в «Готовое», а на читалку — только через «На читалку».'));
  const books = () => h('section', { class: 'wr-sec' },
    h('h3', {}, 'Книги: что во что превращается'),
    h('div', { class: 'wr-rules' },
      rule(['FB2', 'FB2.ZIP'], 'как было', b.covers === false ? 'обложка и название' : 'обложка, название и автор',
        [b.covers === false ? 'Обложки не рисую' : b.replace ? 'Рисую обложку всем книгам' : 'Рисую обложку, если её нет', b.unlink === false ? null : 'ссылки на сайты — обычным текстом'].filter(Boolean).join('; ')),
      rule(['EPUB', 'FB3'], 'как было', 'обложка и название', b.covers === false ? 'Обложки не рисую' : b.replace ? 'Рисую обложку всем книгам' : 'Рисую обложку, если её нет'),
      b.txt === false ? rule(['TXT'], '', 'Пропускаю', 'Выключено «TXT в книги» в настройках раздела', true)
        : rule(['TXT'], 'FB2', 'с обложкой и названием', 'Простой текст читалка показывает хуже — делаю из него книгу'),
      b.niceNames === false ? rule(['Имя файла'], '', 'как скачано', 'Выключено «Понятные имена файлов» в настройках раздела', true)
        : rule(['Имя файла'], 'Автор — Название', 'из данных книги', 'Только новым книгам; сохранённые раньше остаются под своим именем — на читалке не будет двух копий'),
      rule(['PDF', 'DJVU', 'MOBI', 'AZW3', 'DOC', 'RTF'], '', 'Не трогаю', 'В готовое не попадают — такие файлы можно скопировать на читалку самому', true)));
  const comics = () => h('section', { class: 'wr-sec' },
    h('h3', {}, 'Комиксы и манга'),
    h('div', { class: 'wr-rules' },
      rule(['CBZ', 'CBR', 'CB7', 'ZIP', 'RAR', '7Z'], 'CBZ', `сжато под экран — ${modes[c.mode] || modes.balanced}`,
        `Страницы уменьшаю под экран ${d.name}${c.rename === false ? '' : ', названия томов — по порядку'}`),
      rule(['Манга'], d.readerDirs.manga, 'на читалке', 'Серия — манга, если так называется её папка («Манга», «Manga») или вы так отметили'),
      rule(['Комиксы'], d.readerDirs.comics, 'на читалке', 'Серия — комикс, если так называется её папка («Комиксы», «Comics») или вы так отметили'),
      rule(['PDF'], '', 'Не трогаю', 'Не попадают в список', true)));
  const D = drawer('Что и куда', [], null);
  D.body.classList.add('wr');
  D.body.append(where(), ...(focus === 'comics' ? [comics(), books()] : [books(), comics()]),
    h('p', { class: 'hint' }, 'Изменить — «Настройки» в разделе. Правила общие для всех читалок, папки — у каждой свои.'));
  return D;
}
