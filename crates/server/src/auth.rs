use std::sync::OnceLock;

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use beholder_core::{DisplayName, Password, UnixMillis, User, UserId, Username};
use beholder_protocol::SessionToken;
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::campaigns::blob_id;
use crate::db::DbError;

pub const SESSION_TTL_MS: i64 = 30 * 24 * 3600 * 1000;

pub const ROTATE_AFTER_MS: i64 = 24 * 3600 * 1000;

fn argon2() -> Result<Argon2<'static>, AuthError> {
    let params = Params::new(19 * 1024, 2, 1, None).map_err(|_| AuthError::Hash)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("password hashing failed")]
    Hash,
    #[error("random source failed")]
    Random,
}

pub fn hash_password(password: &Password) -> Result<String, AuthError> {
    let salt: [u8; 16] = crate::random_bytes().map_err(|_| AuthError::Random)?;
    let hash: PasswordHash = argon2()?
        .hash_password_with_salt(password.expose().as_bytes(), &salt)
        .map_err(|_| AuthError::Hash)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &Password, phc: Option<&str>) -> Result<bool, AuthError> {
    static DUMMY: OnceLock<Option<String>> = OnceLock::new();
    let Some(phc) = phc else {
        let dummy = DUMMY.get_or_init(|| {
            Password::new("dummy-password-for-timing".into())
                .ok()
                .and_then(|p| hash_password(&p).ok())
        });
        if let Some(d) = dummy {
            let _ = verify(password, d);
        }
        return Ok(false);
    };
    verify(password, phc)
}

fn verify(password: &Password, phc: &str) -> Result<bool, AuthError> {
    let hash = PasswordHash::new(phc).map_err(|_| AuthError::Hash)?;
    Ok(argon2()?
        .verify_password(password.expose().as_bytes(), &hash)
        .is_ok())
}

pub fn create_user(
    conn: &mut Connection,
    username: &Username,
    display_name: &DisplayName,
    phc: &str,
    now: UnixMillis,
) -> Result<Option<User>, DbError> {
    let id = UserId(uuid::Uuid::now_v7());
    let tx = conn.transaction()?;
    let inserted = tx.execute(
        "INSERT INTO users (id, username, display_name, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            id.as_bytes(),
            username.as_str(),
            display_name.as_str(),
            now.0
        ],
    );
    match inserted {
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == ErrorCode::ConstraintViolation => {
            return Ok(None);
        }
        r => r?,
    };
    tx.execute(
        "INSERT INTO password_credentials (user_id, phc, updated_at) VALUES (?1, ?2, ?3)",
        params![id.as_bytes(), phc, now.0],
    )?;
    tx.commit()?;
    Ok(Some(User {
        id,
        username: username.clone(),
        display_name: display_name.clone(),
    }))
}

pub fn credentials(
    conn: &Connection,
    username: &Username,
) -> Result<Option<(User, String)>, DbError> {
    conn.query_row(
        "SELECT u.id, u.username, u.display_name, p.phc
         FROM users u JOIN password_credentials p ON p.user_id = u.id
         WHERE u.username = ?1",
        [username.as_str()],
        |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        },
    )
    .optional()?
    .map(|(id, name, display, phc)| Ok((user_from_row(&id, name, &display)?, phc)))
    .transpose()
}

pub fn user_by_id(conn: &Connection, id: UserId) -> Result<Option<User>, DbError> {
    conn.query_row(
        "SELECT username, display_name FROM users WHERE id = ?1",
        [id.as_bytes()],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )
    .optional()?
    .map(|(name, display)| user_from_row(id.as_bytes(), name, &display))
    .transpose()
}

pub fn create_session(
    conn: &Connection,
    user: UserId,
    now: UnixMillis,
) -> Result<SessionToken, AuthError> {
    let token = SessionToken(crate::random_bytes().map_err(|_| AuthError::Random)?);
    conn.execute(
        "INSERT INTO sessions (token_hash, user_id, created_at, last_used_at, expires_at)
         VALUES (?1, ?2, ?3, ?3, ?4)",
        params![
            token_hash(&token),
            user.as_bytes(),
            now.0,
            now.0 + SESSION_TTL_MS
        ],
    )
    .map_err(DbError::from)?;
    Ok(token)
}

#[derive(Debug)]
pub struct Session {
    pub user: User,
    pub rotated: Option<SessionToken>,
}

