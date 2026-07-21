//! Process-tree supervision for executable integration runtimes.
//!
//! This provides failure containment and deterministic cleanup for managed
//! descendants. It is not an OS sandbox and does not establish a trust
//! boundary for child-process IO. On Unix, the boundary is one process group;
//! a program that deliberately creates a new session/process group escapes it.

use std::fmt;
use std::io;
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;
use tokio::process::ChildStderr;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupOutcome {
    AlreadyExited,
    #[cfg(unix)]
    Graceful,
    Forced,
}

pub struct ProcessTreeChild {
    child: Child,
    platform: PlatformProcessTree,
}

impl fmt::Debug for ProcessTreeChild {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessTreeChild")
            .field("process_id", &self.child.id())
            .finish_non_exhaustive()
    }
}

impl ProcessTreeChild {
    pub async fn spawn(command: &mut Command) -> io::Result<Self> {
        command.kill_on_drop(true);

        #[cfg(unix)]
        {
            command.process_group(0);
            let child = command.spawn()?;
            let process_group_id = child
                .id()
                .ok_or_else(|| io::Error::other("spawned process has no process id"))?
                as i32;
            Ok(Self {
                child,
                platform: PlatformProcessTree { process_group_id },
            })
        }

        #[cfg(windows)]
        {
            spawn_windows_process_tree(command).await
        }
    }

    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    pub async fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        self.child.wait().await
    }

    pub async fn terminate(&mut self, grace: Duration) -> io::Result<CleanupOutcome> {
        let parent_exited = self.child.try_wait()?.is_some();

        #[cfg(unix)]
        {
            if parent_exited && !process_group_is_alive(self.platform.process_group_id) {
                return Ok(CleanupOutcome::AlreadyExited);
            }
            terminate_unix_process_tree(&mut self.child, self.platform.process_group_id, grace)
                .await
        }

        #[cfg(windows)]
        {
            let _ = grace;
            let had_job = self.platform.job.take().is_some();
            if !parent_exited {
                self.child.wait().await?;
            }
            Ok(if had_job {
                CleanupOutcome::Forced
            } else {
                CleanupOutcome::AlreadyExited
            })
        }
    }

    /// Schedules best-effort graceful cleanup without requiring an async caller.
    /// If the cleanup thread or runtime cannot be created, `Drop` still performs
    /// immediate process-tree cleanup.
    pub fn spawn_cleanup(mut self, grace: Duration) {
        let _ = std::thread::Builder::new()
            .name("process-tree-cleanup".to_string())
            .spawn(move || {
                if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    let _ = runtime.block_on(self.terminate(grace));
                }
            });
    }
}

impl Drop for ProcessTreeChild {
    fn drop(&mut self) {
        let parent_exited = self.child.try_wait().is_ok_and(|status| status.is_some());

        #[cfg(unix)]
        {
            // SAFETY: negative PID targets the process group created by
            // `process_group(0)`; no borrowed memory crosses the FFI boundary.
            let _ = unsafe { libc::kill(-self.platform.process_group_id, libc::SIGKILL) };
        }
        #[cfg(windows)]
        {
            self.platform.job.take();
        }
        if !parent_exited {
            let _ = self.child.start_kill();
        }
    }
}

#[cfg(unix)]
struct PlatformProcessTree {
    process_group_id: i32,
}

