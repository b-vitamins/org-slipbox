use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use slipbox_core::{
    AuditRemediationApplyAction, AuditRemediationPreviewPayload, CorpusAuditKind, ExplorationLens,
    IndexFileParams, NodeFromIdParams, NodeRecord, ReportJsonlLineKind, ReportProfileMetadata,
    ReportProfileMode, ReportProfileSpec, ReportProfileSubject, ReviewRoutineMetadata,
    ReviewRoutineSaveReviewPolicy, ReviewRoutineSource, ReviewRoutineSpec,
    StructuralWriteIndexRefreshStatus, StructuralWriteOperationKind, StructuralWriteReport,
    StructuralWriteResult, WorkbenchPackCompatibility, WorkbenchPackIssueKind,
    WorkbenchPackManifest, WorkbenchPackMetadata, WorkflowExecutionResult, WorkflowExploreFocus,
    WorkflowInputKind, WorkflowInputSpec, WorkflowMetadata,
    WorkflowResolveTarget as WorkflowSpecResolveTarget, WorkflowSpec, WorkflowSpecCompatibility,
    WorkflowStepPayload, WorkflowStepReportPayload, WorkflowStepSpec,
};

use crate::slipbox_bench::WorkbenchBench;
use crate::slipbox_bench::constants::{BENCHMARK_PACK_AUDIT_ROUTINE_ID, WORKFLOW_BENCHMARK_ID};
use crate::slipbox_bench::fixtures::{
    CorpusFixture, DeclarativeExtensionBenchmarkFixture, ReviewBenchmarkFixture,
};

