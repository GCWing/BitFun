use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::protocol::{
    DispatchEvent, DispatchJobListEntry, DispatchJobState, DispatchSubmitRequest,
    DISPATCH_PROTOCOL_VERSION,
};

const JOB_RECORD_FILE: &str = "job.json";
const STATE_FILE: &str = "state";
const EVENTS_FILE: &str = "events.ndjson";
const EVENTS_LOCK_FILE: &str = ".events.lock";
const PID_FILE: &str = "job.pid";
const PREPARING_FILE: &str = "preparing";
const SPAWN_LOCK_FILE: &str = ".spawn.lock";
const WORKER_LOCK_FILE: &str = ".worker.lock";
const DEFAULT_MAX_EVENTS_BYTES: u64 = 64 * 1024 * 1024;
// Keep a single projected event and a complete status page comfortably below
// the server transport's 256 KiB WebSocket frame ceiling.
const MAX_EVENT_BYTES: usize = 96 * 1024;
const MAX_STATUS_PAGE_BYTES: u64 = 128 * 1024;
const MAX_STATUS_PAGE_EVENTS: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct EventLogHeader {
    cursor_base: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchJobRecord {
    pub(crate) protocol_version: u32,
    pub(crate) intent_sha256: String,
    pub(crate) request: DispatchSubmitRequest,
    pub(crate) created_at: String,
    pub(crate) title: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchStateRecord {
    pub(crate) state: DispatchJobState,
    #[serde(default)]
    pub(crate) started_at: Option<String>,
    #[serde(default)]
    pub(crate) finished_at: Option<String>,
    #[serde(default)]
    pub(crate) turn_id: Option<String>,
    #[serde(default)]
    pub(crate) cancel_requested_at: Option<String>,
    #[serde(default)]
    pub(crate) last_error: Option<String>,
}

impl DispatchStateRecord {
    fn queued() -> Self {
        Self {
            state: DispatchJobState::Queued,
            started_at: None,
            finished_at: None,
            turn_id: None,
            cancel_requested_at: None,
            last_error: None,
        }
    }

    pub(crate) fn cancel_requested(&self) -> bool {
        self.cancel_requested_at.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CreateJobOutcome {
    Created(DispatchStateRecord),
    Existing(DispatchStateRecord),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EventPage {
    pub(crate) cursor: u64,
    pub(crate) events: Vec<DispatchEvent>,
    pub(crate) cursor_reset: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DispatchStore {
    root: PathBuf,
    max_events_bytes: u64,
}

impl DispatchStore {
    pub(crate) fn open_default() -> Result<Self> {
        let path_manager = bitfun_core::infrastructure::PathManager::new()
            .map_err(|error| anyhow!("resolve BitFun storage root: {error}"))?;
        Self::open(path_manager.bitfun_home_dir().join("dispatch"))
    }

    pub(crate) fn open(root: PathBuf) -> Result<Self> {
        create_private_dir(&root)?;
        create_private_dir(&root.join("jobs"))?;
        create_private_dir(&root.join("workspaces"))?;
        Ok(Self {
            root,
            max_events_bytes: DEFAULT_MAX_EVENTS_BYTES,
        })
    }

    pub(crate) fn create_job_with_intent(
        &self,
        intent: DispatchSubmitRequest,
        request: DispatchSubmitRequest,
        title: String,
    ) -> Result<CreateJobOutcome> {
        validate_id("jobId", &request.job_id)?;
        let intent_sha256 = submit_intent_fingerprint(&intent)?;
        let job_dir = self.job_dir(&request.job_id)?;
        create_private_dir(&job_dir)?;
        let _lock = JobLock::exclusive(&job_dir.join(".lock"))?;

        let record_path = job_dir.join(JOB_RECORD_FILE);
        match fs::symlink_metadata(&record_path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    bail!(
                        "dispatch job commit marker is not a regular file: {}",
                        request.job_id
                    );
                }
                let existing = read_json::<DispatchJobRecord>(&record_path)?;
                if existing.intent_sha256 != intent_sha256 {
                    bail!(
                        "jobId '{}' already exists with a different dispatch request",
                        request.job_id
                    );
                }
                return Ok(CreateJobOutcome::Existing(
                    self.load_state_unlocked(&job_dir)?,
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "inspect dispatch job commit marker {}",
                        record_path.display()
                    )
                })
            }
        }

        let record = DispatchJobRecord {
            protocol_version: DISPATCH_PROTOCOL_VERSION,
            intent_sha256,
            request,
            created_at: chrono::Utc::now().to_rfc3339(),
            title,
        };
        // job.json is the commit marker. Any fragments left without it came
        // from an interrupted initialization and are safely rebuilt while the
        // job lock is held. Publishing the record last means its presence
        // guarantees state and the initial event stream are already durable.
        let state = DispatchStateRecord::queued();
        atomic_write_json(&job_dir.join(STATE_FILE), &state)?;
        ensure_private_file(&job_dir.join(EVENTS_LOCK_FILE))?;
        atomic_write_event_log(&job_dir.join(EVENTS_FILE), 0, None)?;
        self.append_event_unlocked(
            &job_dir,
            &DispatchEvent::approval_policy_selected(record.request.approval_policy),
        )?;
        self.append_event_unlocked(
            &job_dir,
            &DispatchEvent::job_state(DispatchJobState::Queued, None),
        )?;
        atomic_write_json(&record_path, &record)?;
        Ok(CreateJobOutcome::Created(state))
    }

    #[cfg(test)]
    pub(crate) fn create_job(
        &self,
        request: DispatchSubmitRequest,
        title: String,
    ) -> Result<CreateJobOutcome> {
        self.create_job_with_intent(request.clone(), request, title)
    }

    pub(crate) fn load_existing_job_for_intent(
        &self,
        intent: &DispatchSubmitRequest,
    ) -> Result<Option<(DispatchJobRecord, DispatchStateRecord)>> {
        validate_id("jobId", &intent.job_id)?;
        let job_dir = self.job_dir(&intent.job_id)?;
        let metadata = match fs::symlink_metadata(&job_dir) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspect dispatch job {}", job_dir.display()))
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            bail!(
                "dispatch job path is not a private directory: {}",
                intent.job_id
            );
        }
        let _lock = JobLock::shared(&job_dir.join(".lock"))?;
        let record_path = job_dir.join(JOB_RECORD_FILE);
        let record_metadata = match fs::symlink_metadata(&record_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "inspect dispatch job commit marker {}",
                        record_path.display()
                    )
                })
            }
        };
        if record_metadata.file_type().is_symlink() || !record_metadata.is_file() {
            bail!(
                "dispatch job commit marker is not a regular file: {}",
                intent.job_id
            );
        }
        let record = read_json::<DispatchJobRecord>(&record_path)?;
        if record.intent_sha256 != submit_intent_fingerprint(intent)? {
            bail!(
                "jobId '{}' already exists with a different dispatch request",
                intent.job_id
            );
        }
        let state = self.load_state_unlocked(&job_dir)?;
        Ok(Some((record, state)))
    }

    pub(crate) fn load_job(&self, job_id: &str) -> Result<DispatchJobRecord> {
        let job_dir = self.existing_job_dir(job_id)?;
        read_json(&job_dir.join(JOB_RECORD_FILE))
    }

    pub(crate) fn load_state(&self, job_id: &str) -> Result<DispatchStateRecord> {
        let job_dir = self.existing_job_dir(job_id)?;
        let _lock = JobLock::shared(&job_dir.join(".lock"))?;
        self.load_state_unlocked(&job_dir)
    }

    pub(crate) fn mark_state(
        &self,
        job_id: &str,
        state: DispatchJobState,
        turn_id: Option<&str>,
        message: Option<String>,
    ) -> Result<(DispatchStateRecord, bool)> {
        let job_dir = self.existing_job_dir(job_id)?;
        let _lock = JobLock::exclusive(&job_dir.join(".lock"))?;
        let mut current = self.load_state_unlocked(&job_dir)?;
        if current.state.is_terminal() {
            return Ok((current, false));
        }
        if current.state == state {
            if current.turn_id.is_none() {
                current.turn_id = turn_id.map(ToOwned::to_owned);
                atomic_write_json(&job_dir.join(STATE_FILE), &current)?;
            }
            return Ok((current, false));
        }

        let now = chrono::Utc::now().to_rfc3339();
        current.state = state;
        if state == DispatchJobState::Running && current.started_at.is_none() {
            current.started_at = Some(now.clone());
        }
        if state.is_terminal() {
            current.finished_at = Some(now);
        }
        if let Some(turn_id) = turn_id {
            current.turn_id = Some(turn_id.to_string());
        }
        if state.is_terminal() {
            current.last_error = if state == DispatchJobState::Failed {
                message.clone()
            } else {
                None
            };
        }
        atomic_write_json(&job_dir.join(STATE_FILE), &current)?;
        self.append_event_unlocked(&job_dir, &DispatchEvent::job_state(state, message))?;
        Ok((current, true))
    }

    pub(crate) fn request_cancel(&self, job_id: &str) -> Result<DispatchStateRecord> {
        let job_dir = self.existing_job_dir(job_id)?;
        let _lock = JobLock::exclusive(&job_dir.join(".lock"))?;
        let mut state = self.load_state_unlocked(&job_dir)?;
        if state.state.is_terminal() || state.cancel_requested() {
            return Ok(state);
        }
        state.cancel_requested_at = Some(chrono::Utc::now().to_rfc3339());
        state.last_error = None;
        atomic_write_json(&job_dir.join(STATE_FILE), &state)?;
        self.append_event_unlocked(&job_dir, &DispatchEvent::cancel_requested())?;
        Ok(state)
    }

    pub(crate) fn record_nonterminal_error(&self, job_id: &str, error: &str) -> Result<()> {
        let job_dir = self.existing_job_dir(job_id)?;
        let _lock = JobLock::exclusive(&job_dir.join(".lock"))?;
        let mut state = self.load_state_unlocked(&job_dir)?;
        if !state.state.is_terminal() {
            state.last_error = Some(error.to_string());
            atomic_write_json(&job_dir.join(STATE_FILE), &state)?;
        }
        Ok(())
    }

    pub(crate) fn settle_exited_worker(&self, job_id: &str) -> Result<DispatchStateRecord> {
        let job_dir = self.existing_job_dir(job_id)?;
        let _lock = JobLock::exclusive(&job_dir.join(".lock"))?;
        let mut state = self.load_state_unlocked(&job_dir)?;
        if state.state.is_terminal() {
            return Ok(state);
        }

        let (terminal_state, message) = if state.cancel_requested() {
            (
                DispatchJobState::Cancelled,
                "Dispatch worker stopped after a cancellation request",
            )
        } else if state.turn_id.is_some() {
            (
                DispatchJobState::Failed,
                "Dispatch worker exited after reserving a turn; the prompt was not replayed to avoid duplicate side effects",
            )
        } else {
            (
                DispatchJobState::Failed,
                "Dispatch worker exited without writing a terminal state",
            )
        };
        state.state = terminal_state;
        state.finished_at = Some(chrono::Utc::now().to_rfc3339());
        state.last_error = if terminal_state == DispatchJobState::Failed {
            Some(message.to_string())
        } else {
            None
        };
        atomic_write_json(&job_dir.join(STATE_FILE), &state)?;
        self.append_event_unlocked(
            &job_dir,
            &DispatchEvent::job_state(terminal_state, Some(message.to_string())),
        )?;
        Ok(state)
    }

    pub(crate) fn record_turn_id(&self, job_id: &str, turn_id: &str) -> Result<()> {
        let job_dir = self.existing_job_dir(job_id)?;
        let _lock = JobLock::exclusive(&job_dir.join(".lock"))?;
        let mut state = self.load_state_unlocked(&job_dir)?;
        if !state.state.is_terminal() && state.turn_id.as_deref() != Some(turn_id) {
            state.turn_id = Some(turn_id.to_string());
            atomic_write_json(&job_dir.join(STATE_FILE), &state)?;
        }
        Ok(())
    }

    pub(crate) fn try_claim_worker_spawn(&self, job_id: &str) -> Result<Option<DispatchLease>> {
        let job_dir = self.existing_job_dir(job_id)?;
        let Some(lease) = DispatchLease::try_acquire(&job_dir.join(SPAWN_LOCK_FILE))? else {
            return Ok(None);
        };
        let _lock = JobLock::exclusive(&job_dir.join(".lock"))?;
        let state = self.load_state_unlocked(&job_dir)?;
        if state.state != DispatchJobState::Queued
            || state.turn_id.is_some()
            || state.cancel_requested()
        {
            return Ok(None);
        }
        if let Some(pid) = self.read_pid(job_id)? {
            if super::runner::process_alive(pid) {
                return Ok(None);
            }
            remove_file_if_present(&job_dir.join(PID_FILE));
        }
        atomic_write(
            &job_dir.join(PREPARING_FILE),
            chrono::Utc::now().to_rfc3339().as_bytes(),
        )?;
        Ok(Some(lease))
    }

    pub(crate) fn try_acquire_worker_lease(&self, job_id: &str) -> Result<Option<DispatchLease>> {
        let job_dir = self.existing_job_dir(job_id)?;
        DispatchLease::try_acquire(&job_dir.join(WORKER_LOCK_FILE))
    }

    pub(crate) fn append_event(&self, job_id: &str, event: &DispatchEvent) -> Result<u64> {
        let job_dir = self.existing_job_dir(job_id)?;
        self.append_event_unlocked(&job_dir, event)
    }

    pub(crate) fn read_events(&self, job_id: &str, cursor: u64) -> Result<EventPage> {
        let job_dir = self.existing_job_dir(job_id)?;
        let lock_path = job_dir.join(EVENTS_LOCK_FILE);
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .with_context(|| format!("open dispatch event lock {}", lock_path.display()))?;
        let _lock = FileLock::shared(&lock_file)?;
        let path = job_dir.join(EVENTS_FILE);
        let mut file = OpenOptions::new()
            .read(true)
            .open(&path)
            .with_context(|| format!("open dispatch events {}", path.display()))?;
        set_private_file_permissions(&path)?;
        let len = file.metadata()?.len();
        let (header, data_start) = read_event_log_header(&mut file, &path)?;
        let data_len = len.saturating_sub(data_start);
        let retained_end = header.cursor_base.saturating_add(data_len);
        let (start, cursor_reset) = if cursor < header.cursor_base || cursor > retained_end {
            (0, true)
        } else {
            (cursor.saturating_sub(header.cursor_base), false)
        };
        file.seek(SeekFrom::Start(data_start.saturating_add(start)))?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_STATUS_PAGE_BYTES)
            .read_to_end(&mut bytes)?;

        let mut events = Vec::new();
        let mut consumed = 0_usize;
        while events.len() < MAX_STATUS_PAGE_EVENTS {
            let Some(relative_newline) = bytes[consumed..].iter().position(|byte| *byte == b'\n')
            else {
                break;
            };
            let line_end = consumed + relative_newline;
            let line = &bytes[consumed..line_end];
            consumed = line_end + 1;
            if line.is_empty() {
                continue;
            }
            let event = serde_json::from_slice(line)
                .with_context(|| format!("decode dispatch event for job {job_id}"))?;
            events.push(event);
        }
        Ok(EventPage {
            cursor: header
                .cursor_base
                .saturating_add(start)
                .saturating_add(consumed as u64),
            events,
            cursor_reset,
        })
    }

    pub(crate) fn list_jobs(&self) -> Result<Vec<DispatchJobListEntry>> {
        let jobs_dir = self.root.join("jobs");
        let mut entries = Vec::new();
        for entry in fs::read_dir(&jobs_dir)
            .with_context(|| format!("read dispatch jobs {}", jobs_dir.display()))?
        {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(job_id) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            let Ok(job) = self.load_job(&job_id) else {
                continue;
            };
            let Ok(state) = self.load_state(&job_id) else {
                continue;
            };
            entries.push(DispatchJobListEntry {
                job_id,
                session_id: job.request.session_id,
                state: state.state,
                started_at: state.started_at,
                workspace_path: job.request.workspace_path,
                title: job.title,
            });
        }
        entries.sort_by(|left, right| right.started_at.cmp(&left.started_at));
        Ok(entries)
    }

    pub(crate) fn write_pid(&self, job_id: &str, pid: u32) -> Result<()> {
        let job_dir = self.existing_job_dir(job_id)?;
        atomic_write(&job_dir.join(PID_FILE), format!("{pid}\n").as_bytes())
    }

    pub(crate) fn read_pid(&self, job_id: &str) -> Result<Option<u32>> {
        let job_dir = self.existing_job_dir(job_id)?;
        let path = job_dir.join(PID_FILE);
        match fs::read_to_string(&path) {
            Ok(raw) => {
                let pid = raw
                    .trim()
                    .parse::<u32>()
                    .with_context(|| format!("decode worker pid {}", path.display()))?;
                if pid <= 1 || i32::try_from(pid).is_err() {
                    bail!("dispatch worker pid is outside the safe process range: {pid}");
                }
                Ok(Some(pid))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("read worker pid {}", path.display())),
        }
    }

    pub(crate) fn remove_pid(&self, job_id: &str) {
        if let Ok(job_dir) = self.job_dir(job_id) {
            remove_file_if_present(&job_dir.join(PID_FILE));
        }
    }

    pub(crate) fn remove_pid_if_matches(&self, job_id: &str, expected_pid: u32) {
        if matches!(self.read_pid(job_id), Ok(Some(pid)) if pid == expected_pid) {
            self.remove_pid(job_id);
        }
    }

    pub(crate) fn clear_preparing(&self, job_id: &str) {
        if let Ok(job_dir) = self.job_dir(job_id) {
            remove_file_if_present(&job_dir.join(PREPARING_FILE));
        }
    }

    pub(crate) fn preparing_age_seconds(&self, job_id: &str) -> Result<Option<u64>> {
        let job_dir = self.existing_job_dir(job_id)?;
        let path = job_dir.join(PREPARING_FILE);
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read preparing marker {}", path.display()))
            }
        };
        let modified = metadata.modified()?;
        Ok(Some(modified.elapsed().unwrap_or_default().as_secs()))
    }

    pub(crate) fn workspace_lock_path(&self, workspace_path: &str) -> PathBuf {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(workspace_path.as_bytes());
        self.root
            .join("workspaces")
            .join(format!("{digest:x}.lock"))
    }

    fn load_state_unlocked(&self, job_dir: &Path) -> Result<DispatchStateRecord> {
        read_json(&job_dir.join(STATE_FILE))
    }

    fn append_event_unlocked(&self, job_dir: &Path, event: &DispatchEvent) -> Result<u64> {
        let lock_path = job_dir.join(EVENTS_LOCK_FILE);
        let lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("open dispatch event lock {}", lock_path.display()))?;
        set_private_file_permissions(&lock_path)?;
        let _lock = FileLock::exclusive(&lock_file)?;
        let path = job_dir.join(EVENTS_FILE);
        let mut file = OpenOptions::new()
            .append(true)
            .read(true)
            .open(&path)
            .with_context(|| format!("open dispatch events {}", path.display()))?;
        set_private_file_permissions(&path)?;
        let encoded = serde_json::to_vec(event).context("encode dispatch event")?;
        let encoded = if encoded.len() > MAX_EVENT_BYTES {
            serde_json::to_vec(&DispatchEvent::oversized_event_omitted(
                encoded.len(),
                MAX_EVENT_BYTES,
            ))
            .context("encode oversized dispatch event marker")?
        } else {
            encoded
        };
        let (header, data_start) = read_event_log_header(&mut file, &path)?;
        let physical_len = truncate_incomplete_event_tail(&mut file)?;
        let current_len = physical_len.saturating_sub(data_start);
        if current_len
            .saturating_add(encoded.len() as u64)
            .saturating_add(1)
            > self.max_events_bytes
        {
            let cursor_base = header.cursor_base.saturating_add(current_len);
            atomic_write_event_log(&path, cursor_base, Some(&encoded))?;
            return Ok(cursor_base
                .saturating_add(encoded.len() as u64)
                .saturating_add(1));
        }
        file.write_all(&encoded)?;
        file.write_all(b"\n")?;
        file.sync_data()?;
        Ok(header
            .cursor_base
            .saturating_add(file.metadata()?.len().saturating_sub(data_start)))
    }

    fn existing_job_dir(&self, job_id: &str) -> Result<PathBuf> {
        let path = self.job_dir(job_id)?;
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("dispatch job not found: {job_id}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            bail!("dispatch job path is not a private directory: {job_id}");
        }
        let record_path = path.join(JOB_RECORD_FILE);
        let record_metadata = fs::symlink_metadata(&record_path)
            .with_context(|| format!("dispatch job is not committed: {job_id}"))?;
        if record_metadata.file_type().is_symlink() || !record_metadata.is_file() {
            bail!("dispatch job commit marker is not a regular file: {job_id}");
        }
        Ok(path)
    }

    fn job_dir(&self, job_id: &str) -> Result<PathBuf> {
        validate_id("jobId", job_id)?;
        Ok(self.root.join("jobs").join(job_id))
    }

    #[cfg(test)]
    fn open_with_event_limit(root: PathBuf, max_events_bytes: u64) -> Result<Self> {
        let mut store = Self::open(root)?;
        store.max_events_bytes = max_events_bytes;
        Ok(store)
    }
}

