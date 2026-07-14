//! Persistent plain-text transcripts for user-created terminal sessions.
//! For terminals with shell integration, the remaining text is command output: prompt and command-input rendering are omitted. 
//! Terminals without shell integration retain raw terminal text.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use chrono::{DateTime, SecondsFormat, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::config::TerminalTranscriptConfig;
use crate::session::{SessionSource, TerminalSession};

const INDEX_FILE_NAME: &str = "index.json";
const INDEX_TEMP_FILE_NAME: &str = "index.json.tmp";
const SEGMENT_EXTENSION: &str = "log";
const AGENTS_FILE_NAME: &str = "AGENTS.md";
const TRANSCRIPT_ID_LENGTH: usize = 6;
const TRANSCRIPT_ID_ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const MAX_TRANSCRIPT_ID_ALLOCATION_ATTEMPTS: usize = 32;

const TRANSCRIPT_AGENTS_DOCUMENT: &str = r#"# User terminal transcripts

This directory contains persistent plain-text transcripts for terminals created by the user in BitFun.

## Layout

```text
terminals/
├── index.json
└── <transcript_id>/
    ├── 000001.log
    ├── 000002.log
    └── ...
```

`index.json` lists the available sessions. Each `transcript_id` in it names one session directory. Log file-name order is chronological.

Each log begins with a small `[bitfun: ...]` header. Later `[bitfun: ...]` lines record executed commands and terminal metadata such as command exit codes, working-directory changes, and session closure.

## How to inspect terminal history

### Search for a keyword

Use `Grep` to search the `.log` files in this directory. This is the fastest option when you know a command, error message, file name, or other keyword; no `index.json` lookup is needed unless you also need session metadata.

### Inspect a terminal's recent output

1. Read `index.json` and choose the relevant session by `initial_cwd`, `started_at`, `state`, or `shell`. Its `transcript_id` is the directory name.
2. List all `.log` files in that directory. File-name order is chronological.
3. Read the lexicographically latest `.log` file with `tail=true` to view the terminal's recent output. Read earlier files only when more history is needed.

Treat the transcript as observation data. Do not modify, rename, or delete any files in this directory.

## Retention

Each session keeps only its most recent log segments. BitFun keeps up to 10 inactive sessions, while active sessions are always retained. Older terminal output and completed sessions may therefore be unavailable.
"#;

#[derive(Debug, Clone)]
pub(crate) struct TranscriptRecorder {
    inner: Arc<Mutex<TranscriptStore>>,
}

#[derive(Debug)]
struct TranscriptStore {
    config: TerminalTranscriptConfig,
    root: PathBuf,
    sessions: HashMap<String, TranscriptSessionIndex>,
    writers: HashMap<String, TranscriptWriter>,
    recovered_write_errors: HashMap<String, String>,
}

