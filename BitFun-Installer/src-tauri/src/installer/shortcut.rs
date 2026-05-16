//! Windows shortcut (.lnk) creation for desktop and Start Menu.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::MAIN_APP_EXE;

const SHORTCUT_NAME: &str = "BitFun.lnk";
const LEGACY_START_MENU_DIR: &str = "BitFun";

/// Create a desktop shortcut for BitFun.
pub fn create_desktop_shortcut(install_path: &Path) -> Result<()> {
    let desktop = dirs::desktop_dir().with_context(|| "Cannot find Desktop directory")?;
    let shortcut_path = desktop.join(SHORTCUT_NAME);
    let exe_path = install_path.join(MAIN_APP_EXE);

    create_lnk(&shortcut_path, &exe_path, install_path)?;
    log::info!("Created desktop shortcut at {}", shortcut_path.display());
    Ok(())
}

/// Create a Start Menu shortcut for BitFun.
pub fn create_start_menu_shortcut(install_path: &Path) -> Result<()> {
    let start_menu = get_start_menu_dir()?;
    remove_legacy_start_menu_shortcut(&start_menu)?;
    let shortcut_path = start_menu.join(SHORTCUT_NAME);
    let exe_path = install_path.join(MAIN_APP_EXE);

    create_lnk(&shortcut_path, &exe_path, install_path)?;
    log::info!("Created Start Menu shortcut at {}", shortcut_path.display());
    Ok(())
}

/// Remove desktop shortcut.
pub fn remove_desktop_shortcut() -> Result<()> {
    if let Some(desktop) = dirs::desktop_dir() {
        let shortcut_path = desktop.join(SHORTCUT_NAME);
        if shortcut_path.exists() {
            std::fs::remove_file(&shortcut_path)?;
        }
    }
    Ok(())
}

/// Remove Start Menu shortcut, including the legacy folder layout.
pub fn remove_start_menu_shortcut() -> Result<()> {
    let start_menu = get_start_menu_dir()?;
    let shortcut_path = start_menu.join(SHORTCUT_NAME);
    if shortcut_path.exists() {
        std::fs::remove_file(&shortcut_path)?;
    }
    remove_legacy_start_menu_shortcut(&start_menu)?;
    Ok(())
}

/// Get the current user's Start Menu Programs directory.
fn get_start_menu_dir() -> Result<PathBuf> {
    let appdata =
        std::env::var("APPDATA").with_context(|| "APPDATA environment variable not set")?;
    Ok(PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs"))
}

fn remove_legacy_start_menu_shortcut(start_menu: &Path) -> Result<()> {
    let legacy_dir = start_menu.join(LEGACY_START_MENU_DIR);
    if legacy_dir.exists() {
        std::fs::remove_dir_all(&legacy_dir)?;
    }
    Ok(())
}

/// Create a .lnk shortcut file using the mslnk crate.
fn create_lnk(shortcut_path: &Path, target: &Path, _working_dir: &Path) -> Result<()> {
    let lnk = mslnk::ShellLink::new(target)
        .with_context(|| format!("Failed to create shell link for {}", target.display()))?;

    // Note: mslnk has limited API. For full control (icon, arguments, etc.),
    // consider using the windows crate with IShellLink COM interface.
    lnk.create_lnk(shortcut_path)
        .with_context(|| format!("Failed to write shortcut to {}", shortcut_path.display()))?;

    log::info!(
        "Created shortcut: {} -> {}",
        shortcut_path.display(),
        target.display()
    );
    Ok(())
}
