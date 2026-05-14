use super::{
    assets::{render_workflow_resolve_target, render_workflow_step_report},
    explorations::{render_anchor_identity, render_node_identity},
    notes::render_structural_index_refresh,
};
use slipbox_core::{
    AuditRemediationApplyAction, AuditRemediationConfidence, AuditRemediationPreviewPayload,
    CorpusAuditEntry, CorpusAuditKind, CorpusAuditResult, ListReviewRunsResult,
    MarkReviewFindingResult, ReviewFinding, ReviewFindingKind, ReviewFindingPair,
    ReviewFindingPayload, ReviewFindingRemediationApplyResult, ReviewFindingRemediationPreview,
    ReviewFindingStatus, ReviewFindingStatusDiff, ReviewRun, ReviewRunDiff, ReviewRunKind,
    ReviewRunPayload, ReviewRunSummary,
};

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

pub(crate) fn push_indented(output: &mut String, text: &str, indent: &str) {
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