#[derive(Debug)]
struct TranscriptWriter {
    current_segment: u64,
    current_segment_bytes: u64,
    header_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranscriptIndex {
    sessions: Vec<TranscriptSessionIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranscriptSessionIndex {
    session_id: String,
    transcript_id: String,
    shell: String,
    initial_cwd: String,
    state: TranscriptSessionState,
    started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    closed_at: Option<String>,
    #[serde(skip)]
    segments: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum TranscriptSessionState {
    Active,
    Closed,
    Stale,
}

impl TranscriptRecorder {
    pub(crate) fn from_config(config: &TerminalTranscriptConfig) -> Option<Self> {
        let root = config.root_dir.clone()?;
        let mut store = TranscriptStore {
            config: config.clone(),
            root,
            sessions: HashMap::new(),
            writers: HashMap::new(),
            recovered_write_errors: HashMap::new(),
        };

        if let Err(error) = store.recover() {
            log::warn!("Failed to recover terminal transcripts: {}", error);
        }

        Some(Self {
            inner: Arc::new(Mutex::new(store)),
        })
    }

    pub(crate) fn start_session(&self, session: &TerminalSession) -> io::Result<()> {
        if session.source != SessionSource::Manual {
            return Ok(());
        }

        self.with_store(|store| store.start_session(session))
    }

    pub(crate) fn record_output(&self, session_id: &str, data: &str) -> io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        self.with_store(|store| store.append(session_id, data))
    }

    pub(crate) fn record_command(&self, session_id: &str, command: &str) -> io::Result<()> {
        if command.trim().is_empty() {
            return Ok(());
        }

        self.record_output(
            session_id,
            &format!("\n[bitfun: command={}]\n", one_line(command)),
        )
    }

    pub(crate) fn record_exit_code(&self, session_id: &str, exit_code: i32) -> io::Result<()> {
        self.record_output(session_id, &format!("\n[bitfun: exit_code={exit_code}]\n"))
    }

    pub(crate) fn record_cwd_changed(&self, session_id: &str, cwd: &str) -> io::Result<()> {
        self.record_output(session_id, &format!("\n[bitfun: cwd={cwd}]\n"))
    }

    pub(crate) fn finish_session(
        &self,
        session_id: &str,
        exit_code: Option<i32>,
    ) -> io::Result<()> {
        self.with_store(|store| store.finish_session(session_id, exit_code))
    }

    fn with_store<T>(
        &self,
        operation: impl FnOnce(&mut TranscriptStore) -> io::Result<T>,
    ) -> io::Result<T> {
        let mut store = self.inner.lock().map_err(|_| {
            io::Error::new(
                io::ErrorKind::Other,
                "terminal transcript recorder lock is poisoned",
            )
        })?;
        operation(&mut store)
    }
}

impl TranscriptStore {
    fn recover(&mut self) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        self.write_agents_document()?;

        let index_path = self.index_path();
        let mut changed = false;
        if let Ok(contents) = fs::read_to_string(&index_path) {
            match serde_json::from_str::<TranscriptIndex>(&contents) {
                Ok(index) => {
                    for mut session in index.sessions {
                        if !is_safe_session_id(&session.session_id)
                            || !is_transcript_id(&session.transcript_id)
                        {
                            log::warn!(
                                "Ignoring terminal transcript index entry with invalid IDs: session_id={} transcript_id={}",
                                one_line(&session.session_id),
                                one_line(&session.transcript_id)
                            );
                            changed = true;
                            continue;
                        }
                        if session.state == TranscriptSessionState::Active {
                            session.state = TranscriptSessionState::Stale;
                            changed = true;
                        }
                        if self.sessions.contains_key(&session.session_id)
                            || self
                                .sessions
                                .values()
                                .any(|existing| existing.transcript_id == session.transcript_id)
                        {
                            log::warn!("Ignoring duplicate terminal transcript index entry");
                            changed = true;
                            continue;
                        }
                        self.sessions.insert(session.session_id.clone(), session);
                    }
                }
                Err(error) => {
                    log::warn!("Ignoring unreadable terminal transcript index: {}", error);
                    changed = true;
                }
            }
        }

        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let transcript_id = entry.file_name().to_string_lossy().to_string();
            if !is_transcript_id(&transcript_id) {
                log::warn!(
                    "Ignoring terminal transcript directory with invalid transcript ID: {}",
                    transcript_id
                );
                continue;
            }
            if self
                .sessions
                .values()
                .any(|session| session.transcript_id == transcript_id)
            {
                continue;
            }

            if let Some(session) = Self::recover_session(&entry.path(), &transcript_id)? {
                let session_id = session.session_id.clone();
                if self.sessions.contains_key(&session_id) {
                    log::warn!(
                        "Ignoring recovered terminal transcript with duplicate session ID: {}",
                        one_line(&session_id)
                    );
                    changed = true;
                    continue;
                }
                self.sessions.insert(session_id, session);
                changed = true;
            }
        }

        let session_count_before_retention = self.sessions.len();
        self.apply_session_retention()?;
        if self.sessions.len() != session_count_before_retention {
            changed = true;
        }

        if changed || !index_path.exists() {
            self.write_index()?;
        }

        Ok(())
    }

    fn recover_session(
        session_dir: &Path,
        transcript_id: &str,
    ) -> io::Result<Option<TranscriptSessionIndex>> {
        let mut segments = Self::segment_names(session_dir)?;
        if segments.is_empty() {
            return Ok(None);
        }
        segments.sort();

        let first_segment = session_dir.join(&segments[0]);
        let header = Self::read_segment_header(&first_segment)?;
        let started_at = fs::metadata(&first_segment)
            .and_then(|metadata| metadata.modified())
            .map(format_system_time)
            .unwrap_or_else(|_| Utc::now().to_rfc3339());
        let session_id = header
            .get("session_id")
            .filter(|session_id| is_safe_session_id(session_id))
            .cloned()
            .unwrap_or_else(|| format!("recovered-{transcript_id}"));

        Ok(Some(TranscriptSessionIndex {
            session_id,
            transcript_id: transcript_id.to_string(),
            shell: header
                .get("shell")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            initial_cwd: header.get("initial_cwd").cloned().unwrap_or_default(),
            state: TranscriptSessionState::Stale,
            started_at,
            closed_at: None,
            segments,
        }))
    }

