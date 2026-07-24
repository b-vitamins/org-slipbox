use serde_json::json;
use slipbox_daemon_client::DaemonClientError;
use slipbox_rpc::{JsonRpcErrorKind, JsonRpcErrorObject};

use crate::error::ReadingBridgeError;

/// An HTTP-shaped error for the reading API. The `kind` is a stable slug a
/// client branches on, so it is contractual; the `message` is human-facing
/// detail.
#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: u16,
    kind: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: u16, kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            kind,
            message: message.into(),
        }
    }

    /// A malformed or missing query parameter.
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, "bad-request", message)
    }

    /// A well-formed lookup that resolved to nothing, or an unknown route.
    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, "not-found", message)
    }

    /// A verb other than `GET` reached a reading route.
    pub(crate) fn method_not_allowed() -> Self {
        Self::new(
            405,
            "method-not-allowed",
            "the reading API serves GET requests only",
        )
    }

    /// The request carried a body. A reading route has none to consume at any
    /// size, so the length is refused rather than measured.
    pub(crate) fn payload_too_large() -> Self {
        Self::new(
            413,
            "payload-too-large",
            "the reading API serves requests without a body",
        )
    }

    /// A response value could not be serialized: a defect in this server, kept
    /// distinct from an upstream failure.
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(500, "internal", message)
    }

    /// The JSON body sent to the client.
    pub(crate) fn body(&self) -> String {
        json!({
            "error": {
                "kind": self.kind,
                "message": self.message,
            }
        })
        .to_string()
    }
}

impl From<ReadingBridgeError> for ApiError {
    fn from(error: ReadingBridgeError) -> Self {
        match error {
            // Poisoning is a panic inside this process, so 503 rather than a
            // status that blames the daemon.
            ReadingBridgeError::Poisoned => Self::new(
                503,
                "unavailable",
                "read-only slipbox session is no longer able to serve requests",
            ),
            ReadingBridgeError::Daemon(daemon) => Self::from(daemon),
        }
    }
}

impl From<DaemonClientError> for ApiError {
    fn from(error: DaemonClientError) -> Self {
        match error {
            DaemonClientError::Rpc(object) => rpc_error_to_api(object),
            // The daemon is gone: nothing upstream is available to answer, and a
            // caller may retry a 503.
            DaemonClientError::ConnectionClosed | DaemonClientError::DaemonExited { .. } => {
                Self::new(503, "unavailable", error.to_string())
            }
            // A transport-level fault: the daemon answered unusably.
            other => Self::new(502, "upstream", other.to_string()),
        }
    }
}

/// Map a structured JSON-RPC error to an HTTP status, preferring the daemon's
/// own error kind and falling back to its numeric code.
fn rpc_error_to_api(object: JsonRpcErrorObject) -> ApiError {
    let status = match object.data.as_ref().map(|data| data.kind) {
        Some(JsonRpcErrorKind::NotFound) => 404,
        Some(JsonRpcErrorKind::InvalidParams | JsonRpcErrorKind::ParseError) => 400,
        Some(JsonRpcErrorKind::PathDenied) => 403,
        Some(JsonRpcErrorKind::Conflict | JsonRpcErrorKind::StaleRequest) => 409,
        // A read-only front-end issues a fixed method set, so an unknown method
        // is this server asking for a route that does not exist, not a bad
        // request from the reader.
        Some(JsonRpcErrorKind::MethodNotFound) => 404,
        Some(JsonRpcErrorKind::Internal) => 502,
        None => match object.code {
            -32700 | -32600 => 400,
            -32601 => 404,
            _ => 502,
        },
    };
    let kind = match status {
        400 => "bad-request",
        403 => "forbidden",
        404 => "not-found",
        409 => "conflict",
        _ => "upstream",
    };
    ApiError::new(status, kind, object.message)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use slipbox_rpc::JsonRpcErrorData;

    use super::*;

    fn classified(kind: JsonRpcErrorKind) -> DaemonClientError {
        DaemonClientError::Rpc(JsonRpcErrorObject {
            code: -32000,
            message: "the daemon said so".to_owned(),
            data: Some(JsonRpcErrorData {
                kind,
                details: None,
            }),
        })
    }

    fn unclassified(code: i64) -> DaemonClientError {
        DaemonClientError::Rpc(JsonRpcErrorObject {
            code,
            message: "unclassified".to_owned(),
            data: None,
        })
    }

    #[test]
    fn a_classified_daemon_error_takes_the_status_its_kind_names() {
        for (kind, status) in [
            (JsonRpcErrorKind::NotFound, 404),
            (JsonRpcErrorKind::InvalidParams, 400),
            (JsonRpcErrorKind::ParseError, 400),
            (JsonRpcErrorKind::PathDenied, 403),
            (JsonRpcErrorKind::Conflict, 409),
            (JsonRpcErrorKind::StaleRequest, 409),
            (JsonRpcErrorKind::Internal, 502),
        ] {
            let error = ApiError::from(classified(kind));
            assert_eq!(error.status, status, "{kind:?} should map to {status}");
        }
    }

    #[test]
    fn an_unknown_method_reads_as_a_missing_route() {
        assert_eq!(
            ApiError::from(classified(JsonRpcErrorKind::MethodNotFound)).status,
            404
        );
    }

    #[test]
    fn an_unclassified_daemon_error_falls_back_to_its_numeric_code() {
        assert_eq!(ApiError::from(unclassified(-32700)).status, 400);
        assert_eq!(ApiError::from(unclassified(-32600)).status, 400);
        assert_eq!(ApiError::from(unclassified(-32601)).status, 404);
        assert_eq!(ApiError::from(unclassified(-32000)).status, 502);
    }

    #[test]
    fn a_lost_daemon_is_unavailable_and_a_misbehaving_one_is_upstream() {
        assert_eq!(
            ApiError::from(DaemonClientError::ConnectionClosed).status,
            503
        );
        assert_eq!(ApiError::from(DaemonClientError::UnexpectedEof).status, 502);
    }

    #[test]
    fn a_poisoned_bridge_is_unavailable_not_an_upstream_fault() {
        let error = ApiError::from(ReadingBridgeError::Poisoned);
        assert_eq!(error.status, 503);
        assert_eq!(body_kind(&error), "unavailable");
    }

    #[test]
    fn every_constructor_pairs_its_status_with_a_stable_kind_slug() {
        for (error, status, kind) in [
            (ApiError::bad_request("no"), 400, "bad-request"),
            (ApiError::not_found("gone"), 404, "not-found"),
            (ApiError::method_not_allowed(), 405, "method-not-allowed"),
            (ApiError::payload_too_large(), 413, "payload-too-large"),
            (ApiError::internal("ours"), 500, "internal"),
        ] {
            assert_eq!(error.status, status);
            assert_eq!(body_kind(&error), kind);
        }
    }

    #[test]
    fn the_body_carries_the_kind_and_message_a_client_reads() {
        let error = ApiError::not_found("no note answers to that key");
        let body: Value = serde_json::from_str(&error.body()).expect("the body is JSON");
        assert_eq!(body["error"]["kind"], "not-found");
        assert_eq!(body["error"]["message"], "no note answers to that key");
    }

    fn body_kind(error: &ApiError) -> String {
        let body: Value = serde_json::from_str(&error.body()).expect("the body is JSON");
        body["error"]["kind"]
            .as_str()
            .expect("the envelope carries a kind")
            .to_owned()
    }
}
