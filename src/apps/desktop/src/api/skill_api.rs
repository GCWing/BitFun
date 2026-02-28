//! Skill Management API

use log::info;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::OnceLock;
use tauri::State;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::time::{timeout, Duration};

use crate::api::app_state::AppState;
use bitfun_core::agentic::tools::implementations::skills::{
    SkillData, SkillLocation, SkillRegistry,
};
use bitfun_core::infrastructure::{get_path_manager_arc, get_workspace_path};
use bitfun_core::service::runtime::RuntimeManager;
use bitfun_core::util::process_manager;

const SKILLS_SEARCH_API_BASE: &str = "https://skills.sh";
const DEFAULT_MARKET_QUERY: &str = "skill";
const DEFAULT_MARKET_LIMIT: u32 = 12;
const MAX_MARKET_LIMIT: u32 = 500;
const MAX_OUTPUT_PREVIEW_CHARS: usize = 2000;
const MARKET_DESC_FETCH_TIMEOUT_SECS: u64 = 4;
const MARKET_DESC_FETCH_CONCURRENCY: usize = 6;
const MARKET_DESC_MAX_LEN: usize = 220;

static MARKET_DESCRIPTION_CACHE: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillValidationResult {
    pub valid: bool,
    pub name: Option<String>,
    pub description: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMarketListRequest {
    pub query: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMarketSearchRequest {
    pub query: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMarketDownloadRequest {
    pub package: String,
    pub level: Option<SkillLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMarketDownloadResponse {
    pub package: String,
    pub level: SkillLocation,
    pub installed_skills: Vec<String>,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMarketItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub installs: u64,
    pub url: String,
    pub install_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SkillSearchApiResponse {
    #[serde(default)]
    skills: Vec<SkillSearchApiItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct SkillSearchApiItem {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    installs: u64,
}

#[tauri::command]
pub async fn get_skill_configs(
    _state: State<'_, AppState>,
    force_refresh: Option<bool>,
) -> Result<Value, String> {
    let registry = SkillRegistry::global();

    if force_refresh.unwrap_or(false) {
        registry.refresh().await;
    }

    let all_skills = registry.get_all_skills().await;

    serde_json::to_value(all_skills)
        .map_err(|e| format!("Failed to serialize skill configs: {}", e))
}

#[tauri::command]
pub async fn set_skill_enabled(
    _state: State<'_, AppState>,
    skill_name: String,
    enabled: bool,
) -> Result<String, String> {
    let registry = SkillRegistry::global();

    let skill_md_path = registry
        .find_skill_path(&skill_name)
        .await
        .ok_or_else(|| format!("Skill '{}' not found", skill_name))?;

    SkillData::set_enabled_and_save(
        skill_md_path
            .to_str()
            .ok_or_else(|| "Invalid path".to_string())?,
        enabled,
    )
    .map_err(|e| format!("Failed to save skill config: {}", e))?;

    registry.update_skill_enabled(&skill_name, enabled).await;

    Ok(format!(
        "Skill '{}' configuration saved successfully",
        skill_name
    ))
}

#[tauri::command]
pub async fn validate_skill_path(path: String) -> Result<SkillValidationResult, String> {
    use std::path::Path;

    let skill_path = Path::new(&path);

    if !skill_path.exists() {
        return Ok(SkillValidationResult {
            valid: false,
            name: None,
            description: None,
            error: Some("Path does not exist".to_string()),
        });
    }

    if !skill_path.is_dir() {
        return Ok(SkillValidationResult {
            valid: false,
            name: None,
            description: None,
            error: Some("Path is not a directory".to_string()),
        });
    }

    let skill_md_path = skill_path.join("SKILL.md");
    if !skill_md_path.exists() {
        return Ok(SkillValidationResult {
            valid: false,
            name: None,
            description: None,
            error: Some("Directory is missing SKILL.md file".to_string()),
        });
    }

    match tokio::fs::read_to_string(&skill_md_path).await {
        Ok(content) => {
            match SkillData::from_markdown(path.clone(), &content, SkillLocation::User, false) {
                Ok(data) => Ok(SkillValidationResult {
                    valid: true,
                    name: Some(data.name),
                    description: Some(data.description),
                    error: None,
                }),
                Err(e) => Ok(SkillValidationResult {
                    valid: false,
                    name: None,
                    description: None,
                    error: Some(e.to_string()),
                }),
            }
        }
        Err(e) => Ok(SkillValidationResult {
            valid: false,
            name: None,
            description: None,
            error: Some(format!("Failed to read SKILL.md: {}", e)),
        }),
    }
}

#[tauri::command]
pub async fn add_skill(
    _state: State<'_, AppState>,
    source_path: String,
    level: String,
) -> Result<String, String> {
    use std::path::Path;

    let validation = validate_skill_path(source_path.clone()).await?;
    if !validation.valid {
        return Err(validation.error.unwrap_or("Invalid skill path".to_string()));
    }

    let skill_name = validation
        .name
        .as_ref()
        .ok_or_else(|| "Skill name missing after validation".to_string())?;
    let source = Path::new(&source_path);

    let target_dir = if level == "project" {
        if let Some(workspace_path) = get_workspace_path() {
            workspace_path.join(".bitfun").join("skills")
        } else {
            return Err("No workspace open, cannot add project-level Skill".to_string());
        }
    } else {
        get_path_manager_arc().user_skills_dir()
    };

    if let Err(e) = tokio::fs::create_dir_all(&target_dir).await {
        return Err(format!("Failed to create skills directory: {}", e));
    }

    let folder_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Unable to get folder name")?;

    let target_path = target_dir.join(folder_name);

    if target_path.exists() {
        return Err(format!(
            "Skill '{}' already exists in {} level directory",
            folder_name,
            if level == "project" {
                "project"
            } else {
                "user"
            }
        ));
    }

    if let Err(e) = copy_dir_all(source, &target_path).await {
        return Err(format!("Failed to copy skill folder: {}", e));
    }

    SkillRegistry::global().refresh().await;

    info!(
        "Skill added: name={}, level={}, path={}",
        skill_name,
        level,
        target_path.display()
    );
    Ok(format!("Skill '{}' added successfully", skill_name))
}

async fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            Box::pin(copy_dir_all(&src_path, &dst_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_skill(
    _state: State<'_, AppState>,
    skill_name: String,
) -> Result<String, String> {
    let registry = SkillRegistry::global();

    let skill_info = registry
        .find_skill(&skill_name)
        .await
        .ok_or_else(|| format!("Skill '{}' not found", skill_name))?;

    let skill_path = std::path::PathBuf::from(&skill_info.path);

    if skill_path.exists() {
        if let Err(e) = tokio::fs::remove_dir_all(&skill_path).await {
            return Err(format!("Failed to delete skill folder: {}", e));
        }
    }

    registry.remove_skill(&skill_name).await;

    info!(
        "Skill deleted: name={}, path={}",
        skill_name,
        skill_path.display()
    );
    Ok(format!("Skill '{}' deleted successfully", skill_name))
}

#[tauri::command]
pub async fn list_skill_market(
    _state: State<'_, AppState>,
    request: SkillMarketListRequest,
) -> Result<Vec<SkillMarketItem>, String> {
    let query = request
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(DEFAULT_MARKET_QUERY);
    let limit = normalize_market_limit(request.limit);
    fetch_skill_market(query, limit).await
}

#[tauri::command]
pub async fn search_skill_market(
    _state: State<'_, AppState>,
    request: SkillMarketSearchRequest,
) -> Result<Vec<SkillMarketItem>, String> {
    let query = request.query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let limit = normalize_market_limit(request.limit);
    fetch_skill_market(query, limit).await
}

#[tauri::command]
pub async fn download_skill_market(
    _state: State<'_, AppState>,
    request: SkillMarketDownloadRequest,
) -> Result<SkillMarketDownloadResponse, String> {
    let package = request.package.trim().to_string();
    if package.is_empty() {
        return Err("Skill package cannot be empty".to_string());
    }

    let level = request.level.unwrap_or(SkillLocation::Project);
    let workspace_path = if level == SkillLocation::Project {
        Some(
            get_workspace_path()
                .ok_or_else(|| "No workspace open, cannot add project-level Skill".to_string())?,
        )
    } else {
        None
    };

    let registry = SkillRegistry::global();
    let before_names: HashSet<String> = registry
        .get_all_skills()
        .await
        .into_iter()
        .map(|skill| skill.name)
        .collect();

    let runtime_manager = RuntimeManager::new()
        .map_err(|e| format!("Failed to initialize runtime manager: {}", e))?;
    let resolved_npx = runtime_manager.resolve_command("npx").ok_or_else(|| {
        "Command 'npx' is not available. Install Node.js or configure BitFun runtimes.".to_string()
    })?;

    let mut command = process_manager::create_tokio_command(&resolved_npx.command);
    command
        .arg("-y")
        .arg("skills")
        .arg("add")
        .arg(&package)
        .arg("-y")
        .arg("-a")
        .arg("universal");

    if level == SkillLocation::User {
        command.arg("-g");
    }

    if let Some(path) = workspace_path.as_ref() {
        command.current_dir(path);
    }

    let current_path = std::env::var("PATH").ok();
    if let Some(merged_path) = runtime_manager.merged_path_env(current_path.as_deref()) {
        command.env("PATH", &merged_path);
        #[cfg(windows)]
        {
            command.env("Path", &merged_path);
        }
    }

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let output = command
        .output()
        .await
        .map_err(|e| format!("Failed to execute skills installer: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        let detail = if !stderr.trim().is_empty() {
            truncate_preview(stderr.trim())
        } else if !stdout.trim().is_empty() {
            truncate_preview(stdout.trim())
        } else {
            "Unknown installer error".to_string()
        };
        return Err(format!(
            "Failed to download skill package '{}' (exit code {}): {}",
            package, exit_code, detail
        ));
    }

    registry.refresh().await;
    let mut installed_skills: Vec<String> = registry
        .get_all_skills()
        .await
        .into_iter()
        .map(|skill| skill.name)
        .filter(|name| !before_names.contains(name))
        .collect();
    installed_skills.sort();
    installed_skills.dedup();

    info!(
        "Skill market download completed: package={}, level={}, installed_count={}",
        package,
        level.as_str(),
        installed_skills.len()
    );

    Ok(SkillMarketDownloadResponse {
        package,
        level,
        installed_skills,
        output: summarize_command_output(&stdout, &stderr),
    })
}

fn normalize_market_limit(value: Option<u32>) -> u32 {
    value
        .unwrap_or(DEFAULT_MARKET_LIMIT)
        .clamp(1, MAX_MARKET_LIMIT)
}

async fn fetch_skill_market(query: &str, limit: u32) -> Result<Vec<SkillMarketItem>, String> {
    let api_base =
        std::env::var("SKILLS_API_URL").unwrap_or_else(|_| SKILLS_SEARCH_API_BASE.into());
    let base_url = api_base.trim_end_matches('/');
    let endpoint = format!("{}/api/search", base_url);

    let client = Client::new();
    let response = client
        .get(&endpoint)
        .query(&[("q", query), ("limit", &limit.to_string())])
        .send()
        .await
        .map_err(|e| format!("Failed to query skill market: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Skill market request failed with status {}",
            response.status()
        ));
    }

    let payload: SkillSearchApiResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to decode skill market response: {}", e))?;

    let mut seen_install_ids: HashSet<String> = HashSet::new();
    let mut items = Vec::new();

    for raw in payload.skills {
        let source = raw.source.trim().to_string();
        let install_id = if source.is_empty() {
            if raw.id.contains('@') {
                raw.id.clone()
            } else {
                format!("{}@{}", raw.id, raw.name)
            }
        } else {
            format!("{}@{}", source, raw.name)
        };

        if !seen_install_ids.insert(install_id.clone()) {
            continue;
        }

        items.push(SkillMarketItem {
            id: raw.id.clone(),
            name: raw.name,
            description: raw.description,
            source,
            installs: raw.installs,
            url: format!("{}/{}", base_url, raw.id.trim_start_matches('/')),
            install_id,
        });
    }

    fill_market_descriptions(&client, base_url, &mut items).await;

    Ok(items)
}

fn summarize_command_output(stdout: &str, stderr: &str) -> String {
    let primary = if !stdout.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };

    if primary.is_empty() {
        return "Skill downloaded successfully.".to_string();
    }

    truncate_preview(primary)
}

fn truncate_preview(text: &str) -> String {
    if text.chars().count() <= MAX_OUTPUT_PREVIEW_CHARS {
        return text.to_string();
    }

    let truncated: String = text.chars().take(MAX_OUTPUT_PREVIEW_CHARS).collect();
    format!("{}...", truncated)
}

fn market_description_cache() -> &'static RwLock<HashMap<String, String>> {
    MARKET_DESCRIPTION_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

async fn fill_market_descriptions(client: &Client, base_url: &str, items: &mut [SkillMarketItem]) {
    let cache = market_description_cache();

    {
        let reader = cache.read().await;
        for item in items.iter_mut() {
            if !item.description.trim().is_empty() {
                continue;
            }
            if let Some(cached) = reader.get(&item.id) {
                item.description = cached.clone();
            }
        }
    }

    let mut missing_ids = Vec::new();
    for item in items.iter() {
        if item.description.trim().is_empty() {
            missing_ids.push(item.id.clone());
        }
    }

    if missing_ids.is_empty() {
        return;
    }

    let mut join_set = JoinSet::new();
    let mut fetched = HashMap::new();

    for skill_id in missing_ids {
        let client_clone = client.clone();
        let page_url = format!("{}/{}", base_url, skill_id.trim_start_matches('/'));

        join_set.spawn(async move {
            let description = fetch_description_from_skill_page(&client_clone, &page_url).await;
            (skill_id, description)
        });

        if join_set.len() >= MARKET_DESC_FETCH_CONCURRENCY {
            if let Some(result) = join_set.join_next().await {
                if let Ok((skill_id, Some(desc))) = result {
                    fetched.insert(skill_id, desc);
                }
            }
        }
    }

    while let Some(result) = join_set.join_next().await {
        if let Ok((skill_id, Some(desc))) = result {
            fetched.insert(skill_id, desc);
        }
    }

    if fetched.is_empty() {
        return;
    }

    {
        let mut writer = cache.write().await;
        for (skill_id, desc) in &fetched {
            writer.insert(skill_id.clone(), desc.clone());
        }
    }

    for item in items.iter_mut() {
        if item.description.trim().is_empty() {
            if let Some(desc) = fetched.get(&item.id) {
                item.description = desc.clone();
            }
        }
    }
}

async fn fetch_description_from_skill_page(client: &Client, page_url: &str) -> Option<String> {
    let response = timeout(
        Duration::from_secs(MARKET_DESC_FETCH_TIMEOUT_SECS),
        client.get(page_url).send(),
    )
    .await
    .ok()?
    .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let html = timeout(
        Duration::from_secs(MARKET_DESC_FETCH_TIMEOUT_SECS),
        response.text(),
    )
    .await
    .ok()?
    .ok()?;

    extract_description_from_html(&html)
}

fn extract_description_from_html(html: &str) -> Option<String> {
    if let Some(prose_index) = html.find("class=\"prose") {
        let scope = &html[prose_index..];
        if let Some(p_start) = scope.find("<p>") {
            let content = &scope[p_start + 3..];
            if let Some(p_end) = content.find("</p>") {
                let raw = &content[..p_end];
                let normalized = normalize_html_text(raw);
                if !normalized.is_empty() {
                    return Some(limit_text_len(&normalized, MARKET_DESC_MAX_LEN));
                }
            }
        }
    }

    if let Some(twitter_desc) = extract_meta_content(html, "twitter:description") {
        let normalized = normalize_html_text(&twitter_desc);
        if is_meaningful_meta_description(&normalized) {
            return Some(limit_text_len(&normalized, MARKET_DESC_MAX_LEN));
        }
    }

    None
}

fn extract_meta_content(html: &str, key: &str) -> Option<String> {
    let pattern = format!(r#"<meta name="{}" content="([^"]+)""#, regex::escape(key));
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(html)?;
    Some(caps.get(1)?.as_str().to_string())
}

fn normalize_html_text(raw: &str) -> String {
    let without_tags = if let Ok(re) = Regex::new(r"<[^>]+>") {
        re.replace_all(raw, " ").into_owned()
    } else {
        raw.to_string()
    };

    without_tags
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn is_meaningful_meta_description(text: &str) -> bool {
    let lower = text.to_lowercase();
    if lower.is_empty() {
        return false;
    }

    if lower == "discover and install skills for ai agents." {
        return false;
    }

    !lower.starts_with("install the ")
}

fn limit_text_len(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }

    let mut truncated: String = text.chars().take(max_len).collect();
    truncated.push_str("...");
    truncated
}
