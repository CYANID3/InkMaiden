//! Правка XML «по месту»: roxmltree даёт расположение узлов в тексте, правки применяются к исходной строке.
//! Так сохраняется всё, чего мы не трогали: оформление, комментарии, порядок атрибутов, картинки.

use roxmltree::{Document, Node, ParsingOptions};

pub fn parse(s: &str) -> Result<Document<'_>, String> {
    Document::parse_with_options(s, ParsingOptions { allow_dtd: true, nodes_limit: u32::MAX }).map_err(|e| format!("XML повреждён: {e}"))
}

pub fn is(n: &Node, name: &str) -> bool { n.is_element() && n.tag_name().name() == name }
pub fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> { n.children().find(|c| is(c, name)) }
pub fn children<'a, 'i>(n: Node<'a, 'i>, name: &'static str) -> impl Iterator<Item = Node<'a, 'i>> { n.children().filter(move |c| is(c, name)) }
pub fn path<'a, 'i>(n: Node<'a, 'i>, names: &[&str]) -> Option<Node<'a, 'i>> {
    let mut cur = n;
    for nm in names { cur = child(cur, nm)?; }
    Some(cur)
}
/// атрибут по локальному имени (href у l:href, xlink:href …)
pub fn attr<'a>(n: &Node<'a, '_>, local: &str) -> Option<&'a str> { n.attributes().find(|a| a.name() == local).map(|a| a.value()) }
/// текст узла: кусочки через пробел (иначе «Глава 2Антон»)
pub fn text(n: &Node) -> String { n.descendants().filter(|d| d.is_text()).filter_map(|d| d.text()).collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>().join(" ") }
pub fn text1(n: Option<Node>) -> String { n.map(|n| text(&n)).unwrap_or_default() }

/// Начало и конец содержимого элемента (между открывающим и закрывающим тегом)
pub fn inner_range(src: &str, n: &Node) -> std::ops::Range<usize> {
    let r = n.range();
    let open_end = src[r.start..r.end].find('>').map(|i| r.start + i + 1).unwrap_or(r.end);
    if src[r.start..open_end].ends_with("/>") { return open_end..open_end }
    let close = src[..r.end].rfind("</").unwrap_or(r.end);
    open_end..close.max(open_end)
}
/// Префикс тега, как он записан в файле: <fb:section> → "fb:"
pub fn prefix_of(src: &str, n: &Node) -> String {
    let r = n.range();
    let tag: String = src[r.start + 1..].chars().take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/').collect();
    tag.find(':').map(|i| tag[..=i].to_string()).unwrap_or_default()
}

/// Набор правок к исходной строке. Вложенные правки (внутри удаляемого куска) пропускаются.
#[derive(Default)]
pub struct Edits { list: Vec<(usize, usize, String)> }

impl Edits {
    pub fn replace(&mut self, r: std::ops::Range<usize>, with: impl Into<String>) { self.list.push((r.start, r.end, with.into())) }
    pub fn delete(&mut self, r: std::ops::Range<usize>) { self.replace(r, "") }
    pub fn insert(&mut self, at: usize, s: impl Into<String>) { self.list.push((at, at, s.into())) }
    pub fn is_empty(&self) -> bool { self.list.is_empty() }
    pub fn apply(mut self, src: &str) -> String {
        // по началу; при равном начале — сначала вставки, потом замены; потом — более широкие
        self.list.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| (a.0 == a.1).cmp(&(b.0 == b.1)).reverse()).then_with(|| (b.1 - b.0).cmp(&(a.1 - a.0))));
        let mut out = String::with_capacity(src.len());
        let mut pos = 0;
        for (s, e, w) in self.list {
            if s < pos { continue } // внутри уже заменённого куска
            out.push_str(&src[pos..s]);
            out.push_str(&w);
            pos = e;
        }
        out.push_str(&src[pos..]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_nested_and_ranges() {
        let src = r##"<r xmlns:l="x"><a l:href="#1">t<b>u</b></a><c/><d>x</d></r>"##;
        let doc = parse(src).unwrap();
        let a = doc.descendants().find(|n| is(n, "a")).unwrap();
        let b = doc.descendants().find(|n| is(n, "b")).unwrap();
        assert_eq!(attr(&a, "href"), Some("#1"));
        assert_eq!(&src[inner_range(src, &a)], "t<b>u</b>");
        let c = doc.descendants().find(|n| is(n, "c")).unwrap();
        assert_eq!(inner_range(src, &c).len(), 0);
        let mut e = Edits::default();
        e.delete(b.range());
        e.replace(a.range(), "T");
        e.insert(c.range().start, "<new/>");
        assert_eq!(e.apply(src), r##"<r xmlns:l="x">T<new/><c/><d>x</d></r>"##);
    }
}
