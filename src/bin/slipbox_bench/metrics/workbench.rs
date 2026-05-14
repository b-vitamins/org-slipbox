use std::hint::black_box;

use anyhow::{Context, Result, bail};
use slipbox_core::{
    CorpusAuditKind, CorpusAuditParams, ImportWorkbenchPackParams, MarkReviewFindingParams,
    ReviewFindingRemediationPreviewParams, ReviewFindingStatus, ReviewRunDiffParams,
    ReviewRunIdParams, RunReviewRoutineParams, RunWorkflowParams, SaveCorpusAuditReviewParams,
    SaveWorkflowReviewParams, ValidateWorkbenchPackParams, WorkflowInputAssignment,
    WorkflowResolveTarget,
};

use crate::slipbox_bench::WorkbenchBench;
use crate::slipbox_bench::assertions::{
    assert_benchmark_workflow_execution_result, assert_benchmark_workflow_result,
    assert_pack_catalog_fixture, assert_pack_validation_fixture,
    assert_remediation_preview_fixture, assert_report_profile_rendering_fixture,
    assert_review_diff_fixture, assert_review_list_fixture, assert_review_routine_catalog_fixture,
    assert_review_show_fixture, assert_routine_run_fixture, assert_saved_audit_review_fixture,
    assert_workflow_catalog_fixture, benchmark_pack_manifest, invalid_benchmark_pack_manifest,
};
use crate::slipbox_bench::constants::{
    AUDIT_REVIEW_BASE_ID, AUDIT_REVIEW_TARGET_ID, BENCHMARK_PACK_AUDIT_ROUTINE_ID,
    BENCHMARK_PACK_ID, BENCHMARK_PACK_REPORT_PROFILE_ID, BENCHMARK_PACK_REPORT_ROUTINE_ID,
    WORKFLOW_BENCHMARK_ID, WORKFLOW_REVIEW_ID,
};
use crate::slipbox_bench::fixtures::{
    CorpusFixture, DeclarativeExtensionBenchmarkFixture, ReviewBenchmarkFixture,
};
use crate::slipbox_bench::profile::BenchmarkProfile;
use crate::slipbox_bench::report::{TimingReport, measure_iterations};

