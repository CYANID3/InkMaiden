// Приветствие Мэйди: один раз при первом запуске (пока не выбрана ни одна папка), потом — из «Настроек → О программе».
import { h, dialog, maidy } from './ui.js';

/** opts.go(id) — открыть раздел и сразу предложить выбрать папку */
export async function welcome(ctx, go) {
  const step = (n, b, t) => h('li', {}, h('i', {}, String(n)), h('span', {}, h('b', {}, b), t));
  const r = await dialog(close => h('div', { class: 'dlg mz-welcome' },
    h('div', {},
      h('h2', {}, 'Привет! Я Мэйди.'),
      h('p', { class: 'lead' }, 'Помогу подготовить книги, мангу и обои для вашей PocketBook — с обложками, понятными названиями и без лишнего веса.'),
      h('ol', { class: 'mz-steps' },
        step(1, 'Выберите папку', ', куда скачиваете книги или мангу.'),
        step(2, '«Сохранить в папку»', ' — я всё подготовлю. Ваши файлы не меняются.'),
        step(3, '«На читалку»', ' — подключите PocketBook кабелем, и я перенесу готовое.')),
      h('div', { class: 'mz-btns' },
        h('button', { class: 'btn primary', onclick: () => close('books') }, 'Выбрать папку с книгами'),
        h('button', { class: 'btn', onclick: () => close('comics') }, 'Папку с мангой и комиксами'),
        h('button', { class: 'btn quiet', onclick: () => close(null) }, 'Позже'))),
    maidy('sit', 'mz-breathe mz-pop')));
  if (!ctx.config.welcomed) await ctx.set('welcomed', true).catch(() => {});
  if (r && go) go(r);
}
