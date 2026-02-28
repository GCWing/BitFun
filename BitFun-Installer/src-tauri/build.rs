use std::fs;
use std::fs::File;
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

fn main() {
    if let Err(err) = build_embedded_payload() {
        panic!("failed to build embedded payload: {err}");
    }

    tauri_build::build()
}

fn build_embedded_payload() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let payload_dir = manifest_dir.join("payload");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let out_zip = out_dir.join("embedded_payload.zip");

    println!("cargo:rerun-if-changed={}", payload_dir.display());

    let mut file_count = 0usize;
    if payload_dir.exists() && payload_dir.is_dir() {
        file_count = create_payload_zip(&payload_dir, &out_zip)?;
        emit_rerun_for_files(&payload_dir)?;
    } else {
        create_empty_zip(&out_zip)?;
    }

    let available = if file_count > 0 { "1" } else { "0" };
    println!("cargo:rustc-env=EMBEDDED_PAYLOAD_AVAILABLE={available}");
    println!("cargo:warning=embedded payload files: {file_count}");

    Ok(())
}

fn emit_rerun_for_files(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            emit_rerun_for_files(&path)?;
        }
    }
    Ok(())
}

fn create_empty_zip(out_zip: &Path) -> zip::result::ZipResult<()> {
    let file = File::create(out_zip)?;
    let mut zip = ZipWriter::new(file);
    zip.finish()?;
    Ok(())
}

fn create_payload_zip(payload_dir: &Path, out_zip: &Path) -> zip::result::ZipResult<usize> {
    let file = File::create(out_zip)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut file_count = 0usize;
    add_dir_to_zip(&mut zip, payload_dir, payload_dir, options, &mut file_count)?;

    zip.finish()?;
    Ok(file_count)
}

fn add_dir_to_zip<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    root: &Path,
    current: &Path,
    options: FileOptions,
    file_count: &mut usize,
) -> zip::result::ZipResult<()> {
    let mut entries = fs::read_dir(current)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(zip::result::ZipError::Io)?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .map_err(|_| zip::result::ZipError::FileNotFound)?;
        let rel_name = rel.to_string_lossy().replace('\\', "/");

        if path.is_dir() {
            zip.add_directory(format!("{rel_name}/"), options)?;
            add_dir_to_zip(zip, root, &path, options, file_count)?;
            continue;
        }

        zip.start_file(rel_name, options)?;
        let mut src = File::open(&path)?;
        let mut buf = Vec::new();
        src.read_to_end(&mut buf)?;
        zip.write_all(&buf)?;
        *file_count += 1;
    }

    Ok(())
}
