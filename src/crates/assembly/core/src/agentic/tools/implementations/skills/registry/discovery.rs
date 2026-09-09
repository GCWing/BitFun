//! Standard skill-root traversal. Keep the logical path in stable keys and
//! follow linked skill directories on the filesystem that owns each root.
use super::*;
use std::collections::VecDeque;
use tokio::io::AsyncReadExt;

const MAX_SCAN_DEPTH: usize = 32;
const MAX_SCAN_DIRECTORIES: usize = 4096;
const MAX_DIRECTORY_ENTRIES: usize = 8192;
const MAX_SKILL_BYTES: usize = 1024 * 1024;

pub(super) fn diagnostic(
    path: impl Into<String>,
    source_id: &str,
    message: impl ToString,
) -> SkillScanDiagnostic {
    SkillScanDiagnostic {
        path: path.into(),
        source_id: source_id.into(),
        message: message.to_string(),
    }
}

fn set_nested_key(candidate: &mut SkillCandidate, relative_dir: &str) {
    // Direct children keep their existing persisted keys. Nested siblings with
    // the same leaf directory name must not acquire the same key.
    candidate.info.key = format!(
        "{}::{}::{}",
        candidate.info.level.as_str(),
        candidate.info.source_slot,
        relative_dir
    );
}

fn supports_flat_skills(source_id: &str) -> bool {
    matches!(source_id, "pi" | "deepseek-harness")
}

fn flat_skill_data(
    directory: &str,
    filename: &str,
    content: &str,
    level: SkillLocation,
    slot: &str,
) -> Result<SkillData, openbitfun_agent_runtime::skills::SkillParseError> {
    let stem = filename.strip_suffix(".md").unwrap_or(filename);
    let mut data =
        SkillRegistry::parse_skill_markdown(directory.to_string(), content, level, false, slot)?;
    data.path = directory.to_string();
    data.dir_name = stem.to_string();
    data.entry_file = Some(filename.to_string());
    Ok(data)
}

