//! Materialize the bundled DeepSeek Harness bridge into the user's dsh install.
//!
//! OpenBitFun ships a compiled ACP bridge, but it does not ship a harness: `dsh` is
//! a profile launcher, and a profile is just a directory under
//! `$DSH_HOME/profiles/`. So instead of publishing the bridge to a registry, we
//! copy the built profile next to the harness the user already installed and
//! launch it with `dsh --profile openbitfun-acp`. Every `@deepseek-ai/dsh-*` row in
//! that profile resolves out of the user's own installation through the flat
//! `profiles/node_modules` fallback the launcher maintains, so nothing here
//! installs a second harness, and the user's model and credentials — which live
//! in dsh, not in OpenBitFun — apply unchanged.
//!
//! The copy is idempotent: the built profile carries a content digest, and a
//! destination already stamped with the same digest is left alone.
//!
//! A remote workspace gets the same treatment over its own transport — see
//! [`ensure_bundled_profile_remote`] — so "works on my machine" and "works on
//! the box I ssh into" are the same feature and not two.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use openbitfun_core::service::remote_ssh::SSHConnectionManager;
use openbitfun_core::util::errors::{OpenBitFunError, OpenBitFunResult};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::remote_shell::{remote_user_shell_command, render_remote_env_assignments, shell_escape};
use super::requirements::probe_executable;

/// Points directly at a built profile directory, overriding every other
/// candidate. This is how a packaged build that lays resources out unusually —
/// or a developer testing an unbuilt tree — names the source explicitly.
const SOURCE_DIR_ENV: &str = "OPENBITFUN_DSH_PROFILE_DIR";

/// dsh's own state directory variable. Respecting it matters: a user who moved
/// `$DSH_HOME` keeps their models and credentials there, and a profile written
/// under `~/.dsh` instead would simply not be found by the launcher.
const DSH_HOME_ENV: &str = "DSH_HOME";

/// Basename of the build stamp `scripts/build-profile.mjs` writes.
const STAMP_FILENAME: &str = ".openbitfun-bridge.json";

/// Directories the build owns end to end, replaced rather than merged.
///
/// Merging would leave a previous build's emit behind — a `lib/*.js` that no
/// longer exists in the source still resolves, and the profile boots a mix of
/// two versions. Everything outside these three is left untouched, so anything
/// the user added to the profile directory survives an upgrade.
const MANAGED_SUBDIRECTORIES: &[&str] = &["lib", "presets", "node_modules"];

/// What `scripts/build-profile.mjs` records about a build.
#[derive(Debug, Deserialize)]
struct BridgeStamp {
    /// Digest over every path and its bytes in the built profile. A version
    /// string would not do: during development the bridge version stands still
    /// while its code changes, and a stale profile would look current.
    content: String,
    /// The oldest `@deepseek-ai/dsh` this build is known to boot against.
    #[serde(rename = "minDshVersion")]
    min_dsh_version: String,
}

/// Copy the bundled profile into the user's dsh install if it is not there yet.
///
/// Returns the profile directory the launcher will boot. Errors are the ones
/// worth stopping for: no bundled profile to copy, a dsh too old to run it, or
/// a filesystem that refused the write — each of which would otherwise surface
/// as `dsh` complaining about a profile that does not exist, or as a pile of
/// module resolution failures with no hint about the cause.
pub(crate) async fn ensure_bundled_profile(profile: &str) -> OpenBitFunResult<PathBuf> {
    let (source, stamp) = bundled_build(profile)?;

    // Checked on every launch, not just when copying: the pairing can break
    // later by the user downgrading dsh under a profile that is already current.
    require_supported_dsh(&stamp.min_dsh_version).await?;

    let destination = dsh_profiles_directory()?.join(profile);
    if read_stamp(&destination)
        .ok()
        .flatten()
        .is_some_and(|installed| installed.content == stamp.content)
    {
        return Ok(destination);
    }

    let target = destination.clone();
    tokio::task::spawn_blocking(move || install_profile(&source, &target))
        .await
        .map_err(|error| {
            OpenBitFunError::service(format!(
                "Failed to install the DeepSeek Harness profile: {error}"
            ))
        })??;

    log::info!(
        "Installed the bundled DeepSeek Harness profile at {}",
        destination.display()
    );
    Ok(destination)
}

