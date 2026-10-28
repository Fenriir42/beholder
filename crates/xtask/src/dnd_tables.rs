const VOCAB: [&str; 61] = [
    "Cantrips Known",
    "Spells Known",
    "Rage Damage",
    "Martial Arts",
    "Ki Points",
    "Unarmored Movement",
    "Sorcery Points",
    "Sneak Attack",
    "Spell Slots",
    "Slot Level",
    "Invocations Known",
    "Infusions Known",
    "Infused Items",
    "Prepared Spells",
    "Weapon Mastery",
    "Bardic Die",
    "Channel Divinity",
    "Wild Shape",
    "Second Wind",
    "Focus Points",
    "Favored Enemy",
    "Eldritch Invocations",
    "Theories Known",
    "Malediction Metamorphoses",
    "Prime Talents",
    "Talents Known",
    "Maximum Talent Rank",
    "Talent Points",
    "Formulae Known",
    "Reagent Dice",
    "Discoveries Known",
    "Primal Exploits",
    "Words of Power",
    "Vitality Maximum",
    "Focus Maximum",
    "Martial Techniques",
    "Battle Dice",
    "Cohort Feature",
    "Intrigue Die Total",
    "Intrigue Dice",
    "Flourishes Known",
    "Talent Slots",
    "Slot Rank",
    "Complex Patterns",
    "Risk Dice",
    "Techniques Known",
    "Max Spell Level",
    "Spell Uses",
    "Cultivated Potential",
    "Bonus Hit Dice",
    "Moxie Points",
    "Maximum Favor",
    "Tier 1 Invocations",
    "Tier 2 Invocations",
    "Tier 3 Invocations",
    "Merciless Strikes",
    "Ninja Tools",
    "Deft Strike",
    "Cantrip Bonus Dice",
    "Tricks Known",
    "Hexes Known",
];

const SLOT_GROUPS: [&str; 7] = [
    "Spell Level Slots:",
    "Spell Slots per Spell Level:",
    "Spell Slots per Spell Level",
    "Spells Slots Per Level",
    "Tech Slots per Technique Rank",
    "Spell Slots Per Spell Level",
    "Slots per Spell Level",
];

const DICE: [&str; 6] = ["d4", "d6", "d8", "d10", "d12", "d20"];

