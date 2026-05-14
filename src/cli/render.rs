use slipbox_core::{
    AgendaResult, AnchorRecord, AppliedReportProfile, AuditRemediationApplyAction,
    AuditRemediationConfidence, AuditRemediationPreviewPayload, BacklinksResult,
    CaptureTemplatePreviewResult, ComparisonConnectorDirection, CorpusAuditEntry, CorpusAuditKind,
    CorpusAuditResult, ExecutedExplorationArtifact, ExecutedExplorationArtifactPayload,
    ExplorationArtifactKind, ExplorationArtifactSummary, ExplorationEntry, ExplorationExplanation,
    ExplorationLens, ExplorationSectionKind, ExploreResult, FileDiagnosticIssue, FileDiagnostics,
    FileRecord, ForwardLinksResult, IndexDiagnostics, IndexStats, IndexedFilesResult,
    ListExplorationArtifactsResult, ListReviewRoutinesResult, ListReviewRunsResult,
    ListWorkbenchPacksResult, ListWorkflowsResult, MarkReviewFindingResult, NodeDiagnosticIssue,
    NodeDiagnostics, NodeRecord, NoteComparisonEntry, NoteComparisonExplanation,
    NoteComparisonGroup, NoteComparisonResult, NoteComparisonSectionKind, OccurrenceRecord,
    PlanningField, PlanningRelationRecord, RandomNodeResult, RefRecord, ReviewFinding,
    ReviewFindingKind, ReviewFindingPair, ReviewFindingPayload,
    ReviewFindingRemediationApplyResult, ReviewFindingRemediationPreview, ReviewFindingStatus,
    ReviewFindingStatusDiff, ReviewRoutineCompareResult, ReviewRoutineExecutionResult,
    ReviewRoutineReportLine, ReviewRoutineSource, ReviewRoutineSourceExecutionResult,
    ReviewRoutineSpec, ReviewRoutineSummary, ReviewRun, ReviewRunDiff, ReviewRunKind,
    ReviewRunPayload, ReviewRunSummary, SavedComparisonArtifact, SavedExplorationArtifact,
    SavedLensViewArtifact, SavedTrailArtifact, SavedTrailStep, SearchFilesResult,
    SearchNodesResult, SearchOccurrencesResult, SearchRefsResult, SearchTagsResult,
    SlipboxLinkRewriteApplyResult, SlipboxLinkRewritePreview, StructuralWriteReport,
    StructuralWriteResult, TrailReplayResult, TrailReplayStepResult, ValidateWorkbenchPackResult,
    WorkbenchPackIssue, WorkbenchPackManifest, WorkflowArtifactSaveSource, WorkflowCatalogIssue,
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

pub(crate) fn render_corpus_audit_result(result: &CorpusAuditResult) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "audit: {}\n",
        render_corpus_audit_kind(result.audit)
    ));
    if result.entries.is_empty() {
        output.push_str("(none)\n");
        return output;
    }
    for entry in &result.entries {
        match entry {
            CorpusAuditEntry::DanglingLink { record } => {
                output.push_str(&format!(
                    "\n- {} -> missing id {}\n",
                    render_anchor_identity(&record.source),
                    record.missing_explicit_id
                ));
                output.push_str(&format!(
                    "  location: {}:{}:{}\n",
                    record.source.file_path, record.line, record.column
                ));
                output.push_str(&format!("  preview: {}\n", record.preview));
            }
            CorpusAuditEntry::DuplicateTitle { record } => {
                output.push_str(&format!("\n- duplicate title: {}\n", record.title));
                for note in &record.notes {
                    output.push_str(&format!("  note: {} [{}]\n", note.title, note.node_key));
                    output.push_str(&format!("  file: {}:{}\n", note.file_path, note.line));
                }
            }
            CorpusAuditEntry::OrphanNote { record } => {
                output.push_str(&format!(
                    "\n- orphan note: {} [{}]\n",
                    record.note.title, record.note.node_key
                ));
                output.push_str(&format!(
                    "  refs/backlinks/forward-links: {}/{}/{}\n",
                    record.reference_count, record.backlink_count, record.forward_link_count
                ));
            }
            CorpusAuditEntry::WeaklyIntegratedNote { record } => {
                output.push_str(&format!(
                    "\n- weakly integrated note: {} [{}]\n",
                    record.note.title, record.note.node_key
                ));
                output.push_str(&format!(
                    "  refs/backlinks/forward-links: {}/{}/{}\n",
                    record.reference_count, record.backlink_count, record.forward_link_count
                ));
            }
        }
    }
    output
}

pub(crate) fn render_corpus_audit_kind(kind: CorpusAuditKind) -> &'static str {
    match kind {
        CorpusAuditKind::DanglingLinks => "dangling-links",
        CorpusAuditKind::DuplicateTitles => "duplicate-titles",
        CorpusAuditKind::OrphanNotes => "orphan-notes",
        CorpusAuditKind::WeaklyIntegratedNotes => "weakly-integrated-notes",
    }
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

pub(crate) fn render_saved_artifact_summary(artifact: &ExplorationArtifactSummary) -> String {
    format!(
        "saved artifact: {} [{}]\n",
        artifact.metadata.artifact_id,
        render_artifact_kind(artifact.kind)
    )
}

pub(crate) fn render_saved_review_summary(review: &ReviewRunSummary) -> String {
    format!(
        "saved review: {} [{}]\n",
        review.metadata.review_id,
        render_review_kind(review.kind)
    )
}

pub(crate) fn render_review_list(result: &ListReviewRunsResult) -> String {
    let mut output = String::new();
    if result.reviews.is_empty() {
        output.push_str("(none)\n");
        return output;
    }

    for review in &result.reviews {
        output.push_str(&format!(
            "- {} [{}]\n",
            review.metadata.title,
            render_review_kind(review.kind)
        ));
        output.push_str(&format!("  review id: {}\n", review.metadata.review_id));
        output.push_str(&format!("  findings: {}\n", review.finding_count));
        output.push_str(&format!(
            "  status: {}\n",
            render_review_status_counts(review)
        ));
        if let Some(summary) = &review.metadata.summary {
            output.push_str(&format!("  summary: {summary}\n"));
        }
    }
    output
}

