#[path = "../occurrences_query.rs"]
mod occurrences_query;
#[path = "../reflinks_query.rs"]
mod reflinks_query;
#[allow(dead_code)]
#[path = "../server/mod.rs"]
mod server;
#[path = "../text_query.rs"]
mod text_query;
#[path = "../unlinked_references_query.rs"]
mod unlinked_references_query;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use slipbox_core::{
    AuditRemediationPreviewPayload, BacklinkRecord, CompareNotesParams, CorpusAuditKind,
    CorpusAuditParams, ExplorationEntry, ExplorationLens, ExplorationSection,
    ExplorationSectionKind, ExploreResult, ForwardLinkRecord, MarkReviewFindingParams, NodeRecord,
    NoteComparisonResult, ReviewFindingRemediationPreviewParams, ReviewFindingStatus,
    ReviewRunDiffParams, ReviewRunIdParams, RunWorkflowParams, SaveCorpusAuditReviewParams,
    SaveWorkflowReviewParams, SearchNodesSort, WorkflowExecutionResult, WorkflowExploreFocus,
    WorkflowInputAssignment, WorkflowInputKind, WorkflowInputSpec, WorkflowMetadata,
    WorkflowResolveTarget, WorkflowResolveTarget as WorkflowSpecResolveTarget, WorkflowSpec,
    WorkflowSpecCompatibility, WorkflowStepPayload, WorkflowStepReportPayload, WorkflowStepSpec,
};
use slipbox_index::{DiscoveryPolicy, scan_path_with_policy, scan_root_with_policy};
use slipbox_store::Database;
use tempfile::TempDir;

use occurrences_query::query_occurrences;
use reflinks_query::query_reflinks;
use unlinked_references_query::query_unlinked_references;

const HOT_NODE_ID: &str = "node-000000";
const EXPLORATION_FOCUS_INDEX: usize = 2;
const EXPLORATION_SHARED_REF: &str = "cite:bench-explore2026";
const EXPLORATION_FOCUS_REF: &str = "cite:bench-explore-focus2026";
const WORKFLOW_DISCOVERY_DIR: &str = "workflows";
const WORKFLOW_BENCHMARK_ID: &str = "workflow/discovered/benchmark-research-sweep";
const AUDIT_REVIEW_BASE_ID: &str = "review/benchmark/audit/base";
const AUDIT_REVIEW_TARGET_ID: &str = "review/benchmark/audit/target";
const WORKFLOW_REVIEW_ID: &str = "review/benchmark/workflow/base";
const AGENDA_START: &str = "2026-03-01";
const AGENDA_END: &str = "2026-03-31";
const DEDICATED_COMPARE_CANDIDATE_LIMIT: usize = 12;

#[derive(Parser)]
#[command(name = "slipbox-bench")]
#[command(about = "Corpus benchmarks and regression gates for org-slipbox")]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Subcommand)]
enum CommandKind {
    Run(RunArgs),
    Check(RunArgs),
    Generate(GenerateArgs),
}

#[derive(Args, Clone)]
struct RunArgs {
    #[arg(long, default_value = "ci")]
    profile: String,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    keep_corpus: bool,
    #[arg(long)]
    skip_elisp: bool,
}