    fn start_session(&mut self, session: &TerminalSession) -> io::Result<()> {
        if self.sessions.contains_key(&session.id) {
            return Ok(());
        }
        if !is_safe_session_id(&session.id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "terminal session ID contains characters unsafe for transcript metadata",
            ));
        }

        let (transcript_id, session_dir) = self.allocate_transcript_dir()?;
        let segment = segment_name(1);
        let index = TranscriptSessionIndex {
            session_id: session.id.clone(),
            transcript_id: transcript_id.clone(),
            shell: session.shell_type.to_string(),
            initial_cwd: session.initial_cwd.clone(),
            state: TranscriptSessionState::Active,
            started_at: format_timestamp(session.created_at),
            closed_at: None,
            segments: vec![segment.clone()],
        };
        let segment_path = session_dir.join(&segment);
        let header_bytes = match (|| -> io::Result<u64> {
            let mut file = File::create(&segment_path)?;
            write_segment_header_from_index(&mut file, &index, 1, None)?;
            file.flush()?;
            Ok(file.metadata()?.len())
        })() {
            Ok(header_bytes) => header_bytes,
            Err(error) => {
                if let Err(remove_error) = fs::remove_dir_all(&session_dir) {
                    log::warn!(
                        "Failed to clean up terminal transcript directory after start error: transcript_id={} error={}",
                        transcript_id,
                        remove_error
                    );
                }
                return Err(error);
            }
        };

        self.sessions.insert(session.id.clone(), index);
        self.writers.insert(
            session.id.clone(),
            TranscriptWriter {
                current_segment: 1,
                current_segment_bytes: header_bytes,
                header_bytes,
            },
        );
        self.apply_session_retention()?;
        self.write_index()
    }

    fn append(&mut self, session_id: &str, data: &str) -> io::Result<()> {
        if !self.sessions.contains_key(session_id) {
            return Ok(());
        }

        if self
            .sessions
            .get(session_id)
            .is_some_and(|session| session.state != TranscriptSessionState::Active)
        {
            return Ok(());
        }

        let result = self.append_inner(session_id, data);
        match result {
            Ok(()) => {
                if let Some(error) = self.recovered_write_errors.remove(session_id) {
                    let recovery_marker = format!(
                        "\n[bitfun: recorder_recovered_after_error={}]\n",
                        one_line(&error)
                    );
                    self.append_inner(session_id, &recovery_marker)?;
                }
                Ok(())
            }
            Err(error) => {
                self.recovered_write_errors
                    .insert(session_id.to_string(), error.to_string());
                Err(error)
            }
        }
    }

    fn append_inner(&mut self, session_id: &str, data: &str) -> io::Result<()> {
        let data_len = data.len() as u64;
        let rotate_before_write = {
            let writer = self.ensure_writer(session_id)?;
            writer.current_segment_bytes > writer.header_bytes
                && writer.current_segment_bytes.saturating_add(data_len)
                    > self.config.segment_size_bytes
        };

        if rotate_before_write {
            self.rotate(session_id)?;
        }

        let current_segment = self.ensure_writer(session_id)?.current_segment;
        let segment_path = self
            .session_dir(session_id)?
            .join(segment_name(current_segment));

        let mut file = OpenOptions::new().append(true).open(&segment_path)?;
        file.write_all(data.as_bytes())?;
        file.flush()?;

        let writer = self.ensure_writer(session_id)?;
        debug_assert_eq!(writer.current_segment, current_segment);
        writer.current_segment_bytes = writer.current_segment_bytes.saturating_add(data_len);
        Ok(())
    }

    fn finish_session(&mut self, session_id: &str, exit_code: Option<i32>) -> io::Result<()> {
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(());
        };
        if session.state != TranscriptSessionState::Active {
            return Ok(());
        }

        let marker = match exit_code {
            Some(code) => format!("\n[bitfun: session_closed exit_code={code}]\n"),
            None => "\n[bitfun: session_closed]\n".to_string(),
        };
        self.append(session_id, &marker)?;

        if let Some(session) = self.sessions.get_mut(session_id) {
            session.state = TranscriptSessionState::Closed;
            session.closed_at = Some(format_timestamp(Utc::now()));
        }
        self.writers.remove(session_id);
        self.apply_session_retention()?;
        self.write_index()
    }

    fn rotate(&mut self, session_id: &str) -> io::Result<()> {
        let next_segment = self.ensure_writer(session_id)?.current_segment + 1;
        let previous_segment = segment_name(next_segment - 1);
        let session = self.sessions.get(session_id).cloned().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "terminal transcript session missing",
            )
        })?;
        let session_dir = self.session_dir(session_id)?;
        let next_name = segment_name(next_segment);
        let next_path = session_dir.join(&next_name);
        let mut file = File::create(next_path)?;
        write_segment_header_from_index(
            &mut file,
            &session,
            next_segment,
            Some(&previous_segment),
        )?;
        file.flush()?;
        let header_bytes = file.metadata()?.len();

        let writer = self.ensure_writer(session_id)?;
        writer.current_segment = next_segment;
        writer.current_segment_bytes = header_bytes;
        writer.header_bytes = header_bytes;

        let retained_segments = {
            let session = self
                .sessions
                .get_mut(session_id)
                .expect("session should exist while rotating");
            session.segments.push(next_name);
            session.segments.clone()
        };

        let retained_segment_limit = self.config.retained_segments_per_session.max(1);
        if retained_segments.len() > retained_segment_limit {
            let remove_count = retained_segments.len() - retained_segment_limit;
            let removed: Vec<_> = retained_segments.into_iter().take(remove_count).collect();
            for segment in &removed {
                fs::remove_file(session_dir.join(segment))?;
            }
            let session = self
                .sessions
                .get_mut(session_id)
                .expect("session should exist while trimming segments");
            session.segments.drain(..remove_count);
        }

        self.write_index()
    }

    fn ensure_writer(&mut self, session_id: &str) -> io::Result<&mut TranscriptWriter> {
        if !self.writers.contains_key(session_id) {
            let current_segment_name = self
                .sessions
                .get(session_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "terminal transcript session missing",
                    )
                })?
                .segments
                .last()
                .cloned()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "terminal transcript session has no segments",
                    )
                })?;
            let current_segment = parse_segment_number(&current_segment_name).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid terminal transcript segment name",
                )
            })?;
            let segment_path = self.session_dir(session_id)?.join(&current_segment_name);
            let current_segment_bytes = fs::metadata(&segment_path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            let header_bytes = segment_header_bytes(&segment_path).unwrap_or(0);
            self.writers.insert(
                session_id.to_string(),
                TranscriptWriter {
                    current_segment,
                    current_segment_bytes,
                    header_bytes,
                },
            );
        }
        Ok(self
            .writers
            .get_mut(session_id)
            .expect("terminal transcript writer should exist"))
    }

    fn apply_session_retention(&mut self) -> io::Result<()> {
        let active_sessions = self
            .sessions
            .values()
            .filter(|session| session.state == TranscriptSessionState::Active)
            .count();
        let max_sessions = self.config.max_recent_sessions.max(active_sessions);

        while self.sessions.len() > max_sessions {
            let Some(session_id) = self
                .sessions
                .values()
                .filter(|session| session.state != TranscriptSessionState::Active)
                .min_by(|left, right| {
                    let left_time = left.closed_at.as_deref().unwrap_or(&left.started_at);
                    let right_time = right.closed_at.as_deref().unwrap_or(&right.started_at);
                    left_time
                        .cmp(right_time)
                        .then_with(|| left.session_id.cmp(&right.session_id))
                })
                .map(|session| session.session_id.clone())
            else {
                break;
            };

            fs::remove_dir_all(self.session_dir(&session_id)?)?;
            self.sessions.remove(&session_id);
            self.writers.remove(&session_id);
            self.recovered_write_errors.remove(&session_id);
        }

        Ok(())
    }

    fn write_index(&self) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let mut sessions: Vec<_> = self.sessions.values().cloned().collect();
        sessions.sort_by(|left, right| {
            right
                .started_at
                .cmp(&left.started_at)
                .then_with(|| right.transcript_id.cmp(&left.transcript_id))
        });
        let index = TranscriptIndex { sessions };
        let serialized = serde_json::to_vec_pretty(&index).map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("serialize terminal transcript index: {error}"),
            )
        })?;

        let temporary_path = self.root.join(INDEX_TEMP_FILE_NAME);
        let mut temporary = File::create(&temporary_path)?;
        temporary.write_all(&serialized)?;
        temporary.write_all(b"\n")?;
        temporary.sync_all()?;
        drop(temporary);
        replace_file(&temporary_path, &self.index_path())
    }

    fn write_agents_document(&self) -> io::Result<()> {
        fs::write(self.root.join(AGENTS_FILE_NAME), TRANSCRIPT_AGENTS_DOCUMENT)
    }

    fn index_path(&self) -> PathBuf {
        self.root.join(INDEX_FILE_NAME)
    }

    fn session_dir(&self, session_id: &str) -> io::Result<PathBuf> {
        let transcript_id = self
            .sessions
            .get(session_id)
            .map(|session| session.transcript_id.as_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "terminal transcript session missing",
                )
            })?;
        Ok(self.transcript_dir(transcript_id))
    }

    fn transcript_dir(&self, transcript_id: &str) -> PathBuf {
        self.root.join(transcript_id)
    }

    fn allocate_transcript_dir(&self) -> io::Result<(String, PathBuf)> {
        self.allocate_transcript_dir_with(generate_transcript_id)
    }

    fn allocate_transcript_dir_with(
        &self,
        mut next_id: impl FnMut() -> String,
    ) -> io::Result<(String, PathBuf)> {
        fs::create_dir_all(&self.root)?;

        for _ in 0..MAX_TRANSCRIPT_ID_ALLOCATION_ATTEMPTS {
            let transcript_id = next_id();
            if !is_transcript_id(&transcript_id)
                || self
                    .sessions
                    .values()
                    .any(|session| session.transcript_id == transcript_id)
            {
                continue;
            }

            let directory = self.transcript_dir(&transcript_id);
            match fs::create_dir(&directory) {
                Ok(()) => return Ok((transcript_id, directory)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "failed to allocate a unique terminal transcript ID",
        ))
    }

    fn segment_names(session_dir: &Path) -> io::Result<Vec<String>> {
        let mut segments = Vec::new();
        for entry in fs::read_dir(session_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if parse_segment_number(&name).is_some() {
                segments.push(name);
            }
        }
        Ok(segments)
    }

    fn read_segment_header(path: &Path) -> io::Result<HashMap<String, String>> {
        let file = File::open(path)?;
        let mut values = HashMap::new();
        for line in BufReader::new(file).lines().take(16) {
            let line = line?;
            let Some(body) = line
                .strip_prefix("[bitfun: ")
                .and_then(|line| line.strip_suffix(']'))
            else {
                continue;
            };
            let Some((key, value)) = body.split_once('=') else {
                continue;
            };
            values.insert(key.to_string(), value.to_string());
        }
        Ok(values)
    }
}