pub(crate) fn render_review_run(review: &ReviewRun) -> String {
    let summary = ReviewRunSummary::from(review);
    let mut output = String::new();
    output.push_str(&format!("review id: {}\n", review.metadata.review_id));
    output.push_str(&format!("title: {}\n", review.metadata.title));
    output.push_str(&format!("kind: {}\n", render_review_kind(review.kind())));
    if let Some(summary_text) = &review.metadata.summary {
        output.push_str(&format!("summary: {summary_text}\n"));
    }
    output.push_str(&format!("findings: {}\n", summary.finding_count));
    output.push_str(&format!(
        "status: {}\n",
        render_review_status_counts(&summary)
    ));
    render_review_payload(&mut output, &review.payload);

    if review.findings.is_empty() {
        output.push_str("\n[findings]\n(none)\n");
        return output;
    }

    output.push_str("\n[findings]\n");
    for finding in &review.findings {
        render_review_finding(&mut output, finding, "");
    }
    output
}

pub(crate) fn render_review_diff(diff: &ReviewRunDiff) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "base review: {} [{}]\n",
        diff.base_review.metadata.review_id,
        render_review_kind(diff.base_review.kind)
    ));
    output.push_str(&format!(
        "target review: {} [{}]\n",
        diff.target_review.metadata.review_id,
        render_review_kind(diff.target_review.kind)
    ));
    output.push_str(&format!("added: {}\n", diff.added.len()));
    output.push_str(&format!("removed: {}\n", diff.removed.len()));
    output.push_str(&format!("unchanged: {}\n", diff.unchanged.len()));
    output.push_str(&format!(
        "content changed: {}\n",
        diff.content_changed.len()
    ));
    output.push_str(&format!("status changed: {}\n", diff.status_changed.len()));

    render_review_diff_findings(&mut output, "added", &diff.added);
    render_review_diff_findings(&mut output, "removed", &diff.removed);
    render_review_diff_pairs(&mut output, "unchanged", &diff.unchanged);
    render_review_diff_pairs(&mut output, "content-changed", &diff.content_changed);
    render_review_diff_status_changes(&mut output, &diff.status_changed);
    output
}

pub(crate) fn render_review_diff_findings(
    output: &mut String,
    section: &str,
    findings: &[ReviewFinding],
) {
    if findings.is_empty() {
        return;
    }
    output.push_str(&format!("\n[{section}]\n"));
    for finding in findings {
        render_review_finding(output, finding, "");
    }
}

pub(crate) fn render_review_diff_pairs(
    output: &mut String,
    section: &str,
    pairs: &[ReviewFindingPair],
) {
    if pairs.is_empty() {
        return;
    }
    output.push_str(&format!("\n[{section}]\n"));
    for pair in pairs {
        output.push_str(&format!("- {}\n", pair.finding_id));
        output.push_str("  base:\n");
        render_review_finding(output, &pair.base, "    ");
        output.push_str("  target:\n");
        render_review_finding(output, &pair.target, "    ");
    }
}

pub(crate) fn render_review_diff_status_changes(
    output: &mut String,
    changes: &[ReviewFindingStatusDiff],
) {
    if changes.is_empty() {
        return;
    }
    output.push_str("\n[status-changed]\n");
    for change in changes {
        output.push_str(&format!("- {}\n", change.finding_id));
        output.push_str(&format!(
            "  status: {} -> {}\n",
            render_review_finding_status(change.from_status),
            render_review_finding_status(change.to_status)
        ));
        output.push_str("  target:\n");
        render_review_finding(output, &change.target, "    ");
    }
}

pub(crate) fn render_mark_review_finding_result(result: &MarkReviewFindingResult) -> String {
    format!(
        "marked review finding: {} {} {} -> {}\n",
        result.transition.review_id,
        result.transition.finding_id,
        render_review_finding_status(result.transition.from_status),
        render_review_finding_status(result.transition.to_status)
    )
}

pub(crate) fn render_review_remediation_preview(
    preview: &ReviewFindingRemediationPreview,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("review id: {}\n", preview.review_id));
    output.push_str(&format!("finding id: {}\n", preview.finding_id));
    output.push_str(&format!(
        "status: {}\n",
        render_review_finding_status(preview.status)
    ));

    match &preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview: preview_text,
            suggestion,
            confidence,
            reason,
        } => {
            output.push_str("remediation: unlink-dangling-link\n");
            output.push_str(&format!(
                "confidence: {}\n",
                render_audit_remediation_confidence(*confidence)
            ));
            output.push_str(&format!("source: {}\n", render_anchor_identity(source)));
            output.push_str(&format!("missing id: {missing_explicit_id}\n"));
            output.push_str(&format!("location: {file_path}:{line}:{column}\n"));
            output.push_str(&format!("preview: {preview_text}\n"));
            output.push_str(&format!("suggestion: {suggestion}\n"));
            output.push_str(&format!("reason: {reason}\n"));
            output.push_str(&format!(
                "apply: slipbox review remediation apply {} {} --confirm-unlink-dangling-link\n",
                preview.review_id, preview.finding_id
            ));
        }
        AuditRemediationPreviewPayload::DuplicateTitle {
            title,
            notes,
            suggestion,
            confidence,
            reason,
        } => {
            output.push_str("remediation: manual-review\n");
            output.push_str(&format!(
                "confidence: {}\n",
                render_audit_remediation_confidence(*confidence)
            ));
            output.push_str(&format!("title: {title}\n"));
            output.push_str(&format!("notes: {}\n", notes.len()));
            for note in notes {
                output.push_str(&format!("  - {}\n", render_node_identity(note)));
            }
            output.push_str(&format!("suggestion: {suggestion}\n"));
            output.push_str(&format!("reason: {reason}\n"));
            output.push_str("apply: unsupported by safe remediation apply\n");
        }
    }
    output
}

pub(crate) fn render_review_remediation_application(
    result: &ReviewFindingRemediationApplyResult,
) -> String {
    let application = &result.application;
    let mut output = String::new();
    output.push_str(&format!(
        "applied remediation: {} {}\n",
        application.review_id, application.finding_id
    ));
    output.push_str(&format!(
        "action: {}\n",
        render_audit_remediation_apply_action(&application.action)
    ));
    output.push_str(&format!(
        "index: {}\n",
        render_structural_index_refresh(application.index_refresh)
    ));
    output.push_str("changed files:\n");
    if application.affected_files.changed_files.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for file in &application.affected_files.changed_files {
            output.push_str(&format!("  - {file}\n"));
        }
    }
    output.push_str("removed files:\n");
    if application.affected_files.removed_files.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for file in &application.affected_files.removed_files {
            output.push_str(&format!("  - {file}\n"));
        }
    }
    output.push_str(&format!(
        "inspect: slipbox review show {}\n",
        application.review_id
    ));
    output
}

