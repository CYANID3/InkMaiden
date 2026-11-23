// Правка текста книги «как в текстовом редакторе»: раздел показывается обычной страницей (designMode),
// после правки страница переводится обратно в исходник части — XHTML главы EPUB или XML раздела FB2.
// Картинки и ссылки, которые программа показала по-своему, возвращаются как были (data-pb-src / data-pb-raw / data-pb-open).

const XHTML_NS = ' xmlns="http://www.w3.org/1999/xhtml"';
const escT = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/* ---------------------------------------------------------------- EPUB: тело главы → XHTML */
function xhtmlBody(body) {
  const b = body.cloneNode(true);
  b.querySelectorAll('.pb-note').forEach(n => n.remove());
  b.querySelectorAll('[data-pb-src]').forEach(el => {
    const v = el.getAttribute('data-pb-src');
    if (el.localName === 'img') el.setAttribute('src', v);
    else if (el.hasAttribute('xlink:href') || el.hasAttributeNS('http://www.w3.org/1999/xlink', 'href')) el.setAttributeNS('http://www.w3.org/1999/xlink', 'xlink:href', v);
    else el.setAttribute('href', v);
    el.removeAttribute('data-pb-src');
  });
  b.querySelectorAll('[contenteditable]').forEach(el => el.removeAttribute('contenteditable'));
  const ser = new XMLSerializer();
  return [...b.childNodes].map(n => ser.serializeToString(n)).join('').split(XHTML_NS).join('');
}
/** новое тело — в исходник главы (всё вне <body> остаётся как было) */
export function epubFromPage(doc, source) {
  const inner = xhtmlBody(doc.body);
  const m = source.match(/(<(?:[\w-]+:)?body\b[^>]*>)([\s\S]*)(<\/(?:[\w-]+:)?body\s*>)/i);
  if (!m) throw new Error('В главе не нашлось <body> — правьте код');
  return source.slice(0, m.index) + m[1] + inner + m[3] + source.slice(m.index + m[0].length);
}

/* ---------------------------------------------------------------- FB2: страница → XML раздела */
const INLINE = { em: 'emphasis', i: 'emphasis', strong: 'strong', b: 'strong', s: 'strikethrough', strike: 'strikethrough', del: 'strikethrough', sub: 'sub', sup: 'sup', code: 'code' };
export function fb2FromPage(doc, source) {
  const open = source.match(/^\s*<([\w.-]+:)?([\w.-]+)\b[^>]*>/);
  if (!open) throw new Error('Раздел не начинается с тега — правьте код');
  const P = open[1] || '', tag = n => `${P}${n}`;
  const closeTag = `</${P}${open[2]}>`;
  const inl = node => {
    if (node.nodeType === 3) return escT(node.nodeValue.replace(/ /g, ' '));
    if (node.nodeType !== 1) return '';
    const n = node.localName, kids = () => [...node.childNodes].map(inl).join('');
    if (node.hasAttribute('data-pb-raw')) return node.getAttribute('data-pb-raw');
    if (node.hasAttribute('data-pb-open')) return node.getAttribute('data-pb-open') + kids() + node.getAttribute('data-pb-close');
    if (n === 'br') return ' ';
    if (INLINE[n]) { const k = kids(); return k ? `<${tag(INLINE[n])}>${k}</${tag(INLINE[n])}>` : ''; }
    return kids(); // span, font, a без исходника и прочее — только текст
  };
  const para = (name, node) => { const t = [...node.childNodes].map(inl).join('').replace(/\s+$/, '');
    return t.trim() ? `<${tag(name)}>${t}</${tag(name)}>` : `<${tag('empty-line')}/>`; };
  const BLOCK = new Set(['p', 'div', 'section', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'blockquote', 'table', 'tr', 'td', 'th', 'ul', 'ol', 'li', 'img', 'br']);
  const isBlock = n => n.nodeType === 1 && (BLOCK.has(n.localName) || n.hasAttribute('data-pb-raw'));
  // блоки внутри раздела/эпиграфа/цитаты; подряд идущий «голый» текст — в абзац
  const blocks = (node, ctx) => {
    let out = '', run = [];
    const flush = () => { if (run.length) { const t = run.map(inl).join('').trim(); if (t) out += `<${tag('p')}>${t}</${tag('p')}>`; run = []; } };
    for (const c of node.childNodes) {
      if (!isBlock(c)) { run.push(c); continue; }
      flush(); out += block(c, ctx);
    }
    flush();
    return out;
  };
  const block = (c, ctx) => {
    const n = c.localName, cls = c.className || '';
    if (c.hasAttribute('data-pb-raw')) return c.getAttribute('data-pb-raw');
    if (n === 'br') return `<${tag('empty-line')}/>`;
    if (n === 'img') return '';
    if (n === 'section') return `<${tag('section')}>${blocks(c, 'section')}</${tag('section')}>`;
    if (n === 'h3' || n === 'h1' || n === 'h2') { const ps = [...c.children].some(x => x.localName === 'p') ? blocks(c, 'title') : para('p', c); return `<${tag('title')}>${ps}</${tag('title')}>`; }
    if (n === 'h4' || n === 'h5' || n === 'h6') return para('subtitle', c);
    if (n === 'blockquote') { const t = cls.includes('ep') ? 'epigraph' : 'cite'; return `<${tag(t)}>${blocks(c, t)}</${tag(t)}>`; }
    if (n === 'div' && cls.includes('poem')) return `<${tag('poem')}>${blocks(c, 'poem')}</${tag('poem')}>`;
    if (n === 'div' && cls.includes('stanza')) return `<${tag('stanza')}>${blocks(c, 'stanza')}</${tag('stanza')}>`;
    if (n === 'table') return `<${tag('table')}>${[...c.querySelectorAll('tr')].map(r => `<${tag('tr')}>${[...r.children].map(d => `<${tag(d.localName === 'th' ? 'th' : 'td')}>${[...d.childNodes].map(inl).join('')}</${tag(d.localName === 'th' ? 'th' : 'td')}>`).join('')}</${tag('tr')}>`).join('')}</${tag('table')}>`;
    if (n === 'ul' || n === 'ol') return [...c.children].map(li => para('p', li)).join('');
    if (n === 'p' && cls.includes('ta')) return para('text-author', c);
    if (n === 'p' && (cls.includes('v') || ctx === 'stanza')) return para('v', c);
    // p, div (новая строка в редакторе), li — абзац; если внутри блоки — разобрать их
    if ([...c.childNodes].some(isBlock) && !(c.childNodes.length === 1 && c.firstChild.localName === 'br')) return blocks(c, ctx);
    return para('p', c);
  };
  const body = doc.body.cloneNode(true);
  body.querySelectorAll('.pb-note').forEach(n => n.remove());
  // показ раздела — это его содержимое; если вложено в <section> целиком — снять обёртку не нужно: оно и есть содержимое
  return source.slice(0, open.index + open[0].length) + blocks(body, open[2]) + closeTag;
}

/** включить правку прямо на странице (iframe с allow-same-origin, без скриптов) */
export function makeEditable(frame, onInput) {
  const go = () => {
    const d = frame.contentDocument; if (!d || !d.body) return;
    d.designMode = 'on';
    try { d.execCommand('defaultParagraphSeparator', false, 'p'); } catch (e) {}
    d.body.spellcheck = true;
    d.addEventListener('input', () => onInput && onInput());
    d.body.focus();
  };
  if (frame.contentDocument && frame.contentDocument.readyState === 'complete' && frame.contentDocument.body && frame.contentDocument.body.childNodes.length) go();
  else frame.addEventListener('load', go, { once: true });
}
