//! Built-in skills shipped with BitFun.
//!
//! These skills are embedded into the `bitfun-core` binary and installed into a
//! managed `.system` directory under the user skills root on demand.

use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::BitFunResult;
use fs2::FileExt;
use include_dir::{include_dir, Dir};
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::task;

static BUILTIN_SKILLS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin_skills");
static BUILTIN_SKILL_DIR_NAMES: OnceLock<HashSet<String>> = OnceLock::new();
include!(concat!(env!("OUT_DIR"), "/embedded_builtin_skills.rs"));

const BUILTIN_SKILLS_MANIFEST_FILE_NAME: &str = ".manifest.json";
const BUILTIN_SKILLS_INSTALL_LOCK_FILE_NAME: &str = ".system.install.lock";
const BUILTIN_SKILLS_STAGING_PREFIX: &str = ".system.tmp";
const LEGACY_BUILTIN_SKILL_DIR_NAMES: &[&str] = &[
    // Historical bundled "Superpowers" skills removed in 2026-04.
    "brainstorming",
    "dispatching-parallel-agents",
    "executing-plans",
    "finishing-a-development-branch",
    "receiving-code-review",
    "requesting-code-review",
    "subagent-driven-development",
    "systematic-debugging",
    "test-driven-development",
    "using-git-worktrees",
    "using-superpowers",
    "verification-before-completion",
    "writing-plans",
    // Earlier built-in skill bundled before the Superpowers set.
    "skill-creator",
];
const LEGACY_BUILTIN_ROOT_FILES: &[&str] = &["SUPERPOWERS_LICENSE.txt"];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuiltinSkillsManifest {
    bundle_hash: String,
}

struct BuiltinSkillsInstallLock {
    file: std::fs::File,
}

impl Drop for BuiltinSkillsInstallLock {
    fn drop(&mut self) {
        if let Err(error) = self.file.unlock() {
            warn!("Failed to unlock built-in skills install lock: {}", error);
        }
    }
}

fn collect_builtin_skill_dir_names() -> HashSet<String> {
    BUILTIN_SKILLS_DIR
        .dirs()
        .filter_map(|dir| {
            let rel = dir.path();
            if rel.components().count() != 1 {
                return None;
            }

            rel.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
        })
        .collect()
}

pub fn builtin_skill_dir_names() -> &'static HashSet<String> {
    BUILTIN_SKILL_DIR_NAMES.get_or_init(collect_builtin_skill_dir_names)
}

pub fn builtin_skills_bundle_hash() -> &'static str {
    BUILTIN_SKILLS_BUNDLE_HASH
}

pub fn is_builtin_skill_dir_name(dir_name: &str) -> bool {
    builtin_skill_dir_names().contains(dir_name)
}

fn builtin_skills_manifest_path(root: &Path) -> PathBuf {
    root.join(BUILTIN_SKILLS_MANIFEST_FILE_NAME)
}

fn builtin_skills_install_lock_path(root: &Path) -> PathBuf {
    root.join(BUILTIN_SKILLS_INSTALL_LOCK_FILE_NAME)
}

fn builtin_skills_staging_root(parent: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    parent.join(format!(
        "{}.{}.{}",
        BUILTIN_SKILLS_STAGING_PREFIX,
        std::process::id(),
        timestamp
    ))
}

