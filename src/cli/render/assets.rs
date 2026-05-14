use super::{
    explorations::{
        render_compare_result, render_comparison_group, render_executed_exploration_artifact,
        render_exploration_lens, render_explore_result, render_node_identity,
        render_saved_artifact_summary,
    },
    notes::render_node_summary,
    reviews::{
        push_indented, render_corpus_audit_kind, render_corpus_audit_result,
        render_review_audit_entry, render_review_diff, render_review_finding,
        render_review_finding_status, render_review_status_counts, render_saved_review_summary,
    },
};
use slipbox_core::{
    AppliedReportProfile, ListReviewRoutinesResult, ListWorkbenchPacksResult, ListWorkflowsResult,
    NoteComparisonGroup, ReviewRoutineCompareResult, ReviewRoutineExecutionResult,
    ReviewRoutineReportLine, ReviewRoutineSource, ReviewRoutineSourceExecutionResult,
    ReviewRoutineSpec, ReviewRoutineSummary, ValidateWorkbenchPackResult, WorkbenchPackIssue,
    WorkbenchPackManifest, WorkflowArtifactSaveSource, WorkflowCatalogIssue,
    WorkflowExecutionResult, WorkflowExploreFocus, WorkflowInputKind, WorkflowInputSpec,
    WorkflowResolveTarget, WorkflowSpec, WorkflowStepPayload, WorkflowStepReport,
    WorkflowStepReportPayload, WorkflowStepSpec,
};

pub(crate) fn render_workflow_list(result: &ListWorkflowsResult) -> String {
    let mut output = String::new();
    if result.workflows.is_empty() {
        output.push_str("(none)\n");
    } else {
        for workflow in &result.workflows {
            output.push_str(&format!(
                "- {} [{}]\n",
                workflow.metadata.title, workflow.metadata.workflow_id
            ));
            output.push_str(&format!("  steps: {}\n", workflow.step_count));
            if let Some(summary) = &workflow.metadata.summary {
                output.push_str(&format!("  summary: {summary}\n"));
            }
        }
    }

    if !result.issues.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        render_workflow_catalog_issues(&mut output, &result.issues);
    }

    output
}

pub(crate) fn render_workflow_catalog_issues(output: &mut String, issues: &[WorkflowCatalogIssue]) {
    output.push_str("[issues]\n");
    for issue in issues {
        output.push_str(&format!("- path: {}\n", issue.path));
        output.push_str(&format!("  kind: {}\n", issue.kind.label()));
        if let Some(pack_id) = &issue.pack_id {
            output.push_str(&format!("  pack id: {pack_id}\n"));
        }
        if let Some(workflow_id) = &issue.workflow_id {
            output.push_str(&format!("  workflow id: {workflow_id}\n"));
        }
        if let Some(routine_id) = &issue.routine_id {
            output.push_str(&format!("  routine id: {routine_id}\n"));
        }
        if let Some(profile_id) = &issue.profile_id {
            output.push_str(&format!("  profile id: {profile_id}\n"));
        }
        output.push_str(&format!("  message: {}\n", issue.message));
    }
}

pub(crate) fn render_workbench_pack_list(result: &ListWorkbenchPacksResult) -> String {
    let mut output = String::new();
    if result.packs.is_empty() {
        output.push_str("(none)\n");
    } else {
        for pack in &result.packs {
            output.push_str(&format!(
                "- {} [{}]\n",
                pack.metadata.title, pack.metadata.pack_id
            ));
            output.push_str(&format!(
                "  workflows/routines/profiles: {}/{}/{}\n",
                pack.workflow_count, pack.review_routine_count, pack.report_profile_count
            ));
            output.push_str(&format!(
                "  compatibility: workbench-pack/v{}\n",
                pack.compatibility.version
            ));
            if !pack.entrypoint_routine_ids.is_empty() {
                output.push_str(&format!(
                    "  entrypoint routines: {}\n",
                    pack.entrypoint_routine_ids.join(", ")
                ));
            }
            if let Some(summary) = &pack.metadata.summary {
                output.push_str(&format!("  summary: {summary}\n"));
            }
        }
    }

    if !result.issues.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        render_workflow_catalog_issues(&mut output, &result.issues);
    }

    output
}

