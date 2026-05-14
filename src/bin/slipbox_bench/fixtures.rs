use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use slipbox_core::{
    BacklinkRecord, ExplorationLens, ExploreResult, ExtractSubtreeParams, ForwardLinkRecord,
    NodeRecord, NoteComparisonResult, RefileRegionParams, RefileSubtreeParams,
    ReviewFindingRemediationApplyParams, RewriteFileParams, SlipboxLinkRewriteApplyParams,
    SlipboxLinkRewritePreviewParams, WorkbenchPackManifest,
};

#[derive(Debug)]
pub(crate) struct CorpusFixture {
    pub(crate) root: PathBuf,
    pub(crate) workflow_dirs: Vec<PathBuf>,
    pub(crate) mutable_file: PathBuf,
    pub(crate) mutable_relative_path: String,
    pub(crate) mutable_template: String,
    pub(crate) hot_node_id: String,
    pub(crate) exploration_node_id: String,
    pub(crate) forward_node_id: String,
    pub(crate) workflow_focus_point: PointQuery,
    pub(crate) workflow_specs: usize,
    pub(crate) search_queries: Vec<String>,
    pub(crate) file_queries: Vec<String>,
    pub(crate) point_queries: Vec<PointQuery>,
    pub(crate) expected_files: usize,
    pub(crate) expected_nodes: usize,
    pub(crate) expected_links: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PointQuery {
    pub(crate) file_path: String,
    pub(crate) line: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct BufferFixture<'a> {
    pub(crate) node: &'a NodeRecord,
    pub(crate) backlinks: &'a [BacklinkRecord],
    pub(crate) forward_links: &'a [ForwardLinkRecord],
}

#[derive(Debug, Serialize)]
pub(crate) struct DedicatedBufferFixture<'a> {
    pub(crate) node: &'a NodeRecord,
    pub(crate) compare_target: &'a NodeRecord,
    pub(crate) comparison_result: &'a NoteComparisonResult,
}

#[derive(Debug, Serialize)]
pub(crate) struct DedicatedExplorationBufferFixture<'a> {
    pub(crate) node: &'a NodeRecord,
    pub(crate) lens: ExplorationLens,
    pub(crate) exploration_result: &'a ExploreResult,
}

#[derive(Debug, Clone)]
pub(crate) struct ReviewBenchmarkFixture {
    pub(crate) audit_base_review_id: String,
    pub(crate) audit_target_review_id: String,
    pub(crate) workflow_review_id: String,
    pub(crate) remediation_finding_id: String,
    pub(crate) mark_finding_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct DeclarativeExtensionBenchmarkFixture {
    pub(crate) pack: WorkbenchPackManifest,
    pub(crate) invalid_pack: WorkbenchPackManifest,
    pub(crate) pack_id: String,
    pub(crate) audit_routine_id: String,
    pub(crate) report_routine_id: String,
    pub(crate) report_profile_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct StructuralBenchmarkFixture {
    pub(crate) refile_subtree: Vec<RefileSubtreeParams>,
    pub(crate) refile_region: Vec<RefileRegionParams>,
    pub(crate) extract_subtree: Vec<ExtractSubtreeParams>,
    pub(crate) promote_file: Vec<RewriteFileParams>,
    pub(crate) demote_file: Vec<RewriteFileParams>,
}

#[derive(Debug, Clone)]
pub(crate) struct RemediationApplyBenchmarkFixture {
    pub(crate) apply_params: Vec<ReviewFindingRemediationApplyParams>,
}

#[derive(Debug, Clone)]
pub(crate) struct SlipboxLinkRewriteBenchmarkFixture {
    pub(crate) preview_params: SlipboxLinkRewritePreviewParams,
    pub(crate) apply_params: Vec<SlipboxLinkRewriteApplyParams>,
}
