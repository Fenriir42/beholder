//! SQLite access: one connection, used from the blocking pool (ADR-0002).

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

/// Migrations in order; `PRAGMA user_version` records how many have run.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

/// A database failure.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// SQLite reported an error.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A stored blob didn't decode; the database is from a newer server or corrupt.
    #[error("corrupt row: {0}")]
    Corrupt(&'static str),
    /// The blocking task panicked or was cancelled.
    #[error("database task failed")]
    Task,
}

impl From<postcard::Error> for DbError {
    fn from(_: postcard::Error) -> Self {
        Self::Corrupt("postcard")
    }
}

/// Shared handle to the server database.
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    /// Opens (creating if needed) and migrates the database at `path`.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        Self::init(Connection::open(path)?)
    }

    /// A fresh in-memory database, for tests.
    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, DbError> {
        // WAL for crash safety without blocking readers; NORMAL sync is safe under WAL.
        // A 4 MiB page cache and no mmap keep RSS bounded on the 1 GB VPS.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA cache_size = -4096;
             PRAGMA mmap_size = 0;
             PRAGMA busy_timeout = 5000;",
        )?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// The raw connection, for synchronous unit tests.
    #[cfg(test)]
    pub fn into_connection(self) -> Connection {
        match Arc::into_inner(self.conn) {
            Some(m) => m.into_inner().unwrap(),
            None => panic!("Db still shared"),
        }
    }

    /// Runs `f` with the connection on the blocking pool. Calls are serialized,
    /// which is what makes seq assignment and fan-out ordering race-free.
    pub async fn call<F, R, E>(&self, f: F) -> Result<R, E>
    where
        F: FnOnce(&mut Connection) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
        E: From<DbError> + Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            // A panic mid-transaction rolls back on unwind, so the connection is still usable.
            let mut guard = conn.lock().unwrap_or_else(|p| p.into_inner());
            f(&mut guard)
        })
        .await
        .map_err(|_| E::from(DbError::Task))?
    }
}

fn migrate(conn: &mut Connection) -> Result<(), DbError> {
    let done: usize = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(done) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i + 1)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_run_once_and_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        drop(Db::open(&path).unwrap());
        // Reopening must not re-run migrations (CREATE TABLE would fail).
        let db = Db::open(&path).unwrap();
        let conn = db.conn.lock().unwrap();
        let v: usize = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len());
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }
}