pub(crate) fn render_slipbox_link_rewrite_preview(preview: &SlipboxLinkRewritePreview) -> String {
    let mut output = String::new();
    output.push_str(&format!("file: {}\n", preview.file_path));
    output.push_str(&format!("rewrites: {}\n", preview.rewrites.len()));
    if preview.rewrites.is_empty() {
        output.push_str("  (none)\n");
        return output;
    }
    for rewrite in &preview.rewrites {
        output.push_str(&format!(
            "- {}:{} slipbox:{} -> ",
            rewrite.line, rewrite.column, rewrite.title_or_alias
        ));
        match &rewrite.target_explicit_id {
            Some(explicit_id) => output.push_str(&format!("id:{explicit_id}\n")),
            None => output.push_str("id will be assigned on apply\n"),
        }
        output.push_str(&format!(
            "  target: {}\n",
            render_node_identity(&rewrite.target)
        ));
        output.push_str(&format!("  description: {}\n", rewrite.description));
        output.push_str(&format!("  preview: {}\n", rewrite.preview));
        if let Some(replacement) = &rewrite.replacement {
            output.push_str(&format!("  replacement: {replacement}\n"));
        }
    }
    output
}

pub(crate) fn render_slipbox_link_rewrite_application(
    result: &SlipboxLinkRewriteApplyResult,
) -> String {
    let application = &result.application;
    let mut output = String::new();
    output.push_str(&format!(
        "rewrote slipbox links: {}\n",
        application.file_path
    ));
    output.push_str(&format!("rewrites: {}\n", application.rewrites.len()));
    for rewrite in &application.rewrites {
        output.push_str(&format!(
            "- {}:{} slipbox:{} -> id:{}\n",
            rewrite.line, rewrite.column, rewrite.title_or_alias, rewrite.target_explicit_id
        ));
        output.push_str(&format!("  target node: {}\n", rewrite.target_node_key));
        output.push_str(&format!("  replacement: {}\n", rewrite.replacement));
    }
    output.push_str(&format!(
        "index: {}\n",
        render_structural_index_refresh(application.index_refresh)
    ));
    output.push_str("changed files:\n");
    for file in &application.affected_files.changed_files {
        output.push_str(&format!("  - {file}\n"));
    }
    output.push_str("removed files:\n");
    if application.affected_files.removed_files.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for file in &application.affected_files.removed_files {
            output.push_str(&format!("  - {file}\n"));
        }
    }
    output
}

pub(crate) fn render_audit_remediation_apply_action(
    action: &AuditRemediationApplyAction,
) -> &'static str {
    match action {
        AuditRemediationApplyAction::UnlinkDanglingLink { .. } => "unlink-dangling-link",
    }
}

pub(crate) fn render_audit_remediation_confidence(
    confidence: AuditRemediationConfidence,
) -> &'static str {
    match confidence {
        AuditRemediationConfidence::Low => "low",
        AuditRemediationConfidence::Medium => "medium",
        AuditRemediationConfidence::High => "high",
    }
}

pub(crate) fn render_review_payload(output: &mut String, payload: &ReviewRunPayload) {
    match payload {
        ReviewRunPayload::Audit { audit, limit } => {
            output.push_str(&format!("audit: {}\n", render_corpus_audit_kind(*audit)));
            output.push_str(&format!("limit: {limit}\n"));
        }
        ReviewRunPayload::Workflow {
            workflow,
            inputs,
            step_ids,
        } => {
            output.push_str(&format!(
                "workflow: {} [{}]\n",
                workflow.metadata.title, workflow.metadata.workflow_id
            ));
            output.push_str(&format!("steps: {}\n", workflow.step_count));
            output.push_str(&format!("source step ids: {}\n", step_ids.join(", ")));
            if inputs.is_empty() {
                output.push_str("inputs: 0\n");
            } else {
                output.push_str(&format!("inputs: {}\n", inputs.len()));
                for input in inputs {
                    output.push_str(&format!(
                        "  {}: {}\n",
                        input.input_id,
                        render_workflow_resolve_target(&input.target)
                    ));
                }
            }
        }
    }
}

pub(crate) fn render_review_finding(output: &mut String, finding: &ReviewFinding, indent: &str) {
    output.push_str(&format!(
        "{indent}- {} [{}]\n",
        finding.finding_id,
        render_review_finding_kind(finding.kind())
    ));
    output.push_str(&format!(
        "{indent}  status: {}\n",
        render_review_finding_status(finding.status)
    ));
    let payload = render_review_finding_payload_block(&finding.payload);
    push_indented(output, &payload, indent);
}

pub(crate) fn render_review_finding_payload(output: &mut String, payload: &ReviewFindingPayload) {
    match payload {
        ReviewFindingPayload::Audit { entry } => {
            render_review_audit_entry(output, entry);
        }
        ReviewFindingPayload::WorkflowStep { step } => {
            render_workflow_step_report(output, step);
        }
    }
}

pub(crate) fn render_review_finding_payload_block(payload: &ReviewFindingPayload) -> String {
    let mut output = String::new();
    render_review_finding_payload(&mut output, payload);
    output
}

fn push_indented(output: &mut String, text: &str, indent: &str) {
    for line in text.lines() {
        output.push_str(indent);
        output.push_str(line);
        output.push('\n');
    }
}

pub(crate) fn render_review_audit_entry(output: &mut String, entry: &CorpusAuditEntry) {
    match entry {
        CorpusAuditEntry::DanglingLink { record } => {
            output.push_str(&format!(
                "  dangling link: {} -> missing id {}\n",
                render_anchor_identity(&record.source),
                record.missing_explicit_id
            ));
            output.push_str(&format!(
                "  location: {}:{}:{}\n",
                record.source.file_path, record.line, record.column
            ));
            output.push_str(&format!("  preview: {}\n", record.preview));
        }
        CorpusAuditEntry::DuplicateTitle { record } => {
            output.push_str(&format!("  duplicate title: {}\n", record.title));
            output.push_str(&format!("  notes: {}\n", record.notes.len()));
        }
        CorpusAuditEntry::OrphanNote { record } => {
            output.push_str(&format!(
                "  orphan note: {} [{}]\n",
                record.note.title, record.note.node_key
            ));
            output.push_str(&format!(
                "  refs/backlinks/forward-links: {}/{}/{}\n",
                record.reference_count, record.backlink_count, record.forward_link_count
            ));
        }
        CorpusAuditEntry::WeaklyIntegratedNote { record } => {
            output.push_str(&format!(
                "  weakly integrated note: {} [{}]\n",
                record.note.title, record.note.node_key
            ));
            output.push_str(&format!(
                "  refs/backlinks/forward-links: {}/{}/{}\n",
                record.reference_count, record.backlink_count, record.forward_link_count
            ));
        }
    }
}

pub(crate) fn render_review_kind(kind: ReviewRunKind) -> &'static str {
    match kind {
        ReviewRunKind::Audit => "audit",
        ReviewRunKind::Workflow => "workflow",
    }
}

