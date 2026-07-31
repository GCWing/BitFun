//! Automatic hdc fallback for `start_app`.
//!
//! When devecocli cannot enumerate devices or drive a run (missing binary,
//! permission/PATH issues, no devices visible, or `devecocli run` fails),
//! `start_app` delegates here so the overall deploy+launch flow stays smooth
//! instead of forcing the model to hand-drive a multi-step hdc sequence.
//!
//! The fallback drives `hdc` directly through the same shell wrapper as
//! devecocli (so SDK paths only present in shell profiles are honored):
//!   1. `hdc list targets`            — discover devices devecocli could not.
//!   2. resolve `hvd` against targets — match by serial/connect key, or pick
//!      the sole device when `hvd` is omitted; list and bail when ambiguous.
//!   3. find the built `*.hap`         — prefer `build/outputs/<target>/` then
//!      a bounded recursive walk of the project, skipping `node_modules` /
//!      `.git` / `target`.
//!   4. read `bundleName` from `AppScope/app.json5` (json5 → regex extract).
//!   5. `hdc -t <serial> install -r <hap>`   — (re)install the HAP.
//!   6. `hdc -t <serial> shell aa start -a <ability> -b <bundleName>`.
//!
//! All steps are logged; a single `ToolResult` summarizes the outcome so the
//! model can continue without a second tool round-trip.

use super::devecocli_run::{resolve_harmony_cwd, run_hdc, DevecocliOptions, DevecocliOutput};
use crate::agentic::tools::framework::{ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

const HDC_START_TIMEOUT: Duration = Duration::from_secs(300);
const HAP_SEARCH_SKIP_DIRS: &[&str] = &["node_modules", ".git", "target", ".hvigor", "oh_modules"];

/// Outcome of a successful hdc fallback run.
struct HdcFallbackSuccess {
    device: String,
    hap_path: String,
    bundle_name: String,
    install_summary: String,
    start_summary: String,
}

/// Run the full hdc-based start path. Returns a ready `ToolResult` on success.
pub(crate) async fn run_hdc_start_fallback(
    hvd: Option<&str>,
    module: &str,
    target: &str,
    ability: &str,
    context: &ToolUseContext,
) -> BitFunResult<ToolResult> {
    let cwd_str = resolve_harmony_cwd(context);
    let cwd = PathBuf::from(&cwd_str);

    // 1. List devices via hdc directly.
    let targets = hdc_list_targets(context).await?;
    if targets.is_empty() {
        return Err(BitFunError::tool(
            "hdc found no devices either. devecocli and hdc both cannot enumerate a connected device/emulator. \
             Check USB debugging / wireless debugging (system settings → developer options), then hdc tconn <ip:port> for network devices."
                .to_string(),
        ));
    }

    // 2. Resolve the target device.
    let device = match resolve_hdc_target(hvd, &targets)? {
        TargetChoice::Single(d) => d,
        TargetChoice::Ambiguous(listing) => {
            // Multiple devices and no `hvd` selector: list and let the model
            // pick. Do not auto-install on an ambiguous device set.
            return Ok(listing);
        }
    };

    // 3. Find the built HAP.
    let hap = find_hap(&cwd, module, target).ok_or_else(|| {
        BitFunError::tool(format!(
            "No built .hap found under {} (looked for module \"{}\", target \"{}\"). \
             Run build_project first, then start_app again.",
            cwd.display(),
            module,
            target
        ))
    })?;
    let hap_path = hap.to_string_lossy().to_string();

    // 4. Read bundleName from AppScope/app.json5.
    let bundle_name = read_bundle_name(&cwd).ok_or_else(|| {
        BitFunError::tool(format!(
            "Could not read bundleName from {}/AppScope/app.json5. \
             Ensure the project is a HarmonyOS project with an AppScope/app.json5 containing app.bundleName.",
            cwd.display()
        ))
    })?;

    // 5. (Re)install the HAP on the resolved device.
    let install_out = hdc_install(&device, &hap_path, context).await?;
    let install_combined = combine_output(&install_out);
    if install_out.exit_code != 0 {
        return Err(BitFunError::tool(format!(
            "hdc install failed (exit {}):\n{}",
            install_out.exit_code, install_combined
        )));
    }

    // 6. Launch the ability on the resolved device.
    let start_out = hdc_aa_start(&device, ability, &bundle_name, context).await?;
    let start_combined = combine_output(&start_out);
    if start_out.exit_code != 0 {
        return Err(BitFunError::tool(format!(
            "hdc aa start failed (exit {}):\n{}",
            start_out.exit_code, start_combined
        )));
    }

    let success = HdcFallbackSuccess {
        device: device.clone(),
        hap_path: hap_path.clone(),
        bundle_name: bundle_name.clone(),
        install_summary: install_combined,
        start_summary: start_combined,
    };

    Ok(format_success_result(success))
}

// ---------- device discovery ----------

/// Parse `hdc list targets` output into a list of connect keys (serials /
/// `ip:port`). The raw output is one target per line, possibly with an
/// `Empty`/`[Empty]` placeholder when nothing is connected.
fn parse_hdc_targets(output: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if lower == "empty" || lower == "[empty]" || lower.contains("no device") {
            continue;
        }
        // Skip obvious headers that some hdc versions print.
        if lower.starts_with("connect key") || lower.starts_with("list of") {
            continue;
        }
        if !trimmed.chars().any(|c| c.is_alphanumeric()) {
            continue;
        }
        out.push(trimmed.to_string());
    }
    out
}