#[derive(Args)]
struct GenerateArgs {
    #[arg(long, default_value = "ci")]
    profile: String,
    #[arg(long)]
    output_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct BenchmarkProfile {
    corpus: CorpusConfig,
    iterations: IterationConfig,
    thresholds: ThresholdConfig,
}

#[derive(Debug, Clone, Deserialize)]
struct CorpusConfig {
    files: usize,
    headings_per_file: usize,
    workflow_specs: usize,
    hot_link_stride: usize,
    ref_stride: usize,
    scheduled_stride: usize,
    deadline_stride: usize,
    query_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct IterationConfig {
    full_index: usize,
    index_file: usize,
    search_nodes: usize,
    search_nodes_sorted: usize,
    search_files: usize,
    search_occurrences: usize,
    backlinks: usize,
    forward_links: usize,
    reflinks: usize,
    unlinked_references: usize,
    node_at_point: usize,
    agenda: usize,
    persistent_buffer_samples: usize,
    persistent_buffer_iterations: usize,
    dedicated_buffer_samples: usize,
    dedicated_buffer_iterations: usize,
    dedicated_exploration_buffer_samples: usize,
    dedicated_exploration_buffer_iterations: usize,
    workflow_catalog: usize,
    workflow_run: usize,
    corpus_audit: usize,
    review_list: usize,
    review_show: usize,
    review_diff: usize,
    review_mark: usize,
    audit_save_review: usize,
    workflow_save_review: usize,
    remediation_preview: usize,
    search_limit: usize,
    backlinks_limit: usize,
    reflinks_limit: usize,
    unlinked_references_limit: usize,
    agenda_limit: usize,
    audit_limit: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct ThresholdConfig {
    full_index_p95_ms: f64,
    index_file_p95_ms: f64,
    search_nodes_p95_ms: f64,
    search_nodes_sorted_p95_ms: f64,
    search_files_p95_ms: f64,
    search_occurrences_p95_ms: f64,
    backlinks_p95_ms: f64,
    forward_links_p95_ms: f64,
    reflinks_p95_ms: f64,
    unlinked_references_p95_ms: f64,
    node_at_point_p95_ms: f64,
    agenda_p95_ms: f64,
    persistent_buffer_p95_ms: Option<f64>,
    dedicated_buffer_p95_ms: Option<f64>,
    dedicated_exploration_buffer_p95_ms: Option<f64>,
    workflow_catalog_p95_ms: f64,
    workflow_run_p95_ms: f64,
    corpus_audit_p95_ms: f64,
    review_list_p95_ms: f64,
    review_show_p95_ms: f64,
    review_diff_p95_ms: f64,
    review_mark_p95_ms: f64,
    audit_save_review_p95_ms: f64,
    workflow_save_review_p95_ms: f64,
    remediation_preview_p95_ms: f64,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    profile: String,
    corpus: CorpusSummary,
    full_index: TimingReport,
    index_file: TimingReport,
    search_nodes: TimingReport,
    search_nodes_sorted: TimingReport,
    search_files: TimingReport,
    search_occurrences: TimingReport,
    backlinks: TimingReport,
    forward_links: TimingReport,
    reflinks: TimingReport,
    unlinked_references: TimingReport,
    node_at_point: TimingReport,
    agenda: TimingReport,
    persistent_buffer: Option<TimingReport>,
    dedicated_buffer: Option<TimingReport>,
    dedicated_exploration_buffer: Option<TimingReport>,
    workflow_catalog: TimingReport,
    workflow_run: TimingReport,
    corpus_audit: TimingReport,
    review_list: TimingReport,
    review_show: TimingReport,
    review_diff: TimingReport,
    review_mark: TimingReport,
    audit_save_review: TimingReport,
    workflow_save_review: TimingReport,
    remediation_preview: TimingReport,
}

#[derive(Debug, Serialize)]
struct CorpusSummary {
    root: String,
    files: usize,
    headings_per_file: usize,
    expected_nodes: usize,
    expected_links: usize,
    search_queries: usize,
    point_queries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimingReport {
    samples_ms: Vec<f64>,
    mean_ms: f64,
    median_ms: f64,
    p95_ms: f64,
    max_ms: f64,
}

#[derive(Debug)]
struct CorpusFixture {
    root: PathBuf,
    workflow_dirs: Vec<PathBuf>,
    mutable_file: PathBuf,
    mutable_relative_path: String,
    mutable_template: String,
    hot_node_id: String,
    exploration_node_id: String,
    forward_node_id: String,
    workflow_focus_point: PointQuery,
    workflow_specs: usize,
    search_queries: Vec<String>,
    file_queries: Vec<String>,
    point_queries: Vec<PointQuery>,
    expected_files: usize,
    expected_nodes: usize,
    expected_links: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PointQuery {
    file_path: String,
    line: u32,
}

#[derive(Debug, Serialize)]
struct BufferFixture<'a> {
    node: &'a NodeRecord,
    backlinks: &'a [BacklinkRecord],
    forward_links: &'a [ForwardLinkRecord],
}

#[derive(Debug, Serialize)]
struct DedicatedBufferFixture<'a> {
    node: &'a NodeRecord,
    compare_target: &'a NodeRecord,
    comparison_result: &'a NoteComparisonResult,
}

#[derive(Debug, Serialize)]
struct DedicatedExplorationBufferFixture<'a> {
    node: &'a NodeRecord,
    lens: ExplorationLens,
    exploration_result: &'a ExploreResult,
}

#[derive(Debug, Clone)]
struct ReviewBenchmarkFixture {
    audit_base_review_id: String,
    audit_target_review_id: String,
    workflow_review_id: String,
    remediation_finding_id: String,
    mark_finding_id: String,
}

#[derive(Debug, Deserialize)]
struct ElispTimingReport {
    samples_ms: Vec<f64>,
}

enum BenchWorkspace {
    Temporary(TempDir),
    Persistent(PathBuf),
}

impl BenchWorkspace {
    fn path(&self) -> &Path {
        match self {
            Self::Temporary(tempdir) => tempdir.path(),
            Self::Persistent(path) => path.as_path(),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CommandKind::Run(args) => run_command(args, false),
        CommandKind::Check(args) => run_command(args, true),
        CommandKind::Generate(args) => generate_command(args),
    }
}

fn run_command(args: RunArgs, check: bool) -> Result<()> {
    let profile_path = resolve_profile_path(&args.profile)?;
    let profile = load_profile(&profile_path)?;
    profile.validate()?;

    let workspace = if args.keep_corpus {
        BenchWorkspace::Persistent(make_persistent_workspace(&args.profile)?)
    } else {
        BenchWorkspace::Temporary(
            tempfile::tempdir().context("failed to create temporary benchmark workspace")?,
        )
    };

    let fixture = generate_corpus(workspace.path(), &profile.corpus)?;
    let repo_root = manifest_dir();
    let report = run_profile(
        &repo_root,
        &args.profile,
        &profile,
        &fixture,
        args.skip_elisp,
    )?;

    let output_path = args
        .output
        .unwrap_or_else(|| default_report_path(&args.profile, check));
    write_json(&output_path, &report)?;
    print_summary(&report, check, &output_path);

    if check {
        enforce_thresholds(&report, &profile.thresholds)?;
    }

    if args.keep_corpus {
        println!("Corpus kept at {}", fixture.root.display());
    }

    Ok(())
}

fn generate_command(args: GenerateArgs) -> Result<()> {
    let profile_path = resolve_profile_path(&args.profile)?;
    let profile = load_profile(&profile_path)?;
    profile.validate()?;

    if args.output_dir.exists() {
        fs::remove_dir_all(&args.output_dir).with_context(|| {
            format!(
                "failed to clear existing benchmark output directory {}",
                args.output_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&args.output_dir).with_context(|| {
        format!(
            "failed to create benchmark output directory {}",
            args.output_dir.display()
        )
    })?;

    let fixture = generate_corpus(&args.output_dir, &profile.corpus)?;
    let summary = CorpusSummary {
        root: fixture.root.display().to_string(),
        files: fixture.expected_files,
        headings_per_file: profile.corpus.headings_per_file,
        expected_nodes: fixture.expected_nodes,
        expected_links: fixture.expected_links,
        search_queries: fixture.search_queries.len(),
        point_queries: fixture.point_queries.len(),
    };
    write_json(&args.output_dir.join("manifest.json"), &summary)?;
    println!("Generated corpus at {}", fixture.root.display());
    println!(
        "Files: {}, Nodes: {}, Links: {}",
        summary.files, summary.expected_nodes, summary.expected_links
    );
    Ok(())
}

fn run_profile(
    repo_root: &Path,
    profile_name: &str,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
    skip_elisp: bool,
) -> Result<BenchmarkReport> {
    let policy = DiscoveryPolicy::default();

    let full_index = benchmark_full_index(profile, fixture, &policy)?;
    let mut database = prepare_database(fixture, &policy)?;
    let mut workbench = server::WorkbenchBench::new(
        fixture.root.clone(),
        baseline_db_path(fixture),
        fixture.workflow_dirs.clone(),
        policy.clone(),
    )?;
    let hot_node = database
        .node_from_id(&fixture.hot_node_id)?
        .context("failed to resolve hot benchmark node")?;
    let exploration_node = database
        .node_from_id(&fixture.exploration_node_id)?
        .context("failed to resolve dedicated exploration benchmark node")?;
    let forward_node = database
        .node_from_id(&fixture.forward_node_id)?
        .context("failed to resolve forward-link benchmark node")?;

    let search_nodes = benchmark_search_nodes(&mut database, profile, fixture)?;
    let search_nodes_sorted = benchmark_search_nodes_sorted(&mut database, profile, fixture)?;
    let search_files = benchmark_search_files(&mut database, profile, fixture)?;
    let search_occurrences = benchmark_search_occurrences(&mut database, profile, fixture)?;
    let backlinks = benchmark_backlinks(&mut database, profile, &hot_node)?;
    let forward_links = benchmark_forward_links(&mut database, profile, &forward_node)?;
    let reflinks = benchmark_reflinks(&mut database, profile, &fixture.root, &hot_node)?;
    let unlinked_references =
        benchmark_unlinked_references(&mut database, profile, &fixture.root, &hot_node)?;
    let node_at_point = benchmark_node_at_point(&mut database, profile, fixture)?;
    let agenda = benchmark_agenda(&mut database, profile)?;
    let workflow_focus_anchor = database
        .anchor_at_point(
            &fixture.workflow_focus_point.file_path,
            fixture.workflow_focus_point.line,
        )?
        .context("failed to resolve workflow benchmark focus anchor")?;
    let workflow_catalog = benchmark_workflow_catalog(&mut workbench, profile, fixture)?;
    let workflow_run = benchmark_workflow_run(
        &mut workbench,
        profile,
        fixture,
        &workflow_focus_anchor.node_key,
    )?;
    let corpus_audit = benchmark_corpus_audit(&mut workbench, profile)?;
    let review_fixture = prepare_review_benchmark_fixtures(
        &mut workbench,
        profile,
        fixture,
        &workflow_focus_anchor.node_key,
    )?;
    let review_list = benchmark_review_list(&mut workbench, profile, &review_fixture)?;
    let review_show = benchmark_review_show(&mut workbench, profile, &review_fixture)?;
    let review_diff = benchmark_review_diff(&mut workbench, profile, &review_fixture)?;
    let remediation_preview =
        benchmark_remediation_preview(&mut workbench, profile, &review_fixture)?;
    let review_mark = benchmark_review_mark(&mut workbench, profile, &review_fixture)?;
    let audit_save_review = benchmark_audit_save_review(&mut workbench, profile)?;
    let workflow_save_review = benchmark_workflow_save_review(
        &mut workbench,
        profile,
        fixture,
        &workflow_focus_anchor.node_key,
    )?;
    let (persistent_buffer, dedicated_buffer, dedicated_exploration_buffer) = if skip_elisp {
        (None, None, None)
    } else {
        let buffer_backlinks = database.backlinks(
            &hot_node.node_key,
            profile.iterations.backlinks_limit,
            false,
        )?;
        let buffer_forward_links = database.forward_links(
            &hot_node.node_key,
            profile.iterations.backlinks_limit,
            false,
        )?;
        let persistent_buffer = benchmark_persistent_buffer(
            repo_root,
            profile,
            &hot_node,
            &buffer_backlinks,
            &buffer_forward_links,
        )?;
        let (compare_target, comparison_result) = select_dedicated_compare_fixture(
            &database,
            &hot_node,
            &buffer_backlinks,
            &buffer_forward_links,
            profile.iterations.backlinks_limit,
        )?;
        let dedicated_buffer = benchmark_dedicated_buffer(
            repo_root,
            profile,
            &hot_node,
            &compare_target,
            &comparison_result,
        )?;
        let (exploration_lens, exploration_result) = select_dedicated_exploration_fixture(
            &database,
            &exploration_node,
            profile.iterations.backlinks_limit,
        )?;
        let dedicated_exploration_buffer = benchmark_dedicated_exploration_buffer(
            repo_root,
            profile,
            &exploration_node,
            exploration_lens,
            &exploration_result,
        )?;
        (
            Some(persistent_buffer),
            Some(dedicated_buffer),
            Some(dedicated_exploration_buffer),
        )
    };
    let index_file = benchmark_index_file(&mut database, profile, fixture, &policy)?;

    Ok(BenchmarkReport {
        profile: profile_name.to_owned(),
        corpus: CorpusSummary {
            root: fixture.root.display().to_string(),
            files: fixture.expected_files,
            headings_per_file: profile.corpus.headings_per_file,
            expected_nodes: fixture.expected_nodes,
            expected_links: fixture.expected_links,
            search_queries: fixture.search_queries.len(),
            point_queries: fixture.point_queries.len(),
        },
        full_index,
        index_file,
        search_nodes,
        search_nodes_sorted,
        search_files,
        search_occurrences,
        backlinks,
        forward_links,
        reflinks,
        unlinked_references,
        node_at_point,
        agenda,
        persistent_buffer,
        dedicated_buffer,
        dedicated_exploration_buffer,
        workflow_catalog,
        workflow_run,
        corpus_audit,
        review_list,
        review_show,
        review_diff,
        review_mark,
        audit_save_review,
        workflow_save_review,
        remediation_preview,
    })
}

fn benchmark_full_index(
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
    policy: &DiscoveryPolicy,
) -> Result<TimingReport> {
    let mut samples = Vec::with_capacity(profile.iterations.full_index);
    let bench_root = fixture.root.parent().unwrap_or(fixture.root.as_path());
    for iteration in 0..profile.iterations.full_index {
        let db_path = bench_root.join(format!("full-index-{iteration}.sqlite3"));
        remove_sqlite_artifacts(&db_path)?;
        let start = Instant::now();
        let files = scan_root_with_policy(&fixture.root, policy)
            .context("failed to scan benchmark corpus")?;
        let mut database = Database::open(&db_path)
            .with_context(|| format!("failed to open benchmark database {}", db_path.display()))?;
        database
            .sync_index(&files)
            .context("failed to index benchmark corpus")?;
        assert_expected_counts(&database, fixture)?;
        samples.push(elapsed_ms(start));
    }
    Ok(TimingReport::from_samples(samples))
}

fn prepare_database(fixture: &CorpusFixture, policy: &DiscoveryPolicy) -> Result<Database> {
    let db_path = baseline_db_path(fixture);
    remove_sqlite_artifacts(&db_path)?;
    let files =
        scan_root_with_policy(&fixture.root, policy).context("failed to scan benchmark corpus")?;
    let mut database = Database::open(&db_path)
        .with_context(|| format!("failed to open baseline database {}", db_path.display()))?;
    database
        .sync_index(&files)
        .context("failed to index baseline benchmark corpus")?;
    assert_expected_counts(&database, fixture)?;
    Ok(database)
}

fn baseline_db_path(fixture: &CorpusFixture) -> PathBuf {
    fixture
        .root
        .parent()
        .unwrap_or(fixture.root.as_path())
        .join("baseline.sqlite3")
}

fn benchmark_search_nodes(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.search_nodes, |iteration| {
        let query = &fixture.search_queries[iteration % fixture.search_queries.len()];
        let nodes = database
            .search_nodes(query, profile.iterations.search_limit, None)
            .with_context(|| format!("failed to search nodes for query {query}"))?;
        if nodes.is_empty() {
            bail!("benchmark search query {query} returned no nodes");
        }
        black_box(nodes.len());
        Ok(())
    })
}

fn benchmark_search_nodes_sorted(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    const SORTS: [SearchNodesSort; 5] = [
        SearchNodesSort::Title,
        SearchNodesSort::File,
        SearchNodesSort::FileMtime,
        SearchNodesSort::BacklinkCount,
        SearchNodesSort::ForwardLinkCount,
    ];

    measure_iterations(profile.iterations.search_nodes_sorted, |iteration| {
        let query = &fixture.search_queries[iteration % fixture.search_queries.len()];
        let sort = SORTS[iteration % SORTS.len()].clone();
        let nodes = database
            .search_nodes(query, profile.iterations.search_limit, Some(sort.clone()))
            .with_context(|| {
                format!("failed to search nodes for query {query} with sort {sort:?}")
            })?;
        if nodes.is_empty() {
            bail!("benchmark sorted search query {query} with sort {sort:?} returned no nodes");
        }
        black_box(nodes.len());
        Ok(())
    })
}

fn benchmark_backlinks(
    database: &mut Database,
    profile: &BenchmarkProfile,
    hot_node: &NodeRecord,
) -> Result<TimingReport> {
    let sample = database
        .backlinks(
            &hot_node.node_key,
            profile.iterations.backlinks_limit,
            false,
        )
        .context("failed to fetch backlink sample")?;
    if sample.is_empty() {
        bail!("benchmark hot node produced no backlinks");
    }
    measure_iterations(profile.iterations.backlinks, |_| {
        let backlinks = database
            .backlinks(
                &hot_node.node_key,
                profile.iterations.backlinks_limit,
                false,
            )
            .context("failed to query backlinks")?;
        black_box(backlinks.len());
        Ok(())
    })
}

fn benchmark_search_files(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.search_files, |iteration| {
        let query = &fixture.file_queries[iteration % fixture.file_queries.len()];
        let files = database
            .search_files(query, profile.iterations.search_limit)
            .with_context(|| format!("failed to search files for query {query}"))?;
        if files.is_empty() {
            bail!("benchmark file search query {query} returned no files");
        }
        black_box(files.len());
        Ok(())
    })
}

fn benchmark_search_occurrences(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    let trace_occurrences = env::var_os("SLIPBOX_BENCH_TRACE_OCCURRENCES").is_some();
    measure_iterations(profile.iterations.search_occurrences, |iteration| {
        let query = &fixture.search_queries[iteration % fixture.search_queries.len()];
        let start = Instant::now();
        let occurrences = query_occurrences(database, query, profile.iterations.search_limit)
            .with_context(|| format!("failed to search occurrences for query {query}"))?;
        if occurrences.is_empty() {
            bail!("benchmark occurrence query {query} returned no hits");
        }
        if trace_occurrences {
            eprintln!(
                "searchOccurrences query={query:?} elapsed_ms={:.2} hits={}",
                elapsed_ms(start),
                occurrences.len()
            );
        }
        black_box(occurrences.len());
        Ok(())
    })
}

fn benchmark_forward_links(
    database: &mut Database,
    profile: &BenchmarkProfile,
    source_node: &NodeRecord,
) -> Result<TimingReport> {
    let sample = database
        .forward_links(
            &source_node.node_key,
            profile.iterations.backlinks_limit,
            false,
        )
        .context("failed to fetch forward-link sample")?;
    if sample.is_empty() {
        bail!("benchmark source node produced no forward links");
    }
    measure_iterations(profile.iterations.forward_links, |_| {
        let forward_links = database
            .forward_links(
                &source_node.node_key,
                profile.iterations.backlinks_limit,
                false,
            )
            .context("failed to query forward links")?;
        black_box(forward_links.len());
        Ok(())
    })
}

fn benchmark_reflinks(
    database: &mut Database,
    profile: &BenchmarkProfile,
    root: &Path,
    source_node: &NodeRecord,
) -> Result<TimingReport> {
    let source_anchor = source_node.clone().into();
    let sample = query_reflinks(
        database,
        root,
        &source_anchor,
        profile.iterations.reflinks_limit,
    )
    .context("failed to fetch reflink sample")?;
    if sample.is_empty() {
        bail!("benchmark source node produced no reflinks");
    }
    measure_iterations(profile.iterations.reflinks, |_| {
        let reflinks = query_reflinks(
            database,
            root,
            &source_anchor,
            profile.iterations.reflinks_limit,
        )
        .context("failed to query reflinks")?;
        black_box(reflinks.len());
        Ok(())
    })
}

fn benchmark_unlinked_references(
    database: &mut Database,
    profile: &BenchmarkProfile,
    root: &Path,
    node: &NodeRecord,
) -> Result<TimingReport> {
    let node_anchor = node.clone().into();
    let sample = query_unlinked_references(
        database,
        root,
        &node_anchor,
        profile.iterations.unlinked_references_limit,
    )
    .context("failed to query unlinked references")?;
    if sample.is_empty() {
        bail!("benchmark hot node produced no unlinked references");
    }

    measure_iterations(profile.iterations.unlinked_references, |_| {
        let unlinked_references = query_unlinked_references(
            database,
            root,
            &node_anchor,
            profile.iterations.unlinked_references_limit,
        )
        .context("failed to query unlinked references")?;
        black_box(unlinked_references.len());
        Ok(())
    })
}

fn benchmark_node_at_point(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.node_at_point, |iteration| {
        let point = &fixture.point_queries[iteration % fixture.point_queries.len()];
        let node = database
            .node_at_point(&point.file_path, point.line)
            .with_context(|| {
                format!(
                    "failed to query node at point {}:{}",
                    point.file_path, point.line
                )
            })?;
        if node.is_none() {
            bail!(
                "benchmark node-at-point query {}:{} returned no node",
                point.file_path,
                point.line
            );
        }
        black_box(node);
        Ok(())
    })
}

fn benchmark_agenda(database: &mut Database, profile: &BenchmarkProfile) -> Result<TimingReport> {
    let sample = database
        .agenda_nodes(AGENDA_START, AGENDA_END, profile.iterations.agenda_limit)
        .context("failed to fetch agenda sample")?;
    if sample.is_empty() {
        bail!("benchmark agenda query returned no nodes");
    }
    measure_iterations(profile.iterations.agenda, |_| {
        let nodes = database
            .agenda_nodes(AGENDA_START, AGENDA_END, profile.iterations.agenda_limit)
            .context("failed to query agenda nodes")?;
        black_box(nodes.len());
        Ok(())
    })
}

fn benchmark_index_file(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
    policy: &DiscoveryPolicy,
) -> Result<TimingReport> {
    let mut samples = Vec::with_capacity(profile.iterations.index_file);
    for iteration in 0..profile.iterations.index_file {
        let source = fixture
            .mutable_template
            .replace("__BENCH_MUTABLE__", &format!("iteration-{iteration:04}"));
        fs::write(&fixture.mutable_file, source).with_context(|| {
            format!(
                "failed to write mutable benchmark file {}",
                fixture.mutable_file.display()
            )
        })?;
        let start = Instant::now();
        let indexed = scan_path_with_policy(&fixture.root, &fixture.mutable_file, policy)
            .with_context(|| {
                format!(
                    "failed to scan mutable benchmark file {}",
                    fixture.mutable_file.display()
                )
            })?;
        database
            .sync_file_index(&indexed)
            .context("failed to sync mutable benchmark file")?;
        samples.push(elapsed_ms(start));
    }

    fs::write(&fixture.mutable_file, &fixture.mutable_template).with_context(|| {
        format!(
            "failed to restore mutable benchmark file {}",
            fixture.mutable_file.display()
        )
    })?;
    let indexed = scan_path_with_policy(&fixture.root, &fixture.mutable_file, policy)
        .with_context(|| {
            format!(
                "failed to rescan mutable benchmark file {}",
                fixture.mutable_file.display()
            )
        })?;
    database
        .sync_file_index(&indexed)
        .context("failed to restore mutable benchmark file in index")?;

    let node = database
        .node_at_point(&fixture.mutable_relative_path, 7)
        .context("failed to verify mutable file after incremental index")?;
    if node.is_none() {
        bail!("mutable benchmark file no longer resolves a node after incremental sync");
    }

    Ok(TimingReport::from_samples(samples))
}

fn benchmark_persistent_buffer(
    repo_root: &Path,
    profile: &BenchmarkProfile,
    node: &NodeRecord,
    backlinks: &[BacklinkRecord],
    forward_links: &[ForwardLinkRecord],
) -> Result<TimingReport> {
    let fixture = BufferFixture {
        node,
        backlinks,
        forward_links,
    };
    let fixture_file = repo_root
        .join("target")
        .join("bench")
        .join("persistent-buffer-fixture.json");
    if let Some(parent) = fixture_file.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create persistent buffer fixture directory {}",
                parent.display()
            )
        })?;
    }
    write_json(&fixture_file, &fixture)?;

    let emacs = std::env::var("EMACS").unwrap_or_else(|_| "emacs".to_owned());
    let eval = format!(
        "(princ (org-slipbox-buffer-bench-run-file {:?} {} {}))",
        fixture_file.to_string_lossy(),
        profile.iterations.persistent_buffer_samples,
        profile.iterations.persistent_buffer_iterations
    );
    let output = Command::new(&emacs)
        .current_dir(repo_root)
        .arg("-Q")
        .arg("--batch")
        .arg("-L")
        .arg(".")
        .arg("-l")
        .arg("org-slipbox.el")
        .arg("-l")
        .arg("benches/org-slipbox-buffer-bench.el")
        .arg("--eval")
        .arg(eval)
        .output()
        .with_context(|| format!("failed to execute {emacs} for persistent buffer benchmark"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("{emacs} exited with {}", output.status)
        } else {
            stderr
        };
        bail!("persistent buffer benchmark failed: {message}");
    }

    let report: ElispTimingReport = serde_json::from_slice(&output.stdout)
        .context("failed to parse persistent buffer report")?;
    if report.samples_ms.is_empty() {
        bail!("persistent buffer benchmark produced no samples");
    }
    Ok(TimingReport::from_samples(report.samples_ms))
}

