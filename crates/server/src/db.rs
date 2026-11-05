use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_notes.sql"),
    include_str!("../migrations/0003_sources.sql"),
    include_str!("../migrations/0004_library.sql"),
    include_str!("../migrations/0005_imports.sql"),
    include_str!("../migrations/0006_server_meta.sql"),
    include_str!("../migrations/0007_characters.sql"),
    include_str!("../migrations/0008_rolls.sql"),
    include_str!("../migrations/0009_encounters.sql"),
];

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("corrupt row: {0}")]
    Corrupt(&'static str),
    #[error("database task failed")]
    Task,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl From<postcard::Error> for DbError {
    fn from(_: postcard::Error) -> Self {
        Self::Corrupt("postcard")
    }
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, DbError> {
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

    #[cfg(test)]
    pub fn into_connection(self) -> Connection {
        match Arc::into_inner(self.conn) {
            Some(m) => m.into_inner().unwrap(),
            None => panic!("Db still shared"),
        }
    }

    pub async fn call<F, R, E>(&self, f: F) -> Result<R, E>
    where
        F: FnOnce(&mut Connection) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
        E: From<DbError> + Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
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

pub fn backup(
    conn: &Connection,
    dir: &Path,
    stamp: u64,
    keep: usize,
) -> Result<std::path::PathBuf, DbError> {
    std::fs::create_dir_all(dir)?;
    let target = dir.join(format!("beholder-{stamp:020}.db"));
    let path = target.to_str().ok_or(DbError::Corrupt("backup path"))?;
    conn.execute("VACUUM INTO ?1", [path])?;
    let mut old: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("beholder-") && n.ends_with(".db"))
        })
        .collect();
    old.sort();
    let extra = old.len().saturating_sub(keep.max(1));
    for p in old.drain(..extra) {
        std::fs::remove_file(p)?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_are_consistent_copies_and_old_ones_are_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("beholder.db")).unwrap();
        let conn = db.conn.lock().unwrap();
        let backups = dir.path().join("backups");
        for stamp in 1..=4 {
            backup(&conn, &backups, stamp, 3).unwrap();
        }
        let mut names: Vec<String> = std::fs::read_dir(&backups)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names.len(), 3, "the oldest backup is pruned");
        assert!(names[0].ends_with("02.db"));
        let copy = Connection::open(backups.join(&names[2])).unwrap();
        let v: usize = copy
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len(), "the copy is a full database");
    }

    #[test]
    fn migrations_run_once_and_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        drop(Db::open(&path).unwrap());
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