/// The built profile this OpenBitFun ships, and what it says about itself.
fn bundled_build(profile: &str) -> OpenBitFunResult<(PathBuf, BridgeStamp)> {
    let source = bundled_profile_source(profile).ok_or_else(|| {
        OpenBitFunError::service(format!(
            "The bundled DeepSeek Harness profile '{profile}' is missing from this OpenBitFun build. \
             Build it with `npm run build && node scripts/build-profile.mjs` in packages/dsh-acp, \
             or point {SOURCE_DIR_ENV} at a built profile directory."
        ))
    })?;
    let stamp = read_stamp(&source)?.ok_or_else(|| {
        OpenBitFunError::service(format!(
            "The bundled DeepSeek Harness profile at {} has no {STAMP_FILENAME}; it is not a \
             finished build.",
            source.display()
        ))
    })?;
    Ok((source, stamp))
}

/// Fail with an actionable message when the installed dsh predates this build.
///
/// A dsh we cannot find, or whose version we cannot parse, is not treated as a
/// failure here: a missing `dsh` already surfaces as an install prompt in the
/// agent list, and guessing about an unrecognized version string would block a
/// launch that may well work.
async fn require_supported_dsh(minimum: &str) -> OpenBitFunResult<()> {
    let Some(installed) = probe_executable("dsh").await.version else {
        return Ok(());
    };
    if version_is_supported(&installed, minimum) {
        return Ok(());
    }
    Err(OpenBitFunError::service(format!(
        "DeepSeek Harness {installed} is older than {minimum}, which this OpenBitFun build requires. \
         Update it with `npm install -g @deepseek-ai/dsh`."
    )))
}

/// The oldest Node the harness can boot on.
///
/// `@deepseek-ai/dsh-app-boot` imports `util.parseEnv` on its very first line,
/// and Node added that in 20.12. The harness declares no `engines` of its own,
/// so npm installs it happily onto anything and the mismatch only surfaces as a
/// `SyntaxError` from inside the launcher — which is not a sentence anyone can
/// act on. Its toolchain targets Node 22 LTS; this is the floor, not the
/// recommendation.
const MIN_NODE_VERSION: &str = "20.12.0";

/// The reported Node version, when it is too old to boot the harness.
///
/// Anything unparsable is not a verdict and passes: a host with no `node` on
/// the login shell's PATH may still resolve one for `dsh` itself, and a launch
/// that fails now says why on its own.
fn unsupported_node_version(reported: &str) -> Option<String> {
    let reported = reported.trim();
    let version = semver::Version::parse(reported.trim_start_matches('v')).ok()?;
    let minimum = semver::Version::parse(MIN_NODE_VERSION).ok()?;
    (version < minimum).then(|| reported.to_string())
}

/// Whether `installed` is new enough for a build that needs `minimum`.
///
/// Unparsable on either side means "assume yes" — see the caller.
fn version_is_supported(installed: &str, minimum: &str) -> bool {
    let Ok(minimum) = semver::Version::parse(minimum.trim_start_matches('v')) else {
        return true;
    };
    let Ok(installed) = semver::Version::parse(installed.trim().trim_start_matches('v')) else {
        return true;
    };
    installed >= minimum
}

/// Locate the built profile that ships with this OpenBitFun.
fn bundled_profile_source(profile: &str) -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os(SOURCE_DIR_ENV) {
        let configured = PathBuf::from(configured);
        return configured.is_dir().then_some(configured);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("resources").join("dsh-profile"));
            if let Some(parent) = directory.parent() {
                candidates.push(
                    parent
                        .join("Resources")
                        .join("resources")
                        .join("dsh-profile"),
                );
                candidates.push(parent.join("Resources").join("dsh-profile"));
                if let Some(name) = executable.file_name().and_then(|name| name.to_str()) {
                    candidates.push(
                        parent
                            .join("lib")
                            .join(name)
                            .join("resources")
                            .join("dsh-profile"),
                    );
                    candidates.push(
                        parent
                            .join("share")
                            .join(name)
                            .join("resources")
                            .join("dsh-profile"),
                    );
                }
            }
        }
    }
    // The development tree, where the build writes straight into the package.
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../packages/dsh-acp/dist-profile"),
    );

    candidates
        .into_iter()
        // A profile directory is only the right one if it is the named profile.
        .find(|candidate| {
            candidate.is_dir() && profile_name_of(candidate) == Some(profile.to_string())
        })
}

