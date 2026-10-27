mod open5e;
mod srd;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn open5e_dir() -> PathBuf {
    root().join("data/open5e")
}

fn committed_open5e() -> Result<BTreeMap<String, Vec<u8>>> {
    let mut out = BTreeMap::new();
    let Ok(dir) = std::fs::read_dir(open5e_dir()) else {
        return Ok(out);
    };
    for e in dir {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "bundle")
            && let Some(id) = p.file_stem().and_then(|s| s.to_str())
        {
            out.insert(id.to_owned(), std::fs::read(&p)?);
        }
    }
    Ok(out)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["srd"] => {
            let dir = root().join("data/srd-5.2");
            let bytes = srd::bundle(&dir.join("upstream"))?;
            std::fs::write(dir.join("srd.bundle"), &bytes).context("writing srd.bundle")?;
            println!("wrote data/srd-5.2/srd.bundle ({} bytes)", bytes.len());
            Ok(())
        }
        ["srd", "--check"] => {
            let dir = root().join("data/srd-5.2");
            let bytes = srd::bundle(&dir.join("upstream"))?;
            let committed = std::fs::read(dir.join("srd.bundle")).context("reading srd.bundle")?;
            if bytes != committed {
                bail!("srd.bundle is stale: run `cargo xtask srd`");
            }
            println!("srd.bundle is up to date ({} bytes)", bytes.len());
            Ok(())
        }
        ["open5e"] => {
            let v2 = open5e::checkout(&root())?;
            let built = open5e::bundles(&v2)?;
            std::fs::create_dir_all(open5e_dir())?;
            for id in committed_open5e()?.keys() {
                if !built.iter().any(|(b, _)| b == id) {
                    std::fs::remove_file(open5e_dir().join(format!("{id}.bundle")))?;
                }
            }
            let mut total = 0;
            for (id, bytes) in &built {
                std::fs::write(open5e_dir().join(format!("{id}.bundle")), bytes)?;
                total += bytes.len();
            }
            println!(
                "wrote {} bundles to data/open5e ({total} bytes)",
                built.len()
            );
            Ok(())
        }
        ["open5e", "--check"] => {
            let v2 = open5e::checkout(&root())?;
            let built: BTreeMap<String, Vec<u8>> = open5e::bundles(&v2)?.into_iter().collect();
            if built != committed_open5e()? {
                bail!("data/open5e is stale: run `cargo xtask open5e`");
            }
            println!("data/open5e is up to date ({} bundles)", built.len());
            Ok(())
        }
        ["show", bundle, keys @ ..] => {
            let path = if *bundle == "srd" {
                root().join("data/srd-5.2/srd.bundle")
            } else {
                open5e_dir().join(format!("{bundle}.bundle"))
            };
            let entries =
                srd::entries(&std::fs::read(&path).with_context(|| path.display().to_string())?)?;
            if keys.is_empty() {
                for e in &entries {
                    println!("{}\t{}", e.key, e.name);
                }
            }
            for e in entries.iter().filter(|e| keys.contains(&e.key.as_str())) {
                println!("# {} [{}] {:?}\n\n{}\n", e.name, e.key, e.facets, e.text);
            }
            Ok(())
        }
        _ => bail!(
            "usage: cargo xtask srd [--check] | open5e [--check] | show <srd|bundle-id> [key...]"
        ),
    }
}
