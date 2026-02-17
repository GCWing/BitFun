use anyhow::{Context, Result};
use std::fs;
use std::io;
use std::path::Path;

/// Estimated install size in bytes (~200MB for typical Tauri app with WebView)
pub const ESTIMATED_INSTALL_SIZE: u64 = 200 * 1024 * 1024;

/// Extract a zip archive to the target directory.
///
/// In production, the payload is embedded via `include_bytes!` or read from
/// a resource bundled next to the installer executable.
pub fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| "Failed to read zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let out_path = target_dir.join(file.mangled_name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&out_path)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

/// Copy application files from a source directory to the target.
///
/// Used during development or when the payload is pre-extracted beside the installer.
pub fn copy_directory(source: &Path, target: &Path) -> Result<u64> {
    let mut bytes_copied: u64 = 0;

    if !target.exists() {
        fs::create_dir_all(target)?;
    }

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest = target.join(entry.file_name());

        if file_type.is_dir() {
            bytes_copied += copy_directory(&entry.path(), &dest)?;
        } else {
            let size = entry.metadata()?.len();
            fs::copy(entry.path(), &dest)?;
            bytes_copied += size;
        }
    }

    Ok(bytes_copied)
}