async fn hdc_list_targets(context: &ToolUseContext) -> BitFunResult<Vec<String>> {
    let out = run_hdc(&["list", "targets"], context, DevecocliOptions::default()).await?;
    if out.exit_code != 0 {
        return Err(BitFunError::tool(format!(
            "hdc list targets failed (exit {}):\n{}",
            out.exit_code,
            if out.stderr.is_empty() {
                out.stdout.clone()
            } else {
                out.stderr.clone()
            }
        )));
    }
    Ok(parse_hdc_targets(&format!("{}\n{}", out.stdout, out.stderr)))
}

enum TargetChoice {
    Single(String),
    Ambiguous(ToolResult),
}

fn resolve_hdc_target(hvd: Option<&str>, targets: &[String]) -> BitFunResult<TargetChoice> {
    let hvd = hvd.map(|h| h.trim()).filter(|h| !h.is_empty());

    let Some(query) = hvd else {
        // No selector: auto-pick only when exactly one device is connected.
        return if targets.len() == 1 {
            Ok(TargetChoice::Single(targets[0].clone()))
        } else {
            Ok(TargetChoice::Ambiguous(format_target_list(targets)))
        };
    };

    // Match by normalized substring both ways (serial / ip:port).
    let q = normalize(query);
    for t in targets {
        let n = normalize(t);
        if !n.is_empty() && (n == q || n.contains(&q) || q.contains(&n)) {
            return Ok(TargetChoice::Single(t.clone()));
        }
    }

    Err(BitFunError::tool(format!(
        "Device \"{}\" not found by hdc. hdc targets:\n{}",
        query,
        targets.iter().map(|t| format!("- {}", t)).collect::<Vec<_>>().join("\n")
    )))
}

fn format_target_list(targets: &[String]) -> ToolResult {
    let mut lines = vec!["Multiple HarmonyOS devices found by hdc:".to_string()];
    for (i, t) in targets.iter().enumerate() {
        lines.push(format!("{}. {}", i + 1, t));
    }
    lines.push(
        "Specify the target with the `hvd` parameter (e.g. {\"hvd\": \"<connect-key>\"}) and call start_app again."
            .to_string(),
    );
    ToolResult::Result {
        data: json!({
            "tool": "start_app",
            "action": "list",
            "source": "hdc",
            "deviceCount": targets.len(),
        }),
        result_for_assistant: Some(lines.join("\n")),
        image_attachments: None,
    }
}

// ---------- HAP discovery ----------

/// Find the built `*.hap`, preferring `build/outputs/<target>/` then a bounded
/// recursive walk. Prefers HAPs whose name starts with the module and prefers
/// signed bundles.
fn find_hap(cwd: &Path, module: &str, target: &str) -> Option<PathBuf> {
    // Preferred location: build/outputs/<target>/*.hap
    let preferred_root = cwd.join("build").join("outputs").join(target);
    if let Some(h) = pick_best_hap(&walk_dir(&preferred_root, 3), module) {
        return Some(h);
    }
    // Broader: any build/outputs/**/*.hap
    let outputs_root = cwd.join("build").join("outputs");
    if let Some(h) = pick_best_hap(&walk_dir(&outputs_root, 5), module) {
        return Some(h);
    }
    // Last resort: bounded recursive walk from project root, skipping heavy dirs.
    let mut found = walk_dir_skipping(cwd, 8);
    if let Some(h) = pick_best_hap(&mut found, module) {
        return Some(h);
    }
    None
}

