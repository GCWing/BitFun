use bitfun_core::agentic::learning_proposals::{
    get_learning_proposal_service, ApproveLearningProposalRequest, CreateLearningProposalRequest,
    GetLearningProposalRequest, LearningProposal, ListLearningProposalsRequest,
    RefreshLearningProposalRequest, RejectLearningProposalRequest,
};
use log::{error, info};

#[tauri::command]
pub async fn create_learning_proposal(
    request: CreateLearningProposalRequest,
) -> Result<LearningProposal, String> {
    let session_id = request.session_id.clone();
    let turn_id = request.source.turn_id.clone();
    let result = get_learning_proposal_service().create(request).await;
    match result {
        Ok(proposal) => {
            info!(
                "Learning proposal created: proposal_id={}, session_id={}, turn_id={}, status={:?}",
                proposal.proposal_id, session_id, turn_id, proposal.status
            );
            Ok(proposal)
        }
        Err(err) => {
            error!(
                "Failed to create learning proposal: session_id={}, turn_id={}, error={err}",
                session_id, turn_id
            );
            Err(err.to_string())
        }
    }
}

#[tauri::command]
pub async fn get_learning_proposal(
    request: GetLearningProposalRequest,
) -> Result<LearningProposal, String> {
    get_learning_proposal_service()
        .get(&request)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn list_learning_proposals(
    request: ListLearningProposalsRequest,
) -> Result<Vec<LearningProposal>, String> {
    get_learning_proposal_service()
        .list(&request)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn refresh_learning_proposal(
    request: RefreshLearningProposalRequest,
) -> Result<LearningProposal, String> {
    let proposal_id = request.proposal_id.clone();
    let result = get_learning_proposal_service().refresh(&request).await;
    if let Err(err) = &result {
        error!(
            "Failed to refresh learning proposal: proposal_id={}, error={err}",
            proposal_id
        );
    }
    result.map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn approve_learning_proposal(
    request: ApproveLearningProposalRequest,
) -> Result<LearningProposal, String> {
    let proposal_id = request.proposal_id.clone();
    let result = get_learning_proposal_service().approve(&request).await;
    match result {
        Ok(proposal) => {
            info!(
                "Learning proposal approval handled: proposal_id={}, status={:?}",
                proposal_id, proposal.status
            );
            Ok(proposal)
        }
        Err(err) => {
            error!(
                "Failed to approve learning proposal: proposal_id={}, error={err}",
                proposal_id
            );
            Err(err.to_string())
        }
    }
}

#[tauri::command]
pub async fn reject_learning_proposal(
    request: RejectLearningProposalRequest,
) -> Result<LearningProposal, String> {
    let proposal_id = request.proposal_id.clone();
    let result = get_learning_proposal_service().reject(&request).await;
    if let Err(err) = &result {
        error!(
            "Failed to reject learning proposal: proposal_id={}, error={err}",
            proposal_id
        );
    }
    result.map_err(|err| err.to_string())
}
