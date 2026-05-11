//! Unified process management to avoid Windows child process leaks

use std::io;
use std::process::Command;
use std::sync::LazyLock;
use tokio::process::{Child, Command as TokioCommand};
use tokio::time::Duration;
#[cfg(unix)]
use tokio::time::timeout;

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
        cmd
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
        cmd
    }

    #[cfg(not(windows))]
    cmd
}

#[cfg(unix)]
pub fn configure_process_group(command: &mut TokioCommand) {
    command.process_group(0);
}

#[cfg(not(unix))]
pub fn configure_process_group(_command: &mut TokioCommand) {}

#[cfg(unix)]
pub async fn terminate_child_process_tree(
    child: &mut Child,
    graceful_timeout: Duration,
) -> io::Result<()> {
    let pid = child.id();

    if let Some(pid) = pid {
        let process_group = format!("-{}", pid);
        let _ = create_tokio_command("kill")
            .arg("-TERM")
            .arg(&process_group)
            .status()
            .await;

        match timeout(graceful_timeout, child.wait()).await {
            Ok(wait_result) => return wait_result.map(|_| ()),
            Err(_) => {
                let _ = create_tokio_command("kill")
                    .arg("-KILL")
                    .arg(&process_group)
                    .status()
                    .await;
                return child.wait().await.map(|_| ());
            }
        }
    }

    child.start_kill()?;
    child.wait().await.map(|_| ())
}

#[cfg(windows)]
pub async fn terminate_child_process_tree(
    child: &mut Child,
    graceful_timeout: Duration,
) -> io::Result<()> {
    let pid = child.id();

    let _ = graceful_timeout;

    if let Some(pid) = pid {
        let _ = create_tokio_command("taskkill")
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .arg("/F")
            .status()
            .await;
        return child.wait().await.map(|_| ());
    }

    child.start_kill()?;
    child.wait().await.map(|_| ())
}

pub fn spawn_child_process_tree_cleanup(child: Child, graceful_timeout: Duration) {
    let _ = std::thread::Builder::new()
        .name("process-tree-cleanup".to_string())
        .spawn(move || {
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => {
                    runtime.block_on(async move {
                        let mut child = child;
                        let _ = terminate_child_process_tree(&mut child, graceful_timeout).await;
                    });
                }
                Err(_) => {
                    let mut child = child;
                    let _ = child.start_kill();
                }
            }
        });
}

pub fn cleanup_all_processes() {
    GLOBAL_PROCESS_MANAGER.cleanup_all();
}
