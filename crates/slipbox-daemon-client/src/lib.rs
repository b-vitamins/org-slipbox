use std::ffi::OsString;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use slipbox_core::{
    CompareNotesParams, CorpusAuditParams, CorpusAuditResult, ExecuteExplorationArtifactResult,
    ExplorationArtifactIdParams, ExplorationArtifactResult, ExploreParams, ExploreResult,
    ListExplorationArtifactsParams, ListExplorationArtifactsResult, ListReviewRunsParams,
    ListReviewRunsResult, ListWorkflowsParams, ListWorkflowsResult, MarkReviewFindingParams,
    MarkReviewFindingResult, NodeAtPointParams, NodeFromIdParams, NodeFromKeyParams,
    NodeFromRefParams, NodeFromTitleOrAliasParams, NodeRecord, NoteComparisonResult, PingInfo,
    ReviewRunDiffParams, ReviewRunDiffResult, ReviewRunIdParams, ReviewRunResult,
    RunWorkflowParams, RunWorkflowResult, SaveCorpusAuditReviewParams, SaveCorpusAuditReviewResult,
    SaveExplorationArtifactParams, SaveExplorationArtifactResult, SaveReviewRunParams,
    SaveReviewRunResult, SaveWorkflowReviewParams, SaveWorkflowReviewResult, SearchNodesParams,
    SearchNodesResult, StatusInfo, WorkflowIdParams, WorkflowResult,
};
use slipbox_rpc::{
    JsonRpcErrorObject, JsonRpcRequest, JsonRpcResponse, METHOD_COMPARE_NOTES, METHOD_CORPUS_AUDIT,
    METHOD_DELETE_EXPLORATION_ARTIFACT, METHOD_DELETE_REVIEW_RUN, METHOD_DIFF_REVIEW_RUNS,
    METHOD_EXECUTE_EXPLORATION_ARTIFACT, METHOD_EXPLORATION_ARTIFACT, METHOD_EXPLORE,
    METHOD_LIST_EXPLORATION_ARTIFACTS, METHOD_LIST_REVIEW_RUNS, METHOD_LIST_WORKFLOWS,
    METHOD_MARK_REVIEW_FINDING, METHOD_NODE_AT_POINT, METHOD_NODE_FROM_ID, METHOD_NODE_FROM_KEY,
    METHOD_NODE_FROM_REF, METHOD_NODE_FROM_TITLE_OR_ALIAS, METHOD_PING, METHOD_REVIEW_RUN,
    METHOD_RUN_WORKFLOW, METHOD_SAVE_CORPUS_AUDIT_REVIEW, METHOD_SAVE_EXPLORATION_ARTIFACT,
    METHOD_SAVE_REVIEW_RUN, METHOD_SAVE_WORKFLOW_REVIEW, METHOD_SEARCH_NODES, METHOD_STATUS,
    METHOD_WORKFLOW, read_framed_message, write_framed_message,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonServeConfig {
    pub root: PathBuf,
    pub db: PathBuf,
    pub workflow_dirs: Vec<PathBuf>,
    pub file_extensions: Vec<String>,
    pub exclude_regexps: Vec<String>,
}

impl DaemonServeConfig {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, db: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            db: db.into(),
            workflow_dirs: Vec::new(),
            file_extensions: Vec::new(),
            exclude_regexps: Vec::new(),
        }
    }

    fn command_args(&self) -> Vec<OsString> {
        let mut args = vec![
            OsString::from("serve"),
            OsString::from("--root"),
            self.root.as_os_str().to_owned(),
            OsString::from("--db"),
            self.db.as_os_str().to_owned(),
        ];
        for workflow_dir in &self.workflow_dirs {
            args.push(OsString::from("--workflow-dir"));
            args.push(workflow_dir.as_os_str().to_owned());
        }
        for extension in &self.file_extensions {
            args.push(OsString::from("--file-extension"));
            args.push(OsString::from(extension));
        }
        for regexp in &self.exclude_regexps {
            args.push(OsString::from("--exclude-regexp"));
            args.push(OsString::from(regexp));
        }
        args
    }
}

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

