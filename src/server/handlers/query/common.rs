use slipbox_core::{
    ExplorationArtifactIdParams, ReviewRoutineIdParams, ReviewRunIdParams, WorkbenchPackIdParams,
    WorkflowIdParams,
};
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

pub(super) fn invalid_request(message: String) -> JsonRpcError {
    JsonRpcError::new(JsonRpcErrorObject::invalid_request(message))
}

pub(super) fn with_step_context(step_id: &str, error: JsonRpcError) -> JsonRpcError {
    let inner = error.into_inner();
    JsonRpcError::new(JsonRpcErrorObject {
        code: inner.code,
        message: format!("workflow step {step_id} failed: {}", inner.message),
    })
}

pub(super) fn validate_artifact_id_params(
    params: &ExplorationArtifactIdParams,
) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}

pub(super) fn validate_review_id_params(params: &ReviewRunIdParams) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}

pub(super) fn validate_pack_id_params(params: &WorkbenchPackIdParams) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}

pub(super) fn validate_workflow_id_params(params: &WorkflowIdParams) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}

pub(super) fn validate_review_routine_id_params(
    params: &ReviewRoutineIdParams,
) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}
