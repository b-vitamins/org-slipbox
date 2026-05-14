use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use slipbox_index::DiscoveryPolicy;

use crate::server;
use crate::slipbox_bench::corpus::generate_corpus;
use crate::slipbox_bench::fixtures::CorpusFixture;
use crate::slipbox_bench::metrics::{
    baseline_db_path, benchmark_agenda, benchmark_audit_save_review, benchmark_backlinks,
    benchmark_corpus_audit, benchmark_dedicated_buffer, benchmark_dedicated_exploration_buffer,
    benchmark_everyday_agenda_range, benchmark_everyday_capture_create,
    benchmark_everyday_daily_append, benchmark_everyday_file_sync, benchmark_everyday_graph_dot,
    benchmark_everyday_metadata_update, benchmark_everyday_node_search,
    benchmark_everyday_node_show, benchmark_everyday_occurrence_search, benchmark_forward_links,
    benchmark_full_index, benchmark_index_file, benchmark_node_at_point, benchmark_pack_catalog,
    benchmark_pack_import, benchmark_pack_validation, benchmark_persistent_buffer,
    benchmark_reflinks, benchmark_remediation_apply, benchmark_remediation_preview,
    benchmark_report_profile_rendering, benchmark_review_diff, benchmark_review_list,
    benchmark_review_mark, benchmark_review_show, benchmark_routine_run, benchmark_search_files,
    benchmark_search_nodes, benchmark_search_nodes_sorted, benchmark_search_occurrences,
    benchmark_slipbox_link_rewrite_apply, benchmark_slipbox_link_rewrite_preview,
    benchmark_structural_demote_file, benchmark_structural_extract_subtree,
    benchmark_structural_promote_file, benchmark_structural_refile_region,
    benchmark_structural_refile_subtree, benchmark_unlinked_references, benchmark_workflow_catalog,
    benchmark_workflow_run, benchmark_workflow_save_review, prepare_database,
    prepare_declarative_extension_benchmark_fixture, prepare_remediation_apply_benchmark_fixture,
    prepare_review_benchmark_fixtures, prepare_slipbox_link_rewrite_benchmark_fixture,
    prepare_structural_benchmark_fixture, select_dedicated_compare_fixture,
    select_dedicated_exploration_fixture,
};
use crate::slipbox_bench::profile::{
    BenchmarkProfile, default_report_path, load_profile, resolve_profile_path,
};
use crate::slipbox_bench::report::{
    BenchWorkspace, BenchmarkReport, CorpusSummary, enforce_thresholds, make_persistent_workspace,
    manifest_dir, print_summary, write_json,
};

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
pub(crate) fn main() -> Result<()> {
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

pub(crate) fn run_profile(
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
    let declarative_fixture = prepare_declarative_extension_benchmark_fixture(&mut workbench)?;
    let pack_catalog = benchmark_pack_catalog(&mut workbench, profile, &declarative_fixture)?;
    let pack_validation = benchmark_pack_validation(&mut workbench, profile, &declarative_fixture)?;
    let routine_run = benchmark_routine_run(&mut workbench, profile, &declarative_fixture)?;
    let report_profile_rendering = benchmark_report_profile_rendering(
        &mut workbench,
        profile,
        &declarative_fixture,
        &workflow_focus_anchor.node_key,
    )?;
    let pack_import = benchmark_pack_import(&mut workbench, profile)?;
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
    let everyday_node_show = benchmark_everyday_node_show(&mut workbench, profile, &hot_node)?;
    let everyday_node_search = benchmark_everyday_node_search(&mut workbench, profile, fixture)?;
    let everyday_occurrence_search =
        benchmark_everyday_occurrence_search(&mut workbench, profile, fixture)?;
    let everyday_agenda_range = benchmark_everyday_agenda_range(&mut workbench, profile)?;
    let everyday_graph_dot = benchmark_everyday_graph_dot(&mut workbench, profile, &hot_node)?;
    let everyday_file_sync = benchmark_everyday_file_sync(&mut workbench, profile, fixture)?;
    let everyday_capture_create = benchmark_everyday_capture_create(&mut workbench, profile)?;
    let everyday_daily_append = benchmark_everyday_daily_append(&mut workbench, profile)?;
    let everyday_metadata_update =
        benchmark_everyday_metadata_update(&mut workbench, profile, &hot_node)?;
    let structural_fixture =
        prepare_structural_benchmark_fixture(&mut workbench, profile, fixture)?;
    let structural_refile_subtree =
        benchmark_structural_refile_subtree(&mut workbench, profile, &structural_fixture)?;
    let structural_refile_region =
        benchmark_structural_refile_region(&mut workbench, profile, &structural_fixture)?;
    let structural_extract_subtree =
        benchmark_structural_extract_subtree(&mut workbench, profile, &structural_fixture)?;
    let structural_promote_file =
        benchmark_structural_promote_file(&mut workbench, profile, &structural_fixture)?;
    let structural_demote_file =
        benchmark_structural_demote_file(&mut workbench, profile, &structural_fixture)?;
    let remediation_apply_fixture =
        prepare_remediation_apply_benchmark_fixture(&mut workbench, profile, fixture)?;
    let remediation_apply =
        benchmark_remediation_apply(&mut workbench, profile, &remediation_apply_fixture)?;
    let link_rewrite_fixture =
        prepare_slipbox_link_rewrite_benchmark_fixture(&mut workbench, profile, fixture)?;
    let slipbox_link_rewrite_preview =
        benchmark_slipbox_link_rewrite_preview(&mut workbench, profile, &link_rewrite_fixture)?;
    let slipbox_link_rewrite_apply =
        benchmark_slipbox_link_rewrite_apply(&mut workbench, profile, &link_rewrite_fixture)?;

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
        pack_catalog,
        pack_validation,
        pack_import,
        routine_run,
        report_profile_rendering,
        everyday_file_sync,
        everyday_node_show,
        everyday_node_search,
        everyday_occurrence_search,
        everyday_agenda_range,
        everyday_graph_dot,
        everyday_capture_create,
        everyday_daily_append,
        everyday_metadata_update,
        structural_refile_subtree,
        structural_refile_region,
        structural_extract_subtree,
        structural_promote_file,
        structural_demote_file,
        remediation_apply,
        slipbox_link_rewrite_preview,
        slipbox_link_rewrite_apply,
    })
}