#[cfg(unix)]
async fn terminate_unix_process_tree(
    child: &mut Child,
    process_group_id: i32,
    grace: Duration,
) -> io::Result<CleanupOutcome> {
    signal_process_group(process_group_id, libc::SIGTERM)?;
    let deadline = Instant::now() + grace;
    loop {
        let parent_exited = child.try_wait()?.is_some();
        if !process_group_is_alive(process_group_id) {
            if !parent_exited {
                child.wait().await?;
            }
            return Ok(CleanupOutcome::Graceful);
        }
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    signal_process_group(process_group_id, libc::SIGKILL)?;
    child.wait().await?;
    Ok(CleanupOutcome::Forced)
}

#[cfg(unix)]
fn signal_process_group(process_group_id: i32, signal: i32) -> io::Result<()> {
    // SAFETY: the integer process-group id was captured from the spawned child
    // and the signal value is a libc constant.
    let result = unsafe { libc::kill(-process_group_id, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(unix)]
fn process_group_is_alive(process_group_id: i32) -> bool {
    // SAFETY: signal 0 performs an existence/permission check only.
    let result = unsafe { libc::kill(-process_group_id, 0) };
    result == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
struct PlatformProcessTree {
    job: Option<win32job::Job>,
}

#[cfg(windows)]
async fn spawn_windows_process_tree(command: &mut Command) -> io::Result<ProcessTreeChild> {
    use windows::Win32::System::Threading::CREATE_SUSPENDED;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let job = win32job::Job::create().map_err(job_error)?;
    let mut limits = win32job::ExtendedLimitInfo::new();
    limits.limit_kill_on_job_close();
    job.set_extended_limit_info(&limits).map_err(job_error)?;

    command.creation_flags(CREATE_SUSPENDED.0 | CREATE_NO_WINDOW);
    let mut child = command.spawn()?;
    let attach_result = (|| {
        let process_id = child
            .id()
            .ok_or_else(|| io::Error::other("spawned process has no process id"))?;
        let process_handle = child
            .raw_handle()
            .ok_or_else(|| io::Error::other("spawned process has no process handle"))?;
        job.assign_process(process_handle as isize)
            .map_err(job_error)?;
        resume_primary_thread(process_id)
    })();

    if let Err(error) = attach_result {
        drop(job);
        let _ = child.kill().await;
        return Err(error);
    }

    Ok(ProcessTreeChild {
        child,
        platform: PlatformProcessTree { job: Some(job) },
    })
}

#[cfg(windows)]
fn resume_primary_thread(process_id: u32) -> io::Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    // SAFETY: all handles are checked, closed on every branch, and the
    // THREADENTRY32 size is initialized as required by ToolHelp.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let result = (|| {
            let mut entry = THREADENTRY32 {
                dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
                ..Default::default()
            };
            Thread32First(snapshot, &mut entry)
                .map_err(|error| io::Error::other(error.to_string()))?;
            loop {
                if entry.th32OwnerProcessID == process_id {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID)
                        .map_err(|error| io::Error::other(error.to_string()))?;
                    let resumed = ResumeThread(thread);
                    let _ = CloseHandle(thread);
                    if resumed == u32::MAX {
                        return Err(io::Error::last_os_error());
                    }
                    return Ok(());
                }
                if Thread32Next(snapshot, &mut entry).is_err() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "suspended process primary thread was not found",
                    ));
                }
            }
        })();
        let _ = CloseHandle(snapshot);
        result
    }
}

