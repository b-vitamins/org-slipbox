use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use crate::slipbox_bench::profile::ThresholdConfig;

#[derive(Debug, Serialize)]
pub(crate) struct BenchmarkReport {
    pub(crate) profile: String,
    pub(crate) corpus: CorpusSummary,
    pub(crate) full_index: TimingReport,
    pub(crate) index_file: TimingReport,
    pub(crate) search_nodes: TimingReport,
    pub(crate) search_nodes_sorted: TimingReport,
    pub(crate) search_files: TimingReport,
    pub(crate) search_occurrences: TimingReport,
    pub(crate) backlinks: TimingReport,
    pub(crate) forward_links: TimingReport,
    pub(crate) reflinks: TimingReport,
    pub(crate) unlinked_references: TimingReport,
    pub(crate) node_at_point: TimingReport,
    pub(crate) agenda: TimingReport,
    pub(crate) persistent_buffer: Option<TimingReport>,
    pub(crate) dedicated_buffer: Option<TimingReport>,
    pub(crate) dedicated_exploration_buffer: Option<TimingReport>,
    pub(crate) workflow_catalog: TimingReport,
    pub(crate) workflow_run: TimingReport,
    pub(crate) corpus_audit: TimingReport,
    pub(crate) review_list: TimingReport,
    pub(crate) review_show: TimingReport,
    pub(crate) review_diff: TimingReport,
    pub(crate) review_mark: TimingReport,
    pub(crate) audit_save_review: TimingReport,
    pub(crate) workflow_save_review: TimingReport,
    pub(crate) remediation_preview: TimingReport,
    pub(crate) pack_catalog: TimingReport,
    pub(crate) pack_validation: TimingReport,
    pub(crate) pack_import: TimingReport,
    pub(crate) routine_run: TimingReport,
    pub(crate) report_profile_rendering: TimingReport,
    pub(crate) everyday_file_sync: TimingReport,
    pub(crate) everyday_node_show: TimingReport,
    pub(crate) everyday_node_search: TimingReport,
    pub(crate) everyday_occurrence_search: TimingReport,
    pub(crate) everyday_agenda_range: TimingReport,
    pub(crate) everyday_graph_dot: TimingReport,
    pub(crate) everyday_capture_create: TimingReport,
    pub(crate) everyday_daily_append: TimingReport,
    pub(crate) everyday_metadata_update: TimingReport,
    pub(crate) structural_refile_subtree: TimingReport,
    pub(crate) structural_refile_region: TimingReport,
    pub(crate) structural_extract_subtree: TimingReport,
    pub(crate) structural_promote_file: TimingReport,
    pub(crate) structural_demote_file: TimingReport,
    pub(crate) remediation_apply: TimingReport,
    pub(crate) slipbox_link_rewrite_preview: TimingReport,
    pub(crate) slipbox_link_rewrite_apply: TimingReport,
}

#[derive(Debug, Serialize)]
pub(crate) struct CorpusSummary {
    pub(crate) root: String,
    pub(crate) files: usize,
    pub(crate) headings_per_file: usize,
    pub(crate) expected_nodes: usize,
    pub(crate) expected_links: usize,
    pub(crate) search_queries: usize,
    pub(crate) point_queries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TimingReport {
    pub(crate) samples_ms: Vec<f64>,
    pub(crate) mean_ms: f64,
    pub(crate) median_ms: f64,
    pub(crate) p95_ms: f64,
    pub(crate) max_ms: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ElispTimingReport {
    pub(crate) samples_ms: Vec<f64>,
}

pub(crate) enum BenchWorkspace {
    Temporary(TempDir),
    Persistent(PathBuf),
}

impl BenchWorkspace {
    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::Temporary(tempdir) => tempdir.path(),
            Self::Persistent(path) => path.as_path(),
        }
    }
}

