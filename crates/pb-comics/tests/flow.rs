use pb_comics::process::{estimate_ratio, run, RunMode};
use pb_comics::Plan;
use pb_core::jobs::Cancel;
use std::path::Path;

fn reader(id: &str) -> pb_core::Reader { pb_core::Readers::samples().get(id).unwrap().clone() }

#[test]
fn whole_flow() {
    let src = Path::new("/tmp/polka-t/lib");
    if !src.exists() { eprintln!("нет тестовых архивов"); return }
    let root = std::env::temp_dir().join("polka-flow"); let _ = std::fs::remove_dir_all(&root);
    copy_dir(src, &root.join("lib"));
    let lib = root.join("lib");
    let mut plan = Plan::scan(&lib, true, true, &[]);
    plan.set_names("", true, true);
    for it in &plan.items { println!("{:?} {} → {} ({:?})", it.kind, it.file_name, it.new_name, it.parts) }
    assert_eq!(plan.items.len(), 4);
    let opt = pb_image::page::mode_opt(&reader("color3"), "balanced");
    for it in &plan.items { let r = estimate_ratio(&it.path, &opt, 4).unwrap(); println!("оценка {}: {:.2}", it.file_name, r); assert!(r > 0.0 && r < 1.5) }
    let out = root.join("out"); let bak = root.join("bak");
    let log = |p: pb_comics::process::Progress| if p.status != "…" { println!("[{}/{}] {} — {} ({} → {})", p.index + 1, p.total, p.name, p.status, p.in_bytes, p.out_bytes) };
    let (ok, fail) = run(&plan, RunMode::Copies, Some(&opt), &out, &bak, &Cancel::new(), &log);
    assert_eq!((ok, fail), (4, 0));
    let built = out.join("Color").join(&plan.items.iter().find(|i| i.file_name.ends_with(".cb7")).unwrap().new_name);
    let z = zip::ZipArchive::new(std::fs::File::open(&built).unwrap()).unwrap();
    let names: Vec<_> = z.file_names().map(String::from).collect();
    assert!(names.iter().any(|n| n == "ComicInfo.xml") && names.iter().any(|n| n.starts_with("0001.")));
    // на месте с бэкапом
    let (ok, _) = run(&plan, RunMode::InPlace, Some(&opt), &out, &bak, &Cancel::new(), &log);
    assert_eq!(ok, 4);
    assert_eq!(std::fs::read_dir(bak.join("Naruto")).unwrap().count(), 2);
    // только переименовать (по шаблону)
    let mut p2 = Plan::scan(&lib, true, true, &[]);
    p2.set_names("{series}[ - Том {vol}][ - Глава {ch:3}][ - {title}]", true, false);
    let (ok, _) = run(&p2, RunMode::Rename, None, &out, &bak, &Cancel::new(), &log);
    assert_eq!(ok, 4);
}

fn copy_dir(a: &Path, b: &Path) {
    std::fs::create_dir_all(b).unwrap();
    for e in std::fs::read_dir(a).unwrap().flatten() { let p = e.path(); if p.is_dir() { copy_dir(&p, &b.join(e.file_name())) } else { std::fs::copy(&p, b.join(e.file_name())).unwrap(); } }
}
