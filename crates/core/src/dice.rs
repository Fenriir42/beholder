use core::fmt;

use serde::{Deserialize, Serialize};

use crate::actions::Roll;

pub const MAX_DICE: u8 = 100;
pub const MAX_SIDES: u16 = 1000;
pub const MAX_TERMS: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keep {
    #[default]
    All,
    Highest(u8),
    Lowest(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Term {
    pub negative: bool,
    pub count: u8,
    pub sides: u16,
    pub keep: Keep,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expr {
    pub terms: Vec<Term>,
    pub modifier: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Advantage {
    Normal,
    Advantage,
    Disadvantage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TermOutcome {
    pub term: Term,
    pub rolls: Vec<u16>,
    pub kept: Vec<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub terms: Vec<TermOutcome>,
    pub modifier: i32,
    pub total: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DiceError {
    #[error("not a dice expression")]
    Syntax,
    #[error("too many dice, sides or terms")]
    TooBig,
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for t in &self.terms {
            if t.negative {
                f.write_str(if first { "-" } else { " - " })?;
            } else if !first {
                f.write_str(" + ")?;
            }
            write!(f, "{}d{}", t.count, t.sides)?;
            match t.keep {
                Keep::All => {}
                Keep::Highest(n) => write!(f, "kh{n}")?,
                Keep::Lowest(n) => write!(f, "kl{n}")?,
            }
            first = false;
        }
        match (first, self.modifier) {
            (true, m) => write!(f, "{m}"),
            (false, 0) => Ok(()),
            (false, m) if m > 0 => write!(f, " + {m}"),
            (false, m) => write!(f, " - {}", -m),
        }
    }
}

fn number<T: core::str::FromStr>(s: &str) -> Result<T, DiceError> {
    s.parse().map_err(|_| DiceError::Syntax)
}

fn term(s: &str, negative: bool) -> Result<Option<Term>, DiceError> {
    let Some((count, rest)) = s.split_once('d') else {
        return Ok(None);
    };
    let count: u8 = if count.is_empty() { 1 } else { number(count)? };
    let (sides, keep) = match rest.find('k') {
        Some(i) => {
            let (sides, k) = rest.split_at(i);
            let keep = match (k.get(..2), k.get(2..)) {
                (Some("kh"), Some(n)) => Keep::Highest(if n.is_empty() { 1 } else { number(n)? }),
                (Some("kl"), Some(n)) => Keep::Lowest(if n.is_empty() { 1 } else { number(n)? }),
                _ => return Err(DiceError::Syntax),
            };
            (sides, keep)
        }
        None => (rest, Keep::All),
    };
    let sides: u16 = number(sides)?;
    let kept = match keep {
        Keep::All => count,
        Keep::Highest(n) | Keep::Lowest(n) => n,
    };
    if count == 0 || sides < 2 || kept == 0 || kept > count {
        return Err(DiceError::Syntax);
    }
    if count > MAX_DICE || sides > MAX_SIDES {
        return Err(DiceError::TooBig);
    }
    Ok(Some(Term {
        negative,
        count,
        sides,
        keep,
    }))
}

impl Expr {
    pub fn parse(input: &str) -> Result<Self, DiceError> {
        let mut s: String = input
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let mut advantage = Advantage::Normal;
        for (suffix, a) in [
            ("adv", Advantage::Advantage),
            ("dis", Advantage::Disadvantage),
        ] {
            if let Some(rest) = s.strip_suffix(suffix) {
                s = rest.to_owned();
                advantage = a;
            }
        }
        if s.is_empty() {
            return Err(DiceError::Syntax);
        }
        let mut expr = Self::default();
        let mut start = 0;
        let bytes = s.as_bytes();
        for i in 0..=s.len() {
            let at_end = i == s.len();
            if !at_end && (i == 0 || !matches!(bytes[i], b'+' | b'-')) {
                continue;
            }
            let chunk = &s[start..i];
            let (negative, body) = match chunk.strip_prefix('-') {
                Some(b) => (true, b),
                None => (false, chunk.strip_prefix('+').unwrap_or(chunk)),
            };
            if body.is_empty() {
                return Err(DiceError::Syntax);
            }
            match term(body, negative)? {
                Some(t) => expr.terms.push(t),
                None => {
                    let n: i32 = number(body)?;
                    expr.modifier += if negative { -n } else { n };
                }
            }
            start = i;
        }
        if expr.terms.len() > MAX_TERMS || expr.modifier.abs() > 10_000 {
            return Err(DiceError::TooBig);
        }
        Ok(expr.with_advantage(advantage))
    }

    pub fn d20(modifier: i32) -> Self {
        Self {
            terms: vec![Term {
                negative: false,
                count: 1,
                sides: 20,
                keep: Keep::All,
            }],
            modifier,
        }
    }

    pub fn with_advantage(mut self, advantage: Advantage) -> Self {
        let keep = match advantage {
            Advantage::Normal => return self,
            Advantage::Advantage => Keep::Highest(1),
            Advantage::Disadvantage => Keep::Lowest(1),
        };
        if let Some(t) = self
            .terms
            .iter_mut()
            .find(|t| t.sides == 20 && t.count == 1 && !t.negative)
        {
            t.count = 2;
            t.keep = keep;
        }
        self
    }

    pub fn critical(mut self) -> Self {
        for t in &mut self.terms {
            t.count = t.count.saturating_mul(2).min(MAX_DICE);
            if let Keep::Highest(n) | Keep::Lowest(n) = &mut t.keep {
                *n = n.saturating_mul(2);
            }
        }
        self
    }

    pub fn is_d20_test(&self) -> bool {
        self.terms
            .first()
            .is_some_and(|t| t.sides == 20 && !t.negative)
    }
}

impl From<Roll> for Expr {
    fn from(r: Roll) -> Self {
        let terms = if r.count == 0 || r.sides < 2 {
            Vec::new()
        } else {
            vec![Term {
                negative: false,
                count: r.count.min(MAX_DICE),
                sides: u16::from(r.sides),
                keep: Keep::All,
            }]
        };
        Self {
            terms,
            modifier: i32::from(r.bonus),
        }
    }
}

pub fn roll(expr: &Expr, mut die: impl FnMut(u16) -> u16) -> Outcome {
    let mut total = expr.modifier;
    let terms = expr
        .terms
        .iter()
        .map(|t| {
            let rolls: Vec<u16> = (0..t.count)
                .map(|_| die(t.sides).clamp(1, t.sides))
                .collect();
            let mut order: Vec<usize> = (0..rolls.len()).collect();
            order.sort_by_key(|&i| rolls[i]);
            let mut kept = vec![true; rolls.len()];
            match t.keep {
                Keep::All => {}
                Keep::Highest(n) => {
                    for &i in &order[..rolls.len() - usize::from(n)] {
                        kept[i] = false;
                    }
                }
                Keep::Lowest(n) => {
                    for &i in &order[usize::from(n)..] {
                        kept[i] = false;
                    }
                }
            }
            let sum: i32 = rolls
                .iter()
                .zip(&kept)
                .filter(|(_, k)| **k)
                .map(|(r, _)| i32::from(*r))
                .sum();
            total += if t.negative { -sum } else { sum };
            TermOutcome {
                term: *t,
                rolls,
                kept,
            }
        })
        .collect();
    Outcome {
        terms,
        modifier: expr.modifier,
        total,
    }
}

impl Outcome {
    pub fn natural(&self) -> Option<u16> {
        let first = self.terms.first().filter(|t| t.term.sides == 20)?;
        first
            .rolls
            .iter()
            .zip(&first.kept)
            .find(|(_, k)| **k)
            .map(|(r, _)| *r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(values: &[u16]) -> impl FnMut(u16) -> u16 + '_ {
        let mut i = 0;
        move |_| {
            let v = values[i % values.len()];
            i += 1;
            v
        }
    }

    #[test]
    fn parses_common_notation() {
        let e = Expr::parse("2d20kh1 + 5").unwrap();
        assert_eq!(e.to_string(), "2d20kh1 + 5");
        assert_eq!(Expr::parse("d20").unwrap().to_string(), "1d20");
        assert_eq!(Expr::parse("8d6").unwrap().to_string(), "8d6");
        assert_eq!(
            Expr::parse("1d8+1d6-2").unwrap().to_string(),
            "1d8 + 1d6 - 2"
        );
        assert_eq!(Expr::parse("-1d4+3").unwrap().to_string(), "-1d4 + 3");
        assert_eq!(Expr::parse("d20+4 adv").unwrap().to_string(), "2d20kh1 + 4");
        assert_eq!(Expr::parse("1d20-1dis").unwrap().to_string(), "2d20kl1 - 1");
        assert_eq!(Expr::parse("4d6kh3").unwrap().to_string(), "4d6kh3");
        assert_eq!(Expr::parse("7").unwrap().to_string(), "7");
        for bad in ["", "d", "2d", "d1", "abc", "2d6+", "1d20kh2", "0d6", "+"] {
            assert_eq!(Expr::parse(bad), Err(DiceError::Syntax), "{bad}");
        }
        assert_eq!(Expr::parse("101d6"), Err(DiceError::TooBig));
        assert_eq!(Expr::parse("1d1001"), Err(DiceError::TooBig));
    }

    #[test]
    fn rolls_keep_and_total() {
        let o = roll(&Expr::parse("2d20kh1+5").unwrap(), fixed(&[4, 17]));
        assert_eq!((o.total, o.natural()), (22, Some(17)));
        assert_eq!(o.terms[0].kept, [false, true]);
        let o = roll(&Expr::parse("2d20kl1").unwrap(), fixed(&[4, 17]));
        assert_eq!((o.total, o.natural()), (4, Some(4)));
        let o = roll(&Expr::parse("4d6kh3").unwrap(), fixed(&[1, 6, 3, 5]));
        assert_eq!(o.total, 14);
        let o = roll(&Expr::parse("1d8-1d4+2").unwrap(), fixed(&[6, 3]));
        assert_eq!(o.total, 5);
        let o = roll(&Expr::parse("1d6").unwrap(), fixed(&[99]));
        assert_eq!(o.terms[0].rolls, [6], "out-of-range dice are clamped");
        assert_eq!(
            roll(&Expr::parse("8d6").unwrap(), fixed(&[2])).natural(),
            None
        );
    }

    #[test]
    fn sheet_values_become_expressions() {
        let damage: Expr = Roll {
            count: 1,
            sides: 8,
            bonus: 4,
        }
        .into();
        assert_eq!(damage.to_string(), "1d8 + 4");
        assert_eq!(damage.critical().to_string(), "2d8 + 4");
        assert_eq!(
            Expr::d20(6)
                .with_advantage(Advantage::Disadvantage)
                .to_string(),
            "2d20kl1 + 6"
        );
        assert!(Expr::d20(0).is_d20_test());
        let flat: Expr = Roll {
            count: 0,
            sides: 0,
            bonus: 3,
        }
        .into();
        assert_eq!(flat.to_string(), "3");
    }
}
