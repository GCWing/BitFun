use std::path::{Path, PathBuf};

const DEPRECATION: &str = "Warning: `bitfun-cli` is deprecated; use `bitfun` instead.";

fn sibling_primary() -> std::io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    Ok(current.with_file_name(if cfg!(windows) {
        "bitfun.exe"
    } else {
        "bitfun"
    }))
}

#[cfg(unix)]
fn hand_off(primary: &Path) -> i32 {
    use std::os::unix::process::CommandExt;

    let error = std::process::Command::new(primary)
        .args(std::env::args_os().skip(1))
        .exec();
    eprintln!("Error: failed to launch {}: {error}", primary.display());
    1
}

#[cfg(windows)]
unsafe extern "system" fn keep_wrapper_alive(ctrl_type: u32) -> windows::core::BOOL {
    use windows::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT};

    (ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_BREAK_EVENT).into()
}

#[cfg(windows)]
fn hand_off(primary: &Path) -> i32 {
    use windows::Win32::System::Console::SetConsoleCtrlHandler;

    if let Err(error) = unsafe { SetConsoleCtrlHandler(Some(keep_wrapper_alive), true) } {
        eprintln!("Error: failed to initialize deprecated launcher: {error}");
        return 1;
    }

    match std::process::Command::new(primary)
        .args(std::env::args_os().skip(1))
        .status()
    {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("Error: failed to launch {}: {error}", primary.display());
            1
        }
    }
}

fn main() {
    eprintln!("{DEPRECATION}");
    let code = sibling_primary().map_or_else(
        |error| {
            eprintln!("Error: failed to locate bitfun: {error}");
            1
        },
        |primary| {
            if !primary.is_file() {
                eprintln!(
                    "Error: incomplete installation: {} is missing; install both `bitfun` and `bitfun-cli` from the same BitFun release.",
                    primary.display()
                );
                return 1;
            }
            hand_off(&primary)
        },
    );
    std::process::exit(code);
}
