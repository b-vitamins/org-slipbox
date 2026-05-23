use anyhow::Result;
use slipbox_core::{ReviewFindingStatus, WorkflowInputAssignment, WorkflowResolveTarget};
use slipbox_daemon_client::DaemonClientError;
use slipbox_rpc::JsonRpcErrorObject;

pub(crate) fn parse_review_finding_status(value: &str) -> Result<ReviewFindingStatus> {
    match value {
        "open" => Ok(ReviewFindingStatus::Open),
        "reviewed" => Ok(ReviewFindingStatus::Reviewed),
        "dismissed" => Ok(ReviewFindingStatus::Dismissed),
        "accepted" => Ok(ReviewFindingStatus::Accepted),
        _ => anyhow::bail!(
            "invalid review finding status `{value}`; expected one of: open, reviewed, dismissed, accepted"
        ),
    }
}

pub(crate) fn parse_workflow_input_assignments(
    values: &[String],
) -> Result<Vec<WorkflowInputAssignment>, DaemonClientError> {
    values
        .iter()
        .map(|value| parse_workflow_input_assignment(value))
        .collect()
}

fn parse_workflow_input_assignment(
    value: &str,
) -> Result<WorkflowInputAssignment, DaemonClientError> {
    let (input_id, encoded_target) = value.split_once('=').ok_or_else(|| {
        DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(format!(
            "invalid workflow input assignment {value}: expected input-id=kind:value"
        )))
    })?;
    let (kind, target_value) = encoded_target.split_once(':').ok_or_else(|| {
        DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(format!(
            "invalid workflow input assignment {value}: expected input-id=kind:value"
        )))
    })?;
    if input_id.trim().is_empty() || target_value.trim().is_empty() {
        return Err(DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(
            format!(
                "invalid workflow input assignment {value}: expected non-empty input id and target value"
            ),
        )));
    }

    let target = match kind {
        "id" => WorkflowResolveTarget::Id {
            id: target_value.to_owned(),
        },
        "title" => WorkflowResolveTarget::Title {
            title: target_value.to_owned(),
        },
        "ref" => WorkflowResolveTarget::Reference {
            reference: target_value.to_owned(),
        },
        "key" => WorkflowResolveTarget::NodeKey {
            node_key: target_value.to_owned(),
        },
        _ => {
            return Err(DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(
                format!("invalid workflow input assignment {value}: unknown target kind {kind}"),
            )));
        }
    };

    Ok(WorkflowInputAssignment {
        input_id: input_id.to_owned(),
        target,
    })
}