pub(crate) fn render_workbench_pack_manifest(pack: &WorkbenchPackManifest) -> String {
    let mut output = String::new();
    output.push_str(&format!("pack id: {}\n", pack.metadata.pack_id));
    output.push_str(&format!("title: {}\n", pack.metadata.title));
    output.push_str(&format!(
        "compatibility: workbench-pack/v{}\n",
        pack.compatibility.version
    ));
    if let Some(summary) = &pack.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    output.push_str(&format!("workflows: {}\n", pack.workflows.len()));
    output.push_str(&format!(
        "review routines: {}\n",
        pack.review_routines.len()
    ));
    output.push_str(&format!(
        "report profiles: {}\n",
        pack.report_profiles.len()
    ));
    if !pack.entrypoint_routine_ids.is_empty() {
        output.push_str(&format!(
            "entrypoint routines: {}\n",
            pack.entrypoint_routine_ids.join(", ")
        ));
    }

    if !pack.workflows.is_empty() {
        output.push_str("\n[workflows]\n");
        for workflow in &pack.workflows {
            output.push_str(&format!(
                "- {} [{}]\n",
                workflow.metadata.title, workflow.metadata.workflow_id
            ));
            output.push_str(&format!("  steps: {}\n", workflow.steps.len()));
        }
    }
    if !pack.review_routines.is_empty() {
        output.push_str("\n[review-routines]\n");
        for routine in &pack.review_routines {
            output.push_str(&format!(
                "- {} [{}]\n",
                routine.metadata.title, routine.metadata.routine_id
            ));
            output.push_str(&format!("  source: {}\n", routine.source.kind().label()));
        }
    }
    if !pack.report_profiles.is_empty() {
        output.push_str("\n[report-profiles]\n");
        for profile in &pack.report_profiles {
            output.push_str(&format!(
                "- {} [{}]\n",
                profile.metadata.title, profile.metadata.profile_id
            ));
        }
    }

    output
}

pub(crate) fn render_workbench_pack_validation(result: &ValidateWorkbenchPackResult) -> String {
    let mut output = String::new();
    if result.valid {
        if let Some(pack) = &result.pack {
            output.push_str(&format!(
                "valid pack: {} (workflows: {}, routines: {}, profiles: {})\n",
                pack.metadata.pack_id,
                pack.workflow_count,
                pack.review_routine_count,
                pack.report_profile_count
            ));
        } else {
            output.push_str("valid pack\n");
        }
        return output;
    }

    output.push_str("invalid pack\n");
    render_workbench_pack_issues(&mut output, &result.issues);
    output
}

pub(crate) fn render_workbench_pack_issues(output: &mut String, issues: &[WorkbenchPackIssue]) {
    if issues.is_empty() {
        return;
    }
    output.push_str("[issues]\n");
    for issue in issues {
        output.push_str(&format!("- kind: {}\n", issue.kind.label()));
        if let Some(asset_id) = &issue.asset_id {
            output.push_str(&format!("  asset id: {asset_id}\n"));
        }
        output.push_str(&format!("  message: {}\n", issue.message));
    }
}

pub(crate) fn render_review_routine_list(result: &ListReviewRoutinesResult) -> String {
    let mut output = String::new();
    if result.routines.is_empty() {
        output.push_str("(none)\n");
    } else {
        for routine in &result.routines {
            render_review_routine_summary(&mut output, routine);
        }
    }

    if !result.issues.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        render_workflow_catalog_issues(&mut output, &result.issues);
    }

    output
}

pub(crate) fn render_review_routine_summary(output: &mut String, routine: &ReviewRoutineSummary) {
    output.push_str(&format!(
        "- {} [{}]\n",
        routine.metadata.title, routine.metadata.routine_id
    ));
    output.push_str(&format!("  source: {}\n", routine.source_kind.label()));
    output.push_str(&format!("  inputs: {}\n", routine.input_count));
    output.push_str(&format!(
        "  report profiles: {}\n",
        routine.report_profile_count
    ));
    if let Some(summary) = &routine.metadata.summary {
        output.push_str(&format!("  summary: {summary}\n"));
    }
}

