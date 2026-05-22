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
    JsonRpcRequest, JsonRpcResponse, METHOD_AGENDA, METHOD_ANCHOR_AT_POINT, METHOD_APPEND_HEADING,
    METHOD_APPEND_HEADING_AT_OUTLINE_PATH, METHOD_APPEND_HEADING_TO_NODE, METHOD_BACKLINKS,
    METHOD_CAPTURE_NODE, METHOD_CAPTURE_TEMPLATE, METHOD_CAPTURE_TEMPLATE_PREVIEW,
    METHOD_COMPARE_NOTES, METHOD_CORPUS_AUDIT, METHOD_DELETE_EXPLORATION_ARTIFACT,
    METHOD_DELETE_REVIEW_RUN, METHOD_DELETE_WORKBENCH_PACK, METHOD_DEMOTE_ENTIRE_FILE,
    METHOD_DIAGNOSE_FILE, METHOD_DIAGNOSE_INDEX, METHOD_DIAGNOSE_NODE, METHOD_DIFF_REVIEW_RUNS,
    METHOD_ENSURE_FILE_NODE, METHOD_ENSURE_NODE_ID, METHOD_EXECUTE_EXPLORATION_ARTIFACT,
    METHOD_EXPLORATION_ARTIFACT, METHOD_EXPLORE, METHOD_EXPORT_WORKBENCH_PACK,
    METHOD_EXTRACT_SUBTREE, METHOD_FORWARD_LINKS, METHOD_GRAPH_DOT, METHOD_IMPORT_WORKBENCH_PACK,
    METHOD_INDEX, METHOD_INDEX_FILE, METHOD_INDEXED_FILES, METHOD_LIST_EXPLORATION_ARTIFACTS,
    METHOD_LIST_REVIEW_ROUTINES, METHOD_LIST_REVIEW_RUNS, METHOD_LIST_WORKBENCH_PACKS,
    METHOD_LIST_WORKFLOWS, METHOD_MARK_REVIEW_FINDING, METHOD_NODE_AT_POINT, METHOD_NODE_FROM_ID,
    METHOD_NODE_FROM_KEY, METHOD_NODE_FROM_REF, METHOD_NODE_FROM_TITLE_OR_ALIAS, METHOD_PING,
    METHOD_PROMOTE_ENTIRE_FILE, METHOD_RANDOM_NODE, METHOD_REFILE_REGION, METHOD_REFILE_SUBTREE,
    METHOD_REFLINKS, METHOD_REVIEW_FINDING_REMEDIATION_APPLY,
    METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW, METHOD_REVIEW_ROUTINE, METHOD_REVIEW_RUN,
    METHOD_RUN_REVIEW_ROUTINE, METHOD_RUN_WORKFLOW, METHOD_SAVE_CORPUS_AUDIT_REVIEW,
    METHOD_SAVE_EXPLORATION_ARTIFACT, METHOD_SAVE_REVIEW_RUN, METHOD_SAVE_WORKFLOW_REVIEW,
    METHOD_SEARCH_FILES, METHOD_SEARCH_NODES, METHOD_SEARCH_OCCURRENCES, METHOD_SEARCH_REFS,
    METHOD_SEARCH_TAGS, METHOD_SLIPBOX_LINK_REWRITE_APPLY, METHOD_SLIPBOX_LINK_REWRITE_PREVIEW,
    METHOD_STATUS, METHOD_UNLINKED_REFERENCES, METHOD_UPDATE_NODE_METADATA,
    METHOD_VALIDATE_WORKBENCH_PACK, METHOD_WORKBENCH_PACK, METHOD_WORKFLOW,
};

use crate::error::DaemonClientError;

pub(crate) trait JsonRpcTransport {
    fn round_trip(&mut self, request: JsonRpcRequest)
    -> Result<JsonRpcResponse, DaemonClientError>;
    fn shutdown(&mut self) -> Result<(), DaemonClientError>;
}

pub(crate) struct RpcClient<T> {
    pub(crate) transport: T,
    next_request_id: u64,
}

