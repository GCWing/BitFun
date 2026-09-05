//! Rebuildable SQLite FTS index for presentation-safe persisted session facts.

use openbitfun_product_domains::product_search::{
    SessionContentSearchResponse, SessionSearchHit, SessionSearchHitKind, SessionSearchMatchField,
    SessionSearchSessionDocument, SessionSearchTurnDocument,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const SEARCH_INDEX_FILE_NAME: &str = "product-search-v1.sqlite";
const SEARCH_INDEX_SCHEMA_VERSION: i64 = 1;
const MAX_SNIPPET_CHARS: usize = 220;

pub type SessionSearchIndexResult<T> = Result<T, SessionSearchIndexError>;

#[derive(Debug, thiserror::Error)]
pub enum SessionSearchIndexError {
    #[error("Invalid session search document: {0}")]
    InvalidDocument(String),
    #[error(
        "Unsupported session search index schema version {found}; expected at most {expected}"
    )]
    UnsupportedSchema { found: i64, expected: i64 },
    #[error("Session search index worker failed: {0}")]
    Worker(String),
    #[error("Session search index backend failed: {0}")]
    Backend(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionSearchIndexReconciliation {
    pub stale_session_ids: Vec<String>,
    pub removed_session_count: usize,
}

/// One versioned search index beneath an already-resolved sessions root.
///
/// The database is always disposable. Authoritative session JSON remains the
/// only source of truth and callers reconcile before every search boundary.
#[derive(Debug, Clone)]
pub struct SessionSearchSqliteIndex {
    path: PathBuf,
}

impl SessionSearchSqliteIndex {
    pub fn new(sessions_root: impl Into<PathBuf>) -> Self {
        Self {
            path: sessions_root.into().join(SEARCH_INDEX_FILE_NAME),
        }
    }

    pub fn from_index_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    async fn execute<R: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> SessionSearchIndexResult<R> + Send + 'static,
    ) -> SessionSearchIndexResult<R> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = open_connection(&path)?;
            operation(&mut connection)
        })
        .await
        .map_err(|error| SessionSearchIndexError::Worker(error.to_string()))?
    }

    /// Synchronizes lightweight metadata and returns sessions whose visible
    /// transcript documents must be rebuilt from authoritative storage.
    pub async fn reconcile_sessions(
        &self,
        sessions: Vec<SessionSearchSessionDocument>,
    ) -> SessionSearchIndexResult<SessionSearchIndexReconciliation> {
        validate_session_documents(&sessions)?;
        self.execute(move |connection| reconcile_sessions(connection, &sessions))
            .await
    }

    /// Atomically replaces every visible transcript row for one source revision.
    pub async fn replace_session_turns(
        &self,
        session_id: impl Into<String>,
        source_revision: impl Into<String>,
        turns: Vec<SessionSearchTurnDocument>,
    ) -> SessionSearchIndexResult<()> {
        let session_id = session_id.into();
        let source_revision = source_revision.into();
        validate_turn_documents(&session_id, &turns)?;
        self.execute(move |connection| {
            replace_session_turns(connection, &session_id, &source_revision, &turns)
        })
        .await
    }

    pub async fn remove_session(
        &self,
        session_id: impl Into<String>,
    ) -> SessionSearchIndexResult<()> {
        let session_id = session_id.into();
        self.execute(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(backend_error)?;
            remove_session_rows(&transaction, &session_id)?;
            transaction.commit().map_err(backend_error)
        })
        .await
    }

    /// Marks transcript rows stale after any authoritative Turn mutation.
    /// No database is created until the search feature has first been used.
    pub async fn invalidate_session_if_present(
        &self,
        session_id: impl Into<String>,
    ) -> SessionSearchIndexResult<()> {
        if !self.path.exists() {
            return Ok(());
        }
        let session_id = session_id.into();
        self.execute(move |connection| {
            connection
                .execute(
                    "UPDATE sessions
                     SET source_revision = 'invalidated:' || lower(hex(randomblob(16))),
                         indexed_revision = NULL
                     WHERE session_id = ?1",
                    params![session_id],
                )
                .map(|_| ())
                .map_err(backend_error)
        })
        .await
    }

    pub async fn search(
        &self,
        query: impl Into<String>,
        limit: usize,
        include_archived: bool,
    ) -> SessionSearchIndexResult<SessionContentSearchResponse> {
        let query = query.into();
        let query = query.trim().to_string();
        if query.is_empty() {
            return Ok(SessionContentSearchResponse::default());
        }
        let limit = limit.clamp(1, 100);
        self.execute(move |connection| search_index(connection, &query, limit, include_archived))
            .await
    }
}

