use std::path::PathBuf;

use anyhow::{Context, Result};
use slipbox::service::SlipboxService;
use slipbox_core::{
    AgendaParams, AgendaResult, AnchorRecord, AppendHeadingParams, CaptureNodeParams,
    CorpusAuditParams, CorpusAuditResult, ExtractSubtreeParams, GraphParams, GraphResult,
    ImportWorkbenchPackParams, ImportWorkbenchPackResult, IndexFileParams, IndexFileResult,
    ListReviewRoutinesResult, ListReviewRunsResult, ListWorkbenchPacksResult, ListWorkflowsResult,
    MarkReviewFindingParams, MarkReviewFindingResult, NodeFromIdParams, NodeRecord,
    RefileRegionParams, RefileSubtreeParams, ReviewFindingRemediationApplyParams,
    ReviewFindingRemediationApplyResult, ReviewFindingRemediationPreviewParams,
    ReviewFindingRemediationPreviewResult, ReviewRunDiffParams, ReviewRunDiffResult,
    ReviewRunIdParams, ReviewRunResult, RewriteFileParams, RunReviewRoutineParams,
    RunReviewRoutineResult, RunWorkflowParams, RunWorkflowResult, SaveCorpusAuditReviewParams,
    SaveCorpusAuditReviewResult, SaveWorkflowReviewParams, SaveWorkflowReviewResult,
    SearchNodesParams, SearchNodesResult, SearchOccurrencesParams, SearchOccurrencesResult,
    SlipboxLinkRewriteApplyParams, SlipboxLinkRewriteApplyResult, SlipboxLinkRewritePreviewParams,
    SlipboxLinkRewritePreviewResult, StructuralWriteReport, UpdateNodeMetadataParams,
    ValidateWorkbenchPackParams, ValidateWorkbenchPackResult,
};
use slipbox_index::DiscoveryPolicy;
use slipbox_rpc::{
    METHOD_AGENDA, METHOD_APPEND_HEADING, METHOD_CAPTURE_NODE, METHOD_CORPUS_AUDIT,
    METHOD_DEMOTE_ENTIRE_FILE, METHOD_DIFF_REVIEW_RUNS, METHOD_EXTRACT_SUBTREE, METHOD_GRAPH_DOT,
    METHOD_IMPORT_WORKBENCH_PACK, METHOD_INDEX_FILE, METHOD_LIST_REVIEW_ROUTINES,
    METHOD_LIST_REVIEW_RUNS, METHOD_LIST_WORKBENCH_PACKS, METHOD_LIST_WORKFLOWS,
    METHOD_MARK_REVIEW_FINDING, METHOD_NODE_FROM_ID, METHOD_PROMOTE_ENTIRE_FILE,
    METHOD_REFILE_REGION, METHOD_REFILE_SUBTREE, METHOD_REVIEW_FINDING_REMEDIATION_APPLY,
    METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW, METHOD_REVIEW_RUN, METHOD_RUN_REVIEW_ROUTINE,
    METHOD_RUN_WORKFLOW, METHOD_SAVE_CORPUS_AUDIT_REVIEW, METHOD_SAVE_WORKFLOW_REVIEW,
    METHOD_SEARCH_NODES, METHOD_SEARCH_OCCURRENCES, METHOD_SLIPBOX_LINK_REWRITE_APPLY,
    METHOD_SLIPBOX_LINK_REWRITE_PREVIEW, METHOD_UPDATE_NODE_METADATA,
    METHOD_VALIDATE_WORKBENCH_PACK,
};

pub(crate) struct WorkbenchBench {
    service: SlipboxService,
}

impl WorkbenchBench {
    pub(crate) fn new(
        root: PathBuf,
        db: PathBuf,
        workflow_dirs: Vec<PathBuf>,
        discovery: DiscoveryPolicy,
    ) -> Result<Self> {
        Ok(Self {
            service: SlipboxService::new(root, db, workflow_dirs, discovery)?,
        })
    }

    pub(crate) fn list_workflows(&mut self) -> Result<ListWorkflowsResult> {
        self.service
            .invoke(METHOD_LIST_WORKFLOWS, serde_json::json!({}))
            .context("workflow discovery benchmark request failed")
    }

