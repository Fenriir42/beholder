use std::collections::HashSet;
use std::io::Read;

use beholder_core::compendium::{Kind, RulesVersion};
use beholder_core::library::{LibraryDraft, LibraryEntry, LibraryOp, MAX_BATCH};
use beholder_core::{CommandId, ImportId, LibraryId, UserId};
use beholder_dnd_data::{FILES, convert, each_record, numbers_as_text, to_draft};
use beholder_protocol::ServerMsg;
use rusqlite::{Connection, OptionalExtension, params};

use crate::db::DbError;
use crate::permissions::Outgoing;
use crate::{AppState, library, now};

pub const SETUP_ACTOR: UserId = UserId(uuid::Uuid::nil());

const DONE_KEY: &str = "setup_import";

const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

type Key = (Kind, String, Option<String>, RulesVersion);

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("database: {0}")]
    Db(#[from] DbError),
    #[error("archive: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON in {0}: {1}")]
    Json(String, serde_json::Error),
    #[error("download: {0}")]
    Download(String),
    #[error("batch refused: {0}")]
    Refused(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub files: usize,
    pub entries: usize,
    pub skipped: usize,
}

pub fn archive_url(repo: &str, git_ref: &str) -> String {
    let repo = repo.trim().trim_end_matches('/');
    if repo.ends_with(".tar.gz") || repo.ends_with(".tgz") {
        return repo.to_owned();
    }
    let repo = repo.trim_end_matches(".git");
    if repo.contains("gitlab") {
        let name = repo.rsplit('/').next().unwrap_or(repo);
        format!("{repo}/-/archive/{git_ref}/{name}-{git_ref}.tar.gz")
    } else {
        format!("{repo}/archive/{git_ref}.tar.gz")
    }
}

fn key(kind: Kind, name: &str, book: Option<&str>, rules: RulesVersion) -> Key {
    (
        kind,
        name.to_lowercase(),
        book.map(str::to_lowercase),
        rules,
    )
}

fn existing_keys(conn: &Connection) -> Result<HashSet<Key>, DbError> {
    let mut stmt = conn.prepare("SELECT body FROM library_entries WHERE deleted = 0")?;
    let mut rows = stmt.query([])?;
    let mut out = HashSet::new();
    while let Some(row) = rows.next()? {
        let e: LibraryEntry = postcard::from_bytes(&row.get::<_, Vec<u8>>(0)?)?;
        out.insert(key(
            e.facets.kind(),
            e.name.as_str(),
            e.book.as_ref().map(|b| b.as_str()),
            e.rules,
        ));
    }
    Ok(out)
}

pub fn import_archive(
    reader: impl Read,
    mut seen: HashSet<Key>,
    mut write: impl FnMut(Vec<(LibraryDraft, u32)>) -> Result<(), SetupError>,
) -> Result<Summary, SetupError> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    let mut summary = Summary::default();
    let mut batch: Vec<(LibraryDraft, u32)> = Vec::new();
    for file in archive.entries()? {
        let file = file?;
        let path = file.path()?.into_owned();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_lowercase)
            .unwrap_or_default();
        if path.extension().and_then(|e| e.to_str()) != Some("json")
            || !FILES.contains(&stem.as_str())
            || file.size() > MAX_FILE_BYTES
        {
            continue;
        }
        summary.files += 1;
        let mut failed = None;
        each_record(file, |mut v| {
            if failed.is_some() {
                return;
            }
            numbers_as_text(&mut v);
            for e in convert(&stem, &v) {
                let Some(d) = to_draft(&e) else {
                    summary.skipped += 1;
                    continue;
                };
                let draft = LibraryDraft {
                    id: LibraryId(uuid::Uuid::now_v7()),
                    name: d.name,
                    facets: d.facets,
                    text: d.text,
                    book: d.book,
                    rules: d.rules,
                };
                let k = key(
                    draft.facets.kind(),
                    draft.name.as_str(),
                    draft.book.as_ref().map(|b| b.as_str()),
                    draft.rules,
                );
                if draft.check_facets().is_err() || !seen.insert(k) {
                    summary.skipped += 1;
                    continue;
                }
                batch.push((draft, 0));
                summary.entries += 1;
                if batch.len() == MAX_BATCH
                    && let Err(e) = write(std::mem::take(&mut batch))
                {
                    failed = Some(e);
                    return;
                }
            }
        })
        .map_err(|e| SetupError::Json(path.display().to_string(), e))?;
        if let Some(e) = failed {
            return Err(e);
        }
    }
    if !batch.is_empty() {
        write(batch)?;
    }
    Ok(summary)
}

fn write_batch(
    conn: &mut Connection,
    import: ImportId,
    items: Vec<(LibraryDraft, u32)>,
) -> Result<Result<u64, String>, DbError> {
    let tx = conn.transaction()?;
    let op = LibraryOp::ImportMany { import, items };
    let decided = library::decide(
        &tx,
        SETUP_ACTOR,
        CommandId(uuid::Uuid::now_v7()),
        &op,
        now(),
    )?;
    tx.commit()?;
    Ok(decided.map_err(|r| format!("{r:?}")))
}