fn validate_session_documents(
    sessions: &[SessionSearchSessionDocument],
) -> SessionSearchIndexResult<()> {
    let mut ids = HashSet::with_capacity(sessions.len());
    for session in sessions {
        if session.session_id.trim().is_empty() {
            return Err(SessionSearchIndexError::InvalidDocument(
                "session ID must be non-empty".to_string(),
            ));
        }
        if session.source_revision.trim().is_empty() {
            return Err(SessionSearchIndexError::InvalidDocument(format!(
                "source revision must be non-empty for session {}",
                session.session_id
            )));
        }
        if !ids.insert(session.session_id.as_str()) {
            return Err(SessionSearchIndexError::InvalidDocument(format!(
                "duplicate session ID {}",
                session.session_id
            )));
        }
    }
    Ok(())
}

fn validate_turn_documents(
    session_id: &str,
    turns: &[SessionSearchTurnDocument],
) -> SessionSearchIndexResult<()> {
    if session_id.trim().is_empty() {
        return Err(SessionSearchIndexError::InvalidDocument(
            "session ID must be non-empty".to_string(),
        ));
    }
    let mut ids = HashSet::with_capacity(turns.len());
    for turn in turns {
        if turn.session_id != session_id {
            return Err(SessionSearchIndexError::InvalidDocument(format!(
                "turn {} belongs to session {}, not {}",
                turn.turn_id, turn.session_id, session_id
            )));
        }
        if turn.turn_id.trim().is_empty() || !ids.insert(turn.turn_id.as_str()) {
            return Err(SessionSearchIndexError::InvalidDocument(format!(
                "turn IDs must be non-empty and unique in session {session_id}"
            )));
        }
    }
    Ok(())
}