/// The profile a built directory declares itself to be.
fn profile_name_of(directory: &Path) -> Option<String> {
    #[derive(Deserialize)]
    struct NamedProfile {
        profile: String,
    }
    let contents = std::fs::read(directory.join(STAMP_FILENAME)).ok()?;
    serde_json::from_slice::<NamedProfile>(&contents)
        .ok()
        .map(|stamp| stamp.profile)
}

/// Read a build stamp, distinguishing "not installed" from "unreadable".
fn read_stamp(directory: &Path) -> OpenBitFunResult<Option<BridgeStamp>> {
    let path = directory.join(STAMP_FILENAME);
    let contents = match std::fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(OpenBitFunError::service(format!(
                "Failed to read {}: {error}",
                path.display()
            )))
        }
    };
    serde_json::from_slice(&contents)
        .map(Some)
        .map_err(|error| {
            OpenBitFunError::service(format!("Failed to parse {}: {error}", path.display()))
        })
}

/// `$DSH_HOME/profiles`, created if the user has not run dsh yet.
fn dsh_profiles_directory() -> OpenBitFunResult<PathBuf> {
    let home = match std::env::var_os(DSH_HOME_ENV) {
        Some(home) if !home.is_empty() => PathBuf::from(home),
        _ => dirs::home_dir()
            .ok_or_else(|| {
                OpenBitFunError::service(
                    "Cannot locate the home directory that holds the DeepSeek Harness install"
                        .to_string(),
                )
            })?
            .join(".dsh"),
    };
    Ok(home.join("profiles"))
}

/// Write the built profile into `destination`, stamp last.
///
/// The stamp is copied only after every other file lands, so an interrupted
/// install reads as absent rather than current and is simply redone.
fn install_profile(source: &Path, destination: &Path) -> OpenBitFunResult<()> {
    let write_error = |error: std::io::Error| {
        OpenBitFunError::service(format!(
            "Failed to install the DeepSeek Harness profile into {}: {error}",
            destination.display()
        ))
    };

    // A half-written stamp is the one file that must never be believed, so drop
    // it up front: everything below runs with the destination marked stale.
    match std::fs::remove_file(destination.join(STAMP_FILENAME)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(write_error(error)),
    }
    std::fs::create_dir_all(destination).map_err(write_error)?;
    for managed in MANAGED_SUBDIRECTORIES {
        match std::fs::remove_dir_all(destination.join(managed)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(write_error(error)),
        }
    }

    copy_tree(source, destination, true).map_err(write_error)?;
    std::fs::copy(
        source.join(STAMP_FILENAME),
        destination.join(STAMP_FILENAME),
    )
    .map_err(write_error)?;
    Ok(())
}

