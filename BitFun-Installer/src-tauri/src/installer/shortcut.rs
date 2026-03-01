//! Windows shortcut (.lnk) creation for desktop and Start Menu.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Create a desktop shortcut for BitFun.
pub fn create_desktop_shortcut(install_path: &Path) -> Result<()> {
    let desktop = dirs::desktop_dir().with_context(|| "Cannot find Desktop directory")?;
    let shortcut_path = desktop.join("BitFun.lnk");
    let exe_path = install_path.join("BitFun.exe");

    create_lnk(&shortcut_path, &exe_path, install_path)?;
    log::info!("Created desktop shortcut at {}", shortcut_path.display());
    Ok(())
}

/// Create a Start Menu shortcut for BitFun.
pub fn create_start_menu_shortcut(install_path: &Path) -> Result<()> {
    let start_menu = get_start_menu_dir()?;
    let bitfun_folder = start_menu.join("BitFun");
    std::fs::create_dir_all(&bitfun_folder)?;

    let shortcut_path = bitfun_folder.join("BitFun.lnk");
    let exe_path = install_path.join("BitFun.exe");

    create_lnk(&shortcut_path, &exe_path, install_path)?;
    log::info!("Created Start Menu shortcut at {}", shortcut_path.display());
    Ok(())
}

/// Remove desktop shortcut.
pub fn remove_desktop_shortcut() -> Result<()> {
    if let Some(desktop) = dirs::desktop_dir() {
        let shortcut_path = desktop.join("BitFun.lnk");
        if shortcut_path.exists() {
            std::fs::remove_file(&shortcut_path)?;
        }
    }
    Ok(())
}

/// Remove Start Menu shortcut folder.
pub fn remove_start_menu_shortcut() -> Result<()> {
    let start_menu = get_start_menu_dir()?;
    let bitfun_folder = start_menu.join("BitFun");
    if bitfun_folder.exists() {
        std::fs::remove_dir_all(&bitfun_folder)?;
    }
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
