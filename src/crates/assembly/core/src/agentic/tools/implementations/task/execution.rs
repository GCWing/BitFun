use super::*;

impl TaskTool {
    pub(super) async fn load_configured_tool_execution_timeout() -> Option<u64> {
        let service = GlobalConfigManager::get_service().await.ok()?;
        let ai_config: AIConfig = service.get_config(Some("ai")).await.ok()?;
        ai_config
            .tool_execution_timeout_secs
            .filter(|seconds| *seconds > 0)
    }

    pub(super) fn resolve_subagent_timeout_seconds(
        requested_timeout_seconds: Option<u64>,
        configured_execution_timeout_secs: Option<u64>,
    ) -> Option<u64> {
        match (
            requested_timeout_seconds.filter(|seconds| *seconds > 0),
            configured_execution_timeout_secs.filter(|seconds| *seconds > 0),
        ) {
            (Some(requested), Some(configured)) => Some(requested.max(configured)),
            (Some(requested), None) => Some(requested),
            (None, Some(configured)) => Some(configured),
            (None, None) => None,
        }
    }

    pub(super) async fn call_task_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        self.call_task_impl_with_deep_review_mode(input, context, false)
            .await
    }

    pub(super) async fn call_deep_review_task_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        self.call_task_impl_with_deep_review_mode(input, context, true)
            .await
    }

    async fn call_task_impl_with_deep_review_mode(
        &self,
        input: &Value,
        context: &ToolUseContext,
        is_deep_review_parent: bool,
    ) -> BitFunResult<Vec<ToolResult>> {
        let start_time = std::time::Instant::now();
        let invocation = Self::parse_invocation(input, is_deep_review_parent)?;

        let session_id = context
            .session_id
            .clone()
            .ok_or_else(|| BitFunError::tool("session_id is required in context".to_string()))?;

        if invocation.action == TaskAction::Cancel {
            return Self::cancel_background_runs(&session_id, invocation).await;
        }

        self.run_subagent_invocation(input, context, invocation, start_time, session_id)
            .await
    }

    async fn cancel_background_runs(
        parent_session_id: &str,
        invocation: TaskInvocation,
    ) -> BitFunResult<Vec<ToolResult>> {
        let target_session_id = invocation.target_session_id.as_deref().ok_or_else(|| {
            BitFunError::tool("session_id is required when action is cancel".to_string())
        })?;
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let cancelled_count = coordinator
            .cancel_background_subagents_for_parent(parent_session_id, target_session_id)
            .await?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "action": "cancel",
                "status": "cancelled",
                "session_id": target_session_id,
                "cancelled_background_tasks": cancelled_count,
            }),
            result_for_assistant: Some(format!(
                "Cancelled {} background Task run(s) for subagent session {}.\n<background_task status=\"cancelled\" session_id=\"{}\" cancelled_count=\"{}\">Cancelled background runs will not deliver results back to you.</background_task>",
                cancelled_count, target_session_id, target_session_id, cancelled_count
            )),
            image_attachments: None,
        }])
    }

    async fn run_subagent_invocation(
        &self,
        input: &Value,
        context: &ToolUseContext,
        invocation: TaskInvocation,
        start_time: Instant,
        session_id: String,
    ) -> BitFunResult<Vec<ToolResult>> {
        Self::ensure_delegation_allowed(context)?;

        let description = invocation.description.clone();
        let mut prompt = invocation.prompt.clone().ok_or_else(|| {
            BitFunError::tool(
                "Required parameters: prompt and description. Missing prompt".to_string(),
            )
        })?;
        let context_mode = invocation.context_mode;
        let target_session_id = invocation.target_session_id.clone();
        let model_id = invocation.model_id.clone();
        let mut timeout_seconds = invocation.timeout_seconds;
        let run_in_background = invocation.run_in_background;
        let is_retry = invocation.is_retry;
        let requested_auto_retry = invocation.requested_auto_retry;
        let is_auto_retry = is_retry && requested_auto_retry;
        let is_deep_review_parent = Self::is_deep_review_context(Some(context));

        let subagent_type = match context_mode {
            SubagentContextMode::Fresh => {
                if target_session_id.is_some() {
                    None
                } else {
                    let subagent_type = invocation.subagent_type.clone().ok_or_else(|| {
                        BitFunError::tool(
                            "subagent_type is required when fork_context is false or omitted and session_id is not provided"
                                .to_string(),
                        )
                    })?;
                    let all_agent_types = self.get_agents_types(Some(context)).await;
                    if !all_agent_types.contains(&subagent_type) {
                        return Err(BitFunError::tool(format!(
                            "subagent_type {} is not valid, must be one of: {}",
                            subagent_type,
                            all_agent_types.join(", ")
                        )));
                    }
                    Some(subagent_type)
                }
            }
            SubagentContextMode::Fork => None,
        };
        let delegate_target_label = match subagent_type.as_deref() {
            Some(subagent_type) => format!("subagent '{}'", subagent_type),
            None if target_session_id.is_some() => "existing subagent session".to_string(),
            None => "forked subagent".to_string(),
        };

        let current_workspace_path = context
            .workspace_root()
            .map(|path| path.to_string_lossy().into_owned());
        let effective_workspace_path = if subagent_type.is_some() {
            Some(current_workspace_path.clone().ok_or_else(|| {
                BitFunError::tool(
                    "current workspace is required when creating a fresh subagent session"
                        .to_string(),
                )
            })?)
        } else {
            None
        };

        let tool_call_id = context
            .tool_call_id
            .clone()
            .ok_or_else(|| BitFunError::tool("tool_call_id is required in context".to_string()))?;
        let dialog_turn_id = context.dialog_turn_id.clone().ok_or_else(|| {
            BitFunError::tool("dialog_turn_id is required in context".to_string())
        })?;
        let mut deep_review_effective_policy: Option<DeepReviewExecutionPolicy> = None;
        let mut deep_review_active_guard: Option<DeepReviewActiveReviewerGuard<'static>> = None;
        let mut deep_review_reviewer_configured_max_parallel_instances: Option<usize> = None;
        let mut deep_review_concurrency_policy: Option<DeepReviewConcurrencyPolicy> = None;
        let mut deep_review_is_optional_reviewer = false;
        let mut deep_review_launch_batch_info: Option<DeepReviewLaunchBatchInfo> = None;
        let mut deep_review_retry_scope_files: Option<Vec<String>> = None;
        let mut deep_review_subagent_role: Option<DeepReviewSubagentRole> = None;
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;

        if is_deep_review_parent {
            let subagent_type = subagent_type.as_deref().ok_or_else(|| {
                BitFunError::tool("subagent_type is required for DeepReview Task calls".to_string())
            })?;
            let base_policy = load_default_deep_review_policy().await.map_err(|error| {
                BitFunError::tool(format!(
                    "Failed to load DeepReview execution policy: {}",
                    error
                ))
            })?;
            let mut run_manifest = context.custom_data.get("deep_review_run_manifest").cloned();
            if let Some(workspace) = context.workspace.as_ref() {
                let session_storage_dir = workspace.session_storage_dir();
                match coordinator
                    .get_session_manager()
                    .load_session_metadata(&session_storage_dir, &session_id)
                    .await
                {
                    Ok(Some(metadata)) => {
                        if run_manifest.is_none() {
                            run_manifest = metadata.deep_review_run_manifest;
                        }
                        if let Some(run_manifest) = run_manifest.as_mut() {
                            LaunchReviewAgentTool::attach_deep_review_cache(
                                run_manifest,
                                metadata.deep_review_cache,
                            );
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        warn!(
                            "Failed to load DeepReview session metadata for run-manifest policy: session_id={}, error={}",
                            session_id, error
                        );
                    }
                }
            }
            let policy = if let Some(manifest) = run_manifest.as_ref() {
                base_policy.with_run_manifest_execution_policy(manifest)
            } else {
                base_policy
            };
            deep_review_effective_policy = Some(policy.clone());
            let role = policy
                .classify_subagent(subagent_type)
                .map_err(|violation| {
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            deep_review_subagent_role = Some(role);
            if requested_auto_retry && !is_retry {
                return Err(BitFunError::tool(
                    "auto_retry requires retry=true for DeepReview Task calls".to_string(),
                ));
            }
            if let Some(gate) = run_manifest
                .as_ref()
                .and_then(DeepReviewRunManifestGate::from_value)
            {
                gate.ensure_active(subagent_type).map_err(|violation| {
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            }
            let conc_policy = policy
                .concurrency_policy_from_manifest(run_manifest.as_ref().unwrap_or(&Value::Null));
            deep_review_concurrency_policy = Some(conc_policy.clone());
            if is_retry && role == DeepReviewSubagentRole::Reviewer {
                deep_review_retry_scope_files = Some(
                    match LaunchReviewAgentTool::ensure_deep_review_retry_coverage(
                        input,
                        subagent_type,
                        run_manifest.as_ref(),
                    ) {
                        Ok(retry_scope_files) => retry_scope_files,
                        Err(violation) => {
                            if is_auto_retry {
                                record_deep_review_runtime_auto_retry_suppressed(
                                    &dialog_turn_id,
                                    LaunchReviewAgentTool::auto_retry_suppression_reason(
                                        violation.code,
                                    ),
                                );
                            }
                            return Err(BitFunError::tool(format!(
                                "DeepReview Task policy violation: {}",
                                violation.to_tool_error_message()
                            )));
                        }
                    },
                );
                if is_auto_retry {
                    LaunchReviewAgentTool::ensure_deep_review_auto_retry_allowed(
                        &conc_policy,
                        &dialog_turn_id,
                    )
                    .map_err(|violation| {
                        record_deep_review_runtime_auto_retry_suppressed(
                            &dialog_turn_id,
                            LaunchReviewAgentTool::auto_retry_suppression_reason(violation.code),
                        );
                        BitFunError::tool(format!(
                            "DeepReview Task policy violation: {}",
                            violation.to_tool_error_message()
                        ))
                    })?;
                }
            }
            let is_readonly = get_agent_registry()
                .get_subagent_is_readonly(subagent_type)
                .unwrap_or(false);
            if !is_readonly {
                return Err(BitFunError::tool(format!(
                    "DeepReview Task policy violation: {}",
                    json!({
                        "code": "deep_review_subagent_not_readonly",
                        "message": format!(
                            "DeepReview review-phase subagent '{}' must be read-only",
                            subagent_type
                        )
                    })
                )));
            }
            let is_review = get_agent_registry()
                .get_subagent_is_review(subagent_type)
                .unwrap_or(false);
            if !is_review {
                return Err(BitFunError::tool(format!(
                    "DeepReview Task policy violation: {}",
                    json!({
                        "code": "deep_review_subagent_not_review",
                        "message": format!(
                            "DeepReview review-phase subagent '{}' must be marked for review",
                            subagent_type
                        )
                    })
                )));
            }
            timeout_seconds = policy.effective_timeout_seconds(role, timeout_seconds);

            if role == DeepReviewSubagentRole::Reviewer && !is_retry {
                if let Some(cache_hit) =
                    deep_review_task_adapter::deep_review_incremental_cache_hit_for_task(
                        subagent_type,
                        description.as_deref(),
                        run_manifest.as_ref(),
                    )
                {
                    let (data, cached_result) =
                        deep_review_task_adapter::deep_review_incremental_cache_hit_result(
                            subagent_type,
                            &cache_hit,
                        );
                    return Ok(vec![ToolResult::ok(data, Some(cached_result))]);
                }
            }

            match role {
                DeepReviewSubagentRole::Reviewer => {
                    deep_review_reviewer_configured_max_parallel_instances =
                        Some(conc_policy.max_parallel_instances);
                    let effective_parallel_instances = deep_review_effective_parallel_instances(
                        &dialog_turn_id,
                        conc_policy.max_parallel_instances,
                    );
                    let is_optional_reviewer = policy
                        .extra_subagent_ids
                        .iter()
                        .any(|id| id == subagent_type);
                    deep_review_is_optional_reviewer = is_optional_reviewer;
                    deep_review_launch_batch_info =
                        LaunchReviewAgentTool::deep_review_launch_batch_for_task(
                            subagent_type,
                            description.as_deref(),
                            run_manifest.as_ref(),
                        );
                    match LaunchReviewAgentTool::try_begin_deep_review_reviewer_admission(
                        &dialog_turn_id,
                        effective_parallel_instances,
                        deep_review_launch_batch_info.as_ref(),
                    ) {
                        Ok(Some(guard)) => {
                            deep_review_active_guard = Some(guard);
                        }
                        Ok(None)
                        | Err(DeepReviewPolicyViolation {
                            code: "deep_review_launch_batch_blocked",
                            ..
                        }) => match LaunchReviewAgentTool::wait_for_deep_review_reviewer_admission(
                            &session_id,
                            &dialog_turn_id,
                            &tool_call_id,
                            subagent_type,
                            &conc_policy,
                            is_optional_reviewer,
                            deep_review_launch_batch_info.as_ref(),
                        )
                        .await?
                        {
                            DeepReviewQueueWaitOutcome::Ready { guard } => {
                                deep_review_active_guard = Some(guard);
                            }
                            DeepReviewQueueWaitOutcome::Skipped {
                                queue_elapsed_ms,
                                skip_reason,
                                capacity_reason,
                            } => {
                                return Ok(vec![
                                        LaunchReviewAgentTool::deep_review_local_capacity_skip_tool_result(
                                            &dialog_turn_id,
                                            subagent_type,
                                            &conc_policy,
                                            capacity_reason,
                                            skip_reason,
                                            queue_elapsed_ms,
                                            start_time.elapsed().as_millis(),
                                        ),
                                    ]);
                            }
                        },
                        Err(violation) => {
                            return Err(BitFunError::tool(format!(
                                "DeepReview Task policy violation: {}",
                                violation.to_tool_error_message()
                            )));
                        }
                    }
                }
                DeepReviewSubagentRole::Judge => {
                    let active_reviewers = deep_review_active_reviewer_count(&dialog_turn_id);
                    let judge_pending = deep_review_has_judge_been_launched(&dialog_turn_id);
                    conc_policy
                        .check_launch_allowed(active_reviewers, role, judge_pending)
                        .map_err(|violation| {
                            BitFunError::tool(format!(
                                "DeepReview concurrency policy violation: {}",
                                violation.to_tool_error_message()
                            ))
                        })?;
                }
            }
            record_deep_review_task_budget(&dialog_turn_id, &policy, role, subagent_type, is_retry)
                .map_err(|violation| {
                    if is_auto_retry {
                        record_deep_review_runtime_auto_retry_suppressed(
                            &dialog_turn_id,
                            LaunchReviewAgentTool::auto_retry_suppression_reason(violation.code),
                        );
                    }
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            if is_retry && role == DeepReviewSubagentRole::Reviewer {
                if is_auto_retry {
                    record_deep_review_runtime_auto_retry(&dialog_turn_id);
                } else {
                    record_deep_review_runtime_manual_retry(&dialog_turn_id);
                }
            }
        }

        if deep_review_subagent_role.is_none() {
            let configured_timeout = Self::load_configured_tool_execution_timeout().await;
            timeout_seconds =
                Self::resolve_subagent_timeout_seconds(timeout_seconds, configured_timeout);
        }

        if let Some(retry_scope_files) = deep_review_retry_scope_files.as_ref() {
            prompt = LaunchReviewAgentTool::prompt_with_deep_review_retry_scope(
                &prompt,
                retry_scope_files,
            );
        }

        let subagent_context = deep_review_subagent_role.map(|role| {
            let mut values = HashMap::new();
            values.insert(
                "deep_review_subagent_role".to_string(),
                match role {
                    DeepReviewSubagentRole::Reviewer => "reviewer",
                    DeepReviewSubagentRole::Judge => "judge",
                }
                .to_string(),
            );
            if let Some(subagent_type) = subagent_type.as_ref() {
                values.insert(
                    "deep_review_subagent_type".to_string(),
                    subagent_type.clone(),
                );
            }
            values
        });
        let prepared_prompt = prompt;
        if run_in_background {
            return Self::start_background_task(
                &coordinator,
                context,
                context_mode,
                target_session_id,
                subagent_type,
                effective_workspace_path,
                model_id,
                subagent_context,
                prepared_prompt,
                timeout_seconds,
                tool_call_id,
                session_id,
                dialog_turn_id,
            )
            .await;
        }

        Self::run_foreground_task(
            &coordinator,
            context,
            context_mode,
            target_session_id,
            subagent_type,
            effective_workspace_path,
            model_id,
            subagent_context,
            prepared_prompt,
            timeout_seconds,
            tool_call_id,
            session_id,
            dialog_turn_id,
            delegate_target_label,
            deep_review_subagent_role,
            deep_review_active_guard,
            deep_review_reviewer_configured_max_parallel_instances,
            deep_review_concurrency_policy,
            deep_review_is_optional_reviewer,
            deep_review_launch_batch_info,
            deep_review_effective_policy,
            is_retry,
            start_time,
        )
        .await
    }

    async fn start_background_task(
        coordinator: &std::sync::Arc<crate::agentic::coordination::ConversationCoordinator>,
        context: &ToolUseContext,
        context_mode: SubagentContextMode,
        target_session_id: Option<String>,
        subagent_type: Option<String>,
        effective_workspace_path: Option<String>,
        model_id: Option<String>,
        subagent_context: Option<HashMap<String, String>>,
        prepared_prompt: String,
        timeout_seconds: Option<u64>,
        tool_call_id: String,
        session_id: String,
        dialog_turn_id: String,
    ) -> BitFunResult<Vec<ToolResult>> {
        let parent_info = SubagentParentInfo {
            tool_call_id,
            session_id,
            dialog_turn_id,
        };
        let background_result = coordinator
            .start_background_subagent(
                SubagentExecutionRequest {
                    task_description: prepared_prompt,
                    context_mode,
                    target_session_id,
                    subagent_type,
                    workspace_path: effective_workspace_path,
                    model_id,
                    subagent_parent_info: parent_info,
                    context: subagent_context.unwrap_or_default(),
                    delegation_policy: context.delegation_policy().spawn_child(),
                },
                timeout_seconds,
            )
            .await?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "context_mode": context_mode.as_str(),
                "status": "started",
                "run_in_background": true,
                "background_task_id": background_result.background_task_id.clone(),
                "session_id": background_result.session_id.clone(),
            }),
            result_for_assistant: Some(Self::background_subagent_started_assistant_message(
                &background_result.session_id,
            )),
            image_attachments: None,
        }])
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_foreground_task(
        coordinator: &std::sync::Arc<crate::agentic::coordination::ConversationCoordinator>,
        context: &ToolUseContext,
        context_mode: SubagentContextMode,
        target_session_id: Option<String>,
        subagent_type: Option<String>,
        effective_workspace_path: Option<String>,
        model_id: Option<String>,
        subagent_context: Option<HashMap<String, String>>,
        prepared_prompt: String,
        timeout_seconds: Option<u64>,
        tool_call_id: String,
        session_id: String,
        dialog_turn_id: String,
        delegate_target_label: String,
        deep_review_subagent_role: Option<DeepReviewSubagentRole>,
        deep_review_active_guard: Option<DeepReviewActiveReviewerGuard<'static>>,
        deep_review_reviewer_configured_max_parallel_instances: Option<usize>,
        deep_review_concurrency_policy: Option<DeepReviewConcurrencyPolicy>,
        deep_review_is_optional_reviewer: bool,
        deep_review_launch_batch_info: Option<DeepReviewLaunchBatchInfo>,
        deep_review_effective_policy: Option<DeepReviewExecutionPolicy>,
        is_retry: bool,
        start_time: Instant,
    ) -> BitFunResult<Vec<ToolResult>> {
        let mut deep_review_active_guard = deep_review_active_guard;
        let mut provider_capacity_retry =
            deep_review_task_adapter::DeepReviewProviderCapacityRetryRuntime::default();
        let deep_review_subagent_id = subagent_type.as_deref().unwrap_or("");
        let result = loop {
            let parent_info = SubagentParentInfo {
                tool_call_id: tool_call_id.clone(),
                session_id: session_id.clone(),
                dialog_turn_id: dialog_turn_id.clone(),
            };
            let subagent_execution_started_at = Instant::now();
            debug!(
                "TaskTool awaiting subagent result: parent_session_id={}, dialog_turn_id={}, tool_call_id={}, context_mode={}, delegate_target={}, timeout_seconds={:?}, workspace_path={:?}, model_id={:?}",
                session_id,
                dialog_turn_id,
                tool_call_id,
                context_mode.as_str(),
                delegate_target_label,
                timeout_seconds,
                effective_workspace_path,
                model_id
            );
            let execution_result = coordinator
                .execute_subagent(
                    SubagentExecutionRequest {
                        task_description: prepared_prompt.clone(),
                        context_mode,
                        target_session_id: target_session_id.clone(),
                        subagent_type: subagent_type.clone(),
                        workspace_path: effective_workspace_path.clone(),
                        model_id: model_id.clone(),
                        subagent_parent_info: parent_info,
                        context: subagent_context.clone().unwrap_or_default(),
                        delegation_policy: context.delegation_policy().spawn_child(),
                    },
                    context.cancellation_token(),
                    timeout_seconds,
                )
                .await;

            match execution_result {
                Ok(result) => {
                    debug!(
                        "TaskTool subagent returned: parent_session_id={}, dialog_turn_id={}, tool_call_id={}, context_mode={}, delegate_target={}, status={:?}, text_len={}, duration_ms={}, ledger_event_id={:?}",
                        session_id,
                        dialog_turn_id,
                        tool_call_id,
                        context_mode.as_str(),
                        delegate_target_label,
                        result.status,
                        result.text.len(),
                        elapsed_ms_u64(subagent_execution_started_at),
                        result.ledger_event_id()
                    );
                    if let Some(reason) = provider_capacity_retry.last_retry_reason() {
                        LaunchReviewAgentTool::record_deep_review_provider_capacity_retry_success(
                            &dialog_turn_id,
                            reason,
                        );
                    }
                    break result;
                }
                Err(error) => {
                    warn!(
                        "TaskTool subagent failed: parent_session_id={}, dialog_turn_id={}, tool_call_id={}, context_mode={}, delegate_target={}, duration_ms={}, error={}",
                        session_id,
                        dialog_turn_id,
                        tool_call_id,
                        context_mode.as_str(),
                        delegate_target_label,
                        elapsed_ms_u64(subagent_execution_started_at),
                        error
                    );
                    if matches!(
                        deep_review_subagent_role,
                        Some(DeepReviewSubagentRole::Reviewer)
                    ) && matches!(error, BitFunError::Cancelled(_))
                        && !context
                            .cancellation_token()
                            .as_ref()
                            .is_some_and(|token| token.is_cancelled())
                    {
                        let reason = match &error {
                            BitFunError::Cancelled(reason) => reason.as_str(),
                            _ => "",
                        };
                        return Ok(vec![
                            LaunchReviewAgentTool::deep_review_cancelled_reviewer_tool_result(
                                deep_review_subagent_id,
                                reason,
                                start_time.elapsed().as_millis(),
                            ),
                        ]);
                    }
                    if matches!(
                        deep_review_subagent_role,
                        Some(DeepReviewSubagentRole::Reviewer)
                    ) {
                        if let Some(conc_policy) = deep_review_concurrency_policy.as_ref() {
                            let decision =
                                LaunchReviewAgentTool::deep_review_capacity_decision_for_provider_error(&error);
                            match provider_capacity_retry.decide_after_error(&decision, conc_policy)
                            {
                                deep_review_task_adapter::DeepReviewProviderCapacityRetryDecision::NotQueueable => {}
                                deep_review_task_adapter::DeepReviewProviderCapacityRetryDecision::CapacitySkipped {
                                    reason,
                                    queue_elapsed_ms,
                                } => {
                                    drop(deep_review_active_guard.take());
                                    let (data, assistant_message) = LaunchReviewAgentTool::deep_review_capacity_skip_result_for_provider_queue_outcome(
                                        reason,
                                        &dialog_turn_id,
                                        deep_review_subagent_id,
                                        conc_policy,
                                        start_time.elapsed().as_millis(),
                                        queue_elapsed_ms,
                                        None,
                                    );
                                    let effective_parallel_instances = data
                                        .get("effective_parallel_instances")
                                        .and_then(Value::as_u64)
                                        .and_then(|value| usize::try_from(value).ok());
                                    LaunchReviewAgentTool::emit_deep_review_queue_state(
                                        &session_id,
                                        &dialog_turn_id,
                                        &tool_call_id,
                                        deep_review_subagent_id,
                                        DeepReviewQueueStatus::CapacitySkipped,
                                        Some(reason),
                                        0,
                                        deep_review_active_reviewer_count(&dialog_turn_id),
                                        deep_review_is_optional_reviewer.then_some(1),
                                        effective_parallel_instances,
                                        queue_elapsed_ms,
                                        conc_policy.max_queue_wait_seconds,
                                    )
                                    .await;
                                    return Ok(vec![ToolResult::Result {
                                        data,
                                        result_for_assistant: Some(assistant_message),
                                        image_attachments: None,
                                    }]);
                                }
                                deep_review_task_adapter::DeepReviewProviderCapacityRetryDecision::WaitForCapacity {
                                    reason,
                                    max_wait_seconds,
                                } => {
                                    drop(deep_review_active_guard.take());
                                    match LaunchReviewAgentTool::wait_for_deep_review_provider_capacity_retry(
                                        &session_id,
                                        &dialog_turn_id,
                                        &tool_call_id,
                                        deep_review_subagent_id,
                                        conc_policy,
                                        reason,
                                        max_wait_seconds,
                                        deep_review_is_optional_reviewer,
                                    )
                                    .await
                                    {
                                        DeepReviewProviderQueueWaitOutcome::ReadyToRetry {
                                            queue_elapsed_ms,
                                            early_capacity_probe,
                                        } => {
                                            provider_capacity_retry.record_ready_to_retry(
                                                reason,
                                                queue_elapsed_ms,
                                                early_capacity_probe,
                                            );
                                            let effective_parallel_instances =
                                                deep_review_effective_parallel_instances(
                                                    &dialog_turn_id,
                                                    conc_policy.max_parallel_instances,
                                                );
                                            match LaunchReviewAgentTool::try_begin_deep_review_reviewer_admission(
                                                &dialog_turn_id,
                                                effective_parallel_instances,
                                                deep_review_launch_batch_info.as_ref(),
                                            ) {
                                                Ok(Some(guard)) => {
                                                    deep_review_active_guard = Some(guard);
                                                }
                                                Ok(None)
                                                | Err(DeepReviewPolicyViolation {
                                                    code: "deep_review_launch_batch_blocked",
                                                    ..
                                                }) => {
                                                    match LaunchReviewAgentTool::wait_for_deep_review_reviewer_admission(
                                                        &session_id,
                                                        &dialog_turn_id,
                                                        &tool_call_id,
                                                        deep_review_subagent_id,
                                                        conc_policy,
                                                        deep_review_is_optional_reviewer,
                                                        deep_review_launch_batch_info.as_ref(),
                                                    )
                                                    .await?
                                                    {
                                                        DeepReviewQueueWaitOutcome::Ready { guard } => {
                                                            deep_review_active_guard = Some(guard);
                                                        }
                                                        DeepReviewQueueWaitOutcome::Skipped {
                                                            queue_elapsed_ms,
                                                            skip_reason,
                                                            capacity_reason,
                                                        } => {
                                                            return Ok(vec![
                                                                LaunchReviewAgentTool::deep_review_local_capacity_skip_tool_result(
                                                                    &dialog_turn_id,
                                                                    deep_review_subagent_id,
                                                                    conc_policy,
                                                                    capacity_reason,
                                                                    skip_reason,
                                                                    queue_elapsed_ms,
                                                                    start_time.elapsed().as_millis(),
                                                                ),
                                                            ]);
                                                        }
                                                    }
                                                }
                                                Err(violation) => {
                                                    return Err(BitFunError::tool(format!(
                                                        "DeepReview Task policy violation: {}",
                                                        violation.to_tool_error_message()
                                                    )));
                                                }
                                            }
                                            LaunchReviewAgentTool::record_deep_review_provider_capacity_retry(
                                                &dialog_turn_id,
                                                reason,
                                            );
                                            continue;
                                        }
                                        DeepReviewProviderQueueWaitOutcome::Skipped {
                                            queue_elapsed_ms,
                                            skip_reason,
                                        } => {
                                            let total_provider_capacity_queue_elapsed_ms =
                                                provider_capacity_retry
                                                    .record_queue_skipped(queue_elapsed_ms);
                                            let (data, assistant_message) = LaunchReviewAgentTool::deep_review_capacity_skip_result_for_provider_queue_outcome(
                                                reason,
                                                &dialog_turn_id,
                                                deep_review_subagent_id,
                                                conc_policy,
                                                start_time.elapsed().as_millis(),
                                                total_provider_capacity_queue_elapsed_ms,
                                                Some(skip_reason),
                                            );
                                            return Ok(vec![ToolResult::Result {
                                                data,
                                                result_for_assistant: Some(assistant_message),
                                                image_attachments: None,
                                            }]);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return Err(error);
                }
            }
        };
        if !result.is_partial_timeout() {
            if let Some(configured_max_parallel_instances) =
                deep_review_reviewer_configured_max_parallel_instances
            {
                record_deep_review_effective_concurrency_success(
                    &dialog_turn_id,
                    configured_max_parallel_instances,
                );
            }
        }
        drop(deep_review_active_guard);

        let duration = start_time.elapsed().as_millis();
        let retry_hint = if LaunchReviewAgentTool::should_emit_deep_review_retry_guidance(
            result.is_partial_timeout(),
            is_retry,
            deep_review_subagent_role,
        ) {
            let retries_used = crate::agentic::deep_review_policy::deep_review_retries_used(
                &dialog_turn_id,
                deep_review_subagent_id,
            );
            let max_retries = LaunchReviewAgentTool::deep_review_retry_guidance_max_retries(
                deep_review_effective_policy.as_ref(),
                &dialog_turn_id,
            );
            deep_review_task_adapter::deep_review_retry_guidance(retries_used, max_retries)
        } else {
            String::new()
        };

        let (mut data, mut result_for_assistant) =
            deep_review_task_adapter::deep_review_task_completion_result(
                &delegate_target_label,
                &result.text,
                context_mode.as_str(),
                duration,
                result.is_partial_timeout(),
                result.reason.as_deref(),
                result.ledger_event_id(),
                &retry_hint,
            );
        if let Some(subagent_session_id) = result.session_id() {
            data["session_id"] = json!(subagent_session_id);
            result_for_assistant.push_str(&format!(
                "\n<subagent_session id=\"{}\">Use this session_id to continue the same subagent session.</subagent_session>",
                subagent_session_id
            ));
        }

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}