pub(crate) fn render_review_finding_kind(kind: ReviewFindingKind) -> &'static str {
    match kind {
        ReviewFindingKind::Audit => "audit",
        ReviewFindingKind::WorkflowStep => "workflow-step",
    }
}

pub(crate) fn render_review_finding_status(status: ReviewFindingStatus) -> &'static str {
    match status {
        ReviewFindingStatus::Open => "open",
        ReviewFindingStatus::Reviewed => "reviewed",
        ReviewFindingStatus::Dismissed => "dismissed",
        ReviewFindingStatus::Accepted => "accepted",
    }
}

pub(crate) fn render_review_status_counts(summary: &ReviewRunSummary) -> String {
    format!(
        "open/reviewed/dismissed/accepted: {}/{}/{}/{}",
        summary.status_counts.open,
        summary.status_counts.reviewed,
        summary.status_counts.dismissed,
        summary.status_counts.accepted
    )
}

pub(crate) fn render_index_stats(stats: &IndexStats) -> String {
    format!(
        "files indexed: {}\nnodes indexed: {}\nlinks indexed: {}\n",
        stats.files_indexed, stats.nodes_indexed, stats.links_indexed
    )
}

pub(crate) fn render_indexed_files(result: &IndexedFilesResult) -> String {
    let mut output = format!("indexed files: {}\n", result.files.len());
    for file_path in &result.files {
        output.push_str(&format!("- {file_path}\n"));
    }
    output
}

pub(crate) fn render_file_search_result(result: &SearchFilesResult) -> String {
    let mut output = format!("files: {}\n", result.files.len());
    for file in &result.files {
        output.push_str(&render_file_record(file));
    }
    output
}

pub(crate) fn render_file_record(file: &FileRecord) -> String {
    format!(
        "- {} | {} | nodes: {}\n",
        file.file_path, file.title, file.node_count
    )
}

pub(crate) fn render_file_diagnostics(diagnostic: &FileDiagnostics) -> String {
    let mut output = String::new();
    output.push_str(&format!("file: {}\n", diagnostic.file_path));
    output.push_str(&format!("absolute path: {}\n", diagnostic.absolute_path));
    output.push_str(&format!("exists: {}\n", yes_no(diagnostic.exists)));
    output.push_str(&format!("eligible: {}\n", yes_no(diagnostic.eligible)));
    output.push_str(&format!("indexed: {}\n", yes_no(diagnostic.indexed)));
    if let Some(record) = &diagnostic.index_record {
        output.push_str(&format!("title: {}\n", record.title));
        output.push_str(&format!("nodes: {}\n", record.node_count));
        output.push_str(&format!("mtime ns: {}\n", record.mtime_ns));
    }
    output.push_str("issues:\n");
    render_file_diagnostic_issues(&mut output, &diagnostic.issues, "  ");
    output
}

pub(crate) fn render_node_diagnostics(diagnostic: &NodeDiagnostics) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "node: {}\n",
        render_anchor_identity(&diagnostic.node)
    ));
    output.push_str(&format!("file: {}\n", diagnostic.file.file_path));
    output.push_str(&format!("line: {}\n", diagnostic.node.line));
    output.push_str(&format!(
        "line present: {}\n",
        yes_no(diagnostic.line_present)
    ));
    output.push_str("file issues:\n");
    render_file_diagnostic_issues(&mut output, &diagnostic.file.issues, "  ");
    output.push_str("node issues:\n");
    render_node_diagnostic_issues(&mut output, &diagnostic.issues, "  ");
    output
}

pub(crate) fn render_index_diagnostics(diagnostic: &IndexDiagnostics) -> String {
    let mut output = String::new();
    output.push_str(&format!("root: {}\n", diagnostic.root));
    output.push_str(&format!(
        "eligible files: {}\n",
        diagnostic.eligible_files.len()
    ));
    output.push_str(&format!(
        "indexed files: {}\n",
        diagnostic.indexed_files.len()
    ));
    output.push_str(&format!(
        "status consistent: {}\n",
        yes_no(diagnostic.status_consistent)
    ));
    output.push_str(&format!(
        "index current: {}\n",
        yes_no(diagnostic.index_current)
    ));
    output.push_str(&format!(
        "status counts: files={} nodes={} links={}\n",
        diagnostic.status.files_indexed,
        diagnostic.status.nodes_indexed,
        diagnostic.status.links_indexed
    ));
    render_path_list(
        &mut output,
        "missing from index",
        &diagnostic.missing_from_index,
    );
    render_path_list(
        &mut output,
        "indexed but missing",
        &diagnostic.indexed_but_missing,
    );
    render_path_list(
        &mut output,
        "indexed but ineligible",
        &diagnostic.indexed_but_ineligible,
    );
    output
}

pub(crate) fn render_file_diagnostic_issues(
    output: &mut String,
    issues: &[FileDiagnosticIssue],
    indent: &str,
) {
    if issues.is_empty() {
        output.push_str(indent);
        output.push_str("(none)\n");
        return;
    }
    for issue in issues {
        output.push_str(indent);
        output.push_str(match issue {
            FileDiagnosticIssue::MissingFromIndex => "missing-from-index",
            FileDiagnosticIssue::IndexedButMissing => "indexed-but-missing",
            FileDiagnosticIssue::IndexedButIneligible => "indexed-but-ineligible",
        });
        output.push('\n');
    }
}

pub(crate) fn render_node_diagnostic_issues(
    output: &mut String,
    issues: &[NodeDiagnosticIssue],
    indent: &str,
) {
    if issues.is_empty() {
        output.push_str(indent);
        output.push_str("(none)\n");
        return;
    }
    for issue in issues {
        output.push_str(indent);
        output.push_str(match issue {
            NodeDiagnosticIssue::SourceFileMissing => "source-file-missing",
            NodeDiagnosticIssue::SourceFileIneligible => "source-file-ineligible",
            NodeDiagnosticIssue::SourceFileUnindexed => "source-file-unindexed",
            NodeDiagnosticIssue::LineOutOfRange => "line-out-of-range",
        });
        output.push('\n');
    }
}