async fn read_installed_manifest(root: &Path) -> BitFunResult<Option<BuiltinSkillsManifest>> {
    let path = builtin_skills_manifest_path(root);
    match fs::read_to_string(&path).await {
        Ok(content) => match serde_json::from_str::<BuiltinSkillsManifest>(&content) {
            Ok(manifest) => Ok(Some(manifest)),
            Err(error) => {
                warn!(
                    "Invalid built-in skills manifest at {}: {}",
                    path.display(),
                    error
                );
                Ok(None)
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

async fn write_installed_manifest(root: &Path) -> BitFunResult<()> {
    let path = builtin_skills_manifest_path(root);
    let manifest = BuiltinSkillsManifest {
        bundle_hash: builtin_skills_bundle_hash().to_string(),
    };
    let content = serde_json::to_vec_pretty(&manifest)?;
    fs::write(path, content).await?;
    Ok(())
}

async fn remove_existing_path(path: &Path) -> BitFunResult<()> {
    let Ok(metadata) = fs::symlink_metadata(path).await else {
        return Ok(());
    };

    if metadata.is_dir() {
        fs::remove_dir_all(path).await?;
    } else {
        fs::remove_file(path).await?;
    }

    Ok(())
}

async fn cleanup_legacy_builtin_dirs(legacy_root: &Path) -> BitFunResult<()> {
    for dir_name in builtin_skill_dir_names() {
        let path = legacy_root.join(dir_name);
        remove_existing_path(&path).await?;
    }

    for dir_name in LEGACY_BUILTIN_SKILL_DIR_NAMES {
        let path = legacy_root.join(dir_name);
        remove_existing_path(&path).await?;
    }

    for file_name in LEGACY_BUILTIN_ROOT_FILES {
        let path = legacy_root.join(file_name);
        remove_existing_path(&path).await?;
    }

    Ok(())
}

async fn acquire_install_lock(legacy_root: &Path) -> BitFunResult<BuiltinSkillsInstallLock> {
    let lock_path = builtin_skills_install_lock_path(legacy_root);

    // Use an OS-backed advisory file lock so parallel test processes and app
    // instances serialize built-in skill installation across the shared
    // `.system` directory.
    let file = task::spawn_blocking(move || -> BitFunResult<std::fs::File> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        file.lock_exclusive()?;
        Ok(file)
    })
    .await
    .map_err(|error| {
        crate::util::errors::BitFunError::io(format!(
            "Failed to join built-in skills install lock task: {}",
            error
        ))
    })??;

    Ok(BuiltinSkillsInstallLock { file })
}

async fn install_builtin_skills_to_staging(staging_root: &Path) -> BitFunResult<(usize, usize)> {
    let mut installed = 0usize;
    let mut updated = 0usize;

    for skill_dir in BUILTIN_SKILLS_DIR.dirs() {
        let rel = skill_dir.path();
        if rel.components().count() != 1 {
            continue;
        }

        let stats = sync_dir(skill_dir, staging_root).await?;
        installed += stats.installed;
        updated += stats.updated;
    }

    write_installed_manifest(staging_root).await?;
    Ok((installed, updated))
}

pub async fn ensure_builtin_skills_installed() -> BitFunResult<()> {
    let pm = get_path_manager_arc();
    let legacy_root = pm.user_skills_dir();
    let dest_root = pm.builtin_skills_dir();

    // Create the parent user skills directory before taking the shared install
    // lock so every contender points at the same stable path.
    if let Err(e) = fs::create_dir_all(&legacy_root).await {
        error!(
            "Failed to create user skills directory: path={}, error={}",
            legacy_root.display(),
            e
        );
        return Err(e.into());
    }

    let _install_lock = acquire_install_lock(&legacy_root).await?;
    let system_dir_preexisting = fs::symlink_metadata(&dest_root).await.is_ok();

    if !system_dir_preexisting {
        cleanup_legacy_builtin_dirs(&legacy_root).await?;
    }

    if let Some(manifest) = read_installed_manifest(&dest_root).await? {
        if manifest.bundle_hash == builtin_skills_bundle_hash() {
            return Ok(());
        }
    }

    let staging_root = builtin_skills_staging_root(&legacy_root);
    if let Err(error) = fs::remove_dir_all(&staging_root).await {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error.into());
        }
    }
    fs::create_dir_all(&staging_root).await?;

    let publish_result = async {
        let (installed, updated) = install_builtin_skills_to_staging(&staging_root).await?;

        if let Err(error) = fs::remove_dir_all(&dest_root).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error.into());
            }
        }
        fs::rename(&staging_root, &dest_root).await?;

        if installed > 0 || updated > 0 {
            debug!(
                "Built-in skills synchronized: installed={}, updated={}, dest_root={}",
                installed,
                updated,
                dest_root.display()
            );
        }

        Ok(())
    }
    .await;

    if let Err(error) = fs::remove_dir_all(&staging_root).await {
        if error.kind() != std::io::ErrorKind::NotFound {
            warn!(
                "Failed to remove built-in skills staging directory {}: {}",
                staging_root.display(),
                error
            );
        }
    }

    publish_result
}

