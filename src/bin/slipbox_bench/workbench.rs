use std::path::PathBuf;

use anyhow::{Context, Result};
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

use crate::server::{handlers, state::ServerState};

pub(crate) struct WorkbenchBench {
    state: ServerState,
}

impl WorkbenchBench {
    pub(crate) fn new(
        root: PathBuf,
        db: PathBuf,
        workflow_dirs: Vec<PathBuf>,
        discovery: DiscoveryPolicy,
    ) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
        Ok(Self {
            state: ServerState::new(root, db, workflow_dirs, discovery)?,
        })
    }

    pub(crate) fn list_workflows(&mut self) -> Result<ListWorkflowsResult> {
        let value = handlers::query::list_workflows(&mut self.state, serde_json::json!({}))
            .context("workflow discovery benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode workflow discovery benchmark result")
    }

    pub(crate) fn index_file(&mut self, params: &IndexFileParams) -> Result<IndexFileResult> {
        let value = handlers::query::index_file(&mut self.state, serde_json::to_value(params)?)
            .context("file sync benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode file sync benchmark result")
    }

    pub(crate) fn node_from_id(&mut self, params: &NodeFromIdParams) -> Result<Option<NodeRecord>> {
        let value = handlers::query::node_from_id(&mut self.state, serde_json::to_value(params)?)
            .context("node show benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode node show benchmark result")
    }

    pub(crate) fn search_nodes(&mut self, params: &SearchNodesParams) -> Result<SearchNodesResult> {
        let value = handlers::query::search_nodes(&mut self.state, serde_json::to_value(params)?)
            .context("node search benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode node search benchmark result")
    }

    pub(crate) fn search_occurrences(
        &mut self,
        params: &SearchOccurrencesParams,
    ) -> Result<SearchOccurrencesResult> {
        let value = handlers::query::search_occurrences(&self.state, serde_json::to_value(params)?)
            .context("occurrence search benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode occurrence search benchmark result")
    }

    pub(crate) fn agenda(&mut self, params: &AgendaParams) -> Result<AgendaResult> {
        let value = handlers::query::agenda(&mut self.state, serde_json::to_value(params)?)
            .context("agenda benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode agenda benchmark result")
    }

    pub(crate) fn graph_dot(&mut self, params: &GraphParams) -> Result<GraphResult> {
        let value = handlers::query::graph_dot(&mut self.state, serde_json::to_value(params)?)
            .context("graph DOT benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode graph DOT benchmark result")
    }

    pub(crate) fn capture_node(&mut self, params: &CaptureNodeParams) -> Result<NodeRecord> {
        let value = handlers::write::capture_node(&mut self.state, serde_json::to_value(params)?)
            .context("capture benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode capture benchmark result")
    }

    pub(crate) fn append_heading(&mut self, params: &AppendHeadingParams) -> Result<AnchorRecord> {
        let value = handlers::write::append_heading(&mut self.state, serde_json::to_value(params)?)
            .context("daily append benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode daily append benchmark result")
    }

    pub(crate) fn update_node_metadata(
        &mut self,
        params: &UpdateNodeMetadataParams,
    ) -> Result<NodeRecord> {
        let value =
            handlers::write::update_node_metadata(&mut self.state, serde_json::to_value(params)?)
                .context("metadata update benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode metadata update benchmark result")
    }

    pub(crate) fn run_workflow(&mut self, params: &RunWorkflowParams) -> Result<RunWorkflowResult> {
        let value = handlers::query::run_workflow(&mut self.state, serde_json::to_value(params)?)
            .context("workflow benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode workflow benchmark result")
    }

    pub(crate) fn corpus_audit(&mut self, params: &CorpusAuditParams) -> Result<CorpusAuditResult> {
        let value = handlers::query::corpus_audit(&mut self.state, serde_json::to_value(params)?)
            .context("corpus audit benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode corpus audit benchmark result")
    }

    pub(crate) fn list_review_runs(&mut self) -> Result<ListReviewRunsResult> {
        let value = handlers::query::list_review_runs(&mut self.state, serde_json::json!({}))
            .context("review list benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review list benchmark result")
    }

    pub(crate) fn review_run(&mut self, params: &ReviewRunIdParams) -> Result<ReviewRunResult> {
        let value = handlers::query::review_run(&mut self.state, serde_json::to_value(params)?)
            .context("review show benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review show benchmark result")
    }

    pub(crate) fn diff_review_runs(
        &mut self,
        params: &ReviewRunDiffParams,
    ) -> Result<ReviewRunDiffResult> {
        let value =
            handlers::query::diff_review_runs(&mut self.state, serde_json::to_value(params)?)
                .context("review diff benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review diff benchmark result")
    }

    pub(crate) fn review_finding_remediation_preview(
        &mut self,
        params: &ReviewFindingRemediationPreviewParams,
    ) -> Result<ReviewFindingRemediationPreviewResult> {
        let value = handlers::query::review_finding_remediation_preview(
            &mut self.state,
            serde_json::to_value(params)?,
        )
        .context("review remediation preview benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode review remediation preview benchmark result")
    }

    pub(crate) fn review_finding_remediation_apply(
        &mut self,
        params: &ReviewFindingRemediationApplyParams,
    ) -> Result<ReviewFindingRemediationApplyResult> {
        let value = handlers::query::review_finding_remediation_apply(
            &mut self.state,
            serde_json::to_value(params)?,
        )
        .context("review remediation apply benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode review remediation apply benchmark result")
    }

    pub(crate) fn mark_review_finding(
        &mut self,
        params: &MarkReviewFindingParams,
    ) -> Result<MarkReviewFindingResult> {
        let value =
            handlers::query::mark_review_finding(&mut self.state, serde_json::to_value(params)?)
                .context("review mark benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review mark benchmark result")
    }

    pub(crate) fn save_corpus_audit_review(
        &mut self,
        params: &SaveCorpusAuditReviewParams,
    ) -> Result<SaveCorpusAuditReviewResult> {
        let value = handlers::query::save_corpus_audit_review(
            &mut self.state,
            serde_json::to_value(params)?,
        )
        .context("audit save-review benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode audit save-review benchmark result")
    }

    pub(crate) fn save_workflow_review(
        &mut self,
        params: &SaveWorkflowReviewParams,
    ) -> Result<SaveWorkflowReviewResult> {
        let value =
            handlers::query::save_workflow_review(&mut self.state, serde_json::to_value(params)?)
                .context("workflow save-review benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode workflow save-review benchmark result")
    }

    pub(crate) fn list_review_routines(&mut self) -> Result<ListReviewRoutinesResult> {
        let value = handlers::query::list_review_routines(&mut self.state, serde_json::json!({}))
            .context("review routine catalog benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode review routine catalog benchmark result")
    }

    pub(crate) fn run_review_routine(
        &mut self,
        params: &RunReviewRoutineParams,
    ) -> Result<RunReviewRoutineResult> {
        let value =
            handlers::query::run_review_routine(&mut self.state, serde_json::to_value(params)?)
                .context("review routine benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review routine benchmark result")
    }

    pub(crate) fn import_workbench_pack(
        &mut self,
        params: &ImportWorkbenchPackParams,
    ) -> Result<ImportWorkbenchPackResult> {
        let value =
            handlers::query::import_workbench_pack(&mut self.state, serde_json::to_value(params)?)
                .context("workbench pack import benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode workbench pack import result")
    }

    pub(crate) fn validate_workbench_pack(
        &mut self,
        params: &ValidateWorkbenchPackParams,
    ) -> Result<ValidateWorkbenchPackResult> {
        let value = handlers::query::validate_workbench_pack(
            &mut self.state,
            serde_json::to_value(params)?,
        )
        .context("workbench pack validation benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode workbench pack validation result")
    }

    pub(crate) fn list_workbench_packs(&mut self) -> Result<ListWorkbenchPacksResult> {
        let value = handlers::query::list_workbench_packs(&mut self.state, serde_json::json!({}))
            .context("workbench pack catalog benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode workbench pack catalog result")
    }

    pub(crate) fn refile_subtree(
        &mut self,
        params: &RefileSubtreeParams,
    ) -> Result<StructuralWriteReport> {
        let value = handlers::write::refile_subtree(&mut self.state, serde_json::to_value(params)?)
            .context("refile-subtree benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode refile-subtree benchmark result")
    }

    pub(crate) fn refile_region(
        &mut self,
        params: &RefileRegionParams,
    ) -> Result<StructuralWriteReport> {
        let value = handlers::write::refile_region(&mut self.state, serde_json::to_value(params)?)
            .context("refile-region benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode refile-region benchmark result")
    }

    pub(crate) fn extract_subtree(
        &mut self,
        params: &ExtractSubtreeParams,
    ) -> Result<StructuralWriteReport> {
        let value =
            handlers::write::extract_subtree(&mut self.state, serde_json::to_value(params)?)
                .context("extract-subtree benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode extract-subtree benchmark result")
    }

    pub(crate) fn promote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport> {
        let value =
            handlers::write::promote_entire_file(&mut self.state, serde_json::to_value(params)?)
                .context("promote-file benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode promote-file benchmark result")
    }

    pub(crate) fn demote_entire_file(
        &mut self,
        params: &RewriteFileParams,
    ) -> Result<StructuralWriteReport> {
        let value =
            handlers::write::demote_entire_file(&mut self.state, serde_json::to_value(params)?)
                .context("demote-file benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode demote-file benchmark result")
    }

    pub(crate) fn slipbox_link_rewrite_preview(
        &mut self,
        params: &SlipboxLinkRewritePreviewParams,
    ) -> Result<SlipboxLinkRewritePreviewResult> {
        let value = handlers::write::slipbox_link_rewrite_preview(
            &mut self.state,
            serde_json::to_value(params)?,
        )
        .context("slipbox link rewrite preview benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode slipbox link rewrite preview benchmark result")
    }

    pub(crate) fn slipbox_link_rewrite_apply(
        &mut self,
        params: &SlipboxLinkRewriteApplyParams,
    ) -> Result<SlipboxLinkRewriteApplyResult> {
        let value = handlers::write::slipbox_link_rewrite_apply(
            &mut self.state,
            serde_json::to_value(params)?,
        )
        .context("slipbox link rewrite apply benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode slipbox link rewrite apply benchmark result")
    }
}