pub fn validate_session(
    conn: &mut Connection,
    token: &SessionToken,
    now: UnixMillis,
    rotate: bool,
) -> Result<Option<Session>, AuthError> {
    let hash = token_hash(token);
    let tx = conn.transaction().map_err(DbError::from)?;
    let row = tx
        .query_row(
            "SELECT s.user_id, s.created_at, s.expires_at, s.replaced_by, u.username, u.display_name
             FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ?1",
            [hash.as_slice()],
            |r| {
                Ok((
                    r.get::<_, Vec<u8>>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<Vec<u8>>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(DbError::from)?;
    let Some((user_id, created, expires, replaced_by, username, display)) = row else {
        return Ok(None);
    };
    if expires <= now.0 {
        tx.execute(
            "DELETE FROM sessions WHERE token_hash = ?1",
            [hash.as_slice()],
        )
        .map_err(DbError::from)?;
        tx.commit().map_err(DbError::from)?;
        return Ok(None);
    }
    let user = user_from_row(&user_id, username, &display)?;
    tx.execute(
        "DELETE FROM sessions WHERE replaced_by = ?1",
        [hash.as_slice()],
    )
    .map_err(DbError::from)?;
    tx.execute(
        "UPDATE sessions SET last_used_at = ?2, expires_at = ?3 WHERE token_hash = ?1",
        params![hash.as_slice(), now.0, now.0 + SESSION_TTL_MS],
    )
    .map_err(DbError::from)?;
    let rotated = if rotate && (now.0 - created >= ROTATE_AFTER_MS || replaced_by.is_some()) {
        if let Some(old) = replaced_by {
            tx.execute(
                "DELETE FROM sessions WHERE token_hash = ?1",
                [old.as_slice()],
            )
            .map_err(DbError::from)?;
        }
        let new = create_session(&tx, user.id, now)?;
        tx.execute(
            "UPDATE sessions SET replaced_by = ?2 WHERE token_hash = ?1",
            params![hash.as_slice(), token_hash(&new).as_slice()],
        )
        .map_err(DbError::from)?;
        Some(new)
    } else {
        None
    };
    tx.commit().map_err(DbError::from)?;
    Ok(Some(Session { user, rotated }))
}

pub fn delete_session(conn: &Connection, token: &SessionToken) -> Result<(), DbError> {
    let hash = token_hash(token);
    conn.execute(
        "DELETE FROM sessions WHERE token_hash = ?1 OR replaced_by = ?1
            OR token_hash = (SELECT replaced_by FROM sessions WHERE token_hash = ?1)",
        [hash.as_slice()],
    )?;
    Ok(())
}

pub struct Account {
    pub username: String,
    pub display_name: String,
    pub created_at: UnixMillis,
    pub sessions: u32,
}

pub fn accounts(conn: &Connection) -> Result<Vec<Account>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT u.username, u.display_name, u.created_at,
                (SELECT COUNT(*) FROM sessions s WHERE s.user_id = u.id)
         FROM users u ORDER BY u.username COLLATE NOCASE",
    )?;
    stmt.query_map([], |r| {
        Ok(Account {
            username: r.get(0)?,
            display_name: r.get(1)?,
            created_at: UnixMillis(r.get(2)?),
            sessions: r.get(3)?,
        })
    })?
    .map(|row| Ok(row?))
    .collect()
}

pub fn reset_password(
    conn: &mut Connection,
    username: &str,
    phc: &str,
    now: UnixMillis,
) -> Result<bool, DbError> {
    let tx = conn.transaction()?;
    let Some(id): Option<Vec<u8>> = tx
        .query_row(
            "SELECT id FROM users WHERE username = ?1 COLLATE NOCASE",
            [username],
            |r| r.get(0),
        )
        .optional()?
    else {
        return Ok(false);
    };
    tx.execute(
        "INSERT INTO password_credentials (user_id, phc, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (user_id) DO UPDATE SET phc = excluded.phc, updated_at = excluded.updated_at",
        params![id, phc, now.0],
    )?;
    tx.execute("DELETE FROM sessions WHERE user_id = ?1", [&id])?;
    tx.commit()?;
    Ok(true)
}

pub fn random_password() -> Result<Password, AuthError> {
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzACDEFGHJKLMNPQRTUVWXY34679";
    let bytes: [u8; 16] = crate::random_bytes().map_err(|_| AuthError::Random)?;
    let text: String = bytes
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(*b) % ALPHABET.len()]))
        .collect();
    Password::new(text).map_err(|_| AuthError::Random)
}

pub fn purge_sessions(conn: &Connection, now: UnixMillis) -> Result<(), DbError> {
    conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [now.0])?;
    Ok(())
}

fn token_hash(token: &SessionToken) -> [u8; 32] {
    Sha256::digest(token.0).into()
}