pub(crate) struct WorkspaceLock {
    _file: File,
}

impl WorkspaceLock {
    pub(crate) fn acquire(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            create_private_dir(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .with_context(|| format!("open workspace dispatch lock {}", path.display()))?;
        set_private_file_permissions(path)?;
        FileLock::exclusive(&file)?;
        Ok(Self { _file: file })
    }
}

pub(crate) struct DispatchLease {
    _file: File,
}

impl DispatchLease {
    fn try_acquire(path: &Path) -> Result<Option<Self>> {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .with_context(|| format!("open dispatch lease {}", path.display()))?;
        set_private_file_permissions(path)?;
        try_lock_file_exclusive(&file).map(|acquired| acquired.then_some(Self { _file: file }))
    }
}

struct JobLock {
    _file: File,
}

impl JobLock {
    fn exclusive(path: &Path) -> Result<Self> {
        Self::open(path, true)
    }

    fn shared(path: &Path) -> Result<Self> {
        Self::open(path, false)
    }

    fn open(path: &Path, exclusive: bool) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .with_context(|| format!("open dispatch job lock {}", path.display()))?;
        set_private_file_permissions(path)?;
        if exclusive {
            FileLock::exclusive(&file)?;
        } else {
            FileLock::shared(&file)?;
        }
        Ok(Self { _file: file })
    }
}