fn open_connection(path: &Path) -> SessionSearchIndexResult<Connection> {
    let parent = path.parent().ok_or_else(|| {
        SessionSearchIndexError::Backend(format!("database path has no parent: {}", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(backend_error)?;
    let mut connection = Connection::open(path).map_err(backend_error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(backend_error)?;
    connection
        .execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            PRAGMA synchronous = NORMAL;
            "#,
        )
        .map_err(backend_error)?;
    initialize_schema(&mut connection)?;
    Ok(connection)
}

fn initialize_schema(connection: &mut Connection) -> SessionSearchIndexResult<()> {
    let schema_version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(backend_error)?;
    if schema_version == SEARCH_INDEX_SCHEMA_VERSION {
        return Ok(());
    }
    if schema_version != 0 {
        return Err(SessionSearchIndexError::UnsupportedSchema {
            found: schema_version,
            expected: SEARCH_INDEX_SCHEMA_VERSION,
        });
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(backend_error)?;
    transaction
        .execute_batch(
            r#"
            CREATE TABLE sessions (
                session_id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                tags TEXT NOT NULL,
                archived INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                source_revision TEXT NOT NULL,
                indexed_revision TEXT
            );

            CREATE TABLE turns (
                row_key TEXT PRIMARY KEY NOT NULL,
                session_id TEXT NOT NULL,
                turn_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                user_text TEXT NOT NULL,
                assistant_text TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                UNIQUE (session_id, turn_id),
                FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
            );

            CREATE INDEX idx_turns_session_index
                ON turns (session_id, turn_index);
            CREATE INDEX idx_sessions_recency
                ON sessions (archived, updated_at_ms DESC);

            CREATE VIRTUAL TABLE session_fts USING fts5(
                session_id UNINDEXED,
                title,
                tags,
                tokenize = 'trigram'
            );
            CREATE VIRTUAL TABLE turn_fts USING fts5(
                row_key UNINDEXED,
                session_id UNINDEXED,
                user_text,
                assistant_text,
                tokenize = 'trigram'
            );
            "#,
        )
        .map_err(backend_error)?;
    transaction
        .pragma_update(None, "user_version", SEARCH_INDEX_SCHEMA_VERSION)
        .map_err(backend_error)?;
    transaction.commit().map_err(backend_error)
}

fn reconcile_sessions(
    connection: &mut Connection,
    sessions: &[SessionSearchSessionDocument],
) -> SessionSearchIndexResult<SessionSearchIndexReconciliation> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(backend_error)?;

    let existing = {
        let mut statement = transaction
            .prepare(
                "SELECT session_id, title, tags, archived, updated_at_ms,
                        source_revision, indexed_revision,
                        EXISTS(SELECT 1 FROM session_fts f WHERE f.session_id = sessions.session_id)
                 FROM sessions",
            )
            .map_err(backend_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, bool>(7)?,
                ))
            })
            .map_err(backend_error)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(backend_error)?
            .into_iter()
            .map(
                |(id, title, tags, archived, updated_at_ms, source, indexed, fts_present)| {
                    (
                        id,
                        (
                            title,
                            tags,
                            archived,
                            updated_at_ms,
                            source,
                            indexed,
                            fts_present,
                        ),
                    )
                },
            )
            .collect::<HashMap<_, _>>()
    };

    let expected_ids = sessions
        .iter()
        .map(|session| session.session_id.as_str())
        .collect::<HashSet<_>>();
    let mut removed_session_count = 0;
    for session_id in existing.keys() {
        if !expected_ids.contains(session_id.as_str()) {
            remove_session_rows(&transaction, session_id)?;
            removed_session_count += 1;
        }
    }

    let mut stale_session_ids = Vec::new();
    for session in sessions {
        let tags = session.tags.join(" \u{2022} ");
        let existing_session = existing.get(&session.session_id);
        let metadata_changed = existing_session.is_none_or(
            |(title, existing_tags, archived, updated_at_ms, source, _, _)| {
                title != &session.title
                    || existing_tags != &tags
                    || *archived != session.archived
                    || *updated_at_ms != session.updated_at_ms
                    || source != &session.source_revision
            },
        );
        let fts_changed =
            existing_session.is_none_or(|(title, existing_tags, _, _, _, _, fts_present)| {
                title != &session.title || existing_tags != &tags || !fts_present
            });

        if existing_session.is_none() {
            transaction
                .execute(
                    "INSERT INTO sessions (
                        session_id, title, tags, archived, updated_at_ms, source_revision, indexed_revision
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                    params![
                        session.session_id,
                        session.title,
                        tags,
                        session.archived,
                        session.updated_at_ms,
                        session.source_revision,
                    ],
                )
                .map_err(backend_error)?;
        } else if metadata_changed {
            transaction
                .execute(
                    "UPDATE sessions
                     SET title = ?2, tags = ?3, archived = ?4,
                         updated_at_ms = ?5, source_revision = ?6
                     WHERE session_id = ?1",
                    params![
                        session.session_id,
                        session.title,
                        tags,
                        session.archived,
                        session.updated_at_ms,
                        session.source_revision,
                    ],
                )
                .map_err(backend_error)?;
        }

        if fts_changed {
            transaction
                .execute(
                    "DELETE FROM session_fts WHERE session_id = ?1",
                    params![session.session_id],
                )
                .map_err(backend_error)?;
            transaction
                .execute(
                    "INSERT INTO session_fts (session_id, title, tags) VALUES (?1, ?2, ?3)",
                    params![session.session_id, session.title, tags],
                )
                .map_err(backend_error)?;
        }

        let indexed_revision =
            existing_session.and_then(|(_, _, _, _, _, indexed, _)| indexed.as_deref());
        if indexed_revision != Some(session.source_revision.as_str()) {
            stale_session_ids.push(session.session_id.clone());
        }
    }

    stale_session_ids.sort();
    transaction.commit().map_err(backend_error)?;
    Ok(SessionSearchIndexReconciliation {
        stale_session_ids,
        removed_session_count,
    })
}

fn replace_session_turns(
    connection: &mut Connection,
    session_id: &str,
    source_revision: &str,
    turns: &[SessionSearchTurnDocument],
) -> SessionSearchIndexResult<()> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(backend_error)?;
    let current_revision = transaction
        .query_row(
            "SELECT source_revision FROM sessions WHERE session_id = ?1",
            params![session_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(backend_error)?
        .ok_or_else(|| {
            SessionSearchIndexError::InvalidDocument(format!(
                "session {session_id} must be reconciled before indexing turns"
            ))
        })?;
    if current_revision != source_revision {
        return Err(SessionSearchIndexError::InvalidDocument(format!(
            "source revision changed while indexing session {session_id}"
        )));
    }

    transaction
        .execute(
            "DELETE FROM turn_fts WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(backend_error)?;
    transaction
        .execute(
            "DELETE FROM turns WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(backend_error)?;

    for turn in turns {
        let row_key = format!("{}\u{001f}{}", session_id, turn.turn_id);
        transaction
            .execute(
                "INSERT INTO turns (
                    row_key, session_id, turn_id, turn_index, user_text, assistant_text, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    row_key,
                    session_id,
                    turn.turn_id,
                    turn.turn_index,
                    turn.user_text,
                    turn.assistant_text,
                    turn.updated_at_ms,
                ],
            )
            .map_err(backend_error)?;
        transaction
            .execute(
                "INSERT INTO turn_fts (
                    row_key, session_id, user_text, assistant_text
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![row_key, session_id, turn.user_text, turn.assistant_text],
            )
            .map_err(backend_error)?;
    }

    transaction
        .execute(
            "UPDATE sessions SET indexed_revision = ?2
             WHERE session_id = ?1 AND source_revision = ?2",
            params![session_id, source_revision],
        )
        .map_err(backend_error)?;
    transaction.commit().map_err(backend_error)
}

fn remove_session_rows(
    transaction: &Transaction<'_>,
    session_id: &str,
) -> SessionSearchIndexResult<()> {
    transaction
        .execute(
            "DELETE FROM turn_fts WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(backend_error)?;
    transaction
        .execute(
            "DELETE FROM session_fts WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(backend_error)?;
    transaction
        .execute(
            "DELETE FROM sessions WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(backend_error)?;
    Ok(())
}

fn search_index(
    connection: &Connection,
    query: &str,
    limit: usize,
    include_archived: bool,
) -> SessionSearchIndexResult<SessionContentSearchResponse> {
    let fetch_limit = i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX);
    let use_fts = query
        .split_whitespace()
        .all(|term| term.chars().count() >= 3)
        && query.chars().count() >= 3;
    let mut hits = if use_fts {
        search_sessions_fts(connection, query, fetch_limit, include_archived)?
    } else {
        search_sessions_like(connection, query, fetch_limit, include_archived)?
    };
    let mut message_hits = if use_fts {
        search_turns_fts(connection, query, fetch_limit, include_archived)?
    } else {
        search_turns_like(connection, query, fetch_limit, include_archived)?
    };
    hits.append(&mut message_hits);
    hits.sort_by(compare_hits);
    let truncated = hits.len() > limit;
    hits.truncate(limit);
    Ok(SessionContentSearchResponse {
        hits,
        truncated,
        diagnostics: Vec::new(),
    })
}

fn search_sessions_fts(
    connection: &Connection,
    query: &str,
    limit: i64,
    include_archived: bool,
) -> SessionSearchIndexResult<Vec<SessionSearchHit>> {
    let expression = fts_expression(query);
    let mut statement = connection
        .prepare(
            "SELECT s.session_id, s.title, s.tags, s.archived, s.updated_at_ms
             FROM session_fts f
             JOIN sessions s ON s.session_id = f.session_id
             WHERE session_fts MATCH ?1 AND (?2 OR s.archived = 0)
             ORDER BY bm25(session_fts, 0.0, 8.0, 4.0), s.updated_at_ms DESC
             LIMIT ?3",
        )
        .map_err(backend_error)?;
    let rows = statement
        .query_map(params![expression, include_archived, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(backend_error)?;
    rows.map(|row| row.map_err(backend_error))
        .map(|row| row.map(|facts| session_hit(facts, query)))
        .collect()
}

fn search_sessions_like(
    connection: &Connection,
    query: &str,
    limit: i64,
    include_archived: bool,
) -> SessionSearchIndexResult<Vec<SessionSearchHit>> {
    let pattern = like_pattern(query);
    let mut statement = connection
        .prepare(
            "SELECT session_id, title, tags, archived, updated_at_ms
             FROM sessions
             WHERE (?2 OR archived = 0)
               AND (title LIKE ?1 ESCAPE '\\' OR tags LIKE ?1 ESCAPE '\\')
             ORDER BY updated_at_ms DESC
             LIMIT ?3",
        )
        .map_err(backend_error)?;
    let rows = statement
        .query_map(params![pattern, include_archived, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(backend_error)?;
    rows.map(|row| row.map_err(backend_error))
        .map(|row| row.map(|facts| session_hit(facts, query)))
        .collect()
}

fn session_hit(
    (session_id, title, tags, archived, updated_at_ms): (String, String, String, bool, i64),
    query: &str,
) -> SessionSearchHit {
    let (matched_field, snippet, score) = if matches_query(&title, query) {
        (
            SessionSearchMatchField::Title,
            build_snippet(&title, query),
            relevance_score(&title, query, 88.0, archived),
        )
    } else {
        (
            SessionSearchMatchField::Tags,
            build_snippet(&tags, query),
            relevance_score(&tags, query, 72.0, archived),
        )
    };
    SessionSearchHit {
        kind: SessionSearchHitKind::Session,
        matched_field,
        session_id,
        session_title: title,
        turn_id: None,
        turn_index: None,
        snippet,
        archived,
        updated_at_ms,
        score,
    }
}

fn search_turns_fts(
    connection: &Connection,
    query: &str,
    limit: i64,
    include_archived: bool,
) -> SessionSearchIndexResult<Vec<SessionSearchHit>> {
    let expression = fts_expression(query);
    let mut statement = connection
        .prepare(
            "SELECT t.session_id, s.title, t.turn_id, t.turn_index,
                    t.user_text, t.assistant_text, s.archived,
                    MAX(t.updated_at_ms, s.updated_at_ms)
             FROM turn_fts f
             JOIN turns t ON t.row_key = f.row_key
             JOIN sessions s ON s.session_id = t.session_id
             WHERE turn_fts MATCH ?1
               AND s.indexed_revision = s.source_revision
               AND (?2 OR s.archived = 0)
             ORDER BY bm25(turn_fts, 0.0, 0.0, 7.0, 4.0), t.updated_at_ms DESC
             LIMIT ?3",
        )
        .map_err(backend_error)?;
    map_turn_rows(
        &mut statement,
        params![expression, include_archived, limit],
        query,
    )
}

fn search_turns_like(
    connection: &Connection,
    query: &str,
    limit: i64,
    include_archived: bool,
) -> SessionSearchIndexResult<Vec<SessionSearchHit>> {
    let pattern = like_pattern(query);
    let mut statement = connection
        .prepare(
            "SELECT t.session_id, s.title, t.turn_id, t.turn_index,
                    t.user_text, t.assistant_text, s.archived,
                    MAX(t.updated_at_ms, s.updated_at_ms)
             FROM turns t
             JOIN sessions s ON s.session_id = t.session_id
             WHERE (?2 OR s.archived = 0)
               AND s.indexed_revision = s.source_revision
               AND (t.user_text LIKE ?1 ESCAPE '\\' OR t.assistant_text LIKE ?1 ESCAPE '\\')
             ORDER BY t.updated_at_ms DESC
             LIMIT ?3",
        )
        .map_err(backend_error)?;
    map_turn_rows(
        &mut statement,
        params![pattern, include_archived, limit],
        query,
    )
}

fn map_turn_rows<P: rusqlite::Params>(
    statement: &mut rusqlite::Statement<'_>,
    params: P,
    query: &str,
) -> SessionSearchIndexResult<Vec<SessionSearchHit>> {
    let rows = statement
        .query_map(params, |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, bool>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })
        .map_err(backend_error)?;
    rows.map(|row| row.map_err(backend_error))
        .map(|row| {
            row.map(
                |(
                    session_id,
                    session_title,
                    turn_id,
                    turn_index,
                    user_text,
                    assistant_text,
                    archived,
                    updated_at_ms,
                )| {
                    let (matched_field, text, base_score) = if matches_query(&user_text, query) {
                        (
                            SessionSearchMatchField::UserMessage,
                            user_text.as_str(),
                            82.0,
                        )
                    } else {
                        (
                            SessionSearchMatchField::AssistantMessage,
                            assistant_text.as_str(),
                            68.0,
                        )
                    };
                    SessionSearchHit {
                        kind: SessionSearchHitKind::Message,
                        matched_field,
                        session_id,
                        session_title,
                        turn_id: Some(turn_id),
                        turn_index: usize::try_from(turn_index).ok(),
                        snippet: build_snippet(text, query),
                        archived,
                        updated_at_ms,
                        score: relevance_score(text, query, base_score, archived),
                    }
                },
            )
        })
        .collect()
}

fn fts_expression(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn matches_query(text: &str, query: &str) -> bool {
    let text = text.to_lowercase();
    let query = query.to_lowercase();
    text.contains(&query)
        || query
            .split_whitespace()
            .filter(|term| !term.is_empty())
            .all(|term| text.contains(term))
}

fn relevance_score(text: &str, query: &str, base: f64, archived: bool) -> f64 {
    let text = text.trim().to_lowercase();
    let query = query.trim().to_lowercase();
    let quality = if text == query {
        12.0
    } else if text.starts_with(&query) {
        9.0
    } else if text.contains(&query) {
        6.0
    } else {
        3.0
    };
    (base + quality - if archived { 8.0 } else { 0.0 }).clamp(0.0, 100.0)
}

fn build_snippet(text: &str, query: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MAX_SNIPPET_CHARS {
        return text.to_string();
    }

    let lower = text.to_lowercase();
    let query_lower = query.to_lowercase();
    let byte_match = lower.find(&query_lower).or_else(|| {
        query_lower
            .split_whitespace()
            .filter(|term| !term.is_empty())
            .find_map(|term| lower.find(term))
    });
    let match_char = byte_match
        .map(|byte| lower[..byte].chars().count())
        .unwrap_or(0);
    let total_chars = text.chars().count();
    let start = match_char.saturating_sub(MAX_SNIPPET_CHARS / 3);
    let end = (start + MAX_SNIPPET_CHARS).min(total_chars);
    let snippet = text
        .chars()
        .skip(start)
        .take(end - start)
        .collect::<String>();
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        snippet,
        if end < total_chars { "…" } else { "" }
    )
}

fn compare_hits(left: &SessionSearchHit, right: &SessionSearchHit) -> std::cmp::Ordering {
    right
        .score
        .total_cmp(&left.score)
        .then_with(|| right.updated_at_ms.cmp(&left.updated_at_ms))
        .then_with(|| match (left.kind, right.kind) {
            (SessionSearchHitKind::Session, SessionSearchHitKind::Message) => {
                std::cmp::Ordering::Less
            }
            (SessionSearchHitKind::Message, SessionSearchHitKind::Session) => {
                std::cmp::Ordering::Greater
            }
            _ => std::cmp::Ordering::Equal,
        })
        .then_with(|| left.session_id.cmp(&right.session_id))
        .then_with(|| left.turn_index.cmp(&right.turn_index))
}

fn backend_error(error: impl std::fmt::Display) -> SessionSearchIndexError {
    SessionSearchIndexError::Backend(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(
        id: &str,
        title: &str,
        revision: &str,
        archived: bool,
    ) -> SessionSearchSessionDocument {
        SessionSearchSessionDocument {
            session_id: id.to_string(),
            title: title.to_string(),
            tags: vec!["architecture".to_string()],
            archived,
            updated_at_ms: 100,
            source_revision: revision.to_string(),
        }
    }

    fn turn(
        session_id: &str,
        turn_id: &str,
        user_text: &str,
        assistant_text: &str,
    ) -> SessionSearchTurnDocument {
        SessionSearchTurnDocument {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            turn_index: 0,
            user_text: user_text.to_string(),
            assistant_text: assistant_text.to_string(),
            updated_at_ms: 100,
        }
    }

    #[tokio::test]
    async fn indexes_titles_visible_messages_and_cjk_substrings() {
        let root = tempfile::tempdir().expect("tempdir");
        let index = SessionSearchSqliteIndex::new(root.path());
        let reconciliation = index
            .reconcile_sessions(vec![session(
                "session-1",
                "Global search design",
                "r1",
                false,
            )])
            .await
            .expect("reconcile");
        assert_eq!(reconciliation.stale_session_ids, vec!["session-1"]);
        index
            .replace_session_turns(
                "session-1",
                "r1",
                vec![turn(
                    "session-1",
                    "turn-1",
                    "请实现真正的全局搜索能力",
                    "The command catalog is now unified.",
                )],
            )
            .await
            .expect("index turns");

        let title = index
            .search("Global", 10, false)
            .await
            .expect("title search");
        assert_eq!(title.hits[0].kind, SessionSearchHitKind::Session);
        assert_eq!(title.hits[0].matched_field, SessionSearchMatchField::Title);

        let cjk = index
            .search("全局搜索", 10, false)
            .await
            .expect("CJK search");
        assert_eq!(cjk.hits[0].kind, SessionSearchHitKind::Message);
        assert_eq!(
            cjk.hits[0].matched_field,
            SessionSearchMatchField::UserMessage
        );

        let short = index.search("搜索", 10, false).await.expect("short search");
        assert_eq!(short.hits.len(), 1);

        let assistant = index
            .search("command catalog", 10, false)
            .await
            .expect("assistant search");
        assert_eq!(
            assistant.hits[0].matched_field,
            SessionSearchMatchField::AssistantMessage
        );
    }

    #[tokio::test]
    async fn archived_sessions_are_explicitly_gated() {
        let root = tempfile::tempdir().expect("tempdir");
        let index = SessionSearchSqliteIndex::new(root.path());
        index
            .reconcile_sessions(vec![session("archived-1", "Archived search", "r1", true)])
            .await
            .expect("reconcile");
        index
            .replace_session_turns("archived-1", "r1", vec![])
            .await
            .expect("index turns");

        assert!(index
            .search("Archived", 10, false)
            .await
            .expect("search")
            .hits
            .is_empty());
        assert_eq!(
            index
                .search("Archived", 10, true)
                .await
                .expect("search archived")
                .hits
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn reconciliation_repairs_revisions_and_removes_deleted_sessions() {
        let root = tempfile::tempdir().expect("tempdir");
        let index = SessionSearchSqliteIndex::new(root.path());
        index
            .reconcile_sessions(vec![session("session-1", "First title", "r1", false)])
            .await
            .expect("reconcile");
        index
            .replace_session_turns("session-1", "r1", vec![])
            .await
            .expect("index turns");

        let fresh = index
            .reconcile_sessions(vec![session("session-1", "Renamed", "r1", false)])
            .await
            .expect("fresh reconcile");
        assert!(fresh.stale_session_ids.is_empty());
        assert_eq!(
            index
                .search("Renamed", 10, false)
                .await
                .expect("renamed")
                .hits
                .len(),
            1
        );

        let stale = index
            .reconcile_sessions(vec![session("session-1", "Renamed", "r2", false)])
            .await
            .expect("stale reconcile");
        assert_eq!(stale.stale_session_ids, vec!["session-1"]);

        let removed = index.reconcile_sessions(vec![]).await.expect("remove");
        assert_eq!(removed.removed_session_count, 1);
        assert!(index
            .search("Renamed", 10, true)
            .await
            .expect("removed search")
            .hits
            .is_empty());
    }

    #[tokio::test]
    async fn unchanged_reconciliation_performs_no_sqlite_writes() {
        let root = tempfile::tempdir().expect("tempdir");
        let index = SessionSearchSqliteIndex::new(root.path());
        let document = session("session-1", "Stable title", "r1", false);
        index
            .reconcile_sessions(vec![document.clone()])
            .await
            .expect("initial reconcile");
        index
            .replace_session_turns("session-1", "r1", vec![])
            .await
            .expect("index turns");

        let mut connection = open_connection(index.path()).expect("connection");
        let changes_before = connection.total_changes();
        let reconciliation =
            reconcile_sessions(&mut connection, &[document]).expect("unchanged reconcile");

        assert!(reconciliation.stale_session_ids.is_empty());
        assert_eq!(connection.total_changes(), changes_before);
    }

    #[tokio::test]
    async fn invalidation_rejects_an_in_flight_stale_rebuild() {
        let root = tempfile::tempdir().expect("tempdir");
        let index = SessionSearchSqliteIndex::new(root.path());
        let document = session("session-1", "Concurrent update", "r1", false);
        index
            .reconcile_sessions(vec![document.clone()])
            .await
            .expect("reconcile");
        index
            .invalidate_session_if_present("session-1")
            .await
            .expect("invalidate");

        let error = index
            .replace_session_turns("session-1", "r1", vec![])
            .await
            .expect_err("an old rebuild must not overwrite invalidation");
        assert!(matches!(error, SessionSearchIndexError::InvalidDocument(_)));

        let reconciliation = index
            .reconcile_sessions(vec![document])
            .await
            .expect("authoritative reconcile");
        assert_eq!(reconciliation.stale_session_ids, vec!["session-1"]);
    }
}