pub(crate) fn render_review_routine_spec(routine: &ReviewRoutineSpec) -> String {
    let mut output = String::new();
    output.push_str(&format!("routine id: {}\n", routine.metadata.routine_id));
    output.push_str(&format!("title: {}\n", routine.metadata.title));
    output.push_str(&format!("source: {}\n", routine.source.kind().label()));
    if let Some(summary) = &routine.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    render_review_routine_source(&mut output, &routine.source);
    output.push_str(&format!("save review: {}\n", routine.save_review.enabled));
    if let Some(review_id) = &routine.save_review.review_id {
        output.push_str(&format!("review id: {review_id}\n"));
    }
    if let Some(title) = &routine.save_review.title {
        output.push_str(&format!("review title: {title}\n"));
    }
    if let Some(summary) = &routine.save_review.summary {
        output.push_str(&format!("review summary: {summary}\n"));
    }
    output.push_str(&format!("overwrite: {}\n", routine.save_review.overwrite));
    if let Some(compare) = &routine.compare {
        output.push_str(&format!("compare: {}\n", compare.target.label()));
        if let Some(profile_id) = &compare.report_profile_id {
            output.push_str(&format!("compare report profile: {profile_id}\n"));
        }
    }
    if !routine.report_profile_ids.is_empty() {
        output.push_str(&format!(
            "report profiles: {}\n",
            routine.report_profile_ids.join(", ")
        ));
    }
    if !routine.inputs.is_empty() {
        output.push_str("\n[inputs]\n");
        for input in &routine.inputs {
            render_workflow_input_spec(&mut output, input);
        }
    }
    output
}

pub(crate) fn render_review_routine_source(output: &mut String, source: &ReviewRoutineSource) {
    match source {
        ReviewRoutineSource::Audit { audit, limit } => {
            output.push_str(&format!("audit: {}\n", render_corpus_audit_kind(*audit)));
            output.push_str(&format!("limit: {limit}\n"));
        }
        ReviewRoutineSource::Workflow { workflow_id } => {
            output.push_str(&format!("workflow id: {workflow_id}\n"));
        }
        ReviewRoutineSource::Unsupported => {
            output.push_str("unsupported source\n");
        }
    }
}

pub(crate) fn render_review_routine_execution_result(
    result: &ReviewRoutineExecutionResult,
) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "routine: {} [{}]\n",
        result.routine.metadata.title, result.routine.metadata.routine_id
    ));
    output.push_str(&format!("source: {}\n", result.routine.source_kind.label()));
    if let Some(summary) = &result.routine.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    match &result.source {
        ReviewRoutineSourceExecutionResult::Audit { result } => {
            output.push('\n');
            output.push_str(&render_corpus_audit_result(result));
        }
        ReviewRoutineSourceExecutionResult::Workflow { result } => {
            output.push('\n');
            output.push_str(&render_workflow_execution_result(result));
        }
    }
    if let Some(saved_review) = &result.saved_review {
        output.push('\n');
        output.push_str(&render_saved_review_summary(saved_review));
        output.push_str(&format!("{}\n", render_review_status_counts(saved_review)));
    }
    if let Some(compare) = &result.compare {
        output.push('\n');
        output.push_str(&render_review_routine_compare(compare));
    }
    if !result.reports.is_empty() {
        output.push('\n');
        output.push_str("[reports]\n");
        for report in &result.reports {
            render_applied_report_profile(&mut output, report);
        }
    }
    output
}

pub(crate) fn render_review_routine_compare(compare: &ReviewRoutineCompareResult) -> String {
    let mut output = String::new();
    output.push_str(&format!("compare: {}\n", compare.target.label()));
    if let Some(base_review) = &compare.base_review {
        output.push_str(&format!(
            "base review: {} [{}]\n",
            base_review.metadata.title, base_review.metadata.review_id
        ));
    } else {
        output.push_str("base review: none\n");
    }
    if let Some(diff) = &compare.diff {
        output.push_str(&render_review_diff(diff));
    }
    if let Some(report) = &compare.report {
        output.push_str("\n[compare-report]\n");
        render_applied_report_profile(&mut output, report);
    }
    output
}

pub(crate) fn render_applied_report_profile(output: &mut String, report: &AppliedReportProfile) {
    output.push_str(&format!(
        "- {} [{}]\n",
        report.profile.metadata.title, report.profile.metadata.profile_id
    ));
    if report.lines.is_empty() {
        output.push_str("  (no lines)\n");
        return;
    }
    for line in &report.lines {
        let rendered = render_review_routine_report_line(line);
        push_indented(output, &rendered, "  ");
    }
}