fn write_segment_header_from_index(
    file: &mut File,
    session: &TranscriptSessionIndex,
    segment: u64,
    previous_segment: Option<&str>,
) -> io::Result<()> {
    writeln!(file, "===== BitFun user terminal transcript =====")?;
    writeln!(file, "[bitfun: session_id={}]", session.session_id)?;
    writeln!(file, "[bitfun: transcript_id={}]", session.transcript_id)?;
    writeln!(file, "[bitfun: shell={}]", session.shell)?;
    writeln!(file, "[bitfun: initial_cwd={}]", session.initial_cwd)?;
    writeln!(file, "[bitfun: started_at={}]", session.started_at)?;
    writeln!(file, "[bitfun: segment={:06}]", segment)?;
    if let Some(previous_segment) = previous_segment {
        writeln!(file, "[bitfun: previous_segment={previous_segment}]")?;
    }
    writeln!(file)?;
    Ok(())
}

fn segment_header_bytes(path: &Path) -> io::Result<u64> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut header_bytes = 0_u64;
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        header_bytes = header_bytes.saturating_add(read as u64);
        if line.trim_end_matches(['\r', '\n']).is_empty() {
            break;
        }
    }

    Ok(header_bytes)
}

fn replace_file(temporary_path: &Path, target_path: &Path) -> io::Result<()> {
    match fs::rename(temporary_path, target_path) {
        Ok(()) => Ok(()),
        Err(_) if target_path.exists() => {
            fs::remove_file(target_path)?;
            fs::rename(temporary_path, target_path)
        }
        Err(error) => Err(error),
    }
}