fn select_dedicated_compare_fixture(
    database: &Database,
    node: &NodeRecord,
    backlinks: &[BacklinkRecord],
    forward_links: &[ForwardLinkRecord],
    limit: usize,
) -> Result<(NodeRecord, NoteComparisonResult)> {
    let params = |left: &NodeRecord, right: &NodeRecord| CompareNotesParams {
        left_node_key: left.node_key.clone(),
        right_node_key: right.node_key.clone(),
        limit,
    };

    let mut seen = BTreeSet::new();
    let candidates = forward_links
        .iter()
        .map(|record| record.destination_note.clone())
        .chain(backlinks.iter().map(|record| record.source_note.clone()))
        .filter(|candidate| {
            candidate.node_key != node.node_key && seen.insert(candidate.node_key.clone())
        })
        .take(DEDICATED_COMPARE_CANDIDATE_LIMIT)
        .collect::<Vec<_>>();

    let mut best = None;
    for candidate in candidates {
        let comparison = database.compare_notes(node, &candidate, &params(node, &candidate))?;
        let score = comparison
            .sections
            .iter()
            .map(|section| section.entries.len())
            .sum::<usize>();
        if best
            .as_ref()
            .is_none_or(|(_, _, best_score)| score > *best_score)
        {
            best = Some((candidate, comparison, score));
        }
    }

    if let Some((candidate, comparison, _score)) = best {
        Ok((candidate, comparison))
    } else {
        let comparison = database.compare_notes(node, node, &params(node, node))?;
        Ok((node.clone(), comparison))
    }
}

