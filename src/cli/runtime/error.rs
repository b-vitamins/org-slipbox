use slipbox_core::{AnchorRecord, NodeRecord};
use slipbox_daemon_client::DaemonClientError;
use slipbox_rpc::JsonRpcErrorObject;

pub(crate) fn require_resolved_node(
    node: Option<NodeRecord>,
    error_message: String,
) -> Result<NodeRecord, DaemonClientError> {
    node.ok_or_else(|| DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(error_message)))
}

pub(crate) fn require_resolved_anchor(
    anchor: Option<AnchorRecord>,
    error_message: String,
) -> Result<AnchorRecord, DaemonClientError> {
    anchor.ok_or_else(|| DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(error_message)))
}

pub(crate) fn invalid_request_error(message: impl Into<String>) -> DaemonClientError {
    DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(message.into()))
}