trait JsonRpcTransport {
    fn round_trip(&mut self, request: JsonRpcRequest)
    -> Result<JsonRpcResponse, DaemonClientError>;
    fn shutdown(&mut self) -> Result<(), DaemonClientError>;
}

struct RpcClient<T> {
    transport: T,
    next_request_id: u64,
}

impl<T> RpcClient<T>
where
    T: JsonRpcTransport,
{
    fn new(transport: T) -> Self {
        Self {
            transport,
            next_request_id: 1,
        }
    }

    fn request<Params, Response>(
        &mut self,
        method: &'static str,
        params: &Params,
    ) -> Result<Response, DaemonClientError>
    where
        Params: Serialize,
        Response: DeserializeOwned,
    {
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let params = serde_json::to_value(params)
            .map_err(|source| DaemonClientError::SerializeRequest { method, source })?;
        let request = JsonRpcRequest::new(Value::from(request_id), method, params);
        let response = self.transport.round_trip(request)?;
        self.decode_response(method, request_id, response)
    }

    fn decode_response<Response>(
        &self,
        method: &'static str,
        request_id: u64,
        response: JsonRpcResponse,
    ) -> Result<Response, DaemonClientError>
    where
        Response: DeserializeOwned,
    {
        let expected = request_id.to_string();
        let actual = response.id.to_string();
        if actual != expected {
            return Err(DaemonClientError::ResponseIdMismatch { expected, actual });
        }
        if let Some(error) = response.error {
            return Err(DaemonClientError::Rpc(error));
        }
        let result = response
            .result
            .ok_or(DaemonClientError::MissingResponsePayload { method })?;
        serde_json::from_value(result)
            .map_err(|source| DaemonClientError::MalformedResult { method, source })
    }

    fn ping(&mut self) -> Result<PingInfo, DaemonClientError> {
        self.request(METHOD_PING, &Value::Null)
    }

    fn status(&mut self) -> Result<StatusInfo, DaemonClientError> {
        self.request(METHOD_STATUS, &Value::Null)
    }

    fn search_nodes(
        &mut self,
        params: &SearchNodesParams,
    ) -> Result<SearchNodesResult, DaemonClientError> {
        self.request(METHOD_SEARCH_NODES, params)
    }

    fn node_from_id(
        &mut self,
        params: &NodeFromIdParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_FROM_ID, params)
    }

    fn node_from_key(
        &mut self,
        params: &NodeFromKeyParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_FROM_KEY, params)
    }

    fn node_from_title_or_alias(
        &mut self,
        params: &NodeFromTitleOrAliasParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_FROM_TITLE_OR_ALIAS, params)
    }

    fn node_from_ref(
        &mut self,
        params: &NodeFromRefParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_FROM_REF, params)
    }

    fn node_at_point(
        &mut self,
        params: &NodeAtPointParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_AT_POINT, params)
    }

    fn explore(&mut self, params: &ExploreParams) -> Result<ExploreResult, DaemonClientError> {
        self.request(METHOD_EXPLORE, params)
    }

    fn compare_notes(
        &mut self,
        params: &CompareNotesParams,
    ) -> Result<NoteComparisonResult, DaemonClientError> {
        self.request(METHOD_COMPARE_NOTES, params)
    }

    fn list_workflows(&mut self) -> Result<ListWorkflowsResult, DaemonClientError> {
        self.request(METHOD_LIST_WORKFLOWS, &ListWorkflowsParams::default())
    }

    fn workflow(&mut self, params: &WorkflowIdParams) -> Result<WorkflowResult, DaemonClientError> {
        self.request(METHOD_WORKFLOW, params)
    }

    fn run_workflow(
        &mut self,
        params: &RunWorkflowParams,
    ) -> Result<RunWorkflowResult, DaemonClientError> {
        self.request(METHOD_RUN_WORKFLOW, params)
    }

    fn corpus_audit(
        &mut self,
        params: &CorpusAuditParams,
    ) -> Result<CorpusAuditResult, DaemonClientError> {
        self.request(METHOD_CORPUS_AUDIT, params)
    }

    fn save_exploration_artifact(
        &mut self,
        params: &SaveExplorationArtifactParams,
    ) -> Result<SaveExplorationArtifactResult, DaemonClientError> {
        self.request(METHOD_SAVE_EXPLORATION_ARTIFACT, params)
    }

    fn exploration_artifact(
        &mut self,
        params: &ExplorationArtifactIdParams,
    ) -> Result<ExplorationArtifactResult, DaemonClientError> {
        self.request(METHOD_EXPLORATION_ARTIFACT, params)
    }

    fn list_exploration_artifacts(
        &mut self,
    ) -> Result<ListExplorationArtifactsResult, DaemonClientError> {
        self.request(
            METHOD_LIST_EXPLORATION_ARTIFACTS,
            &ListExplorationArtifactsParams::default(),
        )
    }

    fn delete_exploration_artifact(
        &mut self,
        params: &ExplorationArtifactIdParams,
    ) -> Result<slipbox_core::DeleteExplorationArtifactResult, DaemonClientError> {
        self.request(METHOD_DELETE_EXPLORATION_ARTIFACT, params)
    }

    fn execute_exploration_artifact(
        &mut self,
        params: &ExplorationArtifactIdParams,
    ) -> Result<ExecuteExplorationArtifactResult, DaemonClientError> {
        self.request(METHOD_EXECUTE_EXPLORATION_ARTIFACT, params)
    }

    fn save_review_run(
        &mut self,
        params: &SaveReviewRunParams,
    ) -> Result<SaveReviewRunResult, DaemonClientError> {
        self.request(METHOD_SAVE_REVIEW_RUN, params)
    }

    fn review_run(
        &mut self,
        params: &ReviewRunIdParams,
    ) -> Result<ReviewRunResult, DaemonClientError> {
        self.request(METHOD_REVIEW_RUN, params)
    }

    fn diff_review_runs(
        &mut self,
        params: &ReviewRunDiffParams,
    ) -> Result<ReviewRunDiffResult, DaemonClientError> {
        self.request(METHOD_DIFF_REVIEW_RUNS, params)
    }

    fn list_review_runs(&mut self) -> Result<ListReviewRunsResult, DaemonClientError> {
        self.request(METHOD_LIST_REVIEW_RUNS, &ListReviewRunsParams::default())
    }

    fn delete_review_run(
        &mut self,
        params: &ReviewRunIdParams,
    ) -> Result<slipbox_core::DeleteReviewRunResult, DaemonClientError> {
        self.request(METHOD_DELETE_REVIEW_RUN, params)
    }

    fn mark_review_finding(
        &mut self,
        params: &MarkReviewFindingParams,
    ) -> Result<MarkReviewFindingResult, DaemonClientError> {
        self.request(METHOD_MARK_REVIEW_FINDING, params)
    }

    fn save_corpus_audit_review(
        &mut self,
        params: &SaveCorpusAuditReviewParams,
    ) -> Result<SaveCorpusAuditReviewResult, DaemonClientError> {
        self.request(METHOD_SAVE_CORPUS_AUDIT_REVIEW, params)
    }

    fn save_workflow_review(
        &mut self,
        params: &SaveWorkflowReviewParams,
    ) -> Result<SaveWorkflowReviewResult, DaemonClientError> {
        self.request(METHOD_SAVE_WORKFLOW_REVIEW, params)
    }

    fn shutdown(&mut self) -> Result<(), DaemonClientError> {
        self.transport.shutdown()
    }
}