fn benchmark_dedicated_buffer(
    repo_root: &Path,
    profile: &BenchmarkProfile,
    node: &NodeRecord,
    compare_target: &NodeRecord,
    comparison_result: &NoteComparisonResult,
) -> Result<TimingReport> {
    let fixture = DedicatedBufferFixture {
        node,
        compare_target,
        comparison_result,
    };
    let fixture_file = repo_root
        .join("target")
        .join("bench")
        .join("dedicated-buffer-fixture.json");
    if let Some(parent) = fixture_file.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create dedicated buffer fixture directory {}",
                parent.display()
            )
        })?;
    }
    write_json(&fixture_file, &fixture)?;

    let emacs = std::env::var("EMACS").unwrap_or_else(|_| "emacs".to_owned());
    let eval = format!(
        "(princ (org-slipbox-buffer-bench-run-dedicated-file {:?} {} {}))",
        fixture_file.to_string_lossy(),
        profile.iterations.dedicated_buffer_samples,
        profile.iterations.dedicated_buffer_iterations
    );
    let output = Command::new(&emacs)
        .current_dir(repo_root)
        .arg("-Q")
        .arg("--batch")
        .arg("-L")
        .arg(".")
        .arg("-l")
        .arg("org-slipbox.el")
        .arg("-l")
        .arg("benches/org-slipbox-buffer-bench.el")
        .arg("--eval")
        .arg(eval)
        .output()
        .with_context(|| format!("failed to execute {emacs} for dedicated buffer benchmark"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("{emacs} exited with {}", output.status)
        } else {
            stderr
        };
        bail!("dedicated buffer benchmark failed: {message}");
    }

    let report: ElispTimingReport = serde_json::from_slice(&output.stdout)
        .context("failed to parse dedicated buffer report")?;
    if report.samples_ms.is_empty() {
        bail!("dedicated buffer benchmark produced no samples");
    }
    Ok(TimingReport::from_samples(report.samples_ms))
}

fn select_dedicated_exploration_fixture(
    database: &Database,
    node: &NodeRecord,
    limit: usize,
) -> Result<(ExplorationLens, ExploreResult)> {
    let unresolved_tasks = database.unresolved_tasks(node, limit)?;
    let weakly_integrated_notes = database.weakly_integrated_notes(node, limit)?;
    if unresolved_tasks.is_empty() || weakly_integrated_notes.is_empty() {
        bail!(
            "dedicated exploration benchmark requires non-empty unresolved and weakly integrated sections for node {}",
            node.node_key
        );
    }

    Ok((
        ExplorationLens::Unresolved,
        ExploreResult {
            lens: ExplorationLens::Unresolved,
            sections: vec![
                ExplorationSection {
                    kind: ExplorationSectionKind::UnresolvedTasks,
                    entries: unresolved_tasks
                        .into_iter()
                        .map(|record| ExplorationEntry::Anchor {
                            record: Box::new(record),
                        })
                        .collect(),
                },
                ExplorationSection {
                    kind: ExplorationSectionKind::WeaklyIntegratedNotes,
                    entries: weakly_integrated_notes
                        .into_iter()
                        .map(|record| ExplorationEntry::Anchor {
                            record: Box::new(record),
                        })
                        .collect(),
                },
            ],
        },
    ))
}

fn benchmark_dedicated_exploration_buffer(
    repo_root: &Path,
    profile: &BenchmarkProfile,
    node: &NodeRecord,
    lens: ExplorationLens,
    exploration_result: &ExploreResult,
) -> Result<TimingReport> {
    let fixture = DedicatedExplorationBufferFixture {
        node,
        lens,
        exploration_result,
    };
    let fixture_file = repo_root
        .join("target")
        .join("bench")
        .join("dedicated-exploration-buffer-fixture.json");
    if let Some(parent) = fixture_file.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create dedicated exploration fixture directory {}",
                parent.display()
            )
        })?;
    }
    write_json(&fixture_file, &fixture)?;

    let emacs = std::env::var("EMACS").unwrap_or_else(|_| "emacs".to_owned());
    let eval = format!(
        "(princ (org-slipbox-buffer-bench-run-exploration-file {:?} {} {}))",
        fixture_file.to_string_lossy(),
        profile.iterations.dedicated_exploration_buffer_samples,
        profile.iterations.dedicated_exploration_buffer_iterations
    );
    let output = Command::new(&emacs)
        .current_dir(repo_root)
        .arg("-Q")
        .arg("--batch")
        .arg("-L")
        .arg(".")
        .arg("-l")
        .arg("org-slipbox.el")
        .arg("-l")
        .arg("benches/org-slipbox-buffer-bench.el")
        .arg("--eval")
        .arg(eval)
        .output()
        .with_context(|| {
            format!("failed to execute {emacs} for dedicated exploration benchmark")
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("{emacs} exited with {}", output.status)
        } else {
            stderr
        };
        bail!("dedicated exploration benchmark failed: {message}");
    }

    let report: ElispTimingReport = serde_json::from_slice(&output.stdout)
        .context("failed to parse dedicated exploration buffer report")?;
    if report.samples_ms.is_empty() {
        bail!("dedicated exploration benchmark produced no samples");
    }
    Ok(TimingReport::from_samples(report.samples_ms))
}

fn benchmark_workflow_catalog(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    let sample = workbench.list_workflows()?;
    assert_workflow_catalog_fixture(&sample, fixture)?;
    measure_iterations(profile.iterations.workflow_catalog, |_| {
        let workflows = workbench.list_workflows()?;
        assert_workflow_catalog_fixture(&workflows, fixture)?;
        black_box(workflows.workflows.len());
        Ok(())
    })
}

fn benchmark_workflow_run(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
    focus_node_key: &str,
) -> Result<TimingReport> {
    let params = benchmark_workflow_params(focus_node_key);
    let sample = workbench.run_workflow(&params)?;
    assert_benchmark_workflow_result(&sample, fixture, focus_node_key)?;
    measure_iterations(profile.iterations.workflow_run, |_| {
        let result = workbench.run_workflow(&params)?;
        assert_benchmark_workflow_result(&result, fixture, focus_node_key)?;
        black_box(result.result.steps.len());
        Ok(())
    })
}

fn benchmark_corpus_audit(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
) -> Result<TimingReport> {
    const AUDITS: [CorpusAuditKind; 4] = [
        CorpusAuditKind::DanglingLinks,
        CorpusAuditKind::DuplicateTitles,
        CorpusAuditKind::OrphanNotes,
        CorpusAuditKind::WeaklyIntegratedNotes,
    ];

    for audit in AUDITS {
        let sample = workbench.corpus_audit(&CorpusAuditParams {
            audit,
            limit: profile.iterations.audit_limit,
        })?;
        if sample.entries.is_empty() {
            bail!("benchmark audit {audit:?} returned no entries");
        }
    }

    measure_iterations(profile.iterations.corpus_audit, |iteration| {
        let audit = AUDITS[iteration % AUDITS.len()];
        let result = workbench.corpus_audit(&CorpusAuditParams {
            audit,
            limit: profile.iterations.audit_limit,
        })?;
        if result.audit != audit {
            bail!(
                "benchmark audit returned mismatched kind: expected {audit:?}, got {:?}",
                result.audit
            );
        }
        if result.entries.is_empty() {
            bail!("benchmark audit {audit:?} returned no entries");
        }
        black_box(result.entries.len());
        Ok(())
    })
}