pub(crate) fn enforce_thresholds(
    report: &BenchmarkReport,
    thresholds: &ThresholdConfig,
) -> Result<()> {
    check_threshold(
        "full_index",
        report.full_index.p95_ms,
        thresholds.full_index_p95_ms,
    )?;
    check_threshold(
        "index_file",
        report.index_file.p95_ms,
        thresholds.index_file_p95_ms,
    )?;
    check_threshold(
        "search_nodes",
        report.search_nodes.p95_ms,
        thresholds.search_nodes_p95_ms,
    )?;
    check_threshold(
        "search_nodes_sorted",
        report.search_nodes_sorted.p95_ms,
        thresholds.search_nodes_sorted_p95_ms,
    )?;
    check_threshold(
        "search_files",
        report.search_files.p95_ms,
        thresholds.search_files_p95_ms,
    )?;
    check_threshold(
        "search_occurrences",
        report.search_occurrences.p95_ms,
        thresholds.search_occurrences_p95_ms,
    )?;
    check_threshold(
        "backlinks",
        report.backlinks.p95_ms,
        thresholds.backlinks_p95_ms,
    )?;
    check_threshold(
        "forward_links",
        report.forward_links.p95_ms,
        thresholds.forward_links_p95_ms,
    )?;
    check_threshold(
        "reflinks",
        report.reflinks.p95_ms,
        thresholds.reflinks_p95_ms,
    )?;
    check_threshold(
        "unlinked_references",
        report.unlinked_references.p95_ms,
        thresholds.unlinked_references_p95_ms,
    )?;
    check_threshold(
        "node_at_point",
        report.node_at_point.p95_ms,
        thresholds.node_at_point_p95_ms,
    )?;
    check_threshold("agenda", report.agenda.p95_ms, thresholds.agenda_p95_ms)?;
    if let (Some(observed), Some(limit)) = (
        &report.persistent_buffer,
        thresholds.persistent_buffer_p95_ms,
    ) {
        check_threshold("persistent_buffer", observed.p95_ms, limit)?;
    }
    if let (Some(observed), Some(limit)) =
        (&report.dedicated_buffer, thresholds.dedicated_buffer_p95_ms)
    {
        check_threshold("dedicated_buffer", observed.p95_ms, limit)?;
    }
    if let (Some(observed), Some(limit)) = (
        &report.dedicated_exploration_buffer,
        thresholds.dedicated_exploration_buffer_p95_ms,
    ) {
        check_threshold("dedicated_exploration_buffer", observed.p95_ms, limit)?;
    }
    check_threshold(
        "workflow_catalog",
        report.workflow_catalog.p95_ms,
        thresholds.workflow_catalog_p95_ms,
    )?;
    check_threshold(
        "workflow_run",
        report.workflow_run.p95_ms,
        thresholds.workflow_run_p95_ms,
    )?;
    check_threshold(
        "corpus_audit",
        report.corpus_audit.p95_ms,
        thresholds.corpus_audit_p95_ms,
    )?;
    check_threshold(
        "review_list",
        report.review_list.p95_ms,
        thresholds.review_list_p95_ms,
    )?;
    check_threshold(
        "review_show",
        report.review_show.p95_ms,
        thresholds.review_show_p95_ms,
    )?;
    check_threshold(
        "review_diff",
        report.review_diff.p95_ms,
        thresholds.review_diff_p95_ms,
    )?;
    check_threshold(
        "review_mark",
        report.review_mark.p95_ms,
        thresholds.review_mark_p95_ms,
    )?;
    check_threshold(
        "audit_save_review",
        report.audit_save_review.p95_ms,
        thresholds.audit_save_review_p95_ms,
    )?;
    check_threshold(
        "workflow_save_review",
        report.workflow_save_review.p95_ms,
        thresholds.workflow_save_review_p95_ms,
    )?;
    check_threshold(
        "remediation_preview",
        report.remediation_preview.p95_ms,
        thresholds.remediation_preview_p95_ms,
    )?;
    check_threshold(
        "pack_catalog",
        report.pack_catalog.p95_ms,
        thresholds.pack_catalog_p95_ms,
    )?;
    check_threshold(
        "pack_validation",
        report.pack_validation.p95_ms,
        thresholds.pack_validation_p95_ms,
    )?;
    check_threshold(
        "pack_import",
        report.pack_import.p95_ms,
        thresholds.pack_import_p95_ms,
    )?;
    check_threshold(
        "routine_run",
        report.routine_run.p95_ms,
        thresholds.routine_run_p95_ms,
    )?;
    check_threshold(
        "report_profile_rendering",
        report.report_profile_rendering.p95_ms,
        thresholds.report_profile_rendering_p95_ms,
    )?;
    check_threshold(
        "everyday_file_sync",
        report.everyday_file_sync.p95_ms,
        thresholds.everyday_file_sync_p95_ms,
    )?;
    check_threshold(
        "everyday_node_show",
        report.everyday_node_show.p95_ms,
        thresholds.everyday_node_show_p95_ms,
    )?;
    check_threshold(
        "everyday_node_search",
        report.everyday_node_search.p95_ms,
        thresholds.everyday_node_search_p95_ms,
    )?;
    check_threshold(
        "everyday_occurrence_search",
        report.everyday_occurrence_search.p95_ms,
        thresholds.everyday_occurrence_search_p95_ms,
    )?;
    check_threshold(
        "everyday_agenda_range",
        report.everyday_agenda_range.p95_ms,
        thresholds.everyday_agenda_range_p95_ms,
    )?;
    check_threshold(
        "everyday_graph_dot",
        report.everyday_graph_dot.p95_ms,
        thresholds.everyday_graph_dot_p95_ms,
    )?;
    check_threshold(
        "everyday_capture_create",
        report.everyday_capture_create.p95_ms,
        thresholds.everyday_capture_create_p95_ms,
    )?;
    check_threshold(
        "everyday_daily_append",
        report.everyday_daily_append.p95_ms,
        thresholds.everyday_daily_append_p95_ms,
    )?;
    check_threshold(
        "everyday_metadata_update",
        report.everyday_metadata_update.p95_ms,
        thresholds.everyday_metadata_update_p95_ms,
    )?;
    check_threshold(
        "structural_refile_subtree",
        report.structural_refile_subtree.p95_ms,
        thresholds.structural_refile_subtree_p95_ms,
    )?;
    check_threshold(
        "structural_refile_region",
        report.structural_refile_region.p95_ms,
        thresholds.structural_refile_region_p95_ms,
    )?;
    check_threshold(
        "structural_extract_subtree",
        report.structural_extract_subtree.p95_ms,
        thresholds.structural_extract_subtree_p95_ms,
    )?;
    check_threshold(
        "structural_promote_file",
        report.structural_promote_file.p95_ms,
        thresholds.structural_promote_file_p95_ms,
    )?;
    check_threshold(
        "structural_demote_file",
        report.structural_demote_file.p95_ms,
        thresholds.structural_demote_file_p95_ms,
    )?;
    check_threshold(
        "remediation_apply",
        report.remediation_apply.p95_ms,
        thresholds.remediation_apply_p95_ms,
    )?;
    check_threshold(
        "slipbox_link_rewrite_preview",
        report.slipbox_link_rewrite_preview.p95_ms,
        thresholds.slipbox_link_rewrite_preview_p95_ms,
    )?;
    check_threshold(
        "slipbox_link_rewrite_apply",
        report.slipbox_link_rewrite_apply.p95_ms,
        thresholds.slipbox_link_rewrite_apply_p95_ms,
    )?;
    Ok(())
}