pub(crate) fn render_path_list(output: &mut String, label: &str, paths: &[String]) {
    output.push_str(&format!("{label}: {}\n", paths.len()));
    for path in paths {
        output.push_str(&format!("- {path}\n"));
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub(crate) fn render_structural_write_report(report: &StructuralWriteReport) -> String {
    let mut output = String::new();
    output.push_str(&format!("operation: {}\n", report.operation.label()));
    output.push_str(&format!(
        "index refresh: {}\n",
        render_structural_index_refresh(report.index_refresh)
    ));
    output.push_str(&format!(
        "changed files: {}\n",
        report.affected_files.changed_files.len()
    ));
    for file_path in &report.affected_files.changed_files {
        output.push_str(&format!("- {file_path}\n"));
    }
    output.push_str(&format!(
        "removed files: {}\n",
        report.affected_files.removed_files.len()
    ));
    for file_path in &report.affected_files.removed_files {
        output.push_str(&format!("- {file_path}\n"));
    }
    match &report.result {
        Some(StructuralWriteResult::Node { node }) => {
            output.push_str("result: node\n");
            output.push_str(&render_node_summary(node));
        }
        Some(StructuralWriteResult::Anchor { anchor }) => {
            output.push_str("result: anchor\n");
            output.push_str(&render_anchor_summary(anchor));
        }
        None => output.push_str("result: none\n"),
    }
    output
}

pub(crate) fn render_structural_index_refresh(
    status: slipbox_core::StructuralWriteIndexRefreshStatus,
) -> &'static str {
    match status {
        slipbox_core::StructuralWriteIndexRefreshStatus::Refreshed => "refreshed",
        slipbox_core::StructuralWriteIndexRefreshStatus::Pending => "pending",
    }
}

pub(crate) fn render_node_summary(node: &NodeRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("node key: {}\n", node.node_key));
    if let Some(explicit_id) = &node.explicit_id {
        output.push_str(&format!("id: {explicit_id}\n"));
    }
    output.push_str(&format!("title: {}\n", node.title));
    output.push_str(&format!("kind: {}\n", node.kind.as_str()));
    output.push_str(&format!("file: {}\n", node.file_path));
    output.push_str(&format!("line: {}\n", node.line));
    if !node.outline_path.is_empty() {
        output.push_str(&format!("outline path: {}\n", node.outline_path));
    }
    if !node.aliases.is_empty() {
        output.push_str(&format!("aliases: {}\n", node.aliases.join(", ")));
    }
    if !node.refs.is_empty() {
        output.push_str(&format!("refs: {}\n", node.refs.join(", ")));
    }
    if !node.tags.is_empty() {
        output.push_str(&format!("tags: {}\n", node.tags.join(", ")));
    }
    if let Some(todo_keyword) = &node.todo_keyword {
        output.push_str(&format!("todo: {todo_keyword}\n"));
    }
    if let Some(scheduled_for) = &node.scheduled_for {
        output.push_str(&format!("scheduled: {scheduled_for}\n"));
    }
    if let Some(deadline_for) = &node.deadline_for {
        output.push_str(&format!("deadline: {deadline_for}\n"));
    }
    if let Some(closed_at) = &node.closed_at {
        output.push_str(&format!("closed: {closed_at}\n"));
    }
    output
}

pub(crate) fn render_anchor_summary(anchor: &AnchorRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("anchor key: {}\n", anchor.node_key));
    if let Some(explicit_id) = &anchor.explicit_id {
        output.push_str(&format!("id: {explicit_id}\n"));
    }
    output.push_str(&format!("title: {}\n", anchor.title));
    output.push_str(&format!("kind: {}\n", anchor.kind.as_str()));
    output.push_str(&format!("file: {}\n", anchor.file_path));
    output.push_str(&format!("line: {}\n", anchor.line));
    if !anchor.outline_path.is_empty() {
        output.push_str(&format!("outline path: {}\n", anchor.outline_path));
    }
    if !anchor.aliases.is_empty() {
        output.push_str(&format!("aliases: {}\n", anchor.aliases.join(", ")));
    }
    if !anchor.refs.is_empty() {
        output.push_str(&format!("refs: {}\n", anchor.refs.join(", ")));
    }
    if !anchor.tags.is_empty() {
        output.push_str(&format!("tags: {}\n", anchor.tags.join(", ")));
    }
    if let Some(todo_keyword) = &anchor.todo_keyword {
        output.push_str(&format!("todo: {todo_keyword}\n"));
    }
    if let Some(scheduled_for) = &anchor.scheduled_for {
        output.push_str(&format!("scheduled: {scheduled_for}\n"));
    }
    if let Some(deadline_for) = &anchor.deadline_for {
        output.push_str(&format!("deadline: {deadline_for}\n"));
    }
    if let Some(closed_at) = &anchor.closed_at {
        output.push_str(&format!("closed: {closed_at}\n"));
    }
    output
}

pub(crate) fn render_node_search_result(result: &SearchNodesResult) -> String {
    let mut output = format!("nodes: {}\n", result.nodes.len());
    for node in &result.nodes {
        output.push_str(&format!("- {}\n", render_node_identity(node)));
    }
    output
}

pub(crate) fn render_random_node_result(result: &RandomNodeResult) -> String {
    match &result.node {
        Some(node) => render_node_summary(node),
        None => "node: none\n".to_owned(),
    }
}

pub(crate) fn render_backlinks_result(result: &BacklinksResult) -> String {
    let mut output = format!("backlinks: {}\n", result.backlinks.len());
    for record in &result.backlinks {
        output.push_str(&format!(
            "- {} at {}:{}\n",
            render_node_identity(&record.source_note),
            record.row,
            record.col
        ));
        if let Some(anchor) = &record.source_anchor {
            output.push_str(&format!("  anchor: {}\n", render_anchor_identity(anchor)));
        }
        output.push_str(&format!("  preview: {}\n", record.preview));
    }
    output
}

pub(crate) fn render_forward_links_result(result: &ForwardLinksResult) -> String {
    let mut output = format!("forward links: {}\n", result.forward_links.len());
    for record in &result.forward_links {
        output.push_str(&format!(
            "- {} at {}:{}\n",
            render_node_identity(&record.destination_note),
            record.row,
            record.col
        ));
        output.push_str(&format!("  preview: {}\n", record.preview));
    }
    output
}

pub(crate) fn render_capture_preview(preview: &CaptureTemplatePreviewResult) -> String {
    let mut output = format!("preview file: {}\n", preview.file_path);
    output.push_str(&format!(
        "preview node: {} | {} | line {}\n",
        preview.preview_node.node_key, preview.preview_node.title, preview.preview_node.line
    ));
    output.push_str("--- content ---\n");
    output.push_str(&preview.content);
    if !preview.content.ends_with('\n') {
        output.push('\n');
    }
    output
}

pub(crate) fn render_ref_search_result(result: &SearchRefsResult) -> String {
    let mut output = format!("refs: {}\n", result.refs.len());
    for record in &result.refs {
        output.push_str(&render_ref_record(record));
    }
    output
}

pub(crate) fn render_ref_record(record: &RefRecord) -> String {
    format!(
        "- {} -> {}\n",
        record.reference,
        render_node_identity(&record.node)
    )
}

