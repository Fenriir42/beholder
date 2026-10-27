mod srd;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
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
        ["srd-show", keys @ ..] => {
            let bytes = std::fs::read(root().join("data/srd-5.2/srd.bundle"))?;
            for e in srd::entries(&bytes)?
                .iter()
                .filter(|e| keys.contains(&e.key.as_str()))
            {
                println!("# {} [{}] {:?}\n\n{}\n", e.name, e.key, e.facets, e.text);
            }
            Ok(())
        }
        _ => bail!("usage: cargo xtask srd [--check] | srd-show <key>..."),
    }
}
