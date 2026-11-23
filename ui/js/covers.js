// Обложки для книг без обложки: 13 стилей из «Мастерской» (pb-tools, part2_covers_core.ps1), перенесены на canvas.
// Каждый стиль рисуется в размере экрана читалки; всё меряется в долях ширины (u = W/758 — единица макета Verse).
// Оформление книги: книга > серия > категория > общие настройки (resolve).

export const STYLES = [
  ['Табло', 'Тёмный фон, имя прописными, крупное название'],
  ['Крупно', 'Только название во весь лист — лучше всего видно на 6″'],
  ['Номер', 'Огромный номер книги в серии, удобно искать по полке'],
  ['Буквица', 'Гигантская первая буква названия на фоне'],
  ['Корешок', 'Цветная полоса слева с номером, как корешок'],
  ['Серия', 'Плашка серии сверху и номер в кружке, как у издательств'],
  ['Блок', 'Цветной блок с названием, автор ниже'],
  ['Полосы', 'Три полосы, как у старых карманных изданий'],
  ['Диагональ', 'Диагональный срез цвета'],
  ['Геометрия', 'Узор из фигур, у каждой книги свой'],
  ['Классика', 'Рамка с уголками и название в картуше'],
  ['Светлый', 'Светлый лист, двойная рамка, шрифт с засечками'],
  ['Контур', 'Белый лист, толстая чёрная рамка, максимум контраста'],
];
const NAMES = STYLES.map(s => s[0]);

// для цветного Kaleido — насыщенные средние тона (тёмные там превращаются в почти чёрный), для ч/б — тёмные
const PALETTE_COLOR = ['#1E6FDB', '#2E8B3E', '#CC3328', '#C06A00', '#7B3FB0', '#0F8C8C', '#B0306A', '#6B5B2E'];
const PALETTE_GRAY = ['#085041', '#0C447C', '#3C3489', '#712B13', '#72243E', '#27500A', '#633806', '#26215C', '#444441', '#04342C'];
export const SWATCHES = ['#1E6FDB', '#5B3FB0', '#B0306A', '#CC3328', '#7A1E1E', '#C06A00', '#D08A00', '#2E8B3E', '#0F8C8C', '#6B5B2E', '#3A3A37', '#1F1F1E'];
// на ч/б экране разные цвета сливаются в один серый — каждый цвет получает свой уровень
const GRAY_LEVELS = ['#1C1C1B', '#3E3E3B', '#605F5A', '#83827C', '#C4C2BA', '#E4E2DA'];

export const DEFAULT_CATEGORIES = [
  { name: 'Любовный роман', color: '#B0306A', style: '', rules: 'жанр: love*, *любовн*, romance*' },
  { name: 'Фэнтези', color: '#5B3FB0', style: '', rules: 'жанр: sf_fantasy*, fantasy*, sf_epic, sf_heroic, sf_technofantasy, magician_book, *фэнтези*' },
  { name: 'Ужасы и мистика', color: '#7A1E1E', style: '', rules: 'жанр: sf_horror, horror*, sf_mystic, *ужас*, *мистик*' },
  { name: 'Фантастика', color: '#1E6FDB', style: '', rules: 'жанр: sf, sf_*, popadanec, popadanets, litrpg, *фантастик*, *science fiction*' },
  { name: 'Детектив', color: '#3A3A37', style: '', rules: 'жанр: det*, thriller*, *детектив*, *триллер*' },
  { name: 'Приключения', color: '#C06A00', style: '', rules: 'жанр: adv*, *приключ*' },
  { name: 'Юмор', color: '#2E8B3E', style: '', rules: 'жанр: humor*, *юмор*' },
  { name: 'Классика и проза', color: '#6B5B2E', style: '', rules: 'жанр: prose*, poetry*, *классик*, *поэзи*' },
  { name: 'Нон-фикшн', color: '#0F8C8C', style: '', rules: 'жанр: sci_*, nonf*, ref_*, religion*, foreign_*, comp_*, home*, *справочн*, *словар*, *учебн*' },
  { name: 'Детское', color: '#D08A00', style: '', rules: 'жанр: child*, *детск*, *сказк*' },
];

