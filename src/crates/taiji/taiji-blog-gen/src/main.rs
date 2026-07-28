//! 太极博客生成器 — 将 Agent 分析 JSON 转换为 Hugo Markdown 博文。
//!
//! ## Template rendering: Tera
//!
//! This crate uses the **Tera** template engine (`tera::Tera`) for rendering
//! Hugo Markdown from agent analysis JSON. Templates are compiled at build time
//! via `include_str!` and registered with `tera.add_raw_template()`.
//!
//! ### Cross-reference: `taiji-content` uses manual `String::replace`
//!
//! [`taiji-content::chart_option`](../taiji-content/src/chart_option.rs) renders
//! ECharts option JSON templates using **manual `String::replace`** on
//! `{{variable}}` placeholders. That approach has zero dependencies and compiles
//! fast, but lacks conditionals, loops, and filters.
//!
//! ### Recommended unification
//!
//! The two crates currently use **different template rendering strategies**:
//!
//! | Crate | Engine | Pros | Cons |
//! |---|---|---|---|
//! | `taiji-blog-gen` (`main.rs`) | Tera v1 | Full template logic, typed context | Adds a dependency |
//! | `taiji-content` (`chart_option.rs`) | Manual `String::replace` | Zero deps, fast compile | Fragile; no conditionals/loops/filters |
//!
//! **Recommendation**: Eventually migrate `taiji-content::chart_option` to Tera
//! (or extract a shared `taiji-templates` helper crate) so all taiji crates use
//! one rendering engine. The current Tera usage in this crate serves as the
//! **reference pattern** for template features — prefer extending Tera rather
//! than adding more `String::replace` variants elsewhere.
//!
//! ## File system access
//!
//! This crate uses raw `std::fs` for file I/O and directory traversal.
//! TODO(taiji): Migrate to bitfun_services FileSystemService (`src/crates/services`)
//! for platform-agnostic path canonicalization, file reads, and directory operations.
//! The FileSystemService abstraction supports desktop, remote workspace, and
//! future WASM targets.
use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use tera::{Context as TeraContext, Tera};

/// 太极博客生成器 — 将 Agent 分析 JSON 转换为 Hugo Markdown 博文。
#[derive(Parser)]
#[command(name = "taiji-blog-gen", version)]
struct Cli {
    /// 单个 Agent JSON 输入文件路径
    #[arg(short, long, group = "input_mode")]
    input: Option<PathBuf>,

    /// 批量模式：从目录读取所有 JSON 文件
    #[arg(short = 'B', long, group = "input_mode")]
    batch: bool,

    /// 批量模式输入目录
    #[arg(long, requires = "batch")]
    input_dir: Option<PathBuf>,

    /// 输出目录（Markdown 文件写入位置）
    #[arg(short = 'o', long, default_value = "posts/")]
    output_dir: PathBuf,

    /// 博文模板：daily_post / weekly_summary / special_topic
    #[arg(short = 't', long, default_value = "daily_post")]
    template: String,
}

// ── Agent JSON input structure ──

#[derive(Debug, Deserialize)]
struct AgentInput {
    timestamp: Option<String>,
    instrument: Option<String>,
    freq: Option<String>,
    #[serde(default)]
    structure_agent: Option<AgentOutput>,
    #[serde(default)]
    delta_agent: Option<AgentOutput>,
    #[serde(default)]
    magnet_agent: Option<AgentOutput>,
    #[serde(default)]
    thrust_agent: Option<AgentOutput>,
    #[serde(default)]
    resonance_agent: Option<AgentOutput>,
    #[serde(default)]
    decision_agent: Option<AgentOutput>,
    #[serde(default)]
    risk_agent: Option<AgentOutput>,
}

#[derive(Debug, Deserialize)]
struct AgentOutput {
    #[serde(default)]
    analysis: Option<serde_json::Value>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    decision: Option<serde_json::Value>,
    #[serde(default)]
    constraints: Option<serde_json::Value>,
}

// ── Tag auto-mapping ──