struct FileLock;

impl FileLock {
    fn exclusive(file: &File) -> Result<Self> {
        lock_file(file, true)?;
        Ok(Self)
    }

    fn shared(file: &File) -> Result<Self> {
        lock_file(file, false)?;
        Ok(Self)
    }
}

fn validate_id(field: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || value == "."
        || value == ".."
    {
        bail!(
            "{field} must be 1-128 ASCII letters, digits, '.', '_' or '-' without path separators"
        );
    }
    Ok(())
}

fn submit_intent_fingerprint(request: &DispatchSubmitRequest) -> Result<String> {
    use sha2::{Digest, Sha256};
    let encoded = serde_json::to_vec(request).context("encode dispatch submit intent")?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("decode {}", path.display()))
}

fn read_event_log_header(file: &mut File, path: &Path) -> Result<(EventLogHeader, u64)> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(64);
    let mut next = [0_u8; 1];
    loop {
        if bytes.len() >= 4 * 1024 {
            bail!("dispatch event log header is too large: {}", path.display());
        }
        let read = file.read(&mut next)?;
        if read == 0 {
            bail!(
                "dispatch event log header is incomplete: {}",
                path.display()
            );
        }
        if next[0] == b'\n' {
            break;
        }
        bytes.push(next[0]);
    }
    let header = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode dispatch event log header {}", path.display()))?;
    Ok((header, bytes.len() as u64 + 1))
}

