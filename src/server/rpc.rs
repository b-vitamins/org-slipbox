use anyhow::anyhow;
use serde::de::DeserializeOwned;

use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

pub(super) fn parse_params<T>(params: serde_json::Value) -> Result<T, JsonRpcError>
where
    T: DeserializeOwned,
{
    let value = if params.is_null() {
        serde_json::json!({})
    } else {
        params
    };

    serde_json::from_value(value).map_err(|error| {
        JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
            "invalid request parameters: {error}"
        )))
    })
}

pub(super) fn to_value<T>(value: T) -> Result<serde_json::Value, JsonRpcError>
where
    T: serde::Serialize,
{
    serde_json::to_value(value)
        .map_err(|error| internal_error(anyhow!("failed to serialize JSON-RPC result: {error}")))
}

pub(super) fn internal_error(error: anyhow::Error) -> JsonRpcError {
    JsonRpcError::new(JsonRpcErrorObject::internal_error(error.to_string()))
}