fn prepare_review_benchmark_fixtures(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
    focus_node_key: &str,
) -> Result<ReviewBenchmarkFixture> {
    let audit_base = workbench.save_corpus_audit_review(&SaveCorpusAuditReviewParams {
        audit: CorpusAuditKind::DanglingLinks,
        limit: profile.iterations.audit_limit,
        review_id: Some(AUDIT_REVIEW_BASE_ID.to_owned()),
        title: Some("Benchmark Dangling Link Review Base".to_owned()),
        summary: Some("Stable base fixture for operational review benchmarks.".to_owned()),
        overwrite: true,
    })?;
    assert_saved_audit_review_fixture(&audit_base, AUDIT_REVIEW_BASE_ID)?;

    let audit_target = workbench.save_corpus_audit_review(&SaveCorpusAuditReviewParams {
        audit: CorpusAuditKind::DanglingLinks,
        limit: profile.iterations.audit_limit,
        review_id: Some(AUDIT_REVIEW_TARGET_ID.to_owned()),
        title: Some("Benchmark Dangling Link Review Target".to_owned()),
        summary: Some("Mutable target fixture for review diff and mark benchmarks.".to_owned()),
        overwrite: true,
    })?;
    assert_saved_audit_review_fixture(&audit_target, AUDIT_REVIEW_TARGET_ID)?;

    let target_review = workbench.review_run(&ReviewRunIdParams {
        review_id: AUDIT_REVIEW_TARGET_ID.to_owned(),
    })?;
    let mark_finding_id = target_review
        .review
        .findings
        .first()
        .context("benchmark target audit review produced no findings")?
        .finding_id
        .clone();
    let remediation_finding_id = mark_finding_id.clone();
    let transition = workbench.mark_review_finding(&MarkReviewFindingParams {
        review_id: AUDIT_REVIEW_TARGET_ID.to_owned(),
        finding_id: mark_finding_id.clone(),
        status: ReviewFindingStatus::Reviewed,
    })?;
    if transition.transition.to_status != ReviewFindingStatus::Reviewed {
        bail!("benchmark review mark fixture failed to enter reviewed status");
    }

    let diff = workbench.diff_review_runs(&ReviewRunDiffParams {
        base_review_id: AUDIT_REVIEW_BASE_ID.to_owned(),
        target_review_id: AUDIT_REVIEW_TARGET_ID.to_owned(),
    })?;
    assert_review_diff_fixture(&diff)?;

    let preview =
        workbench.review_finding_remediation_preview(&ReviewFindingRemediationPreviewParams {
            review_id: AUDIT_REVIEW_TARGET_ID.to_owned(),
            finding_id: remediation_finding_id.clone(),
        })?;
    assert_remediation_preview_fixture(&preview)?;

    let workflow_review = workbench.save_workflow_review(&benchmark_workflow_review_params(
        focus_node_key,
        WORKFLOW_REVIEW_ID.to_owned(),
        true,
    ))?;
    assert_benchmark_workflow_execution_result(&workflow_review.result, fixture, focus_node_key)?;
    if workflow_review.review.metadata.review_id != WORKFLOW_REVIEW_ID {
        bail!(
            "workflow review fixture saved unexpected review id {}",
            workflow_review.review.metadata.review_id
        );
    }
    if workflow_review.review.finding_count != workflow_review.result.steps.len() {
        bail!(
            "workflow review fixture expected one finding per step, found {} findings for {} steps",
            workflow_review.review.finding_count,
            workflow_review.result.steps.len()
        );
    }

    Ok(ReviewBenchmarkFixture {
        audit_base_review_id: AUDIT_REVIEW_BASE_ID.to_owned(),
        audit_target_review_id: AUDIT_REVIEW_TARGET_ID.to_owned(),
        workflow_review_id: WORKFLOW_REVIEW_ID.to_owned(),
        remediation_finding_id,
        mark_finding_id,
    })
}

fn benchmark_review_list(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &ReviewBenchmarkFixture,
) -> Result<TimingReport> {
    let sample = workbench.list_review_runs()?;
    assert_review_list_fixture(&sample, fixture)?;
    measure_iterations(profile.iterations.review_list, |_| {
        let result = workbench.list_review_runs()?;
        assert_review_list_fixture(&result, fixture)?;
        black_box(result.reviews.len());
        Ok(())
    })
}

fn benchmark_review_show(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &ReviewBenchmarkFixture,
) -> Result<TimingReport> {
    let params = ReviewRunIdParams {
        review_id: fixture.workflow_review_id.clone(),
    };
    let sample = workbench.review_run(&params)?;
    assert_review_show_fixture(&sample)?;
    measure_iterations(profile.iterations.review_show, |_| {
        let result = workbench.review_run(&params)?;
        assert_review_show_fixture(&result)?;
        black_box(result.review.findings.len());
        Ok(())
    })
}

fn benchmark_review_diff(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &ReviewBenchmarkFixture,
) -> Result<TimingReport> {
    let params = ReviewRunDiffParams {
        base_review_id: fixture.audit_base_review_id.clone(),
        target_review_id: fixture.audit_target_review_id.clone(),
    };
    let sample = workbench.diff_review_runs(&params)?;
    assert_review_diff_fixture(&sample)?;
    measure_iterations(profile.iterations.review_diff, |_| {
        let result = workbench.diff_review_runs(&params)?;
        assert_review_diff_fixture(&result)?;
        black_box(result.diff.status_changed.len());
        Ok(())
    })
}

fn benchmark_review_mark(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &ReviewBenchmarkFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.review_mark, |iteration| {
        let status = if iteration % 2 == 0 {
            ReviewFindingStatus::Open
        } else {
            ReviewFindingStatus::Reviewed
        };
        let result = workbench.mark_review_finding(&MarkReviewFindingParams {
            review_id: fixture.audit_target_review_id.clone(),
            finding_id: fixture.mark_finding_id.clone(),
            status,
        })?;
        if result.transition.to_status != status {
            bail!(
                "review mark benchmark returned {:?}, expected {:?}",
                result.transition.to_status,
                status
            );
        }
        black_box(result.transition);
        Ok(())
    })
}

fn benchmark_audit_save_review(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.audit_save_review, |iteration| {
        let review_id = format!("review/benchmark/audit-save/{iteration:04}");
        let result = workbench.save_corpus_audit_review(&SaveCorpusAuditReviewParams {
            audit: CorpusAuditKind::DanglingLinks,
            limit: profile.iterations.audit_limit,
            review_id: Some(review_id.clone()),
            title: Some(format!("Benchmark Audit Save Review {iteration:04}")),
            summary: Some("Per-iteration audit save-review benchmark fixture.".to_owned()),
            overwrite: true,
        })?;
        assert_saved_audit_review_fixture(&result, &review_id)?;
        black_box(result.review.finding_count);
        Ok(())
    })
}

fn benchmark_workflow_save_review(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
    focus_node_key: &str,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.workflow_save_review, |iteration| {
        let review_id = format!("review/benchmark/workflow-save/{iteration:04}");
        let result = workbench.save_workflow_review(&benchmark_workflow_review_params(
            focus_node_key,
            review_id.clone(),
            true,
        ))?;
        assert_benchmark_workflow_execution_result(&result.result, fixture, focus_node_key)?;
        if result.review.metadata.review_id != review_id {
            bail!(
                "workflow save-review benchmark returned unexpected review id {}",
                result.review.metadata.review_id
            );
        }
        black_box(result.review.finding_count);
        Ok(())
    })
}

fn benchmark_remediation_preview(
    workbench: &mut server::WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &ReviewBenchmarkFixture,
) -> Result<TimingReport> {
    let params = ReviewFindingRemediationPreviewParams {
        review_id: fixture.audit_target_review_id.clone(),
        finding_id: fixture.remediation_finding_id.clone(),
    };
    let sample = workbench.review_finding_remediation_preview(&params)?;
    assert_remediation_preview_fixture(&sample)?;
    measure_iterations(profile.iterations.remediation_preview, |_| {
        let result = workbench.review_finding_remediation_preview(&params)?;
        assert_remediation_preview_fixture(&result)?;
        black_box(result.preview.finding_id.len());
        Ok(())
    })
}

fn benchmark_workflow_params(focus_node_key: &str) -> RunWorkflowParams {
    RunWorkflowParams {
        workflow_id: WORKFLOW_BENCHMARK_ID.to_owned(),
        inputs: vec![WorkflowInputAssignment {
            input_id: "focus".to_owned(),
            target: WorkflowResolveTarget::NodeKey {
                node_key: focus_node_key.to_owned(),
            },
        }],
    }
}

fn benchmark_workflow_review_params(
    focus_node_key: &str,
    review_id: String,
    overwrite: bool,
) -> SaveWorkflowReviewParams {
    SaveWorkflowReviewParams {
        workflow_id: WORKFLOW_BENCHMARK_ID.to_owned(),
        inputs: benchmark_workflow_params(focus_node_key).inputs,
        review_id: Some(review_id),
        title: Some("Benchmark Workflow Review".to_owned()),
        summary: Some("Workflow save-review benchmark fixture.".to_owned()),
        overwrite,
    }
}

fn assert_workflow_catalog_fixture(
    catalog: &slipbox_core::ListWorkflowsResult,
    fixture: &CorpusFixture,
) -> Result<()> {
    let benchmark_workflow = catalog
        .workflows
        .iter()
        .find(|workflow| workflow.metadata.workflow_id == WORKFLOW_BENCHMARK_ID)
        .context(
            "benchmark workflow discovery catalog omitted the discovered benchmark workflow",
        )?;
    if benchmark_workflow.step_count != 5 {
        bail!(
            "benchmark workflow discovery catalog reported unexpected step count {}",
            benchmark_workflow.step_count
        );
    }
    let expected_workflow_count = slipbox_core::built_in_workflows().len() + fixture.workflow_specs;
    if catalog.workflows.len() != expected_workflow_count {
        bail!(
            "benchmark workflow discovery catalog expected {expected_workflow_count} workflows, found {}",
            catalog.workflows.len()
        );
    }
    if !catalog.issues.is_empty() {
        bail!(
            "benchmark workflow discovery catalog expected no issues, found {}",
            catalog.issues.len()
        );
    }
    Ok(())
}

