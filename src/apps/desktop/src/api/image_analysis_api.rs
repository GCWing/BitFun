//! Image Analysis API

use crate::api::app_state::AppState;
use bitfun_core::agentic::coordination::ConversationCoordinator;
use bitfun_core::agentic::image_analysis::*;
use log::error;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn analyze_images(
    request: AnalyzeImagesRequest,
    state: State<'_, AppState>,
) -> Result<Vec<ImageAnalysisResult>, String> {
    let ai_config: bitfun_core::service::config::types::AIConfig = state
        .config_service
        .get_config(Some("ai"))
        .await
        .map_err(|e| {
            error!("Failed to get AI config: error={}", e);
            format!("Failed to get AI config: {}", e)
        })?;

    let image_model_id = ai_config
        .default_models
        .image_understanding
        .ok_or_else(|| {
            error!("Image understanding model not configured");
            "Image understanding model not configured".to_string()
        })?;

    let image_model_id = if image_model_id.is_empty() {
        let vision_model = ai_config
            .models
            .iter()
            .find(|m| {
                m.enabled
                    && m.capabilities.iter().any(|cap| {
                        matches!(
                        cap,
                        bitfun_core::service::config::types::ModelCapability::ImageUnderstanding
                    )
                    })
            })
            .map(|m| m.id.as_str());

        match vision_model {
            Some(model_id) => model_id,
            None => {
                error!("No image understanding model found");
                return Err(
                    "Image understanding model not configured and no compatible model found.\n\n\
                    Please add a model that supports image understanding\
                    in [Settings → AI Model Config], enable 'image_understanding' capability, \
                    and assign it in [Settings → Super Agent]."
                        .to_string(),
                );
            }
        }
    } else {
        &image_model_id
    };

    let image_model = ai_config
        .models
        .iter()
        .find(|m| &m.id == image_model_id)
        .ok_or_else(|| {
            error!(
                "Model not found: model_id={}, available_models={:?}",
                image_model_id,
                ai_config.models.iter().map(|m| &m.id).collect::<Vec<_>>()
            );
            format!("Model not found: {}", image_model_id)
        })?
        .clone();

    let workspace_path = state.workspace_path.read().await.clone();

    let ai_client = state
        .ai_client_factory
        .get_client_by_id(image_model_id)
        .await
        .map_err(|e| format!("Failed to create AI client: {}", e))?;

    let analyzer = ImageAnalyzer::new(workspace_path, ai_client);

    let results = analyzer
        .analyze_images(request, &image_model)
        .await
        .map_err(|e| format!("Image analysis failed: {}", e))?;

    Ok(results)
}

#[tauri::command]
pub async fn send_enhanced_message(
    request: SendEnhancedMessageRequest,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    let enhanced_message = MessageEnhancer::enhance_with_image_analysis(
        &request.original_message,
        &request.image_analyses,
        &request.other_contexts,
    );

    let _stream = coordinator
        .start_dialog_turn(
            request.session_id.clone(),
            enhanced_message.clone(),
            Some(request.dialog_turn_id.clone()),
            request.agent_type.clone(),
        )
        .await
        .map_err(|e| format!("Failed to send enhanced message: {}", e))?;

    Ok(())
}