impl SkillRegistry {
    pub(super) async fn scan_remote_project_skills(
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
    ) -> SkillCandidateScan {
        let root = remote_root.trim_end_matches('/');
        let roots = PROJECT_SKILL_ROOTS
            .iter()
            .enumerate()
            .map(|(priority, spec)| async move {
                let entry = RemoteSkillRootEntry {
                    path: format!("{}/{}/{}", root, spec.parent, spec.subdir),
                    slot: spec.slot,
                    source_id: spec.source_id,
                    source_label: spec.source_label,
                    priority,
                };
                Self::scan_remote_skill_root(fs, &entry, root).await
            })
            .collect::<Vec<_>>();
        let scans = stream::iter(roots)
            .buffered(REMOTE_SKILL_SCAN_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
        let mut result = SkillCandidateScan::default();
        for mut scan in scans {
            result.candidates.append(&mut scan.candidates);
            result.diagnostics.append(&mut scan.diagnostics);
        }
        result
    }

    async fn scan_remote_skill_root(
        fs: &dyn WorkspaceFileSystem,
        entry: &RemoteSkillRootEntry,
        workspace_root: &str,
    ) -> SkillCandidateScan {
        let mut scan = SkillCandidateScan::default();
        match fs.is_dir(&entry.path).await {
            Ok(true) => {}
            Ok(false) => return scan,
            Err(error) => {
                scan.diagnostics
                    .push(diagnostic(&entry.path, entry.source_id, error));
                return scan;
            }
        }
        let mut installation_sources = HashMap::new();
        if entry.slot == "agents" {
            let lock_path = format!("{workspace_root}/skills-lock.json");
            match fs.exists(&lock_path).await {
                Ok(true) => match fs.read_file_text(&lock_path).await {
                    Ok(content) => {
                        installation_sources = parse_skill_installation_sources(&content)
                    }
                    Err(error) => {
                        scan.diagnostics
                            .push(diagnostic(&lock_path, entry.source_id, error))
                    }
                },
                Ok(false) => {}
                Err(error) => scan
                    .diagnostics
                    .push(diagnostic(&lock_path, entry.source_id, error)),
            }
        }

        let mut pending = VecDeque::from([(entry.path.clone(), 0usize)]);
        let mut visited = HashSet::new();
        while let Some((path, depth)) = pending.pop_front() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if visited.len() > MAX_SCAN_DIRECTORIES || depth > MAX_SCAN_DEPTH {
                scan.diagnostics.push(diagnostic(
                    &path,
                    entry.source_id,
                    "Skill directory traversal limit reached (possibly a symbolic-link cycle)",
                ));
                if visited.len() > MAX_SCAN_DIRECTORIES {
                    break;
                }
                continue;
            }
            if depth > 0 {
                let skill_md = format!("{path}/SKILL.md");
                match fs.is_file(&skill_md).await {
                    Ok(true) => {
                        match fs.read_file_text_bounded(&skill_md, MAX_SKILL_BYTES).await {
                            Ok(Some(content)) => match Self::parse_skill_markdown(
                                path.clone(),
                                &content,
                                SkillLocation::Project,
                                false,
                                entry.slot,
                            ) {
                                Ok(mut data) => {
                                    for warning in &data.compatibility_warnings {
                                        scan.diagnostics.push(diagnostic(
                                            &skill_md,
                                            entry.source_id,
                                            warning,
                                        ));
                                    }
                                    if let Some(error) =
                                        Self::apply_remote_openai_policy(&mut data, fs, &path).await
                                    {
                                        scan.diagnostics.push(diagnostic(
                                            format!("{path}/agents/openai.yaml"),
                                            entry.source_id,
                                            error,
                                        ));
                                    }
                                    let mut candidate = SkillCandidate::from_data(
                                        data,
                                        entry.slot,
                                        entry.source_id,
                                        entry.source_label,
                                        PROJECT_SKILL_KEY_PREFIX,
                                        entry.priority,
                                        false,
                                    );
                                    set_nested_key(
                                        &mut candidate,
                                        path.strip_prefix(&format!("{}/", entry.path))
                                            .expect("discovered child"),
                                    );
                                    candidate.info.installation_source =
                                        installation_sources.get(&candidate.info.name).cloned();
                                    scan.candidates.push(candidate);
                                }
                                Err(error) => scan.diagnostics.push(diagnostic(
                                    &skill_md,
                                    entry.source_id,
                                    error,
                                )),
                            },
                            Ok(None) => scan.diagnostics.push(diagnostic(
                                &skill_md,
                                entry.source_id,
                                "SKILL.md exceeds the discovery size limit",
                            )),
                            Err(error) => {
                                scan.diagnostics
                                    .push(diagnostic(&skill_md, entry.source_id, error))
                            }
                        }
                        continue;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        scan.diagnostics
                            .push(diagnostic(&skill_md, entry.source_id, error));
                        continue;
                    }
                }
            }
            // DSH accepts direct bundles and flat entries, but not recursive groups.
            if entry.source_id == "deepseek-harness" && depth > 0 {
                continue;
            }
            let mut children = match fs.read_dir_bounded(&path, MAX_DIRECTORY_ENTRIES + 1).await {
                Ok(children) => children,
                Err(error) => {
                    scan.diagnostics
                        .push(diagnostic(&path, entry.source_id, error));
                    continue;
                }
            };
            if children.len() > MAX_DIRECTORY_ENTRIES {
                scan.diagnostics.push(diagnostic(
                    &path,
                    entry.source_id,
                    "Skill directory entry limit reached",
                ));
                children.truncate(MAX_DIRECTORY_ENTRIES);
            }
            sort_remote_dir_entries(&mut children);
            for child in children {
                if child.name.is_empty()
                    || child.name == "."
                    || child.name == ".."
                    || child.name.contains('/')
                    || child.name.contains('\0')
                {
                    scan.diagnostics.push(diagnostic(
                        &path,
                        entry.source_id,
                        "Invalid child name returned by workspace filesystem",
                    ));
                    continue;
                }
                let child_path = format!("{path}/{}", child.name);
                let is_dir = if child.is_symlink {
                    match fs.is_dir(&child_path).await {
                        Ok(is_dir) => is_dir,
                        Err(error) => {
                            scan.diagnostics
                                .push(diagnostic(&child_path, entry.source_id, error));
                            false
                        }
                    }
                } else {
                    child.is_dir
                };
                if is_dir {
                    if pending.len() + visited.len() >= MAX_SCAN_DIRECTORIES {
                        scan.diagnostics.push(diagnostic(
                            &path,
                            entry.source_id,
                            "Skill directory traversal limit reached",
                        ));
                        break;
                    }
                    pending.push_back((child_path, depth + 1));
                } else if supports_flat_skills(entry.source_id) && child.name.ends_with(".md") {
                    match fs
                        .read_file_text_bounded(&child_path, MAX_SKILL_BYTES)
                        .await
                    {
                        Ok(Some(content)) => match flat_skill_data(
                            &path,
                            &child.name,
                            &content,
                            SkillLocation::Project,
                            entry.slot,
                        ) {
                            Ok(data) => {
                                let mut candidate = SkillCandidate::from_data(
                                    data,
                                    entry.slot,
                                    entry.source_id,
                                    entry.source_label,
                                    PROJECT_SKILL_KEY_PREFIX,
                                    entry.priority,
                                    false,
                                );
                                set_nested_key(
                                    &mut candidate,
                                    child_path
                                        .strip_prefix(&format!("{}/", entry.path))
                                        .expect("discovered child"),
                                );
                                scan.candidates.push(candidate);
                            }
                            // PI ignores ordinary Markdown files with no skill frontmatter.
                            Err(_) if entry.source_id == "pi" && child.name != "SKILL.md" => {}
                            Err(error) => scan.diagnostics.push(diagnostic(
                                &child_path,
                                entry.source_id,
                                error,
                            )),
                        },
                        Ok(None) => scan.diagnostics.push(diagnostic(
                            &child_path,
                            entry.source_id,
                            "Skill Markdown exceeds the discovery size limit",
                        )),
                        Err(error) => {
                            scan.diagnostics
                                .push(diagnostic(&child_path, entry.source_id, error))
                        }
                    }
                }
            }
        }
        scan.candidates = sort_skill_candidates_by_dir(scan.candidates);
        scan
    }

