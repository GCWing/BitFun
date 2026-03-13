use crate::agentic::insights::cancellation;
use crate::agentic::insights::collector::InsightsCollector;
use crate::agentic::insights::html::generate_html;
use crate::agentic::insights::types::*;
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::infrastructure::ai::AIClient;
use crate::infrastructure::events::{emit_global_event, BackendEvent};
use crate::infrastructure::get_path_manager_arc;
use crate::service::config::get_global_config_service;
use crate::service::config::AppConfig;
use crate::service::workspace::get_global_workspace_service;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::Message;
use log::{debug, info, warn};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

const FACET_PROMPT_TEMPLATE: &str = include_str!("prompts/facet_extraction.md");
const SUGGESTIONS_PROMPT_TEMPLATE: &str = include_str!("prompts/suggestions.md");
const AREAS_PROMPT_TEMPLATE: &str = include_str!("prompts/areas.md");
const WINS_PROMPT_TEMPLATE: &str = include_str!("prompts/wins.md");
const FRICTION_PROMPT_TEMPLATE: &str = include_str!("prompts/friction.md");
const INTERACTION_STYLE_PROMPT_TEMPLATE: &str = include_str!("prompts/interaction_style.md");
const AT_A_GLANCE_PROMPT_TEMPLATE: &str = include_str!("prompts/at_a_glance.md");
const HORIZON_PROMPT_TEMPLATE: &str = include_str!("prompts/horizon.md");
const FUN_ENDING_PROMPT_TEMPLATE: &str = include_str!("prompts/fun_ending.md");

const MAX_CONCURRENT_FACET_EXTRACTIONS: usize = 5;

pub struct InsightsService;

impl InsightsService {
    async fn get_user_language() -> String {
        match get_global_config_service().await {
            Ok(config_service) => {
                match config_service.get_config::<AppConfig>(Some("app")).await {
                    Ok(app_config) => app_config.language,
                    Err(_) => "en-US".to_string(),
                }
            }
            Err(_) => "en-US".to_string(),
        }
    }

    fn build_language_instruction(lang: &str) -> String {
        let json_rule = concat!(
            "\n\nCRITICAL JSON RULE: Inside JSON string values you MUST escape every literal double-quote as \\\".",
            " Do NOT place unescaped \" characters inside string values.",
            " For example, write \"he said \\\"hello\\\"\" instead of \"he said \"hello\"\".",
        );

        if lang.starts_with("en") {
            json_rule.to_string()
        } else {
            let lang_name = match lang {
                "zh-CN" => "Simplified Chinese (简体中文)",
                "zh-TW" => "Traditional Chinese (繁體中文)",
                "ja" | "ja-JP" => "Japanese (日本語)",
                "ko" | "ko-KR" => "Korean (한국어)",
                "fr" | "fr-FR" => "French (Français)",
                "de" | "de-DE" => "German (Deutsch)",
                "es" | "es-ES" => "Spanish (Español)",
                "pt" | "pt-BR" => "Portuguese (Português)",
                "ru" | "ru-RU" => "Russian (Русский)",
                _ => lang,
            };
            format!(
                "\n\nIMPORTANT: All descriptive text, summaries, suggestions, and narrative content in your response MUST be written in {}. Keep JSON keys and enum values in English.{}",
                lang_name, json_rule
            )
        }
    }

    /// Main entry: run the full insights pipeline
    pub async fn generate(days: u32) -> BitFunResult<InsightsReport> {
        let token = cancellation::register().await;
        let result = Self::generate_inner(days, &token).await;
        cancellation::unregister().await;
        result
    }

    /// Cancel the current insights generation.
    pub async fn cancel() -> Result<(), String> {
        cancellation::cancel().await
    }

    async fn generate_inner(
        days: u32,
        token: &CancellationToken,
    ) -> BitFunResult<InsightsReport> {
        let user_lang = Self::get_user_language().await;
        let lang_instruction = Self::build_language_instruction(&user_lang);
        debug!("Insights generation using language: {}", user_lang);

        // Stage 1: Data Collection
        Self::emit_progress("Collecting session data...", "data_collection", 0, 0).await;
        let (base_stats, transcripts) = InsightsCollector::collect(days).await?;

        if transcripts.is_empty() {
            return Err(BitFunError::service(
                "No sessions found in the specified time range",
            ));
        }

        info!(
            "Collected {} sessions, {} messages",
            transcripts.len(),
            base_stats.total_messages
        );

        Self::check_cancelled(token)?;

        // Stage 2: Parallel Facet Extraction (fast model)
        let ai_factory = get_global_ai_client_factory()
            .await
            .map_err(|e| BitFunError::service(format!("Failed to get AI client factory: {}", e)))?;
        let ai_client_fast = ai_factory
            .get_client_resolved("fast")
            .await
            .map_err(|e| BitFunError::service(format!("Failed to resolve fast model: {}", e)))?;

        // Primary model for analysis stages — falls back to fast if not configured
        let ai_client_primary = match ai_factory.get_client_resolved("primary").await {
            Ok(client) => client,
            Err(_) => {
                warn!("Primary model not configured, falling back to fast model for analysis");
                ai_client_fast.clone()
            }
        };

        let facets =
            Self::extract_facets_adaptive(&ai_client_fast, &transcripts, &lang_instruction, token)
                .await?;

        info!("Extracted facets for {} sessions", facets.len());

        Self::check_cancelled(token)?;

        // Stage 3: Aggregation (Rust-side, no AI)
        Self::emit_progress("Aggregating analysis...", "aggregation", 0, 0).await;
        let aggregate = InsightsCollector::aggregate(&base_stats, &facets);

        Self::check_cancelled(token)?;

        // Stage 4a: Parallel analysis (primary model) — 7 independent tasks
        Self::emit_progress("Analyzing patterns...", "analysis", 0, 0).await;

        let (suggestions, areas, wins_friction, interaction, horizon, fun_ending) =
            Self::generate_analysis_parallel(
                &ai_client_primary,
                &aggregate,
                &lang_instruction,
            )
            .await;

        Self::check_cancelled(token)?;

        // Stage 4b: Synthesis (primary model) — at_a_glance depends on 4a results
        Self::emit_progress("Writing summary...", "synthesis", 0, 0).await;

        let at_a_glance = Self::generate_synthesis(
            &ai_client_primary,
            &aggregate,
            &suggestions,
            &areas,
            &wins_friction,
            &interaction,
            &lang_instruction,
        )
        .await;

        Self::check_cancelled(token)?;

        // Stage 5: Assembly
        Self::emit_progress("Assembling report...", "assembly", 0, 0).await;
        let report = Self::assemble_report(
            base_stats,
            aggregate,
            suggestions,
            areas,
            wins_friction,
            interaction,
            at_a_glance,
            horizon,
            fun_ending,
        );

        let report = Self::save_report(report, &user_lang).await?;

        Self::emit_progress("Complete!", "complete", 0, 0).await;
        info!("Insights report generated successfully");

        Ok(report)
    }