/// Recursively collect `*.hap` files under `root` up to `max_depth`.
fn walk_dir(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_into(root, max_depth, &[], &mut out);
    out
}

/// Recursive walk from `root` skipping heavy/irrelevant directories.
fn walk_dir_skipping(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_into(root, max_depth, HAP_SEARCH_SKIP_DIRS, &mut out);
    out
}

fn walk_into(dir: &Path, depth_left: usize, skip: &[&str], out: &mut Vec<PathBuf>) {
    if depth_left == 0 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ft.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if skip.iter().any(|s| *s == name) {
                continue;
            }
            walk_into(&path, depth_left - 1, skip, out);
        } else if ft.is_file() {
            if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("hap")).unwrap_or(false) {
                out.push(path);
            }
        }
    }
}

/// Score and pick the best HAP candidate: prefer filename starting with module,
/// prefer `signed`, prefer paths under `outputs`, and prefer shallower paths.
fn pick_best_hap(candidates: &[PathBuf], module: &str) -> Option<PathBuf> {
    if candidates.is_empty() {
        return None;
    }
    let module_lower = module.to_lowercase();
    let mut best: Option<(PathBuf, i64)> = None;
    for c in candidates {
        let name = c
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_lowercase())
            .unwrap_or_default();
        let mut score: i64 = 0;
        if !module_lower.is_empty() {
            if name.starts_with(&module_lower) {
                score += 10;
            } else if name.contains(&module_lower) {
                score += 4;
            }
        }
        if name.contains("signed") {
            score += 3;
        }
        if name.contains("default") {
            score += 1;
        }
        if c.to_string_lossy().to_lowercase().contains("outputs") {
            score += 2;
        }
        // Prefer shallower (shorter) paths on ties.
        let depth = c.components().count() as i64;
        let score = score * 1000 - depth;
        match &best {
            Some((_, prev)) if score <= *prev => {}
            _ => best = Some((c.clone(), score)),
        }
    }
    best.map(|(p, _)| p)
}

// ---------- bundleName extraction ----------

/// Extract `app.bundleName` from `AppScope/app.json5`. json5 permits comments
/// and trailing commas, so parse via regex instead of `serde_json`.
fn read_bundle_name(cwd: &Path) -> Option<String> {
    let app_json5 = cwd.join("AppScope").join("app.json5");
    let content = std::fs::read_to_string(&app_json5).ok()?;
    extract_json5_string_field(&content, "bundleName")
}