    pub(crate) fn index_file(&mut self, params: &IndexFileParams) -> Result<IndexFileResult> {
        self.service
            .invoke(METHOD_INDEX_FILE, params)
            .context("file sync benchmark request failed")
    }

    pub(crate) fn node_from_id(&mut self, params: &NodeFromIdParams) -> Result<Option<NodeRecord>> {
        self.service
            .invoke(METHOD_NODE_FROM_ID, params)
            .context("node show benchmark request failed")
    }

    pub(crate) fn search_nodes(&mut self, params: &SearchNodesParams) -> Result<SearchNodesResult> {
        self.service
            .invoke(METHOD_SEARCH_NODES, params)
            .context("node search benchmark request failed")
    }

    pub(crate) fn search_occurrences(
        &mut self,
        params: &SearchOccurrencesParams,
    ) -> Result<SearchOccurrencesResult> {
        self.service
            .invoke(METHOD_SEARCH_OCCURRENCES, params)
            .context("occurrence search benchmark request failed")
    }

    pub(crate) fn agenda(&mut self, params: &AgendaParams) -> Result<AgendaResult> {
        self.service
            .invoke(METHOD_AGENDA, params)
            .context("agenda benchmark request failed")
    }

    pub(crate) fn graph_dot(&mut self, params: &GraphParams) -> Result<GraphResult> {
        self.service
            .invoke(METHOD_GRAPH_DOT, params)
            .context("graph DOT benchmark request failed")
    }

    pub(crate) fn capture_node(&mut self, params: &CaptureNodeParams) -> Result<NodeRecord> {
        self.service
            .invoke(METHOD_CAPTURE_NODE, params)
            .context("capture benchmark request failed")
    }

    pub(crate) fn append_heading(&mut self, params: &AppendHeadingParams) -> Result<AnchorRecord> {
        self.service
            .invoke(METHOD_APPEND_HEADING, params)
            .context("daily append benchmark request failed")
    }

    pub(crate) fn update_node_metadata(
        &mut self,
        params: &UpdateNodeMetadataParams,
    ) -> Result<NodeRecord> {
        self.service
            .invoke(METHOD_UPDATE_NODE_METADATA, params)
            .context("metadata update benchmark request failed")
    }

    pub(crate) fn run_workflow(&mut self, params: &RunWorkflowParams) -> Result<RunWorkflowResult> {
        self.service
            .invoke(METHOD_RUN_WORKFLOW, params)
            .context("workflow benchmark request failed")
    }

    pub(crate) fn corpus_audit(&mut self, params: &CorpusAuditParams) -> Result<CorpusAuditResult> {
        self.service
            .invoke(METHOD_CORPUS_AUDIT, params)
            .context("corpus audit benchmark request failed")
    }

    pub(crate) fn list_review_runs(&mut self) -> Result<ListReviewRunsResult> {
        self.service
            .invoke(METHOD_LIST_REVIEW_RUNS, serde_json::json!({}))
            .context("review list benchmark request failed")
    }

    pub(crate) fn review_run(&mut self, params: &ReviewRunIdParams) -> Result<ReviewRunResult> {
        self.service
            .invoke(METHOD_REVIEW_RUN, params)
            .context("review show benchmark request failed")
    }

    pub(crate) fn diff_review_runs(
        &mut self,
        params: &ReviewRunDiffParams,
    ) -> Result<ReviewRunDiffResult> {
        self.service
            .invoke(METHOD_DIFF_REVIEW_RUNS, params)
            .context("review diff benchmark request failed")
    }

    pub(crate) fn review_finding_remediation_preview(
        &mut self,
        params: &ReviewFindingRemediationPreviewParams,
    ) -> Result<ReviewFindingRemediationPreviewResult> {
        self.service
            .invoke(METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW, params)
            .context("review remediation preview benchmark request failed")
    }

    pub(crate) fn review_finding_remediation_apply(
        &mut self,
        params: &ReviewFindingRemediationApplyParams,
    ) -> Result<ReviewFindingRemediationApplyResult> {
        self.service
            .invoke(METHOD_REVIEW_FINDING_REMEDIATION_APPLY, params)
            .context("review remediation apply benchmark request failed")
    }

    pub(crate) fn mark_review_finding(
        &mut self,
        params: &MarkReviewFindingParams,
    ) -> Result<MarkReviewFindingResult> {
        self.service
            .invoke(METHOD_MARK_REVIEW_FINDING, params)
            .context("review mark benchmark request failed")
    }