fn map_tags(input: &AgentInput) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();

    // structure_agent: trend_direction
    if let Some(s) = &input.structure_agent {
        if let Some(analysis) = &s.analysis {
            if let Some(dir) = analysis["trend_direction"].as_str() {
                match dir {
                    "up" => tags.push("#多头".into()),
                    "down" => tags.push("#空头".into()),
                    "sideways" => tags.push("#震荡".into()),
                    _ => {}
                }
            }
        }
    }

    // magnet_agent: magnet_valid
    if let Some(m) = &input.magnet_agent {
        if let Some(analysis) = &m.analysis {
            match analysis["magnet_valid"].as_bool() {
                Some(true) => tags.push("#磁体共振".into()),
                Some(false) => tags.push("#无磁体".into()),
                None => {}
            }
        }
    }

    // thrust_agent: push_count >= 1
    if let Some(t) = &input.thrust_agent {
        if let Some(analysis) = &t.analysis {
            if let Some(count) = analysis["push_count"].as_u64() {
                if count >= 1 {
                    tags.push("#三推".into());
                }
            } else if analysis["triple_push_found"].as_bool() == Some(true) {
                tags.push("#三推".into());
            }
        }
    }

    // resonance_agent: resonance == true
    if let Some(r) = &input.resonance_agent {
        if let Some(analysis) = &r.analysis {
            if analysis["resonance"].as_bool() == Some(true) {
                tags.push("#共振".into());
                // 附加共振类型
                if let Some(rt) = analysis["resonance_type"].as_str() {
                    match rt {
                        "bullish" => tags.push("#多头共振".into()),
                        "bearish" => tags.push("#空头共振".into()),
                        _ => {}
                    }
                }
            }
        }
    }

    // delta_agent: net_position
    if let Some(d) = &input.delta_agent {
        if let Some(analysis) = &d.analysis {
            if let Some(np) = analysis["net_position"].as_str() {
                match np {
                    "long_building" | "short_liquidating" => tags.push("#多头净持仓".into()),
                    "short_building" | "long_liquidating" => tags.push("#空头净持仓".into()),
                    _ => {}
                }
            }
        }
    }

    // 品种标签
    if let Some(instr) = &input.instrument {
        tags.push(format!("#{}", instr));
    }

    tags
}

// ── Template rendering ──

fn build_tera_context(input: &AgentInput, tags: &[String]) -> TeraContext {
    let mut ctx = TeraContext::new();

    // 基础信息
    ctx.insert("timestamp", &input.timestamp.as_deref().unwrap_or(""));
    ctx.insert("instrument", &input.instrument.as_deref().unwrap_or(""));
    ctx.insert("freq", &input.freq.as_deref().unwrap_or(""));
    ctx.insert("tags", tags);

    // 各 Agent 置信度
    let conf =
        |a: &Option<AgentOutput>| -> f64 { a.as_ref().and_then(|o| o.confidence).unwrap_or(0.0) };
    ctx.insert("structure_confidence", &conf(&input.structure_agent));
    ctx.insert("delta_confidence", &conf(&input.delta_agent));
    ctx.insert("magnet_confidence", &conf(&input.magnet_agent));
    ctx.insert("thrust_confidence", &conf(&input.thrust_agent));
    ctx.insert("resonance_confidence", &conf(&input.resonance_agent));
    ctx.insert("decision_confidence", &conf(&input.decision_agent));

    // 各 Agent analysis 原始 JSON（模板中可按需取值）
    let analysis_json = |a: &Option<AgentOutput>| -> serde_json::Value {
        a.as_ref()
            .and_then(|o| o.analysis.clone())
            .unwrap_or(serde_json::Value::Null)
    };
    ctx.insert("structure", &analysis_json(&input.structure_agent));
    ctx.insert("delta", &analysis_json(&input.delta_agent));
    ctx.insert("magnet", &analysis_json(&input.magnet_agent));
    ctx.insert("thrust", &analysis_json(&input.thrust_agent));
    ctx.insert("resonance", &analysis_json(&input.resonance_agent));
    ctx.insert("risk", &analysis_json(&input.risk_agent));

    // decision 特殊处理
    let decision_json = |a: &Option<AgentOutput>| -> serde_json::Value {
        a.as_ref()
            .and_then(|o| o.decision.clone())
            .unwrap_or(serde_json::Value::Null)
    };
    ctx.insert("decision", &decision_json(&input.decision_agent));

    // constraints (risk)
    let constraints_json = |a: &Option<AgentOutput>| -> serde_json::Value {
        a.as_ref()
            .and_then(|o| o.constraints.clone())
            .unwrap_or(serde_json::Value::Null)
    };
    ctx.insert("constraints", &constraints_json(&input.risk_agent));

    // 生成时间
    ctx.insert("generated_at", &Utc::now().to_rfc3339());

    // 标签字符串（逗号分隔，用于 Hugo front matter）
    let tag_strings: Vec<String> = tags
        .iter()
        .map(|t| t.trim_start_matches('#').to_string())
        .collect();
    ctx.insert("tag_list", &tag_strings.join(", "));

    ctx
}