fn atomic_write_event_log(path: &Path, cursor_base: u64, event: Option<&[u8]>) -> Result<()> {
    let mut bytes = serde_json::to_vec(&EventLogHeader { cursor_base })
        .context("encode dispatch event log header")?;
    bytes.push(b'\n');
    if let Some(event) = event {
        bytes.extend_from_slice(event);
        bytes.push(b'\n');
    }
    atomic_write(path, &bytes)
}

fn truncate_incomplete_event_tail(file: &mut File) -> Result<u64> {
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(0);
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0_u8; 1];
    file.read_exact(&mut last)?;
    if last[0] == b'\n' {
        return Ok(len);
    }

    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(len.min(DEFAULT_MAX_EVENTS_BYTES) as usize);
    file.read_to_end(&mut bytes)?;
    let retained = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    file.set_len(retained as u64)?;
    file.seek(SeekFrom::Start(retained as u64))?;
    file.sync_data()?;
    Ok(retained as u64)
}

fn atomic_write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).context("encode dispatch state")?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("dispatch state path has no parent: {}", path.display()))?;
    create_private_dir(parent)?;
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("dispatch"),
        uuid::Uuid::new_v4()
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .with_context(|| format!("create temporary dispatch file {}", temp.display()))?;
        set_private_file_permissions(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)
            .with_context(|| format!("publish dispatch file {}", path.display()))?;
        set_private_file_permissions(path)?;
        sync_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        remove_file_if_present(&temp);
    }
    result
}