fn assert_benchmark_workflow_result(
    workflow: &slipbox_core::RunWorkflowResult,
    fixture: &CorpusFixture,
    focus_node_key: &str,
) -> Result<()> {
    assert_benchmark_workflow_execution_result(&workflow.result, fixture, focus_node_key)
}

fn assert_benchmark_workflow_execution_result(
    workflow: &WorkflowExecutionResult,
    fixture: &CorpusFixture,
    focus_node_key: &str,
) -> Result<()> {
    if workflow.workflow.metadata.workflow_id != WORKFLOW_BENCHMARK_ID {
        bail!(
            "workflow benchmark returned unexpected workflow id {}",
            workflow.workflow.metadata.workflow_id
        );
    }
    if workflow.steps.len() != 5 {
        bail!(
            "workflow benchmark expected 5 steps, found {}",
            workflow.steps.len()
        );
    }

    let refs_step = workflow
        .steps
        .iter()
        .find(|step| step.step_id == "explore-refs")
        .context("workflow benchmark result omitted explore-refs step")?;
    match &refs_step.payload {
        WorkflowStepReportPayload::Explore {
            focus_node_key: observed_focus,
            result,
        } => {
            if observed_focus != focus_node_key {
                bail!(
                    "workflow benchmark refs step lost anchor focus: expected {focus_node_key}, got {observed_focus}"
                );
            }
            if result.lens != ExplorationLens::Refs
                || result
                    .sections
                    .iter()
                    .all(|section| section.entries.is_empty())
            {
                bail!("workflow benchmark refs step did not return a populated refs result");
            }
        }
        other => {
            bail!(
                "workflow benchmark explore-refs step returned wrong payload kind {:?}",
                other.kind()
            );
        }
    }

    for (step_id, expected_lens) in [
        ("explore-unresolved", ExplorationLens::Unresolved),
        ("explore-tasks", ExplorationLens::Tasks),
        ("explore-time", ExplorationLens::Time),
    ] {
        let step = workflow
            .steps
            .iter()
            .find(|report| report.step_id == step_id)
            .with_context(|| format!("workflow benchmark result omitted {step_id}"))?;
        match &step.payload {
            WorkflowStepReportPayload::Explore {
                focus_node_key: observed_focus,
                result,
            } => {
                if step_id != "explore-unresolved" && observed_focus != focus_node_key {
                    bail!(
                        "workflow benchmark {step_id} lost anchor focus: expected {focus_node_key}, got {observed_focus}"
                    );
                }
                if result.lens != expected_lens
                    || result
                        .sections
                        .iter()
                        .all(|section| section.entries.is_empty())
                {
                    bail!(
                        "workflow benchmark {step_id} did not return a populated {:?} result",
                        expected_lens
                    );
                }
            }
            other => {
                bail!(
                    "workflow benchmark {step_id} returned wrong payload kind {:?}",
                    other.kind()
                );
            }
        }
    }

    if fixture.workflow_focus_point.file_path.is_empty() {
        bail!("workflow benchmark fixture did not record a focus point");
    }

    Ok(())
}

fn assert_saved_audit_review_fixture(
    result: &slipbox_core::SaveCorpusAuditReviewResult,
    expected_review_id: &str,
) -> Result<()> {
    if result.result.audit != CorpusAuditKind::DanglingLinks {
        bail!(
            "audit save-review fixture returned {:?}, expected dangling links",
            result.result.audit
        );
    }
    if result.result.entries.is_empty() {
        bail!("audit save-review fixture produced no audit entries");
    }
    if result.review.metadata.review_id != expected_review_id {
        bail!(
            "audit save-review fixture returned unexpected review id {}",
            result.review.metadata.review_id
        );
    }
    if result.review.finding_count != result.result.entries.len() {
        bail!(
            "audit save-review fixture expected {} findings, found {}",
            result.result.entries.len(),
            result.review.finding_count
        );
    }
    Ok(())
}

fn assert_review_list_fixture(
    result: &slipbox_core::ListReviewRunsResult,
    fixture: &ReviewBenchmarkFixture,
) -> Result<()> {
    for review_id in [
        &fixture.audit_base_review_id,
        &fixture.audit_target_review_id,
        &fixture.workflow_review_id,
    ] {
        if !result
            .reviews
            .iter()
            .any(|review| review.metadata.review_id == *review_id)
        {
            bail!("review list benchmark omitted fixture review {review_id}");
        }
    }
    Ok(())
}

fn assert_review_show_fixture(result: &slipbox_core::ReviewRunResult) -> Result<()> {
    if result.review.findings.is_empty() {
        bail!("review show benchmark returned a review with no findings");
    }
    if result.review.validation_error().is_some() {
        bail!("review show benchmark returned an invalid review");
    }
    Ok(())
}

fn assert_review_diff_fixture(result: &slipbox_core::ReviewRunDiffResult) -> Result<()> {
    if result.diff.status_changed.is_empty() {
        bail!("review diff benchmark fixture produced no status changes");
    }
    if result.diff.added.len()
        + result.diff.removed.len()
        + result.diff.unchanged.len()
        + result.diff.content_changed.len()
        + result.diff.status_changed.len()
        == 0
    {
        bail!("review diff benchmark produced an empty diff");
    }
    Ok(())
}

fn assert_remediation_preview_fixture(
    result: &slipbox_core::ReviewFindingRemediationPreviewResult,
) -> Result<()> {
    match &result.preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            missing_explicit_id,
            suggestion,
            ..
        } => {
            if missing_explicit_id.is_empty() || suggestion.is_empty() {
                bail!("remediation preview benchmark returned incomplete dangling-link payload");
            }
        }
        other => {
            bail!(
                "remediation preview benchmark returned unsupported payload {:?}",
                other
            );
        }
    }
    Ok(())
}