pub(crate) fn render_tag_search_result(result: &SearchTagsResult) -> String {
    let mut output = format!("tags: {}\n", result.tags.len());
    for tag in &result.tags {
        output.push_str(&format!("- {tag}\n"));
    }
    output
}

pub(crate) fn render_occurrence_search_result(result: &SearchOccurrencesResult) -> String {
    let mut output = format!("occurrences: {}\n", result.occurrences.len());
    for record in &result.occurrences {
        output.push_str(&render_occurrence_record(record));
    }
    output
}

pub(crate) fn render_occurrence_record(record: &OccurrenceRecord) -> String {
    let mut output = format!("- {}:{}:{}\n", record.file_path, record.row, record.col);
    if let Some(anchor) = &record.owning_anchor {
        output.push_str(&format!("  anchor: {}\n", render_anchor_identity(anchor)));
    }
    output.push_str(&format!("  matched text: {}\n", record.matched_text));
    output.push_str(&format!("  preview: {}\n", record.preview));
    output
}

pub(crate) fn render_agenda_result(result: &AgendaResult) -> String {
    let mut output = format!("agenda entries: {}\n", result.nodes.len());
    for node in &result.nodes {
        output.push_str(&format!("- {}\n", render_anchor_identity(node)));
        if let Some(todo_keyword) = &node.todo_keyword {
            output.push_str(&format!("  todo: {todo_keyword}\n"));
        }
        if let Some(scheduled_for) = &node.scheduled_for {
            output.push_str(&format!("  scheduled: {scheduled_for}\n"));
        }
        if let Some(deadline_for) = &node.deadline_for {
            output.push_str(&format!("  deadline: {deadline_for}\n"));
        }
        if let Some(closed_at) = &node.closed_at {
            output.push_str(&format!("  closed: {closed_at}\n"));
        }
    }
    output
}

pub(crate) fn render_explore_result(result: &ExploreResult) -> String {
    let mut output = String::new();
    output.push_str(&format!("lens: {}\n", render_exploration_lens(result.lens)));
    for section in &result.sections {
        output.push('\n');
        output.push_str(&format!(
            "[{}]\n",
            render_exploration_section_kind(section.kind)
        ));
        if section.entries.is_empty() {
            output.push_str("(none)\n");
            continue;
        }
        for entry in &section.entries {
            render_exploration_entry(&mut output, entry);
        }
    }
    output
}

pub(crate) fn render_exploration_lens(lens: ExplorationLens) -> &'static str {
    match lens {
        ExplorationLens::Structure => "structure",
        ExplorationLens::Refs => "refs",
        ExplorationLens::Time => "time",
        ExplorationLens::Tasks => "tasks",
        ExplorationLens::Bridges => "bridges",
        ExplorationLens::Dormant => "dormant",
        ExplorationLens::Unresolved => "unresolved",
    }
}

pub(crate) fn render_exploration_section_kind(kind: ExplorationSectionKind) -> &'static str {
    match kind {
        ExplorationSectionKind::Backlinks => "backlinks",
        ExplorationSectionKind::ForwardLinks => "forward links",
        ExplorationSectionKind::Reflinks => "reflinks",
        ExplorationSectionKind::UnlinkedReferences => "unlinked references",
        ExplorationSectionKind::TimeNeighbors => "time neighbors",
        ExplorationSectionKind::TaskNeighbors => "task neighbors",
        ExplorationSectionKind::BridgeCandidates => "bridge candidates",
        ExplorationSectionKind::DormantNotes => "dormant notes",
        ExplorationSectionKind::UnresolvedTasks => "unresolved tasks",
        ExplorationSectionKind::WeaklyIntegratedNotes => "weakly integrated notes",
    }
}

pub(crate) fn render_exploration_entry(output: &mut String, entry: &ExplorationEntry) {
    match entry {
        ExplorationEntry::Backlink { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_node_identity(&record.source_note),
                record.row,
                record.col
            ));
            if let Some(anchor) = &record.source_anchor {
                output.push_str(&format!("  anchor: {}\n", render_anchor_identity(anchor)));
            }
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::ForwardLink { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_node_identity(&record.destination_note),
                record.row,
                record.col
            ));
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::Reflink { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_anchor_identity(&record.source_anchor),
                record.row,
                record.col
            ));
            output.push_str(&format!(
                "  matched reference: {}\n",
                record.matched_reference
            ));
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::UnlinkedReference { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_anchor_identity(&record.source_anchor),
                record.row,
                record.col
            ));
            output.push_str(&format!("  matched text: {}\n", record.matched_text));
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::Anchor { record } => {
            output.push_str(&format!("- {}\n", render_anchor_identity(&record.anchor)));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
    }
}

pub(crate) fn render_node_identity(node: &NodeRecord) -> String {
    format!(
        "{} [{}] {}:{}",
        node.title, node.node_key, node.file_path, node.line
    )
}

pub(crate) fn render_anchor_identity(anchor: &AnchorRecord) -> String {
    format!(
        "{} [{}] {}:{}",
        anchor.title, anchor.node_key, anchor.file_path, anchor.line
    )
}

