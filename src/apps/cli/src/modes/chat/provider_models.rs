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
        let request = AddModelRequest {
            model: result.to_mutation(model_id.clone()),
            make_primary_if_empty: true,
        };
        let outcome =
            tokio::task::block_in_place(|| rt_handle.block_on(self.agent.add_model(request)));

        match outcome {
            Ok(_) => {
                chat_view.set_status(Some(format!("Model added: {}", result.name)));
                chat_state.current_model_name = format!("{} / {}", result.model_name, result.name);
                tracing::info!("Added new AI model: {} ({})", model_id, result.model_name);
                crate::account_sync::notify_local_settings_changed();
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
            rt_handle.block_on(self.agent.get_model(model_id.clone()))
        });

        match outcome {
            Ok(response) => {
                let form_data = ModelFormResult::from_projection(response.model);
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
        let request = UpdateModelRequest {
            model_id: model_id.clone(),
            model: result.to_mutation(model_id.clone()),
        };
        let outcome =
            tokio::task::block_in_place(|| rt_handle.block_on(self.agent.update_model(request)));

        match outcome {
            Ok(_) => {
                chat_view.set_status(Some(format!("Model updated: {}", result.name)));
                chat_state.current_model_name = format!("{} / {}", result.model_name, result.name);
                tracing::info!("Updated AI model: {model_id}");
                crate::account_sync::notify_local_settings_changed();
            }
            Err(error) => {
                tracing::error!("Failed to update AI model: {error}");
                chat_view.set_status(Some(format!("Failed to update model: {error}")));
            }
        }
    }
}