fn measure_iterations(
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

fn generate_corpus(workspace: &Path, config: &CorpusConfig) -> Result<CorpusFixture> {
    let total_headings = config
        .files
        .checked_mul(config.headings_per_file)
        .context("benchmark corpus heading count overflowed")?;
    if total_headings < 8 {
        bail!(
            "benchmark corpus requires at least 8 headings to reserve workflow, audit, and exploration fixtures"
        );
    }
    let duplicate_title_upper_index = total_headings - 8;
    let duplicate_title_lower_index = total_headings - 7;
    let dangling_one_index = total_headings - 6;
    let dangling_two_index = total_headings - 5;
    let orphan_index = total_headings - 4;
    let audit_weak_index = total_headings - 3;
    let exploration_unresolved_index = total_headings - 2;
    let exploration_weak_index = total_headings - 1;
    let exploration_node_id = format!("node-{EXPLORATION_FOCUS_INDEX:06}");

    let root = workspace.join("corpus");
    if root.exists() {
        fs::remove_dir_all(&root)
            .with_context(|| format!("failed to clear corpus directory {}", root.display()))?;
    }
    fs::create_dir_all(&root)
        .with_context(|| format!("failed to create corpus directory {}", root.display()))?;

    let notes_dir = root.join("notes");
    fs::create_dir_all(&notes_dir)
        .with_context(|| format!("failed to create notes directory {}", notes_dir.display()))?;
    let workflow_dir = root.join(WORKFLOW_DISCOVERY_DIR);
    fs::create_dir_all(&workflow_dir).with_context(|| {
        format!(
            "failed to create workflow directory {}",
            workflow_dir.display()
        )
    })?;

    let mut search_queries = BTreeSet::new();
    let mut file_queries = BTreeSet::new();
    let mut point_queries = Vec::new();
    let mut workflow_focus_point = None;
    let mut mutable_file = PathBuf::new();
    let mut mutable_relative_path = String::new();
    let mut mutable_template = String::new();
    let mut forward_node_id = None;
    let mut expected_links = 0_usize;

    for file_index in 0..config.files {
        let relative_path = format!("notes/file-{file_index:04}.org");
        let absolute_path = root.join(&relative_path);
        let bucket_tag = format!("bucket{}", file_index % 8);
        let file_title = format!("Bench File {file_index:04}");
        if file_queries.len() < config.query_count {
            file_queries.insert(relative_path.clone());
            file_queries.insert(file_title.clone());
        }
        let mut lines = vec![
            format!("#+title: {file_title}"),
            format!("#+filetags: :bench:{bucket_tag}:"),
            String::from(":PROPERTIES:"),
            format!(":ID: file-{file_index:04}"),
            String::from(":END:"),
            String::new(),
        ];

        for heading_index in 0..config.headings_per_file {
            let global_index = file_index * config.headings_per_file + heading_index;
            let heading_line = (lines.len() + 1) as u32;
            if point_queries.len() < config.query_count {
                point_queries.push(PointQuery {
                    file_path: relative_path.clone(),
                    line: heading_line,
                });
            }

            let is_exploration_focus = global_index == EXPLORATION_FOCUS_INDEX;
            let is_duplicate_title_upper = global_index == duplicate_title_upper_index;
            let is_duplicate_title_lower = global_index == duplicate_title_lower_index;
            let is_dangling_one = global_index == dangling_one_index;
            let is_dangling_two = global_index == dangling_two_index;
            let is_orphan = global_index == orphan_index;
            let is_audit_weak = global_index == audit_weak_index;
            let is_exploration_unresolved = global_index == exploration_unresolved_index;
            let is_exploration_weak = global_index == exploration_weak_index;
            let is_special_fixture = is_exploration_focus
                || is_duplicate_title_upper
                || is_duplicate_title_lower
                || is_dangling_one
                || is_dangling_two
                || is_orphan
                || is_audit_weak
                || is_exploration_unresolved
                || is_exploration_weak;

            let title = if is_exploration_focus {
                "Exploration Focus".to_owned()
            } else if is_duplicate_title_upper {
                "Shared Audit Title".to_owned()
            } else if is_duplicate_title_lower {
                "shared audit title".to_owned()
            } else if is_dangling_one {
                "Dangling Audit One".to_owned()
            } else if is_dangling_two {
                "Dangling Audit Two".to_owned()
            } else if is_orphan {
                "Orphan Audit".to_owned()
            } else if is_audit_weak {
                "Weak Audit".to_owned()
            } else if is_exploration_unresolved {
                "Exploration Unresolved".to_owned()
            } else if is_exploration_weak {
                "Exploration Weak".to_owned()
            } else {
                format!("Bench Topic {global_index:06}")
            };
            let alias = if is_exploration_focus {
                "Exploration Focus Alias".to_owned()
            } else if is_duplicate_title_upper {
                "Shared Audit Title Alias".to_owned()
            } else if is_duplicate_title_lower {
                "shared audit title alias".to_owned()
            } else if is_dangling_one {
                "Dangling Audit One Alias".to_owned()
            } else if is_dangling_two {
                "Dangling Audit Two Alias".to_owned()
            } else if is_orphan {
                "Orphan Audit Alias".to_owned()
            } else if is_audit_weak {
                "Weak Audit Alias".to_owned()
            } else if is_exploration_unresolved {
                "Exploration Unresolved Alias".to_owned()
            } else if is_exploration_weak {
                "Exploration Weak Alias".to_owned()
            } else {
                format!("Alias {global_index:06}")
            };
            let tag = format!("tag{}", global_index % 17);
            let todo = if is_exploration_focus
                || is_audit_weak
                || is_exploration_unresolved
                || (!is_special_fixture && global_index % 4 == 0)
            {
                "TODO "
            } else {
                ""
            };
            let day = (global_index % 28) + 1;

            lines.push(format!("* {todo}{title} :{tag}:{bucket_tag}:"));
            lines.push(String::from(":PROPERTIES:"));
            lines.push(format!(":ID: node-{global_index:06}"));
            lines.push(format!(":ROAM_ALIASES: \"{alias}\""));
            if is_exploration_focus {
                lines.push(format!(
                    ":ROAM_REFS: {EXPLORATION_SHARED_REF} {EXPLORATION_FOCUS_REF}"
                ));
            } else if is_audit_weak || is_exploration_unresolved || is_exploration_weak {
                lines.push(format!(":ROAM_REFS: {EXPLORATION_SHARED_REF}"));
            } else if !is_special_fixture && global_index % config.ref_stride == 0 {
                lines.push(format!(":ROAM_REFS: @cite{global_index:06}"));
            }
            lines.push(String::from(":END:"));
            if is_exploration_focus || is_audit_weak || is_exploration_unresolved {
                lines.push(String::from("SCHEDULED: <2026-03-05 Thu>"));
            } else if !is_special_fixture && global_index % config.scheduled_stride == 0 {
                lines.push(format!("SCHEDULED: <2026-03-{day:02} Tue>"));
            }
            if is_exploration_focus || is_exploration_weak {
                lines.push(String::from("DEADLINE: <2026-03-09 Mon>"));
            } else if !is_special_fixture && global_index % config.deadline_stride == 0 {
                lines.push(format!("DEADLINE: <2026-03-{day:02} Tue>"));
            }
            lines.push(format!("Bench body for {title}."));
            if is_duplicate_title_upper {
                lines.push(format!(
                    "Links to [[id:node-{:06}][matching duplicate]].",
                    duplicate_title_lower_index
                ));
                expected_links += 1;
            } else if is_duplicate_title_lower {
                lines.push(format!(
                    "Links to [[id:node-{:06}][matching duplicate]].",
                    duplicate_title_upper_index
                ));
                expected_links += 1;
            } else if is_dangling_one {
                lines.push(String::from(
                    "Broken [[id:missing-bench-audit-one][missing]].",
                ));
                expected_links += 1;
            } else if is_dangling_two {
                lines.push(String::from(
                    "Broken [[id:missing-bench-audit-two][missing]].",
                ));
                expected_links += 1;
            } else if !is_special_fixture && global_index > 0 {
                lines.push(format!("Prev [[id:node-{:06}][prev]].", global_index - 1));
                expected_links += 1;
                if forward_node_id.is_none() {
                    forward_node_id = Some(format!("node-{global_index:06}"));
                }
            }
            if !is_special_fixture
                && global_index != 0
                && global_index % config.hot_link_stride == 0
            {
                lines.push(format!("Hub [[id:{HOT_NODE_ID}][hub]]."));
                expected_links += 1;
            }
            if !is_special_fixture && global_index % config.hot_link_stride == 0 {
                lines.push(String::from("Reference cite:cite000000."));
                lines.push(String::from("Mention Bench Topic 000000."));
                lines.push(String::from("[[id:node-000000][Bench Topic 000000]]."));
                expected_links += 1;
            }
            lines.push(String::new());

            if is_exploration_focus {
                let workflow_anchor_line = (lines.len() + 1) as u32;
                lines.push(String::from("** TODO Workflow Focus Anchor"));
                lines.push(String::from(":PROPERTIES:"));
                lines.push(format!(
                    ":ROAM_REFS: {EXPLORATION_SHARED_REF} {EXPLORATION_FOCUS_REF}"
                ));
                lines.push(String::from(":END:"));
                lines.push(String::from("SCHEDULED: <2026-03-05 Thu>"));
                lines.push(String::from("DEADLINE: <2026-03-09 Mon>"));
                lines.push(String::from(
                    "Anchor-only focus for workflow benchmark paths.",
                ));
                lines.push(String::new());
                workflow_focus_point = Some(PointQuery {
                    file_path: relative_path.clone(),
                    line: workflow_anchor_line,
                });
            }

            if search_queries.len() < config.query_count {
                search_queries.insert(title);
                search_queries.insert(alias);
                search_queries.insert(tag);
                search_queries.insert(bucket_tag.clone());
            }
        }

        if file_index == 0 {
            lines.push(String::from("Mutable __BENCH_MUTABLE__"));
            lines.push(String::new());
            mutable_file = absolute_path.clone();
            mutable_relative_path = relative_path.clone();
        }

        let rendered = lines.join("\n") + "\n";
        fs::write(&absolute_path, &rendered)
            .with_context(|| format!("failed to write corpus file {}", absolute_path.display()))?;
        if file_index == 0 {
            mutable_template = rendered;
        }
    }

    for workflow_index in 0..config.workflow_specs {
        let path = workflow_dir.join(format!("workflow-{workflow_index:04}.json"));
        let workflow = if workflow_index == 0 {
            benchmark_workflow_spec()
        } else {
            catalog_workflow_spec(workflow_index)
        };
        write_json(&path, &workflow)?;
    }

    Ok(CorpusFixture {
        root,
        workflow_dirs: vec![PathBuf::from(WORKFLOW_DISCOVERY_DIR)],
        mutable_file,
        mutable_relative_path,
        mutable_template,
        hot_node_id: HOT_NODE_ID.to_owned(),
        exploration_node_id,
        workflow_focus_point: workflow_focus_point
            .context("benchmark corpus did not produce a workflow focus anchor")?,
        workflow_specs: config.workflow_specs,
        forward_node_id: forward_node_id
            .context("benchmark corpus did not produce a forward-link source node")?,
        search_queries: search_queries
            .into_iter()
            .take(config.query_count)
            .collect(),
        file_queries: file_queries.into_iter().take(config.query_count).collect(),
        point_queries,
        expected_files: config.files,
        expected_nodes: config.files * (config.headings_per_file + 1) + 1,
        expected_links,
    })
}

fn benchmark_workflow_spec() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: WORKFLOW_BENCHMARK_ID.to_owned(),
            title: "Benchmark Research Sweep".to_owned(),
            summary: Some(
                "Exercise discovery plus rich refs, unresolved, task, and time workflow paths."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus target".to_owned(),
            summary: Some("Note or anchor target to sweep".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowSpecResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-refs".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-unresolved".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Unresolved,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-tasks".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Tasks,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-time".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Time,
                    limit: 25,
                    unique: false,
                },
            },
        ],
    }
}

fn catalog_workflow_spec(workflow_index: usize) -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: format!("workflow/discovered/catalog-{workflow_index:04}"),
            title: format!("Catalog Workflow {workflow_index:04}"),
            summary: Some("Discovery-only catalog workflow for benchmark scale.".to_owned()),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus note".to_owned(),
            summary: Some("Exact note target".to_owned()),
            kind: WorkflowInputKind::NoteTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowSpecResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-structure".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Structure,
                    limit: 15,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-dormant".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Dormant,
                    limit: 15,
                    unique: false,
                },
            },
        ],
    }
}

fn assert_expected_counts(database: &Database, fixture: &CorpusFixture) -> Result<()> {
    let stats = database
        .stats()
        .context("failed to read benchmark index stats")?;
    if stats.files_indexed != fixture.expected_files as u64 {
        bail!(
            "expected {} indexed files, found {}",
            fixture.expected_files,
            stats.files_indexed
        );
    }
    if stats.nodes_indexed != fixture.expected_nodes as u64 {
        bail!(
            "expected {} indexed nodes, found {}",
            fixture.expected_nodes,
            stats.nodes_indexed
        );
    }
    if stats.links_indexed != fixture.expected_links as u64 {
        bail!(
            "expected {} indexed links, found {}",
            fixture.expected_links,
            stats.links_indexed
        );
    }
    Ok(())
}

fn enforce_thresholds(report: &BenchmarkReport, thresholds: &ThresholdConfig) -> Result<()> {
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
    Ok(())
}

fn check_threshold(metric: &str, observed: f64, limit: f64) -> Result<()> {
    if observed > limit {
        bail!(
            "{metric} p95 {:.2} ms exceeds threshold {:.2} ms",
            observed,
            limit
        );
    }
    Ok(())
}

fn print_summary(report: &BenchmarkReport, check: bool, output_path: &Path) {
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
}

