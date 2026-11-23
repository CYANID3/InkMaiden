//! Книги целиком: EPUB и FB2 с рекламой, обложка, чистка, TXT → FB2. Проверяется, что книги остаются корректными.
use pb_books::container::ZipBook;
use pb_books::{junk, meta, process, scan, Job};
use std::io::Write;

const XHTML: &str = r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>t</title></head><body>BODY</body></html>"#;
fn page(body: &str) -> Vec<u8> { XHTML.replace("BODY", body).into_bytes() }

fn epub() -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let st = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let df = zip::write::SimpleFileOptions::default();
    let mut put = |n: &str, d: &[u8], o| { w.start_file(n, o).unwrap(); w.write_all(d).unwrap(); };
    put("mimetype", b"application/epub+zip", st);
    put("META-INF/container.xml", br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#, df);
    put("OEBPS/content.opf", r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">x</dc:identifier><dc:title>Тихий берег</dc:title><dc:creator>Анна Лесная</dc:creator><dc:language>ru</dc:language><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata>
<manifest>
<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
<item id="ad" href="Text/ad.xhtml" media-type="application/xhtml+xml"/>
<item id="banner" href="Images/banner.png" media-type="image/png"/>
<item id="c1" href="Text/c1.xhtml" media-type="application/xhtml+xml"/>
<item id="c2" href="Text/c2.xhtml" media-type="application/xhtml+xml"/>
</manifest>
<spine toc="ncx"><itemref idref="ad"/><itemref idref="c1"/><itemref idref="c2"/></spine>
</package>"#.as_bytes(), df);
    put("OEBPS/nav.xhtml", r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>toc</title></head><body><nav epub:type="toc"><ol><li><a href="Text/ad.xhtml">Реклама</a></li><li><a href="Text/c1.xhtml">Глава 1</a></li><li><a href="Text/c2.xhtml#x">Глава 2</a></li></ol></nav></body></html>"#.as_bytes(), df);
    put("OEBPS/toc.ncx", r#"<?xml version="1.0" encoding="utf-8"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head/><docTitle><text>t</text></docTitle><navMap><navPoint id="a" playOrder="1"><navLabel><text>Реклама</text></navLabel><content src="Text/ad.xhtml"/></navPoint><navPoint id="b" playOrder="2"><navLabel><text>Глава 1</text></navLabel><content src="Text/c1.xhtml"/></navPoint><navPoint id="c" playOrder="3"><navLabel><text>Глава 2</text></navLabel><content src="Text/c2.xhtml"/></navPoint></navMap></ncx>"#.as_bytes(), df);
    put("OEBPS/Text/ad.xhtml", &page(r#"<p>Купите полную версию на <a href="https://www.litres.ru/book">ЛитРес</a>. Скидка по промокоду!</p><img src="../Images/banner.png" alt=""/>"#), df);
    put("OEBPS/Images/banner.png", b"\x89PNG fake", df);
    let long = "Он шёл вдоль берега и думал о своём. ".repeat(40);
    put("OEBPS/Text/c1.xhtml", &page(&format!(r#"<h1>Глава 1</h1><p>{long}</p><p>Подписывайтесь на наш канал в Telegram: <a href="https://t.me/books">t.me/books</a></p><p>Он сказал: «Магазин закрыт».</p>"#)), df);
    put("OEBPS/Text/c2.xhtml", &page(&format!(r#"<h1 id="x">Глава 2</h1><p>{long} Подробнее на <a href="http://example.com">сайте автора</a>.</p>"#)), df);
    w.finish().unwrap().into_inner()
}

fn fb2() -> String {
    let long = "Текст главы, обычный и спокойный. ".repeat(40);
    format!(r##"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
<description><title-info><genre>det_classic</genre><author><first-name>М.</first-name><last-name>Ветров</last-name></author><book-title>Город после полуночи</book-title><annotation><p>Роман о городе.</p><p>Скачать бесплатно и читать онлайн на https://flibusta.example.ru</p></annotation><lang>ru</lang></title-info></description>
<body><section><p>Эта книга подготовлена для вас магазином ЛитРес. Покупайте книги на <a l:href="https://litres.ru">litres.ru</a></p><image l:href="#ad.png"/></section>
<section><title><p>Глава 1</p></title><p>{long}</p><p>Подробности <a l:href="http://example.com">тут</a>.</p></section></body>
<binary id="ad.png" content-type="image/png">QUJD</binary>
</FictionBook>"##)
}

#[test]
fn epub_clean_and_cover() {
    let b = epub();
    let j = junk::find(&b, meta::Format::Epub).unwrap();
    let kinds: Vec<_> = j.findings.iter().map(|f| (f.kind.as_str(), f.sure)).collect();
    assert!(kinds.contains(&("page", true)), "{kinds:?}");
    assert!(kinds.contains(&("para", true)), "{kinds:?}");
    assert!(kinds.iter().any(|k| k.0 == "links"), "{kinds:?}");
    // «Магазин закрыт» — реплика героя, не реклама
    assert!(!j.findings.iter().any(|f| f.preview.contains("Магазин закрыт")));
    let dir = std::env::temp_dir().join(format!("pbbooks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("in/Лесная Анна")).unwrap();
    std::fs::write(dir.join("in/Лесная Анна/Тихий берег.epub"), &b).unwrap();
    std::fs::write(dir.join("in/Город.fb2"), fb2()).unwrap();
    std::fs::write(dir.join("in/Иван Серов - Заметки о море.txt"), encoding_rs::WINDOWS_1251.encode("Глава 1\r\nМоре шумело.\r\n\r\nГлава 2\r\nВетер стих.").0).unwrap();
    std::fs::write(dir.join("in/старое.mobi"), b"x").unwrap();
    let s = scan(&dir.join("in"), true);
    assert_eq!(s.books.len(), 3);
    assert_eq!(s.skipped.get("mobi"), Some(&1));
    let mut jpeg = b"\xFF\xD8\xFF\xE0".to_vec(); jpeg.extend(std::iter::repeat(7u8).take(300)); jpeg.extend(b"\xFF\xD9");
    let out = dir.join("out");
    for bk in &s.books {
        let junk = pb_books::scan::analyze(bk).unwrap();
        let d = process(bk, &out, &Job { cover: Some(jpeg.clone()), junk: junk.findings.clone(), ..Default::default() }).unwrap();
        let data = std::fs::read(&d.path).unwrap();
        match bk.format {
            meta::Format::Epub => {
                let z = ZipBook::read(&data).unwrap();
                assert_eq!(z.entries[0].name, "mimetype");
                let raw = zip::ZipArchive::new(std::io::Cursor::new(&data)).unwrap();
                assert_eq!(raw.name_for_index(0), Some("mimetype"));
                let opf = String::from_utf8(z.get("OEBPS/content.opf").unwrap().to_vec()).unwrap();
                roxmltree::Document::parse(&opf).unwrap();
                assert!(!opf.contains("Text/ad.xhtml") && !opf.contains("idref=\"ad\"") && !opf.contains("banner.png"), "{opf}");
                assert!(opf.contains("cover-image") && opf.contains("name=\"cover\"") && opf.contains("pbcover-page"));
                assert!(z.get("OEBPS/Text/ad.xhtml").is_none() && z.get("OEBPS/Images/banner.png").is_none() && z.get("OEBPS/pbcover.jpg").is_some());
                let nav = String::from_utf8(z.get("OEBPS/nav.xhtml").unwrap().to_vec()).unwrap();
                assert!(!nav.contains("ad.xhtml") && nav.contains("c2.xhtml#x"));
                let ncx = String::from_utf8(z.get("OEBPS/toc.ncx").unwrap().to_vec()).unwrap();
                assert!(!ncx.contains("ad.xhtml") && ncx.contains("playOrder=\"1\"") && ncx.contains("playOrder=\"2\"") && !ncx.contains("playOrder=\"3\""), "{ncx}");
                let c1 = String::from_utf8(z.get("OEBPS/Text/c1.xhtml").unwrap().to_vec()).unwrap();
                assert!(!c1.contains("t.me") && c1.contains("Магазин закрыт"));
                let c2 = String::from_utf8(z.get("OEBPS/Text/c2.xhtml").unwrap().to_vec()).unwrap();
                assert!(c2.contains("сайте автора") && !c2.contains("example.com"));
                let m = meta::epub_meta(&z).unwrap();
                assert!(m.has_cover && m.title == "Тихий берег");
                assert!(d.removed > 0);
            }
            meta::Format::Fb2 => {
                let t = String::from_utf8(data).unwrap();
                roxmltree::Document::parse(&t).unwrap();
                assert!(!t.contains("litres.ru") && !t.contains("flibusta") && !t.contains("id=\"ad.png\""), "{t}");
                assert!(t.contains("Роман о городе") && t.contains("Подробности тут.") && !t.contains("example.com"), "{}", &t[..t.len().min(3000)]);
                let m = meta::fb2_meta(&t).unwrap();
                assert!(m.has_cover, "обложка");
            }
            meta::Format::Txt => {
                assert!(d.rel.ends_with(".fb2"));
                let t = String::from_utf8(data).unwrap();
                let m = meta::fb2_meta(&t).unwrap();
                assert_eq!((m.author.as_str(), m.title.as_str(), m.has_cover), ("Иван Серов", "Заметки о море", true));
                assert!(t.contains("Море шумело") && t.contains("<title><p>Глава 2</p></title>"));
            }
            _ => {}
        }
    }
    // оригиналы не тронуты
    assert_eq!(std::fs::read(dir.join("in/Лесная Анна/Тихий берег.epub")).unwrap(), b);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn edit_parts_meta_cover() {
    use pb_books::edit::{self, MetaEdit, PartEdit};
    use meta::Format;
    // FB2: части, просмотр, проверка правки, данные
    let src = fb2().into_bytes();
    let parts = edit::parts(&src, Format::Fb2).unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[1].title, "Глава 1");
    let v = edit::part(&src, Format::Fb2, "s1").unwrap();
    assert!(v.html.contains("<h3><p>Глава 1</p></h3>") && v.source.starts_with("<section>"));
    let bad = PartEdit { id: "s1".into(), source: "<section><p>оборвано</section>".into(), delete: false };
    assert!(edit::check_part(&src, Format::Fb2, &bad).is_err());
    let two = PartEdit { id: "s1".into(), source: "<section><p>a</p></section><section><p>b</p></section>".into(), delete: false };
    assert!(edit::check_part(&src, Format::Fb2, &two).is_err());
    let good = PartEdit { id: "s1".into(), source: "<section><title><p>Глава первая</p></title><p>Новый текст &amp; правка.</p></section>".into(), delete: false };
    edit::check_part(&src, Format::Fb2, &good).unwrap();
    // чистка убирает раздел 0 (реклама), правка раздела 1 всё равно попадает на место
    let dir = std::env::temp_dir().join(format!("pbedit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("in")).unwrap();
    std::fs::write(dir.join("in/Город.fb2"), &src).unwrap();
    let s = scan(&dir.join("in"), false);
    let bk = &s.books[0];
    let junk = pb_books::scan::analyze(bk).unwrap();
    let links = junk.findings.iter().find(|f| f.kind == "links").unwrap();
    assert!(links.preview.contains("http://example.com"), "{}", links.preview);
    let m = MetaEdit { title: Some("Ночной город".into()), author: Some("Максим Ветров".into()), series: Some("Город".into()), number: Some("2".into()) };
    let d = process(bk, &dir.join("out"), &Job { junk: junk.findings.clone(), meta: m.clone(), parts: vec![good.clone()], ..Default::default() }).unwrap();
    let t = std::fs::read_to_string(&d.path).unwrap();
    roxmltree::Document::parse(&t).unwrap();
    assert!(t.contains("Новый текст &amp; правка.") && !t.contains("litres.ru") && !t.contains("обычный и спокойный"), "{t}");
    let mm = meta::fb2_meta(&t).unwrap();
    assert_eq!((mm.title.as_str(), mm.author.as_str(), mm.series.as_str(), mm.number.as_str()), ("Ночной город", "Максим Ветров", "Город", "2"));
    // EPUB: части по порядку чтения, данные и серия calibre, обложка читается обратно
    let e = epub();
    let ps = edit::parts(&e, Format::Epub).unwrap();
    assert_eq!(ps.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["OEBPS/Text/ad.xhtml", "OEBPS/Text/c1.xhtml", "OEBPS/Text/c2.xhtml"]);
    assert_eq!(ps[1].title, "Глава 1");
    let pv = edit::part(&e, Format::Epub, "OEBPS/Text/ad.xhtml").unwrap();
    assert!(pv.html.contains("data:image/png;base64,"), "{}", pv.html);
    assert!(edit::check_part(&e, Format::Epub, &PartEdit { id: "OEBPS/Text/c2.xhtml".into(), source: "<html><body><p>x</body></html>".into(), delete: false }).is_err());
    let e2 = edit::apply_parts(e.clone(), Format::Epub, &[PartEdit { id: "OEBPS/Text/c2.xhtml".into(), source: XHTML.replace("BODY", "<p>Конец.</p>"), delete: false }], &[]).unwrap();
    let e3 = edit::apply_meta(e2, Format::Epub, &m).unwrap();
    let z = ZipBook::read(&e3).unwrap();
    assert!(String::from_utf8_lossy(z.get("OEBPS/Text/c2.xhtml").unwrap()).contains("Конец."));
    let em = meta::epub_meta(&z).unwrap();
    assert_eq!((em.title.as_str(), em.author.as_str(), em.series.as_str(), em.number.as_str()), ("Ночной город", "Максим Ветров", "Город", "2"));
    assert!(edit::cover(&e3, Format::Epub).is_none());
    let mut c = ZipBook::read(&e3).unwrap();
    pb_books::cover::epub(&mut c, b"\xFF\xD8jpeg").unwrap();
    assert_eq!(edit::cover(&c.write().unwrap(), Format::Epub).unwrap(), b"\xFF\xD8jpeg");
    let f2 = pb_books::cover::fb2(&fb2(), b"\xFF\xD8x").unwrap();
    assert_eq!(edit::cover(f2.as_bytes(), Format::Fb2).unwrap(), b"\xFF\xD8x");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn fb2_without_sections_and_zip() {
    use pb_books::edit;
    use meta::Format;
    let src = r#"<?xml version="1.0" encoding="utf-8"?><FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0"><description><title-info><book-title>Т</book-title></title-info></description><body><p>Просто текст.</p></body></FictionBook>"#;
    let p = edit::parts(src.as_bytes(), Format::Fb2).unwrap();
    assert_eq!(p.len(), 1);
    let v = edit::part(src.as_bytes(), Format::Fb2, "s0").unwrap();
    assert!(v.html.contains("Просто текст") && v.source.starts_with("<body>"));
    let e = edit::PartEdit { id: "s0".into(), source: "<body><p>Новый.</p></body>".into(), delete: false };
    edit::check_part(src.as_bytes(), Format::Fb2, &e).unwrap();
    let out = edit::apply_parts(src.as_bytes().to_vec(), Format::Fb2, &[e], &[]).unwrap();
    assert!(String::from_utf8(out).unwrap().contains("Новый."));
    // FB2.ZIP: части берутся из fb2 внутри архива
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    w.start_file("book.fb2", zip::write::SimpleFileOptions::default()).unwrap();
    w.write_all(fb2().as_bytes()).unwrap();
    let z = w.finish().unwrap().into_inner();
    assert_eq!(edit::parts(&z, Format::Fb2Zip).unwrap().len(), 2);
    assert!(edit::part(&z, Format::Fb2Zip, "s1").unwrap().html.contains("Глава 1"));
}

/// У книги не было обложки, добавили свою и сохранили — готовая книга определяется как книга С обложкой
#[test]
fn added_cover_is_detected() {
    // настоящий маленький JPEG (≈1 КБ), чтобы проверка «картинка не пустая» сработала как в жизни
    let mut jpg = vec![0xFF, 0xD8, 0xFF, 0xE0];
    jpg.extend(std::iter::repeat(0x5Au8).take(1200));
    jpg.extend([0xFF, 0xD9]);
    let dir = std::env::temp_dir().join(format!("pbcov-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("in")).unwrap();
    std::fs::write(dir.join("in/Город.fb2"), fb2()).unwrap();
    std::fs::write(dir.join("in/Берег.epub"), epub()).unwrap();
    let s = scan(&dir.join("in"), false);
    assert_eq!(s.books.len(), 2);
    for bk in &s.books { assert!(!bk.meta.has_cover, "{} уже с обложкой", bk.rel) }
    for bk in &s.books { process(bk, &dir.join("out"), &Job { cover: Some(jpg.clone()), ..Default::default() }).unwrap(); }
    let out = scan(&dir.join("out"), false);
    assert_eq!(out.books.len(), 2);
    for bk in &out.books { assert!(bk.meta.has_cover, "{}: обложка добавлена, но не видна", bk.rel) }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Пересохранение готовой книги: прежняя версия уходит в «Бэкапы», одинаковая книга не переписывается
#[test]
fn resave_keeps_previous_in_backups() {
    let dir = std::env::temp_dir().join(format!("pbbak-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("in/Детектив")).unwrap();
    std::fs::write(dir.join("in/Детектив/Город.fb2"), fb2()).unwrap();
    let s = scan(&dir.join("in"), false);
    let bk = &s.books[0];
    let (out, bak) = (dir.join("out"), dir.join("Бэкапы/Книги"));
    let job = |t: &str| Job { meta: pb_books::edit::MetaEdit { title: Some(t.into()), ..Default::default() }, backup: Some(bak.clone()), ..Default::default() };
    let d1 = process(bk, &out, &job("Первое")).unwrap();
    assert!(!bak.exists(), "в первый раз копировать нечего");
    let v1 = std::fs::read(&d1.path).unwrap();
    // то же самое ещё раз — файл не переписывается, копии нет
    process(bk, &out, &job("Первое")).unwrap();
    assert!(!bak.join(&d1.rel).exists());
    // другое — прежняя версия в «Бэкапах», новая на месте
    let d2 = process(bk, &out, &job("Второе")).unwrap();
    assert_eq!(std::fs::read(bak.join(&d2.rel)).unwrap(), v1);
    assert!(String::from_utf8(std::fs::read(&d2.path).unwrap()).unwrap().contains("Второе"));
    // временных файлов не осталось
    let left: Vec<_> = walkdir(&out).into_iter().filter(|p| p.to_string_lossy().contains("inkmaiden")).collect();
    assert!(left.is_empty(), "{left:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
fn walkdir(p: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v = vec![];
    for e in std::fs::read_dir(p).unwrap().flatten() { let q = e.path(); if q.is_dir() { v.extend(walkdir(&q)) } else { v.push(q) } }
    v
}

/// своё имя готового файла: та же папка, имя — заданное; исходный файл не трогается; плохие имена не принимаются
#[test]
fn out_name_renames_ready_file_only() {
    let dir = std::env::temp_dir().join(format!("pbname-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("in/Фантастика")).unwrap();
    let src = dir.join("in/Фантастика/Strugackiy_Trudno-byt-bogom.434934.fb2");
    std::fs::write(&src, fb2()).unwrap();
    let s = scan(&dir.join("in"), false);
    let bk = &s.books[0];
    let out = dir.join("out");
    let job = Job { out_name: Some("Стругацкие — Трудно быть богом.fb2".into()), ..Default::default() };
    let d = process(bk, &out, &job).unwrap();
    assert_eq!(d.rel.replace('\\', "/"), "Фантастика/Стругацкие — Трудно быть богом.fb2");
    assert!(out.join("Фантастика/Стругацкие — Трудно быть богом.fb2").is_file());
    assert!(!out.join("Фантастика/Strugackiy_Trudno-byt-bogom.434934.fb2").exists());
    assert!(src.is_file(), "исходный файл на месте");
    assert!(pb_books::scan::is_done_as(bk, &out, Some("Стругацкие — Трудно быть богом.fb2")));
    assert!(!pb_books::scan::is_done(bk, &out));
    for bad in ["", "a/b.fb2", "..", "x:y.fb2", "имя.", " x.fb2"] { assert!(!pb_books::scan::safe_name(bad), "{bad}"); }
    // плохое имя — файл ложится под исходным
    let d2 = process(bk, &out, &Job { out_name: Some("a/b.fb2".into()), ..Default::default() }).unwrap();
    assert_eq!(d2.rel.replace('\\', "/"), "Фантастика/Strugackiy_Trudno-byt-bogom.434934.fb2");
    let _ = std::fs::remove_dir_all(&dir);
}
