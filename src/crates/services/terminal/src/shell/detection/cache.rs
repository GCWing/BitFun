use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

use dashmap::DashMap;

use super::{path, ShellType};

const FAILED_PROBE_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CandidateProbeOutcome {
    Available(Option<String>),
    AvailableWithProbeFailure,
    Unavailable,
}

#[derive(Clone, Debug, Eq)]
struct CandidateCacheKey {
    shell_type: ShellType,
    path_identity: String,
}

impl PartialEq for CandidateCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.shell_type == other.shell_type && self.path_identity == other.path_identity
    }
}

impl Hash for CandidateCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.shell_type.hash(state);
        self.path_identity.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileFingerprint {
    len: u64,
    modified: Option<(u64, u32)>,
}

#[derive(Clone, Debug)]
struct CachedCandidateProbe {
    fingerprint: Option<FileFingerprint>,
    outcome: CandidateProbeOutcome,
    retry_after: Option<Instant>,
}

type CandidateProbeSlot = Arc<Mutex<Option<CachedCandidateProbe>>>;

fn cache() -> &'static DashMap<CandidateCacheKey, CandidateProbeSlot> {
    static CACHE: OnceLock<DashMap<CandidateCacheKey, CandidateProbeSlot>> = OnceLock::new();
    CACHE.get_or_init(DashMap::new)
}

pub(super) fn probe_candidate(
    shell_type: &ShellType,
    executable: &Path,
    probe: impl FnOnce() -> CandidateProbeOutcome,
) -> CandidateProbeOutcome {
    let key = CandidateCacheKey {
        shell_type: shell_type.clone(),
        path_identity: path::normalized_path_identity(executable),
    };
    let slot = cache()
        .entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(None)))
        .clone();
    let mut cached = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let fingerprint = file_fingerprint(executable);
    let now = Instant::now();

    if let Some(entry) = cached.as_ref().filter(|entry| {
        entry.fingerprint == fingerprint
            && (matches!(&entry.outcome, CandidateProbeOutcome::Available(_))
                || entry
                    .retry_after
                    .is_some_and(|retry_after| retry_after > now))
    }) {
        return entry.outcome.clone();
    }

    let outcome = if fingerprint.is_some() {
        probe()
    } else {
        CandidateProbeOutcome::Unavailable
    };
    let retry_after = matches!(
        &outcome,
        CandidateProbeOutcome::AvailableWithProbeFailure | CandidateProbeOutcome::Unavailable
    )
    .then(|| now + FAILED_PROBE_CACHE_TTL);
    *cached = Some(CachedCandidateProbe {
        fingerprint,
        outcome: outcome.clone(),
        retry_after,
    });
    outcome
}

pub(super) fn invalidate_path(executable: &Path) {
    let path_identity = path::normalized_path_identity(executable);
    cache().retain(|key, _| key.path_identity != path_identity);
}

fn file_fingerprint(path: &Path) -> Option<FileFingerprint> {
    let metadata = path.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    let modified = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()));
    Some(FileFingerprint {
        len: metadata.len(),
        modified,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    use tempfile::tempdir;

    use super::{invalidate_path, probe_candidate, CandidateProbeOutcome};
    use crate::shell::ShellType;

    #[test]
    fn successful_probe_is_reused_until_the_file_changes() {
        let directory = tempdir().expect("temporary directory");
        let executable = directory.path().join("pwsh");
        fs::write(&executable, b"first").expect("write candidate");
        let probes = AtomicUsize::new(0);

        let first = probe_candidate(&ShellType::PowerShellCore, &executable, || {
            probes.fetch_add(1, Ordering::Relaxed);
            CandidateProbeOutcome::Available(Some("7.5.0".to_string()))
        });
        let cached = probe_candidate(&ShellType::PowerShellCore, &executable, || {
            probes.fetch_add(1, Ordering::Relaxed);
            CandidateProbeOutcome::Available(Some("unexpected".to_string()))
        });
        fs::write(&executable, b"second-version").expect("replace candidate");
        let refreshed = probe_candidate(&ShellType::PowerShellCore, &executable, || {
            probes.fetch_add(1, Ordering::Relaxed);
            CandidateProbeOutcome::Available(Some("7.6.0".to_string()))
        });
        invalidate_path(&executable);
        let invalidated = probe_candidate(&ShellType::PowerShellCore, &executable, || {
            probes.fetch_add(1, Ordering::Relaxed);
            CandidateProbeOutcome::Available(Some("7.7.0".to_string()))
        });

        assert_eq!(
            first,
            CandidateProbeOutcome::Available(Some("7.5.0".to_string()))
        );
        assert_eq!(cached, first);
        assert_eq!(
            refreshed,
            CandidateProbeOutcome::Available(Some("7.6.0".to_string()))
        );
        assert_eq!(
            invalidated,
            CandidateProbeOutcome::Available(Some("7.7.0".to_string()))
        );
        assert_eq!(probes.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn failed_probe_is_short_cached_but_file_changes_retry_immediately() {
        let directory = tempdir().expect("temporary directory");
        let executable = directory.path().join("bash");
        fs::write(&executable, b"first").expect("write candidate");
        let probes = AtomicUsize::new(0);

        assert_eq!(
            probe_candidate(&ShellType::Bash, &executable, || {
                probes.fetch_add(1, Ordering::Relaxed);
                CandidateProbeOutcome::Unavailable
            }),
            CandidateProbeOutcome::Unavailable
        );
        assert_eq!(
            probe_candidate(&ShellType::Bash, &executable, || {
                probes.fetch_add(1, Ordering::Relaxed);
                CandidateProbeOutcome::Available(None)
            }),
            CandidateProbeOutcome::Unavailable
        );
        fs::write(&executable, b"second-version").expect("replace candidate");
        assert_eq!(
            probe_candidate(&ShellType::Bash, &executable, || {
                probes.fetch_add(1, Ordering::Relaxed);
                CandidateProbeOutcome::Available(None)
            }),
            CandidateProbeOutcome::Available(None)
        );
        assert_eq!(probes.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn concurrent_requests_share_one_probe() {
        let directory = tempdir().expect("temporary directory");
        let executable = directory.path().join("concurrent-pwsh");
        fs::write(&executable, b"candidate").expect("write candidate");
        let probes = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(3));

        let handles = (0..2)
            .map(|_| {
                let executable = executable.clone();
                let probes = Arc::clone(&probes);
                let start = Arc::clone(&start);
                thread::spawn(move || {
                    start.wait();
                    probe_candidate(&ShellType::PowerShellCore, &executable, || {
                        probes.fetch_add(1, Ordering::Relaxed);
                        thread::sleep(Duration::from_millis(50));
                        CandidateProbeOutcome::Available(Some("7.5.0".to_string()))
                    })
                })
            })
            .collect::<Vec<_>>();
        start.wait();

        for handle in handles {
            assert_eq!(
                handle.join().expect("probe thread"),
                CandidateProbeOutcome::Available(Some("7.5.0".to_string()))
            );
        }
        assert_eq!(probes.load(Ordering::Relaxed), 1);
    }
}
