impl ChatMode {
    /// Handle provider selection result (step 1 → step 2)
    fn handle_provider_selection(&self, selection: ProviderSelection, chat_view: &mut ChatView) {
        match selection {
            ProviderSelection::Provider(template) => {
                let default_model = template.models.first().cloned().unwrap_or_default();
                chat_view.show_model_config_form_from_provider(
                    &template.name,
                    &template.base_url,
                    &template.format,
                    &default_model,
                );
            }
            ProviderSelection::Custom => {
                chat_view.show_model_config_form_custom();
            }
        }
    }

    /// Save new model to global config
    fn save_new_model(
        &self,
        result: ModelFormResult,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let model_id = format!(
            "model_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        // Parse custom headers JSON if provided
        let custom_headers: Option<std::collections::HashMap<String, String>> =
            if result.custom_headers.is_empty() {
                None
            } else {
                serde_json::from_str(&result.custom_headers).ok()
            };

        let custom_request_body: Option<String> = if result.custom_request_body.is_empty() {
            None
        } else {
            Some(result.custom_request_body.clone())
        };

        let model_config = bitfun_core::service::config::AIModelConfig {
            id: model_id.clone(),
            name: result.name.clone(),
            provider: result.provider_format.clone(),
            model_name: result.model_name.clone(),
            base_url: result.base_url.clone(),
            api_key: result.api_key.clone(),
            context_window: Some(result.context_window),
            max_tokens: Some(result.max_tokens),
            enabled: true,
            enable_thinking_process: result.enable_thinking || result.support_preserved_thinking,
            skip_ssl_verify: result.skip_ssl_verify,
            custom_headers,
            custom_headers_mode: if result.custom_headers_mode.is_empty()
                || result.custom_headers_mode == "merge"
            {
                None
            } else {
                Some(result.custom_headers_mode.clone())
            },
            custom_request_body,
            ..Default::default()
        };

        let success = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return false;
                    }
                };

                if let Err(e) = config_service.add_ai_model(model_config).await {
                    tracing::error!("Failed to add AI model: {}", e);
                    return false;
                }

                // Auto-set as primary model if no primary model exists
                match config_service
                    .get_config::<bitfun_core::service::config::GlobalConfig>(None)
                    .await
                {
                    Ok(global_config) => {
                        let has_primary = global_config
                            .ai
                            .default_models
                            .primary
                            .as_ref()
                            .map(|p| !p.is_empty())
                            .unwrap_or(false);
                        if !has_primary {
                            if let Err(e) = config_service
                                .set_config("ai.default_models.primary", &model_id)
                                .await
                            {
                                tracing::warn!("Failed to auto-set primary model: {}", e);
                            } else {
                                tracing::info!("Auto-set primary model: {}", model_id);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read config for auto-primary: {}", e);
                    }
                }

                true
            })
        });

        if success {
            chat_view.set_status(Some(format!("Model added: {}", result.name)));
            chat_state.current_model_name = format!("{} / {}", result.model_name, result.name);
            tracing::info!("Added new AI model: {} ({})", model_id, result.model_name);
            crate::account_sync::notify_local_settings_changed();
        } else {
            chat_view.set_status(Some("Failed to add model".to_string()));
        }
    }

    /// Fetch full model config and open the edit form
    fn edit_model(
        &self,
        selected: &ModelItem,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let model_id = selected.id.clone();
        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = GlobalConfigManager::get_service().await.ok()?;
                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                models.into_iter().find(|m| m.id == model_id)
            })
        });

        match result {
            Some(model) => {
                let form_data = ModelFormResult {
                    editing_model_id: Some(model.id.clone()),
                    name: model.name,
                    model_name: model.model_name,
                    base_url: model.base_url,
                    api_key: model.api_key,
                    provider_format: model.provider.clone(),
                    context_window: model.context_window.unwrap_or(128000),
                    max_tokens: model.max_tokens.unwrap_or(8192),
                    enable_thinking: model.enable_thinking_process,
                    support_preserved_thinking: model.inline_think_in_text,
                    skip_ssl_verify: model.skip_ssl_verify,
                    custom_headers: model
                        .custom_headers
                        .map(|h| serde_json::to_string(&h).unwrap_or_default())
                        .unwrap_or_default(),
                    custom_headers_mode: model
                        .custom_headers_mode
                        .unwrap_or_else(|| "merge".to_string()),
                    custom_request_body: model.custom_request_body.unwrap_or_default(),
                };
                chat_view.show_model_config_form_for_edit(&model.id, &form_data);
            }
            None => {
                chat_view.set_status(Some("Failed to load model configuration".to_string()));
            }
        }
    }

    /// Update an existing model in global config
    fn update_existing_model(
        &self,
        result: ModelFormResult,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let model_id = match &result.editing_model_id {
            Some(id) => id.clone(),
            None => return,
        };

        let custom_headers: Option<std::collections::HashMap<String, String>> =
            if result.custom_headers.is_empty() {
                None
            } else {
                serde_json::from_str(&result.custom_headers).ok()
            };

        let custom_request_body: Option<String> = if result.custom_request_body.is_empty() {
            None
        } else {
            Some(result.custom_request_body.clone())
        };

        let model_config = bitfun_core::service::config::AIModelConfig {
            id: model_id.clone(),
            name: result.name.clone(),
            provider: result.provider_format.clone(),
            model_name: result.model_name.clone(),
            base_url: result.base_url.clone(),
            api_key: result.api_key.clone(),
            context_window: Some(result.context_window),
            max_tokens: Some(result.max_tokens),
            enabled: true,
            enable_thinking_process: result.enable_thinking || result.support_preserved_thinking,
            skip_ssl_verify: result.skip_ssl_verify,
            custom_headers,
            custom_headers_mode: if result.custom_headers_mode.is_empty()
                || result.custom_headers_mode == "merge"
            {
                None
            } else {
                Some(result.custom_headers_mode.clone())
            },
            custom_request_body,
            ..Default::default()
        };

        let success = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return false;
                    }
                };

                if let Err(e) = config_service
                    .update_ai_model(&model_id, model_config)
                    .await
                {
                    tracing::error!("Failed to update AI model: {}", e);
                    return false;
                }

                true
            })
        });

        if success {
            chat_view.set_status(Some(format!("Model updated: {}", result.name)));
            chat_state.current_model_name = format!("{} / {}", result.model_name, result.name);
            tracing::info!("Updated AI model: {}", model_id);
            crate::account_sync::notify_local_settings_changed();
        } else {
            chat_view.set_status(Some("Failed to update model".to_string()));
        }
    }
}