pub(crate) fn benchmark_workflow_catalog(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn benchmark_workflow_run(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn benchmark_corpus_audit(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn prepare_review_benchmark_fixtures(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn benchmark_review_list(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn benchmark_review_show(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn benchmark_review_diff(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn benchmark_review_mark(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn benchmark_audit_save_review(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn benchmark_workflow_save_review(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn benchmark_remediation_preview(
    workbench: &mut WorkbenchBench,
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

pub(crate) fn prepare_declarative_extension_benchmark_fixture(
    workbench: &mut WorkbenchBench,
) -> Result<DeclarativeExtensionBenchmarkFixture> {
    let pack = benchmark_pack_manifest(
        BENCHMARK_PACK_ID,
        "workflow/pack/benchmark-report-context",
        BENCHMARK_PACK_AUDIT_ROUTINE_ID,
        BENCHMARK_PACK_REPORT_ROUTINE_ID,
        BENCHMARK_PACK_REPORT_PROFILE_ID,
        "review/benchmark/pack-report",
    );
    let invalid_pack = invalid_benchmark_pack_manifest();
    let fixture = DeclarativeExtensionBenchmarkFixture {
        pack_id: pack.metadata.pack_id.clone(),
        audit_routine_id: BENCHMARK_PACK_AUDIT_ROUTINE_ID.to_owned(),
        report_routine_id: BENCHMARK_PACK_REPORT_ROUTINE_ID.to_owned(),
        report_profile_id: BENCHMARK_PACK_REPORT_PROFILE_ID.to_owned(),
        pack,
        invalid_pack,
    };

    let imported = workbench.import_workbench_pack(&ImportWorkbenchPackParams {
        pack: fixture.pack.clone(),
        overwrite: true,
    })?;
    if imported.pack.metadata.pack_id != fixture.pack_id {
        bail!(
            "pack import fixture returned unexpected pack id {}",
            imported.pack.metadata.pack_id
        );
    }
    if imported.pack.workflow_count == 0
        || imported.pack.review_routine_count < 2
        || imported.pack.report_profile_count == 0
    {
        bail!("pack import fixture omitted workflow, routine, or report profile assets");
    }

    assert_pack_catalog_fixture(&workbench.list_workbench_packs()?, &fixture)?;
    assert_review_routine_catalog_fixture(&workbench.list_review_routines()?, &fixture)?;
    assert_pack_validation_fixture(
        &workbench.validate_workbench_pack(&ValidateWorkbenchPackParams {
            pack: fixture.pack.clone(),
        })?,
        true,
    )?;
    assert_pack_validation_fixture(
        &workbench.validate_workbench_pack(&ValidateWorkbenchPackParams {
            pack: fixture.invalid_pack.clone(),
        })?,
        false,
    )?;

    Ok(fixture)
}

pub(crate) fn benchmark_pack_catalog(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &DeclarativeExtensionBenchmarkFixture,
) -> Result<TimingReport> {
    let sample = workbench.list_workbench_packs()?;
    assert_pack_catalog_fixture(&sample, fixture)?;
    measure_iterations(profile.iterations.pack_catalog, |_| {
        let result = workbench.list_workbench_packs()?;
        assert_pack_catalog_fixture(&result, fixture)?;
        black_box(result.packs.len());
        Ok(())
    })
}

pub(crate) fn benchmark_pack_validation(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &DeclarativeExtensionBenchmarkFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.pack_validation, |iteration| {
        let expect_valid = iteration % 2 == 0;
        let pack = if expect_valid {
            fixture.pack.clone()
        } else {
            fixture.invalid_pack.clone()
        };
        let result = workbench.validate_workbench_pack(&ValidateWorkbenchPackParams { pack })?;
        assert_pack_validation_fixture(&result, expect_valid)?;
        black_box(result.issues.len());
        Ok(())
    })
}

pub(crate) fn benchmark_routine_run(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &DeclarativeExtensionBenchmarkFixture,
) -> Result<TimingReport> {
    let params = RunReviewRoutineParams {
        routine_id: fixture.audit_routine_id.clone(),
        inputs: Vec::new(),
    };
    let sample = workbench.run_review_routine(&params)?;
    assert_routine_run_fixture(&sample)?;
    measure_iterations(profile.iterations.routine_run, |_| {
        let result = workbench.run_review_routine(&params)?;
        assert_routine_run_fixture(&result)?;
        black_box(result.result.routine.metadata.routine_id.len());
        Ok(())
    })
}

pub(crate) fn benchmark_report_profile_rendering(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &DeclarativeExtensionBenchmarkFixture,
    focus_node_key: &str,
) -> Result<TimingReport> {
    let params = RunReviewRoutineParams {
        routine_id: fixture.report_routine_id.clone(),
        inputs: vec![WorkflowInputAssignment {
            input_id: "focus".to_owned(),
            target: WorkflowResolveTarget::NodeKey {
                node_key: focus_node_key.to_owned(),
            },
        }],
    };
    let sample = workbench.run_review_routine(&params)?;
    assert_report_profile_rendering_fixture(&sample, fixture)?;
    measure_iterations(profile.iterations.report_profile_rendering, |_| {
        let result = workbench.run_review_routine(&params)?;
        assert_report_profile_rendering_fixture(&result, fixture)?;
        black_box(result.result.reports[0].lines.len());
        Ok(())
    })
}

pub(crate) fn benchmark_pack_import(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.pack_import, |iteration| {
        let suffix = format!("import-{iteration:04}");
        let pack = benchmark_pack_manifest(
            &format!("pack/benchmark/{suffix}"),
            &format!("workflow/pack/benchmark-report-context-{suffix}"),
            &format!("routine/pack/benchmark-audit-review-{suffix}"),
            &format!("routine/pack/benchmark-report-review-{suffix}"),
            &format!("profile/pack/benchmark-routine-detail-{suffix}"),
            &format!("review/benchmark/pack-report/{suffix}"),
        );
        let result = workbench.import_workbench_pack(&ImportWorkbenchPackParams {
            pack,
            overwrite: false,
        })?;
        if result.pack.metadata.pack_id != format!("pack/benchmark/{suffix}") {
            bail!(
                "pack import benchmark returned unexpected pack id {}",
                result.pack.metadata.pack_id
            );
        }
        if result.pack.workflow_count == 0
            || result.pack.review_routine_count < 2
            || result.pack.report_profile_count == 0
        {
            bail!("pack import benchmark omitted workflow, routine, or profile assets");
        }
        black_box(result.pack.review_routine_count);
        Ok(())
    })
}

pub(crate) fn benchmark_workflow_params(focus_node_key: &str) -> RunWorkflowParams {
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

pub(crate) fn benchmark_workflow_review_params(
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
