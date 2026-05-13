use std::ffi::OsString;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use slipbox_core::{
    AgendaParams, AgendaResult, AnchorRecord, AppendHeadingAtOutlinePathParams,
    AppendHeadingParams, AppendHeadingToNodeParams, BacklinksParams, BacklinksResult,
    CaptureNodeParams, CaptureTemplateParams, CaptureTemplatePreviewParams,
    CaptureTemplatePreviewResult, CompareNotesParams, CorpusAuditParams, CorpusAuditResult,
    EnsureFileNodeParams, EnsureNodeIdParams, ExecuteExplorationArtifactResult,
    ExplorationArtifactIdParams, ExplorationArtifactResult, ExploreParams, ExploreResult,
    ExtractSubtreeParams, ForwardLinksParams, ForwardLinksResult, GraphParams, GraphResult,
    ImportWorkbenchPackParams, ImportWorkbenchPackResult, IndexFileParams, IndexFileResult,
    IndexStats, IndexedFilesResult, ListExplorationArtifactsParams, ListExplorationArtifactsResult,
    ListReviewRoutinesParams, ListReviewRoutinesResult, ListReviewRunsParams, ListReviewRunsResult,
    ListWorkbenchPacksParams, ListWorkbenchPacksResult, ListWorkflowsParams, ListWorkflowsResult,
    MarkReviewFindingParams, MarkReviewFindingResult, NodeAtPointParams, NodeFromIdParams,
    NodeFromKeyParams, NodeFromRefParams, NodeFromTitleOrAliasParams, NodeRecord,
    NoteComparisonResult, PingInfo, RandomNodeResult, RefileRegionParams, RefileSubtreeParams,
    ReflinksParams, ReflinksResult, ReviewFindingRemediationApplyParams,
    ReviewFindingRemediationApplyResult, ReviewFindingRemediationPreviewParams,
    ReviewFindingRemediationPreviewResult, ReviewRoutineIdParams, ReviewRoutineResult,
    ReviewRunDiffParams, ReviewRunDiffResult, ReviewRunIdParams, ReviewRunResult,
    RewriteFileParams, RunReviewRoutineParams, RunReviewRoutineResult, RunWorkflowParams,
    RunWorkflowResult, SaveCorpusAuditReviewParams, SaveCorpusAuditReviewResult,
    SaveExplorationArtifactParams, SaveExplorationArtifactResult, SaveReviewRunParams,
    SaveReviewRunResult, SaveWorkflowReviewParams, SaveWorkflowReviewResult, SearchFilesParams,
    SearchFilesResult, SearchNodesParams, SearchNodesResult, SearchOccurrencesParams,
    SearchOccurrencesResult, SearchRefsParams, SearchRefsResult, SearchTagsParams,
    SearchTagsResult, StatusInfo, StructuralWriteReport, UnlinkedReferencesParams,
    UnlinkedReferencesResult, UpdateNodeMetadataParams, ValidateWorkbenchPackParams,
    ValidateWorkbenchPackResult, WorkbenchPackIdParams, WorkbenchPackManifest, WorkbenchPackResult,
    WorkflowIdParams, WorkflowResult,
};
use slipbox_rpc::{
    JsonRpcErrorObject, JsonRpcRequest, JsonRpcResponse, METHOD_AGENDA, METHOD_ANCHOR_AT_POINT,
    METHOD_APPEND_HEADING, METHOD_APPEND_HEADING_AT_OUTLINE_PATH, METHOD_APPEND_HEADING_TO_NODE,
    METHOD_BACKLINKS, METHOD_CAPTURE_NODE, METHOD_CAPTURE_TEMPLATE,
    METHOD_CAPTURE_TEMPLATE_PREVIEW, METHOD_COMPARE_NOTES, METHOD_CORPUS_AUDIT,
    METHOD_DELETE_EXPLORATION_ARTIFACT, METHOD_DELETE_REVIEW_RUN, METHOD_DELETE_WORKBENCH_PACK,
    METHOD_DEMOTE_ENTIRE_FILE, METHOD_DIFF_REVIEW_RUNS, METHOD_ENSURE_FILE_NODE,
    METHOD_ENSURE_NODE_ID, METHOD_EXECUTE_EXPLORATION_ARTIFACT, METHOD_EXPLORATION_ARTIFACT,
    METHOD_EXPLORE, METHOD_EXPORT_WORKBENCH_PACK, METHOD_EXTRACT_SUBTREE, METHOD_FORWARD_LINKS,
    METHOD_GRAPH_DOT, METHOD_IMPORT_WORKBENCH_PACK, METHOD_INDEX, METHOD_INDEX_FILE,
    METHOD_INDEXED_FILES, METHOD_LIST_EXPLORATION_ARTIFACTS, METHOD_LIST_REVIEW_ROUTINES,
    METHOD_LIST_REVIEW_RUNS, METHOD_LIST_WORKBENCH_PACKS, METHOD_LIST_WORKFLOWS,
    METHOD_MARK_REVIEW_FINDING, METHOD_NODE_AT_POINT, METHOD_NODE_FROM_ID, METHOD_NODE_FROM_KEY,
    METHOD_NODE_FROM_REF, METHOD_NODE_FROM_TITLE_OR_ALIAS, METHOD_PING, METHOD_PROMOTE_ENTIRE_FILE,
    METHOD_RANDOM_NODE, METHOD_REFILE_REGION, METHOD_REFILE_SUBTREE, METHOD_REFLINKS,
    METHOD_REVIEW_FINDING_REMEDIATION_APPLY, METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW,
    METHOD_REVIEW_ROUTINE, METHOD_REVIEW_RUN, METHOD_RUN_REVIEW_ROUTINE, METHOD_RUN_WORKFLOW,
    METHOD_SAVE_CORPUS_AUDIT_REVIEW, METHOD_SAVE_EXPLORATION_ARTIFACT, METHOD_SAVE_REVIEW_RUN,
    METHOD_SAVE_WORKFLOW_REVIEW, METHOD_SEARCH_FILES, METHOD_SEARCH_NODES,
    METHOD_SEARCH_OCCURRENCES, METHOD_SEARCH_REFS, METHOD_SEARCH_TAGS, METHOD_STATUS,
    METHOD_UNLINKED_REFERENCES, METHOD_UPDATE_NODE_METADATA, METHOD_VALIDATE_WORKBENCH_PACK,
    METHOD_WORKBENCH_PACK, METHOD_WORKFLOW, read_framed_message, write_framed_message,
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

    fn index(&mut self) -> Result<IndexStats, DaemonClientError> {
        self.request(METHOD_INDEX, &Value::Null)
    }

    fn index_file(
        &mut self,
        params: &IndexFileParams,
    ) -> Result<IndexFileResult, DaemonClientError> {
        self.request(METHOD_INDEX_FILE, params)
    }

    fn indexed_files(&mut self) -> Result<IndexedFilesResult, DaemonClientError> {
        self.request(METHOD_INDEXED_FILES, &Value::Null)
    }

    fn search_files(
        &mut self,
        params: &SearchFilesParams,
    ) -> Result<SearchFilesResult, DaemonClientError> {
        self.request(METHOD_SEARCH_FILES, params)
    }

    fn search_occurrences(
        &mut self,
        params: &SearchOccurrencesParams,
    ) -> Result<SearchOccurrencesResult, DaemonClientError> {
        self.request(METHOD_SEARCH_OCCURRENCES, params)
    }

    fn graph_dot(&mut self, params: &GraphParams) -> Result<GraphResult, DaemonClientError> {
        self.request(METHOD_GRAPH_DOT, params)
    }

    fn search_nodes(
        &mut self,
        params: &SearchNodesParams,
    ) -> Result<SearchNodesResult, DaemonClientError> {
        self.request(METHOD_SEARCH_NODES, params)
    }

    fn random_node(&mut self) -> Result<RandomNodeResult, DaemonClientError> {
        self.request(METHOD_RANDOM_NODE, &Value::Null)
    }

    fn search_tags(
        &mut self,
        params: &SearchTagsParams,
    ) -> Result<SearchTagsResult, DaemonClientError> {
        self.request(METHOD_SEARCH_TAGS, params)
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

    fn anchor_at_point(
        &mut self,
        params: &NodeAtPointParams,
    ) -> Result<Option<AnchorRecord>, DaemonClientError> {
        self.request(METHOD_ANCHOR_AT_POINT, params)
    }

    fn backlinks(
        &mut self,
        params: &BacklinksParams,
    ) -> Result<BacklinksResult, DaemonClientError> {
        self.request(METHOD_BACKLINKS, params)
    }

    fn forward_links(
        &mut self,
        params: &ForwardLinksParams,
    ) -> Result<ForwardLinksResult, DaemonClientError> {
        self.request(METHOD_FORWARD_LINKS, params)
    }

    fn reflinks(&mut self, params: &ReflinksParams) -> Result<ReflinksResult, DaemonClientError> {
        self.request(METHOD_REFLINKS, params)
    }

    fn unlinked_references(
        &mut self,
        params: &UnlinkedReferencesParams,
    ) -> Result<UnlinkedReferencesResult, DaemonClientError> {
        self.request(METHOD_UNLINKED_REFERENCES, params)
    }

    fn explore(&mut self, params: &ExploreParams) -> Result<ExploreResult, DaemonClientError> {
        self.request(METHOD_EXPLORE, params)
    }

    fn agenda(&mut self, params: &AgendaParams) -> Result<AgendaResult, DaemonClientError> {
        self.request(METHOD_AGENDA, params)
    }

    fn search_refs(
        &mut self,
        params: &SearchRefsParams,
    ) -> Result<SearchRefsResult, DaemonClientError> {
        self.request(METHOD_SEARCH_REFS, params)
    }

    fn compare_notes(
        &mut self,
        params: &CompareNotesParams,
    ) -> Result<NoteComparisonResult, DaemonClientError> {
        self.request(METHOD_COMPARE_NOTES, params)
    }

    fn capture_node(
        &mut self,
        params: &CaptureNodeParams,
    ) -> Result<NodeRecord, DaemonClientError> {
        self.request(METHOD_CAPTURE_NODE, params)
    }

    fn capture_template(
        &mut self,
        params: &CaptureTemplateParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_CAPTURE_TEMPLATE, params)
    }

    fn capture_template_preview(
        &mut self,
        params: &CaptureTemplatePreviewParams,
    ) -> Result<CaptureTemplatePreviewResult, DaemonClientError> {
        self.request(METHOD_CAPTURE_TEMPLATE_PREVIEW, params)
    }

    fn ensure_file_node(
        &mut self,
        params: &EnsureFileNodeParams,
    ) -> Result<NodeRecord, DaemonClientError> {
        self.request(METHOD_ENSURE_FILE_NODE, params)
    }

    fn append_heading(
        &mut self,
        params: &AppendHeadingParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_APPEND_HEADING, params)
    }

    fn append_heading_to_node(
        &mut self,
        params: &AppendHeadingToNodeParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_APPEND_HEADING_TO_NODE, params)
    }

    fn append_heading_at_outline_path(
        &mut self,
        params: &AppendHeadingAtOutlinePathParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_APPEND_HEADING_AT_OUTLINE_PATH, params)
    }

    fn ensure_node_id(
        &mut self,
        params: &EnsureNodeIdParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_ENSURE_NODE_ID, params)
    }

    fn update_node_metadata(
        &mut self,
        params: &UpdateNodeMetadataParams,
    ) -> Result<NodeRecord, DaemonClientError> {
        self.request(METHOD_UPDATE_NODE_METADATA, params)
    }

    fn refile_subtree(
        &mut self,
        params: &RefileSubtreeParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_REFILE_SUBTREE, params)
    }

    fn refile_region(
        &mut self,
        params: &RefileRegionParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_REFILE_REGION, params)
    }

    fn extract_subtree(
        &mut self,
        params: &ExtractSubtreeParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_EXTRACT_SUBTREE, params)
    }

    fn promote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_PROMOTE_ENTIRE_FILE, params)
    }

    fn demote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_DEMOTE_ENTIRE_FILE, params)
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

    fn list_review_routines(&mut self) -> Result<ListReviewRoutinesResult, DaemonClientError> {
        self.request(
            METHOD_LIST_REVIEW_ROUTINES,
            &ListReviewRoutinesParams::default(),
        )
    }

    fn review_routine(
        &mut self,
        params: &ReviewRoutineIdParams,
    ) -> Result<ReviewRoutineResult, DaemonClientError> {
        self.request(METHOD_REVIEW_ROUTINE, params)
    }

    fn run_review_routine(
        &mut self,
        params: &RunReviewRoutineParams,
    ) -> Result<RunReviewRoutineResult, DaemonClientError> {
        self.request(METHOD_RUN_REVIEW_ROUTINE, params)
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

    fn review_finding_remediation_preview(
        &mut self,
        params: &ReviewFindingRemediationPreviewParams,
    ) -> Result<ReviewFindingRemediationPreviewResult, DaemonClientError> {
        self.request(METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW, params)
    }

    fn review_finding_remediation_apply(
        &mut self,
        params: &ReviewFindingRemediationApplyParams,
    ) -> Result<ReviewFindingRemediationApplyResult, DaemonClientError> {
        self.request(METHOD_REVIEW_FINDING_REMEDIATION_APPLY, params)
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

    fn import_workbench_pack(
        &mut self,
        params: &ImportWorkbenchPackParams,
    ) -> Result<ImportWorkbenchPackResult, DaemonClientError> {
        self.request(METHOD_IMPORT_WORKBENCH_PACK, params)
    }

    fn workbench_pack(
        &mut self,
        params: &WorkbenchPackIdParams,
    ) -> Result<WorkbenchPackResult, DaemonClientError> {
        self.request(METHOD_WORKBENCH_PACK, params)
    }

    fn validate_workbench_pack(
        &mut self,
        params: &ValidateWorkbenchPackParams,
    ) -> Result<ValidateWorkbenchPackResult, DaemonClientError> {
        self.request(METHOD_VALIDATE_WORKBENCH_PACK, params)
    }

    fn export_workbench_pack(
        &mut self,
        params: &WorkbenchPackIdParams,
    ) -> Result<WorkbenchPackManifest, DaemonClientError> {
        self.request(METHOD_EXPORT_WORKBENCH_PACK, params)
    }

    fn list_workbench_packs(&mut self) -> Result<ListWorkbenchPacksResult, DaemonClientError> {
        self.request(
            METHOD_LIST_WORKBENCH_PACKS,
            &ListWorkbenchPacksParams::default(),
        )
    }

    fn delete_workbench_pack(
        &mut self,
        params: &WorkbenchPackIdParams,
    ) -> Result<slipbox_core::DeleteWorkbenchPackResult, DaemonClientError> {
        self.request(METHOD_DELETE_WORKBENCH_PACK, params)
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

    pub fn index(&mut self) -> Result<IndexStats, DaemonClientError> {
        self.rpc.index()
    }

    pub fn index_file(
        &mut self,
        params: &IndexFileParams,
    ) -> Result<IndexFileResult, DaemonClientError> {
        self.rpc.index_file(params)
    }

    pub fn indexed_files(&mut self) -> Result<IndexedFilesResult, DaemonClientError> {
        self.rpc.indexed_files()
    }

    pub fn search_files(
        &mut self,
        params: &SearchFilesParams,
    ) -> Result<SearchFilesResult, DaemonClientError> {
        self.rpc.search_files(params)
    }

    pub fn search_occurrences(
        &mut self,
        params: &SearchOccurrencesParams,
    ) -> Result<SearchOccurrencesResult, DaemonClientError> {
        self.rpc.search_occurrences(params)
    }

    pub fn graph_dot(&mut self, params: &GraphParams) -> Result<GraphResult, DaemonClientError> {
        self.rpc.graph_dot(params)
    }

    pub fn search_nodes(
        &mut self,
        params: &SearchNodesParams,
    ) -> Result<SearchNodesResult, DaemonClientError> {
        self.rpc.search_nodes(params)
    }

    pub fn random_node(&mut self) -> Result<RandomNodeResult, DaemonClientError> {
        self.rpc.random_node()
    }

    pub fn search_tags(
        &mut self,
        params: &SearchTagsParams,
    ) -> Result<SearchTagsResult, DaemonClientError> {
        self.rpc.search_tags(params)
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

    pub fn anchor_at_point(
        &mut self,
        params: &NodeAtPointParams,
    ) -> Result<Option<AnchorRecord>, DaemonClientError> {
        self.rpc.anchor_at_point(params)
    }

    pub fn backlinks(
        &mut self,
        params: &BacklinksParams,
    ) -> Result<BacklinksResult, DaemonClientError> {
        self.rpc.backlinks(params)
    }

    pub fn forward_links(
        &mut self,
        params: &ForwardLinksParams,
    ) -> Result<ForwardLinksResult, DaemonClientError> {
        self.rpc.forward_links(params)
    }

    pub fn reflinks(
        &mut self,
        params: &ReflinksParams,
    ) -> Result<ReflinksResult, DaemonClientError> {
        self.rpc.reflinks(params)
    }

    pub fn unlinked_references(
        &mut self,
        params: &UnlinkedReferencesParams,
    ) -> Result<UnlinkedReferencesResult, DaemonClientError> {
        self.rpc.unlinked_references(params)
    }

    pub fn explore(&mut self, params: &ExploreParams) -> Result<ExploreResult, DaemonClientError> {
        self.rpc.explore(params)
    }

    pub fn agenda(&mut self, params: &AgendaParams) -> Result<AgendaResult, DaemonClientError> {
        self.rpc.agenda(params)
    }

    pub fn search_refs(
        &mut self,
        params: &SearchRefsParams,
    ) -> Result<SearchRefsResult, DaemonClientError> {
        self.rpc.search_refs(params)
    }

    pub fn compare_notes(
        &mut self,
        params: &CompareNotesParams,
    ) -> Result<NoteComparisonResult, DaemonClientError> {
        self.rpc.compare_notes(params)
    }

    pub fn capture_node(
        &mut self,
        params: &CaptureNodeParams,
    ) -> Result<NodeRecord, DaemonClientError> {
        self.rpc.capture_node(params)
    }

    pub fn capture_template(
        &mut self,
        params: &CaptureTemplateParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.rpc.capture_template(params)
    }

    pub fn capture_template_preview(
        &mut self,
        params: &CaptureTemplatePreviewParams,
    ) -> Result<CaptureTemplatePreviewResult, DaemonClientError> {
        self.rpc.capture_template_preview(params)
    }

    pub fn ensure_file_node(
        &mut self,
        params: &EnsureFileNodeParams,
    ) -> Result<NodeRecord, DaemonClientError> {
        self.rpc.ensure_file_node(params)
    }

    pub fn append_heading(
        &mut self,
        params: &AppendHeadingParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.rpc.append_heading(params)
    }

    pub fn append_heading_to_node(
        &mut self,
        params: &AppendHeadingToNodeParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.rpc.append_heading_to_node(params)
    }

    pub fn append_heading_at_outline_path(
        &mut self,
        params: &AppendHeadingAtOutlinePathParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.rpc.append_heading_at_outline_path(params)
    }

    pub fn ensure_node_id(
        &mut self,
        params: &EnsureNodeIdParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.rpc.ensure_node_id(params)
    }

    pub fn update_node_metadata(
        &mut self,
        params: &UpdateNodeMetadataParams,
    ) -> Result<NodeRecord, DaemonClientError> {
        self.rpc.update_node_metadata(params)
    }

    pub fn refile_subtree(
        &mut self,
        params: &RefileSubtreeParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.rpc.refile_subtree(params)
    }

    pub fn refile_region(
        &mut self,
        params: &RefileRegionParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.rpc.refile_region(params)
    }

    pub fn extract_subtree(
        &mut self,
        params: &ExtractSubtreeParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.rpc.extract_subtree(params)
    }

    pub fn promote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.rpc.promote_entire_file(params)
    }

    pub fn demote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.rpc.demote_entire_file(params)
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

    pub fn list_review_routines(&mut self) -> Result<ListReviewRoutinesResult, DaemonClientError> {
        self.rpc.list_review_routines()
    }

    pub fn review_routine(
        &mut self,
        params: &ReviewRoutineIdParams,
    ) -> Result<ReviewRoutineResult, DaemonClientError> {
        self.rpc.review_routine(params)
    }

    pub fn run_review_routine(
        &mut self,
        params: &RunReviewRoutineParams,
    ) -> Result<RunReviewRoutineResult, DaemonClientError> {
        self.rpc.run_review_routine(params)
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

    pub fn review_finding_remediation_preview(
        &mut self,
        params: &ReviewFindingRemediationPreviewParams,
    ) -> Result<ReviewFindingRemediationPreviewResult, DaemonClientError> {
        self.rpc.review_finding_remediation_preview(params)
    }

    pub fn review_finding_remediation_apply(
        &mut self,
        params: &ReviewFindingRemediationApplyParams,
    ) -> Result<ReviewFindingRemediationApplyResult, DaemonClientError> {
        self.rpc.review_finding_remediation_apply(params)
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

    pub fn import_workbench_pack(
        &mut self,
        params: &ImportWorkbenchPackParams,
    ) -> Result<ImportWorkbenchPackResult, DaemonClientError> {
        self.rpc.import_workbench_pack(params)
    }

    pub fn workbench_pack(
        &mut self,
        params: &WorkbenchPackIdParams,
    ) -> Result<WorkbenchPackResult, DaemonClientError> {
        self.rpc.workbench_pack(params)
    }

    pub fn validate_workbench_pack(
        &mut self,
        params: &ValidateWorkbenchPackParams,
    ) -> Result<ValidateWorkbenchPackResult, DaemonClientError> {
        self.rpc.validate_workbench_pack(params)
    }

    pub fn export_workbench_pack(
        &mut self,
        params: &WorkbenchPackIdParams,
    ) -> Result<WorkbenchPackManifest, DaemonClientError> {
        self.rpc.export_workbench_pack(params)
    }

    pub fn list_workbench_packs(&mut self) -> Result<ListWorkbenchPacksResult, DaemonClientError> {
        self.rpc.list_workbench_packs()
    }

    pub fn delete_workbench_pack(
        &mut self,
        params: &WorkbenchPackIdParams,
    ) -> Result<slipbox_core::DeleteWorkbenchPackResult, DaemonClientError> {
        self.rpc.delete_workbench_pack(params)
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

    fn structural_report_response(id: u64, operation: &str) -> JsonRpcResponse {
        JsonRpcResponse::success(
            json!(id),
            json!({
                "operation": operation,
                "changed_files": ["notes.org"],
                "removed_files": [],
                "index_refresh": "refreshed",
                "result": null
            }),
        )
    }

    fn remediation_apply_response(id: u64) -> JsonRpcResponse {
        JsonRpcResponse::success(
            json!(id),
            json!({
                "application": {
                    "review_id": "review/audit/dangling-links",
                    "finding_id": "audit/dangling-links/source/missing-id",
                    "preview_identity": {
                        "kind": "dangling-link",
                        "source_node_key": "file:source.org",
                        "missing_explicit_id": "missing-id",
                        "file_path": "source.org",
                        "line": 6,
                        "column": 11,
                        "preview": "Points to [[id:missing-id][Missing]]."
                    },
                    "action": {
                        "kind": "unlink-dangling-link",
                        "source_node_key": "file:source.org",
                        "missing_explicit_id": "missing-id",
                        "file_path": "source.org",
                        "line": 6,
                        "column": 11,
                        "preview": "Points to [[id:missing-id][Missing]].",
                        "replacement_text": "Missing"
                    },
                    "changed_files": ["source.org"],
                    "removed_files": [],
                    "index_refresh": "refreshed"
                }
            }),
        )
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
    fn structural_rewrite_methods_send_canonical_params() {
        let mut client = RpcClient::new(MockTransport {
            requests: Vec::new(),
            responses: VecDeque::from([
                Ok(structural_report_response(1, "refile-subtree")),
                Ok(structural_report_response(2, "refile-region")),
                Ok(structural_report_response(3, "extract-subtree")),
                Ok(structural_report_response(4, "promote-file")),
                Ok(structural_report_response(5, "demote-file")),
            ]),
            shutdowns: 0,
        });

        let refile_subtree = client
            .refile_subtree(&RefileSubtreeParams {
                source_node_key: "heading:source.org:4".to_owned(),
                target_node_key: "heading:target.org:2".to_owned(),
            })
            .expect("refile subtree should parse report");
        let refile_region = client
            .refile_region(&RefileRegionParams {
                file_path: "source.org".to_owned(),
                start: 4,
                end: 8,
                target_node_key: "heading:target.org:2".to_owned(),
            })
            .expect("refile region should parse report");
        let extract_subtree = client
            .extract_subtree(&ExtractSubtreeParams {
                source_node_key: "heading:source.org:10".to_owned(),
                file_path: "extracted.org".to_owned(),
            })
            .expect("extract subtree should parse report");
        let promote = client
            .promote_entire_file(&RewriteFileParams {
                file_path: "promote.org".to_owned(),
            })
            .expect("promote should parse report");
        let demote = client
            .demote_entire_file(&RewriteFileParams {
                file_path: "demote.org".to_owned(),
            })
            .expect("demote should parse report");

        assert_eq!(
            refile_subtree.operation,
            slipbox_core::StructuralWriteOperationKind::RefileSubtree
        );
        assert_eq!(
            refile_region.operation,
            slipbox_core::StructuralWriteOperationKind::RefileRegion
        );
        assert_eq!(
            extract_subtree.operation,
            slipbox_core::StructuralWriteOperationKind::ExtractSubtree
        );
        assert_eq!(
            promote.operation,
            slipbox_core::StructuralWriteOperationKind::PromoteFile
        );
        assert_eq!(
            demote.operation,
            slipbox_core::StructuralWriteOperationKind::DemoteFile
        );

        assert_eq!(client.transport.requests[0].method, METHOD_REFILE_SUBTREE);
        assert_eq!(
            client.transport.requests[0].params,
            json!({
                "source_node_key": "heading:source.org:4",
                "target_node_key": "heading:target.org:2"
            })
        );
        assert_eq!(client.transport.requests[1].method, METHOD_REFILE_REGION);
        assert_eq!(
            client.transport.requests[1].params,
            json!({
                "file_path": "source.org",
                "start": 4,
                "end": 8,
                "target_node_key": "heading:target.org:2"
            })
        );
        assert_eq!(client.transport.requests[2].method, METHOD_EXTRACT_SUBTREE);
        assert_eq!(
            client.transport.requests[2].params,
            json!({
                "source_node_key": "heading:source.org:10",
                "file_path": "extracted.org"
            })
        );
        assert_eq!(
            client.transport.requests[3].method,
            METHOD_PROMOTE_ENTIRE_FILE
        );
        assert_eq!(
            client.transport.requests[3].params,
            json!({"file_path": "promote.org"})
        );
        assert_eq!(
            client.transport.requests[4].method,
            METHOD_DEMOTE_ENTIRE_FILE
        );
        assert_eq!(
            client.transport.requests[4].params,
            json!({"file_path": "demote.org"})
        );
    }

    #[test]
    fn remediation_apply_sends_canonical_params() {
        let mut client = RpcClient::new(MockTransport {
            requests: Vec::new(),
            responses: VecDeque::from([Ok(remediation_apply_response(1))]),
            shutdowns: 0,
        });

        let expected_preview = slipbox_core::AuditRemediationPreviewIdentity::DanglingLink {
            source_node_key: "file:source.org".to_owned(),
            missing_explicit_id: "missing-id".to_owned(),
            file_path: "source.org".to_owned(),
            line: 6,
            column: 11,
            preview: "Points to [[id:missing-id][Missing]].".to_owned(),
        };
        let action = slipbox_core::AuditRemediationApplyAction::UnlinkDanglingLink {
            source_node_key: "file:source.org".to_owned(),
            missing_explicit_id: "missing-id".to_owned(),
            file_path: "source.org".to_owned(),
            line: 6,
            column: 11,
            preview: "Points to [[id:missing-id][Missing]].".to_owned(),
            replacement_text: "Missing".to_owned(),
        };
        let result = client
            .review_finding_remediation_apply(&ReviewFindingRemediationApplyParams {
                review_id: "review/audit/dangling-links".to_owned(),
                finding_id: "audit/dangling-links/source/missing-id".to_owned(),
                expected_preview: expected_preview.clone(),
                action: action.clone(),
            })
            .expect("apply result should parse");

        assert_eq!(result.application.preview_identity, expected_preview);
        assert_eq!(result.application.action, action);
        assert_eq!(
            result.application.affected_files.changed_files,
            vec!["source.org".to_owned()]
        );
        assert_eq!(
            client.transport.requests[0].method,
            METHOD_REVIEW_FINDING_REMEDIATION_APPLY
        );
        assert_eq!(
            client.transport.requests[0].params,
            json!({
                "review_id": "review/audit/dangling-links",
                "finding_id": "audit/dangling-links/source/missing-id",
                "expected_preview": {
                    "kind": "dangling-link",
                    "source_node_key": "file:source.org",
                    "missing_explicit_id": "missing-id",
                    "file_path": "source.org",
                    "line": 6,
                    "column": 11,
                    "preview": "Points to [[id:missing-id][Missing]]."
                },
                "action": {
                    "kind": "unlink-dangling-link",
                    "source_node_key": "file:source.org",
                    "missing_explicit_id": "missing-id",
                    "file_path": "source.org",
                    "line": 6,
                    "column": 11,
                    "preview": "Points to [[id:missing-id][Missing]].",
                    "replacement_text": "Missing"
                }
            })
        );
    }

    #[test]
    fn structural_rewrite_methods_map_rpc_errors() {
        let response = JsonRpcResponse::error(
            json!(1),
            JsonRpcErrorObject::invalid_request("unknown source node: missing".to_owned()),
        );
        let mut client = RpcClient::new(MockTransport::with_response(response));

        let error = client
            .refile_subtree(&RefileSubtreeParams {
                source_node_key: "missing".to_owned(),
                target_node_key: "heading:target.org:2".to_owned(),
            })
            .expect_err("rpc error should surface");

        match error {
            DaemonClientError::Rpc(error) => {
                assert_eq!(error.code, -32600);
                assert_eq!(error.message, "unknown source node: missing");
            }
            other => panic!("unexpected error: {other}"),
        }
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