pub(crate) fn assert_workflow_catalog_fixture(
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

pub(crate) fn assert_benchmark_workflow_result(
    workflow: &slipbox_core::RunWorkflowResult,
    fixture: &CorpusFixture,
    focus_node_key: &str,
) -> Result<()> {
    assert_benchmark_workflow_execution_result(&workflow.result, fixture, focus_node_key)
}

pub(crate) fn assert_benchmark_workflow_execution_result(
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

pub(crate) fn assert_saved_audit_review_fixture(
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

pub(crate) fn assert_review_list_fixture(
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

pub(crate) fn assert_review_show_fixture(result: &slipbox_core::ReviewRunResult) -> Result<()> {
    if result.review.findings.is_empty() {
        bail!("review show benchmark returned a review with no findings");
    }
    if result.review.validation_error().is_some() {
        bail!("review show benchmark returned an invalid review");
    }
    Ok(())
}

pub(crate) fn assert_review_diff_fixture(result: &slipbox_core::ReviewRunDiffResult) -> Result<()> {
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

pub(crate) fn assert_remediation_preview_fixture(
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

pub(crate) fn assert_structural_write_fixture(
    report: &StructuralWriteReport,
    operation: StructuralWriteOperationKind,
) -> Result<()> {
    if report.operation != operation {
        bail!(
            "structural benchmark returned {:?}, expected {:?}",
            report.operation,
            operation
        );
    }
    if let Some(error) = report.validation_error() {
        bail!("structural benchmark returned invalid report: {error}");
    }
    if report.index_refresh != StructuralWriteIndexRefreshStatus::Refreshed {
        bail!("structural benchmark returned a non-refreshed index status");
    }
    if report.affected_files.changed_files.is_empty() {
        bail!("structural benchmark returned no changed files");
    }
    match operation {
        StructuralWriteOperationKind::RefileRegion => {
            if report.result.is_some() {
                bail!("refile-region benchmark unexpectedly returned a node result");
            }
        }
        StructuralWriteOperationKind::DemoteFile => match &report.result {
            Some(StructuralWriteResult::Anchor { anchor }) if !anchor.node_key.is_empty() => {}
            other => bail!("demote-file benchmark returned unexpected result {other:?}"),
        },
        _ => match &report.result {
            Some(StructuralWriteResult::Node { node }) if !node.node_key.is_empty() => {}
            other => bail!(
                "structural benchmark for {:?} returned unexpected result {other:?}",
                operation
            ),
        },
    }
    Ok(())
}

pub(crate) fn assert_slipbox_link_rewrite_preview_fixture(
    result: &slipbox_core::SlipboxLinkRewritePreviewResult,
) -> Result<()> {
    if let Some(error) = result.preview.validation_error() {
        bail!("slipbox link rewrite preview benchmark returned invalid preview: {error}");
    }
    if result.preview.rewrites.is_empty() {
        bail!("slipbox link rewrite preview benchmark returned no rewrites");
    }
    Ok(())
}

pub(crate) fn unlink_dangling_link_action_from_preview(
    preview: &slipbox_core::ReviewFindingRemediationPreview,
    expected_missing_id: &str,
) -> Result<AuditRemediationApplyAction> {
    match &preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview,
            ..
        } if missing_explicit_id == expected_missing_id => {
            let replacement_text = org_link_description(preview)
                .context("remediation apply benchmark could not derive link label")?;
            Ok(AuditRemediationApplyAction::UnlinkDanglingLink {
                source_node_key: source.node_key.clone(),
                missing_explicit_id: missing_explicit_id.clone(),
                file_path: file_path.clone(),
                line: *line,
                column: *column,
                preview: preview.clone(),
                replacement_text,
            })
        }
        other => bail!(
            "remediation apply benchmark expected dangling-link preview for {expected_missing_id}, got {other:?}"
        ),
    }
}

pub(crate) fn org_link_description(preview: &str) -> Option<String> {
    let link_start = preview.find("[[")?;
    let inner_start = link_start + 2;
    let inner_end = preview[inner_start..].find("]]")? + inner_start;
    let inner = &preview[inner_start..inner_end];
    let (_, description) = inner.split_once("][")?;
    (!description.is_empty()).then(|| description.to_owned())
}

pub(crate) fn write_indexed_bench_file(
    workbench: &mut WorkbenchBench,
    root: &Path,
    relative_path: &str,
    source: &str,
) -> Result<()> {
    let absolute_path = root.join(relative_path);
    if let Some(parent) = absolute_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&absolute_path, source).with_context(|| {
        format!(
            "failed to write benchmark fixture {}",
            absolute_path.display()
        )
    })?;
    let result = workbench.index_file(&IndexFileParams {
        file_path: relative_path.to_owned(),
    })?;
    if result.file_path != relative_path {
        bail!(
            "benchmark fixture indexed {}, expected {}",
            result.file_path,
            relative_path
        );
    }
    Ok(())
}

pub(crate) fn indexed_node_from_id(
    workbench: &mut WorkbenchBench,
    explicit_id: &str,
) -> Result<NodeRecord> {
    workbench
        .node_from_id(&NodeFromIdParams {
            id: explicit_id.to_owned(),
        })?
        .with_context(|| format!("benchmark fixture node {explicit_id} was not indexed"))
}

pub(crate) fn assert_pack_catalog_fixture(
    result: &slipbox_core::ListWorkbenchPacksResult,
    fixture: &DeclarativeExtensionBenchmarkFixture,
) -> Result<()> {
    let pack = result
        .packs
        .iter()
        .find(|pack| pack.metadata.pack_id == fixture.pack_id)
        .context("pack catalog benchmark omitted imported benchmark pack")?;
    if pack.workflow_count == 0 || pack.review_routine_count < 2 || pack.report_profile_count == 0 {
        bail!("pack catalog benchmark returned an incomplete benchmark pack summary");
    }
    if !result.issues.is_empty() {
        bail!(
            "pack catalog benchmark expected no catalog issues before import churn, found {}",
            result.issues.len()
        );
    }
    Ok(())
}

pub(crate) fn assert_review_routine_catalog_fixture(
    result: &slipbox_core::ListReviewRoutinesResult,
    fixture: &DeclarativeExtensionBenchmarkFixture,
) -> Result<()> {
    for routine_id in [&fixture.audit_routine_id, &fixture.report_routine_id] {
        if !result
            .routines
            .iter()
            .any(|routine| routine.metadata.routine_id == *routine_id)
        {
            bail!("routine catalog benchmark omitted imported routine {routine_id}");
        }
    }
    if !result.issues.is_empty() {
        bail!(
            "routine catalog benchmark expected no catalog issues before import churn, found {}",
            result.issues.len()
        );
    }
    Ok(())
}

pub(crate) fn assert_pack_validation_fixture(
    result: &slipbox_core::ValidateWorkbenchPackResult,
    expect_valid: bool,
) -> Result<()> {
    if result.valid != expect_valid {
        bail!(
            "pack validation benchmark expected valid={expect_valid}, got {}",
            result.valid
        );
    }
    if expect_valid {
        if result.pack.is_none() || !result.issues.is_empty() {
            bail!("pack validation benchmark returned issues for a valid pack");
        }
    } else if !result
        .issues
        .iter()
        .any(|issue| issue.kind == WorkbenchPackIssueKind::MissingWorkflowReference)
    {
        bail!("pack validation benchmark invalid fixture did not report a missing workflow");
    }
    Ok(())
}

pub(crate) fn assert_routine_run_fixture(
    result: &slipbox_core::RunReviewRoutineResult,
) -> Result<()> {
    if result.result.routine.metadata.routine_id != BENCHMARK_PACK_AUDIT_ROUTINE_ID {
        bail!(
            "routine benchmark returned unexpected routine id {}",
            result.result.routine.metadata.routine_id
        );
    }
    match &result.result.source {
        slipbox_core::ReviewRoutineSourceExecutionResult::Audit { result } => {
            if result.audit != CorpusAuditKind::DuplicateTitles || result.entries.is_empty() {
                bail!("routine benchmark audit source did not return duplicate-title entries");
            }
        }
        other => {
            bail!("routine benchmark returned unexpected source kind {other:?}");
        }
    }
    if result.result.saved_review.is_some() || !result.result.reports.is_empty() {
        bail!("routine benchmark expected the audit fixture to avoid review/report side effects");
    }
    Ok(())
}

pub(crate) fn assert_report_profile_rendering_fixture(
    result: &slipbox_core::RunReviewRoutineResult,
    fixture: &DeclarativeExtensionBenchmarkFixture,
) -> Result<()> {
    if result.result.routine.metadata.routine_id != fixture.report_routine_id {
        bail!(
            "report-profile benchmark returned unexpected routine id {}",
            result.result.routine.metadata.routine_id
        );
    }
    if result.result.saved_review.is_none() {
        bail!("report-profile benchmark did not save a review for report rendering");
    }
    let report = result
        .result
        .reports
        .iter()
        .find(|report| report.profile.metadata.profile_id == fixture.report_profile_id)
        .context("report-profile benchmark omitted the configured report profile")?;
    if report.lines.is_empty() {
        bail!("report-profile benchmark rendered no report lines");
    }
    for expected in [
        ReportJsonlLineKind::Routine,
        ReportJsonlLineKind::Step,
        ReportJsonlLineKind::Review,
        ReportJsonlLineKind::Finding,
    ] {
        if !report.lines.iter().any(|line| line.line_kind() == expected) {
            bail!(
                "report-profile benchmark omitted expected {} lines",
                expected.label()
            );
        }
    }
    Ok(())
}

pub(crate) fn benchmark_pack_manifest(
    pack_id: &str,
    workflow_id: &str,
    audit_routine_id: &str,
    report_routine_id: &str,
    report_profile_id: &str,
    report_review_id: &str,
) -> WorkbenchPackManifest {
    WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: pack_id.to_owned(),
            title: "Benchmark Declarative Extension Pack".to_owned(),
            summary: Some(
                "Imported fixture covering workflows, routines, and report profiles.".to_owned(),
            ),
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: vec![benchmark_pack_workflow_spec(workflow_id)],
        review_routines: vec![
            benchmark_pack_audit_routine(audit_routine_id),
            benchmark_pack_report_routine(
                report_routine_id,
                workflow_id,
                report_profile_id,
                report_review_id,
            ),
        ],
        report_profiles: vec![benchmark_pack_report_profile(report_profile_id)],
        entrypoint_routine_ids: vec![audit_routine_id.to_owned(), report_routine_id.to_owned()],
    }
}

pub(crate) fn invalid_benchmark_pack_manifest() -> WorkbenchPackManifest {
    let routine_id = "routine/pack/benchmark-invalid-missing-workflow";
    WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: "pack/benchmark/invalid-missing-workflow".to_owned(),
            title: "Invalid Benchmark Pack".to_owned(),
            summary: Some("Invalid fixture for pack validation benchmark paths.".to_owned()),
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: Vec::new(),
        review_routines: vec![ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: routine_id.to_owned(),
                title: "Invalid Missing Workflow Routine".to_owned(),
                summary: Some("References a workflow that is intentionally absent.".to_owned()),
            },
            source: ReviewRoutineSource::Workflow {
                workflow_id: "workflow/pack/benchmark-missing".to_owned(),
            },
            inputs: vec![WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus target".to_owned(),
                summary: Some("Focus target for the missing workflow.".to_owned()),
                kind: WorkflowInputKind::FocusTarget,
            }],
            save_review: ReviewRoutineSaveReviewPolicy::default(),
            compare: None,
            report_profile_ids: Vec::new(),
        }],
        report_profiles: Vec::new(),
        entrypoint_routine_ids: vec![routine_id.to_owned()],
    }
}