struct StdioTransport {
    child: Option<Child>,
    reader: Option<BufReader<ChildStdout>>,
    writer: Option<BufWriter<ChildStdin>>,
}

impl StdioTransport {
    fn spawn(
        program: impl Into<PathBuf>,
        config: &DaemonServeConfig,
    ) -> Result<Self, DaemonClientError> {
        let program = program.into();
        let mut command = Command::new(&program);
        command
            .args(config.command_args())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let child = command
            .spawn()
            .map_err(|source| DaemonClientError::StartDaemon {
                program: program.clone(),
                source,
            })?;
        Self::from_child(child)
    }

    fn from_child(mut child: Child) -> Result<Self, DaemonClientError> {
        let stdout = child
            .stdout
            .take()
            .ok_or(DaemonClientError::MissingStdout)?;
        let stdin = child.stdin.take().ok_or(DaemonClientError::MissingStdin)?;
        Ok(Self {
            child: Some(child),
            reader: Some(BufReader::new(stdout)),
            writer: Some(BufWriter::new(stdin)),
        })
    }
}

impl JsonRpcTransport for StdioTransport {
    fn round_trip(
        &mut self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, DaemonClientError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or(DaemonClientError::ConnectionClosed)?;
        write_framed_message(writer, &request)
            .map_err(|source| DaemonClientError::WriteRequest { source })?;

        let reader = self
            .reader
            .as_mut()
            .ok_or(DaemonClientError::ConnectionClosed)?;
        match read_framed_message(reader)
            .map_err(|source| DaemonClientError::ReadResponse { source })?
        {
            Some(response) => Ok(response),
            None => {
                if let Some(child) = self.child.as_mut() {
                    if let Some(status) = child
                        .try_wait()
                        .map_err(|source| DaemonClientError::Shutdown { source })?
                    {
                        Err(DaemonClientError::DaemonExited { status })
                    } else {
                        Err(DaemonClientError::UnexpectedEof)
                    }
                } else {
                    Err(DaemonClientError::ConnectionClosed)
                }
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), DaemonClientError> {
        self.reader.take();
        self.writer.take();
        if let Some(mut child) = self.child.take() {
            child
                .wait()
                .map_err(|source| DaemonClientError::Shutdown { source })?;
        }
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        self.reader.take();
        self.writer.take();
        if let Some(mut child) = self.child.take() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }
}

pub struct DaemonClient {
    rpc: RpcClient<StdioTransport>,
}

impl DaemonClient {
    pub fn spawn(
        program: impl Into<PathBuf>,
        config: &DaemonServeConfig,
    ) -> Result<Self, DaemonClientError> {
        Ok(Self {
            rpc: RpcClient::new(StdioTransport::spawn(program, config)?),
        })
    }

    pub fn from_child(child: Child) -> Result<Self, DaemonClientError> {
        Ok(Self {
            rpc: RpcClient::new(StdioTransport::from_child(child)?),
        })
    }

    pub fn ping(&mut self) -> Result<PingInfo, DaemonClientError> {
        self.rpc.ping()
    }

    pub fn status(&mut self) -> Result<StatusInfo, DaemonClientError> {
        self.rpc.status()
    }

    pub fn search_nodes(
        &mut self,
        params: &SearchNodesParams,
    ) -> Result<SearchNodesResult, DaemonClientError> {
        self.rpc.search_nodes(params)
    }

    pub fn node_from_id(
        &mut self,
        params: &NodeFromIdParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.rpc.node_from_id(params)
    }

    pub fn node_from_key(
        &mut self,
        params: &NodeFromKeyParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.rpc.node_from_key(params)
    }

    pub fn node_from_title_or_alias(
        &mut self,
        params: &NodeFromTitleOrAliasParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.rpc.node_from_title_or_alias(params)
    }

    pub fn node_from_ref(
        &mut self,
        params: &NodeFromRefParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.rpc.node_from_ref(params)
    }

    pub fn node_at_point(
        &mut self,
        params: &NodeAtPointParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.rpc.node_at_point(params)
    }

    pub fn explore(&mut self, params: &ExploreParams) -> Result<ExploreResult, DaemonClientError> {
        self.rpc.explore(params)
    }

    pub fn compare_notes(
        &mut self,
        params: &CompareNotesParams,
    ) -> Result<NoteComparisonResult, DaemonClientError> {
        self.rpc.compare_notes(params)
    }

    pub fn list_workflows(&mut self) -> Result<ListWorkflowsResult, DaemonClientError> {
        self.rpc.list_workflows()
    }

    pub fn workflow(
        &mut self,
        params: &WorkflowIdParams,
    ) -> Result<WorkflowResult, DaemonClientError> {
        self.rpc.workflow(params)
    }

    pub fn run_workflow(
        &mut self,
        params: &RunWorkflowParams,
    ) -> Result<RunWorkflowResult, DaemonClientError> {
        self.rpc.run_workflow(params)
    }

    pub fn corpus_audit(
        &mut self,
        params: &CorpusAuditParams,
    ) -> Result<CorpusAuditResult, DaemonClientError> {
        self.rpc.corpus_audit(params)
    }

    pub fn save_exploration_artifact(
        &mut self,
        params: &SaveExplorationArtifactParams,
    ) -> Result<SaveExplorationArtifactResult, DaemonClientError> {
        self.rpc.save_exploration_artifact(params)
    }

    pub fn exploration_artifact(
        &mut self,
        params: &ExplorationArtifactIdParams,
    ) -> Result<ExplorationArtifactResult, DaemonClientError> {
        self.rpc.exploration_artifact(params)
    }

    pub fn list_exploration_artifacts(
        &mut self,
    ) -> Result<ListExplorationArtifactsResult, DaemonClientError> {
        self.rpc.list_exploration_artifacts()
    }

    pub fn delete_exploration_artifact(
        &mut self,
        params: &ExplorationArtifactIdParams,
    ) -> Result<slipbox_core::DeleteExplorationArtifactResult, DaemonClientError> {
        self.rpc.delete_exploration_artifact(params)
    }

    pub fn execute_exploration_artifact(
        &mut self,
        params: &ExplorationArtifactIdParams,
    ) -> Result<ExecuteExplorationArtifactResult, DaemonClientError> {
        self.rpc.execute_exploration_artifact(params)
    }

    pub fn save_review_run(
        &mut self,
        params: &SaveReviewRunParams,
    ) -> Result<SaveReviewRunResult, DaemonClientError> {
        self.rpc.save_review_run(params)
    }

    pub fn review_run(
        &mut self,
        params: &ReviewRunIdParams,
    ) -> Result<ReviewRunResult, DaemonClientError> {
        self.rpc.review_run(params)
    }

    pub fn diff_review_runs(
        &mut self,
        params: &ReviewRunDiffParams,
    ) -> Result<ReviewRunDiffResult, DaemonClientError> {
        self.rpc.diff_review_runs(params)
    }

    pub fn list_review_runs(&mut self) -> Result<ListReviewRunsResult, DaemonClientError> {
        self.rpc.list_review_runs()
    }

    pub fn delete_review_run(
        &mut self,
        params: &ReviewRunIdParams,
    ) -> Result<slipbox_core::DeleteReviewRunResult, DaemonClientError> {
        self.rpc.delete_review_run(params)
    }

    pub fn mark_review_finding(
        &mut self,
        params: &MarkReviewFindingParams,
    ) -> Result<MarkReviewFindingResult, DaemonClientError> {
        self.rpc.mark_review_finding(params)
    }

    pub fn save_corpus_audit_review(
        &mut self,
        params: &SaveCorpusAuditReviewParams,
    ) -> Result<SaveCorpusAuditReviewResult, DaemonClientError> {
        self.rpc.save_corpus_audit_review(params)
    }

    pub fn save_workflow_review(
        &mut self,
        params: &SaveWorkflowReviewParams,
    ) -> Result<SaveWorkflowReviewResult, DaemonClientError> {
        self.rpc.save_workflow_review(params)
    }

    pub fn shutdown(mut self) -> Result<(), DaemonClientError> {
        self.rpc.shutdown()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::path::Path;

    use super::*;
    use serde_json::json;
    use slipbox_core::{ExplorationLens, SearchNodesSort};

    #[derive(Default)]
    struct MockTransport {
        requests: Vec<JsonRpcRequest>,
        responses: VecDeque<Result<JsonRpcResponse, DaemonClientError>>,
        shutdowns: usize,
    }

    impl MockTransport {
        fn with_response(response: JsonRpcResponse) -> Self {
            Self {
                requests: Vec::new(),
                responses: VecDeque::from([Ok(response)]),
                shutdowns: 0,
            }
        }
    }

    impl JsonRpcTransport for MockTransport {
        fn round_trip(
            &mut self,
            request: JsonRpcRequest,
        ) -> Result<JsonRpcResponse, DaemonClientError> {
            self.requests.push(request);
            self.responses
                .pop_front()
                .expect("mock response should be queued")
        }

        fn shutdown(&mut self) -> Result<(), DaemonClientError> {
            self.shutdowns += 1;
            Ok(())
        }
    }

    #[test]
    fn ping_uses_canonical_method_and_parses_typed_result() {
        let response = JsonRpcResponse::success(
            json!(1),
            serde_json::to_value(PingInfo {
                version: "0.6.1".to_owned(),
                root: "/tmp/notes".to_owned(),
                db: "/tmp/slipbox.sqlite".to_owned(),
            })
            .expect("ping result should serialize"),
        );
        let mut client = RpcClient::new(MockTransport::with_response(response));

        let result = client.ping().expect("ping should succeed");

        assert_eq!(result.root, "/tmp/notes");
        assert_eq!(client.transport.requests.len(), 1);
        assert_eq!(client.transport.requests[0].method, METHOD_PING);
        assert_eq!(client.transport.requests[0].params, Value::Null);
    }

    #[test]
    fn search_nodes_sends_structured_params() {
        let response = JsonRpcResponse::success(
            json!(1),
            serde_json::to_value(SearchNodesResult { nodes: Vec::new() })
                .expect("search result should serialize"),
        );
        let mut client = RpcClient::new(MockTransport::with_response(response));

        let result = client
            .search_nodes(&SearchNodesParams {
                query: "alpha".to_owned(),
                limit: 25,
                sort: Some(SearchNodesSort::Title),
            })
            .expect("search should succeed");

        assert!(result.nodes.is_empty());
        assert_eq!(client.transport.requests[0].method, METHOD_SEARCH_NODES);
        assert_eq!(
            client.transport.requests[0].params,
            json!({"query": "alpha", "limit": 25, "sort": "title"})
        );
    }

    #[test]
    fn maps_rpc_error_responses() {
        let response = JsonRpcResponse::error(
            json!(1),
            JsonRpcErrorObject::invalid_request("bad request".to_owned()),
        );
        let mut client = RpcClient::new(MockTransport::with_response(response));

        let error = client
            .explore(&ExploreParams {
                node_key: "file:alpha.org".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 10,
                unique: false,
            })
            .expect_err("rpc error should surface");

        match error {
            DaemonClientError::Rpc(error) => {
                assert_eq!(error.code, -32600);
                assert_eq!(error.message, "bad request");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_response_id_mismatch() {
        let response = JsonRpcResponse::success(
            json!(99),
            serde_json::to_value(StatusInfo {
                version: "0.6.1".to_owned(),
                root: "/tmp/notes".to_owned(),
                db: "/tmp/slipbox.sqlite".to_owned(),
                files_indexed: 1,
                nodes_indexed: 1,
                links_indexed: 0,
            })
            .expect("status should serialize"),
        );
        let mut client = RpcClient::new(MockTransport::with_response(response));

        let error = client.status().expect_err("mismatched id should fail");

        match error {
            DaemonClientError::ResponseIdMismatch { expected, actual } => {
                assert_eq!(expected, "1");
                assert_eq!(actual, "99");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_malformed_results() {
        let response = JsonRpcResponse::success(json!(1), json!({"version": 7}));
        let mut client = RpcClient::new(MockTransport::with_response(response));

        let error = client.status().expect_err("malformed result should fail");

        match error {
            DaemonClientError::MalformedResult { method, .. } => {
                assert_eq!(method, METHOD_STATUS);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn startup_failure_is_explicit() {
        let config = DaemonServeConfig::new("/tmp/notes", "/tmp/slipbox.sqlite");

        let result =
            DaemonClient::spawn(Path::new("/definitely/not/a/real/slipbox-binary"), &config);

        match result {
            Ok(_) => panic!("missing program should fail"),
            Err(DaemonClientError::StartDaemon { program, .. }) => {
                assert!(program.ends_with("slipbox-binary"));
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }
}