pub(crate) fn render_exploration_explanation(explanation: &ExplorationExplanation) -> String {
    match explanation {
        ExplorationExplanation::Backlink => "backlink".to_owned(),
        ExplorationExplanation::ForwardLink => "forward link".to_owned(),
        ExplorationExplanation::SharedReference { reference } => {
            format!("shared reference {reference}")
        }
        ExplorationExplanation::UnlinkedReference { matched_text } => {
            format!("unlinked reference text match {matched_text}")
        }
        ExplorationExplanation::TimeNeighbor { relations } => {
            format!(
                "planning relations {}",
                render_planning_relations(relations)
            )
        }
        ExplorationExplanation::TaskNeighbor {
            shared_todo_keyword,
            planning_relations,
        } => {
            let mut parts = Vec::new();
            if let Some(keyword) = shared_todo_keyword {
                parts.push(format!("shared todo {keyword}"));
            }
            if !planning_relations.is_empty() {
                parts.push(format!(
                    "planning relations {}",
                    render_planning_relations(planning_relations)
                ));
            }
            parts.join("; ")
        }
        ExplorationExplanation::BridgeCandidate {
            references,
            via_notes,
        } => format!(
            "shared references {}; via {}",
            references.join(", "),
            via_notes
                .iter()
                .map(|note| format!("{} [{}]", note.title, note.node_key))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ExplorationExplanation::DormantSharedReference {
            references,
            modified_at_ns,
        } => format!(
            "shared references {}; modified_at_ns {}",
            references.join(", "),
            modified_at_ns
        ),
        ExplorationExplanation::UnresolvedSharedReference {
            references,
            todo_keyword,
        } => format!(
            "shared references {}; todo {}",
            references.join(", "),
            todo_keyword
        ),
        ExplorationExplanation::WeaklyIntegratedSharedReference {
            references,
            structural_link_count,
        } => format!(
            "shared references {}; structural link count {}",
            references.join(", "),
            structural_link_count
        ),
    }
}

pub(crate) fn render_planning_relations(relations: &[PlanningRelationRecord]) -> String {
    relations
        .iter()
        .map(|relation| {
            format!(
                "{}->{} {}",
                render_planning_field(relation.source_field),
                render_planning_field(relation.candidate_field),
                relation.date
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn render_planning_field(field: PlanningField) -> &'static str {
    match field {
        PlanningField::Scheduled => "scheduled",
        PlanningField::Deadline => "deadline",
    }
}

pub(crate) fn render_compare_result(
    result: &NoteComparisonResult,
    group: NoteComparisonGroup,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("group: {}\n", render_comparison_group(group)));
    output.push_str(&format!(
        "left: {}\n",
        render_node_identity(&result.left_note)
    ));
    output.push_str(&format!(
        "right: {}\n",
        render_node_identity(&result.right_note)
    ));
    for section in &result.sections {
        output.push('\n');
        output.push_str(&format!(
            "[{}]\n",
            render_comparison_section_kind(section.kind)
        ));
        if section.entries.is_empty() {
            output.push_str("(none)\n");
            continue;
        }
        for entry in &section.entries {
            render_comparison_entry(&mut output, entry);
        }
    }
    output
}

pub(crate) fn render_artifact_list(result: &ListExplorationArtifactsResult) -> String {
    let mut output = String::new();
    if result.artifacts.is_empty() {
        output.push_str("(none)\n");
        return output;
    }

    for artifact in &result.artifacts {
        output.push_str(&format!(
            "- {} [{}]\n",
            artifact.metadata.title,
            render_artifact_kind(artifact.kind)
        ));
        output.push_str(&format!(
            "  artifact id: {}\n",
            artifact.metadata.artifact_id
        ));
        if let Some(summary) = &artifact.metadata.summary {
            output.push_str(&format!("  summary: {summary}\n"));
        }
    }
    output
}

pub(crate) fn render_saved_exploration_artifact(artifact: &SavedExplorationArtifact) -> String {
    let mut output = String::new();
    render_artifact_metadata(&mut output, &artifact.metadata, artifact.kind());
    match &artifact.payload {
        slipbox_core::ExplorationArtifactPayload::LensView { artifact } => {
            render_saved_lens_view_artifact(&mut output, artifact);
        }
        slipbox_core::ExplorationArtifactPayload::Comparison { artifact } => {
            render_saved_comparison_artifact(&mut output, artifact);
        }
        slipbox_core::ExplorationArtifactPayload::Trail { artifact } => {
            render_saved_trail_artifact(&mut output, artifact);
        }
    }
    output
}

pub(crate) fn render_executed_exploration_artifact(
    artifact: &ExecutedExplorationArtifact,
) -> String {
    let mut output = String::new();
    render_artifact_metadata(&mut output, &artifact.metadata, artifact.kind());
    match &artifact.payload {
        ExecutedExplorationArtifactPayload::LensView {
            artifact,
            root_note,
            current_note,
            result,
        } => {
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            output.push_str(&format!(
                "current: {}\n",
                render_node_identity(current_note)
            ));
            render_saved_lens_view_state(&mut output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_explore_result(result));
        }
        ExecutedExplorationArtifactPayload::Comparison {
            artifact,
            root_note,
            result,
        } => {
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            render_saved_comparison_state(&mut output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_compare_result(result, NoteComparisonGroup::All));
        }
        ExecutedExplorationArtifactPayload::Trail { artifact, replay } => {
            render_saved_trail_state(&mut output, artifact);
            output.push('\n');
            output.push_str("[replay]\n");
            output.push_str(&render_trail_replay_result(replay));
        }
    }
    output
}

pub(crate) fn render_artifact_metadata(
    output: &mut String,
    metadata: &slipbox_core::ExplorationArtifactMetadata,
    kind: ExplorationArtifactKind,
) {
    output.push_str(&format!("artifact id: {}\n", metadata.artifact_id));
    output.push_str(&format!("title: {}\n", metadata.title));
    output.push_str(&format!("kind: {}\n", render_artifact_kind(kind)));
    if let Some(summary) = &metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
}

pub(crate) fn render_saved_lens_view_artifact(
    output: &mut String,
    artifact: &SavedLensViewArtifact,
) {
    render_saved_lens_view_state(output, artifact, "");
}

pub(crate) fn render_saved_lens_view_state(
    output: &mut String,
    artifact: &SavedLensViewArtifact,
    label_prefix: &str,
) {
    output.push_str(&format!(
        "{}root node key: {}\n",
        label_prefix, artifact.root_node_key
    ));
    output.push_str(&format!(
        "{}current node key: {}\n",
        label_prefix, artifact.current_node_key
    ));
    output.push_str(&format!(
        "{}lens: {}\n",
        label_prefix,
        render_exploration_lens(artifact.lens)
    ));
    output.push_str(&format!("{}limit: {}\n", label_prefix, artifact.limit));
    output.push_str(&format!("{}unique: {}\n", label_prefix, artifact.unique));
    output.push_str(&format!(
        "{}frozen context: {}\n",
        label_prefix, artifact.frozen_context
    ));
}

pub(crate) fn render_saved_comparison_artifact(
    output: &mut String,
    artifact: &SavedComparisonArtifact,
) {
    render_saved_comparison_state(output, artifact, "");
}

pub(crate) fn render_saved_comparison_state(
    output: &mut String,
    artifact: &SavedComparisonArtifact,
    label_prefix: &str,
) {
    output.push_str(&format!(
        "{}root node key: {}\n",
        label_prefix, artifact.root_node_key
    ));
    output.push_str(&format!(
        "{}left node key: {}\n",
        label_prefix, artifact.left_node_key
    ));
    output.push_str(&format!(
        "{}right node key: {}\n",
        label_prefix, artifact.right_node_key
    ));
    output.push_str(&format!(
        "{}active lens: {}\n",
        label_prefix,
        render_exploration_lens(artifact.active_lens)
    ));
    output.push_str(&format!(
        "{}comparison group: {}\n",
        label_prefix,
        render_comparison_group(artifact.comparison_group)
    ));
    output.push_str(&format!("{}limit: {}\n", label_prefix, artifact.limit));
    output.push_str(&format!(
        "{}structure unique: {}\n",
        label_prefix, artifact.structure_unique
    ));
    output.push_str(&format!(
        "{}frozen context: {}\n",
        label_prefix, artifact.frozen_context
    ));
}

pub(crate) fn render_saved_trail_artifact(output: &mut String, artifact: &SavedTrailArtifact) {
    render_saved_trail_state(output, artifact);
    for (index, step) in artifact.steps.iter().enumerate() {
        output.push('\n');
        output.push_str(&format!("[step {index}]\n"));
        render_saved_trail_step(output, step);
    }
    if let Some(step) = &artifact.detached_step {
        output.push('\n');
        output.push_str("[detached step]\n");
        render_saved_trail_step(output, step);
    }
}

pub(crate) fn render_saved_trail_state(output: &mut String, artifact: &SavedTrailArtifact) {
    output.push_str(&format!("steps: {}\n", artifact.steps.len()));
    output.push_str(&format!("cursor: {}\n", artifact.cursor));
    output.push_str(&format!(
        "detached step: {}\n",
        if artifact.detached_step.is_some() {
            "present"
        } else {
            "none"
        }
    ));
}

pub(crate) fn render_saved_trail_step(output: &mut String, step: &SavedTrailStep) {
    match step {
        SavedTrailStep::LensView { artifact } => {
            output.push_str("kind: lens-view\n");
            render_saved_lens_view_state(output, artifact, "");
        }
        SavedTrailStep::Comparison { artifact } => {
            output.push_str("kind: comparison\n");
            render_saved_comparison_state(output, artifact, "");
        }
    }
}

pub(crate) fn render_trail_replay_result(replay: &TrailReplayResult) -> String {
    let mut output = String::new();
    output.push_str(&format!("steps: {}\n", replay.steps.len()));
    output.push_str(&format!("cursor: {}\n", replay.cursor));
    output.push_str(&format!(
        "detached step: {}\n",
        if replay.detached_step.is_some() {
            "present"
        } else {
            "none"
        }
    ));
    for (index, step) in replay.steps.iter().enumerate() {
        output.push('\n');
        output.push_str(&format!("[step {index}]\n"));
        render_trail_replay_step(&mut output, step);
    }
    if let Some(step) = &replay.detached_step {
        output.push('\n');
        output.push_str("[detached step]\n");
        render_trail_replay_step(&mut output, step);
    }
    output
}

pub(crate) fn render_trail_replay_step(output: &mut String, step: &TrailReplayStepResult) {
    match step {
        TrailReplayStepResult::LensView {
            artifact,
            root_note,
            current_note,
            result,
        } => {
            output.push_str("kind: lens-view\n");
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            output.push_str(&format!(
                "current: {}\n",
                render_node_identity(current_note)
            ));
            render_saved_lens_view_state(output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_explore_result(result));
        }
        TrailReplayStepResult::Comparison {
            artifact,
            root_note,
            result,
        } => {
            output.push_str("kind: comparison\n");
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            render_saved_comparison_state(output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_compare_result(result, NoteComparisonGroup::All));
        }
    }
}

pub(crate) fn render_artifact_kind(kind: ExplorationArtifactKind) -> &'static str {
    match kind {
        ExplorationArtifactKind::LensView => "lens-view",
        ExplorationArtifactKind::Comparison => "comparison",
        ExplorationArtifactKind::Trail => "trail",
    }
}

pub(crate) fn render_comparison_group(group: NoteComparisonGroup) -> &'static str {
    match group {
        NoteComparisonGroup::All => "all",
        NoteComparisonGroup::Overlap => "overlap",
        NoteComparisonGroup::Divergence => "divergence",
        NoteComparisonGroup::Tension => "tension",
    }
}

pub(crate) fn render_comparison_section_kind(kind: NoteComparisonSectionKind) -> &'static str {
    match kind {
        NoteComparisonSectionKind::SharedRefs => "shared refs",
        NoteComparisonSectionKind::SharedPlanningDates => "shared planning dates",
        NoteComparisonSectionKind::LeftOnlyRefs => "left-only refs",
        NoteComparisonSectionKind::RightOnlyRefs => "right-only refs",
        NoteComparisonSectionKind::SharedBacklinks => "shared backlinks",
        NoteComparisonSectionKind::SharedForwardLinks => "shared forward links",
        NoteComparisonSectionKind::ContrastingTaskStates => "contrasting task states",
        NoteComparisonSectionKind::PlanningTensions => "planning tensions",
        NoteComparisonSectionKind::IndirectConnectors => "indirect connectors",
    }
}