pub(crate) fn render_review_routine_report_line(line: &ReviewRoutineReportLine) -> String {
    let mut output = String::new();
    match line {
        ReviewRoutineReportLine::Routine { routine } => {
            output.push_str(&format!(
                "routine: {} [{}]\n",
                routine.metadata.title, routine.metadata.routine_id
            ));
        }
        ReviewRoutineReportLine::Workflow { workflow } => {
            output.push_str(&format!(
                "workflow: {} [{}]\n",
                workflow.metadata.title, workflow.metadata.workflow_id
            ));
        }
        ReviewRoutineReportLine::Step { step } => {
            render_workflow_step_report(&mut output, step);
        }
        ReviewRoutineReportLine::Audit { audit } => {
            output.push_str(&format!("audit: {}\n", render_corpus_audit_kind(*audit)));
        }
        ReviewRoutineReportLine::Entry { entry } => {
            render_review_audit_entry(&mut output, entry);
        }
        ReviewRoutineReportLine::Review { review } => {
            output.push_str(&render_saved_review_summary(review));
            output.push_str(&format!("{}\n", render_review_status_counts(review)));
        }
        ReviewRoutineReportLine::Finding { finding }
        | ReviewRoutineReportLine::Added { finding }
        | ReviewRoutineReportLine::Removed { finding } => {
            render_review_finding(&mut output, finding, "");
        }
        ReviewRoutineReportLine::Diff {
            base_review,
            target_review,
        } => {
            output.push_str(&format!(
                "diff: {} -> {}\n",
                base_review.metadata.review_id, target_review.metadata.review_id
            ));
        }
        ReviewRoutineReportLine::Unchanged { finding }
        | ReviewRoutineReportLine::ContentChanged { finding } => {
            output.push_str(&format!("finding pair: {}\n", finding.finding_id));
            output.push_str("base:\n");
            render_review_finding(&mut output, &finding.base, "  ");
            output.push_str("target:\n");
            render_review_finding(&mut output, &finding.target, "  ");
        }
        ReviewRoutineReportLine::StatusChanged { change } => {
            output.push_str(&format!("status changed: {}\n", change.finding_id));
            output.push_str(&format!(
                "status: {} -> {}\n",
                render_review_finding_status(change.from_status),
                render_review_finding_status(change.to_status)
            ));
            render_review_finding(&mut output, &change.target, "");
        }
    }
    output
}

pub(crate) fn render_workflow_spec(workflow: &WorkflowSpec) -> String {
    let mut output = String::new();
    output.push_str(&format!("workflow id: {}\n", workflow.metadata.workflow_id));
    output.push_str(&format!("title: {}\n", workflow.metadata.title));
    output.push_str(&format!(
        "compatibility: workflow-spec/v{}\n",
        workflow.compatibility.version
    ));
    if let Some(summary) = &workflow.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    output.push_str(&format!("steps: {}\n", workflow.steps.len()));
    if !workflow.inputs.is_empty() {
        output.push_str("\n[inputs]\n");
        for input in &workflow.inputs {
            render_workflow_input_spec(&mut output, input);
        }
    }
    output.push_str("\n[steps]\n");
    for step in &workflow.steps {
        render_workflow_step_spec(&mut output, step);
    }
    output
}

pub(crate) fn render_workflow_input_spec(output: &mut String, input: &WorkflowInputSpec) {
    output.push_str(&format!(
        "- {} [{}]\n",
        input.input_id,
        render_workflow_input_kind(input.kind)
    ));
    output.push_str(&format!("  title: {}\n", input.title));
    if let Some(summary) = &input.summary {
        output.push_str(&format!("  summary: {summary}\n"));
    }
}