fn user_from_row(id: &[u8], username: String, display: &str) -> Result<User, DbError> {
    Ok(User {
        id: UserId(blob_id(id)?),
        username: Username::new(username).map_err(|_| DbError::Corrupt("username"))?,
        display_name: DisplayName::new(display).map_err(|_| DbError::Corrupt("display_name"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn conn() -> Connection {
        let db = Db::open_in_memory().unwrap();
        db.into_connection()
    }

    fn user(conn: &mut Connection, name: &str) -> User {
        create_user(
            conn,
            &Username::new(name.into()).unwrap(),
            &DisplayName::new(name).unwrap(),
            "phc",
            UnixMillis(0),
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn admin_can_list_accounts_and_reset_a_password() {
        let mut c = conn();
        let alice = user(&mut c, "alice");
        user(&mut c, "Bob");
        create_session(&c, alice.id, UnixMillis(1)).unwrap();
        let list = accounts(&c).unwrap();
        let names: Vec<(&str, u32)> = list.iter().map(|a| (a.username.as_str(), a.sessions)).collect();
        assert_eq!(names, [("alice", 1), ("Bob", 0)]);
        let pw = random_password().unwrap();
        assert_eq!(pw.expose().len(), 16);
        let phc = hash_password(&pw).unwrap();
        assert!(reset_password(&mut c, "ALICE", &phc, UnixMillis(2)).unwrap());
        assert!(!reset_password(&mut c, "nobody", &phc, UnixMillis(2)).unwrap());
        let (_, stored) = credentials(&c, &Username::new("alice".into()).unwrap())
            .unwrap()
            .unwrap();
        assert!(verify_password(&pw, Some(&stored)).unwrap());
        assert_eq!(accounts(&c).unwrap()[0].sessions, 0, "the user is signed out everywhere");
    }

    #[test]
    fn password_hash_and_verify() {
        let pw = Password::new("correct horse battery".into()).unwrap();
        let phc = hash_password(&pw).unwrap();
        assert!(phc.starts_with("$argon2id$"));
        assert!(verify_password(&pw, Some(&phc)).unwrap());
        let wrong = Password::new("wrong password!".into()).unwrap();
        assert!(!verify_password(&wrong, Some(&phc)).unwrap());
        assert!(!verify_password(&pw, None).unwrap());
    }

    #[test]
    fn usernames_are_unique_case_insensitively() {
        let mut c = conn();
        user(&mut c, "Elise");
        let again = create_user(
            &mut c,
            &Username::new("elise".into()).unwrap(),
            &DisplayName::new("x").unwrap(),
            "phc",
            UnixMillis(0),
        )
        .unwrap();
        assert!(again.is_none());
        let (u, phc) = credentials(&c, &Username::new("ELISE".into()).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(u.username.as_str(), "Elise");
        assert_eq!(phc, "phc");
    }

    #[test]
    fn session_rotation_keeps_old_token_until_new_one_is_used() {
        let mut c = conn();
        let u = user(&mut c, "bob");
        let t0 = UnixMillis(1_000);
        let old = create_session(&c, u.id, t0).unwrap();

        let fresh = validate_session(&mut c, &old, t0, true).unwrap().unwrap();
        assert!(fresh.rotated.is_none(), "young tokens aren't rotated");

        let later = UnixMillis(t0.0 + ROTATE_AFTER_MS);
        let s = validate_session(&mut c, &old, later, true)
            .unwrap()
            .unwrap();
        let new1 = s.rotated.unwrap();
        let s = validate_session(&mut c, &old, later, true)
            .unwrap()
            .unwrap();
        let new2 = s.rotated.unwrap();
        assert!(
            validate_session(&mut c, &new1, later, true)
                .unwrap()
                .is_none()
        );
        let s = validate_session(&mut c, &new2, later, true)
            .unwrap()
            .unwrap();
        assert!(s.rotated.is_none());
        assert_eq!(s.user.id, u.id);
        assert!(
            validate_session(&mut c, &old, later, true)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn sessions_expire_and_logout_removes_both_tokens() {
        let mut c = conn();
        let u = user(&mut c, "carol");
        let t = create_session(&c, u.id, UnixMillis(0)).unwrap();
        assert!(
            validate_session(&mut c, &t, UnixMillis(SESSION_TTL_MS), true)
                .unwrap()
                .is_none()
        );

        let t = create_session(&c, u.id, UnixMillis(0)).unwrap();
        let new = validate_session(&mut c, &t, UnixMillis(ROTATE_AFTER_MS), true)
            .unwrap()
            .unwrap()
            .rotated
            .unwrap();
        delete_session(&c, &t).unwrap();
        assert!(
            validate_session(&mut c, &t, UnixMillis(ROTATE_AFTER_MS), true)
                .unwrap()
                .is_none()
        );
        assert!(
            validate_session(&mut c, &new, UnixMillis(ROTATE_AFTER_MS), true)
                .unwrap()
                .is_none()
        );
    }
}
