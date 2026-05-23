use std::path::PathBuf;
use std::process::Child;

use slipbox_core::{
    AgendaParams, AgendaResult, AnchorFromKeyParams, AnchorRecord,
    AppendHeadingAtOutlinePathParams, AppendHeadingParams, AppendHeadingToNodeParams,
    BacklinksParams, BacklinksResult, CaptureNodeParams, CaptureTemplateParams,
    CaptureTemplatePreviewParams, CaptureTemplatePreviewResult, CompareNotesParams,
    CorpusAuditParams, CorpusAuditResult, EnsureFileNodeParams, EnsureNodeIdParams,
    ExecuteExplorationArtifactResult, ExplorationArtifactIdParams, ExplorationArtifactResult,
    ExploreParams, ExploreResult, ExtractSubtreeParams, FileDiagnosticsParams,
    FileDiagnosticsResult, ForwardLinksParams, ForwardLinksResult, GraphParams, GraphResult,
    ImportWorkbenchPackParams, ImportWorkbenchPackResult, IndexDiagnosticsResult, IndexFileParams,
    IndexFileResult, IndexStats, IndexedFilesResult, ListExplorationArtifactsResult,
    ListReviewRoutinesResult, ListReviewRunsResult, ListWorkbenchPacksResult, ListWorkflowsResult,
    MarkReviewFindingParams, MarkReviewFindingResult, NodeAtPointParams, NodeDiagnosticsParams,
    NodeDiagnosticsResult, NodeFromIdParams, NodeFromKeyParams, NodeFromRefParams,
    NodeFromTitleOrAliasParams, NodeRecord, NoteComparisonResult, NoteContextParams,
    NoteContextResult, PingInfo, RandomNodeResult, ReadFileSourceParams, ReadFileSourceResult,
    ReadNodeSourceParams, ReadNodeSourceResult, RefileRegionParams, RefileSubtreeParams,
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
    SearchTagsResult, SlipboxLinkRewriteApplyParams, SlipboxLinkRewriteApplyResult,
    SlipboxLinkRewritePreviewParams, SlipboxLinkRewritePreviewResult, StatusInfo,
    StructuralWriteReport, UnlinkedReferencesParams, UnlinkedReferencesResult,
    UpdateNodeMetadataParams, ValidateWorkbenchPackParams, ValidateWorkbenchPackResult,
    WorkbenchPackIdParams, WorkbenchPackManifest, WorkbenchPackResult, WorkflowIdParams,
    WorkflowResult,
};

use crate::config::DaemonServeConfig;
use crate::error::DaemonClientError;
use crate::rpc::RpcClient;
use crate::transport::StdioTransport;

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

    pub fn anchor_from_key(
        &mut self,
        params: &AnchorFromKeyParams,
    ) -> Result<Option<AnchorRecord>, DaemonClientError> {
        self.rpc.anchor_from_key(params)
    }

    pub fn read_file_source(
        &mut self,
        params: &ReadFileSourceParams,
    ) -> Result<ReadFileSourceResult, DaemonClientError> {
        self.rpc.read_file_source(params)
    }

    pub fn read_node_source(
        &mut self,
        params: &ReadNodeSourceParams,
    ) -> Result<ReadNodeSourceResult, DaemonClientError> {
        self.rpc.read_node_source(params)
    }

    pub fn note_context(
        &mut self,
        params: &NoteContextParams,
    ) -> Result<NoteContextResult, DaemonClientError> {
        self.rpc.note_context(params)
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