    fn check_cancelled(token: &CancellationToken) -> BitFunResult<()> {
        if token.is_cancelled() {
            Err(BitFunError::service("Insights generation cancelled"))
        } else {
            Ok(())
        }
    }

    // ============ Stage 2: Facet Extraction ============

    async fn extract_facets_adaptive(
        ai_client: &Arc<AIClient>,
        transcripts: &[SessionTranscript],
        lang_instruction: &str,
        token: &CancellationToken,
    ) -> BitFunResult<Vec<SessionFacet>> {
        let total = transcripts.len();
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_FACET_EXTRACTIONS));
        let counter = Arc::new(AtomicUsize::new(0));
        let rate_limited = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));

        let handles: Vec<_> = transcripts
            .iter()
            .enumerate()
            .map(|(idx, t)| {
                let client = ai_client.clone();
                let sem = semaphore.clone();
                let transcript = t.clone();
                let cnt = counter.clone();
                let rl = rate_limited.clone();
                let cl = cancelled.clone();
                let lang = lang_instruction.to_string();
                let child_token = token.clone();

                tokio::spawn(async move {
                    let _permit = sem
                        .acquire()
                        .await
                        .map_err(|e| BitFunError::service(format!("Semaphore error: {}", e)))?;

                    if cl.load(Ordering::Relaxed) || child_token.is_cancelled() {
                        return Err(BitFunError::service("Insights generation cancelled"));
                    }

                    if rl.load(Ordering::Relaxed) {
                        return Err(BitFunError::service("skipped_rate_limited"));
                    }

                    let n = cnt.fetch_add(1, Ordering::Relaxed) + 1;
                    Self::emit_progress(
                        &format!("Analyzing session {}/{}...", n, total),
                        "facet_extraction",
                        n,
                        total,
                    )
                    .await;

                    let result =
                        Self::extract_single_facet(&client, &transcript, &lang).await;

                    if let Err(ref e) = result {
                        if is_rate_limit_error(e) {
                            rl.store(true, Ordering::Relaxed);
                        }
                    }

                    result.map(|facet| (idx, facet))
                })
            })
            .collect();

        let mut facets = Vec::new();
        let mut failed_indices: Vec<usize> = Vec::new();
        let mut hit_rate_limit = false;

        for (idx, handle) in handles.into_iter().enumerate() {
            if token.is_cancelled() {
                return Err(BitFunError::service("Insights generation cancelled"));
            }
            match handle.await {
                Ok(Ok((_orig_idx, facet))) => facets.push(facet),
                Ok(Err(e)) => {
                    let err_str = e.to_string();
                    if err_str.contains("cancelled") {
                        return Err(e);
                    }
                    if err_str.contains("skipped_rate_limited") || is_rate_limit_error(&e) {
                        hit_rate_limit = true;
                        failed_indices.push(idx);
                    } else {
                        warn!("Facet extraction failed for session {}: {}", idx, e);
                    }
                }
                Err(e) => warn!("Facet task panicked: {}", e),
            }
        }

        if hit_rate_limit && !failed_indices.is_empty() {
            let retry_count = failed_indices.len();
            warn!(
                "Rate limit detected, retrying {} sessions sequentially",
                retry_count
            );
            Self::emit_progress(
                &format!(
                    "Rate limited. Retrying {} sessions sequentially...",
                    retry_count
                ),
                "facet_retry",
                0,
                retry_count,
            )
            .await;

            tokio::time::sleep(Duration::from_secs(3)).await;

            for (i, idx) in failed_indices.iter().enumerate() {
                Self::check_cancelled(token)?;

                Self::emit_progress(
                    &format!("Retrying session {}/{}...", i + 1, retry_count),
                    "facet_retry",
                    i + 1,
                    retry_count,
                )
                .await;

                match Self::extract_single_facet(
                    ai_client,
                    &transcripts[*idx],
                    lang_instruction,
                )
                .await
                {
                    Ok(facet) => facets.push(facet),
                    Err(e) => warn!(
                        "Sequential retry also failed for session {}: {}",
                        idx, e
                    ),
                }

                if i + 1 < retry_count {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }

        Ok(facets)
    }

    async fn extract_single_facet(
        ai_client: &Arc<AIClient>,
        transcript: &SessionTranscript,
        lang_instruction: &str,
    ) -> BitFunResult<SessionFacet> {
        let session_info = format!(
            "Session: {}\nAgent: {}\nName: {}\nDate: {}\nDuration: {} min\n\n{}",
            transcript.session_id,
            transcript.agent_type,
            transcript.session_name,
            transcript.created_at,
            transcript.duration_minutes,
            transcript.transcript
        );

        let prompt = format!(
            "{}{}",
            FACET_PROMPT_TEMPLATE.replace("{session_transcript}", &session_info),
            lang_instruction
        );
        let messages = vec![Message::user(prompt)];

        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("AI call failed: {}", e)))?;

        let json_str = extract_json_from_response(&response.text)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to parse facet JSON: {}", e))
        })?;

        Ok(SessionFacet {
            session_id: transcript.session_id.clone(),
            underlying_goal: value["underlying_goal"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            goal_categories: parse_string_u32_map(&value["goal_categories"]),
            outcome: value["outcome"].as_str().unwrap_or("unclear_from_transcript").to_string(),
            user_satisfaction_counts: parse_string_u32_map(&value["user_satisfaction_counts"]),
            claude_helpfulness: value["claude_helpfulness"]
                .as_str()
                .unwrap_or("moderately_helpful")
                .to_string(),
            session_type: value["session_type"]
                .as_str()
                .unwrap_or("single_task")
                .to_string(),
            friction_counts: parse_string_u32_map(&value["friction_counts"]),
            friction_detail: value["friction_detail"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            primary_success: value["primary_success"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            brief_summary: value["brief_summary"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            languages_used: value["languages_used"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            user_instructions: value["user_instructions"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    // ============ Stage 4a: Parallel Analysis ============

    async fn generate_analysis_parallel(
        ai_client: &Arc<AIClient>,
        aggregate: &InsightsAggregate,
        lang_instruction: &str,
    ) -> (InsightsSuggestions, Vec<ProjectArea>, WinsFrictionResult, InteractionStyleResult, HorizonResult, Option<FunEnding>) {
        let aggregate_json =
            serde_json::to_string_pretty(aggregate).unwrap_or_else(|_| "{}".to_string());
        let summaries_text = format!("- {}", aggregate.session_summaries.join("\n- "));
        let friction_text = format!("- {}", aggregate.friction_details.join("\n- "));

        let semaphore = Arc::new(Semaphore::new(3));

        // Task 1: Suggestions
        let client_1 = ai_client.clone();
        let agg_1 = aggregate.clone();
        let lang_1 = lang_instruction.to_string();
        let sem_1 = semaphore.clone();
        let suggestions_handle = tokio::spawn(async move {
            let _permit = sem_1.acquire().await.unwrap();
            Self::generate_suggestions(&client_1, &agg_1, &lang_1).await
        });

        // Task 2: Areas
        let client_2 = ai_client.clone();
        let agg_2 = aggregate.clone();
        let lang_2 = lang_instruction.to_string();
        let sem_2 = semaphore.clone();
        let areas_handle = tokio::spawn(async move {
            let _permit = sem_2.acquire().await.unwrap();
            Self::identify_areas(&client_2, &agg_2, &lang_2).await
        });

        // Task 3a: Wins
        let client_3a = ai_client.clone();
        let agg_json_3a = aggregate_json.clone();
        let summaries_3a = summaries_text.clone();
        let lang_3a = lang_instruction.to_string();
        let sem_3a = semaphore.clone();
        let wins_handle = tokio::spawn(async move {
            let _permit = sem_3a.acquire().await.unwrap();
            Self::analyze_wins(&client_3a, &agg_json_3a, &summaries_3a, &lang_3a).await
        });

        // Task 3b: Friction
        let client_3b = ai_client.clone();
        let agg_json_3b = aggregate_json.clone();
        let summaries_3b = summaries_text.clone();
        let friction_3b = friction_text.clone();
        let lang_3b = lang_instruction.to_string();
        let sem_3b = semaphore.clone();
        let friction_handle = tokio::spawn(async move {
            let _permit = sem_3b.acquire().await.unwrap();
            Self::analyze_friction(&client_3b, &agg_json_3b, &summaries_3b, &friction_3b, &lang_3b).await
        });

        // Task 4: Interaction Style
        let client_4 = ai_client.clone();
        let agg_json_4 = aggregate_json.clone();
        let summaries_4 = summaries_text.clone();
        let lang_4 = lang_instruction.to_string();
        let sem_4 = semaphore.clone();
        let interaction_handle = tokio::spawn(async move {
            let _permit = sem_4.acquire().await.unwrap();
            Self::analyze_interaction_style(&client_4, &agg_json_4, &summaries_4, &lang_4).await
        });

        // Task 5: Horizon
        let client_5 = ai_client.clone();
        let agg_json_5 = aggregate_json.clone();
        let summaries_5 = summaries_text.clone();
        let friction_5 = friction_text.clone();
        let lang_5 = lang_instruction.to_string();
        let sem_5 = semaphore.clone();
        let horizon_handle = tokio::spawn(async move {
            let _permit = sem_5.acquire().await.unwrap();
            Self::generate_horizon(&client_5, &agg_json_5, &summaries_5, &friction_5, &lang_5).await
        });

        // Task 6: Fun Ending
        let client_6 = ai_client.clone();
        let agg_json_6 = aggregate_json.clone();
        let summaries_6 = summaries_text.clone();
        let lang_6 = lang_instruction.to_string();
        let sem_6 = semaphore.clone();
        let fun_ending_handle = tokio::spawn(async move {
            let _permit = sem_6.acquire().await.unwrap();
            Self::generate_fun_ending(&client_6, &agg_json_6, &summaries_6, &lang_6).await
        });

        // Collect results with retry on transient failures
        let suggestions = Self::resolve_with_retry(
            suggestions_handle,
            "Suggestions",
            || async { Self::generate_suggestions(ai_client, aggregate, lang_instruction).await },
            || default_suggestions(),
        ).await;

        let areas = Self::resolve_with_retry(
            areas_handle,
            "Areas",
            || async { Self::identify_areas(ai_client, aggregate, lang_instruction).await },
            Vec::new,
        ).await;

        let wins_result = Self::resolve_with_retry(
            wins_handle,
            "Wins",
            || async {
                Self::analyze_wins(
                    ai_client, &aggregate_json, &summaries_text, lang_instruction,
                ).await
            },
            WinsResult::default,
        ).await;

        let friction_result = Self::resolve_with_retry(
            friction_handle,
            "Friction",
            || async {
                Self::analyze_friction(
                    ai_client, &aggregate_json, &summaries_text, &friction_text, lang_instruction,
                ).await
            },
            FrictionResult::default,
        ).await;

        let wins_friction = WinsFrictionResult {
            wins_intro: wins_result.intro,
            big_wins: wins_result.big_wins,
            friction_intro: friction_result.intro,
            friction_categories: friction_result.friction_categories,
        };

        let interaction = Self::resolve_with_retry(
            interaction_handle,
            "Interaction Style",
            || async {
                Self::analyze_interaction_style(
                    ai_client, &aggregate_json, &summaries_text, lang_instruction,
                ).await
            },
            InteractionStyleResult::default,
        ).await;

        let horizon = Self::resolve_with_retry(
            horizon_handle,
            "Horizon",
            || async {
                Self::generate_horizon(
                    ai_client, &aggregate_json, &summaries_text, &friction_text, lang_instruction,
                ).await
            },
            HorizonResult::default,
        ).await;

        let fun_ending = Self::resolve_with_retry(
            fun_ending_handle,
            "Fun Ending",
            || async {
                Self::generate_fun_ending(
                    ai_client, &aggregate_json, &summaries_text, lang_instruction,
                ).await
            },
            || None,
        ).await;

        (suggestions, areas, wins_friction, interaction, horizon, fun_ending)
    }

    /// Generic helper to resolve a spawned task with retry on transient failures.
    ///
    /// Retries on rate-limit errors, empty AI responses, and JSON extraction failures.
    async fn resolve_with_retry<T, RetryFut, RetryFn, DefaultFn>(
        handle: tokio::task::JoinHandle<BitFunResult<T>>,
        label: &str,
        retry_fn: RetryFn,
        default_fn: DefaultFn,
    ) -> T
    where
        RetryFut: std::future::Future<Output = BitFunResult<T>>,
        RetryFn: FnOnce() -> RetryFut,
        DefaultFn: FnOnce() -> T,
    {
        let result = handle
            .await
            .map_err(|e| BitFunError::service(format!("{} task panicked: {}", label, e)));

        match result {
            Ok(Ok(val)) => val,
            Ok(Err(e)) if is_retryable_error(&e) => {
                warn!("{} failed (retryable): {}, retrying after delay", label, e);
                Self::emit_progress(
                    &format!("Retrying {}...", label.to_lowercase()),
                    "analysis_retry",
                    0,
                    0,
                )
                .await;
                tokio::time::sleep(Duration::from_secs(3)).await;
                retry_fn().await.unwrap_or_else(|e| {
                    warn!("{} retry failed: {}, using defaults", label, e);
                    default_fn()
                })
            }
            Ok(Err(e)) => {
                warn!("{} failed: {}, using defaults", label, e);
                default_fn()
            }
            Err(e) => {
                warn!("{} task error: {}, using defaults", label, e);
                default_fn()
            }
        }
    }

    // ============ Stage 4b: Synthesis ============

    async fn generate_synthesis(
        ai_client: &Arc<AIClient>,
        aggregate: &InsightsAggregate,
        suggestions: &InsightsSuggestions,
        areas: &[ProjectArea],
        wins_friction: &WinsFrictionResult,
        interaction: &InteractionStyleResult,
        lang_instruction: &str,
    ) -> AtAGlance {
        let aggregate_json =
            serde_json::to_string_pretty(aggregate).unwrap_or_else(|_| "{}".to_string());

        let areas_text = areas
            .iter()
            .map(|a| format!("- {}: {}", a.name, a.description))
            .collect::<Vec<_>>()
            .join("\n");
        let suggestions_text =
            serde_json::to_string_pretty(suggestions).unwrap_or_else(|_| "{}".to_string());
        let wins_friction_text =
            serde_json::to_string_pretty(wins_friction).unwrap_or_else(|_| "{}".to_string());
        let interaction_text =
            serde_json::to_string_pretty(interaction).unwrap_or_else(|_| "{}".to_string());

        match Self::generate_at_a_glance(
            ai_client,
            &aggregate_json,
            &areas_text,
            &suggestions_text,
            &wins_friction_text,
            &interaction_text,
            lang_instruction,
        )
        .await
        {
            Ok(val) => val,
            Err(e) => {
                warn!("At a Glance generation failed: {}, using defaults", e);
                AtAGlance::default()
            }
        }
    }

    // ============ Individual Analysis Methods ============

    async fn generate_suggestions(
        ai_client: &Arc<AIClient>,
        aggregate: &InsightsAggregate,
        lang_instruction: &str,
    ) -> BitFunResult<InsightsSuggestions> {
        let aggregate_json =
            serde_json::to_string_pretty(aggregate).unwrap_or_else(|_| "{}".to_string());
        let summaries = aggregate.session_summaries.join("\n- ");
        let friction_details = aggregate.friction_details.join("\n- ");
        let user_instructions = if aggregate.user_instructions.is_empty() {
            "None captured".to_string()
        } else {
            aggregate.user_instructions.join("\n- ")
        };

        let prompt = format!(
            "{}{}",
            SUGGESTIONS_PROMPT_TEMPLATE
                .replace("{aggregate_json}", &aggregate_json)
                .replace("{summaries}", &format!("- {}", summaries))
                .replace("{friction_details}", &format!("- {}", friction_details))
                .replace("{user_instructions}", &user_instructions),
            lang_instruction
        );

        let messages = vec![Message::user(prompt)];
        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("Suggestions AI call failed: {}", e)))?;

        info!("Suggestions response: len={}, finish={:?}", response.text.len(), response.finish_reason);
        debug!("Suggestions text: {}", safe_truncate(&response.text, 300));

        let json_str = extract_json_from_response(&response.text)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            BitFunError::Deserialization(format!(
                "Failed to parse suggestions JSON: {}. Raw: {}",
                e,
                safe_truncate(&json_str, 500)
            ))
        })?;

        debug!(
            "Suggestions parsed: md_additions={}, features={}, patterns={}",
            value["bitfun_md_additions"].as_array().map(|a| a.len()).unwrap_or(0),
            value["features_to_try"].as_array().map(|a| a.len()).unwrap_or(0),
            value["usage_patterns"].as_array().map(|a| a.len()).unwrap_or(0),
        );

        Ok(InsightsSuggestions {
            bitfun_md_additions: value["bitfun_md_additions"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            Some(MdAddition {
                                section: v["section"].as_str()?.to_string(),
                                content: v["content"].as_str()?.to_string(),
                                rationale: v["rationale"]
                                    .as_str()
                                    .or(v["why"].as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            features_to_try: value["features_to_try"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            Some(FeatureRecommendation {
                                feature: v["feature"].as_str()?.to_string(),
                                description: v["description"]
                                    .as_str()
                                    .or(v["one_liner"].as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                example_usage: v["example_usage"]
                                    .as_str()
                                    .or(v["example_code"].as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                benefit: v["benefit"]
                                    .as_str()
                                    .or(v["why_for_you"].as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            usage_patterns: value["usage_patterns"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            Some(UsagePattern {
                                pattern: v["pattern"]
                                    .as_str()
                                    .or(v["title"].as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                description: v["description"]
                                    .as_str()
                                    .or(v["suggestion"].as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                detail: v["detail"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string(),
                                suggested_prompt: v["suggested_prompt"]
                                    .as_str()
                                    .or(v["copyable_prompt"].as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    async fn identify_areas(
        ai_client: &Arc<AIClient>,
        aggregate: &InsightsAggregate,
        lang_instruction: &str,
    ) -> BitFunResult<Vec<ProjectArea>> {
        let aggregate_json =
            serde_json::to_string_pretty(aggregate).unwrap_or_else(|_| "{}".to_string());
        let summaries = aggregate.session_summaries.join("\n- ");

        let prompt = format!(
            "{}{}",
            AREAS_PROMPT_TEMPLATE
                .replace("{aggregate_json}", &aggregate_json)
                .replace("{summaries}", &format!("- {}", summaries)),
            lang_instruction
        );

        let messages = vec![Message::user(prompt)];
        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("Areas AI call failed: {}", e)))?;

        info!("Areas response: len={}, finish={:?}", response.text.len(), response.finish_reason);
        debug!("Areas text: {}", safe_truncate(&response.text, 300));

        let json_str = extract_json_from_response(&response.text)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to parse areas JSON: {}", e))
        })?;

        Ok(value["areas"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        Some(ProjectArea {
                            name: v["name"].as_str()?.to_string(),
                            session_count: v["session_count"].as_u64().unwrap_or(0) as u32,
                            description: v["description"].as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn analyze_wins(
        ai_client: &Arc<AIClient>,
        aggregate_json: &str,
        summaries: &str,
        lang_instruction: &str,
    ) -> BitFunResult<WinsResult> {
        let prompt = format!(
            "{}{}",
            WINS_PROMPT_TEMPLATE
                .replace("{aggregate_json}", aggregate_json)
                .replace("{summaries}", summaries),
            lang_instruction
        );

        let messages = vec![Message::user(prompt)];
        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("Wins AI call failed: {}", e)))?;

        info!("Wins response: len={}, finish={:?}", response.text.len(), response.finish_reason);
        debug!("Wins text: {}", safe_truncate(&response.text, 300));

        let json_str = extract_json_from_response(&response.text)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to parse wins JSON: {}", e))
        })?;

        Ok(WinsResult {
            intro: value["intro"].as_str().unwrap_or("").to_string(),
            big_wins: value["impressive_workflows"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            Some(BigWin {
                                title: v["title"].as_str()?.to_string(),
                                description: v["description"].as_str()?.to_string(),
                                impact: v["impact"].as_str().unwrap_or("").to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    async fn analyze_friction(
        ai_client: &Arc<AIClient>,
        aggregate_json: &str,
        summaries: &str,
        friction_details: &str,
        lang_instruction: &str,
    ) -> BitFunResult<FrictionResult> {
        let prompt = format!(
            "{}{}",
            FRICTION_PROMPT_TEMPLATE
                .replace("{aggregate_json}", aggregate_json)
                .replace("{summaries}", summaries)
                .replace("{friction_details}", friction_details),
            lang_instruction
        );

        let messages = vec![Message::user(prompt)];
        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("Friction AI call failed: {}", e)))?;

        info!("Friction response: len={}, finish={:?}", response.text.len(), response.finish_reason);
        debug!("Friction text: {}", safe_truncate(&response.text, 300));

        let json_str = extract_json_from_response(&response.text)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to parse friction JSON: {}", e))
        })?;

        Ok(FrictionResult {
            intro: value["intro"].as_str().unwrap_or("").to_string(),
            friction_categories: value["friction_categories"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            Some(FrictionCategory {
                                category: v["category"].as_str()?.to_string(),
                                count: v["count"].as_u64().unwrap_or(0) as u32,
                                description: v["description"].as_str()?.to_string(),
                                examples: v["examples"]
                                    .as_array()
                                    .map(|a| {
                                        a.iter()
                                            .filter_map(|e| e.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                suggestion: v["suggestion"].as_str().unwrap_or("").to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    async fn analyze_interaction_style(
        ai_client: &Arc<AIClient>,
        aggregate_json: &str,
        summaries: &str,
        lang_instruction: &str,
    ) -> BitFunResult<InteractionStyleResult> {
        let prompt = format!(
            "{}{}",
            INTERACTION_STYLE_PROMPT_TEMPLATE
                .replace("{aggregate_json}", aggregate_json)
                .replace("{summaries}", summaries),
            lang_instruction
        );

        let messages = vec![Message::user(prompt)];
        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("Interaction Style AI call failed: {}", e)))?;

        info!("Interaction Style response: len={}, finish={:?}", response.text.len(), response.finish_reason);
        debug!("Interaction Style text: {}", safe_truncate(&response.text, 300));

        let json_str = extract_json_from_response(&response.text)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to parse interaction style JSON: {}", e))
        })?;

        Ok(InteractionStyleResult {
            narrative: value["narrative"].as_str().unwrap_or("").to_string(),
            key_patterns: value["key_patterns"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    async fn generate_at_a_glance(
        ai_client: &Arc<AIClient>,
        aggregate_json: &str,
        areas_text: &str,
        suggestions_text: &str,
        wins_friction_text: &str,
        interaction_text: &str,
        lang_instruction: &str,
    ) -> BitFunResult<AtAGlance> {
        let prompt = format!(
            "{}{}",
            AT_A_GLANCE_PROMPT_TEMPLATE
                .replace("{aggregate_json}", aggregate_json)
                .replace("{areas}", areas_text)
                .replace("{suggestions}", suggestions_text)
                .replace("{wins_and_friction}", wins_friction_text)
                .replace("{interaction_style}", interaction_text),
            lang_instruction
        );

        let messages = vec![Message::user(prompt)];
        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("At a Glance AI call failed: {}", e)))?;

        info!("At a Glance response: len={}, finish={:?}", response.text.len(), response.finish_reason);
        debug!("At a Glance text: {}", safe_truncate(&response.text, 300));

        let json_str = extract_json_from_response(&response.text)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to parse at-a-glance JSON: {}", e))
        })?;

        let looking_ahead = {
            let v = json_value_to_string(&value["looking_ahead"]);
            if v.is_empty() {
                json_value_to_string(&value["ambitious_workflows"])
            } else {
                v
            }
        };

        Ok(AtAGlance {
            whats_working: json_value_to_string(&value["whats_working"]),
            whats_hindering: json_value_to_string(&value["whats_hindering"]),
            quick_wins: json_value_to_string(&value["quick_wins"]),
            looking_ahead,
        })
    }

    async fn generate_horizon(
        ai_client: &Arc<AIClient>,
        aggregate_json: &str,
        summaries: &str,
        friction_details: &str,
        lang_instruction: &str,
    ) -> BitFunResult<HorizonResult> {
        let prompt = format!(
            "{}{}",
            HORIZON_PROMPT_TEMPLATE
                .replace("{aggregate_json}", aggregate_json)
                .replace("{summaries}", summaries)
                .replace("{friction_details}", friction_details),
            lang_instruction
        );

        let messages = vec![Message::user(prompt)];
        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("Horizon AI call failed: {}", e)))?;

        info!("Horizon response: len={}, finish={:?}", response.text.len(), response.finish_reason);
        debug!("Horizon text: {}", safe_truncate(&response.text, 300));

        let json_str = extract_json_from_response(&response.text)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to parse horizon JSON: {}", e))
        })?;

        Ok(HorizonResult {
            intro: value["intro"].as_str().unwrap_or("").to_string(),
            opportunities: value["opportunities"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            Some(HorizonWorkflow {
                                title: v["title"].as_str()?.to_string(),
                                whats_possible: v["whats_possible"].as_str()?.to_string(),
                                how_to_try: v["how_to_try"].as_str().unwrap_or("").to_string(),
                                copyable_prompt: v["copyable_prompt"].as_str().unwrap_or("").to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    async fn generate_fun_ending(
        ai_client: &Arc<AIClient>,
        aggregate_json: &str,
        summaries: &str,
        lang_instruction: &str,
    ) -> BitFunResult<Option<FunEnding>> {
        let prompt = format!(
            "{}{}",
            FUN_ENDING_PROMPT_TEMPLATE
                .replace("{aggregate_json}", aggregate_json)
                .replace("{summaries}", summaries),
            lang_instruction
        );

        let messages = vec![Message::user(prompt)];
        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("Fun Ending AI call failed: {}", e)))?;

        info!("Fun Ending response: len={}, finish={:?}", response.text.len(), response.finish_reason);
        debug!("Fun Ending text: {}", safe_truncate(&response.text, 300));

        let json_str = extract_json_from_response(&response.text)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to parse fun ending JSON: {}", e))
        })?;

        Ok(Some(FunEnding {
            headline: value["headline"]
                .as_str()
                .or(value["title"].as_str())
                .unwrap_or("")
                .to_string(),
            detail: value["detail"]
                .as_str()
                .or(value["message"].as_str())
                .unwrap_or("")
                .to_string(),
        }))
    }

    // ============ Stage 5: Assembly ============

    fn assemble_report(
        _base_stats: BaseStats,
        aggregate: InsightsAggregate,
        suggestions: InsightsSuggestions,
        areas: Vec<ProjectArea>,
        wins_friction: WinsFrictionResult,
        interaction: InteractionStyleResult,
        at_a_glance: AtAGlance,
        horizon: HorizonResult,
        fun_ending: Option<FunEnding>,
    ) -> InsightsReport {
        let days_covered = if !aggregate.date_range.start.is_empty()
            && !aggregate.date_range.end.is_empty()
        {
            let parse = |s: &str| -> Option<chrono::DateTime<chrono::Utc>> {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|d| d.with_timezone(&chrono::Utc))
            };
            match (
                parse(&aggregate.date_range.start),
                parse(&aggregate.date_range.end),
            ) {
                (Some(start), Some(end)) => {
                    end.signed_duration_since(start).num_days().unsigned_abs() as u32
                }
                _ => 1,
            }
            .max(1)
        } else {
            1
        };

        InsightsReport {
            generated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            date_range: aggregate.date_range.clone(),
            total_sessions: aggregate.sessions,
            analyzed_sessions: aggregate.analyzed,
            total_messages: aggregate.messages,
            days_covered,
            stats: InsightsStats {
                total_hours: aggregate.hours,
                msgs_per_day: aggregate.msgs_per_day,
                top_tools: aggregate.top_tools.clone(),
                top_goals: aggregate.top_goals.clone(),
                outcomes: aggregate.outcomes.clone(),
                satisfaction: aggregate.satisfaction.clone(),
                session_types: aggregate.session_types.clone(),
                languages: aggregate.languages.clone(),
                hour_counts: aggregate.hour_counts.clone(),
                agent_types: aggregate.agent_types.clone(),
                response_time_buckets: aggregate.response_time_buckets.clone(),
                median_response_time_secs: aggregate.median_response_time_secs,
                avg_response_time_secs: aggregate.avg_response_time_secs,
                friction: aggregate.friction.clone(),
                success: aggregate.success.clone(),
                tool_errors: aggregate.tool_errors.clone(),
                total_lines_added: aggregate.total_lines_added,
                total_lines_removed: aggregate.total_lines_removed,
                total_files_modified: aggregate.total_files_modified,
            },
            at_a_glance,
            interaction_style: InteractionStyle {
                narrative: interaction.narrative,
                key_patterns: interaction.key_patterns,
            },
            project_areas: areas,
            wins_intro: wins_friction.wins_intro,
            big_wins: wins_friction.big_wins,
            friction_intro: wins_friction.friction_intro,
            friction_categories: wins_friction.friction_categories,
            suggestions,
            horizon_intro: horizon.intro,
            on_the_horizon: horizon.opportunities,
            fun_ending,
            html_report_path: None,
        }
    }

    // ============ Save / Load / Utility ============

    async fn save_report(mut report: InsightsReport, locale: &str) -> BitFunResult<InsightsReport> {
        let path_manager = get_path_manager_arc();
        let usage_dir = path_manager.user_data_dir().join("usage-data");
        tokio::fs::create_dir_all(&usage_dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create usage-data dir: {}", e)))?;

        let timestamp = report.generated_at;

        let html_content = generate_html(&report, locale);
        let html_path = usage_dir.join(format!("insights-{}.html", timestamp));
        tokio::fs::write(&html_path, &html_content)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write HTML report: {}", e)))?;

        report.html_report_path = Some(html_path.to_string_lossy().to_string());

        let json_path = usage_dir.join(format!("insights-{}.json", timestamp));
        let json_str = serde_json::to_string_pretty(&report)
            .map_err(|e| BitFunError::serialization(format!("Failed to serialize report: {}", e)))?;
        tokio::fs::write(&json_path, &json_str)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write report JSON: {}", e)))?;

        info!(
            "Report saved: json={}, html={}",
            json_path.display(),
            html_path.display()
        );

        Self::cleanup_old_reports(&usage_dir, 5).await;

        Ok(report)
    }

    async fn cleanup_old_reports(usage_dir: &std::path::Path, keep: usize) {
        let mut entries = match tokio::fs::read_dir(usage_dir).await {
            Ok(dir) => dir,
            Err(_) => return,
        };

        let mut json_files: Vec<std::path::PathBuf> = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("insights-") && name.ends_with(".json") {
                json_files.push(entry.path());
            }
        }

        json_files.sort();
        json_files.reverse();

        for old in json_files.into_iter().skip(keep) {
            let _ = tokio::fs::remove_file(&old).await;
            let html = old.with_extension("html");
            let _ = tokio::fs::remove_file(&html).await;
        }
    }

    pub async fn has_data(days: u32) -> BitFunResult<bool> {
        let path_manager = get_path_manager_arc();
        let pm = PersistenceManager::new(path_manager)?;
        let cutoff = SystemTime::now() - std::time::Duration::from_secs(days as u64 * 86400);

        if let Some(ws_service) = get_global_workspace_service() {
            let workspaces = ws_service.list_workspaces().await;
            for ws in workspaces {
                if !ws.root_path.join(".bitfun").join("sessions").exists() {
                    continue;
                }
                if let Ok(sessions) = pm.list_sessions(&ws.root_path).await {
                    if sessions.iter().any(|s| s.last_activity_at >= cutoff) {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    pub async fn load_report(path: &str) -> BitFunResult<InsightsReport> {
        let json_str = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read report file: {}", e)))?;
        let report: InsightsReport = serde_json::from_str(&json_str)
            .map_err(|e| BitFunError::Deserialization(format!("Failed to parse report: {}", e)))?;
        Ok(report)
    }

    pub async fn load_latest_reports() -> BitFunResult<Vec<InsightsReportMeta>> {
        let path_manager = get_path_manager_arc();
        let usage_dir = path_manager.user_data_dir().join("usage-data");

        if !usage_dir.exists() {
            return Ok(vec![]);
        }

        let mut entries = tokio::fs::read_dir(&usage_dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read usage-data dir: {}", e)))?;

        let mut json_files: Vec<std::path::PathBuf> = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("insights-") && name.ends_with(".json") {
                json_files.push(entry.path());
            }
        }

        json_files.sort();
        json_files.reverse();

        let mut reports = Vec::new();
        for json_path in json_files.iter().take(10) {
            match tokio::fs::read_to_string(json_path).await {
                Ok(json_str) => match serde_json::from_str::<InsightsReport>(&json_str) {
                    Ok(report) => {
                        let top_goals: Vec<String> = report
                            .stats
                            .top_goals
                            .iter()
                            .take(3)
                            .map(|(name, _)| name.clone())
                            .collect();
                        let mut lang_entries: Vec<_> =
                            report.stats.languages.iter().collect();
                        lang_entries
                            .sort_by(|(_, a), (_, b)| b.cmp(a));
                        let languages: Vec<String> = lang_entries
                            .iter()
                            .take(3)
                            .map(|(name, _)| name.to_string())
                            .collect();

                        reports.push(InsightsReportMeta {
                            generated_at: report.generated_at,
                            total_sessions: report.total_sessions,
                            analyzed_sessions: report.analyzed_sessions,
                            date_range: report.date_range,
                            path: json_path.to_string_lossy().to_string(),
                            total_messages: report.total_messages,
                            days_covered: report.days_covered,
                            total_hours: report.stats.total_hours,
                            top_goals,
                            languages,
                        });
                    }
                    Err(e) => {
                        warn!("Failed to parse report {}: {}", json_path.display(), e);
                    }
                },
                Err(e) => {
                    warn!("Failed to read report {}: {}", json_path.display(), e);
                }
            }
        }

        Ok(reports)
    }

    async fn emit_progress(message: &str, stage: &str, current: usize, total: usize) {
        let payload = serde_json::json!({
            "message": message,
            "stage": stage,
            "current": current,
            "total": total,
        });
        if let Err(e) = emit_global_event(BackendEvent::Custom {
            event_name: "insights-progress".to_string(),
            payload,
        })
        .await
        {
            debug!("Failed to emit progress event: {}", e);
        }
    }
}

use crate::agentic::persistence::PersistenceManager;

// ============ Intermediate result types (internal to service) ============

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WinsFrictionResult {
    #[serde(default)]
    wins_intro: String,
    big_wins: Vec<BigWin>,
    #[serde(default)]
    friction_intro: String,
    friction_categories: Vec<FrictionCategory>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WinsResult {
    intro: String,
    big_wins: Vec<BigWin>,
}

impl WinsResult {
    fn default() -> Self {
        Self {
            intro: String::new(),
            big_wins: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct FrictionResult {
    intro: String,
    friction_categories: Vec<FrictionCategory>,
}

impl FrictionResult {
    fn default() -> Self {
        Self {
            intro: String::new(),
            friction_categories: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct InteractionStyleResult {
    narrative: String,
    key_patterns: Vec<String>,
}

impl InteractionStyleResult {
    fn default() -> Self {
        Self {
            narrative: String::new(),
            key_patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct HorizonResult {
    intro: String,
    opportunities: Vec<HorizonWorkflow>,
}

impl HorizonResult {
    fn default() -> Self {
        Self {
            intro: String::new(),
            opportunities: Vec::new(),
        }
    }
}

impl AtAGlance {
    fn default() -> Self {
        Self {
            whats_working: "Analysis in progress...".to_string(),
            whats_hindering: String::new(),
            quick_wins: String::new(),
            looking_ahead: String::new(),
        }
    }
}

// ============ Helper functions ============

fn is_rate_limit_error(e: &BitFunError) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("429")
        || msg.contains("rate limit")
        || msg.contains("too many requests")
        || msg.contains("rate_limit")
}

fn is_retryable_error(e: &BitFunError) -> bool {
    if is_rate_limit_error(e) {
        return true;
    }
    let msg = e.to_string().to_lowercase();
    msg.contains("cannot extract json")
        || msg.contains("sse stream closed")
        || msg.contains("stream closed before")
        || msg.contains("connection reset")
}

fn default_suggestions() -> InsightsSuggestions {
    InsightsSuggestions {
        bitfun_md_additions: Vec::new(),
        features_to_try: Vec::new(),
        usage_patterns: Vec::new(),
    }
}

fn parse_string_u32_map(value: &Value) -> std::collections::HashMap<String, u32> {
    let mut map = std::collections::HashMap::new();
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            if let Some(n) = v.as_u64() {
                map.insert(k.clone(), n as u32);
            } else if let Some(n) = v.as_f64() {
                map.insert(k.clone(), n as u32);
            }
        }
    }
    map
}

fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn extract_json_from_response(response: &str) -> BitFunResult<String> {
    crate::util::extract_json_from_ai_response(response).ok_or_else(|| {
        BitFunError::service("Cannot extract JSON from AI response")
    })
}

/// Extract a string from a JSON value that may be a plain string or a nested object.
/// When the value is an object, concatenate all string values with spaces.
fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(map) => map
            .values()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}
