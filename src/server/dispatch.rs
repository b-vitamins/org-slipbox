use slipbox_rpc::{JsonRpcRequest, JsonRpcResponse};

use crate::service::SlipboxService;

pub(super) fn handle_request(
    service: &mut SlipboxService,
    request: JsonRpcRequest,
) -> JsonRpcResponse {
    let JsonRpcRequest { id, method, .. } = request;
    let id = id.unwrap_or(serde_json::Value::Null);

    let response = service.invoke_value(&method, request.params);

    match response {
        Ok(result) => JsonRpcResponse::success(id, result),
        Err(error) => JsonRpcResponse::error(id, error.into_inner()),
    }
}