pub(crate) fn benchmark_pack_workflow_spec(workflow_id: &str) -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: workflow_id.to_owned(),
            title: "Pack Benchmark Context Review".to_owned(),
            summary: Some("Pack-provided workflow for report-profile benchmark paths.".to_owned()),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus target".to_owned(),
            summary: Some("Note or anchor target to review.".to_owned()),
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
                step_id: "pack-review-refs".to_owned(),
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
                step_id: "pack-review-time".to_owned(),
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

pub(crate) fn benchmark_pack_audit_routine(routine_id: &str) -> ReviewRoutineSpec {
    ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: routine_id.to_owned(),
            title: "Pack Duplicate Title Audit".to_owned(),
            summary: Some("Pack-provided audit routine for benchmark execution.".to_owned()),
        },
        source: ReviewRoutineSource::Audit {
            audit: CorpusAuditKind::DuplicateTitles,
            limit: 25,
        },
        inputs: Vec::new(),
        save_review: ReviewRoutineSaveReviewPolicy {
            enabled: false,
            review_id: None,
            title: None,
            summary: None,
            overwrite: false,
        },
        compare: None,
        report_profile_ids: Vec::new(),
    }
}

pub(crate) fn benchmark_pack_report_routine(
    routine_id: &str,
    workflow_id: &str,
    report_profile_id: &str,
    review_id: &str,
) -> ReviewRoutineSpec {
    ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: routine_id.to_owned(),
            title: "Pack Report Profile Routine".to_owned(),
            summary: Some(
                "Pack-provided workflow routine with report-profile rendering.".to_owned(),
            ),
        },
        source: ReviewRoutineSource::Workflow {
            workflow_id: workflow_id.to_owned(),
        },
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus target".to_owned(),
            summary: Some("Note or anchor target to review.".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        save_review: ReviewRoutineSaveReviewPolicy {
            enabled: true,
            review_id: Some(review_id.to_owned()),
            title: Some("Pack Benchmark Report Review".to_owned()),
            summary: Some(
                "Saved review fixture for report-profile benchmark rendering.".to_owned(),
            ),
            overwrite: true,
        },
        compare: None,
        report_profile_ids: vec![report_profile_id.to_owned()],
    }
}

pub(crate) fn benchmark_pack_report_profile(profile_id: &str) -> ReportProfileSpec {
    ReportProfileSpec {
        metadata: ReportProfileMetadata {
            profile_id: profile_id.to_owned(),
            title: "Pack Routine Detail Report".to_owned(),
            summary: Some("Detail report for routine, step, review, and finding lines.".to_owned()),
        },
        subjects: vec![
            ReportProfileSubject::Routine,
            ReportProfileSubject::Workflow,
            ReportProfileSubject::Review,
        ],
        mode: ReportProfileMode::Detail,
        status_filters: None,
        diff_buckets: None,
        jsonl_line_kinds: Some(vec![
            ReportJsonlLineKind::Routine,
            ReportJsonlLineKind::Step,
            ReportJsonlLineKind::Review,
            ReportJsonlLineKind::Finding,
        ]),
    }
}