fn done(conn: &Connection) -> Result<bool, DbError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM server_meta WHERE key = ?1",
            [DONE_KEY],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn download(url: &str) -> Result<impl Read + use<>, SetupError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(std::time::Duration::from_secs(30)))
        .build()
        .into();
    let response = agent
        .get(url)
        .call()
        .map_err(|e| SetupError::Download(e.to_string()))?;
    if response.status() != 200 {
        return Err(SetupError::Download(format!("HTTP {}", response.status())));
    }
    Ok(response.into_body().into_reader())
}

pub async fn run(state: AppState, repo: String, git_ref: String) {
    match state.db.call(|c| done(c)).await {
        Ok(false) => {}
        Ok(true) => return,
        Err(e) => {
            tracing::warn!("setup import: {e}");
            return;
        }
    }
    let url = archive_url(&repo, &git_ref);
    tracing::info!(%url, "setup import: downloading");
    let handle = tokio::runtime::Handle::current();
    let worker = std::sync::Arc::clone(&state);
    let result = tokio::task::spawn_blocking(move || {
        let seen = handle.block_on(worker.db.call(|c| existing_keys(c)))?;
        let import = ImportId(uuid::Uuid::now_v7());
        let summary = import_archive(download(&url)?, seen, |items| {
            let seq = handle
                .block_on(worker.db.call(move |c| write_batch(c, import, items)))?
                .map_err(SetupError::Refused)?;
            if let Some(out) = Outgoing::control(ServerMsg::LibraryChanged { seq }) {
                worker.hub.broadcast_all(&out);
            }
            Ok(())
        })?;
        let mark = format!("{url} {import}");
        handle.block_on(worker.db.call(move |c| {
            c.execute(
                "INSERT OR REPLACE INTO server_meta (key, value) VALUES (?1, ?2)",
                params![DONE_KEY, mark],
            )
            .map_err(DbError::from)
        }))?;
        Ok::<_, SetupError>(summary)
    })
    .await;
    match result {
        Ok(Ok(s)) => tracing::info!(
            files = s.files,
            entries = s.entries,
            skipped = s.skipped,
            "setup import: done"
        ),
        Ok(Err(e)) => {
            tracing::warn!("setup import failed, it will be retried at the next start: {e}")
        }
        Err(e) => tracing::warn!("setup import stopped: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        for (path, body) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, path, body.as_bytes()).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn archive_urls() {
        assert_eq!(
            archive_url("https://github.com/nick-aschenbach/dnd-data.git/", "HEAD"),
            "https://github.com/nick-aschenbach/dnd-data/archive/HEAD.tar.gz"
        );
        assert_eq!(
            archive_url("https://gitlab.com/group/dnd", "main"),
            "https://gitlab.com/group/dnd/-/archive/main/dnd-main.tar.gz"
        );
        assert_eq!(
            archive_url("https://example.org/data.tar.gz", "HEAD"),
            "https://example.org/data.tar.gz"
        );
    }

    #[test]
    fn archive_imports_once_as_one_undoable_batch() {
        let spells = r#"[{"name":"Fireball","description":"3rd-Level Evocation Casting Time : 1 action","book":"Player's Handbook","properties":{"Category":"Spells","Level":3,"School":"evocation"}},
            {"name":"Fireball","description":"duplicate","book":"Player's Handbook","properties":{"Category":"Spells","Level":3,"School":"evocation"}}]"#;
        let monsters = r#"[{"name":"Goblin","description":"Actions Scimitar : Hit.","book":"Monster Manual","properties":{"Category":"Monsters","Size":"Small","Type":"humanoid","Challenge Rating":"1/4"}}]"#;
        let bytes = archive(&[
            ("dnd-data-main/README.md", "hello"),
            ("dnd-data-main/data/spells.json", spells),
            ("dnd-data-main/data/monsters.json", monsters),
            ("dnd-data-main/data/other.json", "not even json"),
        ]);
        let mut c = Db::open_in_memory().unwrap().into_connection();
        let import = ImportId(uuid::Uuid::now_v7());
        let seen = existing_keys(&c).unwrap();
        let summary = import_archive(&bytes[..], seen, |items| {
            write_batch(&mut c, import, items)
                .unwrap()
                .map_err(SetupError::Refused)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            summary,
            Summary {
                files: 2,
                entries: 2,
                skipped: 1
            }
        );
        let names: Vec<String> = library::bundle(&c)
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
        let listed = library::imports(&c).unwrap();
        assert_eq!((listed[0].id, listed[0].entries), (import, 2));
        let again = import_archive(&bytes[..], existing_keys(&c).unwrap(), |_| {
            panic!("nothing new to write")
        })
        .unwrap();
        assert_eq!((again.entries, again.skipped), (0, 3));
        assert!(!done(&c).unwrap());
    }
}