pub(crate) fn render_workflow_step_spec(output: &mut String, step: &WorkflowStepSpec) {
    output.push_str(&format!("- {} [{}]\n", step.step_id, step.kind().label()));
    match &step.payload {
        WorkflowStepPayload::Resolve { target } => {
            output.push_str(&format!(
                "  target: {}\n",
                render_workflow_resolve_target(target)
            ));
        }
        WorkflowStepPayload::Explore {
            focus,
            lens,
            limit,
            unique,
        } => {
            output.push_str(&format!(
                "  focus: {}\n",
                render_workflow_explore_focus(focus)
            ));
            output.push_str(&format!("  lens: {}\n", render_exploration_lens(*lens)));
            output.push_str(&format!("  limit: {limit}\n"));
            output.push_str(&format!("  unique: {unique}\n"));
        }
        WorkflowStepPayload::Compare {
            left,
            right,
            group,
            limit,
        } => {
            output.push_str(&format!("  left: {}\n", left.step_id));
            output.push_str(&format!("  right: {}\n", right.step_id));
            output.push_str(&format!("  group: {}\n", render_comparison_group(*group)));
            output.push_str(&format!("  limit: {limit}\n"));
        }
        WorkflowStepPayload::ArtifactRun { artifact_id } => {
            output.push_str(&format!("  artifact id: {artifact_id}\n"));
        }
        WorkflowStepPayload::ArtifactSave {
            source,
            metadata,
            overwrite,
        } => {
            output.push_str(&format!(
                "  source: {}\n",
                render_workflow_artifact_save_source(source)
            ));
            output.push_str(&format!("  artifact id: {}\n", metadata.artifact_id));
            output.push_str(&format!("  title: {}\n", metadata.title));
            if let Some(summary) = &metadata.summary {
                output.push_str(&format!("  summary: {summary}\n"));
            }
            output.push_str(&format!("  overwrite: {overwrite}\n"));
        }
    }
}

pub(crate) fn render_workflow_execution_result(result: &WorkflowExecutionResult) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "workflow: {} [{}]\n",
        result.workflow.metadata.title, result.workflow.metadata.workflow_id
    ));
    output.push_str(&format!("steps: {}\n", result.workflow.step_count));
    if let Some(summary) = &result.workflow.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    for step in &result.steps {
        output.push('\n');
        render_workflow_step_report(&mut output, step);
    }
    output
}

pub(crate) fn render_workflow_step_report(output: &mut String, step: &WorkflowStepReport) {
    output.push_str(&format!("[step {}]\n", step.step_id));
    output.push_str(&format!("kind: {}\n", step.kind().label()));
    match &step.payload {
        WorkflowStepReportPayload::Resolve { node } => {
            output.push_str(&render_node_summary(node));
        }
        WorkflowStepReportPayload::Explore {
            focus_node_key,
            result,
        } => {
            output.push_str(&format!("focus node key: {focus_node_key}\n"));
            output.push_str(&render_explore_result(result));
        }
        WorkflowStepReportPayload::Compare {
            left_node,
            right_node,
            result,
        } => {
            output.push_str(&format!("left node: {}\n", render_node_identity(left_node)));
            output.push_str(&format!(
                "right node: {}\n",
                render_node_identity(right_node)
            ));
            output.push_str(&render_compare_result(result, NoteComparisonGroup::All));
        }
        WorkflowStepReportPayload::ArtifactRun { artifact } => {
            output.push_str(&render_executed_exploration_artifact(artifact));
        }
        WorkflowStepReportPayload::ArtifactSave { artifact } => {
            output.push_str(&render_saved_artifact_summary(artifact));
        }
    }
}

pub(crate) fn render_workflow_input_kind(kind: WorkflowInputKind) -> &'static str {
    match kind {
        WorkflowInputKind::NoteTarget => "note-target",
        WorkflowInputKind::FocusTarget => "focus-target",
    }
}

pub(crate) fn render_workflow_resolve_target(target: &WorkflowResolveTarget) -> String {
    match target {
        WorkflowResolveTarget::Id { id } => format!("id:{id}"),
        WorkflowResolveTarget::Title { title } => format!("title:{title}"),
        WorkflowResolveTarget::Reference { reference } => format!("ref:{reference}"),
        WorkflowResolveTarget::NodeKey { node_key } => format!("key:{node_key}"),
        WorkflowResolveTarget::Input { input_id } => format!("input:{input_id}"),
    }
}

pub(crate) fn render_workflow_explore_focus(focus: &WorkflowExploreFocus) -> String {
    match focus {
        WorkflowExploreFocus::NodeKey { node_key } => format!("key:{node_key}"),
        WorkflowExploreFocus::Input { input_id } => format!("input:{input_id}"),
        WorkflowExploreFocus::ResolvedStep { step_id } => format!("resolved-step:{step_id}"),
    }
}

pub(crate) fn render_workflow_artifact_save_source(source: &WorkflowArtifactSaveSource) -> String {
    match source {
        WorkflowArtifactSaveSource::ExploreStep { step_id } => {
            format!("explore-step:{step_id}")
        }
        WorkflowArtifactSaveSource::CompareStep { step_id } => {
            format!("compare-step:{step_id}")
        }
    }
}