pub(crate) fn check_threshold(metric: &str, observed: f64, limit: f64) -> Result<()> {
    if observed > limit {
        bail!(
            "{metric} p95 {:.2} ms exceeds threshold {:.2} ms",
            observed,
            limit
        );
    }
    Ok(())
}

pub(crate) fn print_summary(report: &BenchmarkReport, check: bool, output_path: &Path) {
    let mode = if check { "check" } else { "run" };
    println!(
        "Benchmark {mode} profile={} corpus_nodes={} corpus_links={} report={}",
        report.profile,
        report.corpus.expected_nodes,
        report.corpus.expected_links,
        output_path.display()
    );
    print_metric("index", &report.full_index);
    print_metric("indexFile", &report.index_file);
    print_metric("searchNodes", &report.search_nodes);
    print_metric("searchNodesSorted", &report.search_nodes_sorted);
    print_metric("searchFiles", &report.search_files);
    print_metric("searchOccurrences", &report.search_occurrences);
    print_metric("backlinks", &report.backlinks);
    print_metric("forwardLinks", &report.forward_links);
    print_metric("reflinks", &report.reflinks);
    print_metric("unlinkedReferences", &report.unlinked_references);
    print_metric("nodeAtPoint", &report.node_at_point);
    print_metric("agenda", &report.agenda);
    if let Some(persistent_buffer) = &report.persistent_buffer {
        print_metric("persistentBuffer", persistent_buffer);
    }
    if let Some(dedicated_buffer) = &report.dedicated_buffer {
        print_metric("dedicatedBuffer", dedicated_buffer);
    }
    if let Some(dedicated_exploration_buffer) = &report.dedicated_exploration_buffer {
        print_metric("dedicatedExplorationBuffer", dedicated_exploration_buffer);
    }
    print_metric("workflowCatalog", &report.workflow_catalog);
    print_metric("workflowRun", &report.workflow_run);
    print_metric("corpusAudit", &report.corpus_audit);
    print_metric("reviewList", &report.review_list);
    print_metric("reviewShow", &report.review_show);
    print_metric("reviewDiff", &report.review_diff);
    print_metric("reviewMark", &report.review_mark);
    print_metric("auditSaveReview", &report.audit_save_review);
    print_metric("workflowSaveReview", &report.workflow_save_review);
    print_metric("remediationPreview", &report.remediation_preview);
    print_metric("packCatalog", &report.pack_catalog);
    print_metric("packValidation", &report.pack_validation);
    print_metric("packImport", &report.pack_import);
    print_metric("routineRun", &report.routine_run);
    print_metric("reportProfileRendering", &report.report_profile_rendering);
    print_metric("everydayFileSync", &report.everyday_file_sync);
    print_metric("everydayNodeShow", &report.everyday_node_show);
    print_metric("everydayNodeSearch", &report.everyday_node_search);
    print_metric(
        "everydayOccurrenceSearch",
        &report.everyday_occurrence_search,
    );
    print_metric("everydayAgendaRange", &report.everyday_agenda_range);
    print_metric("everydayGraphDot", &report.everyday_graph_dot);
    print_metric("everydayCaptureCreate", &report.everyday_capture_create);
    print_metric("everydayDailyAppend", &report.everyday_daily_append);
    print_metric("everydayMetadataUpdate", &report.everyday_metadata_update);
    print_metric("structuralRefileSubtree", &report.structural_refile_subtree);
    print_metric("structuralRefileRegion", &report.structural_refile_region);
    print_metric(
        "structuralExtractSubtree",
        &report.structural_extract_subtree,
    );
    print_metric("structuralPromoteFile", &report.structural_promote_file);
    print_metric("structuralDemoteFile", &report.structural_demote_file);
    print_metric("remediationApply", &report.remediation_apply);
    print_metric(
        "slipboxLinkRewritePreview",
        &report.slipbox_link_rewrite_preview,
    );
    print_metric(
        "slipboxLinkRewriteApply",
        &report.slipbox_link_rewrite_apply,
    );
}