/// Copy `source` over `destination`, overwriting files and keeping extras.
///
/// `skip_stamp` holds for the top level only — the stamp is the install marker
/// and its own step.
fn copy_tree(source: &Path, destination: &Path, skip_stamp: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if skip_stamp && name == STAMP_FILENAME {
            continue;
        }
        let target = destination.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target, false)?;
        } else {
            // Remove first: overwriting in place would follow a symlink the
            // user (or a previous vendoring step) left behind.
            match std::fs::remove_file(&target) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

// --- Remote workspaces -------------------------------------------------------

/// Ceiling for the upload, so a stalled channel surfaces as an error instead of
/// a session that silently never starts.
const REMOTE_INSTALL_TIMEOUT: Duration = Duration::from_secs(180);

/// Put the bundled profile on the far side of a remote workspace.
///
/// Same contract as [`ensure_bundled_profile`], expressed over a shell instead
/// of a filesystem: a host already carrying this exact build is left alone. The
/// harness itself stays the user's — only the bridge travels, and it travels as
/// one tar stream over the same transport the session will use, which is what
/// makes this work for container connections where SFTP is not available.
///
/// Unlike the local path, a missing remote `dsh` is fatal here: there is no
/// install prompt in front of a remote workspace, so the only way the user
/// learns about it is this message.
pub(crate) async fn ensure_bundled_profile_remote(
    profile: &str,
    launcher: &str,
    env: &HashMap<String, String>,
    ssh: &SSHConnectionManager,
    connection_id: &str,
) -> OpenBitFunResult<()> {
    let (source, stamp) = bundled_build(profile)?;
    let exports = remote_exports(env);

    let probe_command =
        remote_user_shell_command(&remote_probe_script(profile, launcher, &exports));
    let (stdout, stderr, code) = ssh
        .execute_command(connection_id, &probe_command)
        .await
        .map_err(|error| {
            OpenBitFunError::service(format!(
                "Failed to inspect the DeepSeek Harness install on the remote host: {error}"
            ))
        })?;
    if code != 0 {
        return Err(OpenBitFunError::service(format!(
            "Failed to inspect the DeepSeek Harness install on the remote host: {}",
            remote_failure_summary(&stderr, &stdout)
        )));
    }
    let probe = parse_remote_probe(&stdout);

    // Presence is `command -v`, the same question the agent list asks. Asking a
    // different one here is how the list ends up saying "installed" while the
    // launch says "not installed".
    if probe.launcher.is_empty() {
        return Err(OpenBitFunError::service(format!(
            "DeepSeek Harness is not installed on the remote host: `{launcher}` is not on the login \
             shell's PATH. Install it there with `npm install -g @deepseek-ai/dsh`; OpenBitFun ships \
             the bridge, not the harness."
        )));
    }
    // The harness is a Node program, and it declares no `engines`, so an old
    // Node installs it without complaint and then fails to boot it. Say that
    // here rather than letting the user read a SyntaxError out of a log.
    if let Some(node) = unsupported_node_version(&probe.node) {
        return Err(OpenBitFunError::service(format!(
            "The remote host runs Node {node}, and DeepSeek Harness needs at least \
             {MIN_NODE_VERSION} to start (its own toolchain targets Node 22 LTS). Install a newer \
             Node on that host, then start the session again."
        )));
    }
    if !version_is_supported(&probe.version, &stamp.min_dsh_version) {
        return Err(OpenBitFunError::service(format!(
            "The remote host runs DeepSeek Harness {}, which is older than {}, the version this \
             OpenBitFun build requires. Update it there with `npm install -g @deepseek-ai/dsh`.",
            probe.version, stamp.min_dsh_version
        )));
    }
    if serde_json::from_str::<BridgeStamp>(&probe.stamp)
        .is_ok_and(|installed| installed.content == stamp.content)
    {
        return Ok(());
    }

    let archive = tokio::task::spawn_blocking(move || archive_profile(&source))
        .await
        .map_err(|error| {
            OpenBitFunError::service(format!(
                "Failed to package the DeepSeek Harness profile for upload: {error}"
            ))
        })??;

    let install_command = remote_user_shell_command(&remote_install_script(profile, &exports));
    let transport = ssh
        .open_workspace_stdio(connection_id, &install_command)
        .await
        .map_err(|error| {
            OpenBitFunError::service(format!(
                "Failed to install the DeepSeek Harness profile on the remote host: {error}"
            ))
        })?;
    // Every stream stays bound: dropping all three cancels the command, and the
    // extraction is only over once `completion` says so.
    let (mut stdin, _stdout, mut stderr, _control, completion) = transport.into_parts();

    let upload = async {
        stdin.write_all(&archive).await.map_err(|error| {
            OpenBitFunError::service(format!(
                "Failed to send the DeepSeek Harness profile to the remote host: {error}"
            ))
        })?;
        // The remote `tar` reads until end of input, so the shutdown is what
        // ends the extraction rather than a timeout.
        stdin.shutdown().await.map_err(|error| {
            OpenBitFunError::service(format!(
                "Failed to send the DeepSeek Harness profile to the remote host: {error}"
            ))
        })?;
        Ok::<_, OpenBitFunError>(completion.wait().await)
    };

    let exit = tokio::time::timeout(REMOTE_INSTALL_TIMEOUT, upload)
        .await
        .map_err(|_| {
            OpenBitFunError::service(
                "Timed out installing the DeepSeek Harness profile on the remote host".to_string(),
            )
        })??;

    if exit.exit_code != Some(0) {
        // The launch path sinks remote stderr, so without this the failure would
        // reach the user as an unexplained missing profile.
        let mut diagnostics = String::new();
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            stderr.read_to_string(&mut diagnostics),
        )
        .await;
        let summary = remote_failure_summary(&diagnostics, "");
        return Err(OpenBitFunError::service(format!(
            "Failed to install the DeepSeek Harness profile on the remote host (exit {}){}",
            exit.exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            if summary.is_empty() {
                String::new()
            } else {
                format!(": {summary}")
            }
        )));
    }

    log::info!(
        "Installed the bundled DeepSeek Harness profile on the remote host at {}/{profile}",
        probe.profiles
    );
    Ok(())
}