export function stableHash(s) { let h = 0; for (const c of String(s).toLowerCase()) h = (h * 31 + c.charCodeAt(0)) % 1000003; return h; }
const norm = s => String(s || '').toLowerCase().replace(/ё/g, 'е').replace(/\s+/g, ' ').trim();
const like = (t, pat) => new RegExp('^' + pat.split('*').map(x => x.replace(/[.+?^${}()|[\]\\]/g, '\\$&')).join('.*') + '$').test(t);

/** правила категории: «жанр: a, b*», «папка: X», «автор: X», «серия: X» — по одному виду в строке */
export function matches(cat, b) {
  for (const line of String(cat.rules || '').split(/\r?\n/)) {
    const m = line.match(/^\s*([^:]+):\s*(.+)$/); if (!m) continue;
    const kind = norm(m[1]), vals = m[2].split(',').map(norm).filter(Boolean);
    const targets = kind.startsWith('жанр') ? (b.genres || []).map(norm)
      : kind.startsWith('папк') ? String(b.rel || '').split(/[\\/]/).slice(0, -1).map(norm)
      : kind.startsWith('автор') ? [norm(b.author)] : kind.startsWith('сери') ? [norm(b.series)] : [];
    if (targets.some(t => vals.some(v => like(t, v)))) return true;
  }
  return false;
}
export const seriesKey = b => b.series ? `series|${norm(b.author)}|${norm(b.series)}` : null;
export const bookKey = b => `book|${norm(b.rel)}`;

/** итоговое оформление книги. cfg: { style, bySeries, categories, overrides } */
export function resolve(b, cfg, gray) {
  const look = { style: cfg.style || 'Табло', color: '', category: '', source: 'общие настройки' };
  const cat = (cfg.categories || []).find(c => c.name && matches(c, b));
  if (cat) { look.category = cat.name; look.source = `категория «${cat.name}»`; if (cat.color) look.color = cat.color; if (cat.style) look.style = cat.style; }
  for (const key of [seriesKey(b), bookKey(b)]) {
    const o = key && (cfg.overrides || {})[key]; if (!o) continue;
    if (o.color) look.color = o.color; if (o.style) look.style = o.style;
    look.source = key.startsWith('book|') ? 'выбрано для этой книги' : 'выбрано для серии';
  }
  if (!NAMES.includes(look.style)) look.style = 'Табло';
  if (!look.color) {
    const k = b.series || b.author, pal = gray ? PALETTE_GRAY : PALETTE_COLOR;
    look.color = cfg.bySeries !== false && k ? pal[stableHash(k) % pal.length] : '#2C2C2A';
  }
  if (gray) look.color = grayLevel(look.color);
  return look;
}
function grayLevel(hex) { let i = SWATCHES.indexOf(hex.toUpperCase()); if (i < 0) i = stableHash(hex.toUpperCase()); return GRAY_LEVELS[i % GRAY_LEVELS.length]; }