fn load_tera() -> Result<Tera> {
    let mut tera = Tera::default();

    // 嵌入模板
    tera.add_raw_template(
        "daily_post.tera",
        include_str!("../templates/daily_post.tera"),
    )
    .context("failed to load daily_post template")?;
    tera.add_raw_template(
        "weekly_summary.tera",
        include_str!("../templates/weekly_summary.tera"),
    )
    .context("failed to load weekly_summary template")?;
    tera.add_raw_template(
        "special_topic.tera",
        include_str!("../templates/special_topic.tera"),
    )
    .context("failed to load special_topic template")?;

    Ok(tera)
}

fn render_markdown(template_name: &str, ctx: &TeraContext) -> Result<String> {
    let tera = load_tera()?;
    let tmpl = format!("{}.tera", template_name);
    tera.render(&tmpl, ctx)
        .with_context(|| format!("failed to render template '{}'", tmpl))
}

// ── Main ──

fn process_single(input_path: &Path, output_dir: &Path, template: &str) -> Result<()> {
    // TODO(taiji): Migrate std::fs::canonicalize / read_to_string / create_dir_all / write
    // to bitfun_services FileSystemService for cross-platform and remote-workspace support.
    let input_path = std::fs::canonicalize(input_path)
        .with_context(|| format!("failed to resolve input path: {}", input_path.display()))?;
    let output_dir = std::fs::canonicalize(output_dir).unwrap_or_else(|_| output_dir.to_path_buf()); // output dir may not exist yet

    let raw = fs::read_to_string(&input_path)
        .with_context(|| format!("failed to read input: {}", input_path.display()))?;

    let input: AgentInput = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse Agent JSON: {}", input_path.display()))?;

    let tags = map_tags(&input);
    let ctx = build_tera_context(&input, &tags);
    let markdown = render_markdown(template, &ctx)?;

    // 生成文件名: {instrument}-{date}-{template}.md
    let instr = input.instrument.as_deref().unwrap_or("unknown");
    let date = Utc::now().format("%Y-%m-%d");
    let filename = format!("{}-{}-{}.md", instr, date, template);

    fs::create_dir_all(&output_dir)?;
    let out_path = output_dir.join(&filename);
    fs::write(&out_path, &markdown)
        .with_context(|| format!("failed to write: {}", out_path.display()))?;

    println!("[taiji-blog-gen] generated: {}", out_path.display());
    println!("  tags: {}", tags.join(", "));
    Ok(())
}

fn process_batch(input_dir: &Path, output_dir: &Path, template: &str) -> Result<()> {
    // TODO(taiji): Migrate fs::read_dir to bitfun_services FileSystemService.
    let mut count = 0;
    for entry in fs::read_dir(input_dir)
        .with_context(|| format!("failed to read input dir: {}", input_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            match process_single(&path, output_dir, template) {
                Ok(()) => count += 1,
                Err(e) => eprintln!(
                    "[taiji-blog-gen] error processing {}: {:#}",
                    path.display(),
                    e
                ),
            }
        }
    }
    println!("[taiji-blog-gen] batch complete: {} files generated", count);
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.batch {
        let input_dir = cli
            .input_dir
            .as_deref()
            .unwrap_or_else(|| Path::new("exports"));
        process_batch(input_dir, &cli.output_dir, &cli.template)?;
    } else if let Some(input) = &cli.input {
        process_single(input, &cli.output_dir, &cli.template)?;
    } else {
        // Default: read from stdin? But clap should handle this.
        anyhow::bail!("specify --input FILE or --batch --input-dir DIR");
    }

    Ok(())
}
