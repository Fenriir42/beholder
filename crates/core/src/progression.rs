use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelRow {
    pub level: u8,
    pub features: Vec<String>,
    pub counts: Vec<(String, u8)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progression {
    pub hit_die: Option<u8>,
    pub rows: Vec<LevelRow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Milestone {
    Subclass,
    Improvement,
    EpicBoon,
}

impl Progression {
    pub fn at(&self, level: u8) -> Option<&LevelRow> {
        self.rows.iter().find(|r| r.level == level)
    }
}

fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.replace('*', "").trim().to_owned())
        .collect()
}

fn leading_number(cell: &str) -> Option<u8> {
    let digits: String = cell.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn hit_die(text: &str) -> Option<u8> {
    let lower = text.to_lowercase();
    let at = lower.find("hit di")?;
    let rest = &lower[at..lower.len().min(at + 40)];
    let d = rest.find('d')? + 1;
    let after_label = rest[d..].find('d').map(|i| d + i + 1)?;
    let digits: String = rest[after_label..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .ok()
        .filter(|n| matches!(n, 4 | 6 | 8 | 10 | 12))
}

pub fn parse_class(text: &str) -> Progression {
    let mut lines = text.lines();
    let mut rows = Vec::new();
    let header = lines.by_ref().map(cells).find(|c| {
        c.first().is_some_and(|h| h.eq_ignore_ascii_case("level"))
            && c.iter().any(|h| h.to_lowercase().contains("feature"))
    });
    if let Some(header) = header {
        let lower: Vec<String> = header.iter().map(|h| h.to_lowercase()).collect();
        let features = lower.iter().position(|h| h.contains("feature"));
        let counted: Vec<usize> = (0..lower.len())
            .filter(|&i| {
                let h = &lower[i];
                h.contains("cantrip") || h.contains("prepared") || h.contains("known")
            })
            .collect();
        for row in lines.map(cells) {
            if row
                .iter()
                .all(|c| c.chars().all(|ch| ch == '-' || ch == ':'))
            {
                continue;
            }
            let Some(level) = row.first().and_then(|c| leading_number(c)) else {
                break;
            };
            let names = features
                .and_then(|i| row.get(i))
                .map(|f| {
                    f.split(',')
                        .map(str::trim)
                        .filter(|n| !n.is_empty() && *n != "—" && *n != "-")
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let counts = counted
                .iter()
                .filter_map(|&i| Some((header[i].clone(), leading_number(row.get(i)?)?)))
                .collect();
            rows.push(LevelRow {
                level,
                features: names,
                counts,
            });
        }
    }
    Progression {
        hit_die: hit_die(text),
        rows,
    }
}

pub fn parse_sections(text: &str) -> Progression {
    let mut rows: Vec<LevelRow> = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix('#') else {
            continue;
        };
        let rest = rest.trim_start_matches('#').trim();
        let Some(rest) = rest
            .strip_prefix("Level ")
            .or_else(|| rest.strip_prefix("level "))
        else {
            continue;
        };
        let Some((n, name)) = rest.split_once(':') else {
            continue;
        };
        let Ok(level) = n.trim().parse::<u8>() else {
            continue;
        };
        let name = name.trim().to_owned();
        match rows.iter_mut().find(|r| r.level == level) {
            Some(r) => r.features.push(name),
            None => rows.push(LevelRow {
                level,
                features: vec![name],
                counts: Vec::new(),
            }),
        }
    }
    rows.sort_by_key(|r| r.level);
    Progression {
        hit_die: None,
        rows,
    }
}

pub fn milestone(feature: &str) -> Option<Milestone> {
    let f = feature.to_lowercase();
    if f.contains("ability score improvement") || f == "improvement" {
        Some(Milestone::Improvement)
    } else if f.contains("epic boon") {
        Some(Milestone::EpicBoon)
    } else if f.contains("subclass") {
        Some(Milestone::Subclass)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIZARD: &str = "**Hit Die** d6  \n**Primary Ability** Intelligence\n\n## Wizard Features\n\n| Level | PB | Features | Cantrips Known | Prepared Spells | 1st | 2nd |\n|---|---|---|---|---|---|---|\n| 1 | +2 | Arcane Recovery, Ritual Adept, Spellcasting | 3 | 4 | 2 | — |\n| 2 | +2 | Scholar | 3 | 5 | 3 | — |\n| 3 | +2 | Wizard Subclass | 3 | 6 | 4 | 2 |\n| 4 | +2 | Ability Score Improvement | 4 | 7 | 4 | 3 |\n| 7 | +3 |  | 4 | 11 | 4 | 3 |\n\n### Level 1: Spellcasting\n\nText.";

    #[test]
    fn class_tables() {
        let p = parse_class(WIZARD);
        assert_eq!(p.hit_die, Some(6));
        assert_eq!(p.rows.len(), 5);
        assert_eq!(
            p.at(1).unwrap().features,
            ["Arcane Recovery", "Ritual Adept", "Spellcasting"]
        );
        assert_eq!(
            p.at(4).unwrap().counts,
            [
                ("Cantrips Known".to_owned(), 4),
                ("Prepared Spells".to_owned(), 7)
            ]
        );
        assert!(p.at(7).unwrap().features.is_empty());
        let bard = "**Hit Die** d8  \n| Level | Features | Cantrips Known | Proficiency Bonus | Spells Known |\n|---|---|---|---|---|\n| 1st | Bardic Inspiration, Spellcasting | 2 | +2 | 4 |\n| 4th | Improvement | 3 | +2 | 7 |";
        let b = parse_class(bard);
        assert_eq!((b.hit_die, b.rows.len()), (Some(8), 2));
        assert_eq!(b.at(4).unwrap().counts[1], ("Spells Known".to_owned(), 7));
        assert_eq!(
            parse_class("**Hit Dice:** 1d10 per fighter level").hit_die,
            Some(10)
        );
    }

    #[test]
    fn subclass_sections_and_milestones() {
        let evoker = "*Wizard subclass*\n\n### Level 3: Evocation Savant\n\nx\n\n### Level 3: Potent Cantrip\n\n### Level 6: Sculpt Spells\n";
        let p = parse_sections(evoker);
        assert_eq!(
            p.at(3).unwrap().features,
            ["Evocation Savant", "Potent Cantrip"]
        );
        assert_eq!(p.at(6).unwrap().features, ["Sculpt Spells"]);
        assert_eq!(milestone("Wizard Subclass"), Some(Milestone::Subclass));
        assert_eq!(
            milestone("Ability Score Improvement"),
            Some(Milestone::Improvement)
        );
        assert_eq!(milestone("Improvement"), Some(Milestone::Improvement));
        assert_eq!(milestone("Epic Boon"), Some(Milestone::EpicBoon));
        assert_eq!(milestone("Scholar"), None);
    }
}
