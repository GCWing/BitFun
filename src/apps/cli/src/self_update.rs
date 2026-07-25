use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};
use tar::Archive;

const GITHUB_MANIFEST: &str =
    "https://github.com/GCWing/BitFun/releases/latest/download/linux-binaries.json";
const OPENBITFUN_MANIFEST: &str = "https://openbitfun.com/release/linux-binaries.json";
const AUTO_CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
/// Hard ceiling on how long an automatic check may delay interactive startup.
const AUTO_UPDATE_BUDGET: Duration = Duration::from_secs(90);
const DEPRECATION_WARNING: &str = "Warning: `bitfun-cli` is deprecated; use `bitfun` instead.";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinuxBinariesManifest {
    schema_version: u32,
    version: String,
    platforms: std::collections::HashMap<String, LinuxPlatform>,
}

#[derive(Debug, Deserialize)]
struct LinuxPlatform {
    target: String,
    cli: ReleaseAsset,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseAsset {
    filename: String,
    url: String,
    sha256_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateOutcome {
    Current,
    /// `--check` found a newer release but was asked not to install it.
    Available,
    Updated,
    Unsupported,
}

pub(crate) async fn run_manual(check_only: bool) -> Result<UpdateOutcome> {
    let outcome = update_from_configured_sources(check_only).await?;
    match outcome {
        UpdateOutcome::Current => {
            println!("BitFun CLI is up to date ({}).", env!("CARGO_PKG_VERSION"))
        }
        // `try_source` already printed the available version and its source.
        UpdateOutcome::Available => println!("Run `bitfun update` to install it."),
        UpdateOutcome::Updated => println!(
            "BitFun CLI was updated successfully. Restart this command to use the new version."
        ),
        UpdateOutcome::Unsupported => println!(
            "BitFun CLI self-update supports official Linux x86_64/ARM64 archive installations."
        ),
    }
    Ok(outcome)
}

pub(crate) async fn maybe_run_automatic() {
    if !automatic_update_is_eligible() || !automatic_check_is_due() {
        return;
    }
    mark_automatic_check();
    // Interactive startup must never wait on the network longer than this, no
    // matter how slowly a mirror trickles the archive out.
    match tokio::time::timeout(AUTO_UPDATE_BUDGET, update_from_configured_sources(false)).await {
        Ok(Ok(UpdateOutcome::Updated)) => eprintln!(
            "BitFun CLI updated in the background. The new version will be used next time."
        ),
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::debug!("Automatic CLI update check failed: {error}"),
        Err(_) => tracing::debug!(
            "Automatic CLI update check exceeded {}s; continuing startup.",
            AUTO_UPDATE_BUDGET.as_secs()
        ),
    }
}

async fn update_from_configured_sources(check_only: bool) -> Result<UpdateOutcome> {
    let Some(platform_key) = current_platform_key() else {
        return Ok(UpdateOutcome::Unsupported);
    };
    let current_exe = std::env::current_exe().context("resolve current BitFun CLI executable")?;
    if is_development_binary(&current_exe) {
        return Ok(UpdateOutcome::Unsupported);
    }

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(120))
        .build()
        .context("build CLI updater HTTP client")?;
    let mut errors = Vec::new();

    for (source, manifest_url) in [
        ("GitHub", GITHUB_MANIFEST),
        ("openbitfun.com", OPENBITFUN_MANIFEST),
    ] {
        match try_source(
            &client,
            source,
            manifest_url,
            platform_key,
            &current_exe,
            check_only,
        )
        .await
        {
            Ok(Some(outcome)) => return Ok(outcome),
            Ok(None) => {}
            Err(error) => errors.push(format!("{source}: {error:#}")),
        }
    }

    Err(anyhow!(
        "CLI update failed from both configured sources: {}",
        errors.join("; ")
    ))
}

async fn try_source(
    client: &Client,
    source: &str,
    manifest_url: &str,
    platform_key: &str,
    current_exe: &Path,
    check_only: bool,
) -> Result<Option<UpdateOutcome>> {
    let manifest = client
        .get(manifest_url)
        .send()
        .await
        .with_context(|| format!("request {manifest_url}"))?
        .error_for_status()
        .with_context(|| format!("fetch {manifest_url}"))?
        .json::<LinuxBinariesManifest>()
        .await
        .with_context(|| format!("parse {manifest_url}"))?;
    if manifest.schema_version != 1 {
        return Err(anyhow!(
            "unsupported Linux binaries manifest schema {}",
            manifest.schema_version
        ));
    }
    if !is_newer_version(&manifest.version, env!("CARGO_PKG_VERSION")) {
        return Ok(Some(UpdateOutcome::Current));
    }
    if check_only {
        println!(
            "BitFun CLI {} is available from {} (current {}).",
            manifest.version,
            source,
            env!("CARGO_PKG_VERSION")
        );
        return Ok(Some(UpdateOutcome::Available));
    }

    let platform = manifest
        .platforms
        .get(platform_key)
        .ok_or_else(|| anyhow!("manifest does not contain {platform_key}"))?;
    let expected_target = match platform_key {
        "linux-x86_64" => "x86_64-unknown-linux-gnu",
        "linux-aarch64" => "aarch64-unknown-linux-gnu",
        _ => return Err(anyhow!("unsupported updater platform {platform_key}")),
    };
    if platform.target != expected_target {
        return Err(anyhow!(
            "manifest target {} does not match {}",
            platform.target,
            expected_target
        ));
    }
    if !platform.cli.filename.ends_with(".tar.gz") {
        return Err(anyhow!("CLI release asset is not a tar.gz archive"));
    }

    let archive = download_bytes(client, &platform.cli.url).await?;
    let checksum_text = download_text(client, &platform.cli.sha256_url).await?;
    verify_sha256(&archive, &checksum_text, &platform.cli.filename)?;
    install_archive(&archive, current_exe)?;
    restart_managed_daemon();
    println!("Updated from {source}: {}", manifest.version);
    Ok(Some(UpdateOutcome::Updated))
}

async fn download_bytes(client: &Client, url: &str) -> Result<Vec<u8>> {
    Ok(client
        .get(url)
        .send()
        .await
        .with_context(|| format!("request {url}"))?
        .error_for_status()
        .with_context(|| format!("download {url}"))?
        .bytes()
        .await
        .with_context(|| format!("read {url}"))?
        .to_vec())
}

async fn download_text(client: &Client, url: &str) -> Result<String> {
    client
        .get(url)
        .send()
        .await
        .with_context(|| format!("request {url}"))?
        .error_for_status()
        .with_context(|| format!("download {url}"))?
        .text()
        .await
        .with_context(|| format!("read {url}"))
}

fn verify_sha256(archive: &[u8], checksum_text: &str, filename: &str) -> Result<()> {
    let expected = checksum_text
        .split_whitespace()
        .next()
        .filter(|value| value.len() == 64)
        .ok_or_else(|| anyhow!("invalid SHA256 file for {filename}"))?;
    let actual = format!("{:x}", Sha256::digest(archive));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(anyhow!("SHA256 mismatch for {filename}"));
    }
    Ok(())
}

