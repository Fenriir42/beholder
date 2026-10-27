use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("data/open5e");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "bundle"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = String::from("pub const OPEN5E: &[&[u8]] = &[\n");
    for f in files {
        println!("cargo:rerun-if-changed={}", f.display());
        let _ = writeln!(out, "    include_bytes!({:?}),", f.display().to_string());
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap_or_default()).join("bundles.rs");
    if std::fs::write(&dest, out).is_err() {
        panic!("can't write {}", dest.display());
    }
}