    pub(super) async fn scan_skills_in_dir(entry: &SkillRootEntry) -> LocalSkillScan {
        let mut scan = LocalSkillScan {
            candidates: Vec::new(),
            diagnostics: Vec::new(),
            cacheable: local_source_path_is_cacheable(&entry.path).await,
        };
        match fs::metadata(&entry.path).await {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                scan.diagnostics.push(diagnostic(
                    entry.path.to_string_lossy(),
                    entry.source_id,
                    "Skill root is not a directory",
                ));
                scan.cacheable = false;
                return scan;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return scan,
            Err(error) => {
                scan.diagnostics.push(diagnostic(
                    entry.path.to_string_lossy(),
                    entry.source_id,
                    error,
                ));
                scan.cacheable = false;
                return scan;
            }
        }

        let installation_sources = if let Some(lock_path) = skill_installation_lock_path(entry) {
            scan.cacheable &= local_source_path_is_cacheable(&lock_path).await;
            match fs::read_to_string(&lock_path).await {
                Ok(content) => parse_skill_installation_sources(&content),
                Err(error) => {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        scan.diagnostics.push(diagnostic(
                            lock_path.to_string_lossy(),
                            entry.source_id,
                            error,
                        ));
                        scan.cacheable = false;
                    }
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        let mut pending = VecDeque::from([(entry.path.clone(), 0usize)]);
        let mut visited = HashSet::new();
        while let Some((path, depth)) = pending.pop_front() {
            scan.cacheable &= local_source_path_is_cacheable(&path).await;
            let canonical = match fs::canonicalize(&path).await {
                Ok(path) => path,
                Err(error) => {
                    scan.diagnostics.push(diagnostic(
                        path.to_string_lossy(),
                        entry.source_id,
                        error,
                    ));
                    scan.cacheable = false;
                    continue;
                }
            };
            if !visited.insert(canonical) {
                continue;
            }
            if visited.len() > MAX_SCAN_DIRECTORIES || depth > MAX_SCAN_DEPTH {
                scan.diagnostics.push(diagnostic(
                    path.to_string_lossy(),
                    entry.source_id,
                    "Skill directory traversal limit reached",
                ));
                scan.cacheable = false;
                if visited.len() > MAX_SCAN_DIRECTORIES {
                    break;
                }
                continue;
            }

            if depth > 0 {
                let skill_md = path.join("SKILL.md");
                scan.cacheable &= local_source_path_is_cacheable(&skill_md).await;
                match fs::File::open(&skill_md).await {
                    Ok(file) => {
                        let mut content = String::new();
                        let read = file
                            .take((MAX_SKILL_BYTES + 1) as u64)
                            .read_to_string(&mut content)
                            .await;
                        if let Err(error) = read {
                            scan.diagnostics.push(diagnostic(
                                skill_md.to_string_lossy(),
                                entry.source_id,
                                error,
                            ));
                            scan.cacheable = false;
                        } else if content.len() > MAX_SKILL_BYTES {
                            scan.diagnostics.push(diagnostic(
                                skill_md.to_string_lossy(),
                                entry.source_id,
                                "SKILL.md exceeds the discovery size limit",
                            ));
                        } else {
                            match Self::parse_skill_markdown(
                                path.to_string_lossy().into_owned(),
                                &content,
                                entry.level,
                                false,
                                entry.slot,
                            ) {
                                Ok(mut data) => {
                                    for warning in &data.compatibility_warnings {
                                        scan.diagnostics.push(diagnostic(
                                            skill_md.to_string_lossy(),
                                            entry.source_id,
                                            warning,
                                        ));
                                    }
                                    let (cacheable, policy_error) =
                                        Self::apply_local_openai_policy(&mut data, &path).await;
                                    scan.cacheable &= cacheable;
                                    if let Some(error) = policy_error {
                                        scan.diagnostics.push(diagnostic(
                                            path.join("agents/openai.yaml").to_string_lossy(),
                                            entry.source_id,
                                            error,
                                        ));
                                    }
                                    let mut candidate = SkillCandidate::from_data(
                                        data,
                                        entry.slot,
                                        entry.source_id,
                                        entry.source_label,
                                        entry.level.as_str(),
                                        entry.priority,
                                        entry.is_builtin,
                                    );
                                    let relative_dir = path
                                        .strip_prefix(&entry.path)
                                        .expect("discovered child")
                                        .components()
                                        .map(|component| component.as_os_str().to_string_lossy())
                                        .collect::<Vec<_>>()
                                        .join("/");
                                    set_nested_key(&mut candidate, &relative_dir);
                                    candidate.info.installation_source =
                                        installation_sources.get(&candidate.info.name).cloned();
                                    scan.candidates.push(candidate);
                                }
                                Err(error) => scan.diagnostics.push(diagnostic(
                                    skill_md.to_string_lossy(),
                                    entry.source_id,
                                    error,
                                )),
                            }
                        }
                        // A skill is a package boundary; its reference examples
                        // and scripts are not independent installed skills.
                        continue;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        scan.diagnostics.push(diagnostic(
                            skill_md.to_string_lossy(),
                            entry.source_id,
                            error,
                        ));
                        scan.cacheable = false;
                        continue;
                    }
                }
            }

            if entry.source_id == "deepseek-harness" && depth > 0 {
                continue;
            }

            let mut entries = match fs::read_dir(&path).await {
                Ok(entries) => entries,
                Err(error) => {
                    scan.diagnostics.push(diagnostic(
                        path.to_string_lossy(),
                        entry.source_id,
                        error,
                    ));
                    scan.cacheable = false;
                    continue;
                }
            };
            let mut children = Vec::new();
            let mut count = 0;
            loop {
                let child = match entries.next_entry().await {
                    Ok(Some(child)) => child,
                    Ok(None) => break,
                    Err(error) => {
                        scan.diagnostics.push(diagnostic(
                            path.to_string_lossy(),
                            entry.source_id,
                            error,
                        ));
                        scan.cacheable = false;
                        break;
                    }
                };
                count += 1;
                if count > MAX_DIRECTORY_ENTRIES {
                    scan.diagnostics.push(diagnostic(
                        path.to_string_lossy(),
                        entry.source_id,
                        "Skill directory entry limit reached",
                    ));
                    scan.cacheable = false;
                    break;
                }
                let child_path = child.path();
                if depth == 0
                    && matches!(entry.slot, OPENBITFUN_USER_SKILL_SLOT | "home.dsh")
                    && child.file_name() == OPENBITFUN_SYSTEM_SKILL_DIR
                {
                    continue;
                }
                scan.cacheable &= local_source_path_is_cacheable(&child_path).await;
                match fs::metadata(&child_path).await {
                    Ok(meta) if meta.is_dir() => children.push(child_path),
                    Ok(meta)
                        if meta.is_file()
                            && supports_flat_skills(entry.source_id)
                            && child_path
                                .extension()
                                .is_some_and(|extension| extension == "md") =>
                    {
                        let filename = child.file_name().to_string_lossy().into_owned();
                        let result = async {
                            let file = fs::File::open(&child_path).await?;
                            let mut content = String::new();
                            file.take((MAX_SKILL_BYTES + 1) as u64)
                                .read_to_string(&mut content)
                                .await?;
                            Ok::<_, std::io::Error>(content)
                        }
                        .await;
                        match result {
                            Ok(content) if content.len() <= MAX_SKILL_BYTES => {
                                match flat_skill_data(
                                    &path.to_string_lossy(),
                                    &filename,
                                    &content,
                                    entry.level,
                                    entry.slot,
                                ) {
                                    Ok(data) => {
                                        let mut candidate = SkillCandidate::from_data(
                                            data,
                                            entry.slot,
                                            entry.source_id,
                                            entry.source_label,
                                            entry.level.as_str(),
                                            entry.priority,
                                            false,
                                        );
                                        let relative = child_path
                                            .strip_prefix(&entry.path)
                                            .expect("discovered child")
                                            .components()
                                            .map(|part| part.as_os_str().to_string_lossy())
                                            .collect::<Vec<_>>()
                                            .join("/");
                                        set_nested_key(&mut candidate, &relative);
                                        scan.candidates.push(candidate);
                                    }
                                    Err(_) if entry.source_id == "pi" && filename != "SKILL.md" => {
                                    }
                                    Err(error) => scan.diagnostics.push(diagnostic(
                                        child_path.to_string_lossy(),
                                        entry.source_id,
                                        error,
                                    )),
                                }
                            }
                            Ok(_) => scan.diagnostics.push(diagnostic(
                                child_path.to_string_lossy(),
                                entry.source_id,
                                "Skill Markdown exceeds the discovery size limit",
                            )),
                            Err(error) => {
                                scan.cacheable = false;
                                scan.diagnostics.push(diagnostic(
                                    child_path.to_string_lossy(),
                                    entry.source_id,
                                    error,
                                ));
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        scan.diagnostics.push(diagnostic(
                            child_path.to_string_lossy(),
                            entry.source_id,
                            error,
                        ));
                        scan.cacheable = false;
                    }
                }
            }
            children.sort();
            for child in children {
                if pending.len() + visited.len() >= MAX_SCAN_DIRECTORIES {
                    scan.diagnostics.push(diagnostic(
                        path.to_string_lossy(),
                        entry.source_id,
                        "Skill directory traversal limit reached",
                    ));
                    scan.cacheable = false;
                    break;
                }
                pending.push_back((child, depth + 1));
            }
        }
        scan.candidates = sort_skill_candidates_by_dir(scan.candidates);
        scan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::workspace::WorkspaceDirEntry;
    use async_trait::async_trait;

    fn markdown(name: &str) -> String {
        format!("---\nname: {name}\ndescription: fixture\n---\n{name} body\n")
    }

    #[tokio::test]
    async fn dsh_and_pi_flat_skills_load_with_stable_keys_and_package_boundaries() {
        for (slot, source_id, source_label) in [
            ("dsh", "deepseek-harness", "DeepSeek Harness"),
            ("pi", "pi", "PI"),
        ] {
            let temp = tempfile::tempdir().unwrap();
            for (file, content) in [
                ("review.md", markdown("review")),
                ("review/SKILL.md", markdown("review")),
                ("review/example.md", markdown("ignored")),
                ("group/nested.md", markdown("nested")),
            ] {
                let path = temp.path().join(file);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, content).unwrap();
            }
            let entry = SkillRootEntry {
                path: temp.path().into(),
                level: SkillLocation::Project,
                slot,
                source_id,
                source_label,
                priority: 0,
                is_builtin: false,
            };
            let scan = SkillRegistry::scan_skills_in_dir(&entry).await;
            assert!(scan.diagnostics.is_empty());
            assert_eq!(scan.candidates.len(), if slot == "pi" { 3 } else { 2 });
            let flat = scan
                .candidates
                .iter()
                .find(|candidate| candidate.info.entry_file.as_deref() == Some("review.md"))
                .unwrap();
            assert_eq!(flat.info.key, format!("project::{slot}::review.md"));
            assert!(scan
                .candidates
                .iter()
                .any(|candidate| candidate.info.key == format!("project::{slot}::review")));
            let content = SkillRegistry::read_local_skill_markdown(&flat.info)
                .await
                .unwrap();
            assert!(content.contains("review body"));
            assert_eq!(flat.info.path, temp.path().to_string_lossy());
        }
    }

    struct FlatRemote;
    #[async_trait]
    impl WorkspaceFileSystem for FlatRemote {
        async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(self.read_file_text(path).await?.into_bytes())
        }
        async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
            if path == "/remote/.pi/skills/review.md" {
                return Ok("---\ndescription: remote PI skill\n---\nremote PI body".into());
            }
            if path == "/remote/.dsh/skills/review.md" {
                return Ok(markdown("dsh-review"));
            }
            anyhow::bail!("unexpected remote file")
        }
        async fn write_file(&self, _: &str, _: &[u8]) -> anyhow::Result<()> {
            unreachable!()
        }
        async fn exists(&self, path: &str) -> anyhow::Result<bool> {
            Ok(self.is_file(path).await? || self.is_dir(path).await?)
        }
        async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
            Ok(matches!(
                path,
                "/remote/.pi/skills/review.md" | "/remote/.dsh/skills/review.md"
            ))
        }
        async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
            Ok(matches!(path, "/remote/.pi/skills" | "/remote/.dsh/skills"))
        }
        async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            assert!(self.is_dir(path).await?);
            Ok(vec![WorkspaceDirEntry {
                name: "review.md".into(),
                path: format!("{path}/review.md"),
                is_dir: false,
                is_symlink: true,
                modified: None,
            }])
        }
    }

    #[tokio::test]
    async fn remote_flat_skill_discovery_and_loading_use_remote_posix_paths() {
        let scan = SkillRegistry::scan_remote_project_skills(&FlatRemote, "/remote").await;
        assert!(scan.diagnostics.is_empty());
        assert_eq!(scan.candidates.len(), 2);
        let pi = scan
            .candidates
            .iter()
            .find(|candidate| candidate.info.source_id == "pi")
            .unwrap();
        assert_eq!(pi.info.key, "project::pi::review.md");
        // PI derives a missing name from the containing directory, including
        // for a flat Markdown file (not from its filename).
        assert_eq!(pi.info.name, "skills");
        assert_eq!(pi.info.path, "/remote/.pi/skills");
        let content = SkillRegistry::read_skill_md_for_remote_merge(&pi.info, &FlatRemote)
            .await
            .unwrap();
        assert!(content.contains("remote PI body"));
    }

    #[tokio::test]
    async fn nested_local_skills_keep_distinct_keys_and_partial_failures() {
        let temp = tempfile::tempdir().unwrap();
        for path in [
            ".system/shared",
            "interactive/shared",
            "direct",
            "direct/examples/ignored",
            "broken",
        ] {
            std::fs::create_dir_all(temp.path().join(path)).unwrap();
            std::fs::write(
                temp.path().join(path).join("SKILL.md"),
                if path == "broken" {
                    "invalid".into()
                } else {
                    markdown("shared")
                },
            )
            .unwrap();
        }
        let entry = SkillRootEntry {
            path: temp.path().to_path_buf(),
            level: SkillLocation::User,
            slot: "home.codex",
            source_id: "codex",
            source_label: "Codex",
            priority: 0,
            is_builtin: false,
        };
        let scan = SkillRegistry::scan_skills_in_dir(&entry).await;
        let keys: HashSet<_> = scan
            .candidates
            .iter()
            .map(|item| item.info.key.as_str())
            .collect();
        assert_eq!(
            keys,
            HashSet::from([
                "user::home.codex::.system/shared",
                "user::home.codex::interactive/shared",
                "user::home.codex::direct"
            ])
        );
        assert_eq!(scan.diagnostics.len(), 1);
        assert!(
            scan.diagnostics[0].path.ends_with("broken\\SKILL.md")
                || scan.diagnostics[0].path.ends_with("broken/SKILL.md")
        );
    }

    struct RemoteFixture;
    #[async_trait]
    impl WorkspaceFileSystem for RemoteFixture {
        async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(self.read_file_text(path).await?.into_bytes())
        }
        async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
            if path.ends_with("/bad/SKILL.md") {
                return Ok("invalid".into());
            }
            if path.ends_with("/shared/SKILL.md") {
                return Ok(markdown("shared"));
            }
            anyhow::bail!("missing fixture file")
        }
        async fn write_file(&self, _: &str, _: &[u8]) -> anyhow::Result<()> {
            anyhow::bail!("read-only fixture")
        }
        async fn exists(&self, path: &str) -> anyhow::Result<bool> {
            self.is_file(path).await
        }
        async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
            Ok(path.ends_with("/shared/SKILL.md") || path.ends_with("/bad/SKILL.md"))
        }
        async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
            Ok(path.starts_with("/remote/.codex/skills"))
        }
        async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            if path.ends_with("/unreadable") {
                anyhow::bail!("permission denied");
            }
            let names = if path == "/remote/.codex/skills" {
                vec![".system", "linked", "bad", "unreadable", "loop"]
            } else if path.ends_with("/loop") {
                vec!["loop"]
            } else {
                vec!["shared"]
            };
            Ok(names
                .into_iter()
                .map(|name| WorkspaceDirEntry {
                    name: name.into(),
                    path: format!("{path}/{name}"),
                    is_dir: name != "linked",
                    is_symlink: name == "linked" || name == "loop",
                    modified: None,
                })
                .collect())
        }
    }

    #[tokio::test]
    async fn remote_nested_links_errors_cycles_and_project_priority() {
        let scan = SkillRegistry::scan_remote_project_skills(&RemoteFixture, "/remote").await;
        assert_eq!(scan.candidates.len(), 2);
        assert!(scan
            .candidates
            .iter()
            .any(|item| item.info.key == "project::codex::linked/shared"));
        assert!(scan
            .candidates
            .iter()
            .any(|item| item.info.key == "project::codex::.system/shared"));
        assert!(scan
            .diagnostics
            .iter()
            .any(|item| item.message.contains("permission denied")));
        assert!(scan
            .diagnostics
            .iter()
            .any(|item| item.path.ends_with("/bad/SKILL.md")));
        assert!(scan
            .diagnostics
            .iter()
            .any(|item| item.message.contains("traversal limit")));
        let mut user_candidate = scan.candidates[0].clone();
        user_candidate.info.key = "user::home.claude::shared".into();
        user_candidate.info.level = SkillLocation::User;
        user_candidate.priority = 0;
        let merged = SkillRegistry::merge_remote_skill_scans(
            SkillCandidateScan {
                candidates: vec![user_candidate],
                diagnostics: vec![],
            },
            scan,
        );
        let resolved = resolve_visible_skills(merged.candidates);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].level, SkillLocation::Project);
    }
}
