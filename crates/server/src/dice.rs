use beholder_core::dice::{DiceError, DiceOp, LOG_SIZE, RollRecord, roll};
use beholder_core::{
    CampaignEvent, CampaignId, CommandId, Seq, UnixMillis, UserId, ValidationError,
};
use beholder_protocol::RejectReason;
use rusqlite::{Connection, Transaction, params};

use crate::db::DbError;

fn die(sides: u16) -> Result<u16, DbError> {
    let sides = u32::from(sides);
    // Rejection sampling: drawing below a multiple of `sides` keeps every face equally likely.
    let zone = u32::MAX - (u32::MAX % sides);
    loop {
        let v = u32::from_le_bytes(crate::random_bytes::<4>()?);
        if v < zone {
            return Ok(u16::try_from(v % sides + 1).unwrap_or(1));
        }
    }
}

pub(crate) fn decide(
    actor: UserId,
    cmd_id: CommandId,
    op: &DiceOp,
    now: UnixMillis,
) -> Result<Result<CampaignEvent, RejectReason>, DbError> {
    if let Err(e) = op.check() {
        return Ok(Err(RejectReason::Invalid(match e {
            DiceError::Syntax => ValidationError::Character,
            DiceError::TooBig => ValidationError::Length,
        })));
    }
    let (expr, context, character, visibility, physical, results) = match op {
        DiceOp::Roll {
            expr,
            context,
            character,
            visibility,
        } => {
            let mut results = Vec::with_capacity(expr.dice_count());
            for t in &expr.terms {
                for _ in 0..t.count {
                    results.push(die(t.sides)?);
                }
            }
            (expr, context, character, visibility, false, results)
        }
        DiceOp::Physical {
            expr,
            results,
            context,
            character,
            visibility,
        } => (expr, context, character, visibility, true, results.clone()),
    };
    let mut next = results.into_iter();
    let outcome = roll(expr, |_| next.next().unwrap_or(1));
    Ok(Ok(CampaignEvent::Rolled {
        roll: Box::new(RollRecord {
            id: cmd_id,
            roller: actor,
            at: now,
            character: character.clone(),
            context: context.clone(),
            expr: expr.clone(),
            outcome,
            visibility: *visibility,
            physical,
        }),
    }))
}

pub(crate) fn project(
    tx: &Transaction<'_>,
    campaign_id: CampaignId,
    seq: Seq,
    roll: &RollRecord,
) -> Result<(), DbError> {
    tx.execute(
        "INSERT INTO rolls (campaign_id, seq, body) VALUES (?1, ?2, ?3)",
        params![campaign_id.as_bytes(), seq.0, postcard::to_stdvec(roll)?],
    )?;
    tx.execute(
        "DELETE FROM rolls WHERE campaign_id = ?1 AND seq NOT IN
         (SELECT seq FROM rolls WHERE campaign_id = ?1 ORDER BY seq DESC LIMIT ?2)",
        params![campaign_id.as_bytes(), LOG_SIZE as i64],
    )?;
    Ok(())
}

pub fn campaign_rolls(
    conn: &Connection,
    campaign_id: CampaignId,
) -> Result<Vec<RollRecord>, DbError> {
    let mut stmt =
        conn.prepare_cached("SELECT body FROM rolls WHERE campaign_id = ?1 ORDER BY seq")?;
    stmt.query_map([campaign_id.as_bytes()], |r| r.get::<_, Vec<u8>>(0))?
        .map(|row| Ok(postcard::from_bytes(&row?)?))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dice_stay_within_their_faces() {
        let mut seen = [false; 6];
        for _ in 0..600 {
            let v = die(6).unwrap();
            assert!((1..=6).contains(&v));
            seen[usize::from(v - 1)] = true;
        }
        assert!(seen.iter().all(|s| *s), "every face comes up in 600 rolls");
    }
}