impl<T> RpcClient<T>
where
    T: JsonRpcTransport,
{
    pub(crate) fn new(transport: T) -> Self {
        Self {
            transport,
            next_request_id: 1,
        }
    }

    pub(crate) fn request<Params, Response>(
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

    pub(crate) fn decode_response<Response>(
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

    pub(crate) fn ping(&mut self) -> Result<PingInfo, DaemonClientError> {
        self.request(METHOD_PING, &Value::Null)
    }

    pub(crate) fn status(&mut self) -> Result<StatusInfo, DaemonClientError> {
        self.request(METHOD_STATUS, &Value::Null)
    }

    pub(crate) fn index(&mut self) -> Result<IndexStats, DaemonClientError> {
        self.request(METHOD_INDEX, &Value::Null)
    }

    pub(crate) fn index_file(
        &mut self,
        params: &IndexFileParams,
    ) -> Result<IndexFileResult, DaemonClientError> {
        self.request(METHOD_INDEX_FILE, params)
    }

    pub(crate) fn indexed_files(&mut self) -> Result<IndexedFilesResult, DaemonClientError> {
        self.request(METHOD_INDEXED_FILES, &Value::Null)
    }

    pub(crate) fn diagnose_file(
        &mut self,
        params: &FileDiagnosticsParams,
    ) -> Result<FileDiagnosticsResult, DaemonClientError> {
        self.request(METHOD_DIAGNOSE_FILE, params)
    }

    pub(crate) fn diagnose_node(
        &mut self,
        params: &NodeDiagnosticsParams,
    ) -> Result<NodeDiagnosticsResult, DaemonClientError> {
        self.request(METHOD_DIAGNOSE_NODE, params)
    }

    pub(crate) fn diagnose_index(&mut self) -> Result<IndexDiagnosticsResult, DaemonClientError> {
        self.request(METHOD_DIAGNOSE_INDEX, &Value::Null)
    }

    pub(crate) fn search_files(
        &mut self,
        params: &SearchFilesParams,
    ) -> Result<SearchFilesResult, DaemonClientError> {
        self.request(METHOD_SEARCH_FILES, params)
    }

    pub(crate) fn search_occurrences(
        &mut self,
        params: &SearchOccurrencesParams,
    ) -> Result<SearchOccurrencesResult, DaemonClientError> {
        self.request(METHOD_SEARCH_OCCURRENCES, params)
    }

    pub(crate) fn graph_dot(
        &mut self,
        params: &GraphParams,
    ) -> Result<GraphResult, DaemonClientError> {
        self.request(METHOD_GRAPH_DOT, params)
    }

    pub(crate) fn search_nodes(
        &mut self,
        params: &SearchNodesParams,
    ) -> Result<SearchNodesResult, DaemonClientError> {
        self.request(METHOD_SEARCH_NODES, params)
    }

    pub(crate) fn random_node(&mut self) -> Result<RandomNodeResult, DaemonClientError> {
        self.request(METHOD_RANDOM_NODE, &Value::Null)
    }

    pub(crate) fn search_tags(
        &mut self,
        params: &SearchTagsParams,
    ) -> Result<SearchTagsResult, DaemonClientError> {
        self.request(METHOD_SEARCH_TAGS, params)
    }

    pub(crate) fn node_from_id(
        &mut self,
        params: &NodeFromIdParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_FROM_ID, params)
    }

    pub(crate) fn node_from_key(
        &mut self,
        params: &NodeFromKeyParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_FROM_KEY, params)
    }

    pub(crate) fn node_from_title_or_alias(
        &mut self,
        params: &NodeFromTitleOrAliasParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_FROM_TITLE_OR_ALIAS, params)
    }

    pub(crate) fn node_from_ref(
        &mut self,
        params: &NodeFromRefParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_FROM_REF, params)
    }

    pub(crate) fn node_at_point(
        &mut self,
        params: &NodeAtPointParams,
    ) -> Result<Option<NodeRecord>, DaemonClientError> {
        self.request(METHOD_NODE_AT_POINT, params)
    }

    pub(crate) fn anchor_at_point(
        &mut self,
        params: &NodeAtPointParams,
    ) -> Result<Option<AnchorRecord>, DaemonClientError> {
        self.request(METHOD_ANCHOR_AT_POINT, params)
    }

    pub(crate) fn backlinks(
        &mut self,
        params: &BacklinksParams,
    ) -> Result<BacklinksResult, DaemonClientError> {
        self.request(METHOD_BACKLINKS, params)
    }

    pub(crate) fn forward_links(
        &mut self,
        params: &ForwardLinksParams,
    ) -> Result<ForwardLinksResult, DaemonClientError> {
        self.request(METHOD_FORWARD_LINKS, params)
    }

    pub(crate) fn reflinks(
        &mut self,
        params: &ReflinksParams,
    ) -> Result<ReflinksResult, DaemonClientError> {
        self.request(METHOD_REFLINKS, params)
    }

    pub(crate) fn unlinked_references(
        &mut self,
        params: &UnlinkedReferencesParams,
    ) -> Result<UnlinkedReferencesResult, DaemonClientError> {
        self.request(METHOD_UNLINKED_REFERENCES, params)
    }

    pub(crate) fn explore(
        &mut self,
        params: &ExploreParams,
    ) -> Result<ExploreResult, DaemonClientError> {
        self.request(METHOD_EXPLORE, params)
    }

    pub(crate) fn agenda(
        &mut self,
        params: &AgendaParams,
    ) -> Result<AgendaResult, DaemonClientError> {
        self.request(METHOD_AGENDA, params)
    }

    pub(crate) fn search_refs(
        &mut self,
        params: &SearchRefsParams,
    ) -> Result<SearchRefsResult, DaemonClientError> {
        self.request(METHOD_SEARCH_REFS, params)
    }

    pub(crate) fn compare_notes(
        &mut self,
        params: &CompareNotesParams,
    ) -> Result<NoteComparisonResult, DaemonClientError> {
        self.request(METHOD_COMPARE_NOTES, params)
    }

    pub(crate) fn capture_node(
        &mut self,
        params: &CaptureNodeParams,
    ) -> Result<NodeRecord, DaemonClientError> {
        self.request(METHOD_CAPTURE_NODE, params)
    }

    pub(crate) fn capture_template(
        &mut self,
        params: &CaptureTemplateParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_CAPTURE_TEMPLATE, params)
    }

    pub(crate) fn capture_template_preview(
        &mut self,
        params: &CaptureTemplatePreviewParams,
    ) -> Result<CaptureTemplatePreviewResult, DaemonClientError> {
        self.request(METHOD_CAPTURE_TEMPLATE_PREVIEW, params)
    }

    pub(crate) fn ensure_file_node(
        &mut self,
        params: &EnsureFileNodeParams,
    ) -> Result<NodeRecord, DaemonClientError> {
        self.request(METHOD_ENSURE_FILE_NODE, params)
    }

    pub(crate) fn append_heading(
        &mut self,
        params: &AppendHeadingParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_APPEND_HEADING, params)
    }

    pub(crate) fn append_heading_to_node(
        &mut self,
        params: &AppendHeadingToNodeParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_APPEND_HEADING_TO_NODE, params)
    }

    pub(crate) fn append_heading_at_outline_path(
        &mut self,
        params: &AppendHeadingAtOutlinePathParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_APPEND_HEADING_AT_OUTLINE_PATH, params)
    }

    pub(crate) fn ensure_node_id(
        &mut self,
        params: &EnsureNodeIdParams,
    ) -> Result<AnchorRecord, DaemonClientError> {
        self.request(METHOD_ENSURE_NODE_ID, params)
    }

    pub(crate) fn update_node_metadata(
        &mut self,
        params: &UpdateNodeMetadataParams,
    ) -> Result<NodeRecord, DaemonClientError> {
        self.request(METHOD_UPDATE_NODE_METADATA, params)
    }

    pub(crate) fn refile_subtree(
        &mut self,
        params: &RefileSubtreeParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_REFILE_SUBTREE, params)
    }

    pub(crate) fn refile_region(
        &mut self,
        params: &RefileRegionParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_REFILE_REGION, params)
    }

    pub(crate) fn extract_subtree(
        &mut self,
        params: &ExtractSubtreeParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_EXTRACT_SUBTREE, params)
    }

    pub(crate) fn promote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_PROMOTE_ENTIRE_FILE, params)
    }

    pub(crate) fn demote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport, DaemonClientError> {
        self.request(METHOD_DEMOTE_ENTIRE_FILE, params)
    }

    pub(crate) fn slipbox_link_rewrite_preview(
        &mut self,
        params: &SlipboxLinkRewritePreviewParams,
    ) -> Result<SlipboxLinkRewritePreviewResult, DaemonClientError> {
        self.request(METHOD_SLIPBOX_LINK_REWRITE_PREVIEW, params)
    }

    pub(crate) fn slipbox_link_rewrite_apply(
        &mut self,
        params: &SlipboxLinkRewriteApplyParams,
    ) -> Result<SlipboxLinkRewriteApplyResult, DaemonClientError> {
        self.request(METHOD_SLIPBOX_LINK_REWRITE_APPLY, params)
    }

    pub(crate) fn list_workflows(&mut self) -> Result<ListWorkflowsResult, DaemonClientError> {
        self.request(METHOD_LIST_WORKFLOWS, &ListWorkflowsParams::default())
    }

    pub(crate) fn workflow(
        &mut self,
        params: &WorkflowIdParams,
    ) -> Result<WorkflowResult, DaemonClientError> {
        self.request(METHOD_WORKFLOW, params)
    }

    pub(crate) fn run_workflow(
        &mut self,
        params: &RunWorkflowParams,
    ) -> Result<RunWorkflowResult, DaemonClientError> {
        self.request(METHOD_RUN_WORKFLOW, params)
    }

    pub(crate) fn list_review_routines(
        &mut self,
    ) -> Result<ListReviewRoutinesResult, DaemonClientError> {
        self.request(
            METHOD_LIST_REVIEW_ROUTINES,
            &ListReviewRoutinesParams::default(),
        )
    }

    pub(crate) fn review_routine(
        &mut self,
        params: &ReviewRoutineIdParams,
    ) -> Result<ReviewRoutineResult, DaemonClientError> {
        self.request(METHOD_REVIEW_ROUTINE, params)
    }

    pub(crate) fn run_review_routine(
        &mut self,
        params: &RunReviewRoutineParams,
    ) -> Result<RunReviewRoutineResult, DaemonClientError> {
        self.request(METHOD_RUN_REVIEW_ROUTINE, params)
    }

    pub(crate) fn corpus_audit(
        &mut self,
        params: &CorpusAuditParams,
    ) -> Result<CorpusAuditResult, DaemonClientError> {
        self.request(METHOD_CORPUS_AUDIT, params)
    }

    pub(crate) fn save_exploration_artifact(
        &mut self,
        params: &SaveExplorationArtifactParams,
    ) -> Result<SaveExplorationArtifactResult, DaemonClientError> {
        self.request(METHOD_SAVE_EXPLORATION_ARTIFACT, params)
    }

    pub(crate) fn exploration_artifact(
        &mut self,
        params: &ExplorationArtifactIdParams,
    ) -> Result<ExplorationArtifactResult, DaemonClientError> {
        self.request(METHOD_EXPLORATION_ARTIFACT, params)
    }

    pub(crate) fn list_exploration_artifacts(
        &mut self,
    ) -> Result<ListExplorationArtifactsResult, DaemonClientError> {
        self.request(
            METHOD_LIST_EXPLORATION_ARTIFACTS,
            &ListExplorationArtifactsParams::default(),
        )
    }

    pub(crate) fn delete_exploration_artifact(
        &mut self,
        params: &ExplorationArtifactIdParams,
    ) -> Result<slipbox_core::DeleteExplorationArtifactResult, DaemonClientError> {
        self.request(METHOD_DELETE_EXPLORATION_ARTIFACT, params)
    }

    pub(crate) fn execute_exploration_artifact(
        &mut self,
        params: &ExplorationArtifactIdParams,
    ) -> Result<ExecuteExplorationArtifactResult, DaemonClientError> {
        self.request(METHOD_EXECUTE_EXPLORATION_ARTIFACT, params)
    }

    pub(crate) fn save_review_run(
        &mut self,
        params: &SaveReviewRunParams,
    ) -> Result<SaveReviewRunResult, DaemonClientError> {
        self.request(METHOD_SAVE_REVIEW_RUN, params)
    }

    pub(crate) fn review_run(
        &mut self,
        params: &ReviewRunIdParams,
    ) -> Result<ReviewRunResult, DaemonClientError> {
        self.request(METHOD_REVIEW_RUN, params)
    }

    pub(crate) fn diff_review_runs(
        &mut self,
        params: &ReviewRunDiffParams,
    ) -> Result<ReviewRunDiffResult, DaemonClientError> {
        self.request(METHOD_DIFF_REVIEW_RUNS, params)
    }

    pub(crate) fn review_finding_remediation_preview(
        &mut self,
        params: &ReviewFindingRemediationPreviewParams,
    ) -> Result<ReviewFindingRemediationPreviewResult, DaemonClientError> {
        self.request(METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW, params)
    }

    pub(crate) fn review_finding_remediation_apply(
        &mut self,
        params: &ReviewFindingRemediationApplyParams,
    ) -> Result<ReviewFindingRemediationApplyResult, DaemonClientError> {
        self.request(METHOD_REVIEW_FINDING_REMEDIATION_APPLY, params)
    }

    pub(crate) fn list_review_runs(&mut self) -> Result<ListReviewRunsResult, DaemonClientError> {
        self.request(METHOD_LIST_REVIEW_RUNS, &ListReviewRunsParams::default())
    }

    pub(crate) fn delete_review_run(
        &mut self,
        params: &ReviewRunIdParams,
    ) -> Result<slipbox_core::DeleteReviewRunResult, DaemonClientError> {
        self.request(METHOD_DELETE_REVIEW_RUN, params)
    }

    pub(crate) fn mark_review_finding(
        &mut self,
        params: &MarkReviewFindingParams,
    ) -> Result<MarkReviewFindingResult, DaemonClientError> {
        self.request(METHOD_MARK_REVIEW_FINDING, params)
    }

    pub(crate) fn save_corpus_audit_review(
        &mut self,
        params: &SaveCorpusAuditReviewParams,
    ) -> Result<SaveCorpusAuditReviewResult, DaemonClientError> {
        self.request(METHOD_SAVE_CORPUS_AUDIT_REVIEW, params)
    }

    pub(crate) fn save_workflow_review(
        &mut self,
        params: &SaveWorkflowReviewParams,
    ) -> Result<SaveWorkflowReviewResult, DaemonClientError> {
        self.request(METHOD_SAVE_WORKFLOW_REVIEW, params)
    }

    pub(crate) fn import_workbench_pack(
        &mut self,
        params: &ImportWorkbenchPackParams,
    ) -> Result<ImportWorkbenchPackResult, DaemonClientError> {
        self.request(METHOD_IMPORT_WORKBENCH_PACK, params)
    }

    pub(crate) fn workbench_pack(
        &mut self,
        params: &WorkbenchPackIdParams,
    ) -> Result<WorkbenchPackResult, DaemonClientError> {
        self.request(METHOD_WORKBENCH_PACK, params)
    }

    pub(crate) fn validate_workbench_pack(
        &mut self,
        params: &ValidateWorkbenchPackParams,
    ) -> Result<ValidateWorkbenchPackResult, DaemonClientError> {
        self.request(METHOD_VALIDATE_WORKBENCH_PACK, params)
    }

    pub(crate) fn export_workbench_pack(
        &mut self,
        params: &WorkbenchPackIdParams,
    ) -> Result<WorkbenchPackManifest, DaemonClientError> {
        self.request(METHOD_EXPORT_WORKBENCH_PACK, params)
    }

    pub(crate) fn list_workbench_packs(
        &mut self,
    ) -> Result<ListWorkbenchPacksResult, DaemonClientError> {
        self.request(
            METHOD_LIST_WORKBENCH_PACKS,
            &ListWorkbenchPacksParams::default(),
        )
    }

    pub(crate) fn delete_workbench_pack(
        &mut self,
        params: &WorkbenchPackIdParams,
    ) -> Result<slipbox_core::DeleteWorkbenchPackResult, DaemonClientError> {
        self.request(METHOD_DELETE_WORKBENCH_PACK, params)
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), DaemonClientError> {
        self.transport.shutdown()
    }
}