fn print_metric(name: &str, report: &TimingReport) {
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

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let encoded = serde_json::to_vec_pretty(value).context("failed to serialize JSON")?;
    fs::write(path, encoded).with_context(|| format!("failed to write {}", path.display()))
}

fn resolve_profile_path(selector: &str) -> Result<PathBuf> {
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

fn load_profile(path: &Path) -> Result<BenchmarkProfile> {
    let source =
        fs::read(path).with_context(|| format!("failed to read profile {}", path.display()))?;
    serde_json::from_slice(&source)
        .with_context(|| format!("failed to parse benchmark profile {}", path.display()))
}

fn default_report_path(profile: &str, check: bool) -> PathBuf {
    let mode = if check { "check" } else { "run" };
    manifest_dir()
        .join("target")
        .join("bench")
        .join(format!("{profile}-{mode}.json"))
}

fn make_persistent_workspace(profile: &str) -> Result<PathBuf> {
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

fn remove_sqlite_artifacts(path: &Path) -> Result<()> {
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

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

impl TimingReport {
    fn from_samples(mut samples_ms: Vec<f64>) -> Self {
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

impl BenchmarkProfile {
    fn validate(&self) -> Result<()> {
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

fn percentile(samples: &[f64], fraction: f64) -> f64 {
    let rank = ((samples.len() as f64) * fraction).ceil() as usize;
    let index = rank.saturating_sub(1).min(samples.len().saturating_sub(1));
    samples[index]
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_report_computes_sorted_percentiles() {
        let report = TimingReport::from_samples(vec![5.0, 1.0, 3.0, 2.0, 4.0]);
        assert_eq!(report.samples_ms, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!((report.mean_ms - 3.0).abs() < f64::EPSILON);
        assert_eq!(report.median_ms, 3.0);
        assert_eq!(report.p95_ms, 5.0);
        assert_eq!(report.max_ms, 5.0);
    }

    #[test]
    fn generates_corpus_with_expected_index_counts() -> Result<()> {
        let tempdir = tempfile::tempdir()?;
        let config = CorpusConfig {
            files: 3,
            headings_per_file: 4,
            workflow_specs: 4,
            hot_link_stride: 2,
            ref_stride: 2,
            scheduled_stride: 2,
            deadline_stride: 3,
            query_count: 4,
        };
        let fixture = generate_corpus(tempdir.path(), &config)?;
        let files = scan_root_with_policy(&fixture.root, &DiscoveryPolicy::default())?;
        let mut database = Database::open(&tempdir.path().join("bench.sqlite3"))?;
        database.sync_index(&files)?;
        assert_expected_counts(&database, &fixture)?;
        assert!(!fixture.search_queries.is_empty());
        assert!(!fixture.point_queries.is_empty());
        Ok(())
    }

    #[test]
    fn generated_corpus_guarantees_a_non_structure_exploration_fixture() -> Result<()> {
        let tempdir = tempfile::tempdir()?;
        let config = CorpusConfig {
            files: 3,
            headings_per_file: 4,
            workflow_specs: 4,
            hot_link_stride: 2,
            ref_stride: 2,
            scheduled_stride: 2,
            deadline_stride: 3,
            query_count: 4,
        };
        let fixture = generate_corpus(tempdir.path(), &config)?;
        let files = scan_root_with_policy(&fixture.root, &DiscoveryPolicy::default())?;
        let mut database = Database::open(&tempdir.path().join("bench.sqlite3"))?;
        database.sync_index(&files)?;
        let exploration_node = database
            .node_from_id(&fixture.exploration_node_id)?
            .context("exploration node should exist")?;
        let (lens, result) =
            select_dedicated_exploration_fixture(&database, &exploration_node, 20)?;
        assert_eq!(lens, ExplorationLens::Unresolved);
        assert_eq!(result.lens, ExplorationLens::Unresolved);
        assert_eq!(result.sections.len(), 2);
        assert_eq!(
            result.sections[0].kind,
            ExplorationSectionKind::UnresolvedTasks
        );
        assert_eq!(
            result.sections[1].kind,
            ExplorationSectionKind::WeaklyIntegratedNotes
        );
        assert!(!result.sections[0].entries.is_empty());
        assert!(!result.sections[1].entries.is_empty());
        Ok(())
    }

    #[test]
    fn generated_corpus_guarantees_workflow_and_audit_benchmark_fixtures() -> Result<()> {
        let tempdir = tempfile::tempdir()?;
        let config = CorpusConfig {
            files: 3,
            headings_per_file: 4,
            workflow_specs: 4,
            hot_link_stride: 2,
            ref_stride: 2,
            scheduled_stride: 2,
            deadline_stride: 3,
            query_count: 4,
        };
        let fixture = generate_corpus(tempdir.path(), &config)?;
        let files = scan_root_with_policy(&fixture.root, &DiscoveryPolicy::default())?;
        let mut database = Database::open(&baseline_db_path(&fixture))?;
        database.sync_index(&files)?;

        let workflow_focus_anchor = database
            .anchor_at_point(
                &fixture.workflow_focus_point.file_path,
                fixture.workflow_focus_point.line,
            )?
            .context("workflow focus anchor should exist")?;

        let mut workbench = server::WorkbenchBench::new(
            fixture.root.clone(),
            baseline_db_path(&fixture),
            fixture.workflow_dirs.clone(),
            DiscoveryPolicy::default(),
        )?;
        let catalog = workbench.list_workflows()?;
        assert_workflow_catalog_fixture(&catalog, &fixture)?;

        let workflow =
            workbench.run_workflow(&benchmark_workflow_params(&workflow_focus_anchor.node_key))?;
        assert_benchmark_workflow_result(&workflow, &fixture, &workflow_focus_anchor.node_key)?;

        for audit in [
            CorpusAuditKind::DanglingLinks,
            CorpusAuditKind::DuplicateTitles,
            CorpusAuditKind::OrphanNotes,
            CorpusAuditKind::WeaklyIntegratedNotes,
        ] {
            let result = workbench.corpus_audit(&CorpusAuditParams { audit, limit: 20 })?;
            assert_eq!(result.audit, audit);
            assert!(
                !result.entries.is_empty(),
                "audit fixture for {:?} should not be empty",
                audit
            );
        }

        let profile = BenchmarkProfile {
            corpus: config,
            iterations: IterationConfig {
                full_index: 1,
                index_file: 1,
                search_nodes: 1,
                search_nodes_sorted: 1,
                search_files: 1,
                search_occurrences: 1,
                backlinks: 1,
                forward_links: 1,
                reflinks: 1,
                unlinked_references: 1,
                node_at_point: 1,
                agenda: 1,
                persistent_buffer_samples: 1,
                persistent_buffer_iterations: 1,
                dedicated_buffer_samples: 1,
                dedicated_buffer_iterations: 1,
                dedicated_exploration_buffer_samples: 1,
                dedicated_exploration_buffer_iterations: 1,
                workflow_catalog: 1,
                workflow_run: 1,
                corpus_audit: 1,
                review_list: 1,
                review_show: 1,
                review_diff: 1,
                review_mark: 2,
                audit_save_review: 1,
                workflow_save_review: 1,
                remediation_preview: 1,
                search_limit: 5,
                backlinks_limit: 20,
                reflinks_limit: 20,
                unlinked_references_limit: 20,
                agenda_limit: 20,
                audit_limit: 20,
            },
            thresholds: ThresholdConfig {
                full_index_p95_ms: 1.0,
                index_file_p95_ms: 1.0,
                search_nodes_p95_ms: 1.0,
                search_nodes_sorted_p95_ms: 1.0,
                search_files_p95_ms: 1.0,
                search_occurrences_p95_ms: 1.0,
                backlinks_p95_ms: 1.0,
                forward_links_p95_ms: 1.0,
                reflinks_p95_ms: 1.0,
                unlinked_references_p95_ms: 1.0,
                node_at_point_p95_ms: 1.0,
                agenda_p95_ms: 1.0,
                persistent_buffer_p95_ms: None,
                dedicated_buffer_p95_ms: None,
                dedicated_exploration_buffer_p95_ms: None,
                workflow_catalog_p95_ms: 1.0,
                workflow_run_p95_ms: 1.0,
                corpus_audit_p95_ms: 1.0,
                review_list_p95_ms: 1.0,
                review_show_p95_ms: 1.0,
                review_diff_p95_ms: 1.0,
                review_mark_p95_ms: 1.0,
                audit_save_review_p95_ms: 1.0,
                workflow_save_review_p95_ms: 1.0,
                remediation_preview_p95_ms: 1.0,
            },
        };
        let review_fixture = prepare_review_benchmark_fixtures(
            &mut workbench,
            &profile,
            &fixture,
            &workflow_focus_anchor.node_key,
        )?;
        assert_review_list_fixture(&workbench.list_review_runs()?, &review_fixture)?;
        assert_review_show_fixture(&workbench.review_run(&ReviewRunIdParams {
            review_id: review_fixture.workflow_review_id.clone(),
        })?)?;
        assert_review_diff_fixture(&workbench.diff_review_runs(&ReviewRunDiffParams {
            base_review_id: review_fixture.audit_base_review_id.clone(),
            target_review_id: review_fixture.audit_target_review_id.clone(),
        })?)?;
        assert_remediation_preview_fixture(&workbench.review_finding_remediation_preview(
            &ReviewFindingRemediationPreviewParams {
                review_id: review_fixture.audit_target_review_id.clone(),
                finding_id: review_fixture.remediation_finding_id.clone(),
            },
        )?)?;

        Ok(())
    }

    #[test]
    fn threshold_check_fails_when_limit_is_exceeded() {
        let error = check_threshold("search_nodes", 10.0, 5.0).unwrap_err();
        assert!(error.to_string().contains("search_nodes"));
    }
}