pub(crate) fn render_comparison_entry(output: &mut String, entry: &NoteComparisonEntry) {
    match entry {
        NoteComparisonEntry::Reference { record } => {
            output.push_str(&format!("- {}\n", record.reference));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
        NoteComparisonEntry::Node { record } => {
            output.push_str(&format!("- {}\n", render_node_identity(&record.node)));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
        NoteComparisonEntry::PlanningRelation { record } => {
            output.push_str(&format!(
                "- {} {} <> {} {}\n",
                record.date,
                render_planning_field(record.left_field),
                render_planning_field(record.right_field),
                record.date
            ));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
        NoteComparisonEntry::TaskState { record } => {
            output.push_str(&format!(
                "- {} <> {}\n",
                record.left_todo_keyword, record.right_todo_keyword
            ));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
    }
}

pub(crate) fn render_note_comparison_explanation(
    explanation: &NoteComparisonExplanation,
) -> String {
    match explanation {
        NoteComparisonExplanation::SharedReference => "shared reference".to_owned(),
        NoteComparisonExplanation::SharedPlanningDate => "shared planning date".to_owned(),
        NoteComparisonExplanation::LeftOnlyReference => "left-only reference".to_owned(),
        NoteComparisonExplanation::RightOnlyReference => "right-only reference".to_owned(),
        NoteComparisonExplanation::SharedBacklink => "shared backlink".to_owned(),
        NoteComparisonExplanation::SharedForwardLink => "shared forward link".to_owned(),
        NoteComparisonExplanation::ContrastingTaskState => "contrasting task state".to_owned(),
        NoteComparisonExplanation::PlanningTension => "planning tension".to_owned(),
        NoteComparisonExplanation::IndirectConnector { direction } => {
            format!(
                "indirect connector {}",
                render_connector_direction(*direction)
            )
        }
    }
}

pub(crate) fn render_connector_direction(direction: ComparisonConnectorDirection) -> &'static str {
    match direction {
        ComparisonConnectorDirection::LeftToRight => "left-to-right",
        ComparisonConnectorDirection::RightToLeft => "right-to-left",
        ComparisonConnectorDirection::Bidirectional => "bidirectional",
    }
}