#[derive(Default)]
struct SyncStats {
    installed: usize,
    updated: usize,
}

async fn sync_dir(dir: &Dir<'_>, dest_root: &Path) -> BitFunResult<SyncStats> {
    let mut files: Vec<&include_dir::File<'_>> = Vec::new();
    collect_files(dir, &mut files);

    let mut stats = SyncStats::default();
    for file in files.into_iter() {
        let dest_path = safe_join(dest_root, file.path())?;
        let desired = desired_file_content(file, &dest_path).await?;

        if let Ok(current) = fs::read(&dest_path).await {
            if current == desired {
                continue;
            }
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let existed = dest_path.exists();
        fs::write(&dest_path, desired).await?;
        if existed {
            stats.updated += 1;
        } else {
            stats.installed += 1;
        }
    }

    Ok(stats)
}

fn collect_files<'a>(dir: &'a Dir<'a>, out: &mut Vec<&'a include_dir::File<'a>>) {
    for file in dir.files() {
        out.push(file);
    }

    for sub in dir.dirs() {
        collect_files(sub, out);
    }
}

fn safe_join(root: &Path, relative: &Path) -> BitFunResult<PathBuf> {
    if relative.is_absolute() {
        return Err(crate::util::errors::BitFunError::validation(format!(
            "Unexpected absolute path in built-in skills: {}",
            relative.display()
        )));
    }

    // Prevent `..` traversal even though include_dir should only contain clean relative paths.
    for c in relative.components() {
        if matches!(c, std::path::Component::ParentDir) {
            return Err(crate::util::errors::BitFunError::validation(format!(
                "Unexpected parent dir component in built-in skills path: {}",
                relative.display()
            )));
        }
    }

    Ok(root.join(relative))
}

async fn desired_file_content(
    file: &include_dir::File<'_>,
    _dest_path: &Path,
) -> BitFunResult<Vec<u8>> {
    Ok(file.contents().to_vec())
}

#[cfg(test)]
mod tests {
    use super::{collect_files, BUILTIN_SKILLS_DIR};

