//! Unified process management to avoid Windows child process leaks

use std::process::Command;
use std::sync::LazyLock;
use tokio::process::Command as TokioCommand;

#[cfg(windows)]
use log::warn;

#[cfg(windows)]
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
use win32job::Job;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static GLOBAL_PROCESS_MANAGER: LazyLock<ProcessManager> = LazyLock::new(ProcessManager::new);

pub struct ProcessManager {
    #[cfg(windows)]
    job: Arc<Mutex<Option<Job>>>,
}

impl ProcessManager {
    fn new() -> Self {
        let manager = Self {
            #[cfg(windows)]
            job: Arc::new(Mutex::new(None)),
        };

        #[cfg(windows)]
        {
            if let Err(e) = manager.initialize_job() {
                warn!("Failed to initialize Windows Job object: {}", e);
            }
        }

        manager
    }

    #[cfg(windows)]
    fn initialize_job(&self) -> Result<(), Box<dyn std::error::Error>> {
        use win32job::{ExtendedLimitInfo, Job};

        let job = Job::create()?;

        // Terminate all child processes when the Job closes
        let mut info = ExtendedLimitInfo::new();
        info.limit_kill_on_job_close();
        job.set_extended_limit_info(&info)?;

        // Assign current process to Job so child processes inherit automatically
        if let Err(e) = job.assign_current_process() {
            warn!("Failed to assign current process to job: {}", e);
        }

        let mut job_guard = self.job.lock().map_err(|e| {
            std::io::Error::other(format!("Failed to lock process manager job mutex: {}", e))
        })?;
        *job_guard = Some(job);

        Ok(())
    }

    pub fn cleanup_all(&self) {
        #[cfg(windows)]
        {
            let mut job_guard = match self.job.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    warn!("Process manager job mutex was poisoned during cleanup, recovering lock");
                    poisoned.into_inner() as std::sync::MutexGuard<'_, Option<Job>>
                }
            };
            job_guard.take();
        }
    }
}

/// Create synchronous Command (Windows automatically adds CREATE_NO_WINDOW)
pub fn create_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    let cmd = Command::new(program.as_ref());

    #[cfg(windows)]
    {
        let mut cmd = cmd;
        cmd.creation_flags(CREATE_NO_WINDOW);
        return cmd;
    }

    #[cfg(not(windows))]
    cmd
}

/// Create Tokio async Command (Windows automatically adds CREATE_NO_WINDOW)
pub fn create_tokio_command<S: AsRef<std::ffi::OsStr>>(program: S) -> TokioCommand {
    let cmd = TokioCommand::new(program.as_ref());

    #[cfg(windows)]
    {
        let mut cmd = cmd;
        cmd.creation_flags(CREATE_NO_WINDOW);
        return cmd;
    }

    #[cfg(not(windows))]
    cmd
}

pub fn cleanup_all_processes() {
    GLOBAL_PROCESS_MANAGER.cleanup_all();
}