fn ensure_private_file(path: &Path) -> Result<()> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("create dispatch file {}", path.display()))?;
    drop(file);
    set_private_file_permissions(path)
}

fn create_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("set private permissions on {}", path.display()))?;
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("set private permissions on {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)
            .with_context(|| format!("open dispatch directory {}", path.display()))?
            .sync_all()
            .with_context(|| format!("sync dispatch directory {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn remove_file_if_present(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("Failed to remove dispatch file {}: {error}", path.display());
        }
    }
}

#[cfg(unix)]
fn lock_file(file: &File, exclusive: bool) -> Result<()> {
    use std::os::fd::AsRawFd;
    let operation = if exclusive {
        libc::LOCK_EX
    } else {
        libc::LOCK_SH
    };
    // SAFETY: flock only operates on this live file descriptor.
    if unsafe { libc::flock(file.as_raw_fd(), operation) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error()).context("lock dispatch file")
    }
}

#[cfg(unix)]
fn try_lock_file_exclusive(file: &File) -> Result<bool> {
    use std::os::fd::AsRawFd;
    // SAFETY: flock only operates on this live file descriptor.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::WouldBlock {
        Ok(false)
    } else {
        Err(error).context("try lock dispatch file")
    }
}

#[cfg(not(unix))]
fn lock_file(_file: &File, _exclusive: bool) -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn try_lock_file_exclusive(_file: &File) -> Result<bool> {
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::protocol::{DispatchApprovalPolicy, DispatchSubmitRequest};

    fn request(job_id: &str) -> DispatchSubmitRequest {
        DispatchSubmitRequest {
            protocol_version: DISPATCH_PROTOCOL_VERSION,
            job_id: job_id.to_string(),
            session_id: format!("session-{job_id}"),
            workspace_path: "/tmp/workspace".to_string(),
            agent_type: "agentic".to_string(),
            prompt: "do the work".to_string(),
            approval_policy: DispatchApprovalPolicy::RejectAndReport,
            model: Some("model-1".to_string()),
            title: None,
        }
    }

    fn store() -> (tempfile::TempDir, DispatchStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(dir.path().join("dispatch")).expect("store");
        (dir, store)
    }

    #[test]
    fn event_cursor_is_monotonic_and_does_not_replay() {
        let (_dir, store) = store();
        store
            .create_job(request("job-1"), "Task".to_string())
            .expect("create job");

        let first = store.read_events("job-1", 0).expect("first page");
        assert_eq!(first.events.len(), 2);
        assert!(first.cursor > 0);

        store
            .append_event(
                "job-1",
                &DispatchEvent::job_state(DispatchJobState::Running, Some("started".to_string())),
            )
            .expect("append");
        let second = store
            .read_events("job-1", first.cursor)
            .expect("second page");
        assert_eq!(second.events.len(), 1);
        assert!(second.cursor > first.cursor);

        let empty = store
            .read_events("job-1", second.cursor)
            .expect("empty page");
        assert!(empty.events.is_empty());
        assert_eq!(empty.cursor, second.cursor);
    }

    #[test]
    fn incomplete_trailing_event_is_retried_after_crash_recovery() {
        let (_dir, store) = store();
        store
            .create_job(request("job-2"), "Task".to_string())
            .expect("create job");
        let initial = store.read_events("job-2", 0).expect("initial page");
        let path = store.job_dir("job-2").expect("job dir").join(EVENTS_FILE);
        let mut file = OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open events");
        file.write_all(br#"{"type":"jobState","timestamp":"partial""#)
            .expect("write partial line");
        file.sync_all().expect("sync partial line");

        let page = store
            .read_events("job-2", initial.cursor)
            .expect("read after partial write");
        assert!(page.events.is_empty());
        assert_eq!(page.cursor, initial.cursor);

        store
            .append_event(
                "job-2",
                &DispatchEvent::job_state(DispatchJobState::Running, Some("recovered".to_string())),
            )
            .expect("append after partial write");
        let recovered = store
            .read_events("job-2", initial.cursor)
            .expect("read recovered event");
        assert_eq!(recovered.events.len(), 1);
        assert!(matches!(
            &recovered.events[0],
            DispatchEvent::JobState {
                state: DispatchJobState::Running,
                ..
            }
        ));
        assert!(recovered.cursor > initial.cursor);
    }

    #[test]
    fn terminal_state_is_idempotent() {
        let (_dir, store) = store();
        store
            .create_job(request("job-3"), "Task".to_string())
            .expect("create job");
        let (succeeded, changed) = store
            .mark_state("job-3", DispatchJobState::Succeeded, Some("turn-1"), None)
            .expect("succeed");
        assert!(changed);
        assert_eq!(succeeded.state, DispatchJobState::Succeeded);

        let (still_succeeded, changed) = store
            .mark_state(
                "job-3",
                DispatchJobState::Failed,
                Some("turn-1"),
                Some("late failure".to_string()),
            )
            .expect("late terminal update");
        assert!(!changed);
        assert_eq!(still_succeeded.state, DispatchJobState::Succeeded);
        assert!(still_succeeded.last_error.is_none());
    }

    #[test]
    fn worker_exit_settlement_observes_cancel_request_under_the_state_lock() {
        let (_dir, store) = store();
        store
            .create_job(request("job-cancel-exit"), "Task".to_string())
            .expect("create cancelled job");
        store
            .request_cancel("job-cancel-exit")
            .expect("request cancellation");
        assert_eq!(
            store
                .settle_exited_worker("job-cancel-exit")
                .expect("settle cancelled worker")
                .state,
            DispatchJobState::Cancelled
        );

        store
            .create_job(request("job-crash-exit"), "Task".to_string())
            .expect("create crashed job");
        let crashed = store
            .settle_exited_worker("job-crash-exit")
            .expect("settle crashed worker");
        assert_eq!(crashed.state, DispatchJobState::Failed);
        assert!(crashed.last_error.is_some());
    }

    #[test]
    fn duplicate_submit_is_idempotent_but_conflicts_fail() {
        let (_dir, store) = store();
        let original = request("job-4");
        assert!(matches!(
            store
                .create_job(original.clone(), "Task".to_string())
                .expect("first"),
            CreateJobOutcome::Created(_)
        ));
        assert!(matches!(
            store
                .create_job(original.clone(), "Task".to_string())
                .expect("duplicate"),
            CreateJobOutcome::Existing(_)
        ));

        let mut conflicting = original;
        conflicting.prompt = "different task".to_string();
        assert!(store.create_job(conflicting, "Task".to_string()).is_err());
    }

    #[test]
    fn retry_rebuilds_uncommitted_partial_job_before_publishing_record() {
        let (_dir, store) = store();
        let request = request("job-partial-create");
        let job_dir = store.job_dir("job-partial-create").expect("job dir");
        create_private_dir(&job_dir).expect("partial job dir");
        atomic_write(&job_dir.join(STATE_FILE), b"{\"state\":").expect("partial state artifact");
        ensure_private_file(&job_dir.join(EVENTS_LOCK_FILE)).expect("partial event lock");
        atomic_write(
            &job_dir.join(EVENTS_FILE),
            b"{\"cursorBase\":0}\n{\"type\":",
        )
        .expect("partial event artifact");
        assert!(!job_dir.join(JOB_RECORD_FILE).exists());
        assert!(
            store.load_state("job-partial-create").is_err(),
            "status must not consume an uncommitted partial state"
        );
        assert!(
            store.read_events("job-partial-create", 0).is_err(),
            "status must not consume an uncommitted partial event stream"
        );
        assert!(
            store.request_cancel("job-partial-create").is_err(),
            "cancel must not mutate an uncommitted partial job"
        );
        assert!(
            store
                .load_existing_job_for_intent(&request)
                .expect("lookup uncommitted partial job")
                .is_none(),
            "an exact retry must be allowed to rebuild an uncommitted partial job"
        );

        let outcome = store
            .create_job(request, "Recovered task".to_string())
            .expect("retry partial initialization");
        assert!(matches!(outcome, CreateJobOutcome::Created(_)));
        assert!(
            job_dir.join(JOB_RECORD_FILE).is_file(),
            "job record is published only after the artifacts are rebuilt"
        );
        assert_eq!(
            store
                .load_state("job-partial-create")
                .expect("recovered state")
                .state,
            DispatchJobState::Queued
        );
        let events = store
            .read_events("job-partial-create", 0)
            .expect("recovered events");
        assert_eq!(events.events.len(), 2);
        assert!(matches!(
            events.events.first(),
            Some(DispatchEvent::Audit { action, .. }) if action == "approvalPolicySelected"
        ));
    }

    #[test]
    fn raw_submit_intent_remains_idempotent_when_resolution_changes() {
        let (_dir, store) = store();
        let mut intent = request("job-stable-intent");
        intent.model = None;
        intent.title = None;
        intent.workspace_path = "/symbolic/workspace".to_string();
        let mut resolved_a = intent.clone();
        resolved_a.model = Some("model-a".to_string());
        resolved_a.title = Some("Generated title A".to_string());
        resolved_a.workspace_path = "/canonical/workspace-a".to_string();
        store
            .create_job_with_intent(intent.clone(), resolved_a, "Generated title A".to_string())
            .expect("first resolved submit");

        let mut resolved_b = intent.clone();
        resolved_b.model = Some("model-b".to_string());
        resolved_b.title = Some("Generated title B".to_string());
        resolved_b.workspace_path = "/canonical/workspace-b".to_string();
        assert!(matches!(
            store
                .create_job_with_intent(intent.clone(), resolved_b, "Generated title B".to_string())
                .expect("same raw intent"),
            CreateJobOutcome::Existing(_)
        ));
        let (record, _) = store
            .load_existing_job_for_intent(&intent)
            .expect("lookup")
            .expect("existing job");
        assert_eq!(record.request.model.as_deref(), Some("model-a"));
        assert_eq!(record.request.workspace_path, "/canonical/workspace-a");

        let mut conflicting_intent = intent;
        conflicting_intent.prompt = "different task".to_string();
        assert!(store
            .load_existing_job_for_intent(&conflicting_intent)
            .is_err());
    }

    #[test]
    fn queued_job_spawn_claim_recovers_after_controller_loss() {
        let (_dir, store) = store();
        store
            .create_job(request("job-spawn-retry"), "Task".to_string())
            .expect("create job");

        let first = store
            .try_claim_worker_spawn("job-spawn-retry")
            .expect("first claim")
            .expect("claim available");
        assert!(
            store
                .try_claim_worker_spawn("job-spawn-retry")
                .expect("contended claim")
                .is_none(),
            "a concurrent idempotent submit must not spawn twice"
        );
        drop(first);
        assert!(
            store
                .try_claim_worker_spawn("job-spawn-retry")
                .expect("recovery claim")
                .is_some(),
            "the OS lock must release after controller loss so a retry can recover the queued job"
        );
    }

    #[test]
    fn worker_lease_allows_only_one_executor_per_job() {
        let (_dir, store) = store();
        store
            .create_job(request("job-worker-lease"), "Task".to_string())
            .expect("create job");

        let first = store
            .try_acquire_worker_lease("job-worker-lease")
            .expect("first lease")
            .expect("lease available");
        assert!(store
            .try_acquire_worker_lease("job-worker-lease")
            .expect("contended lease")
            .is_none());
        drop(first);
        assert!(store
            .try_acquire_worker_lease("job-worker-lease")
            .expect("released lease")
            .is_some());
    }

    #[test]
    fn first_event_audits_only_the_explicit_approval_policy() {
        let (_dir, store) = store();
        store
            .create_job(request("job-audit"), "Task".to_string())
            .expect("create job");
        let page = store.read_events("job-audit", 0).expect("events");
        let DispatchEvent::Audit {
            action, details, ..
        } = &page.events[0]
        else {
            panic!("first event must be an audit row");
        };
        assert_eq!(action, "approvalPolicySelected");
        assert_eq!(details["approvalPolicy"], "reject-and-report");
        assert!(details.get("prompt").is_none());
    }

    #[test]
    fn cursor_beyond_the_file_resets_to_the_retained_prefix() {
        let (_dir, store) = store();
        store
            .create_job(request("job-5"), "Task".to_string())
            .expect("create job");
        let page = store.read_events("job-5", u64::MAX).expect("reset page");
        assert!(page.cursor_reset);
        assert_eq!(page.events.len(), 2);
    }

    #[test]
    fn atomic_rotation_resets_old_cursors_and_keeps_terminal_state_visible() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store =
            DispatchStore::open_with_event_limit(dir.path().join("dispatch"), 512).expect("store");
        store
            .create_job(request("job-6"), "Task".to_string())
            .expect("create job");
        let before = store.read_events("job-6", 0).expect("before rotation");
        let job_dir = store.job_dir("job-6").expect("job dir");
        let events_path = job_dir.join(EVENTS_FILE);
        store
            .append_event(
                "job-6",
                &DispatchEvent::job_state(
                    DispatchJobState::Running,
                    Some(format!("rotation-event-{}", "x".repeat(320))),
                ),
            )
            .expect("rotate event log");
        let rotated = store.read_events("job-6", 0).expect("after rotation");
        assert!(rotated.cursor_reset);
        assert!(rotated.cursor > before.cursor);
        assert_eq!(rotated.events.len(), 1);

        let mut file = File::open(&events_path).expect("open rotated log");
        let (header, data_start) =
            read_event_log_header(&mut file, &events_path).expect("coherent header");
        assert!(header.cursor_base >= before.cursor);
        assert!(file.metadata().expect("metadata").len() > data_start);

        // A crash before the final rename may leave an unpublished temporary
        // file, but readers continue to observe the complete active artifact.
        let active_bytes = fs::read(&events_path).expect("active rotated log");
        fs::write(job_dir.join(".events.crash.tmp"), b"incomplete replacement")
            .expect("simulate pre-publish crash");
        assert_eq!(
            fs::read(&events_path).expect("active log after simulated crash"),
            active_bytes
        );
        let caught_up = store
            .read_events("job-6", rotated.cursor)
            .expect("read after simulated crash");
        assert!(caught_up.events.is_empty());
        assert_eq!(caught_up.cursor, rotated.cursor);

        store
            .mark_state("job-6", DispatchJobState::Succeeded, Some("turn-1"), None)
            .expect("write terminal state after rotation");
        let terminal = store.read_events("job-6", 0).expect("terminal page");
        assert!(terminal.cursor_reset);
        assert!(terminal.events.iter().any(|event| matches!(
            event,
            DispatchEvent::JobState {
                state: DispatchJobState::Succeeded,
                ..
            }
        )));
    }

    #[cfg(unix)]
    #[test]
    fn cross_process_reader_and_writer_remain_consistent_during_rotation() {
        const MODE_ENV: &str = "BITFUN_DISPATCH_ROTATION_STRESS_MODE";
        const ROOT_ENV: &str = "BITFUN_DISPATCH_ROTATION_STRESS_ROOT";
        const DONE_ENV: &str = "BITFUN_DISPATCH_ROTATION_STRESS_DONE";

        if let Some(mode) = std::env::var_os(MODE_ENV) {
            let root = PathBuf::from(std::env::var_os(ROOT_ENV).expect("stress root"));
            let done = PathBuf::from(std::env::var_os(DONE_ENV).expect("stress done"));
            let store = DispatchStore::open_with_event_limit(root, 4 * 1024).expect("child store");
            match mode.to_string_lossy().as_ref() {
                "writer" => {
                    for index in 0..240 {
                        store
                            .append_event(
                                "job-stress",
                                &DispatchEvent::job_state(
                                    DispatchJobState::Running,
                                    Some(format!("{index}:{}", "x".repeat(512))),
                                ),
                            )
                            .expect("stress append");
                    }
                    fs::write(done, b"done\n").expect("publish writer completion");
                }
                "reader" => {
                    let mut cursor = 0_u64;
                    let mut empty_after_done = 0_u8;
                    for _ in 0..10_000 {
                        let page = store
                            .read_events("job-stress", cursor)
                            .expect("stress read");
                        assert!(page.cursor >= cursor, "absolute cursor must not regress");
                        cursor = page.cursor;
                        if done.exists() && page.events.is_empty() {
                            empty_after_done += 1;
                            if empty_after_done >= 3 {
                                return;
                            }
                        } else {
                            empty_after_done = 0;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    panic!("reader did not drain the rotated log");
                }
                other => panic!("unexpected stress mode {other}"),
            }
            return;
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("dispatch");
        let done = dir.path().join("writer.done");
        DispatchStore::open_with_event_limit(root.clone(), 4 * 1024)
            .expect("parent store")
            .create_job(request("job-stress"), "Task".to_string())
            .expect("create stress job");
        let executable = std::env::current_exe().expect("test executable");
        let test_name =
            "dispatch::store::tests::cross_process_reader_and_writer_remain_consistent_during_rotation";
        let mut reader = std::process::Command::new(&executable)
            .args(["--exact", test_name, "--nocapture"])
            .env(MODE_ENV, "reader")
            .env(ROOT_ENV, &root)
            .env(DONE_ENV, &done)
            .spawn()
            .expect("spawn stress reader");
        let writer = std::process::Command::new(&executable)
            .args(["--exact", test_name, "--nocapture"])
            .env(MODE_ENV, "writer")
            .env(ROOT_ENV, &root)
            .env(DONE_ENV, &done)
            .output()
            .expect("run stress writer");
        let reader_status = reader.wait().expect("wait for stress reader");
        assert!(
            writer.status.success(),
            "writer failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&writer.stdout),
            String::from_utf8_lossy(&writer.stderr)
        );
        assert!(reader_status.success(), "stress reader failed");
    }

    #[test]
    fn status_pages_are_bounded_and_continue_from_the_returned_cursor() {
        let (_dir, store) = store();
        store
            .create_job(request("job-page"), "Task".to_string())
            .expect("create job");
        for index in 0..48 {
            store
                .append_event(
                    "job-page",
                    &DispatchEvent::job_state(
                        DispatchJobState::Running,
                        Some(format!("{index}:{}", "x".repeat(64 * 1024))),
                    ),
                )
                .expect("append page event");
        }
        let first = store.read_events("job-page", 0).expect("first page");
        assert!(first.events.len() < 50);
        assert!(first.cursor <= MAX_STATUS_PAGE_BYTES);
        assert!(
            serde_json::to_vec(&first.events)
                .expect("serialize status events")
                .len()
                <= MAX_STATUS_PAGE_BYTES as usize + 1
        );
        let second = store
            .read_events("job-page", first.cursor)
            .expect("second page");
        assert!(!second.events.is_empty());
        assert!(second.cursor > first.cursor);
    }

    #[test]
    fn oversized_single_event_is_replaced_without_failing_the_job_log() {
        let (_dir, store) = store();
        store
            .create_job(request("job-large-event"), "Task".to_string())
            .expect("create job");
        let before = store
            .read_events("job-large-event", 0)
            .expect("initial events");
        store
            .append_event(
                "job-large-event",
                &DispatchEvent::job_state(
                    DispatchJobState::Running,
                    Some("x".repeat(MAX_EVENT_BYTES)),
                ),
            )
            .expect("replace oversized event");
        let page = store
            .read_events("job-large-event", before.cursor)
            .expect("oversized marker");
        assert_eq!(page.events.len(), 1);
        let DispatchEvent::Audit {
            action, details, ..
        } = &page.events[0]
        else {
            panic!("oversized event must become an audit marker");
        };
        assert_eq!(action, "eventOmitted");
        assert_eq!(details["reason"], "eventTooLarge");
        assert_eq!(details["maxBytes"], MAX_EVENT_BYTES);
    }

    #[test]
    fn status_pages_cap_event_count_without_skipping_cursor_bytes() {
        let (_dir, store) = store();
        store
            .create_job(request("job-event-cap"), "Task".to_string())
            .expect("create job");
        for index in 0..1_023 {
            store
                .append_event(
                    "job-event-cap",
                    &DispatchEvent::job_state(
                        DispatchJobState::Running,
                        Some(format!("event-{index}")),
                    ),
                )
                .expect("append event");
        }

        let first = store.read_events("job-event-cap", 0).expect("first page");
        assert_eq!(first.events.len(), MAX_STATUS_PAGE_EVENTS);
        let second = store
            .read_events("job-event-cap", first.cursor)
            .expect("second page");
        assert_eq!(second.events.len(), MAX_STATUS_PAGE_EVENTS);
        let third = store
            .read_events("job-event-cap", second.cursor)
            .expect("third page");
        assert_eq!(third.events.len(), 1);
        let end = store
            .read_events("job-event-cap", third.cursor)
            .expect("end page");
        assert!(end.events.is_empty());
        assert!(first.cursor < second.cursor);
        assert!(second.cursor < third.cursor);
        assert_eq!(third.cursor, end.cursor);
        assert_eq!(
            first.events.len() + second.events.len() + third.events.len(),
            1_025,
            "the two initial events plus every appended event must be returned exactly once"
        );
    }

    #[test]
    fn default_store_honors_path_manager_storage_overrides() {
        const CHILD_ENV: &str = "BITFUN_DISPATCH_PATH_TEST_CHILD";
        if let Some(expected_home) = std::env::var_os(CHILD_ENV) {
            let store = DispatchStore::open_default().expect("open isolated default store");
            assert_eq!(store.root, PathBuf::from(expected_home).join("dispatch"));
            return;
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let bitfun_home = dir.path().join("bitfun-home");
        let user_root = dir.path().join("user-root");
        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "dispatch::store::tests::default_store_honors_path_manager_storage_overrides",
                "--nocapture",
            ])
            .env(CHILD_ENV, &bitfun_home)
            .env("BITFUN_HOME", &bitfun_home)
            .env("BITFUN_USER_ROOT", &user_root)
            .env("BITFUN_E2E_STORAGE_GUARD", "1")
            .env_remove("BITFUN_E2E_HOME")
            .env_remove("BITFUN_E2E_USER_ROOT")
            .output()
            .expect("run isolated path test");
        assert!(
            output.status.success(),
            "isolated child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(bitfun_home.join("dispatch/jobs").is_dir());
        assert!(bitfun_home.join("dispatch/workspaces").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn job_storage_uses_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, store) = store();
        store
            .create_job(request("job-private"), "Task".to_string())
            .expect("create job");
        let job_dir = store.job_dir("job-private").expect("job dir");
        assert_eq!(
            fs::metadata(&job_dir)
                .expect("job metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        for file in [JOB_RECORD_FILE, STATE_FILE, EVENTS_FILE, EVENTS_LOCK_FILE] {
            assert_eq!(
                fs::metadata(job_dir.join(file))
                    .expect("file metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
