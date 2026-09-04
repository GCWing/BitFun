impl ChatMode {
    /// Handle provider selection result (step 1 to step 2).
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
            ProviderSelection::Custom => chat_view.show_model_config_form_custom(),
        }
    }

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
        let mutation = result.to_mutation(model_id.clone());
        let make_primary_if_empty = true;
        let outcome = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                if self.agent.is_remote_workspace() {
                    anyhow::bail!("Model management is unavailable for a Remote workspace")
                }
                let config_owner =
                    openbitfun_core::service::config::get_global_config_service().await?;
                let model = crate::model_selection::model_from_mutation(mutation, None)?;
                let added_model_id = model.id.clone();
                config_owner
                    .add_ai_model(model)
                    .await
                    .map_err(|error| anyhow!(error.to_string()))?;
                if make_primary_if_empty {
                    let config: openbitfun_core::service::config::GlobalConfig = config_owner
                        .get_config(None)
                        .await
                        .map_err(|error| anyhow!(error.to_string()))?;
                    if openbitfun_core::service::config::model_projection::selector_is_unset(
                        &config.ai.default_models.primary,
                    ) {
                        config_owner
                            .set_config("ai.default_models.primary", &Some(added_model_id))
                            .await
                            .map_err(|error| anyhow!(error.to_string()))?;
                    }
                }
                Ok::<(), anyhow::Error>(())
            })
        });

        match outcome {
            Ok(_) => {
                chat_view.set_status(Some(format!("Model added: {}", result.name)));
                chat_state.current_model_name = format!("{} / {}", result.model_name, result.name);
                tracing::info!("Added new AI model: {} ({})", model_id, result.model_name);
                if let Some(account) = &self.account_runtime {
                    account.notify_local_settings_changed();
                }
            }
            Err(error) => {
                tracing::error!("Failed to add AI model: {error}");
                chat_view.set_status(Some(format!("Failed to add model: {error}")));
            }
        }
    }

    /// The read projection contains only editable non-secret fields. Existing
    /// secrets stay write-only and are preserved when the edit form is blank.
    fn edit_model(
        &self,
        selected: &ModelItem,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let model_id = selected.id.clone();
        let outcome = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                if self.agent.is_remote_workspace() {
                    anyhow::bail!("Model management is unavailable for a Remote workspace")
                }
                let config_owner =
                    openbitfun_core::service::config::get_global_config_service().await?;
                let model = config_owner
                    .get_ai_models()
                    .await
                    .map_err(|error| anyhow!(error.to_string()))?
                    .into_iter()
                    .find(|model| model.id == model_id)
                    .ok_or_else(|| anyhow!("AI model '{model_id}' was not found"))?;
                Ok::<_, anyhow::Error>(crate::model_selection::model_edit_projection(&model))
            })
        });

        match outcome {
            Ok(projection) => {
                let form_data = ModelFormResult::from_projection(projection);
                chat_view.show_model_config_form_for_edit(&model_id, &form_data);
            }
            Err(error) => {
                tracing::error!("Failed to load model configuration: {error}");
                chat_view.set_status(Some(format!("Failed to load model configuration: {error}")));
            }
        }
    }

    fn update_existing_model(
        &self,
        result: ModelFormResult,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let Some(model_id) = result.editing_model_id.clone() else {
            return;
        };
        let mutation = result.to_mutation(model_id.clone());
        let outcome = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                if self.agent.is_remote_workspace() {
                    anyhow::bail!("Model management is unavailable for a Remote workspace")
                }
                if mutation.id != model_id {
                    anyhow::bail!("Model update identity does not match the request target")
                }
                let config_owner =
                    openbitfun_core::service::config::get_global_config_service().await?;
                let existing = config_owner
                    .get_ai_models()
                    .await
                    .map_err(|error| anyhow!(error.to_string()))?
                    .into_iter()
                    .find(|model| model.id == model_id)
                    .ok_or_else(|| anyhow!("AI model '{model_id}' was not found"))?;
                let model = crate::model_selection::model_from_mutation(mutation, Some(existing))?;
                config_owner
                    .update_ai_model(&model_id, model)
                    .await
                    .map_err(|error| anyhow!(error.to_string()))?;
                Ok::<(), anyhow::Error>(())
            })
        });

        match outcome {
            Ok(_) => {
                chat_view.set_status(Some(format!("Model updated: {}", result.name)));
                chat_state.current_model_name = format!("{} / {}", result.model_name, result.name);
                tracing::info!("Updated AI model: {model_id}");
                if let Some(account) = &self.account_runtime {
                    account.notify_local_settings_changed();
                }
            }
            Err(error) => {
                tracing::error!("Failed to update AI model: {error}");
                chat_view.set_status(Some(format!("Failed to update model: {error}")));
            }
        }
    }
}