    pub(crate) fn save_corpus_audit_review(
        &mut self,
        params: &SaveCorpusAuditReviewParams,
    ) -> Result<SaveCorpusAuditReviewResult> {
        self.service
            .invoke(METHOD_SAVE_CORPUS_AUDIT_REVIEW, params)
            .context("audit save-review benchmark request failed")
    }

    pub(crate) fn save_workflow_review(
        &mut self,
        params: &SaveWorkflowReviewParams,
    ) -> Result<SaveWorkflowReviewResult> {
        self.service
            .invoke(METHOD_SAVE_WORKFLOW_REVIEW, params)
            .context("workflow save-review benchmark request failed")
    }

    pub(crate) fn list_review_routines(&mut self) -> Result<ListReviewRoutinesResult> {
        self.service
            .invoke(METHOD_LIST_REVIEW_ROUTINES, serde_json::json!({}))
            .context("review routine catalog benchmark request failed")
    }

    pub(crate) fn run_review_routine(
        &mut self,
        params: &RunReviewRoutineParams,
    ) -> Result<RunReviewRoutineResult> {
        self.service
            .invoke(METHOD_RUN_REVIEW_ROUTINE, params)
            .context("review routine benchmark request failed")
    }

    pub(crate) fn import_workbench_pack(
        &mut self,
        params: &ImportWorkbenchPackParams,
    ) -> Result<ImportWorkbenchPackResult> {
        self.service
            .invoke(METHOD_IMPORT_WORKBENCH_PACK, params)
            .context("workbench pack import benchmark request failed")
    }

    pub(crate) fn validate_workbench_pack(
        &mut self,
        params: &ValidateWorkbenchPackParams,
    ) -> Result<ValidateWorkbenchPackResult> {
        self.service
            .invoke(METHOD_VALIDATE_WORKBENCH_PACK, params)
            .context("workbench pack validation benchmark request failed")
    }

    pub(crate) fn list_workbench_packs(&mut self) -> Result<ListWorkbenchPacksResult> {
        self.service
            .invoke(METHOD_LIST_WORKBENCH_PACKS, serde_json::json!({}))
            .context("workbench pack catalog benchmark request failed")
    }

    pub(crate) fn refile_subtree(
        &mut self,
        params: &RefileSubtreeParams,
    ) -> Result<StructuralWriteReport> {
        self.service
            .invoke(METHOD_REFILE_SUBTREE, params)
            .context("refile-subtree benchmark request failed")
    }

    pub(crate) fn refile_region(
        &mut self,
        params: &RefileRegionParams,
    ) -> Result<StructuralWriteReport> {
        self.service
            .invoke(METHOD_REFILE_REGION, params)
            .context("refile-region benchmark request failed")
    }

    pub(crate) fn extract_subtree(
        &mut self,
        params: &ExtractSubtreeParams,
    ) -> Result<StructuralWriteReport> {
        self.service
            .invoke(METHOD_EXTRACT_SUBTREE, params)
            .context("extract-subtree benchmark request failed")
    }

    pub(crate) fn promote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport> {
        self.service
            .invoke(METHOD_PROMOTE_ENTIRE_FILE, params)
            .context("promote-file benchmark request failed")
    }

    pub(crate) fn demote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport> {
        self.service
            .invoke(METHOD_DEMOTE_ENTIRE_FILE, params)
            .context("demote-file benchmark request failed")
    }

    pub(crate) fn slipbox_link_rewrite_preview(
        &mut self,
        params: &SlipboxLinkRewritePreviewParams,
    ) -> Result<SlipboxLinkRewritePreviewResult> {
        self.service
            .invoke(METHOD_SLIPBOX_LINK_REWRITE_PREVIEW, params)
            .context("slipbox link rewrite preview benchmark request failed")
    }

    pub(crate) fn slipbox_link_rewrite_apply(
        &mut self,
        params: &SlipboxLinkRewriteApplyParams,
    ) -> Result<SlipboxLinkRewriteApplyResult> {
        self.service
            .invoke(METHOD_SLIPBOX_LINK_REWRITE_APPLY, params)
            .context("slipbox link rewrite apply benchmark request failed")
    }
}