/// What the remote host says about its dsh install and this profile.
#[derive(Debug, Default, PartialEq)]
struct RemoteProbe {
    /// `$DSH_HOME/profiles` as the remote shell expands it. Reported for the
    /// log line only — the install script resolves it again on its own, so the
    /// two never travel through a quoting round trip.
    profiles: String,
    /// Where the remote login shell resolves the launcher, or empty when it
    /// resolves nowhere. This — not the version below — is what "installed"
    /// means, because it is what the agent list's own probe asks.
    launcher: String,
    /// First line of `dsh --version`, on either stream. Best effort: a launcher
    /// that answers nothing is still a launcher.
    version: String,
    /// What `node --version` says on that host, or empty. The harness is a Node
    /// program, so this is half of whether it can start at all.
    node: String,
    /// The installed build stamp, verbatim, or empty when the profile is new.
    stamp: String,
}

/// Environment the workspace configures, applied to the profile shell too.
///
/// A workspace that points `DSH_HOME` or `PATH` somewhere unusual has to reach
/// the same install the session will boot from, or we would upload the bridge
/// next to a harness nobody runs.
fn remote_exports(env: &HashMap<String, String>) -> String {
    let assignments = render_remote_env_assignments(env);
    if assignments.is_empty() {
        return String::new();
    }
    format!("export {}\n", assignments.join(" "))
}

/// One round trip that answers all three questions.
fn remote_probe_script(profile: &str, launcher: &str, exports: &str) -> String {
    let profile = shell_escape(profile);
    let launcher = shell_escape(launcher);
    format!(
        "{exports}profiles=\"${{DSH_HOME:-$HOME/.dsh}}/profiles\"\n\
         printf 'profiles=%s\\n' \"$profiles\"\n\
         printf 'launcher=%s\\n' \"$(command -v {launcher} 2>/dev/null | head -n 1 | tr -d '\\r')\"\n\
         printf 'version=%s\\n' \"$({launcher} --version 2>&1 | head -n 1 | tr -d '\\r')\"\n\
         printf 'node=%s\\n' \"$(node --version 2>/dev/null | head -n 1 | tr -d '\\r')\"\n\
         printf 'stamp=%s\\n' \"$(cat \"$profiles\"/{profile}/{STAMP_FILENAME} 2>/dev/null | tr -d '\\n\\r')\"\n"
    )
}

/// Read the probe's answers, ignoring whatever the login shell printed around
/// them.
fn parse_remote_probe(stdout: &str) -> RemoteProbe {
    let mut probe = RemoteProbe::default();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("profiles=") {
            probe.profiles = value.to_string();
        } else if let Some(value) = line.strip_prefix("launcher=") {
            probe.launcher = value.to_string();
        } else if let Some(value) = line.strip_prefix("version=") {
            probe.version = value.to_string();
        } else if let Some(value) = line.strip_prefix("node=") {
            probe.node = value.to_string();
        } else if let Some(value) = line.strip_prefix("stamp=") {
            probe.stamp = value.to_string();
        }
    }
    probe
}