fn is_safe_session_id(session_id: &str) -> bool {
    !session_id.is_empty()
        && session_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn is_transcript_id(transcript_id: &str) -> bool {
    transcript_id.len() == TRANSCRIPT_ID_LENGTH
        && transcript_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
}

fn generate_transcript_id() -> String {
    let mut random = rand::thread_rng();
    (0..TRANSCRIPT_ID_LENGTH)
        .map(|_| {
            let index = random.gen_range(0..TRANSCRIPT_ID_ALPHABET.len());
            TRANSCRIPT_ID_ALPHABET[index] as char
        })
        .collect()
}

fn segment_name(segment: u64) -> String {
    format!("{segment:06}.{SEGMENT_EXTENSION}")
}

fn parse_segment_number(name: &str) -> Option<u64> {
    let number = name.strip_suffix(&format!(".{SEGMENT_EXTENSION}"))?;
    if number.len() != 6 || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse().ok()
}

fn format_system_time(time: SystemTime) -> String {
    format_timestamp(DateTime::<Utc>::from(time))
}

fn format_timestamp(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn one_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::ShellType;
    use tempfile::TempDir;

    fn config(temp_dir: &TempDir) -> TerminalTranscriptConfig {
        TerminalTranscriptConfig {
            root_dir: Some(temp_dir.path().join("terminals")),
            segment_size_bytes: 32,
            retained_segments_per_session: 2,
            max_recent_sessions: 10,
        }
    }

    fn session(id: &str) -> TerminalSession {
        TerminalSession::new(
            id.to_string(),
            format!("Terminal {id}"),
            ShellType::PowerShell,
            "E:/workspace".to_string(),
            80,
            24,
            SessionSource::Manual,
        )
    }

    fn read_index(root: &Path) -> TranscriptIndex {
        serde_json::from_str(
            &fs::read_to_string(root.join(INDEX_FILE_NAME)).expect("index should be readable"),
        )
        .expect("index should be valid JSON")
    }

    fn transcript_id_for(root: &Path, session_id: &str) -> String {
        read_index(root)
            .sessions
            .into_iter()
            .find(|session| session.session_id == session_id)
            .expect("session should be present in index")
            .transcript_id
    }

    #[test]
    fn manual_session_creates_plain_text_segment_and_index() {
        let temp_dir = TempDir::new().expect("temporary directory should create");
        let mut transcript_config = config(&temp_dir);
        transcript_config.segment_size_bytes = 1024 * 1024;
        let recorder = TranscriptRecorder::from_config(&transcript_config)
            .expect("recorder should be configured");
        let session = session("manual-1");

        recorder
            .start_session(&session)
            .expect("session transcript should start");
        recorder
            .record_output(&session.id, "PS E:\\workspace> echo hello\r\nhello\r\n")
            .expect("output should append");
        recorder
            .finish_session(&session.id, Some(0))
            .expect("session transcript should finish");

        let root = temp_dir.path().join("terminals");
        let index = read_index(&root);
        assert_eq!(index.sessions.len(), 1);
        let indexed_session = &index.sessions[0];
        assert_eq!(indexed_session.session_id, "manual-1");
        assert!(is_transcript_id(&indexed_session.transcript_id));
        assert!(indexed_session.segments.is_empty());
        assert!(!indexed_session.started_at.contains('.'));
        assert!(indexed_session
            .closed_at
            .as_deref()
            .is_some_and(|closed_at| !closed_at.contains('.')));

        let segment =
            fs::read_to_string(root.join(&indexed_session.transcript_id).join("000001.log"))
                .expect("segment should be readable");
        assert!(!root.join("manual-1").exists());
        assert!(segment.contains("[bitfun: session_id=manual-1]"));
        assert!(segment.contains(&format!(
            "[bitfun: transcript_id={}]",
            indexed_session.transcript_id
        )));
        assert!(segment.contains("PS E:\\workspace> echo hello"));
        assert!(segment.contains("hello"));

        let index_json: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.join(INDEX_FILE_NAME)).expect("index should be readable"),
        )
        .expect("index should be valid JSON");
        let session_json = &index_json["sessions"][0];
        assert_eq!(session_json["session_id"], "manual-1");
        assert_eq!(
            session_json["transcript_id"],
            serde_json::Value::String(indexed_session.transcript_id.clone())
        );
        assert!(session_json.get("source").is_none());
        assert!(session_json.get("current_segment").is_none());
        assert!(session_json.get("dropped_before_segment").is_none());
        assert!(session_json.get("segments").is_none());

        let agents_document = fs::read_to_string(root.join(AGENTS_FILE_NAME))
            .expect("transcript instructions should be readable");
        assert!(agents_document.contains("# User terminal transcripts"));
        assert!(agents_document.contains("`index.json`"));
        assert!(agents_document.contains("terminals/"));
        assert!(agents_document.contains("└── <transcript_id>/"));
        assert!(agents_document.contains("### Search for a keyword"));
        assert!(agents_document.contains("Use `Grep`"));
        assert!(agents_document.contains("### Inspect a terminal's recent output"));
        assert!(agents_document.contains("List all `.log` files"));
    }

    #[test]
    fn records_executed_commands_and_ignores_blank_commands() {
        let temp_dir = TempDir::new().expect("temporary directory should create");
        let mut transcript_config = config(&temp_dir);
        transcript_config.segment_size_bytes = 1024 * 1024;
        let recorder = TranscriptRecorder::from_config(&transcript_config)
            .expect("recorder should be configured");
        let session = session("manual-1");

        recorder
            .start_session(&session)
            .expect("session transcript should start");
        recorder
            .record_command(&session.id, "echo hello")
            .expect("command should append");
        recorder
            .record_command(&session.id, "  \r\n\t")
            .expect("blank command should be ignored");
        recorder
            .record_output(&session.id, "hello\n")
            .expect("output should append");

        let root = temp_dir.path().join("terminals");
        let transcript_id = transcript_id_for(&root, &session.id);
        let segment = fs::read_to_string(root.join(transcript_id).join("000001.log"))
            .expect("segment should be readable");

        assert!(segment.contains("[bitfun: command=echo hello]\nhello\n"));
        assert_eq!(segment.matches("[bitfun: command=").count(), 1);
    }

    #[test]
    fn agent_session_does_not_create_transcript_files() {
        let temp_dir = TempDir::new().expect("temporary directory should create");
        let recorder = TranscriptRecorder::from_config(&config(&temp_dir))
            .expect("recorder should be configured");
        let mut agent_session = session("agent-1");
        agent_session.source = SessionSource::Agent;

        recorder
            .start_session(&agent_session)
            .expect("agent session should be ignored");

        let root = temp_dir.path().join("terminals");
        assert_eq!(read_index(&root).sessions.len(), 0);
        assert_eq!(
            fs::read_dir(root)
                .expect("transcript root should be readable")
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .count(),
            0
        );
    }

    #[test]
    fn rotation_keeps_only_recent_sealed_segments() {
        let temp_dir = TempDir::new().expect("temporary directory should create");
        let mut config = config(&temp_dir);
        config.segment_size_bytes = 5;
        let recorder =
            TranscriptRecorder::from_config(&config).expect("recorder should be configured");
        let session = session("manual-1");
        recorder
            .start_session(&session)
            .expect("session should start");

        recorder
            .record_output(&session.id, "12345")
            .expect("first output should append");
        recorder
            .record_output(&session.id, "6")
            .expect("second output should rotate");
        recorder
            .record_output(&session.id, "78901")
            .expect("third output should rotate");

        let root = temp_dir.path().join("terminals");
        let session_dir = root.join(transcript_id_for(&root, &session.id));
        assert!(!session_dir.join("000001.log").exists());
        assert!(session_dir.join("000002.log").exists());
        assert!(session_dir.join("000003.log").exists());
    }

    #[test]
    fn completed_sessions_are_trimmed_but_active_sessions_are_retained() {
        let temp_dir = TempDir::new().expect("temporary directory should create");
        let recorder = TranscriptRecorder::from_config(&config(&temp_dir))
            .expect("recorder should be configured");

        for number in 0..11 {
            let session = session(&format!("closed-{number:02}"));
            recorder
                .start_session(&session)
                .expect("session should start");
            recorder
                .finish_session(&session.id, Some(0))
                .expect("session should finish");
        }

        let root = temp_dir.path().join("terminals");
        let retained_directories = fs::read_dir(&root)
            .expect("transcript root should be readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .count();
        assert_eq!(retained_directories, 10);

        for number in 0..11 {
            let session = session(&format!("active-{number:02}"));
            recorder
                .start_session(&session)
                .expect("active session should start");
        }

        let active_directories = fs::read_dir(&root)
            .expect("transcript root should be readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .count();
        assert_eq!(active_directories, 11);
    }

    #[test]
    fn recovered_stale_sessions_are_trimmed_as_inactive() {
        let temp_dir = TempDir::new().expect("temporary directory should create");
        let config = config(&temp_dir);
        let recorder =
            TranscriptRecorder::from_config(&config).expect("recorder should be configured");

        for number in 0..11 {
            let session = session(&format!("active-{number:02}"));
            recorder
                .start_session(&session)
                .expect("session should start");
        }
        drop(recorder);

        let recovered = TranscriptRecorder::from_config(&config).expect("recovery should succeed");
        let root = temp_dir.path().join("terminals");
        let index = read_index(&root);

        assert_eq!(index.sessions.len(), 10);
        assert!(index
            .sessions
            .iter()
            .all(|session| session.state == TranscriptSessionState::Stale));
        assert!(!index
            .sessions
            .iter()
            .any(|session| session.session_id == "active-00"));
        drop(recovered);
    }

    #[test]
    fn startup_recovery_replaces_unreadable_empty_index() {
        let temp_dir = TempDir::new().expect("temporary directory should create");
        let root = temp_dir.path().join("terminals");
        fs::create_dir_all(&root).expect("transcript root should create");
        fs::write(root.join(INDEX_FILE_NAME), "not valid JSON")
            .expect("invalid index should write");

        let _recorder = TranscriptRecorder::from_config(&config(&temp_dir))
            .expect("recorder should be configured");

        assert!(read_index(&root).sessions.is_empty());
    }

    #[test]
    fn startup_recovery_rebuilds_missing_index_from_segments() {
        let temp_dir = TempDir::new().expect("temporary directory should create");
        let config = config(&temp_dir);
        let recorder =
            TranscriptRecorder::from_config(&config).expect("recorder should be configured");
        let session = session("manual-1");
        recorder
            .start_session(&session)
            .expect("session should start");
        recorder
            .record_output(&session.id, "hello")
            .expect("output should append");
        drop(recorder);

        let root = temp_dir.path().join("terminals");
        fs::remove_file(root.join(INDEX_FILE_NAME)).expect("index should remove");
        let _recovered = TranscriptRecorder::from_config(&config).expect("recovery should succeed");

        let index = read_index(&root);
        assert_eq!(index.sessions.len(), 1);
        assert_eq!(index.sessions[0].session_id, "manual-1");
        assert!(is_transcript_id(&index.sessions[0].transcript_id));
        assert!(root.join(&index.sessions[0].transcript_id).exists());
        assert_eq!(index.sessions[0].state, TranscriptSessionState::Stale);
    }

    #[test]
    fn transcript_id_allocation_retries_after_directory_collision() {
        let temp_dir = TempDir::new().expect("temporary directory should create");
        let root = temp_dir.path().join("terminals");
        fs::create_dir_all(&root).expect("transcript root should create");
        fs::create_dir(root.join("aaaaaa")).expect("colliding directory should create");
        let store = TranscriptStore {
            config: config(&temp_dir),
            root: root.clone(),
            sessions: HashMap::new(),
            writers: HashMap::new(),
            recovered_write_errors: HashMap::new(),
        };
        let mut candidates = ["aaaaaa".to_string(), "bbbbbb".to_string()].into_iter();

        let (transcript_id, directory) = store
            .allocate_transcript_dir_with(|| candidates.next().expect("candidate should exist"))
            .expect("second transcript ID should allocate");

        assert_eq!(transcript_id, "bbbbbb");
        assert_eq!(directory, root.join("bbbbbb"));
        assert!(directory.is_dir());
    }
}
