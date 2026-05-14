use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::slipbox_bench::report::manifest_dir;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BenchmarkProfile {
    pub(crate) corpus: CorpusConfig,
    pub(crate) iterations: IterationConfig,
    pub(crate) thresholds: ThresholdConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CorpusConfig {
    pub(crate) files: usize,
    pub(crate) headings_per_file: usize,
    pub(crate) workflow_specs: usize,
    pub(crate) hot_link_stride: usize,
    pub(crate) ref_stride: usize,
    pub(crate) scheduled_stride: usize,
    pub(crate) deadline_stride: usize,
    pub(crate) query_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct IterationConfig {
    pub(crate) full_index: usize,
    pub(crate) index_file: usize,
    pub(crate) search_nodes: usize,
    pub(crate) search_nodes_sorted: usize,
    pub(crate) search_files: usize,
    pub(crate) search_occurrences: usize,
    pub(crate) backlinks: usize,
    pub(crate) forward_links: usize,
    pub(crate) reflinks: usize,
    pub(crate) unlinked_references: usize,
    pub(crate) node_at_point: usize,
    pub(crate) agenda: usize,
    pub(crate) persistent_buffer_samples: usize,
    pub(crate) persistent_buffer_iterations: usize,
    pub(crate) dedicated_buffer_samples: usize,
    pub(crate) dedicated_buffer_iterations: usize,
    pub(crate) dedicated_exploration_buffer_samples: usize,
    pub(crate) dedicated_exploration_buffer_iterations: usize,
    pub(crate) workflow_catalog: usize,
    pub(crate) workflow_run: usize,
    pub(crate) corpus_audit: usize,
    pub(crate) review_list: usize,
    pub(crate) review_show: usize,
    pub(crate) review_diff: usize,
    pub(crate) review_mark: usize,
    pub(crate) audit_save_review: usize,
    pub(crate) workflow_save_review: usize,
    pub(crate) remediation_preview: usize,
    pub(crate) pack_catalog: usize,
    pub(crate) pack_validation: usize,
    pub(crate) pack_import: usize,
    pub(crate) routine_run: usize,
    pub(crate) report_profile_rendering: usize,
    pub(crate) everyday_file_sync: usize,
    pub(crate) everyday_node_show: usize,
    pub(crate) everyday_node_search: usize,
    pub(crate) everyday_occurrence_search: usize,
    pub(crate) everyday_agenda_range: usize,
    pub(crate) everyday_graph_dot: usize,
    pub(crate) everyday_capture_create: usize,
    pub(crate) everyday_daily_append: usize,
    pub(crate) everyday_metadata_update: usize,
    pub(crate) structural_refile_subtree: usize,
    pub(crate) structural_refile_region: usize,
    pub(crate) structural_extract_subtree: usize,
    pub(crate) structural_promote_file: usize,
    pub(crate) structural_demote_file: usize,
    pub(crate) remediation_apply: usize,
    pub(crate) slipbox_link_rewrite_preview: usize,
    pub(crate) slipbox_link_rewrite_apply: usize,
    pub(crate) search_limit: usize,
    pub(crate) backlinks_limit: usize,
    pub(crate) reflinks_limit: usize,
    pub(crate) unlinked_references_limit: usize,
    pub(crate) agenda_limit: usize,
    pub(crate) audit_limit: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ThresholdConfig {
    pub(crate) full_index_p95_ms: f64,
    pub(crate) index_file_p95_ms: f64,
    pub(crate) search_nodes_p95_ms: f64,
    pub(crate) search_nodes_sorted_p95_ms: f64,
    pub(crate) search_files_p95_ms: f64,
    pub(crate) search_occurrences_p95_ms: f64,
    pub(crate) backlinks_p95_ms: f64,
    pub(crate) forward_links_p95_ms: f64,
    pub(crate) reflinks_p95_ms: f64,
    pub(crate) unlinked_references_p95_ms: f64,
    pub(crate) node_at_point_p95_ms: f64,
    pub(crate) agenda_p95_ms: f64,
    pub(crate) persistent_buffer_p95_ms: Option<f64>,
    pub(crate) dedicated_buffer_p95_ms: Option<f64>,
    pub(crate) dedicated_exploration_buffer_p95_ms: Option<f64>,
    pub(crate) workflow_catalog_p95_ms: f64,
    pub(crate) workflow_run_p95_ms: f64,
    pub(crate) corpus_audit_p95_ms: f64,
    pub(crate) review_list_p95_ms: f64,
    pub(crate) review_show_p95_ms: f64,
    pub(crate) review_diff_p95_ms: f64,
    pub(crate) review_mark_p95_ms: f64,
    pub(crate) audit_save_review_p95_ms: f64,
    pub(crate) workflow_save_review_p95_ms: f64,
    pub(crate) remediation_preview_p95_ms: f64,
    pub(crate) pack_catalog_p95_ms: f64,
    pub(crate) pack_validation_p95_ms: f64,
    pub(crate) pack_import_p95_ms: f64,
    pub(crate) routine_run_p95_ms: f64,
    pub(crate) report_profile_rendering_p95_ms: f64,
    pub(crate) everyday_file_sync_p95_ms: f64,
    pub(crate) everyday_node_show_p95_ms: f64,
    pub(crate) everyday_node_search_p95_ms: f64,
    pub(crate) everyday_occurrence_search_p95_ms: f64,
    pub(crate) everyday_agenda_range_p95_ms: f64,
    pub(crate) everyday_graph_dot_p95_ms: f64,
    pub(crate) everyday_capture_create_p95_ms: f64,
    pub(crate) everyday_daily_append_p95_ms: f64,
    pub(crate) everyday_metadata_update_p95_ms: f64,
    pub(crate) structural_refile_subtree_p95_ms: f64,
    pub(crate) structural_refile_region_p95_ms: f64,
    pub(crate) structural_extract_subtree_p95_ms: f64,
    pub(crate) structural_promote_file_p95_ms: f64,
    pub(crate) structural_demote_file_p95_ms: f64,
    pub(crate) remediation_apply_p95_ms: f64,
    pub(crate) slipbox_link_rewrite_preview_p95_ms: f64,
    pub(crate) slipbox_link_rewrite_apply_p95_ms: f64,
}
pub(crate) fn resolve_profile_path(selector: &str) -> Result<PathBuf> {
    let candidate = PathBuf::from(selector);
    if candidate.exists() {
        return Ok(candidate);
    }
    let named = manifest_dir()
        .join("benches")
        .join("profiles")
        .join(format!("{selector}.json"));
    if named.exists() {
        Ok(named)
    } else {
        bail!("unknown benchmark profile: {selector}")
    }
}

pub(crate) fn load_profile(path: &Path) -> Result<BenchmarkProfile> {
    let source =
        fs::read(path).with_context(|| format!("failed to read profile {}", path.display()))?;
    serde_json::from_slice(&source)
        .with_context(|| format!("failed to parse benchmark profile {}", path.display()))
}

pub(crate) fn default_report_path(profile: &str, check: bool) -> PathBuf {
    let mode = if check { "check" } else { "run" };
    manifest_dir()
        .join("target")
        .join("bench")
        .join(format!("{profile}-{mode}.json"))
}
impl BenchmarkProfile {
    pub(crate) fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("files", self.corpus.files),
            ("headings_per_file", self.corpus.headings_per_file),
            ("workflow_specs", self.corpus.workflow_specs),
            ("hot_link_stride", self.corpus.hot_link_stride),
            ("ref_stride", self.corpus.ref_stride),
            ("scheduled_stride", self.corpus.scheduled_stride),
            ("deadline_stride", self.corpus.deadline_stride),
            ("query_count", self.corpus.query_count),
            ("full_index", self.iterations.full_index),
            ("index_file", self.iterations.index_file),
            ("search_nodes", self.iterations.search_nodes),
            ("search_nodes_sorted", self.iterations.search_nodes_sorted),
            ("search_files", self.iterations.search_files),
            ("search_occurrences", self.iterations.search_occurrences),
            ("backlinks", self.iterations.backlinks),
            ("forward_links", self.iterations.forward_links),
            ("reflinks", self.iterations.reflinks),
            ("unlinked_references", self.iterations.unlinked_references),
            ("node_at_point", self.iterations.node_at_point),
            ("agenda", self.iterations.agenda),
            (
                "persistent_buffer_samples",
                self.iterations.persistent_buffer_samples,
            ),
            (
                "persistent_buffer_iterations",
                self.iterations.persistent_buffer_iterations,
            ),
            (
                "dedicated_buffer_samples",
                self.iterations.dedicated_buffer_samples,
            ),
            (
                "dedicated_buffer_iterations",
                self.iterations.dedicated_buffer_iterations,
            ),
            (
                "dedicated_exploration_buffer_samples",
                self.iterations.dedicated_exploration_buffer_samples,
            ),
            (
                "dedicated_exploration_buffer_iterations",
                self.iterations.dedicated_exploration_buffer_iterations,
            ),
            ("workflow_catalog", self.iterations.workflow_catalog),
            ("workflow_run", self.iterations.workflow_run),
            ("corpus_audit", self.iterations.corpus_audit),
            ("review_list", self.iterations.review_list),
            ("review_show", self.iterations.review_show),
            ("review_diff", self.iterations.review_diff),
            ("review_mark", self.iterations.review_mark),
            ("audit_save_review", self.iterations.audit_save_review),
            ("workflow_save_review", self.iterations.workflow_save_review),
            ("remediation_preview", self.iterations.remediation_preview),
            ("pack_catalog", self.iterations.pack_catalog),
            ("pack_validation", self.iterations.pack_validation),
            ("pack_import", self.iterations.pack_import),
            ("routine_run", self.iterations.routine_run),
            (
                "report_profile_rendering",
                self.iterations.report_profile_rendering,
            ),
            ("everyday_file_sync", self.iterations.everyday_file_sync),
            ("everyday_node_show", self.iterations.everyday_node_show),
            ("everyday_node_search", self.iterations.everyday_node_search),
            (
                "everyday_occurrence_search",
                self.iterations.everyday_occurrence_search,
            ),
            (
                "everyday_agenda_range",
                self.iterations.everyday_agenda_range,
            ),
            ("everyday_graph_dot", self.iterations.everyday_graph_dot),
            (
                "everyday_capture_create",
                self.iterations.everyday_capture_create,
            ),
            (
                "everyday_daily_append",
                self.iterations.everyday_daily_append,
            ),
            (
                "everyday_metadata_update",
                self.iterations.everyday_metadata_update,
            ),
            (
                "structural_refile_subtree",
                self.iterations.structural_refile_subtree,
            ),
            (
                "structural_refile_region",
                self.iterations.structural_refile_region,
            ),
            (
                "structural_extract_subtree",
                self.iterations.structural_extract_subtree,
            ),
            (
                "structural_promote_file",
                self.iterations.structural_promote_file,
            ),
            (
                "structural_demote_file",
                self.iterations.structural_demote_file,
            ),
            ("remediation_apply", self.iterations.remediation_apply),
            (
                "slipbox_link_rewrite_preview",
                self.iterations.slipbox_link_rewrite_preview,
            ),
            (
                "slipbox_link_rewrite_apply",
                self.iterations.slipbox_link_rewrite_apply,
            ),
            ("search_limit", self.iterations.search_limit),
            ("backlinks_limit", self.iterations.backlinks_limit),
            ("reflinks_limit", self.iterations.reflinks_limit),
            (
                "unlinked_references_limit",
                self.iterations.unlinked_references_limit,
            ),
            ("agenda_limit", self.iterations.agenda_limit),
            ("audit_limit", self.iterations.audit_limit),
        ] {
            if value == 0 {
                bail!("benchmark profile field {name} must be greater than zero");
            }
        }
        Ok(())
    }
}