/// Clear the managed directories, then extract what arrives on stdin.
///
/// The mirror of [`install_profile`]: same directories replaced rather than
/// merged, same everything-else left alone, and the stamp removed up front so
/// an interrupted upload reads as stale.
fn remote_install_script(profile: &str, exports: &str) -> String {
    let profile = shell_escape(profile);
    let managed = MANAGED_SUBDIRECTORIES
        .iter()
        .map(|directory| format!("\"$destination\"/{directory}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "set -e\n\
         {exports}profiles=\"${{DSH_HOME:-$HOME/.dsh}}/profiles\"\n\
         destination=\"$profiles\"/{profile}\n\
         mkdir -p \"$destination\"\n\
         rm -f \"$destination\"/{STAMP_FILENAME}\n\
         rm -rf {managed}\n\
         exec tar -xf - -C \"$destination\"\n"
    )
}

/// Pack the built profile, stamp last.
///
/// Ordering is the whole point: `tar` writes entries in the order they appear,
/// so a transfer that dies halfway leaves a destination without a current stamp
/// and the next launch simply redoes it.
fn archive_profile(source: &Path) -> OpenBitFunResult<Vec<u8>> {
    let pack_error = |error: std::io::Error| {
        OpenBitFunError::service(format!(
            "Failed to package the DeepSeek Harness profile at {}: {error}",
            source.display()
        ))
    };

    let mut paths = Vec::new();
    collect_relative_files(source, Path::new(""), &mut paths).map_err(pack_error)?;
    // Deterministic order, so an unchanged build produces an identical stream.
    paths.sort();

    let stamp = Path::new(STAMP_FILENAME);
    let mut builder = tar::Builder::new(Vec::new());
    for relative in paths.iter().filter(|path| path.as_path() != stamp) {
        builder
            .append_path_with_name(source.join(relative), relative)
            .map_err(pack_error)?;
    }
    builder
        .append_path_with_name(source.join(stamp), stamp)
        .map_err(pack_error)?;
    builder.into_inner().map_err(pack_error)
}

/// Every file under `root/relative`, as paths relative to `root`.
fn collect_relative_files(
    root: &Path,
    relative: &Path,
    out: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let child = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            collect_relative_files(root, &child, out)?;
        } else {
            out.push(child);
        }
    }
    Ok(())
}

