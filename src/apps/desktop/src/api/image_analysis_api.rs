//! Image Analysis API

use crate::api::app_state::AppState;
use bitfun_core::agentic::coordination::ConversationCoordinator;
use bitfun_core::agentic::image_analysis::{
    resolve_vision_model_from_ai_config, AnalyzeImagesRequest, ImageAnalysisResult, ImageAnalyzer,
    MessageEnhancer, SendEnhancedMessageRequest,
};
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

    let image_model = resolve_vision_model_from_ai_config(&ai_config).map_err(|e| {
        error!(
            "Image understanding model resolution failed: available_models={:?}, error={}",
            ai_config.models.iter().map(|m| &m.id).collect::<Vec<_>>(),
            e
        );
        format!(
            "Image understanding model is not configured.\n\n\
             Please select a model for [Settings → Default Model Config → Image Understanding Model].\n\n\
             Details: {}",
            e
        )
    })?;

    let workspace_path = state.workspace_path.read().await.clone();

    let ai_client = state
        .ai_client_factory
        .get_client_by_id(&image_model.id)
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
            false,
        )
        .await
        .map_err(|e| format!("Failed to send enhanced message: {}", e))?;

    Ok(())
}