fn tokens(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            if let Some(b) = start.take() {
                out.push((b, &s[b..i]));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(b) = start {
        out.push((b, &s[b..]));
    }
    out
}

fn end_of(t: (usize, &str)) -> usize {
    t.0 + t.1.len()
}

fn ordinal(k: usize) -> String {
    let suffix = match (k % 10, k % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{k}{suffix}")
}

fn is_cell(t: &str) -> bool {
    if matches!(t, "—" | "-" | "–" | "Unlimited") {
        return true;
    }
    let t = t.strip_suffix(" ft.").unwrap_or(t);
    let t = t.strip_prefix('+').unwrap_or(t);
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    match &t[digits..] {
        "" => digits > 0,
        "st" | "nd" | "rd" | "th" => digits > 0,
        rest => rest
            .strip_prefix(['d', 'D'])
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
    }
}

fn merged(words: &[(usize, &str)]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for (_, w) in words {
        match out.last_mut() {
            Some(last) if *w == "ft." && last.starts_with('+') => {
                last.push_str(" ft.");
            }
            _ => out.push((*w).to_owned()),
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Text(usize, usize),
    Cells(usize),
}

fn shape(words: &[String]) -> Shape {
    let lead = words.iter().take_while(|w| is_cell(w)).count();
    if lead == words.len() {
        return Shape::Cells(lead);
    }
    let trail = words.iter().rev().take_while(|w| is_cell(w)).count();
    Shape::Text(lead, trail)
}

fn fits(s: Shape, (l, t): (usize, usize)) -> bool {
    match s {
        Shape::Text(a, b) => a == l && b == t,
        Shape::Cells(n) => n == l + 1 + t,
    }
}

fn cells(words: &[String], (l, t): (usize, usize)) -> Vec<String> {
    let mut out: Vec<String> = words[..l].to_vec();
    out.push(words[l..words.len() - t].join(" "));
    out.extend_from_slice(&words[words.len() - t..]);
    out
}

fn row_marker(k: usize, plain: bool) -> String {
    if plain { k.to_string() } else { ordinal(k) }
}

fn bonus(k: usize) -> String {
    format!("+{}", 2 + (k - 1) / 4)
}

struct Grid {
    starts: Vec<usize>,
    shape: (usize, usize),
}

fn find_rows(
    toks: &[(usize, &str)],
    plain: bool,
    starts: &mut Vec<usize>,
    fixed: Option<(usize, usize)>,
    allow_short: bool,
) -> Option<Grid> {
    let k = starts.len() + 1;
    let prev = *starts.last()?;
    if k == 21 {
        return Some(Grid {
            starts: starts.clone(),
            shape: fixed?,
        });
    }
    let (marker, pb) = (row_marker(k, plain), bonus(k));
    let window = (prev + 2..toks.len().saturating_sub(1)).take(80);
    for i in window {
        if toks[i].1 != marker || toks[i + 1].1 != pb {
            continue;
        }
        let words = merged(&toks[prev + 2..i]);
        if words.is_empty() {
            continue;
        }
        let s = shape(&words);
        let next = match (fixed, s) {
            (Some(sh), _) if !fits(s, sh) => continue,
            (Some(sh), _) => Some(sh),
            (None, Shape::Text(a, b)) => Some((a, b)),
            (None, Shape::Cells(_)) => None,
        };
        starts.push(i);
        if let Some(g) = find_rows(toks, plain, starts, next, allow_short) {
            return Some(g);
        }
        starts.pop();
    }
    match fixed {
        Some(shape) if allow_short && k > 4 => Some(Grid {
            starts: starts.clone(),
            shape,
        }),
        _ => None,
    }
}

fn merged_idx(toks: &[(usize, &str)]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for (i, (_, w)) in toks.iter().enumerate() {
        match out.last_mut() {
            Some(last) if *w == "ft." && last.0.starts_with('+') => {
                last.0.push_str(" ft.");
                last.1 = i + 1;
            }
            _ => out.push(((*w).to_owned(), i + 1)),
        }
    }
    out
}

fn ends_table(words: &[(String, usize)], j: usize, first_feature: &[&str]) -> bool {
    let next = words.get(j + 1).map(|w| w.0.as_str());
    match (words[j].0.as_str(), next) {
        ("Class", Some("Features")) | ("Spell", Some("Slots")) | ("Spellcasting", _) => true,
        _ => {
            !first_feature.is_empty()
                && words.len() >= j + first_feature.len()
                && words[j..j + first_feature.len()]
                    .iter()
                    .zip(first_feature)
                    .all(|(w, f)| w.0 == *f)
        }
    }
}

fn last_row(
    toks: &[(usize, &str)],
    from: usize,
    (l, t): (usize, usize),
    first_feature: &[&str],
) -> Option<usize> {
    let words = merged_idx(toks.get(from..)?);
    if words.len() < l + 1 || !words[..l].iter().all(|w| is_cell(&w.0)) {
        return None;
    }
    let mut i = l;
    if is_cell(&words[i].0) {
        i += 1;
    } else if t == 0 {
        let stop =
            (i + 1..words.len().min(i + 16)).find(|&j| ends_table(&words, j, first_feature))?;
        return Some(from + words[stop - 1].1);
    } else {
        while i < words.len() && !is_cell(&words[i].0) {
            i += 1;
        }
    }
    if words.len() < i + t || !words[i..i + t].iter().all(|w| is_cell(&w.0)) {
        return None;
    }
    Some(from + words[i + t - 1].1)
}

fn split_names(segment: &str, k: usize) -> Vec<String> {
    let mut s = segment.replace(['—', '–'], " ");
    for g in SLOT_GROUPS {
        s = s.replace(g, " ");
    }
    let words: Vec<&str> = s.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let hit = VOCAB
            .iter()
            .map(|v| v.split(' ').collect::<Vec<_>>())
            .filter(|v| words[i..].starts_with(v))
            .max_by_key(Vec::len);
        let n = hit.map_or(1, |v| v.len());
        out.push(words[i..i + n].join(" "));
        i += n;
    }
    while out.len() > k && k > 0 {
        let last = out.pop().unwrap_or_default();
        if let Some(prev) = out.last_mut() {
            prev.push(' ');
            prev.push_str(&last);
        }
    }
    if k == 0 {
        out.clear();
    }
    out.resize(k, String::new());
    out
}

fn header_names(header: &str, (l, t): (usize, usize)) -> Vec<String> {
    let h = header
        .trim()
        .trim_start_matches("Proficiency Bonus")
        .trim_start_matches("Prof. Bonus")
        .trim();
    let (before, after, features) = match h.find("Features") {
        Some(i) => {
            let (b, a) = (&h[..i], &h[i + "Features".len()..]);
            match b.trim_end().strip_suffix("Class") {
                Some(b2) => (b2, a, "Class Features"),
                None => (b, a, "Features"),
            }
        }
        None => (h, "", "Features"),
    };
    let mut out = vec!["Level".to_owned(), "Proficiency Bonus".to_owned()];
    out.extend(split_names(before, l));
    out.push(features.to_owned());
    out.extend(split_names(after, t));
    out
}

fn table(header: &[String], rows: &[Vec<String>]) -> String {
    let line = |cells: &[String]| {
        let escaped: Vec<String> = cells.iter().map(|c| c.replace('|', "\\|")).collect();
        format!("| {} |\n", escaped.join(" | "))
    };
    let mut out = line(header);
    out.push_str(&format!("|{}\n", " --- |".repeat(header.len())));
    for r in rows {
        out.push_str(&line(r));
    }
    out
}

fn caption_start(s: &str, at: usize) -> Option<usize> {
    let before = &s[..at];
    let boundary = [". ", "\n", ": "]
        .iter()
        .filter_map(|p| before.rfind(p).map(|j| j + p.len()))
        .max()
        .unwrap_or(0);
    let words: Vec<&str> = before[boundary..].split_whitespace().collect();
    let ok = (1..=6).contains(&words.len())
        && words[0].starts_with(|c: char| c.is_uppercase())
        && !words.iter().any(|w| is_cell(w));
    ok.then_some(boundary)
}

fn splice(s: &str, start: usize, end: usize, table: &str) -> String {
    let (head, block) = match caption_start(s, start) {
        Some(c) => (
            s[..c].trim_end(),
            format!("**{}**\n\n{table}", s[c..start].trim()),
        ),
        None => (s[..start].trim_end(), table.to_owned()),
    };
    let sep = if head.is_empty() { "" } else { "\n\n" };
    format!("{head}{sep}{block}\n{}", s[end..].trim_start())
}

pub fn drop_cards(s: &str) -> String {
    let mut out = s.to_owned();
    for (card, grid) in [
        ("Level 1 Proficiency Bonus +2 ", " Level Proficiency Bonus "),
        (
            "Character Level 1 Character Class ",
            " Character Level Spell Level 1 ",
        ),
    ] {
        if let Some(a) = out.find(card)
            && let Some(b) = out[a..].find(grid)
        {
            out.replace_range(a..a + b + 1, "");
        }
    }
    out
}

pub fn level_grids(s: &str) -> (String, Vec<String>) {
    let mut out = s.to_owned();
    let mut features = Vec::new();
    while let Some((start, end, md, names)) = level_grid(&out) {
        features.extend(names);
        out = splice(&out, start, end, &md);
    }
    (out, features)
}

fn level_grid(s: &str) -> Option<(usize, usize, String, Vec<String>)> {
    let toks = tokens(s);
    for h in 0..toks.len().saturating_sub(3) {
        if toks[h].1 != "Level" || !matches!(toks[h + 1].1, "Proficiency" | "Prof.") {
            continue;
        }
        for j in h + 2..toks.len().saturating_sub(1).min(h + 90) {
            let plain = match (toks[j].1, toks[j + 1].1) {
                ("1st", "+2") => false,
                ("1", "+2") => true,
                _ => continue,
            };
            let Some(grid) = find_rows(&toks, plain, &mut vec![j], None, false)
                .or_else(|| find_rows(&toks, plain, &mut vec![j], None, true))
            else {
                break;
            };
            let first_row = merged(&toks[j + 2..*grid.starts.get(1)?]);
            let first_cell = cells(&first_row, grid.shape)[grid.shape.0].clone();
            let first_feature: Vec<&str> = first_cell
                .split(", ")
                .next()
                .and_then(|f| f.split(" (").next())
                .map(|f| f.split_whitespace().collect())
                .unwrap_or_default();
            let last = *grid.starts.last()?;
            let Some(end_tok) = last_row(&toks, last + 2, grid.shape, &first_feature) else {
                break;
            };
            let mut rows = Vec::with_capacity(20);
            let mut names = Vec::new();
            for (k, &st) in grid.starts.iter().enumerate() {
                let stop = grid.starts.get(k + 1).copied().unwrap_or(end_tok);
                let words = merged(&toks[st + 2..stop]);
                let mut row = vec![toks[st].1.to_owned(), toks[st + 1].1.to_owned()];
                row.extend(cells(&words, grid.shape));
                names.push(row[2 + grid.shape.0].clone());
                rows.push(row);
            }
            let header = s[end_of(toks[h])..toks[j].0].to_owned();
            let md = table(&header_names(&header, grid.shape), &rows);
            let end = toks.get(end_tok).map_or(s.len(), |t| t.0);
            return Some((toks[h].0, end, md, names));
        }
    }
    None
}

pub fn slot_grid(s: &str) -> String {
    let toks = tokens(s);
    for h in 0..toks.len().saturating_sub(5) {
        if toks[h].1 != "Character"
            || toks[h + 1].1 != "Level"
            || toks[h + 2].1 != "Spell"
            || toks[h + 3].1 != "Level"
            || toks[h + 4].1 != "1"
        {
            continue;
        }
        let mut m = 0;
        while toks.get(h + 2 + m * 3).map(|t| t.1) == Some("Spell")
            && toks.get(h + 4 + m * 3).map(|t| t.1) == Some((m + 1).to_string().as_str())
        {
            m += 1;
        }
        let body = h + 2 + m * 3;
        let width = m + 1;
        let Some(cells) = toks.get(body..body + 20 * width) else {
            continue;
        };
        let ok = cells
            .chunks(width)
            .enumerate()
            .all(|(r, row)| row[0].1 == (r + 1).to_string() && row.iter().all(|c| is_cell(c.1)));
        if !ok {
            continue;
        }
        let mut header = vec!["Level".to_owned()];
        header.extend((1..=m).map(ordinal));
        let rows: Vec<Vec<String>> = cells
            .chunks(width)
            .map(|row| row.iter().map(|c| c.1.to_owned()).collect())
            .collect();
        let end = end_of(cells[cells.len() - 1]);
        return splice(s, toks[h].0, end, &table(&header, &rows));
    }
    s.to_owned()
}

pub fn die_tables(s: &str) -> String {
    let mut out = s.to_owned();
    while let Some((start, end, md)) = die_table(&out) {
        out = splice(&out, start, end, &md);
    }
    out
}

fn starts_upper(w: &str) -> bool {
    w.trim_start_matches(['“', '"', '‘', '\''])
        .starts_with(|c: char| c.is_uppercase())
}

fn die_table(s: &str) -> Option<(usize, usize, String)> {
    let toks = tokens(s);
    'outer: for h in 0..toks.len() {
        let Some(n) = DICE
            .iter()
            .position(|d| *d == toks[h].1)
            .map(|i| [4, 6, 8, 10, 12, 20][i])
        else {
            continue;
        };
        let Some(one) = (h + 2..toks.len().min(h + 7))
            .find(|&j| toks[j].1 == "1" && toks.get(j + 1).is_some_and(|w| starts_upper(w.1)))
        else {
            continue;
        };
        if !starts_upper(toks[h + 1].1) {
            continue;
        }
        let mut starts = vec![one];
        for k in 2..=n {
            let prev = starts[k - 2];
            let key = k.to_string();
            let Some(i) = (prev + 2..toks.len().saturating_sub(1).min(prev + 150))
                .find(|&i| toks[i].1 == key && starts_upper(toks[i + 1].1))
            else {
                continue 'outer;
            };
            starts.push(i);
        }
        let last = starts[n - 1] + 1;
        let tagged = starts.windows(2).all(|w| toks[w[1] - 1].1.ends_with(')'));
        let ends_row = |w: &str| {
            if tagged {
                w.ends_with(')')
            } else {
                w.ends_with(['.', '!', '?', ')'])
            }
        };
        let end_tok = (last..toks.len())
            .find(|&i| {
                DICE.contains(&toks[i].1) || ends_row(toks[i - 1].1) && starts_upper(toks[i].1)
            })
            .unwrap_or(toks.len());
        let rows: Vec<Vec<String>> = starts
            .iter()
            .enumerate()
            .map(|(k, &st)| {
                let stop = starts.get(k + 1).copied().unwrap_or(end_tok);
                let text: Vec<&str> = toks[st + 1..stop].iter().map(|t| t.1).collect();
                vec![toks[st].1.to_owned(), text.join(" ")]
            })
            .collect();
        let title: Vec<&str> = toks[h + 1..one].iter().map(|t| t.1).collect();
        let header = vec![toks[h].1.to_owned(), title.join(" ")];
        let end = toks.get(end_tok).map_or(s.len(), |t| t.0);
        return Some((toks[h].0, end, table(&header, &rows)));
    }
    None
}

pub fn trait_list(s: &str, labels: &[&str], stop: &str, keep_stop: bool) -> String {
    let mut found: Vec<(usize, &str)> = Vec::new();
    let mut from = 0;
    for label in labels {
        let window = &s[from..];
        let hit = window
            .find(&format!("{label} "))
            .filter(|&i| i == 0 || window[..i].ends_with(' '))
            .filter(|&i| found.is_empty() || i < 400);
        if let Some(i) = hit {
            found.push((from + i, label));
            from += i + label.len();
        }
    }
    if found.len() < 2 {
        return s.to_owned();
    }
    let found_stop = if stop.is_empty() {
        Some(s.len() - from)
    } else {
        s[from..].find(stop)
    };
    let Some(end) =
        found_stop.map(|i| from + i + if keep_stop { stop.trim_end().len() } else { 0 })
    else {
        return s.to_owned();
    };
    let mut out = s[..found[0].0].trim_end().to_owned();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    for (k, (at, label)) in found.iter().enumerate() {
        let value_end = found.get(k + 1).map_or(end, |n| n.0);
        let value = s[at + label.len()..value_end]
            .trim()
            .trim_start_matches(':')
            .trim();
        out.push_str(&format!("- **{}:** {value}\n", label.trim_end_matches(':')));
    }
    out.push('\n');
    out.push_str(s[end..].trim_start());
    out
}

pub fn class_rules(s: &str) -> String {
    const STEPS: [(&str, &str); 10] = [
        ("Class Features As a", "\n\n### Class Features\n\nAs a"),
        (
            "Hit Points Hit Dice:",
            "\n\n### Hit Points\n\n- **Hit Dice:**",
        ),
        (
            "Hit Points at 1st Level:",
            "\n- **Hit Points at 1st Level:**",
        ),
        (
            "Hit Points at Higher Levels:",
            "\n- **Hit Points at Higher Levels:**",
        ),
        (
            "Proficiencies Armor:",
            "\n\n### Proficiencies\n\n- **Armor:**",
        ),
        ("Weapons:", "\n- **Weapons:**"),
        ("Tools:", "\n- **Tools:**"),
        ("Saving Throws:", "\n- **Saving Throws:**"),
        ("Skills:", "\n- **Skills:**"),
        ("Equipment You start", "\n\n### Equipment\n\nYou start"),
    ];
    let mut out = s.to_owned();
    for (pat, _) in STEPS {
        if let Some(label) = pat.strip_suffix(':') {
            out = out.replace(&format!("{label} :"), pat);
        }
    }
    let mut from = 0;
    let mut in_block = false;
    for (pat, rep) in STEPS {
        let bullet = rep.starts_with("\n-");
        let Some(i) = out[from..]
            .find(pat)
            .map(|i| i + from)
            .filter(|&i| !bullet || in_block && i - from < 400)
        else {
            in_block = in_block && bullet;
            continue;
        };
        in_block = true;
        let start = if pat.starts_with("Class Features") {
            heading_owner(&out, i)
        } else {
            i
        };
        let rep = if start < i {
            rep.replacen(
                "### Class Features",
                &format!("### {}Class Features", &out[start..i]),
                1,
            )
        } else {
            rep.to_owned()
        };
        let head = out[..start].trim_end().to_owned();
        let tail = out[i + pat.len()..].to_owned();
        out = format!("{head}{rep}{tail}");
        from = head.len() + rep.len();
    }
    if let Some(eq) = out.find("### Equipment") {
        let mut end = (eq + 1500).min(out.len());
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        let section = out[eq..end]
            .replace(" ( a ) ", "\n- (*a*) ")
            .replace(" (a) ", "\n- (*a*) ")
            .replace("( b )", "(*b*)")
            .replace("( c )", "(*c*)");
        out.replace_range(eq..end, &section);
    }
    out
}

fn heading_owner(s: &str, at: usize) -> usize {
    let before = &s[..at];
    let boundary = [". ", "\n"]
        .iter()
        .filter_map(|p| before.rfind(p).map(|j| j + p.len()))
        .max()
        .unwrap_or(0);
    let words = before[boundary..].split_whitespace().count();
    if (1..=2).contains(&words) && starts_upper(&before[boundary..]) {
        boundary
    } else {
        at
    }
}

pub fn core_traits(s: &str) -> String {
    const LABELS: [&str; 8] = [
        "Primary Ability",
        "Hit Point Die",
        "Saving Throw Proficiencies",
        "Skill Proficiencies",
        "Weapon Proficiencies",
        "Tool Proficiencies",
        "Armor Training",
        "Starting Equipment",
    ];
    let Some(core) = s.find(" Traits Primary Ability ") else {
        return s.to_owned();
    };
    let title_start = s[..core].rfind("Core ").unwrap_or(core);
    let title = s[title_start..core + " Traits".len()].to_owned();
    let head = format!("{}\n\n### {title}\n\n", s[..title_start].trim_end());
    let listed = trait_list(
        &s[core + " Traits ".len()..],
        &LABELS,
        " Becoming a ",
        false,
    );
    let mut out = format!("{head}{listed}");
    if let Some(b) = out.find("\nBecoming a ")
        && let Some(e) = out[b..].find(" … ").map(|i| b + i)
    {
        let title = out[b + 1..e].to_owned();
        out.replace_range(b..e + " … ".len(), &format!("\n### {title}\n\n"));
    }
    for label in ["As a Level 1 Character", "As a Multiclass Character"] {
        if let Some(i) = out.find(&format!("{label} ")) {
            let rest = out[i + label.len()..].to_owned();
            out = format!("{}\n\n**{label}.**{rest}", out[..i].trim_end());
        }
    }
    out
}

pub fn feature_names(cells: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for cell in cells {
        for raw in cell.split(", ") {
            let name = raw.split(" (").next().unwrap_or(raw).trim();
            if !name.is_empty()
                && !is_cell(name)
                && !name.to_lowercase().ends_with(" feature")
                && !out.iter().any(|s| s == name)
            {
                out.push(name.to_owned());
            }
        }
    }
    out
}

pub fn feature_headings(s: &str, names: &[String]) -> String {
    let mut out = s.to_owned();
    let start = out.rfind("|\n").unwrap_or(0);
    for name in names {
        {
            let name = name.as_str();
            if out.contains(&format!("### {name}\n")) || out.contains(&format!(": {name}\n")) {
                continue;
            }
            let pat = format!(" {name} ");
            let mut from = start;
            while let Some(i) = out[from..].find(&pat).map(|i| i + from) {
                let after = i + pat.len();
                let next = out[after..].split_whitespace().next().unwrap_or_default();
                if crate::dnd_data::SENTENCE_STARTS.contains(&next) {
                    let rep = format!("\n\n### {name}\n\n");
                    let head = out[..i].trim_end().len();
                    out.replace_range(head..after, &rep);
                    break;
                }
                from = after;
            }
        }
    }
    out
}

const SPELLCASTING_PARTS: [&str; 7] = [
    "Spells Known of 1st Level and Higher",
    "Preparing and Casting Spells",
    "Spellcasting Ability",
    "Spellcasting Focus",
    "Ritual Casting",
    "Spell Slots",
    "Cantrips",
];

const NOT_LABELS: [&str; 6] = ["See", "Use", "Gain", "Roll", "Make", "Add"];

fn label_word(w: &str) -> bool {
    !w.contains(',')
        && (w.starts_with(|c: char| c.is_uppercase())
            || w.starts_with(|c: char| c.is_ascii_digit())
            || matches!(
                w,
                "of" | "the" | "and" | "a" | "an" | "to" | "in" | "on" | "with" | "for" | "or"
            ))
}

fn label_at(toks: &[(usize, &str)], i: usize) -> Option<(usize, String)> {
    for part in SPELLCASTING_PARTS {
        let words: Vec<&str> = part.split(' ').collect();
        let n = words.len();
        if toks.len() > i + n
            && toks[i..i + n].iter().zip(&words).all(|(t, w)| t.1 == *w)
            && starts_upper(toks[i + n].1)
        {
            return Some((n, part.to_owned()));
        }
    }
    let first = toks.get(i)?.1;
    if crate::dnd_data::SENTENCE_STARTS.contains(&first) || NOT_LABELS.contains(&first) {
        return None;
    }
    for k in 1..=6 {
        let (Some(last), Some(next)) = (toks.get(i + k - 1), toks.get(i + k)) else {
            return None;
        };
        let words: Vec<&str> = toks[i..i + k].iter().map(|t| t.1).collect();
        if !words[..k - 1].iter().all(|w| label_word(w)) || !starts_upper(first) {
            return None;
        }
        if let Some(stem) = last.1.strip_suffix('.')
            && label_word(stem)
            && !stem.is_empty()
            && starts_upper(next.1)
        {
            let mut label = words[..k - 1].join(" ");
            if !label.is_empty() {
                label.push(' ');
            }
            label.push_str(stem);
            return Some((k, label));
        }
        if last.1.contains(['.', '!', '?', ':', ';']) {
            return None;
        }
    }
    None
}

pub fn sub_labels(block: &str) -> String {
    if block.starts_with(['|', '#', '-', '*']) {
        return block.to_owned();
    }
    let toks = tokens(block);
    let mut out = String::with_capacity(block.len() + 64);
    let mut last = 0;
    let mut i = 1;
    while i < toks.len() {
        let after_sentence = toks[i - 1].1.ends_with(['.', '!', '?']);
        match label_at(&toks, i).filter(|_| after_sentence) {
            Some((n, label)) => {
                out.push_str(block[last..toks[i].0].trim_end());
                out.push_str(&format!("\n\n**{label}.** "));
                last = toks.get(i + n).map_or(block.len(), |t| t.0);
                i += n;
            }
            None => i += 1,
        }
    }
    out.push_str(&block[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SORCERER_2014: &str = "Third, choose the light cantrip. The Sorcerer Level Proficiency Bonus Sorcery Points Features Cantrips Known Spells Known Spell Level Slots: 1st Spell Level Slots: 2nd 1st +2 — Spellcasting, Sorcerous Origin 4 2 2 — 2nd +2 2 Font of Magic 4 3 3 — 3rd +2 3 Metamagic 4 4 4 2 4th +2 4 Ability Score Improvement 5 5 4 3 5th +3 5 — 5 6 4 3 6th +3 6 Sorcerous Origin feature 5 7 4 3 7th +3 7 — 5 8 4 3 8th +3 8 Ability Score Improvement 5 9 4 3 9th +4 9 — 5 10 4 3 10th +4 10 Metamagic 6 11 4 3 11th +4 11 — 6 12 4 3 12th +4 12 Ability Score Improvement 6 12 4 3 13th +5 13 — 6 13 4 3 14th +5 14 Sorcerous Origin feature 6 13 4 3 15th +5 15 — 6 14 4 3 16th +5 16 Ability Score Improvement 6 14 4 3 17th +6 17 Metamagic 6 15 4 3 18th +6 18 Sorcerous Origin feature 6 15 4 3 19th +6 19 Ability Score Improvement 6 15 4 3 20th +6 20 Sorcerous Restoration 6 15 4 3 Class Features As a sorcerer, you gain the following class features. Hit Points Hit Dice: 1d6 per sorcerer level Hit Points at 1st Level: 6 + your Constitution modifier Proficiencies Armor: None Weapons: Daggers Tools: None Saving Throws: Constitution, Charisma Skills: Choose two Equipment You start with the following: ( a ) a light crossbow or ( b ) any simple weapon ( a ) a component pouch or ( b ) an arcane focus Two daggers Spellcasting An event in your past marked you. Font of Magic At 2nd level, you tap into a deep wellspring.";

    #[test]
    fn class_level_table_2014() {
        let (t, names) = level_grids(SORCERER_2014);
        assert!(t.contains("**The Sorcerer**\n\n| Level | Proficiency Bonus | Sorcery Points | Features | Cantrips Known | Spells Known | 1st | 2nd |\n"), "{t}");
        assert!(
            t.contains("| 1st | +2 | — | Spellcasting, Sorcerous Origin | 4 | 2 | 2 | — |\n"),
            "{t}"
        );
        assert!(
            t.contains(
                "| 20th | +6 | 20 | Sorcerous Restoration | 6 | 15 | 4 | 3 |\n\nClass Features As a"
            ),
            "{t}"
        );
        assert_eq!(names.len(), 20);
        let t = feature_headings(&class_rules(&t), &feature_names(&names));
        assert!(t.contains("### Class Features\n\nAs a sorcerer"), "{t}");
        assert!(t.contains("### Hit Points\n\n- **Hit Dice:** 1d6 per sorcerer level\n- **Hit Points at 1st Level:**"), "{t}");
        assert!(
            t.contains("\n- **Saving Throws:** Constitution, Charisma\n"),
            "{t}"
        );
        assert!(
            t.contains(
                "\n- (*a*) a light crossbow or (*b*) any simple weapon\n- (*a*) a component pouch"
            ),
            "{t}"
        );
        assert!(
            t.contains("Two daggers\n\n### Spellcasting\n\nAn event"),
            "{t}"
        );
        assert!(t.contains("\n\n### Font of Magic\n\nAt 2nd level"), "{t}");
    }

    #[test]
    fn class_level_table_2024_with_cards() {
        let mut d = String::from(
            "These features are listed in the Rogue Features table. Rogue Class Features Level 1 Proficiency Bonus +2 Class Features Expertise Sneak Attack 1d6 Level 2 Proficiency Bonus +2 Class Features Cunning Action Sneak Attack 1d6 Level Proficiency Bonus Class Features Sneak Attack",
        );
        for k in 1..=20usize {
            let pb = 2 + (k - 1) / 4;
            let feature = if k % 2 == 0 {
                "—".to_owned()
            } else {
                format!("Feature {k}, Other")
            };
            d.push_str(&format!(" {k} +{pb} {feature} {}d6", k.div_ceil(2)));
        }
        d.push_str(" Level 1: Expertise You gain expertise.");
        let (t, _) = level_grids(&drop_cards(&d));
        assert!(!t.contains("Level 1 Proficiency Bonus"), "{t}");
        assert!(t.contains("**Rogue Class Features**\n\n| Level | Proficiency Bonus | Class Features | Sneak Attack |\n"), "{t}");
        assert!(
            t.contains("| 1 | +2 | Feature 1, Other | 1d6 |\n| 2 | +2 | — | 1d6 |\n"),
            "{t}"
        );
        assert!(
            t.contains("| 20 | +6 | — | 10d6 |\n\nLevel 1: Expertise"),
            "{t}"
        );
    }

    #[test]
    fn rows_survive_cells_that_look_like_row_starts() {
        let mut d =
            String::from("The Barbarian Level Proficiency Bonus Class Features Rages Rage Damage");
        for k in 1..=20 {
            let pb = 2 + (k - 1) / 4;
            let name = char::from(b'A' + k as u8);
            d.push_str(&format!(" {k} +{pb} Feature {name} 2 +2"));
        }
        d.push_str(" Class Features As a barbarian.");
        let (t, _) = level_grids(&d);
        assert!(
            t.contains("| 1 | +2 | Feature B | 2 | +2 |\n| 2 | +2 | Feature C | 2 | +2 |\n"),
            "{t}"
        );
    }

    #[test]
    fn spell_slot_grid() {
        let mut d = String::from(
            "Sorcerer Spell Slots Character Level 1 Character Class Sorcerer Spell Level 1 2 Spell Level 2 - Character Level Spell Level 1 Spell Level 2",
        );
        for k in 1..=20 {
            d.push_str(&format!(" {k} {} —", k + 1));
        }
        d.push_str(" Next part.");
        let t = slot_grid(&drop_cards(&d));
        assert!(
            t.starts_with("**Sorcerer Spell Slots**\n\n| Level | 1st | 2nd |\n"),
            "{t}"
        );
        assert!(t.contains("| 20 | 21 | — |\n\nNext part."), "{t}");
    }

    #[test]
    fn inline_sub_sections_become_labels() {
        let t = sub_labels(
            "Drawing on magic, you cast spells. Cantrips. You know four. Spell Slots. The table shows slots. Prepared Spells of Level 1+. You prepare spells. See Chapter 7. Then more.",
        );
        assert_eq!(
            t,
            "Drawing on magic, you cast spells.\n\n**Cantrips.** You know four.\n\n**Spell Slots.** The table shows slots.\n\n**Prepared Spells of Level 1+.** You prepare spells. See Chapter 7. Then more."
        );
        let t = sub_labels(
            "You regain them on a long rest. Spell Slots The Sorcerer table shows slots. Spellcasting Focus You can use a focus.",
        );
        assert_eq!(
            t,
            "You regain them on a long rest.\n\n**Spell Slots.** The Sorcerer table shows slots.\n\n**Spellcasting Focus.** You can use a focus."
        );
    }

    #[test]
    fn die_tables_become_tables() {
        let d = "Flaws might be hidden. d4 Personality Trait 1 I idolize a hero. 2 I find common ground. 3 I see omens. The gods speak. 4 Nothing can shake me. d4 Ideal 1 Tradition. Keep it. (Lawful) 2 Charity. Help. (Good) 3 Change. Move. (Chaotic) 4 Power. Rise. (Lawful) Then more prose.";
        let t = die_tables(d);
        assert!(
            t.contains(
                "hidden.\n\n| d4 | Personality Trait |\n| --- | --- |\n| 1 | I idolize a hero. |\n"
            ),
            "{t}"
        );
        assert!(t.contains("| 3 | I see omens. The gods speak. |\n| 4 | Nothing can shake me. |\n\n| d4 | Ideal |"), "{t}");
        assert!(
            t.contains("| 4 | Power. Rise. (Lawful) |\n\nThen more prose."),
            "{t}"
        );
    }

    #[test]
    fn core_traits_and_background_traits() {
        let d = "Sorcerers are rare. Core Sorcerer Traits Primary Ability Charisma Hit Point Die D6 per Sorcerer level Armor Training None Starting Equipment Choose A or B: (A) Spear; or (B) 50 GP Becoming a Sorcerer … As a Level 1 Character Gain all the traits. As a Multiclass Character Gain the Hit Point Die.";
        let t = core_traits(d);
        assert!(t.contains("rare.\n\n### Core Sorcerer Traits\n\n- **Primary Ability:** Charisma\n- **Hit Point Die:** D6 per Sorcerer level\n- **Armor Training:** None\n- **Starting Equipment:** Choose A or B: (A) Spear; or (B) 50 GP\n\n### Becoming a Sorcerer\n\n**As a Level 1 Character.** Gain"), "{t}");
        let b = trait_list(
            "Ability Scores Intelligence, Wisdom Feat Magic Initiate Equipment Choose A or B: (A) Book, 8 GP; or (B) 50 GP You devoted yourself.",
            &["Ability Scores", "Feat", "Equipment"],
            " GP ",
            true,
        );
        assert!(b.starts_with("- **Ability Scores:** Intelligence, Wisdom\n- **Feat:** Magic Initiate\n- **Equipment:** Choose A or B: (A) Book, 8 GP; or (B) 50 GP\n\nYou devoted yourself."), "{b}");
    }
}
