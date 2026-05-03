use std::fmt;
use std::io::{BufRead, Write};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const METHOD_PING: &str = "slipbox/ping";
pub const METHOD_STATUS: &str = "slipbox/status";
pub const METHOD_INDEX: &str = "slipbox/index";
pub const METHOD_INDEX_FILE: &str = "slipbox/indexFile";
pub const METHOD_INDEXED_FILES: &str = "slipbox/indexedFiles";
pub const METHOD_SEARCH_FILES: &str = "slipbox/searchFiles";
pub const METHOD_SEARCH_OCCURRENCES: &str = "slipbox/searchOccurrences";
pub const METHOD_GRAPH_DOT: &str = "slipbox/graphDot";
pub const METHOD_SEARCH_NODES: &str = "slipbox/searchNodes";
pub const METHOD_RANDOM_NODE: &str = "slipbox/randomNode";
pub const METHOD_SEARCH_TAGS: &str = "slipbox/searchTags";
pub const METHOD_NODE_FROM_ID: &str = "slipbox/nodeFromId";
pub const METHOD_NODE_FROM_KEY: &str = "slipbox/nodeFromKey";
pub const METHOD_NODE_FROM_TITLE_OR_ALIAS: &str = "slipbox/nodeFromTitleOrAlias";
pub const METHOD_NODE_AT_POINT: &str = "slipbox/nodeAtPoint";
pub const METHOD_ANCHOR_AT_POINT: &str = "slipbox/anchorAtPoint";
pub const METHOD_BACKLINKS: &str = "slipbox/backlinks";
pub const METHOD_FORWARD_LINKS: &str = "slipbox/forwardLinks";
pub const METHOD_REFLINKS: &str = "slipbox/reflinks";
pub const METHOD_UNLINKED_REFERENCES: &str = "slipbox/unlinkedReferences";
pub const METHOD_EXPLORE: &str = "slipbox/explore";
pub const METHOD_COMPARE_NOTES: &str = "slipbox/compareNotes";
pub const METHOD_LIST_WORKFLOWS: &str = "slipbox/listWorkflows";
pub const METHOD_WORKFLOW: &str = "slipbox/workflow";
pub const METHOD_RUN_WORKFLOW: &str = "slipbox/runWorkflow";
pub const METHOD_SAVE_EXPLORATION_ARTIFACT: &str = "slipbox/saveExplorationArtifact";
pub const METHOD_EXPLORATION_ARTIFACT: &str = "slipbox/explorationArtifact";
pub const METHOD_LIST_EXPLORATION_ARTIFACTS: &str = "slipbox/listExplorationArtifacts";
pub const METHOD_DELETE_EXPLORATION_ARTIFACT: &str = "slipbox/deleteExplorationArtifact";
pub const METHOD_EXECUTE_EXPLORATION_ARTIFACT: &str = "slipbox/executeExplorationArtifact";
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

impl JsonRpcRequest {
    #[must_use]
    pub fn new(id: Value, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: Some(id),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_jsonrpc_result",
        skip_serializing_if = "Option::is_none"
    )]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcErrorObject>,
}

impl JsonRpcResponse {
    #[must_use]
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: Some(result),
            error: None,
        }
    }

    #[must_use]
    pub fn error(id: Value, error: JsonRpcErrorObject) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

fn deserialize_optional_jsonrpc_result<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(
        Option::<Value>::deserialize(deserializer)?.unwrap_or(Value::Null),
    ))
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

pub fn read_framed_message<T>(reader: &mut impl BufRead) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .context("failed to read framing header")?;
        if bytes == 0 {
            return Ok(None);
        }

        if line == "\r\n" {
            break;
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        let (name, value) = trimmed
            .split_once(':')
            .with_context(|| format!("invalid header line: {trimmed}"))?;
        if name.eq_ignore_ascii_case("content-length") {
            let parsed = value
                .trim()
                .parse::<usize>()
                .with_context(|| format!("invalid content length: {}", value.trim()))?;
            content_length = Some(parsed);
        }
    }

    let length = content_length.context("missing Content-Length header")?;
    let mut body = vec![0_u8; length];
    reader
        .read_exact(&mut body)
        .context("failed to read framed body")?;
    let message = serde_json::from_slice(&body).context("invalid framed JSON body")?;
    Ok(Some(message))
}

pub fn write_framed_message<T>(writer: &mut impl Write, message: &T) -> Result<()>
where
    T: Serialize,
{
    let body = serde_json::to_vec(message).context("failed to serialize framed JSON body")?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())
        .context("failed to write framing header")?;
    writer
        .write_all(&body)
        .context("failed to write framed JSON body")?;
    writer
        .flush()
        .context("failed to flush framed JSON message")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use serde_json::json;

    use super::{JsonRpcRequest, JsonRpcResponse, read_framed_message, write_framed_message};

    #[test]
    fn framed_messages_round_trip() {
        let request = JsonRpcRequest::new(json!(7), "slipbox/ping", serde_json::Value::Null);
        let mut framed = Vec::new();

        write_framed_message(&mut framed, &request).expect("request should frame");

        let mut reader = BufReader::new(framed.as_slice());
        let decoded: JsonRpcRequest = read_framed_message(&mut reader)
            .expect("request should decode")
            .expect("request should be present");

        assert_eq!(decoded, request);
    }

    #[test]
    fn framed_messages_reject_missing_content_length() {
        let bytes = b"X-Header: nope\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(bytes.as_slice());

        let error =
            read_framed_message::<JsonRpcRequest>(&mut reader).expect_err("missing length fails");

        assert!(error.to_string().contains("missing Content-Length header"));
    }

    #[test]
    fn response_deserialization_preserves_explicit_null_results() {
        let response: JsonRpcResponse = serde_json::from_value(json!({
            "jsonrpc": "2.0",
            "id": 7,
            "result": null
        }))
        .expect("response with null result should deserialize");

        assert_eq!(response.result, Some(serde_json::Value::Null));
        assert!(response.error.is_none());
    }
}