pub(crate) fn print_metric(name: &str, report: &TimingReport) {
    println!(
        "  {:<17} mean={:>8.2} ms median={:>8.2} ms p95={:>8.2} ms max={:>8.2} ms samples={}",
        name,
        report.mean_ms,
        report.median_ms,
        report.p95_ms,
        report.max_ms,
        report.samples_ms.len()
    );
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let encoded = serde_json::to_vec_pretty(value).context("failed to serialize JSON")?;
    fs::write(path, encoded).with_context(|| format!("failed to write {}", path.display()))
}
pub(crate) fn make_persistent_workspace(profile: &str) -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("failed to compute benchmark workspace timestamp")?
        .as_secs();
    let path = manifest_dir()
        .join("target")
        .join("bench")
        .join(format!("{profile}-{timestamp}"));
    if path.exists() {
        fs::remove_dir_all(&path).with_context(|| format!("failed to clear {}", path.display()))?;
    }
    fs::create_dir_all(&path).with_context(|| format!("failed to create {}", path.display()))?;
    Ok(path)
}

pub(crate) fn remove_sqlite_artifacts(path: &Path) -> Result<()> {
    for candidate in [
        path.to_path_buf(),
        path.with_extension("sqlite3-shm"),
        path.with_extension("sqlite3-wal"),
        PathBuf::from(format!("{}-shm", path.display())),
        PathBuf::from(format!("{}-wal", path.display())),
    ] {
        if candidate.exists() {
            fs::remove_file(&candidate)
                .with_context(|| format!("failed to remove {}", candidate.display()))?;
        }
    }
    Ok(())
}

pub(crate) fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

impl TimingReport {
    pub(crate) fn from_samples(mut samples_ms: Vec<f64>) -> Self {
        samples_ms.sort_by(f64::total_cmp);
        let len = samples_ms.len();
        let mean_ms = samples_ms.iter().sum::<f64>() / len as f64;
        let median_ms = percentile(&samples_ms, 0.5);
        let p95_ms = percentile(&samples_ms, 0.95);
        let max_ms = *samples_ms.last().unwrap_or(&0.0);
        Self {
            samples_ms,
            mean_ms,
            median_ms,
            p95_ms,
            max_ms,
        }
    }
}

pub(crate) fn measure_iterations(
    iterations: usize,
    mut f: impl FnMut(usize) -> Result<()>,
) -> Result<TimingReport> {
    let mut samples = Vec::with_capacity(iterations);
    for iteration in 0..iterations {
        let start = Instant::now();
        f(iteration)?;
        samples.push(elapsed_ms(start));
    }
    Ok(TimingReport::from_samples(samples))
}

pub(crate) fn percentile(samples: &[f64], fraction: f64) -> f64 {
    let rank = ((samples.len() as f64) * fraction).ceil() as usize;
    let index = rank.saturating_sub(1).min(samples.len().saturating_sub(1));
    samples[index]
}
pub(crate) fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
