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
    ExtractSubtreeParams, FileDiagnosticsParams, FileDiagnosticsResult, ForwardLinksParams,
    ForwardLinksResult, GraphParams, GraphResult, ImportWorkbenchPackParams,
    ImportWorkbenchPackResult, IndexDiagnosticsResult, IndexFileParams, IndexFileResult,
    IndexStats, IndexedFilesResult, ListExplorationArtifactsParams, ListExplorationArtifactsResult,
    ListReviewRoutinesParams, ListReviewRoutinesResult, ListReviewRunsParams, ListReviewRunsResult,
    ListWorkbenchPacksParams, ListWorkbenchPacksResult, ListWorkflowsParams, ListWorkflowsResult,
    MarkReviewFindingParams, MarkReviewFindingResult, NodeAtPointParams, NodeDiagnosticsParams,
    NodeDiagnosticsResult, NodeFromIdParams, NodeFromKeyParams, NodeFromRefParams,
    NodeFromTitleOrAliasParams, NodeRecord, NoteComparisonResult, PingInfo, RandomNodeResult,
    RefileRegionParams, RefileSubtreeParams, ReflinksParams, ReflinksResult,
    ReviewFindingRemediationApplyParams, ReviewFindingRemediationApplyResult,
    ReviewFindingRemediationPreviewParams, ReviewFindingRemediationPreviewResult,
    ReviewRoutineIdParams, ReviewRoutineResult, ReviewRunDiffParams, ReviewRunDiffResult,
    ReviewRunIdParams, ReviewRunResult, RewriteFileParams, RunReviewRoutineParams,
    RunReviewRoutineResult, RunWorkflowParams, RunWorkflowResult, SaveCorpusAuditReviewParams,
    SaveCorpusAuditReviewResult, SaveExplorationArtifactParams, SaveExplorationArtifactResult,
    SaveReviewRunParams, SaveReviewRunResult, SaveWorkflowReviewParams, SaveWorkflowReviewResult,
    SearchFilesParams, SearchFilesResult, SearchNodesParams, SearchNodesResult,
    SearchOccurrencesParams, SearchOccurrencesResult, SearchRefsParams, SearchRefsResult,
    SearchTagsParams, SearchTagsResult, SlipboxLinkRewriteApplyParams,
    SlipboxLinkRewriteApplyResult, SlipboxLinkRewritePreviewParams,
    SlipboxLinkRewritePreviewResult, StatusInfo, StructuralWriteReport, UnlinkedReferencesParams,
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
    METHOD_DEMOTE_ENTIRE_FILE, METHOD_DIAGNOSE_FILE, METHOD_DIAGNOSE_INDEX, METHOD_DIAGNOSE_NODE,
    METHOD_DIFF_REVIEW_RUNS, METHOD_ENSURE_FILE_NODE, METHOD_ENSURE_NODE_ID,
    METHOD_EXECUTE_EXPLORATION_ARTIFACT, METHOD_EXPLORATION_ARTIFACT, METHOD_EXPLORE,
    METHOD_EXPORT_WORKBENCH_PACK, METHOD_EXTRACT_SUBTREE, METHOD_FORWARD_LINKS, METHOD_GRAPH_DOT,
    METHOD_IMPORT_WORKBENCH_PACK, METHOD_INDEX, METHOD_INDEX_FILE, METHOD_INDEXED_FILES,
    METHOD_LIST_EXPLORATION_ARTIFACTS, METHOD_LIST_REVIEW_ROUTINES, METHOD_LIST_REVIEW_RUNS,
    METHOD_LIST_WORKBENCH_PACKS, METHOD_LIST_WORKFLOWS, METHOD_MARK_REVIEW_FINDING,
    METHOD_NODE_AT_POINT, METHOD_NODE_FROM_ID, METHOD_NODE_FROM_KEY, METHOD_NODE_FROM_REF,
    METHOD_NODE_FROM_TITLE_OR_ALIAS, METHOD_PING, METHOD_PROMOTE_ENTIRE_FILE, METHOD_RANDOM_NODE,
    METHOD_REFILE_REGION, METHOD_REFILE_SUBTREE, METHOD_REFLINKS,
    METHOD_REVIEW_FINDING_REMEDIATION_APPLY, METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW,
    METHOD_REVIEW_ROUTINE, METHOD_REVIEW_RUN, METHOD_RUN_REVIEW_ROUTINE, METHOD_RUN_WORKFLOW,
    METHOD_SAVE_CORPUS_AUDIT_REVIEW, METHOD_SAVE_EXPLORATION_ARTIFACT, METHOD_SAVE_REVIEW_RUN,
    METHOD_SAVE_WORKFLOW_REVIEW, METHOD_SEARCH_FILES, METHOD_SEARCH_NODES,
    METHOD_SEARCH_OCCURRENCES, METHOD_SEARCH_REFS, METHOD_SEARCH_TAGS,
    METHOD_SLIPBOX_LINK_REWRITE_APPLY, METHOD_SLIPBOX_LINK_REWRITE_PREVIEW, METHOD_STATUS,
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

    fn diagnose_file(
        &mut self,
        params: &FileDiagnosticsParams,
    ) -> Result<FileDiagnosticsResult, DaemonClientError> {
        self.request(METHOD_DIAGNOSE_FILE, params)
    }

    fn diagnose_node(
        &mut self,
        params: &NodeDiagnosticsParams,
    ) -> Result<NodeDiagnosticsResult, DaemonClientError> {
        self.request(METHOD_DIAGNOSE_NODE, params)
    }

    fn diagnose_index(&mut self) -> Result<IndexDiagnosticsResult, DaemonClientError> {
        self.request(METHOD_DIAGNOSE_INDEX, &Value::Null)
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

    fn slipbox_link_rewrite_preview(
        &mut self,
        params: &SlipboxLinkRewritePreviewParams,
    ) -> Result<SlipboxLinkRewritePreviewResult, DaemonClientError> {
        self.request(METHOD_SLIPBOX_LINK_REWRITE_PREVIEW, params)
    }

    fn slipbox_link_rewrite_apply(
        &mut self,
        params: &SlipboxLinkRewriteApplyParams,
    ) -> Result<SlipboxLinkRewriteApplyResult, DaemonClientError> {
        self.request(METHOD_SLIPBOX_LINK_REWRITE_APPLY, params)
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

    pub fn diagnose_file(
        &mut self,
        params: &FileDiagnosticsParams,
    ) -> Result<FileDiagnosticsResult, DaemonClientError> {
        self.rpc.diagnose_file(params)
    }

    pub fn diagnose_node(
        &mut self,
        params: &NodeDiagnosticsParams,
    ) -> Result<NodeDiagnosticsResult, DaemonClientError> {
        self.rpc.diagnose_node(params)
    }

    pub fn diagnose_index(&mut self) -> Result<IndexDiagnosticsResult, DaemonClientError> {
        self.rpc.diagnose_index()
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

    pub fn slipbox_link_rewrite_preview(
        &mut self,
        params: &SlipboxLinkRewritePreviewParams,
    ) -> Result<SlipboxLinkRewritePreviewResult, DaemonClientError> {
        self.rpc.slipbox_link_rewrite_preview(params)
    }

    pub fn slipbox_link_rewrite_apply(
        &mut self,
        params: &SlipboxLinkRewriteApplyParams,
    ) -> Result<SlipboxLinkRewriteApplyResult, DaemonClientError> {
        self.rpc.slipbox_link_rewrite_apply(params)
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
    use slipbox_core::{
        CaptureContentType, CorpusAuditKind, ExplorationArtifactMetadata,
        ExplorationArtifactPayload, ExplorationLens, ReviewFindingStatus, ReviewRun,
        ReviewRunMetadata, ReviewRunPayload, SavedExplorationArtifact, SavedLensViewArtifact,
        SearchNodesSort, WorkbenchPackCompatibility, WorkbenchPackMetadata,
        WorkflowInputAssignment, WorkflowMetadata, WorkflowResolveTarget, WorkflowSummary,
    };

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

    fn error_transport(response_count: u64) -> MockTransport {
        MockTransport {
            requests: Vec::new(),
            responses: (1..=response_count)
                .map(|id| {
                    Ok(JsonRpcResponse::error(
                        json!(id),
                        JsonRpcErrorObject::invalid_request("contract stop".to_owned()),
                    ))
                })
                .collect(),
            shutdowns: 0,
        }
    }

    fn expect_rpc_error<T>(result: Result<T, DaemonClientError>) {
        match result {
            Err(DaemonClientError::Rpc(error)) => {
                assert_eq!(error.code, -32600);
                assert_eq!(error.message, "contract stop");
            }
            Err(other) => panic!("expected queued RPC error, got {other:?}"),
            Ok(_) => panic!("method should surface queued RPC error"),
        }
    }

    fn assert_request(request: &JsonRpcRequest, method: &str, params: Value) {
        assert_eq!(request.method, method);
        assert_eq!(request.params, params);
    }

    fn sample_saved_artifact() -> SavedExplorationArtifact {
        SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "artifact/rpc-contract".to_owned(),
                title: "RPC Contract".to_owned(),
                summary: None,
            },
            payload: ExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "file:alpha.org".to_owned(),
                    current_node_key: "file:alpha.org".to_owned(),
                    lens: ExplorationLens::Structure,
                    limit: 10,
                    unique: false,
                    frozen_context: false,
                }),
            },
        }
    }

    fn sample_review_run() -> ReviewRun {
        ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/rpc-contract".to_owned(),
                title: "RPC Contract Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Workflow {
                workflow: WorkflowSummary {
                    metadata: WorkflowMetadata {
                        workflow_id: "workflow/rpc-contract".to_owned(),
                        title: "RPC Contract Workflow".to_owned(),
                        summary: None,
                    },
                    step_count: 1,
                },
                inputs: Vec::new(),
                step_ids: vec!["resolve".to_owned()],
            },
            findings: Vec::new(),
        }
    }

    fn sample_workbench_pack() -> WorkbenchPackManifest {
        WorkbenchPackManifest {
            metadata: WorkbenchPackMetadata {
                pack_id: "pack/rpc-contract".to_owned(),
                title: "RPC Contract Pack".to_owned(),
                summary: None,
            },
            compatibility: WorkbenchPackCompatibility::default(),
            workflows: Vec::new(),
            review_routines: Vec::new(),
            report_profiles: Vec::new(),
            entrypoint_routine_ids: Vec::new(),
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

    fn slipbox_link_rewrite_preview_response(id: u64) -> JsonRpcResponse {
        JsonRpcResponse::success(
            json!(id),
            json!({
                "preview": {
                    "file_path": "source.org",
                    "rewrites": [{
                        "line": 3,
                        "column": 5,
                        "preview": "See [[slipbox:Target][Target Label]].",
                        "link_text": "[[slipbox:Target][Target Label]]",
                        "title_or_alias": "Target",
                        "description": "Target Label",
                        "target": {
                            "node_key": "file:target.org",
                            "explicit_id": null,
                            "file_path": "target.org",
                            "title": "Target",
                            "outline_path": "Target",
                            "aliases": [],
                            "tags": [],
                            "refs": [],
                            "todo_keyword": null,
                            "scheduled_for": null,
                            "deadline_for": null,
                            "closed_at": null,
                            "level": 1,
                            "line": 1,
                            "kind": "file",
                            "file_mtime_ns": 0,
                            "backlink_count": 0,
                            "forward_link_count": 0
                        },
                        "target_explicit_id": null,
                        "replacement": null
                    }]
                }
            }),
        )
    }

    fn slipbox_link_rewrite_apply_response(id: u64) -> JsonRpcResponse {
        JsonRpcResponse::success(
            json!(id),
            json!({
                "application": {
                    "file_path": "source.org",
                    "rewrites": [{
                        "line": 3,
                        "column": 5,
                        "title_or_alias": "Target",
                        "target_node_key": "file:target.org",
                        "target_explicit_id": "target-id",
                        "replacement": "[[id:target-id][Target Label]]"
                    }],
                    "changed_files": ["source.org", "target.org"],
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
    fn everyday_read_methods_send_canonical_rpc_contracts() {
        let mut client = RpcClient::new(error_transport(24));

        expect_rpc_error(client.status());
        expect_rpc_error(client.index());
        expect_rpc_error(client.index_file(&IndexFileParams {
            file_path: "alpha.org".to_owned(),
        }));
        expect_rpc_error(client.indexed_files());
        expect_rpc_error(client.diagnose_file(&FileDiagnosticsParams {
            file_path: "alpha.org".to_owned(),
        }));
        expect_rpc_error(client.diagnose_node(&NodeDiagnosticsParams {
            node_key: "file:alpha.org".to_owned(),
        }));
        expect_rpc_error(client.diagnose_index());
        expect_rpc_error(client.search_files(&SearchFilesParams {
            query: "alpha".to_owned(),
            limit: 7,
        }));
        expect_rpc_error(client.search_occurrences(&SearchOccurrencesParams {
            query: "needle".to_owned(),
            limit: 8,
        }));
        expect_rpc_error(client.graph_dot(&GraphParams {
            root_node_key: Some("file:alpha.org".to_owned()),
            max_distance: Some(2),
            include_orphans: true,
            hidden_link_types: vec!["ref".to_owned()],
            max_title_length: 42,
            shorten_titles: Some(slipbox_core::GraphTitleShortening::Truncate),
            node_url_prefix: Some("org-protocol://node=".to_owned()),
        }));
        expect_rpc_error(client.random_node());
        expect_rpc_error(client.search_tags(&SearchTagsParams {
            query: "project".to_owned(),
            limit: 9,
        }));
        expect_rpc_error(client.node_from_id(&NodeFromIdParams {
            id: "alpha-id".to_owned(),
        }));
        expect_rpc_error(client.node_from_key(&NodeFromKeyParams {
            node_key: "file:alpha.org".to_owned(),
        }));
        expect_rpc_error(
            client.node_from_title_or_alias(&NodeFromTitleOrAliasParams {
                title_or_alias: "Alpha".to_owned(),
                nocase: true,
            }),
        );
        expect_rpc_error(client.node_from_ref(&NodeFromRefParams {
            reference: "cite:alpha2026".to_owned(),
        }));
        expect_rpc_error(client.node_at_point(&NodeAtPointParams {
            file_path: "alpha.org".to_owned(),
            line: 3,
        }));
        expect_rpc_error(client.anchor_at_point(&NodeAtPointParams {
            file_path: "alpha.org".to_owned(),
            line: 4,
        }));
        expect_rpc_error(client.backlinks(&BacklinksParams {
            node_key: "file:alpha.org".to_owned(),
            limit: 10,
            unique: true,
        }));
        expect_rpc_error(client.forward_links(&ForwardLinksParams {
            node_key: "file:alpha.org".to_owned(),
            limit: 11,
            unique: false,
        }));
        expect_rpc_error(client.reflinks(&ReflinksParams {
            node_key: "file:alpha.org".to_owned(),
            limit: 12,
        }));
        expect_rpc_error(client.unlinked_references(&UnlinkedReferencesParams {
            node_key: "file:alpha.org".to_owned(),
            limit: 13,
        }));
        expect_rpc_error(client.explore(&ExploreParams {
            node_key: "file:alpha.org".to_owned(),
            lens: ExplorationLens::Refs,
            limit: 14,
            unique: false,
        }));
        expect_rpc_error(client.agenda(&AgendaParams {
            start: "2026-05-13T00:00:00".to_owned(),
            end: "2026-05-13T23:59:59".to_owned(),
            limit: 15,
        }));

        assert_eq!(client.transport.requests.len(), 24);
        assert_request(&client.transport.requests[0], METHOD_STATUS, Value::Null);
        assert_request(&client.transport.requests[1], METHOD_INDEX, Value::Null);
        assert_request(
            &client.transport.requests[2],
            METHOD_INDEX_FILE,
            json!({"file_path": "alpha.org"}),
        );
        assert_request(
            &client.transport.requests[3],
            METHOD_INDEXED_FILES,
            Value::Null,
        );
        assert_request(
            &client.transport.requests[4],
            METHOD_DIAGNOSE_FILE,
            json!({"file_path": "alpha.org"}),
        );
        assert_request(
            &client.transport.requests[5],
            METHOD_DIAGNOSE_NODE,
            json!({"node_key": "file:alpha.org"}),
        );
        assert_request(
            &client.transport.requests[6],
            METHOD_DIAGNOSE_INDEX,
            Value::Null,
        );
        assert_request(
            &client.transport.requests[7],
            METHOD_SEARCH_FILES,
            json!({"query": "alpha", "limit": 7}),
        );
        assert_request(
            &client.transport.requests[8],
            METHOD_SEARCH_OCCURRENCES,
            json!({"query": "needle", "limit": 8}),
        );
        assert_request(
            &client.transport.requests[9],
            METHOD_GRAPH_DOT,
            json!({
                "root_node_key": "file:alpha.org",
                "max_distance": 2,
                "include_orphans": true,
                "hidden_link_types": ["ref"],
                "max_title_length": 42,
                "shorten_titles": "truncate",
                "node_url_prefix": "org-protocol://node="
            }),
        );
        assert_request(
            &client.transport.requests[10],
            METHOD_RANDOM_NODE,
            Value::Null,
        );
        assert_request(
            &client.transport.requests[11],
            METHOD_SEARCH_TAGS,
            json!({"query": "project", "limit": 9}),
        );
        assert_request(
            &client.transport.requests[12],
            METHOD_NODE_FROM_ID,
            json!({"id": "alpha-id"}),
        );
        assert_request(
            &client.transport.requests[13],
            METHOD_NODE_FROM_KEY,
            json!({"node_key": "file:alpha.org"}),
        );
        assert_request(
            &client.transport.requests[14],
            METHOD_NODE_FROM_TITLE_OR_ALIAS,
            json!({"title_or_alias": "Alpha", "nocase": true}),
        );
        assert_request(
            &client.transport.requests[15],
            METHOD_NODE_FROM_REF,
            json!({"reference": "cite:alpha2026"}),
        );
        assert_request(
            &client.transport.requests[16],
            METHOD_NODE_AT_POINT,
            json!({"file_path": "alpha.org", "line": 3}),
        );
        assert_request(
            &client.transport.requests[17],
            METHOD_ANCHOR_AT_POINT,
            json!({"file_path": "alpha.org", "line": 4}),
        );
        assert_request(
            &client.transport.requests[18],
            METHOD_BACKLINKS,
            json!({"node_key": "file:alpha.org", "limit": 10, "unique": true}),
        );
        assert_request(
            &client.transport.requests[19],
            METHOD_FORWARD_LINKS,
            json!({"node_key": "file:alpha.org", "limit": 11, "unique": false}),
        );
        assert_request(
            &client.transport.requests[20],
            METHOD_REFLINKS,
            json!({"node_key": "file:alpha.org", "limit": 12}),
        );
        assert_request(
            &client.transport.requests[21],
            METHOD_UNLINKED_REFERENCES,
            json!({"node_key": "file:alpha.org", "limit": 13}),
        );
        assert_request(
            &client.transport.requests[22],
            METHOD_EXPLORE,
            json!({
                "node_key": "file:alpha.org",
                "lens": "refs",
                "limit": 14,
                "unique": false
            }),
        );
        assert_request(
            &client.transport.requests[23],
            METHOD_AGENDA,
            json!({
                "start": "2026-05-13T00:00:00",
                "end": "2026-05-13T23:59:59",
                "limit": 15
            }),
        );
    }

    #[test]
    fn comparison_and_reference_methods_send_canonical_rpc_contracts() {
        let mut client = RpcClient::new(error_transport(2));

        expect_rpc_error(client.search_refs(&SearchRefsParams {
            query: "alpha".to_owned(),
            limit: 16,
        }));
        expect_rpc_error(client.compare_notes(&CompareNotesParams {
            left_node_key: "file:left.org".to_owned(),
            right_node_key: "file:right.org".to_owned(),
            limit: 17,
        }));

        assert_request(
            &client.transport.requests[0],
            METHOD_SEARCH_REFS,
            json!({"query": "alpha", "limit": 16}),
        );
        assert_request(
            &client.transport.requests[1],
            METHOD_COMPARE_NOTES,
            json!({
                "left_node_key": "file:left.org",
                "right_node_key": "file:right.org",
                "limit": 17
            }),
        );
    }

    #[test]
    fn everyday_write_methods_send_canonical_rpc_contracts() {
        let mut client = RpcClient::new(error_transport(9));

        expect_rpc_error(client.capture_node(&CaptureNodeParams {
            title: "Captured".to_owned(),
            file_path: Some("captured.org".to_owned()),
            head: Some("#+title: Captured\n".to_owned()),
            refs: vec!["cite:captured2026".to_owned()],
        }));
        let capture_template = CaptureTemplateParams {
            title: "Template".to_owned(),
            file_path: Some("template.org".to_owned()),
            node_key: None,
            head: None,
            outline_path: vec!["Inbox".to_owned()],
            capture_type: CaptureContentType::Plain,
            content: "Body".to_owned(),
            refs: vec!["cite:template2026".to_owned()],
            prepend: true,
            empty_lines_before: 1,
            empty_lines_after: 2,
            table_line_pos: None,
        };
        expect_rpc_error(client.capture_template(&capture_template));
        expect_rpc_error(
            client.capture_template_preview(&CaptureTemplatePreviewParams {
                capture: capture_template,
                source_override: Some("Source".to_owned()),
                ensure_node_id: true,
            }),
        );
        expect_rpc_error(client.ensure_file_node(&EnsureFileNodeParams {
            file_path: "ensured.org".to_owned(),
            title: "Ensured".to_owned(),
        }));
        expect_rpc_error(client.append_heading(&AppendHeadingParams {
            file_path: "ensured.org".to_owned(),
            title: "Ensured".to_owned(),
            heading: "Child".to_owned(),
            level: 2,
        }));
        expect_rpc_error(client.append_heading_to_node(&AppendHeadingToNodeParams {
            node_key: "file:ensured.org".to_owned(),
            heading: "Grandchild".to_owned(),
        }));
        expect_rpc_error(client.append_heading_at_outline_path(
            &AppendHeadingAtOutlinePathParams {
                file_path: "outline.org".to_owned(),
                heading: "Finding".to_owned(),
                outline_path: vec!["Inbox".to_owned(), "Review".to_owned()],
                head: Some("#+title: Outline\n".to_owned()),
            },
        ));
        expect_rpc_error(client.ensure_node_id(&EnsureNodeIdParams {
            node_key: "heading:outline.org:1".to_owned(),
        }));
        expect_rpc_error(client.update_node_metadata(&UpdateNodeMetadataParams {
            node_key: "file:ensured.org".to_owned(),
            aliases: Some(vec!["Alias".to_owned()]),
            refs: Some(vec!["cite:ensured2026".to_owned()]),
            tags: Some(vec!["tag".to_owned()]),
        }));

        assert_request(
            &client.transport.requests[0],
            METHOD_CAPTURE_NODE,
            json!({
                "title": "Captured",
                "file_path": "captured.org",
                "head": "#+title: Captured\n",
                "refs": ["cite:captured2026"]
            }),
        );
        assert_request(
            &client.transport.requests[1],
            METHOD_CAPTURE_TEMPLATE,
            json!({
                "title": "Template",
                "file_path": "template.org",
                "node_key": null,
                "head": null,
                "outline_path": ["Inbox"],
                "capture_type": "plain",
                "content": "Body",
                "refs": ["cite:template2026"],
                "prepend": true,
                "empty_lines_before": 1,
                "empty_lines_after": 2,
                "table_line_pos": null
            }),
        );
        assert_request(
            &client.transport.requests[2],
            METHOD_CAPTURE_TEMPLATE_PREVIEW,
            json!({
                "title": "Template",
                "file_path": "template.org",
                "node_key": null,
                "head": null,
                "outline_path": ["Inbox"],
                "capture_type": "plain",
                "content": "Body",
                "refs": ["cite:template2026"],
                "prepend": true,
                "empty_lines_before": 1,
                "empty_lines_after": 2,
                "table_line_pos": null,
                "source_override": "Source",
                "ensure_node_id": true
            }),
        );
        assert_request(
            &client.transport.requests[3],
            METHOD_ENSURE_FILE_NODE,
            json!({"file_path": "ensured.org", "title": "Ensured"}),
        );
        assert_request(
            &client.transport.requests[4],
            METHOD_APPEND_HEADING,
            json!({
                "file_path": "ensured.org",
                "title": "Ensured",
                "heading": "Child",
                "level": 2
            }),
        );
        assert_request(
            &client.transport.requests[5],
            METHOD_APPEND_HEADING_TO_NODE,
            json!({"node_key": "file:ensured.org", "heading": "Grandchild"}),
        );
        assert_request(
            &client.transport.requests[6],
            METHOD_APPEND_HEADING_AT_OUTLINE_PATH,
            json!({
                "file_path": "outline.org",
                "heading": "Finding",
                "outline_path": ["Inbox", "Review"],
                "head": "#+title: Outline\n"
            }),
        );
        assert_request(
            &client.transport.requests[7],
            METHOD_ENSURE_NODE_ID,
            json!({"node_key": "heading:outline.org:1"}),
        );
        assert_request(
            &client.transport.requests[8],
            METHOD_UPDATE_NODE_METADATA,
            json!({
                "node_key": "file:ensured.org",
                "aliases": ["Alias"],
                "refs": ["cite:ensured2026"],
                "tags": ["tag"]
            }),
        );
    }

    #[test]
    fn workflow_review_artifact_and_pack_methods_send_canonical_rpc_contracts() {
        let mut client = RpcClient::new(error_transport(27));
        let focus_input = WorkflowInputAssignment {
            input_id: "focus".to_owned(),
            target: WorkflowResolveTarget::NodeKey {
                node_key: "file:alpha.org".to_owned(),
            },
        };
        let saved_artifact = sample_saved_artifact();
        let review = sample_review_run();
        let pack = sample_workbench_pack();

        expect_rpc_error(client.list_workflows());
        expect_rpc_error(client.workflow(&WorkflowIdParams {
            workflow_id: "workflow/rpc-contract".to_owned(),
        }));
        expect_rpc_error(client.run_workflow(&RunWorkflowParams {
            workflow_id: "workflow/rpc-contract".to_owned(),
            inputs: vec![focus_input.clone()],
        }));
        expect_rpc_error(client.list_review_routines());
        expect_rpc_error(client.review_routine(&ReviewRoutineIdParams {
            routine_id: "routine/rpc-contract".to_owned(),
        }));
        expect_rpc_error(client.run_review_routine(&RunReviewRoutineParams {
            routine_id: "routine/rpc-contract".to_owned(),
            inputs: vec![focus_input.clone()],
        }));
        expect_rpc_error(client.corpus_audit(&CorpusAuditParams {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 18,
        }));
        expect_rpc_error(
            client.save_exploration_artifact(&SaveExplorationArtifactParams {
                artifact: saved_artifact,
                overwrite: true,
            }),
        );
        expect_rpc_error(client.exploration_artifact(&ExplorationArtifactIdParams {
            artifact_id: "artifact/rpc-contract".to_owned(),
        }));
        expect_rpc_error(client.list_exploration_artifacts());
        expect_rpc_error(
            client.delete_exploration_artifact(&ExplorationArtifactIdParams {
                artifact_id: "artifact/rpc-contract".to_owned(),
            }),
        );
        expect_rpc_error(
            client.execute_exploration_artifact(&ExplorationArtifactIdParams {
                artifact_id: "artifact/rpc-contract".to_owned(),
            }),
        );
        expect_rpc_error(client.save_review_run(&SaveReviewRunParams {
            review,
            overwrite: true,
        }));
        expect_rpc_error(client.review_run(&ReviewRunIdParams {
            review_id: "review/rpc-contract".to_owned(),
        }));
        expect_rpc_error(client.diff_review_runs(&ReviewRunDiffParams {
            base_review_id: "review/base".to_owned(),
            target_review_id: "review/target".to_owned(),
        }));
        expect_rpc_error(client.review_finding_remediation_preview(
            &ReviewFindingRemediationPreviewParams {
                review_id: "review/rpc-contract".to_owned(),
                finding_id: "finding/rpc-contract".to_owned(),
            },
        ));
        expect_rpc_error(client.list_review_runs());
        expect_rpc_error(client.delete_review_run(&ReviewRunIdParams {
            review_id: "review/rpc-contract".to_owned(),
        }));
        expect_rpc_error(client.mark_review_finding(&MarkReviewFindingParams {
            review_id: "review/rpc-contract".to_owned(),
            finding_id: "finding/rpc-contract".to_owned(),
            status: ReviewFindingStatus::Reviewed,
        }));
        expect_rpc_error(
            client.save_corpus_audit_review(&SaveCorpusAuditReviewParams {
                audit: CorpusAuditKind::DuplicateTitles,
                limit: 19,
                review_id: Some("review/audit/rpc-contract".to_owned()),
                title: Some("Audit Contract".to_owned()),
                summary: None,
                overwrite: true,
            }),
        );
        expect_rpc_error(client.save_workflow_review(&SaveWorkflowReviewParams {
            workflow_id: "workflow/rpc-contract".to_owned(),
            inputs: vec![focus_input],
            review_id: Some("review/workflow/rpc-contract".to_owned()),
            title: Some("Workflow Contract".to_owned()),
            summary: Some("Summary".to_owned()),
            overwrite: false,
        }));
        expect_rpc_error(client.import_workbench_pack(&ImportWorkbenchPackParams {
            pack: pack.clone(),
            overwrite: true,
        }));
        expect_rpc_error(client.workbench_pack(&WorkbenchPackIdParams {
            pack_id: "pack/rpc-contract".to_owned(),
        }));
        expect_rpc_error(client.validate_workbench_pack(&ValidateWorkbenchPackParams { pack }));
        expect_rpc_error(client.export_workbench_pack(&WorkbenchPackIdParams {
            pack_id: "pack/rpc-contract".to_owned(),
        }));
        expect_rpc_error(client.list_workbench_packs());
        expect_rpc_error(client.delete_workbench_pack(&WorkbenchPackIdParams {
            pack_id: "pack/rpc-contract".to_owned(),
        }));

        assert_request(
            &client.transport.requests[0],
            METHOD_LIST_WORKFLOWS,
            json!({}),
        );
        assert_request(
            &client.transport.requests[1],
            METHOD_WORKFLOW,
            json!({"workflow_id": "workflow/rpc-contract"}),
        );
        assert_request(
            &client.transport.requests[2],
            METHOD_RUN_WORKFLOW,
            json!({
                "workflow_id": "workflow/rpc-contract",
                "inputs": [{
                    "input_id": "focus",
                    "kind": "node-key",
                    "node_key": "file:alpha.org"
                }]
            }),
        );
        assert_request(
            &client.transport.requests[3],
            METHOD_LIST_REVIEW_ROUTINES,
            json!({}),
        );
        assert_request(
            &client.transport.requests[4],
            METHOD_REVIEW_ROUTINE,
            json!({"routine_id": "routine/rpc-contract"}),
        );
        assert_request(
            &client.transport.requests[5],
            METHOD_RUN_REVIEW_ROUTINE,
            json!({
                "routine_id": "routine/rpc-contract",
                "inputs": [{
                    "input_id": "focus",
                    "kind": "node-key",
                    "node_key": "file:alpha.org"
                }]
            }),
        );
        assert_request(
            &client.transport.requests[6],
            METHOD_CORPUS_AUDIT,
            json!({"audit": "dangling-links", "limit": 18}),
        );
        assert_eq!(
            client.transport.requests[7].method,
            METHOD_SAVE_EXPLORATION_ARTIFACT
        );
        assert_eq!(client.transport.requests[7].params["overwrite"], true);
        assert_eq!(
            client.transport.requests[7].params["artifact"]["artifact_id"],
            "artifact/rpc-contract"
        );
        assert_request(
            &client.transport.requests[8],
            METHOD_EXPLORATION_ARTIFACT,
            json!({"artifact_id": "artifact/rpc-contract"}),
        );
        assert_request(
            &client.transport.requests[9],
            METHOD_LIST_EXPLORATION_ARTIFACTS,
            json!({}),
        );
        assert_request(
            &client.transport.requests[10],
            METHOD_DELETE_EXPLORATION_ARTIFACT,
            json!({"artifact_id": "artifact/rpc-contract"}),
        );
        assert_request(
            &client.transport.requests[11],
            METHOD_EXECUTE_EXPLORATION_ARTIFACT,
            json!({"artifact_id": "artifact/rpc-contract"}),
        );
        assert_eq!(client.transport.requests[12].method, METHOD_SAVE_REVIEW_RUN);
        assert_eq!(client.transport.requests[12].params["overwrite"], true);
        assert_eq!(
            client.transport.requests[12].params["review"]["review_id"],
            "review/rpc-contract"
        );
        assert_request(
            &client.transport.requests[13],
            METHOD_REVIEW_RUN,
            json!({"review_id": "review/rpc-contract"}),
        );
        assert_request(
            &client.transport.requests[14],
            METHOD_DIFF_REVIEW_RUNS,
            json!({"base_review_id": "review/base", "target_review_id": "review/target"}),
        );
        assert_request(
            &client.transport.requests[15],
            METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW,
            json!({
                "review_id": "review/rpc-contract",
                "finding_id": "finding/rpc-contract"
            }),
        );
        assert_request(
            &client.transport.requests[16],
            METHOD_LIST_REVIEW_RUNS,
            json!({}),
        );
        assert_request(
            &client.transport.requests[17],
            METHOD_DELETE_REVIEW_RUN,
            json!({"review_id": "review/rpc-contract"}),
        );
        assert_request(
            &client.transport.requests[18],
            METHOD_MARK_REVIEW_FINDING,
            json!({
                "review_id": "review/rpc-contract",
                "finding_id": "finding/rpc-contract",
                "status": "reviewed"
            }),
        );
        assert_request(
            &client.transport.requests[19],
            METHOD_SAVE_CORPUS_AUDIT_REVIEW,
            json!({
                "audit": "duplicate-titles",
                "limit": 19,
                "review_id": "review/audit/rpc-contract",
                "title": "Audit Contract",
                "summary": null,
                "overwrite": true
            }),
        );
        assert_request(
            &client.transport.requests[20],
            METHOD_SAVE_WORKFLOW_REVIEW,
            json!({
                "workflow_id": "workflow/rpc-contract",
                "inputs": [{
                    "input_id": "focus",
                    "kind": "node-key",
                    "node_key": "file:alpha.org"
                }],
                "review_id": "review/workflow/rpc-contract",
                "title": "Workflow Contract",
                "summary": "Summary",
                "overwrite": false
            }),
        );
        assert_eq!(
            client.transport.requests[21].method,
            METHOD_IMPORT_WORKBENCH_PACK
        );
        assert_eq!(client.transport.requests[21].params["overwrite"], true);
        assert_eq!(
            client.transport.requests[21].params["pack"]["pack_id"],
            "pack/rpc-contract"
        );
        assert_request(
            &client.transport.requests[22],
            METHOD_WORKBENCH_PACK,
            json!({"pack_id": "pack/rpc-contract"}),
        );
        assert_eq!(
            client.transport.requests[23].method,
            METHOD_VALIDATE_WORKBENCH_PACK
        );
        assert_eq!(
            client.transport.requests[23].params["pack"]["pack_id"],
            "pack/rpc-contract"
        );
        assert_request(
            &client.transport.requests[24],
            METHOD_EXPORT_WORKBENCH_PACK,
            json!({"pack_id": "pack/rpc-contract"}),
        );
        assert_request(
            &client.transport.requests[25],
            METHOD_LIST_WORKBENCH_PACKS,
            json!({}),
        );
        assert_request(
            &client.transport.requests[26],
            METHOD_DELETE_WORKBENCH_PACK,
            json!({"pack_id": "pack/rpc-contract"}),
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
    fn slipbox_link_rewrite_methods_send_canonical_params() {
        let mut client = RpcClient::new(MockTransport {
            requests: Vec::new(),
            responses: VecDeque::from([
                Ok(slipbox_link_rewrite_preview_response(1)),
                Ok(slipbox_link_rewrite_apply_response(2)),
            ]),
            shutdowns: 0,
        });

        let preview_result = client
            .slipbox_link_rewrite_preview(&SlipboxLinkRewritePreviewParams {
                file_path: "source.org".to_owned(),
            })
            .expect("preview result should parse");
        let apply_result = client
            .slipbox_link_rewrite_apply(&SlipboxLinkRewriteApplyParams {
                expected_preview: preview_result.preview.clone(),
            })
            .expect("apply result should parse");

        assert_eq!(preview_result.preview.rewrites.len(), 1);
        assert_eq!(apply_result.application.rewrites.len(), 1);
        assert_eq!(
            client.transport.requests[0].method,
            METHOD_SLIPBOX_LINK_REWRITE_PREVIEW
        );
        assert_eq!(
            client.transport.requests[0].params,
            json!({"file_path": "source.org"})
        );
        assert_eq!(
            client.transport.requests[1].method,
            METHOD_SLIPBOX_LINK_REWRITE_APPLY
        );
        assert_eq!(
            client.transport.requests[1].params,
            json!({
                "expected_preview": {
                    "file_path": "source.org",
                    "rewrites": [{
                        "line": 3,
                        "column": 5,
                        "preview": "See [[slipbox:Target][Target Label]].",
                        "link_text": "[[slipbox:Target][Target Label]]",
                        "title_or_alias": "Target",
                        "description": "Target Label",
                        "target": {
                            "node_key": "file:target.org",
                            "explicit_id": null,
                            "file_path": "target.org",
                            "title": "Target",
                            "outline_path": "Target",
                            "aliases": [],
                            "tags": [],
                            "refs": [],
                            "todo_keyword": null,
                            "scheduled_for": null,
                            "deadline_for": null,
                            "closed_at": null,
                            "level": 1,
                            "line": 1,
                            "kind": "file",
                            "file_mtime_ns": 0,
                            "backlink_count": 0,
                            "forward_link_count": 0
                        },
                        "target_explicit_id": null,
                        "replacement": null
                    }]
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
