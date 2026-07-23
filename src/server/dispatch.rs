use slipbox_rpc::{
    JsonRpcError, JsonRpcErrorObject, JsonRpcRequest, JsonRpcResponse, is_read_only,
};

use crate::service::SlipboxService;

pub(super) fn handle_request(
    service: &mut SlipboxService,
    request: JsonRpcRequest,
    read_only: bool,
) -> JsonRpcResponse {
    let JsonRpcRequest { id, method, .. } = request;
    let id = id.unwrap_or(serde_json::Value::Null);

    let response = match read_only_guard(&method, read_only) {
        Ok(()) => service.invoke_value(&method, request.params),
        Err(error) => Err(error),
    };

    match response {
        Ok(result) => JsonRpcResponse::success(id, result),
        Err(error) => JsonRpcResponse::error(id, error.into_inner()),
    }
}

/// Refuse any method `slipbox-rpc` does not classify `ReadOnly` when the session
/// is read-only. `is_read_only` is fail-closed: an unknown method is rejected.
fn read_only_guard(method: &str, read_only: bool) -> Result<(), JsonRpcError> {
    if read_only && !is_read_only(method) {
        return Err(JsonRpcError::new(JsonRpcErrorObject::invalid_request(
            format!("method {method} is not available on a read-only slipbox session"),
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use slipbox_rpc::{METHOD_CAPTURE_NODE, METHOD_GRADE_TERM, METHOD_NOTE_CONTEXT, METHOD_PING};

    use super::read_only_guard;

    #[test]
    fn read_only_session_admits_read_only_methods() {
        assert!(read_only_guard(METHOD_PING, true).is_ok());
        assert!(read_only_guard(METHOD_NOTE_CONTEXT, true).is_ok());
    }

    #[test]
    fn read_only_session_refuses_mutating_methods() {
        assert!(read_only_guard(METHOD_CAPTURE_NODE, true).is_err());
        assert!(read_only_guard(METHOD_GRADE_TERM, true).is_err());
    }

    #[test]
    fn read_only_session_refuses_unknown_methods_fail_closed() {
        assert!(read_only_guard("slipbox/methodThatDoesNotExist", true).is_err());
    }

    #[test]
    fn open_session_admits_every_method() {
        assert!(read_only_guard(METHOD_CAPTURE_NODE, false).is_ok());
        assert!(read_only_guard(METHOD_GRADE_TERM, false).is_ok());
        assert!(read_only_guard("slipbox/methodThatDoesNotExist", false).is_ok());
    }
}
