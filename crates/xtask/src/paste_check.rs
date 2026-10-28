#[cfg(test)]
mod tests {
    use beholder_core::compendium::{Entry, Facets, Kind};
    use beholder_core::paste::parse;

    fn plain_table(block: &str) -> Vec<String> {
        let rows: Vec<Vec<&str>> = block
            .lines()
            .filter(|l| !l.starts_with("|-") && !l.starts_with("| -"))
            .map(|l| l.trim_matches('|').split(" | ").map(str::trim).collect())
            .collect();
        let keyed = rows.first().is_some_and(|r| r.iter().all(|c| c.is_empty()));
        if keyed {
            rows[1..].iter().map(|r| r.join(": ")).collect()
        } else {
            rows.iter().map(|r| r.join("\t")).collect()
        }
    }

    fn plain(e: &Entry) -> String {
        let mut out = vec![e.name.clone()];
        for block in e.text.split("\n\n") {
            if block.starts_with('|') {
                out.extend(plain_table(block));
                continue;
            }
            for line in block.lines() {
                let l = line
                    .trim_start_matches('#')
                    .replace("**", "")
                    .replace('*', "");
                out.extend(l.split(" · ").map(|p| p.trim().to_owned()));
            }
        }
        out.join("\n")
    }

    fn same(e: &Entry, p: &beholder_core::paste::Pasted) -> Result<(), String> {
        if p.name != e.name {
            return Err(format!("name {:?}", p.name));
        }
        match (&e.facets, &p.facets) {
            (
                Facets::Spell {
                    level: a,
                    school: s,
                    ..
                },
                Facets::Spell {
                    level: b,
                    school: t,
                    ..
                },
            ) if a == b && s == t => Ok(()),
            (Facets::Monster { cr: a, size: s, .. }, Facets::Monster { cr: b, size: t, .. })
                if a == b && s.eq_ignore_ascii_case(t) =>
            {
                Ok(())
            }
            (Facets::MagicItem { rarity: a, .. }, Facets::MagicItem { rarity: b, .. })
                if a.eq_ignore_ascii_case(b) =>
            {
                Ok(())
            }
            (Facets::Feat { category: a }, Facets::Feat { category: b }) if a == b => Ok(()),
            (Facets::Subclass { class: a }, Facets::Subclass { class: b }) if a == b => Ok(()),
            _ => Err(format!("{:?} vs {:?}", e.facets, p.facets)),
        }
    }

    #[test]
    fn srd_entries_survive_a_paste_round_trip() {
        let bytes = std::fs::read(crate::root().join("data/srd-5.2/srd.bundle")).unwrap();
        let entries: Vec<Entry> = crate::srd::entries(&bytes)
            .unwrap()
            .into_iter()
            .filter(|e| e.key != "magic-item/mysterious-deck")
            .filter(|e| {
                matches!(
                    e.facets.kind(),
                    Kind::Spell | Kind::Monster | Kind::MagicItem | Kind::Feat | Kind::Subclass
                )
            })
            .collect();
        let mut failures = Vec::new();
        for e in &entries {
            let parsed = parse(&plain(e), Kind::Trait);
            match parsed.as_slice() {
                [p] => {
                    if let Err(why) = same(e, p) {
                        failures.push(format!("{}: {why}", e.key));
                    }
                }
                other => failures.push(format!("{}: {} entries", e.key, other.len())),
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} failed:\n{}",
            failures.len(),
            entries.len(),
            failures.join("\n")
        );
        let all: Vec<String> = entries.iter().map(plain).collect();
        let parsed = parse(&all.join("\n\n"), Kind::Trait);
        let names: Vec<&str> = parsed.iter().map(|p| p.name.as_str()).collect();
        let expected: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, expected);
    }
}