/// The most useful line of a failed remote command, bounded in length.
fn remote_failure_summary(stderr: &str, stdout: &str) -> String {
    let text = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if text.len() <= 400 {
        return text.to_string();
    }
    let mut cut = 400;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", &text[..cut])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_directory(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("openbitfun-dsh-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("scratch directory should be created");
        path
    }

    fn write_built_profile(root: &Path, content: &str) {
        std::fs::create_dir_all(root.join("lib")).expect("lib should be created");
        std::fs::write(root.join("lib/app.js"), content).expect("app should be written");
        std::fs::write(root.join("cordis.patch.yml"), "- insert: []\n").expect("patch");
        std::fs::write(
            root.join(STAMP_FILENAME),
            format!(
                r#"{{"bridge":"0.0.1","content":"{content}","minDshVersion":"0.1.0-rc.6","profile":"openbitfun-acp"}}"#
            ),
        )
        .expect("stamp should be written");
    }

    #[test]
    fn installs_the_built_profile_and_replaces_a_previous_build() {
        let source = scratch_directory("source");
        let destination = scratch_directory("destination");
        write_built_profile(&source, "first");

        install_profile(&source, &destination).expect("first install");
        assert_eq!(
            std::fs::read_to_string(destination.join("lib/app.js")).expect("app"),
            "first"
        );

        // A file the previous build emitted but this one does not must not
        // survive into a tree that now claims to be the new build.
        std::fs::write(destination.join("lib/stale.js"), "stale").expect("stale file");
        // Anything outside the managed directories is the user's, and stays.
        std::fs::write(destination.join("notes.md"), "mine").expect("user file");

        write_built_profile(&source, "second");
        install_profile(&source, &destination).expect("second install");

        assert_eq!(
            std::fs::read_to_string(destination.join("lib/app.js")).expect("app"),
            "second"
        );
        assert!(!destination.join("lib/stale.js").exists());
        assert_eq!(
            std::fs::read_to_string(destination.join("notes.md")).expect("user file"),
            "mine"
        );

        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&destination);
    }

    #[test]
    fn reads_the_content_digest_that_decides_whether_to_reinstall() {
        let source = scratch_directory("stamp");
        write_built_profile(&source, "digest");

        let stamp = read_stamp(&source)
            .expect("stamp should be readable")
            .expect("stamp should be present");
        assert_eq!(stamp.content, "digest");
        assert_eq!(stamp.min_dsh_version, "0.1.0-rc.6");
        assert_eq!(profile_name_of(&source).as_deref(), Some("openbitfun-acp"));

        let empty = scratch_directory("stamp-empty");
        assert!(read_stamp(&empty)
            .expect("a missing stamp is not an error")
            .is_none());

        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn compares_prerelease_harness_versions() {
        assert!(version_is_supported("0.1.0-rc.6", "0.1.0-rc.6"));
        assert!(version_is_supported("0.1.0-rc.7", "0.1.0-rc.6"));
        assert!(version_is_supported("0.1.0", "0.1.0-rc.6"));
        assert!(version_is_supported("1.2.3", "0.1.0-rc.6"));
        assert!(!version_is_supported("0.1.0-rc.5", "0.1.0-rc.6"));
        assert!(!version_is_supported("0.0.9", "0.1.0-rc.6"));
        // A version we cannot read must not block a launch that may work.
        assert!(version_is_supported("dsh dev build", "0.1.0-rc.6"));
    }

    #[test]
    fn source_lookup_honours_the_override_and_checks_the_profile_name() {
        let source = scratch_directory("override");
        write_built_profile(&source, "override");

        temp_env(
            SOURCE_DIR_ENV,
            Some(source.to_string_lossy().as_ref()),
            || {
                assert_eq!(
                    bundled_profile_source("openbitfun-acp").as_deref(),
                    Some(source.as_path())
                );
            },
        );
        // Without the override, a directory that names a different profile is
        // not a match — the repo checkout below it may still be.
        assert_ne!(profile_name_of(&source).as_deref(), Some("other"));

        let _ = std::fs::remove_dir_all(&source);
    }

    #[test]
    fn the_remote_probe_answers_every_question_in_one_round_trip() {
        let script = remote_probe_script("openbitfun-acp", "dsh", "");

        // $DSH_HOME is resolved on the far side: reading it here would point at
        // the developer's own install, not the workspace's.
        assert!(script.contains("profiles=\"${DSH_HOME:-$HOME/.dsh}/profiles\""));
        assert!(script.contains("command -v dsh"));
        // Both streams: a launcher that prints its version on stderr is still
        // installed, and treating that as absent blocks a working host.
        assert!(script.contains("dsh --version 2>&1"));
        // The harness is a Node program; asking in the same round trip costs
        // nothing and is the difference between a sentence and a stack trace.
        assert!(script.contains("node --version"));
        assert!(script.contains(STAMP_FILENAME));

        let probe = parse_remote_probe(concat!(
            "Welcome to Ubuntu\n",
            "profiles=/home/dev/.dsh/profiles\n",
            "launcher=/usr/local/bin/dsh\n",
            "version=0.1.0-rc.6\n",
            "node=v22.19.0\n",
            r#"stamp={"content":"abc","minDshVersion":"0.1.0-rc.6"}"#,
            "\n",
        ));
        assert_eq!(probe.profiles, "/home/dev/.dsh/profiles");
        assert_eq!(probe.launcher, "/usr/local/bin/dsh");
        assert_eq!(probe.version, "0.1.0-rc.6");
        assert_eq!(probe.node, "v22.19.0");
        assert_eq!(
            probe.stamp,
            r#"{"content":"abc","minDshVersion":"0.1.0-rc.6"}"#
        );

        // A host without dsh reports empty rather than nothing at all, so the
        // caller can tell "not installed" from "the probe never ran".
        let missing =
            parse_remote_probe("profiles=/root/.dsh/profiles\nlauncher=\nversion=\nstamp=\n");
        assert_eq!(missing.launcher, "");
        assert_eq!(missing.version, "");
        assert_eq!(missing.stamp, "");

        // Installed but mute: the version gate lets an unparsable answer
        // through, so this host launches instead of being turned away.
        let mute = parse_remote_probe("launcher=/usr/local/bin/dsh\nversion=\nstamp=\n");
        assert!(!mute.launcher.is_empty());
        assert!(version_is_supported(&mute.version, "0.1.0-rc.6"));
    }

    #[test]
    fn a_node_too_old_for_the_harness_is_named_before_the_launch() {
        // The version that sent us here: dsh's first import is `util.parseEnv`,
        // which this Node does not have, and npm installed it anyway.
        assert_eq!(
            unsupported_node_version("v18.19.1").as_deref(),
            Some("v18.19.1")
        );
        assert_eq!(
            unsupported_node_version("v20.11.1").as_deref(),
            Some("v20.11.1")
        );

        // At or above the floor, and anything we cannot read, launches. A host
        // whose login shell has no `node` may still resolve one for `dsh`.
        assert_eq!(unsupported_node_version("v20.12.0"), None);
        assert_eq!(unsupported_node_version("v24.4.0"), None);
        assert_eq!(unsupported_node_version(""), None);
        assert_eq!(
            unsupported_node_version("bash: node: command not found"),
            None
        );
    }

    #[test]
    fn the_remote_install_replaces_exactly_what_the_local_one_replaces() {
        let script = remote_install_script("openbitfun-acp", "export DSH_HOME=/opt/dsh\n");

        assert!(script.starts_with("set -e\n"));
        assert!(script.contains("export DSH_HOME=/opt/dsh"));
        assert!(script.contains(&format!("rm -f \"$destination\"/{STAMP_FILENAME}")));
        for managed in MANAGED_SUBDIRECTORIES {
            assert!(script.contains(&format!("\"$destination\"/{managed}")));
        }
        // Nothing outside the managed set is removed — a user's own files in
        // the profile directory survive an upgrade here too.
        assert!(!script.contains("rm -rf \"$destination\"\n"));
        assert!(script.contains("exec tar -xf - -C \"$destination\""));
    }

    #[test]
    fn the_uploaded_archive_carries_the_stamp_last() {
        let source = scratch_directory("archive");
        write_built_profile(&source, "packed");

        let archive = archive_profile(&source).expect("profile should pack");
        let entries: Vec<String> = tar::Archive::new(archive.as_slice())
            .entries()
            .expect("entries")
            .map(|entry| {
                entry
                    .expect("entry")
                    .path()
                    .expect("path")
                    .display()
                    .to_string()
            })
            .collect();

        assert!(entries.contains(&"lib/app.js".to_string()));
        assert!(entries.contains(&"cordis.patch.yml".to_string()));
        // Last, so an interrupted upload leaves a profile that reads as stale.
        assert_eq!(entries.last().map(String::as_str), Some(STAMP_FILENAME));

        let _ = std::fs::remove_dir_all(&source);
    }

    #[test]
    fn remote_exports_only_carry_what_the_workspace_configured() {
        assert_eq!(remote_exports(&HashMap::new()), "");
        assert_eq!(
            remote_exports(&HashMap::from([(
                "DSH_HOME".to_string(),
                "/opt/dsh home".to_string()
            )])),
            "export DSH_HOME='/opt/dsh home'\n"
        );
    }

    /// Set an environment variable for the duration of `body`.
    ///
    /// Tests in one binary share a process, so this restores what it found.
    fn temp_env(key: &str, value: Option<&str>, body: impl FnOnce()) {
        let previous = std::env::var_os(key);
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        body();
        match previous {
            Some(previous) => std::env::set_var(key, previous),
            None => std::env::remove_var(key),
        }
    }
}