/// Best-effort extraction of a `"field": "value"` string field from json5 text,
/// ignoring commented-out lines (`//` or `/* */`).
fn extract_json5_string_field(content: &str, field: &str) -> Option<String> {
    let pat = format!("\"{}\"", field);
    for line in content.lines() {
        let stripped = strip_json5_comment(line);
        if !stripped.contains(&pat) {
            continue;
        }
        // Find the first string literal after the field name.
        let after = stripped.split(&pat).nth(1)?;
        if let Some(start) = after.find('"') {
            let rest = &after[start + 1..];
            if let Some(end) = rest.find('"') {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

/// Remove `//` line comments and trailing `/* */` markers from a single line.
fn strip_json5_comment(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            out.push(c as char);
            if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            break; // rest of line is a comment
        }
        out.push(c as char);
        i += 1;
    }
    out
}

// ---------- hdc install / aa start ----------

async fn hdc_install(
    device: &str,
    hap_path: &str,
    context: &ToolUseContext,
) -> BitFunResult<DevecocliOutput> {
    let quoted = quote_shell_arg(hap_path);
    let args: Vec<String> = vec![
        "-t".to_string(),
        device.to_string(),
        "install".to_string(),
        "-r".to_string(),
        quoted,
    ];
    let argv: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_hdc(&argv, context, DevecocliOptions { timeout: HDC_START_TIMEOUT, ..Default::default() }).await
}

async fn hdc_aa_start(
    device: &str,
    ability: &str,
    bundle_name: &str,
    context: &ToolUseContext,
) -> BitFunResult<DevecocliOutput> {
    let args: Vec<String> = vec![
        "-t".to_string(),
        device.to_string(),
        "shell".to_string(),
        "aa".to_string(),
        "start".to_string(),
        "-a".to_string(),
        ability.to_string(),
        "-b".to_string(),
        bundle_name.to_string(),
    ];
    let argv: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_hdc(&argv, context, DevecocliOptions { timeout: HDC_START_TIMEOUT, ..Default::default() }).await
}

// ---------- helpers ----------

fn combine_output(out: &DevecocliOutput) -> String {
    [out.stdout.as_str(), out.stderr.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join("")
}

/// Wrap a path in double quotes when it contains characters that a shell would
/// split on. `run_hdc` re-parses the joined command through the user's shell,
/// so quoting here mirrors what ExecCommand expects from typed commands.
fn quote_shell_arg(s: &str) -> String {
    if s.contains(' ') || s.contains('\t') {
        format!("\"{}\"", s)
    } else {
        s.to_string()
    }
}

fn format_success_result(s: HdcFallbackSuccess) -> ToolResult {
    let mut summary = vec![
        format!("[hdc fallback] device: {}", s.device),
        format!("[hdc fallback] hap: {}", s.hap_path),
        format!("[hdc fallback] bundleName: {}", s.bundle_name),
    ];
    if !s.install_summary.is_empty() {
        summary.push(format!("[hdc fallback] install:\n{}", s.install_summary));
    }
    if !s.start_summary.is_empty() {
        summary.push(format!("[hdc fallback] aa start:\n{}", s.start_summary));
    }
    summary.push("App started via hdc fallback.".to_string());
    ToolResult::Result {
        data: json!({
            "tool": "start_app",
            "action": "run",
            "source": "hdc_fallback",
            "device": s.device,
            "hap": s.hap_path,
            "bundleName": s.bundle_name,
        }),
        result_for_assistant: Some(summary.join("\n\n")),
        image_attachments: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hdc_targets_skips_empty_and_headers() {
        assert!(parse_hdc_targets("Empty").is_empty());
        assert!(parse_hdc_targets("[Empty]").is_empty());
        assert!(parse_hdc_targets("No device connected").is_empty());
        let out = parse_hdc_targets("Connect Key\n127.0.0.1:5555\nABC12345");
        assert_eq!(out, vec!["127.0.0.1:5555".to_string(), "ABC12345".to_string()]);
    }

    #[test]
    fn resolve_hdc_target_auto_picks_sole_device() {
        let r = resolve_hdc_target(None, &["abc".to_string()]).unwrap();
        match r {
            TargetChoice::Single(d) => assert_eq!(d, "abc"),
            _ => panic!("expected single"),
        }
    }

    #[test]
    fn resolve_hdc_target_ambiguous_when_multiple_and_no_selector() {
        let r = resolve_hdc_target(None, &["a".to_string(), "b".to_string()]).unwrap();
        assert!(matches!(r, TargetChoice::Ambiguous(_)));
    }

    #[test]
    fn resolve_hdc_target_matches_by_substring() {
        let r = resolve_hdc_target(Some("5555"), &["127.0.0.1:5555".to_string(), "ABC".to_string()]).unwrap();
        match r {
            TargetChoice::Single(d) => assert_eq!(d, "127.0.0.1:5555"),
            _ => panic!("expected single"),
        }
    }

    #[test]
    fn resolve_hdc_target_errors_when_no_match() {
        let r = resolve_hdc_target(Some("nope"), &["127.0.0.1:5555".to_string()]);
        assert!(r.is_err());
    }

    #[test]
    fn extract_bundle_name_from_json5_with_comments() {
        let content = r#"{
  // app config
  "app": {
    "bundleName": "com.example.myapp", /* primary */
    "vendor": "example",
  }
}"#;
        assert_eq!(
            extract_json5_string_field(content, "bundleName").as_deref(),
            Some("com.example.myapp")
        );
    }

    #[test]
    fn extract_bundle_name_ignores_commented_out_field() {
        let content = r#"{
  // "bundleName": "ignored.example",
  "bundleName": "real.example",
}"#;
        assert_eq!(
            extract_json5_string_field(content, "bundleName").as_deref(),
            Some("real.example")
        );
    }

    #[test]
    fn quote_shell_arg_only_when_needed() {
        assert_eq!(quote_shell_arg("C:/build/x.hap"), "C:/build/x.hap");
        assert_eq!(quote_shell_arg("C:/my build/x.hap"), "\"C:/my build/x.hap\"");
    }

    #[test]
    fn normalize_collapses_whitespace_and_case() {
        assert_eq!(normalize("  ABC 123 "), "abc123");
    }
}