#[cfg(windows)]
fn job_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{CleanupOutcome, ProcessTreeChild};
    use std::path::Path;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    use tokio::process::Command;

    #[tokio::test]
    async fn terminate_removes_the_parent_and_its_descendant() {
        let temporary = tempfile::tempdir().expect("create process-tree fixture directory");
        let pid_file = temporary.path().join("descendant.pid");
        let mut command = descendant_fixture(&pid_file);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut tree = ProcessTreeChild::spawn(&mut command)
            .await
            .expect("spawn attached process tree");
        let descendant_pid = wait_for_descendant_pid(&pid_file).await;
        assert!(process_is_alive(descendant_pid));

        let outcome = tree
            .terminate(Duration::from_millis(250))
            .await
            .expect("terminate process tree");

        #[cfg(unix)]
        assert!(matches!(
            outcome,
            CleanupOutcome::Graceful | CleanupOutcome::Forced
        ));
        #[cfg(windows)]
        assert_eq!(outcome, CleanupOutcome::Forced);
        wait_until_process_exits(descendant_pid).await;
    }

    #[tokio::test]
    async fn terminate_removes_descendant_after_parent_has_exited() {
        let temporary = tempfile::tempdir().expect("create process-tree fixture directory");
        let pid_file = temporary.path().join("orphaned-descendant.pid");
        let mut command = orphaned_descendant_fixture(&pid_file);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut tree = ProcessTreeChild::spawn(&mut command)
            .await
            .expect("spawn attached process tree");
        let descendant_pid = wait_for_descendant_pid(&pid_file).await;
        wait_until_parent_exits(&mut tree).await;
        assert!(process_is_alive(descendant_pid));

        tree.terminate(Duration::from_millis(250))
            .await
            .expect("terminate orphaned descendant");
        wait_until_process_exits(descendant_pid).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn detached_unix_descendant_is_explicitly_outside_the_process_group_boundary() {
        let temporary = tempfile::tempdir().expect("create detached process fixture directory");
        let pid_file = temporary.path().join("detached-descendant.pid");
        let executable = std::env::current_exe().expect("locate test executable");
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("\"$BITFUN_PROCESS_TREE_TEST_EXE\" --exact process_tree::tests::unix_detached_fixture_process --nocapture")
            .env("BITFUN_PROCESS_TREE_TEST_EXE", executable)
            .env("BITFUN_DETACHED_FIXTURE", "1")
            .env("BITFUN_DESCENDANT_PID_FILE", &pid_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut tree = ProcessTreeChild::spawn(&mut command)
            .await
            .expect("spawn process-group fixture");
        let descendant_pid = wait_for_descendant_pid(&pid_file).await;
        tree.terminate(Duration::from_millis(100))
            .await
            .expect("terminate managed process group");

        assert!(
            process_is_alive(descendant_pid),
            "a detached descendant must not be described as managed by the Unix process-group boundary"
        );
        // SAFETY: the PID was written by this test fixture and is cleaned up
        // immediately so the contract test never leaks the escaped process.
        let _ = unsafe { libc::kill(descendant_pid as i32, libc::SIGKILL) };
        wait_until_process_exits(descendant_pid).await;
    }

    #[cfg(unix)]
    #[test]
    fn unix_detached_fixture_process() {
        if std::env::var_os("BITFUN_DETACHED_FIXTURE").is_none() {
            return;
        }
        // SAFETY: the fixture is a single-threaded test subprocess created
        // solely to verify the documented process-group containment limit.
        assert!(
            unsafe { libc::setsid() } >= 0,
            "fixture must create a new session"
        );
        let pid_file = std::env::var("BITFUN_DESCENDANT_PID_FILE").expect("fixture PID file path");
        std::fs::write(pid_file, std::process::id().to_string())
            .expect("publish detached fixture PID");
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }

    #[cfg(windows)]
    fn descendant_fixture(pid_file: &Path) -> Command {
        let script = r#"$child = Start-Process -FilePath "$env:SystemRoot\System32\ping.exe" -ArgumentList '-t','127.0.0.1' -WindowStyle Hidden -PassThru; [IO.File]::WriteAllText($env:BITFUN_DESCENDANT_PID_FILE, [string]$child.Id); while ($true) { Start-Sleep -Seconds 60 }"#;
        let mut command = Command::new("powershell.exe");
        command
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(script)
            .env("BITFUN_DESCENDANT_PID_FILE", pid_file);
        command
    }

    #[cfg(windows)]
    fn orphaned_descendant_fixture(pid_file: &Path) -> Command {
        let script = r#"$child = Start-Process -FilePath "$env:SystemRoot\System32\ping.exe" -ArgumentList '-t','127.0.0.1' -WindowStyle Hidden -PassThru; [IO.File]::WriteAllText($env:BITFUN_DESCENDANT_PID_FILE, [string]$child.Id)"#;
        let mut command = Command::new("powershell.exe");
        command
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(script)
            .env("BITFUN_DESCENDANT_PID_FILE", pid_file);
        command
    }

    #[cfg(unix)]
    fn descendant_fixture(pid_file: &Path) -> Command {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 60 & echo $! > \"$BITFUN_DESCENDANT_PID_FILE\"; wait")
            .env("BITFUN_DESCENDANT_PID_FILE", pid_file);
        command
    }

    #[cfg(unix)]
    fn orphaned_descendant_fixture(pid_file: &Path) -> Command {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 60 & echo $! > \"$BITFUN_DESCENDANT_PID_FILE\"")
            .env("BITFUN_DESCENDANT_PID_FILE", pid_file);
        command
    }

    async fn wait_until_parent_exits(tree: &mut ProcessTreeChild) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if tree.try_wait().expect("query parent process").is_some() {
                return;
            }
            assert!(Instant::now() < deadline, "parent process did not exit");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn wait_for_descendant_pid(path: &Path) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Ok(pid) = raw.trim().parse() {
                    return pid;
                }
            }
            assert!(
                Instant::now() < deadline,
                "descendant PID was not published"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn wait_until_process_exits(pid: u32) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while process_is_alive(pid) {
            assert!(
                Instant::now() < deadline,
                "descendant process survived cleanup"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    #[cfg(windows)]
    fn process_is_alive(pid: u32) -> bool {
        std::process::Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(format!(
                "if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}"
            ))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(unix)]
    fn process_is_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}
