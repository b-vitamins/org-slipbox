use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const METHOD_PING: &str = "slipbox/ping";
pub const METHOD_STATUS: &str = "slipbox/status";
pub const METHOD_INDEX: &str = "slipbox/index";
pub const METHOD_INDEX_FILE: &str = "slipbox/indexFile";
pub const METHOD_INDEXED_FILES: &str = "slipbox/indexedFiles";
pub const METHOD_GRAPH_DOT: &str = "slipbox/graphDot";
pub const METHOD_SEARCH_NODES: &str = "slipbox/searchNodes";
pub const METHOD_RANDOM_NODE: &str = "slipbox/randomNode";
pub const METHOD_SEARCH_TAGS: &str = "slipbox/searchTags";
pub const METHOD_NODE_FROM_ID: &str = "slipbox/nodeFromId";
pub const METHOD_NODE_FROM_TITLE_OR_ALIAS: &str = "slipbox/nodeFromTitleOrAlias";
pub const METHOD_NODE_AT_POINT: &str = "slipbox/nodeAtPoint";
pub const METHOD_BACKLINKS: &str = "slipbox/backlinks";
pub const METHOD_AGENDA: &str = "slipbox/agenda";
pub const METHOD_SEARCH_REFS: &str = "slipbox/searchRefs";
pub const METHOD_NODE_FROM_REF: &str = "slipbox/nodeFromRef";
pub const METHOD_CAPTURE_NODE: &str = "slipbox/captureNode";
pub const METHOD_CAPTURE_TEMPLATE: &str = "slipbox/captureTemplate";
pub const METHOD_CAPTURE_TEMPLATE_PREVIEW: &str = "slipbox/captureTemplatePreview";
pub const METHOD_ENSURE_FILE_NODE: &str = "slipbox/ensureFileNode";
pub const METHOD_APPEND_HEADING: &str = "slipbox/appendHeading";
pub const METHOD_APPEND_HEADING_TO_NODE: &str = "slipbox/appendHeadingToNode";
pub const METHOD_APPEND_HEADING_AT_OUTLINE_PATH: &str = "slipbox/appendHeadingAtOutlinePath";
pub const METHOD_ENSURE_NODE_ID: &str = "slipbox/ensureNodeId";
pub const METHOD_UPDATE_NODE_METADATA: &str = "slipbox/updateNodeMetadata";
pub const METHOD_REFILE_SUBTREE: &str = "slipbox/refileSubtree";
pub const METHOD_REFILE_REGION: &str = "slipbox/refileRegion";
pub const METHOD_EXTRACT_SUBTREE: &str = "slipbox/extractSubtree";
pub const METHOD_PROMOTE_ENTIRE_FILE: &str = "slipbox/promoteEntireFile";
pub const METHOD_DEMOTE_ENTIRE_FILE: &str = "slipbox/demoteEntireFile";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcErrorObject>,
}

impl JsonRpcResponse {
    #[must_use]
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    #[must_use]
    pub fn error(id: Value, error: JsonRpcErrorObject) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcErrorObject {
    pub code: i64,
    pub message: String,
}

impl fmt::Display for JsonRpcErrorObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

impl JsonRpcErrorObject {
    #[must_use]
    pub fn parse_error(message: String) -> Self {
        Self {
            code: -32700,
            message,
        }
    }

    #[must_use]
    pub fn invalid_request(message: String) -> Self {
        Self {
            code: -32600,
            message,
        }
    }

    #[must_use]
    pub fn method_not_found(message: String) -> Self {
        Self {
            code: -32601,
            message,
        }
    }

    #[must_use]
    pub fn internal_error(message: String) -> Self {
        Self {
            code: -32603,
            message,
        }
    }
}

#[derive(Debug, Error)]
#[error("{inner}")]
pub struct JsonRpcError {
    inner: JsonRpcErrorObject,
}

impl JsonRpcError {
    #[must_use]
    pub fn new(inner: JsonRpcErrorObject) -> Self {
        Self { inner }
    }

    #[must_use]
    pub fn into_inner(self) -> JsonRpcErrorObject {
        self.inner
    }
}