#[cfg(unix)]
fn install_archive(archive: &[u8], current_exe: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let install_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow!("current executable has no parent directory"))?;
    if current_exe.file_name().and_then(|name| name.to_str()) != Some("bitfun") {
        return Err(anyhow!(
            "self-update requires the official executable name `bitfun`"
        ));
    }
    let legacy_target = install_dir.join("bitfun-cli");
    if !legacy_target.is_file() {
        return Err(anyhow!(
            "official bitfun-cli companion was not found beside {}",
            current_exe.display()
        ));
    }

    let extract_dir = tempfile::tempdir().context("create CLI update extraction directory")?;
    Archive::new(GzDecoder::new(Cursor::new(archive)))
        .unpack(extract_dir.path())
        .context("extract CLI update archive")?;
    let package_dir = find_package_dir(extract_dir.path())?;
    let new_primary = package_dir.join("bitfun");
    let new_legacy = package_dir.join("bitfun-cli");
    validate_entrypoint_pair(&new_primary, &new_legacy)?;

    let stage = tempfile::Builder::new()
        .prefix(".bitfun-update.")
        .tempdir_in(install_dir)
        .with_context(|| {
            format!(
                "create update staging directory in {}",
                install_dir.display()
            )
        })?;
    let staged_primary = stage.path().join("bitfun");
    let staged_legacy = stage.path().join("bitfun-cli");
    fs::copy(&new_primary, &staged_primary).context("stage bitfun")?;
    fs::copy(&new_legacy, &staged_legacy).context("stage bitfun-cli")?;
    fs::set_permissions(&staged_primary, fs::Permissions::from_mode(0o755))?;
    fs::set_permissions(&staged_legacy, fs::Permissions::from_mode(0o755))?;
    validate_entrypoint_pair(&staged_primary, &staged_legacy)?;

    let primary_backup = stage.path().join("previous-bitfun");
    let legacy_backup = stage.path().join("previous-bitfun-cli");
    fs::rename(current_exe, &primary_backup).context("back up current bitfun")?;
    if let Err(error) = fs::rename(&legacy_target, &legacy_backup) {
        let _ = fs::rename(&primary_backup, current_exe);
        return Err(error).context("back up current bitfun-cli");
    }
    if let Err(error) = fs::rename(&staged_primary, current_exe) {
        let _ = fs::rename(&legacy_backup, &legacy_target);
        let _ = fs::rename(&primary_backup, current_exe);
        return Err(error).context("install updated bitfun");
    }
    if let Err(error) = fs::rename(&staged_legacy, &legacy_target) {
        let _ = fs::remove_file(current_exe);
        let _ = fs::rename(&legacy_backup, &legacy_target);
        let _ = fs::rename(&primary_backup, current_exe);
        return Err(error).context("install updated bitfun-cli");
    }
    if let Err(error) = validate_entrypoint_pair(current_exe, &legacy_target) {
        let _ = fs::remove_file(current_exe);
        let _ = fs::remove_file(&legacy_target);
        let _ = fs::rename(&legacy_backup, &legacy_target);
        let _ = fs::rename(&primary_backup, current_exe);
        return Err(error).context("validate installed CLI update");
    }
    Ok(())
}

