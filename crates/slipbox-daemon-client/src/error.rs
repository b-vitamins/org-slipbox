use std::path::PathBuf;
use std::process::ExitStatus;

use slipbox_rpc::JsonRpcErrorObject;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DaemonClientError {
    #[error("failed to start slipbox daemon `{program}`: {source}")]
    StartDaemon {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("spawned daemon did not expose stdin")]
    MissingStdin,
    #[error("spawned daemon did not expose stdout")]
    MissingStdout,
    #[error("failed to write JSON-RPC request: {source}")]
    WriteRequest {
        #[source]
        source: anyhow::Error,
    },
    #[error("failed to serialize JSON-RPC request for `{method}`: {source}")]
    SerializeRequest {
        method: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to read JSON-RPC response: {source}")]
    ReadResponse {
        #[source]
        source: anyhow::Error,
    },
    #[error("daemon response stream ended unexpectedly")]
    UnexpectedEof,
    #[error("daemon exited before responding: {status}")]
    DaemonExited { status: ExitStatus },
    #[error("daemon response id mismatch: expected {expected}, got {actual}")]
    ResponseIdMismatch { expected: String, actual: String },
    #[error("daemon response for `{method}` contained neither result nor error")]
    MissingResponsePayload { method: &'static str },
    #[error("daemon returned malformed result for `{method}`: {source}")]
    MalformedResult {
        method: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}")]
    Rpc(JsonRpcErrorObject),
    #[error("daemon connection is already closed")]
    ConnectionClosed,
    #[error("failed to shut down daemon process: {source}")]
    Shutdown {
        #[source]
        source: std::io::Error,
    },
}
