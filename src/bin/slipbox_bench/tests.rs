use anyhow::{Context, Result};
use slipbox_core::{
    CorpusAuditKind, CorpusAuditParams, ExplorationLens, ExplorationSectionKind,
    ReviewFindingRemediationPreviewParams, ReviewRunDiffParams, ReviewRunIdParams,
};
use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};
use slipbox_store::Database;

use crate::slipbox_bench::WorkbenchBench;
use crate::slipbox_bench::assertions::{
    assert_benchmark_workflow_result, assert_remediation_preview_fixture,
    assert_review_diff_fixture, assert_review_list_fixture, assert_review_show_fixture,
    assert_workflow_catalog_fixture,
};
use crate::slipbox_bench::corpus::{assert_expected_counts, generate_corpus};
use crate::slipbox_bench::metrics::workbench::benchmark_workflow_params;
use crate::slipbox_bench::metrics::{
    baseline_db_path, benchmark_everyday_agenda_range, benchmark_everyday_capture_create,
    benchmark_everyday_daily_append, benchmark_everyday_file_sync, benchmark_everyday_graph_dot,
    benchmark_everyday_metadata_update, benchmark_everyday_node_search,
    benchmark_everyday_node_show, benchmark_everyday_occurrence_search, benchmark_pack_catalog,
    benchmark_pack_import, benchmark_pack_validation, benchmark_remediation_apply,
    benchmark_report_profile_rendering, benchmark_routine_run,
    benchmark_slipbox_link_rewrite_apply, benchmark_slipbox_link_rewrite_preview,
    benchmark_structural_demote_file, benchmark_structural_extract_subtree,
    benchmark_structural_promote_file, benchmark_structural_refile_region,
    benchmark_structural_refile_subtree, prepare_declarative_extension_benchmark_fixture,
    prepare_remediation_apply_benchmark_fixture, prepare_review_benchmark_fixtures,
    prepare_slipbox_link_rewrite_benchmark_fixture, prepare_structural_benchmark_fixture,
    select_dedicated_exploration_fixture,
};
use crate::slipbox_bench::profile::{
    BenchmarkProfile, CorpusConfig, IterationConfig, ThresholdConfig,
};
use crate::slipbox_bench::report::{TimingReport, check_threshold};

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
    let (lens, result) = select_dedicated_exploration_fixture(&database, &exploration_node, 20)?;
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

    let mut workbench = WorkbenchBench::new(
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
            pack_catalog: 1,
            pack_validation: 2,
            pack_import: 1,
            routine_run: 1,
            report_profile_rendering: 1,
            everyday_file_sync: 1,
            everyday_node_show: 1,
            everyday_node_search: 1,
            everyday_occurrence_search: 1,
            everyday_agenda_range: 1,
            everyday_graph_dot: 1,
            everyday_capture_create: 1,
            everyday_daily_append: 1,
            everyday_metadata_update: 1,
            structural_refile_subtree: 1,
            structural_refile_region: 1,
            structural_extract_subtree: 1,
            structural_promote_file: 1,
            structural_demote_file: 1,
            remediation_apply: 1,
            slipbox_link_rewrite_preview: 1,
            slipbox_link_rewrite_apply: 1,
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
            pack_catalog_p95_ms: 1.0,
            pack_validation_p95_ms: 1.0,
            pack_import_p95_ms: 1.0,
            routine_run_p95_ms: 1.0,
            report_profile_rendering_p95_ms: 1.0,
            everyday_file_sync_p95_ms: 1.0,
            everyday_node_show_p95_ms: 1.0,
            everyday_node_search_p95_ms: 1.0,
            everyday_occurrence_search_p95_ms: 1.0,
            everyday_agenda_range_p95_ms: 1.0,
            everyday_graph_dot_p95_ms: 1.0,
            everyday_capture_create_p95_ms: 1.0,
            everyday_daily_append_p95_ms: 1.0,
            everyday_metadata_update_p95_ms: 1.0,
            structural_refile_subtree_p95_ms: 1.0,
            structural_refile_region_p95_ms: 1.0,
            structural_extract_subtree_p95_ms: 1.0,
            structural_promote_file_p95_ms: 1.0,
            structural_demote_file_p95_ms: 1.0,
            remediation_apply_p95_ms: 1.0,
            slipbox_link_rewrite_preview_p95_ms: 1.0,
            slipbox_link_rewrite_apply_p95_ms: 1.0,
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
fn generated_corpus_guarantees_everyday_operation_benchmark_fixtures() -> Result<()> {
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
    let hot_node = database
        .node_from_id(&fixture.hot_node_id)?
        .context("hot node should exist")?;
    let mut workbench = WorkbenchBench::new(
        fixture.root.clone(),
        baseline_db_path(&fixture),
        fixture.workflow_dirs.clone(),
        DiscoveryPolicy::default(),
    )?;
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
            pack_catalog: 1,
            pack_validation: 2,
            pack_import: 1,
            routine_run: 1,
            report_profile_rendering: 1,
            everyday_file_sync: 1,
            everyday_node_show: 1,
            everyday_node_search: 1,
            everyday_occurrence_search: 1,
            everyday_agenda_range: 1,
            everyday_graph_dot: 1,
            everyday_capture_create: 1,
            everyday_daily_append: 1,
            everyday_metadata_update: 1,
            structural_refile_subtree: 1,
            structural_refile_region: 1,
            structural_extract_subtree: 1,
            structural_promote_file: 1,
            structural_demote_file: 1,
            remediation_apply: 1,
            slipbox_link_rewrite_preview: 1,
            slipbox_link_rewrite_apply: 1,
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
            pack_catalog_p95_ms: 1.0,
            pack_validation_p95_ms: 1.0,
            pack_import_p95_ms: 1.0,
            routine_run_p95_ms: 1.0,
            report_profile_rendering_p95_ms: 1.0,
            everyday_file_sync_p95_ms: 1.0,
            everyday_node_show_p95_ms: 1.0,
            everyday_node_search_p95_ms: 1.0,
            everyday_occurrence_search_p95_ms: 1.0,
            everyday_agenda_range_p95_ms: 1.0,
            everyday_graph_dot_p95_ms: 1.0,
            everyday_capture_create_p95_ms: 1.0,
            everyday_daily_append_p95_ms: 1.0,
            everyday_metadata_update_p95_ms: 1.0,
            structural_refile_subtree_p95_ms: 1.0,
            structural_refile_region_p95_ms: 1.0,
            structural_extract_subtree_p95_ms: 1.0,
            structural_promote_file_p95_ms: 1.0,
            structural_demote_file_p95_ms: 1.0,
            remediation_apply_p95_ms: 1.0,
            slipbox_link_rewrite_preview_p95_ms: 1.0,
            slipbox_link_rewrite_apply_p95_ms: 1.0,
        },
    };

    assert_eq!(
        benchmark_everyday_node_show(&mut workbench, &profile, &hot_node)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_everyday_node_search(&mut workbench, &profile, &fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_everyday_occurrence_search(&mut workbench, &profile, &fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_everyday_agenda_range(&mut workbench, &profile)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_everyday_graph_dot(&mut workbench, &profile, &hot_node)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_everyday_file_sync(&mut workbench, &profile, &fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_everyday_capture_create(&mut workbench, &profile)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_everyday_daily_append(&mut workbench, &profile)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_everyday_metadata_update(&mut workbench, &profile, &hot_node)?
            .samples_ms
            .len(),
        1
    );

    Ok(())
}

#[test]
fn generated_corpus_guarantees_structural_write_benchmark_fixtures() -> Result<()> {
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
    let mut workbench = WorkbenchBench::new(
        fixture.root.clone(),
        baseline_db_path(&fixture),
        fixture.workflow_dirs.clone(),
        DiscoveryPolicy::default(),
    )?;
    let profile = one_iteration_benchmark_profile(config);

    let structural_fixture =
        prepare_structural_benchmark_fixture(&mut workbench, &profile, &fixture)?;
    assert_eq!(
        benchmark_structural_refile_subtree(&mut workbench, &profile, &structural_fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_structural_refile_region(&mut workbench, &profile, &structural_fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_structural_extract_subtree(&mut workbench, &profile, &structural_fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_structural_promote_file(&mut workbench, &profile, &structural_fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_structural_demote_file(&mut workbench, &profile, &structural_fixture)?
            .samples_ms
            .len(),
        1
    );

    let remediation_fixture =
        prepare_remediation_apply_benchmark_fixture(&mut workbench, &profile, &fixture)?;
    assert_eq!(
        benchmark_remediation_apply(&mut workbench, &profile, &remediation_fixture)?
            .samples_ms
            .len(),
        1
    );

    let link_rewrite_fixture =
        prepare_slipbox_link_rewrite_benchmark_fixture(&mut workbench, &profile, &fixture)?;
    assert_eq!(
        benchmark_slipbox_link_rewrite_preview(&mut workbench, &profile, &link_rewrite_fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_slipbox_link_rewrite_apply(&mut workbench, &profile, &link_rewrite_fixture)?
            .samples_ms
            .len(),
        1
    );

    Ok(())
}

#[test]
fn generated_corpus_guarantees_declarative_extension_benchmark_fixtures() -> Result<()> {
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

    let mut workbench = WorkbenchBench::new(
        fixture.root.clone(),
        baseline_db_path(&fixture),
        fixture.workflow_dirs.clone(),
        DiscoveryPolicy::default(),
    )?;
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
            pack_catalog: 1,
            pack_validation: 2,
            pack_import: 1,
            routine_run: 1,
            report_profile_rendering: 1,
            everyday_file_sync: 1,
            everyday_node_show: 1,
            everyday_node_search: 1,
            everyday_occurrence_search: 1,
            everyday_agenda_range: 1,
            everyday_graph_dot: 1,
            everyday_capture_create: 1,
            everyday_daily_append: 1,
            everyday_metadata_update: 1,
            structural_refile_subtree: 1,
            structural_refile_region: 1,
            structural_extract_subtree: 1,
            structural_promote_file: 1,
            structural_demote_file: 1,
            remediation_apply: 1,
            slipbox_link_rewrite_preview: 1,
            slipbox_link_rewrite_apply: 1,
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
            pack_catalog_p95_ms: 1.0,
            pack_validation_p95_ms: 1.0,
            pack_import_p95_ms: 1.0,
            routine_run_p95_ms: 1.0,
            report_profile_rendering_p95_ms: 1.0,
            everyday_file_sync_p95_ms: 1.0,
            everyday_node_show_p95_ms: 1.0,
            everyday_node_search_p95_ms: 1.0,
            everyday_occurrence_search_p95_ms: 1.0,
            everyday_agenda_range_p95_ms: 1.0,
            everyday_graph_dot_p95_ms: 1.0,
            everyday_capture_create_p95_ms: 1.0,
            everyday_daily_append_p95_ms: 1.0,
            everyday_metadata_update_p95_ms: 1.0,
            structural_refile_subtree_p95_ms: 1.0,
            structural_refile_region_p95_ms: 1.0,
            structural_extract_subtree_p95_ms: 1.0,
            structural_promote_file_p95_ms: 1.0,
            structural_demote_file_p95_ms: 1.0,
            remediation_apply_p95_ms: 1.0,
            slipbox_link_rewrite_preview_p95_ms: 1.0,
            slipbox_link_rewrite_apply_p95_ms: 1.0,
        },
    };

    let declarative_fixture = prepare_declarative_extension_benchmark_fixture(&mut workbench)?;
    assert_eq!(
        benchmark_pack_catalog(&mut workbench, &profile, &declarative_fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_pack_validation(&mut workbench, &profile, &declarative_fixture)?
            .samples_ms
            .len(),
        2
    );
    assert_eq!(
        benchmark_routine_run(&mut workbench, &profile, &declarative_fixture)?
            .samples_ms
            .len(),
        1
    );
    assert_eq!(
        benchmark_report_profile_rendering(
            &mut workbench,
            &profile,
            &declarative_fixture,
            &workflow_focus_anchor.node_key,
        )?
        .samples_ms
        .len(),
        1
    );
    assert_eq!(
        benchmark_pack_import(&mut workbench, &profile)?
            .samples_ms
            .len(),
        1
    );

    Ok(())
}

fn one_iteration_benchmark_profile(config: CorpusConfig) -> BenchmarkProfile {
    BenchmarkProfile {
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
            pack_catalog: 1,
            pack_validation: 2,
            pack_import: 1,
            routine_run: 1,
            report_profile_rendering: 1,
            everyday_file_sync: 1,
            everyday_node_show: 1,
            everyday_node_search: 1,
            everyday_occurrence_search: 1,
            everyday_agenda_range: 1,
            everyday_graph_dot: 1,
            everyday_capture_create: 1,
            everyday_daily_append: 1,
            everyday_metadata_update: 1,
            structural_refile_subtree: 1,
            structural_refile_region: 1,
            structural_extract_subtree: 1,
            structural_promote_file: 1,
            structural_demote_file: 1,
            remediation_apply: 1,
            slipbox_link_rewrite_preview: 1,
            slipbox_link_rewrite_apply: 1,
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
            pack_catalog_p95_ms: 1.0,
            pack_validation_p95_ms: 1.0,
            pack_import_p95_ms: 1.0,
            routine_run_p95_ms: 1.0,
            report_profile_rendering_p95_ms: 1.0,
            everyday_file_sync_p95_ms: 1.0,
            everyday_node_show_p95_ms: 1.0,
            everyday_node_search_p95_ms: 1.0,
            everyday_occurrence_search_p95_ms: 1.0,
            everyday_agenda_range_p95_ms: 1.0,
            everyday_graph_dot_p95_ms: 1.0,
            everyday_capture_create_p95_ms: 1.0,
            everyday_daily_append_p95_ms: 1.0,
            everyday_metadata_update_p95_ms: 1.0,
            structural_refile_subtree_p95_ms: 1.0,
            structural_refile_region_p95_ms: 1.0,
            structural_extract_subtree_p95_ms: 1.0,
            structural_promote_file_p95_ms: 1.0,
            structural_demote_file_p95_ms: 1.0,
            remediation_apply_p95_ms: 1.0,
            slipbox_link_rewrite_preview_p95_ms: 1.0,
            slipbox_link_rewrite_apply_p95_ms: 1.0,
        },
    }
}

#[test]
fn threshold_check_fails_when_limit_is_exceeded() {
    let error = check_threshold("search_nodes", 10.0, 5.0).unwrap_err();
    assert!(error.to_string().contains("search_nodes"));
}