    fn embedded_skill_text(path: &str) -> &'static str {
        BUILTIN_SKILLS_DIR
            .get_file(path)
            .unwrap_or_else(|| panic!("Missing embedded built-in skill file: {path}"))
            .contents_utf8()
            .unwrap_or_else(|| panic!("Built-in skill file is not UTF-8: {path}"))
    }

    fn gstack_skill_texts() -> Vec<(String, &'static str)> {
        BUILTIN_SKILLS_DIR
            .dirs()
            .filter_map(|dir| {
                let name = dir.path().file_name()?.to_str()?;
                if !name.starts_with("gstack-") {
                    return None;
                }
                let file = dir.files().find(|file| {
                    file.path().file_name().and_then(|name| name.to_str()) == Some("SKILL.md")
                })?;
                Some((
                    name.to_string(),
                    file.contents_utf8()
                        .unwrap_or_else(|| panic!("{name}/SKILL.md is not UTF-8")),
                ))
            })
            .collect()
    }

    #[test]
    fn gstack_direct_skill_paths_resolve_to_bundled_skills() {
        for (source, text) in gstack_skill_texts() {
            for token in text.split(|ch: char| {
                !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/' | '.' | ':'))
            }) {
                if let Some(path) = token.strip_suffix("/SKILL.md") {
                    let target = path.rsplit('/').next().unwrap_or(path);
                    assert!(
                        BUILTIN_SKILLS_DIR.get_dir(target).is_some(),
                        "{source}/SKILL.md references missing built-in skill {target}/SKILL.md"
                    );
                }
                if let Some(target) = token.strip_prefix("user::bitfun-system::") {
                    assert!(
                        BUILTIN_SKILLS_DIR.get_dir(target).is_some(),
                        "{source}/SKILL.md references missing stable skill key {token}"
                    );
                }
            }
        }
    }

    #[test]
    fn gstack_does_not_emit_pseudo_bitfun_browser_commands() {
        const STALE_BROWSER_GUIDANCE: [&str; 5] = [
            "BitFun browser/computer-use",
            "BitFun built-in browser/computer-use",
            "external browse binary",
            "use `ComputerUse` for browser inspection",
            "use `ComputerUse` for browser/desktop testing",
        ];

        for (source, text) in gstack_skill_texts() {
            let lowercase = text.to_ascii_lowercase();
            for stale in STALE_BROWSER_GUIDANCE {
                assert!(
                    !text.contains(stale),
                    "{source}/SKILL.md still contains stale browser guidance: {stale}"
                );
            }
            assert!(
                !text.contains("agent-browser fill @e4 \"[REDACTED]\""),
                "{source}/SKILL.md still places a password placeholder in a logged command"
            );
            assert!(
                !text.contains("agent-browser state load cookies.json"),
                "{source}/SKILL.md treats a cookie file as agent-browser storage state"
            );
            assert!(
                !text.contains("auth save qa-target") && !text.contains("auth login qa-target"),
                "{source}/SKILL.md reuses a global auth profile across unrelated targets"
            );
            assert!(
                !text.contains("CDP_MODE=true"),
                "{source}/SKILL.md still infers agent-browser state from the legacy CDP mode"
            );
            if text.contains("cookie file or Copy-as-cURL export") {
                assert!(
                    text.contains("agent-browser cookies set --curl cookies.json"),
                    "{source}/SKILL.md does not import cookie files with the supported command"
                );
            }
            assert!(
                !text.contains("Ask the user to enter it through stdin"),
                "{source}/SKILL.md asks for interactive stdin in a non-interactive tool command"
            );
            if text.contains("--password-stdin") {
                assert!(
                    text.contains("own interactive terminal"),
                    "{source}/SKILL.md must route password-stdin setup to the user's terminal"
                );
            }
            if text.contains("agent-browser open") || text.contains("agent-browser get url") {
                assert!(
                    text.contains("agent-browser --version")
                        && text.contains("agent-browser skills get core")
                        && lowercase.contains("once per skill invocation")
                        && lowercase.contains("before the first browser command"),
                    "{source}/SKILL.md must verify the CLI and load version-matched guidance"
                );
            }
            if text.contains("SKETCH_URI") {
                assert!(
                    text.contains("tempfile.mkstemp")
                        && text.contains(".resolve().as_uri()")
                        && text.contains("agent-browser --allow-file-access open")
                        && lowercase.contains("once per skill invocation")
                        && lowercase.contains("before the first browser command"),
                    "{source}/SKILL.md does not open local HTML portably and explicitly"
                );
            }
        }
    }

    #[test]
    fn gstack_does_not_route_to_unbundled_workflows() {
        const ABSENT_WORKFLOWS: [&str; 6] = [
            "plan-devex-review",
            "/design-shotgun",
            "/design-html",
            "/setup-browser-cookies",
            "qa/templates/qa-report-template.md",
            "qa/references/issue-taxonomy.md",
        ];

        for (source, text) in gstack_skill_texts() {
            for absent in ABSENT_WORKFLOWS {
                assert!(
                    !text.contains(absent),
                    "{source}/SKILL.md routes to unbundled workflow {absent}"
                );
            }
        }
    }

    #[test]
    fn agent_browser_uses_dynamic_cli_documentation_only() {
        let text = embedded_skill_text("agent-browser/SKILL.md");
        assert!(text.contains("agent-browser skills get core"));
        assert!(text.contains("agent-browser skills get core --full"));
        assert!(text.contains("agent-browser skills list"));
        assert!(text.contains("agent-browser skills get electron"));
        assert!(text.contains("agent-browser skills get dogfood"));
        assert!(text.contains("npm i -g agent-browser@0.32.3"));
        assert!(text.contains("Install only after user approval"));
        assert!(text.contains("do not silently switch tools"));
        assert!(text.contains("native Rust"));
        assert!(!text.contains("npx playwright install-deps"));

        let dir = BUILTIN_SKILLS_DIR
            .get_dir("agent-browser")
            .expect("agent-browser directory should be embedded");
        assert!(
            dir.dirs().next().is_none(),
            "dynamic agent-browser stub must not retain static reference/template directories"
        );
    }

    #[test]
    fn office_helpers_use_validated_archive_extraction() {
        for skill in ["docx", "pptx", "xlsx"] {
            let helper_path = format!("{skill}/scripts/office/helpers/__init__.py");
            let helper = embedded_skill_text(&helper_path);
            assert!(
                helper.contains("def safe_extract("),
                "{helper_path} lacks safe_extract"
            );
            assert!(
                helper.contains("stat.S_ISLNK"),
                "{helper_path} lacks symlink rejection"
            );
            assert!(
                helper.contains("MAX_ARCHIVE_TOTAL_SIZE")
                    && helper.contains("MAX_ARCHIVE_COMPRESSION_RATIO")
                    && helper.contains("duplicate archive entry"),
                "{helper_path} lacks bounded, collision-safe extraction"
            );

            let dir = BUILTIN_SKILLS_DIR
                .get_dir(skill)
                .unwrap_or_else(|| panic!("Missing embedded Office skill {skill}"));
            let mut files = Vec::new();
            collect_files(dir, &mut files);
            for file in files {
                let text = file.contents_utf8().unwrap_or("");
                assert!(
                    !text.contains(".extractall("),
                    "{} still uses unrestricted ZipFile.extractall",
                    file.path().display()
                );
            }

            assert!(dir
                .get_file(format!("{skill}/scripts/office/pack.py"))
                .is_none());
            assert!(dir
                .get_file(format!("{skill}/scripts/office/unpack.py"))
                .is_none());

            if matches!(skill, "docx" | "pptx") {
                let skill_text = embedded_skill_text(&format!("{skill}/SKILL.md"));
                assert!(
                    skill_text.contains("safe_extract") && skill_text.contains("rezip"),
                    "{skill}/SKILL.md must use the cross-platform safe archive helpers"
                );
                assert!(
                    !skill_text.contains("unzip -q") && !skill_text.contains("zip -Xr"),
                    "{skill}/SKILL.md still recommends unsafe or non-portable archive commands"
                );
            }
        }

        let comment = embedded_skill_text("docx/scripts/comment.py");
        assert!(comment.contains("author: str = \"BitFun\""));
        assert!(comment.contains("initials: str = \"B\""));
        assert!(comment.contains("default=\"BitFun\""));
        assert!(comment.contains("default=\"B\""));

        let docx_skill = embedded_skill_text("docx/SKILL.md");
        assert!(docx_skill.contains(
            "Use \"BitFun\" as the author for tracked changes and comments unless the user explicitly requests a different name."
        ));

        let xlsx_skill = embedded_skill_text("xlsx/SKILL.md");
        assert!(xlsx_skill.contains("years as text (`\"2026\"`, never `2,026`)"));

        let docx_helper = embedded_skill_text("docx/scripts/office/helpers/__init__.py");
        for skill in ["pptx", "xlsx"] {
            assert_eq!(
                docx_helper,
                embedded_skill_text(&format!("{skill}/scripts/office/helpers/__init__.py")),
                "Office safe extraction helpers drifted between bundled skills"
            );
        }
    }
}