#[cfg(not(unix))]
fn install_archive(_archive: &[u8], _current_exe: &Path) -> Result<()> {
    Err(anyhow!("CLI self-update is only available on Linux"))
}

fn find_package_dir(root: &Path) -> Result<PathBuf> {
    for entry in fs::read_dir(root).context("inspect CLI update archive")? {
        let path = entry?.path();
        if path.is_dir() && path.join("bitfun").is_file() && path.join("bitfun-cli").is_file() {
            return Ok(path);
        }
    }
    Err(anyhow!(
        "CLI update archive does not contain the official entrypoint pair"
    ))
}

fn validate_entrypoint_pair(primary: &Path, legacy: &Path) -> Result<()> {
    let primary_status = Command::new(primary)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("run {}", primary.display()))?;
    if !primary_status.success() {
        return Err(anyhow!("{} --version failed", primary.display()));
    }
    let legacy_output = Command::new(legacy)
        .arg("--version")
        .stdout(Stdio::null())
        .output()
        .with_context(|| format!("run {}", legacy.display()))?;
    if !legacy_output.status.success()
        || String::from_utf8_lossy(&legacy_output.stderr).trim() != DEPRECATION_WARNING
    {
        return Err(anyhow!("deprecated bitfun-cli entrypoint contract failed"));
    }
    Ok(())
}

fn current_platform_key() -> Option<&'static str> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    match std::env::consts::ARCH {
        "x86_64" => Some("linux-x86_64"),
        "aarch64" => Some("linux-aarch64"),
        _ => None,
    }
}

fn is_development_binary(executable: &Path) -> bool {
    executable
        .components()
        .any(|component| component.as_os_str() == "target")
}

fn is_newer_version(candidate: &str, current: &str) -> bool {
    fn core(version: &str) -> Option<(u64, u64, u64)> {
        let mut parts = version.split(['-', '+']).next()?.split('.');
        Some((
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        ))
    }
    matches!((core(candidate), core(current)), (Some(next), Some(now)) if next > now)
}

fn automatic_update_is_eligible() -> bool {
    if std::env::var_os("BITFUN_CLI_DISABLE_AUTO_UPDATE").is_some()
        || env!("CARGO_PKG_VERSION").contains("-nightly.")
    {
        return false;
    }
    std::env::current_exe()
        .ok()
        .is_some_and(|path| current_platform_key().is_some() && !is_development_binary(&path))
}

/// Share the CLI's own config directory so a relocated profile (E2E storage
/// guard, non-default home) does not silently re-check on every launch.
fn automatic_stamp_path() -> Option<PathBuf> {
    crate::config::CliConfig::config_dir()
        .ok()
        .map(|dir| dir.join("last-update-check"))
}

fn automatic_check_is_due() -> bool {
    let Some(path) = automatic_stamp_path() else {
        return false;
    };
    let Ok(modified) = fs::metadata(path).and_then(|metadata| metadata.modified()) else {
        return true;
    };
    SystemTime::now()
        .duration_since(modified)
        .map_or(true, |elapsed| elapsed >= AUTO_CHECK_INTERVAL)
}

fn mark_automatic_check() {
    let Some(path) = automatic_stamp_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, env!("CARGO_PKG_VERSION"));
}

fn restart_managed_daemon() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let unit = home.join(".config/systemd/user/bitfun-cli-daemon.service");
    if unit.is_file() {
        let _ = Command::new("systemctl")
            .args(["--user", "try-restart", "bitfun-cli-daemon.service"])
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::{is_newer_version, verify_sha256};
    use sha2::{Digest, Sha256};

    #[test]
    fn version_comparison_ignores_release_metadata() {
        assert!(is_newer_version("0.2.14", "0.2.13"));
        assert!(!is_newer_version("0.2.13", "0.2.13-nightly.1+abc"));
        assert!(!is_newer_version("0.2.12", "0.2.13"));
    }

    #[test]
    fn checksum_contract_accepts_standard_sha_file() {
        let data = b"bitfun";
        let digest = format!("{:x}", Sha256::digest(data));
        verify_sha256(
            data,
            &format!("{digest}  archive.tar.gz\n"),
            "archive.tar.gz",
        )
        .unwrap();
        assert!(verify_sha256(
            data,
            &format!("{}  archive.tar.gz", "0".repeat(64)),
            "archive.tar.gz"
        )
        .is_err());
    }
}