/* ---------------------------------------------------------------- цвет */
const rgb = hex => { const n = parseInt(hex.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; };
const css = (c, a = 255) => `rgba(${c[0]},${c[1]},${c[2]},${a / 255})`;
const luma = c => 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
const onColor = c => luma(c) < 150 ? [250, 248, 242] : [31, 31, 30];
const mix = (c, k) => { const t = k >= 0 ? 255 : 0; k = Math.abs(k); return c.map(v => Math.round(v + (t - v) * k)); };
// разрядка: буквы через тонкий пробел, слова — через широкий; строка переносится только по обычному пробелу
const spaced = s => String(s || '').toUpperCase().split(/\s+/).filter(Boolean).map(w => [...w].join('\u2009')).join(' \u2002 ');
const coverTitle = t => String(t || '').replace(/\s*[([](СИ|ЛП|si|lp)[)\]]/gi, '').replace(/\s+/g, ' ').trim();

/* ---------------------------------------------------------------- текст */
const SANS = 'Onest, "Segoe UI", Arial, sans-serif', SERIF = 'Georgia, "Times New Roman", serif';
const font = (fam, st, px) => `${st === 'Italic' ? 'italic ' : ''}${st === 'Bold' ? (fam === SANS ? 600 : 700) : 400} ${px}px ${fam}`;

const words = s => s.split(/ +/).filter(Boolean);
function wrap(g, text, w) {
  const lines = []; let line = '';
  for (const word of words(text)) {
    if (word === '\u2002' && !line) continue;
    const t = line ? line + ' ' + word : word;
    if (g.measureText(t).width <= w || !line) line = t; else { lines.push(line); line = word; }
  }
  if (line) lines.push(line);
  return lines.map(l => l.replace(/ \u2002$/, ''));
}
/** вписать текст в прямоугольник: от max к min, без разрыва слов; не влезло и на min — многоточие */
function text(g, s, fam, st, max, min, [x, y, w, h], color, align = 'left', valign = 'top') {
  if (!s) return;
  const ww = w * 0.96, LH = 1.18;
  let px = max, lines;
  for (; px >= min; px -= Math.max(1, max / 40)) {
    g.font = font(fam, st, px);
    if (words(s).some(word => g.measureText(word).width > ww)) continue;
    lines = wrap(g, s, ww);
    if (lines.length * px * LH <= h * 0.95) break;
  }
  if (px < min) { px = min; g.font = font(fam, st, px); lines = wrap(g, s, ww); }
  const fit = Math.max(1, Math.floor(h / (px * LH)));
  if (lines.length > fit) {
    lines = lines.slice(0, fit);
    let last = lines[fit - 1];
    while (last && g.measureText(last + '…').width > ww) last = last.replace(/\s*\S+$/, '') || last.slice(0, -1);
    lines[fit - 1] = last + '…';
  }
  const total = lines.length * px * LH;
  let ty = valign === 'middle' ? y + (h - total) / 2 : valign === 'bottom' ? y + h - total : y;
  g.fillStyle = css(color, color[3] ?? 255);
  g.textBaseline = 'top'; g.textAlign = align === 'center' ? 'center' : align === 'right' ? 'right' : 'left';
  const tx = align === 'center' ? x + w / 2 : align === 'right' ? x + w : x;
  for (const l of lines) { g.fillText(l, tx, ty + px * (LH - 1) / 2); ty += px * LH; }
}

/* ---------------------------------------------------------------- фигуры */
const fillR = (g, c, x, y, w, h) => { g.fillStyle = css(c, c[3] ?? 255); g.fillRect(x, y, w, h); };
const line = (g, c, lw, x1, y1, x2, y2) => { g.strokeStyle = css(c, c[3] ?? 255); g.lineWidth = lw; g.beginPath(); g.moveTo(x1, y1); g.lineTo(x2, y2); g.stroke(); };
// как DrawRectangle в .NET: перо по центру линии
const frame = (g, c, lw, x, y, w, h) => { g.strokeStyle = css(c); g.lineWidth = lw; g.strokeRect(x, y, w, h); };
const poly = (g, c, xy) => { g.fillStyle = css(c); g.beginPath(); g.moveTo(xy[0], xy[1]); for (let i = 2; i < xy.length; i += 2) g.lineTo(xy[i], xy[i + 1]); g.closePath(); g.fill(); };
const disc = (g, c, cx, cy, r) => { g.fillStyle = css(c); g.beginPath(); g.arc(cx, cy, r, 0, Math.PI * 2); g.fill(); };
const rand = seed => () => { seed |= 0; seed = seed + 0x6D2B79F5 | 0; let t = Math.imul(seed ^ seed >>> 15, 1 | seed); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; };

/** нарисовать обложку. b: { title, author, series, number }. Возвращает canvas W×H. */
export function draw(b, style, color, W, H, gray, canvas) {
  const cv = canvas || document.createElement('canvas'); cv.width = W; cv.height = H;
  const g = cv.getContext('2d');
  const u = W / 758, pad = 0.09 * W, iw = W - 2 * pad;
  const title = coverTitle(b.title) || 'Без названия', author = String(b.author || ''), series = String(b.series || ''), num = b.number != null && b.number !== '' ? String(b.number) : '';
  const seriesLine = series && num ? `${series} · ${num}` : series;
  const base = rgb(color), on = onColor(base), onSoft = [...on, 190];
  const ink = [31, 31, 30], inkSoft = [95, 94, 90], paper = [244, 242, 236], cream = [246, 241, 226];
  const clear = c => fillR(g, c, 0, 0, W, H);
  const R = (x, y, w, h) => [x, y, w, h];

  switch (style) {
    case 'Светлый':
      clear(paper);
      frame(g, base, 3 * u, 0.045 * W, 0.045 * W, W - 0.09 * W, H - 0.09 * W);
      frame(g, base, 1 * u, 0.062 * W, 0.062 * W, W - 0.124 * W, H - 0.124 * W);
      text(g, spaced(author), SANS, '', 30 * u, 14 * u, R(pad, 0.11 * H, iw, 0.12 * H), ink, 'center', 'middle');
      line(g, base, 2 * u, W / 2 - 40 * u, 0.27 * H, W / 2 + 40 * u, 0.27 * H);
      text(g, title, SERIF, 'Bold', 86 * u, 28 * u, R(pad, 0.31 * H, iw, 0.42 * H), ink, 'center', 'middle');
      text(g, seriesLine, SERIF, 'Italic', 32 * u, 16 * u, R(pad, 0.77 * H, iw, 0.1 * H), inkSoft, 'center', 'middle');
      break;
    case 'Блок':
      clear(paper); fillR(g, base, 0, 0, W, 0.64 * H);
      text(g, title, SANS, 'Bold', 92 * u, 28 * u, R(pad, 0.1 * H, iw, 0.48 * H), on, 'left', 'bottom');
      text(g, author, SANS, '', 42 * u, 18 * u, R(pad, 0.68 * H, iw, 0.12 * H), ink);
      text(g, seriesLine, SANS, '', 30 * u, 14 * u, R(pad, 0.83 * H, iw, 0.09 * H), inkSoft);
      break;
    case 'Крупно':
      clear(base);
      text(g, title, SANS, 'Bold', 170 * u, 40 * u, R(0.07 * W, 0.07 * H, W - 0.14 * W, 0.7 * H), on);
      text(g, author, SANS, '', 44 * u, 20 * u, R(0.07 * W, 0.82 * H, W - 0.14 * W, 0.11 * H), onSoft, 'left', 'bottom');
      break;
    case 'Номер':
      clear(paper); fillR(g, base, 0, 0, W, 0.5 * H);
      text(g, num || title[0].toUpperCase(), SANS, 'Bold', 420 * u, 120 * u, R(0.04 * W, 0, W - 0.08 * W, 0.5 * H), on, 'center', 'middle');
      text(g, title, SANS, 'Bold', 80 * u, 26 * u, R(pad, 0.55 * H, iw, 0.27 * H), ink);
      text(g, author, SANS, '', 36 * u, 16 * u, R(pad, 0.84 * H, iw, 0.06 * H), ink);
      text(g, series, SANS, '', 28 * u, 14 * u, R(pad, 0.905 * H, iw, 0.05 * H), inkSoft);
      break;
    case 'Буквица': {
      clear(base);
      g.font = font(SERIF, 'Bold', 900 * u); g.fillStyle = css(on, 55); g.textAlign = 'center'; g.textBaseline = 'middle';
      g.fillText(title[0].toUpperCase(), 0.18 * W + W / 2, -0.12 * H + H / 2);
      text(g, spaced(author), SANS, '', 30 * u, 14 * u, R(pad, 0.07 * H, iw, 0.1 * H), onSoft);
      text(g, title, SERIF, 'Bold', 100 * u, 30 * u, R(pad, 0.5 * H, iw, 0.34 * H), on, 'left', 'bottom');
      text(g, seriesLine, SANS, '', 30 * u, 14 * u, R(pad, 0.87 * H, iw, 0.07 * H), onSoft);
      break;
    }
    case 'Корешок': {
      clear(paper);
      const bw = 0.2 * W; fillR(g, base, 0, 0, bw, H);
      if (num) text(g, num, SANS, 'Bold', 110 * u, 40 * u, R(0, 0.05 * H, bw, 0.15 * H), on, 'center', 'middle');
      if (series) { g.save(); g.translate(bw / 2, 0.95 * H); g.rotate(-Math.PI / 2); text(g, spaced(series), SANS, '', 30 * u, 14 * u, R(0, -0.4 * bw, 0.68 * H, 0.8 * bw), on, 'left', 'middle'); g.restore(); }
      const x = bw + 0.08 * W, w2 = W - x - 0.07 * W;
      text(g, author, SANS, '', 38 * u, 16 * u, R(x, 0.08 * H, w2, 0.1 * H), inkSoft);
      line(g, base, 4 * u, x, 0.2 * H, x + 60 * u, 0.2 * H);
      text(g, title, SANS, 'Bold', 96 * u, 28 * u, R(x, 0.25 * H, w2, 0.6 * H), ink);
      break;
    }
    case 'Серия': {
      clear(paper);
      const ph = 0.14 * H; fillR(g, base, 0, 0, W, ph);
      text(g, spaced(series || author), SANS, '', 34 * u, 14 * u, R(pad, 0, iw, ph - 10 * u), on, 'center', 'middle');
      let ay = 0.22 * H;
      if (series && num) {
        const r = 0.075 * W;
        disc(g, paper, W / 2, ph, r + 6 * u); disc(g, base, W / 2, ph, r);
        text(g, num, SANS, 'Bold', 64 * u, 24 * u, R(W / 2 - r, ph - r, 2 * r, 2 * r), on, 'center', 'middle');
        ay = 0.25 * H;
      }
      if (series) text(g, spaced(author), SANS, '', 30 * u, 14 * u, R(pad, ay, iw, 0.07 * H), ink, 'center', 'middle');
      text(g, title, SERIF, 'Bold', 92 * u, 28 * u, R(pad, 0.34 * H, iw, 0.46 * H), ink, 'center', 'middle');
      fillR(g, base, pad, 0.88 * H, iw, 6 * u);
      break;
    }
    case 'Полосы':
      clear(base); fillR(g, cream, 0, 0.3 * H, W, 0.4 * H);
      line(g, on, 3 * u, 0, 0.3 * H - 10 * u, W, 0.3 * H - 10 * u);
      line(g, on, 3 * u, 0, 0.7 * H + 10 * u, W, 0.7 * H + 10 * u);
      text(g, spaced(seriesLine), SANS, '', 32 * u, 14 * u, R(pad, 0.08 * H, iw, 0.14 * H), on, 'center', 'middle');
      text(g, title, SERIF, 'Bold', 84 * u, 26 * u, R(pad, 0.32 * H, iw, 0.27 * H), ink, 'center', 'middle');
      text(g, author, SERIF, 'Italic', 40 * u, 18 * u, R(pad, 0.6 * H, iw, 0.08 * H), inkSoft, 'center', 'middle');
      disc(g, on, W / 2, 0.85 * H, 26 * u); disc(g, base, W / 2, 0.85 * H, 20 * u);
      break;
    case 'Диагональ': {
      clear([35, 35, 34]);
      poly(g, base, [0, 0.62 * H, W, 0.4 * H, W, H, 0, H]);
      const light = [245, 243, 236];
      text(g, spaced(author), SANS, '', 30 * u, 14 * u, R(pad, 0.07 * H, iw, 0.1 * H), [...light, 190]);
      text(g, title, SANS, 'Bold', 96 * u, 28 * u, R(pad, 0.17 * H, iw, 0.3 * H), light);
      text(g, seriesLine, SANS, '', 36 * u, 16 * u, R(pad, 0.84 * H, iw, 0.09 * H), on, 'right', 'bottom');
      break;
    }
    case 'Геометрия': {
      clear(paper);
      const gh = 0.6 * H; fillR(g, mix(base, 0.82), 0, 0, W, gh);
      const rnd = rand(stableHash(title + author)), cols = [base, mix(base, 0.35), mix(base, -0.35), mix(base, 0.6)];
      g.save(); g.beginPath(); g.rect(0, 0, W, gh); g.clip();
      for (let i = 0; i < 9; i++) {
        const c = cols[Math.floor(rnd() * cols.length)], x = rnd() * W, y = rnd() * gh, r = (0.08 + rnd() * 0.22) * W;
        switch (Math.floor(rnd() * 4)) {
          case 0: disc(g, c, x, y, r); break;
          case 1: fillR(g, c, x - r / 2, y - r / 2, r, r); break;
          case 2: poly(g, c, [x, y - r, x + r, y + r * 0.8, x - r, y + r * 0.8]); break;
          default: g.strokeStyle = css(c); g.lineWidth = 18 * u; g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.stroke();
        }
      }
      g.restore();
      text(g, title, SANS, 'Bold', 84 * u, 26 * u, R(pad, 0.64 * H, iw, 0.2 * H), ink);
      text(g, author, SANS, '', 36 * u, 16 * u, R(pad, 0.85 * H, iw, 0.06 * H), ink);
      text(g, seriesLine, SANS, '', 28 * u, 14 * u, R(pad, 0.91 * H, iw, 0.05 * H), inkSoft);
      break;
    }
    case 'Классика': {
      clear(base);
      const gold = luma(base) < 150 ? [222, 200, 140] : ink, m1 = 0.05 * W, m2 = 0.075 * W, d = 16 * u;
      frame(g, gold, 4 * u, m1, m1, W - 2 * m1, H - 2 * m1);
      frame(g, gold, 1.5 * u, m2, m2, W - 2 * m2, H - 2 * m2);
      for (const [px, py] of [[m2, m2], [W - m2, m2], [m2, H - m2], [W - m2, H - m2]]) poly(g, gold, [px, py - d, px + d, py, px, py + d, px - d, py]);
      text(g, spaced(author), SERIF, '', 32 * u, 14 * u, R(pad, 0.12 * H, iw, 0.1 * H), gold, 'center', 'middle');
      const cx = 0.14 * W, cy = 0.3 * H, cw = W - 2 * cx, ch = 0.36 * H;
      g.beginPath(); g.roundRect(cx, cy, cw, ch, 20 * u);
      g.fillStyle = css(cream); g.fill(); g.strokeStyle = css(gold); g.lineWidth = 3 * u; g.stroke();
      text(g, title, SERIF, 'Bold', 80 * u, 24 * u, R(cx + 30 * u, cy + 20 * u, cw - 60 * u, ch - 40 * u), ink, 'center', 'middle');
      text(g, seriesLine, SERIF, 'Italic', 32 * u, 14 * u, R(pad, 0.74 * H, iw, 0.1 * H), gold, 'center', 'middle');
      break;
    }
    case 'Контур':
      clear([255, 255, 255]);
      frame(g, ink, 22 * u, 0.05 * W, 0.05 * W, W - 0.1 * W, H - 0.1 * W);
      fillR(g, base, 0.05 * W + 11 * u, 0.05 * W + 11 * u, 70 * u, 70 * u);
      text(g, spaced(author), SANS, 'Bold', 32 * u, 14 * u, R(0.12 * W, 0.16 * H, 0.76 * W, 0.1 * H), ink, 'left', 'middle');
      text(g, title, SANS, 'Bold', 110 * u, 30 * u, R(0.12 * W, 0.3 * H, 0.76 * W, 0.48 * H), ink);
      text(g, seriesLine, SANS, '', 34 * u, 16 * u, R(0.12 * W, 0.82 * H, 0.76 * W, 0.07 * H), ink);
      break;
    default: // Табло
      clear(base);
      text(g, spaced(author), SANS, '', 32 * u, 14 * u, R(pad, 0.08 * H, iw, 0.12 * H), onSoft);
      line(g, on, 3 * u, pad, 0.23 * H, pad + 70 * u, 0.23 * H);
      text(g, title, SANS, 'Bold', 96 * u, 28 * u, R(pad, 0.29 * H, iw, 0.46 * H), on);
      if (seriesLine) {
        line(g, onSoft, 1.5 * u, pad, 0.81 * H, W - pad, 0.81 * H);
        text(g, seriesLine, SANS, '', 34 * u, 16 * u, R(pad, 0.83 * H, iw, 0.1 * H), onSoft);
      }
  }
  if (gray) {
    const im = g.getImageData(0, 0, W, H), p = im.data;
    for (let i = 0; i < p.length; i += 4) { const y = 0.299 * p[i] + 0.587 * p[i + 1] + 0.114 * p[i + 2]; p[i] = p[i + 1] = p[i + 2] = y; }
    g.putImageData(im, 0, 0);
  }
  return cv;
}

/** обложка книги по настройкам: { canvas, look } */
export function cover(b, cfg, W, H, gray, canvas) {
  const look = resolve(b, cfg, gray);
  return { canvas: draw(b, look.style, look.color, W, H, gray, canvas), look };
}

/** JPEG 85 % в base64 без префикса */
export const jpeg = cv => cv.toDataURL('image/jpeg', 0.85).split(',')[1];

/** шрифт Onest должен загрузиться до рисования, иначе canvas возьмёт запасной */
export const ready = () => Promise.all([document.fonts.load('600 40px Onest', 'Аа'), document.fonts.load('400 40px Onest', 'Аа')]).catch(() => {});
