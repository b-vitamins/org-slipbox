#[path = "../reflinks_query.rs"]
mod reflinks_query;
#[path = "../text_query.rs"]
mod text_query;
#[path = "../unlinked_references_query.rs"]
mod unlinked_references_query;

use std::collections::BTreeSet;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use slipbox_core::{BacklinkRecord, ForwardLinkRecord, NodeRecord};
use slipbox_index::{DiscoveryPolicy, scan_path_with_policy, scan_root_with_policy};
use slipbox_store::Database;
use tempfile::TempDir;

use reflinks_query::query_reflinks;
use unlinked_references_query::query_unlinked_references;

const HOT_NODE_ID: &str = "node-000000";
const AGENDA_START: &str = "2026-03-01";
const AGENDA_END: &str = "2026-03-31";

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
    search_files: usize,
    backlinks: usize,
    forward_links: usize,
    reflinks: usize,
    unlinked_references: usize,
    node_at_point: usize,
    agenda: usize,
    persistent_buffer_samples: usize,
    persistent_buffer_iterations: usize,
    search_limit: usize,
    backlinks_limit: usize,
    reflinks_limit: usize,
    unlinked_references_limit: usize,
    agenda_limit: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct ThresholdConfig {
    full_index_p95_ms: f64,
    index_file_p95_ms: f64,
    search_nodes_p95_ms: f64,
    search_files_p95_ms: f64,
    backlinks_p95_ms: f64,
    forward_links_p95_ms: f64,
    reflinks_p95_ms: f64,
    unlinked_references_p95_ms: f64,
    node_at_point_p95_ms: f64,
    agenda_p95_ms: f64,
    persistent_buffer_p95_ms: Option<f64>,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    profile: String,
    corpus: CorpusSummary,
    full_index: TimingReport,
    index_file: TimingReport,
    search_nodes: TimingReport,
    search_files: TimingReport,
    backlinks: TimingReport,
    forward_links: TimingReport,
    reflinks: TimingReport,
    unlinked_references: TimingReport,
    node_at_point: TimingReport,
    agenda: TimingReport,
    persistent_buffer: Option<TimingReport>,
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
    mutable_file: PathBuf,
    mutable_relative_path: String,
    mutable_template: String,
    hot_node_id: String,
    forward_node_id: String,
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
    let hot_node = database
        .node_from_id(&fixture.hot_node_id)?
        .context("failed to resolve hot benchmark node")?;
    let forward_node = database
        .node_from_id(&fixture.forward_node_id)?
        .context("failed to resolve forward-link benchmark node")?;

    let search_nodes = benchmark_search_nodes(&mut database, profile, fixture)?;
    let search_files = benchmark_search_files(&mut database, profile, fixture)?;
    let backlinks = benchmark_backlinks(&mut database, profile, &hot_node)?;
    let forward_links = benchmark_forward_links(&mut database, profile, &forward_node)?;
    let reflinks = benchmark_reflinks(&mut database, profile, &fixture.root, &hot_node)?;
    let unlinked_references =
        benchmark_unlinked_references(&mut database, profile, &fixture.root, &hot_node)?;
    let node_at_point = benchmark_node_at_point(&mut database, profile, fixture)?;
    let agenda = benchmark_agenda(&mut database, profile)?;
    let persistent_buffer = if skip_elisp {
        None
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
        Some(benchmark_persistent_buffer(
            repo_root,
            profile,
            &hot_node,
            &buffer_backlinks,
            &buffer_forward_links,
        )?)
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
        search_files,
        backlinks,
        forward_links,
        reflinks,
        unlinked_references,
        node_at_point,
        agenda,
        persistent_buffer,
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
    let db_path = fixture
        .root
        .parent()
        .unwrap_or(fixture.root.as_path())
        .join("baseline.sqlite3");
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
    let sample = query_reflinks(
        database,
        root,
        source_node,
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
            source_node,
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
    let sample = query_unlinked_references(
        database,
        root,
        node,
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
            node,
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

    let mut search_queries = BTreeSet::new();
    let mut file_queries = BTreeSet::new();
    let mut point_queries = Vec::new();
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

            let title = format!("Bench Topic {global_index:06}");
            let alias = format!("Alias {global_index:06}");
            let tag = format!("tag{}", global_index % 17);
            let todo = if global_index % 4 == 0 { "TODO " } else { "" };
            let day = (global_index % 28) + 1;
            if global_index > 0 && forward_node_id.is_none() {
                forward_node_id = Some(format!("node-{global_index:06}"));
            }

            lines.push(format!("* {todo}{title} :{tag}:{bucket_tag}:"));
            lines.push(String::from(":PROPERTIES:"));
            lines.push(format!(":ID: node-{global_index:06}"));
            lines.push(format!(":ROAM_ALIASES: \"{alias}\""));
            if global_index % config.ref_stride == 0 {
                lines.push(format!(":ROAM_REFS: @cite{global_index:06}"));
            }
            lines.push(String::from(":END:"));
            if global_index % config.scheduled_stride == 0 {
                lines.push(format!("SCHEDULED: <2026-03-{day:02} Tue>"));
            } else if global_index % config.deadline_stride == 0 {
                lines.push(format!("DEADLINE: <2026-03-{day:02} Tue>"));
            }
            lines.push(format!("Bench body for {title}."));
            if global_index > 0 {
                lines.push(format!("Prev [[id:node-{:06}][prev]].", global_index - 1));
                expected_links += 1;
            }
            if global_index != 0 && global_index % config.hot_link_stride == 0 {
                lines.push(format!("Hub [[id:{HOT_NODE_ID}][hub]]."));
                expected_links += 1;
            }
            if global_index % config.hot_link_stride == 0 {
                lines.push(String::from("Reference cite:cite000000."));
                lines.push(String::from("Mention Bench Topic 000000."));
                lines.push(String::from("[[id:node-000000][Bench Topic 000000]]."));
                expected_links += 1;
            }
            lines.push(String::new());

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

    Ok(CorpusFixture {
        root,
        mutable_file,
        mutable_relative_path,
        mutable_template,
        hot_node_id: HOT_NODE_ID.to_owned(),
        forward_node_id: forward_node_id
            .context("benchmark corpus did not produce a forward-link source node")?,
        search_queries: search_queries
            .into_iter()
            .take(config.query_count)
            .collect(),
        file_queries: file_queries.into_iter().take(config.query_count).collect(),
        point_queries,
        expected_files: config.files,
        expected_nodes: config.files * (config.headings_per_file + 1),
        expected_links,
    })
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
        "search_files",
        report.search_files.p95_ms,
        thresholds.search_files_p95_ms,
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
    print_metric("searchFiles", &report.search_files);
    print_metric("backlinks", &report.backlinks);
    print_metric("forwardLinks", &report.forward_links);
    print_metric("reflinks", &report.reflinks);
    print_metric("unlinkedReferences", &report.unlinked_references);
    print_metric("nodeAtPoint", &report.node_at_point);
    print_metric("agenda", &report.agenda);
    if let Some(persistent_buffer) = &report.persistent_buffer {
        print_metric("persistentBuffer", persistent_buffer);
    }
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
            ("hot_link_stride", self.corpus.hot_link_stride),
            ("ref_stride", self.corpus.ref_stride),
            ("scheduled_stride", self.corpus.scheduled_stride),
            ("deadline_stride", self.corpus.deadline_stride),
            ("query_count", self.corpus.query_count),
            ("full_index", self.iterations.full_index),
            ("index_file", self.iterations.index_file),
            ("search_nodes", self.iterations.search_nodes),
            ("search_files", self.iterations.search_files),
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
            ("search_limit", self.iterations.search_limit),
            ("backlinks_limit", self.iterations.backlinks_limit),
            ("reflinks_limit", self.iterations.reflinks_limit),
            (
                "unlinked_references_limit",
                self.iterations.unlinked_references_limit,
            ),
            ("agenda_limit", self.iterations.agenda_limit),
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
    fn threshold_check_fails_when_limit_is_exceeded() {
        let error = check_threshold("search_nodes", 10.0, 5.0).unwrap_err();
        assert!(error.to_string().contains("search_nodes"));
    }
}
