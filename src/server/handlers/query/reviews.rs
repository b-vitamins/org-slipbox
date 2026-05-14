use std::fs;

use anyhow::anyhow;
use slipbox_core::{
    AuditRemediationApplyAction, CorpusAuditEntry, CorpusAuditKind, CorpusAuditParams,
    CorpusAuditResult, DeleteReviewRunResult, ListReviewRunsParams, ListReviewRunsResult,
    MarkReviewFindingParams, MarkReviewFindingResult, ReviewFinding, ReviewFindingPayload,
    ReviewFindingRemediationApplication, ReviewFindingRemediationApplyParams,
    ReviewFindingRemediationApplyResult, ReviewFindingRemediationPreview,
    ReviewFindingRemediationPreviewParams, ReviewFindingRemediationPreviewResult,
    ReviewFindingStatus, ReviewFindingStatusTransition, ReviewRun, ReviewRunDiff,
    ReviewRunDiffParams, ReviewRunDiffResult, ReviewRunIdParams, ReviewRunMetadata,
    ReviewRunPayload, ReviewRunResult, ReviewRunSummary, SaveCorpusAuditReviewParams,
    SaveCorpusAuditReviewResult, SaveReviewRunParams, SaveReviewRunResult,
    StructuralWriteIndexRefreshStatus,
};
use slipbox_rpc::JsonRpcError;

use super::common::{invalid_request, validate_review_id_params};
use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;

pub(super) fn save_review_run_with_policy(
    state: &mut ServerState,
    review: &ReviewRun,
    overwrite: bool,
) -> Result<ReviewRunSummary, JsonRpcError> {
    if overwrite {
        state
            .database
            .save_review_run(review)
            .map_err(|error| internal_error(error.context("failed to save review run")))?;
    } else if !state
        .database
        .save_review_run_if_absent(review)
        .map_err(|error| {
            internal_error(error.context("failed to save review run without overwrite"))
        })?
    {
        return Err(invalid_request(format!(
            "review run already exists: {}",
            review.metadata.review_id
        )));
    }

    Ok(ReviewRunSummary::from(review))
}

fn render_audit_kind(kind: CorpusAuditKind) -> &'static str {
    match kind {
        CorpusAuditKind::DanglingLinks => "dangling-links",
        CorpusAuditKind::DuplicateTitles => "duplicate-titles",
        CorpusAuditKind::OrphanNotes => "orphan-notes",
        CorpusAuditKind::WeaklyIntegratedNotes => "weakly-integrated-notes",
    }
}

fn title_for_audit_kind(kind: CorpusAuditKind) -> &'static str {
    match kind {
        CorpusAuditKind::DanglingLinks => "Dangling Links",
        CorpusAuditKind::DuplicateTitles => "Duplicate Titles",
        CorpusAuditKind::OrphanNotes => "Orphan Notes",
        CorpusAuditKind::WeaklyIntegratedNotes => "Weakly Integrated Notes",
    }
}

fn generated_audit_review_id(params: &CorpusAuditParams) -> String {
    format!(
        "review/audit/{}/limit-{}",
        render_audit_kind(params.audit),
        params.normalized_limit()
    )
}
fn audit_finding_id(entry: &CorpusAuditEntry) -> String {
    match entry {
        CorpusAuditEntry::DanglingLink { record } => format!(
            "audit/dangling-links/{}/{}/{}/{}",
            record.source.node_key, record.missing_explicit_id, record.line, record.column
        ),
        CorpusAuditEntry::DuplicateTitle { record } => {
            let mut node_keys = record
                .notes
                .iter()
                .map(|note| note.node_key.as_str())
                .collect::<Vec<_>>();
            node_keys.sort_unstable();
            format!("audit/duplicate-titles/{}", node_keys.join(","))
        }
        CorpusAuditEntry::OrphanNote { record } => {
            format!("audit/orphan-notes/{}", record.note.node_key)
        }
        CorpusAuditEntry::WeaklyIntegratedNote { record } => {
            format!("audit/weakly-integrated-notes/{}", record.note.node_key)
        }
    }
}

pub(super) fn review_from_audit_result(
    params: &SaveCorpusAuditReviewParams,
    result: &CorpusAuditResult,
) -> Result<ReviewRun, JsonRpcError> {
    let audit_params = params.audit_params();
    let metadata = ReviewRunMetadata {
        review_id: params
            .review_id
            .clone()
            .unwrap_or_else(|| generated_audit_review_id(&audit_params)),
        title: params
            .title
            .clone()
            .unwrap_or_else(|| format!("{} Review", title_for_audit_kind(result.audit))),
        summary: params.summary.clone().or_else(|| {
            Some(format!(
                "{} findings from {} audit with limit {}",
                result.entries.len(),
                render_audit_kind(result.audit),
                audit_params.normalized_limit()
            ))
        }),
    };
    let review = ReviewRun {
        metadata,
        payload: ReviewRunPayload::Audit {
            audit: result.audit,
            limit: audit_params.normalized_limit(),
        },
        findings: result
            .entries
            .iter()
            .map(|entry| ReviewFinding {
                finding_id: audit_finding_id(entry),
                status: ReviewFindingStatus::Open,
                payload: ReviewFindingPayload::Audit {
                    entry: Box::new(entry.clone()),
                },
            })
            .collect(),
    };
    if let Some(message) = review.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(review)
}
pub(super) fn known_review_run(
    state: &ServerState,
    review_id: &str,
) -> Result<ReviewRun, JsonRpcError> {
    let review = state
        .database
        .review_run(review_id)
        .map_err(|error| internal_error(error.context("failed to load review run")))?;
    review.ok_or_else(|| invalid_request(format!("unknown review run: {review_id}")))
}
pub(crate) fn save_review_run(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SaveReviewRunParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let review = save_review_run_with_policy(state, &params.review, params.overwrite)?;
    to_value(SaveReviewRunResult { review })
}

pub(crate) fn review_run(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewRunIdParams = parse_params(params)?;
    validate_review_id_params(&params)?;
    to_value(ReviewRunResult {
        review: known_review_run(state, &params.review_id)?,
    })
}

pub(crate) fn diff_review_runs(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewRunDiffParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let base = known_review_run(state, &params.base_review_id)?;
    let target = known_review_run(state, &params.target_review_id)?;
    let diff = ReviewRunDiff::between(&base, &target).map_err(invalid_request)?;
    to_value(ReviewRunDiffResult { diff })
}

pub(crate) fn review_finding_remediation_preview(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewFindingRemediationPreviewParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let review = known_review_run(state, &params.review_id)?;
    let finding = review
        .findings
        .iter()
        .find(|finding| finding.finding_id == params.finding_id)
        .ok_or_else(|| {
            invalid_request(format!(
                "unknown review finding {} in review run {}",
                params.finding_id, params.review_id
            ))
        })?;
    let preview = ReviewFindingRemediationPreview::from_review_finding(&params.review_id, finding)
        .map_err(invalid_request)?;
    to_value(ReviewFindingRemediationPreviewResult { preview })
}

pub(crate) fn review_finding_remediation_apply(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewFindingRemediationApplyParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }

    let review = known_review_run(state, &params.review_id)?;
    let finding = review
        .findings
        .iter()
        .find(|finding| finding.finding_id == params.finding_id)
        .ok_or_else(|| {
            invalid_request(format!(
                "unknown review finding {} in review run {}",
                params.finding_id, params.review_id
            ))
        })?;
    let current_preview =
        ReviewFindingRemediationPreview::from_review_finding(&params.review_id, finding)
            .map_err(invalid_request)?;
    if current_preview.preview_identity != params.expected_preview {
        return Err(invalid_request(format!(
            "stale remediation preview for finding {} in review run {}",
            params.finding_id, params.review_id
        )));
    }

    let application = apply_review_finding_remediation_action(state, params)?;
    to_value(ReviewFindingRemediationApplyResult { application })
}

fn apply_review_finding_remediation_action(
    state: &mut ServerState,
    params: ReviewFindingRemediationApplyParams,
) -> Result<ReviewFindingRemediationApplication, JsonRpcError> {
    let ReviewFindingRemediationApplyParams {
        review_id,
        finding_id,
        expected_preview,
        action,
    } = params;

    match &action {
        AuditRemediationApplyAction::UnlinkDanglingLink {
            file_path,
            line,
            column,
            preview,
            missing_explicit_id,
            replacement_text,
            ..
        } => {
            let (_relative_path, absolute_path) = state
                .resolve_index_path(file_path)
                .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
            if state
                .database
                .node_from_id(missing_explicit_id)
                .map_err(|error| {
                    internal_error(error.context("failed to recheck missing remediation target"))
                })?
                .is_some()
            {
                return Err(invalid_request(format!(
                    "cannot unlink dangling link because target id {missing_explicit_id} now resolves in the current index"
                )));
            }
            apply_unlink_dangling_link(
                &absolute_path,
                *line,
                *column,
                preview,
                missing_explicit_id,
                replacement_text,
            )?;
            state.sync_path(&absolute_path)?;
            let affected_files = state.structural_affected_files(&[absolute_path], &[])?;
            let application = ReviewFindingRemediationApplication {
                review_id,
                finding_id,
                preview_identity: expected_preview,
                action,
                affected_files,
                index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
            };
            if let Some(message) = application.validation_error() {
                return Err(internal_error(anyhow!(
                    "invalid remediation application: {message}"
                )));
            }
            Ok(application)
        }
    }
}

fn apply_unlink_dangling_link(
    path: &std::path::Path,
    line: u32,
    column: u32,
    expected_preview: &str,
    missing_explicit_id: &str,
    replacement_text: &str,
) -> Result<(), JsonRpcError> {
    let mut content = fs::read_to_string(path)
        .map_err(|error| internal_error(anyhow!("failed to read remediation target: {error}")))?;
    let (line_start, line_end) = line_bounds(&content, line).ok_or_else(|| {
        invalid_request(format!(
            "remediation action line {line} is outside {}",
            path.display()
        ))
    })?;
    let line_text = &content[line_start..line_end];
    if line_text.trim() != expected_preview {
        return Err(invalid_request(format!(
            "remediation action no longer matches file contents at {}:{line}:{column}",
            path.display()
        )));
    }

    let link_start = byte_index_for_column(line_text, column).ok_or_else(|| {
        invalid_request(format!(
            "remediation action column {column} is outside {}:{line}",
            path.display()
        ))
    })?;
    let suffix = &line_text[link_start..];
    if !suffix.starts_with("[[") {
        return Err(invalid_request(format!(
            "remediation action no longer points at an Org link at {}:{line}:{column}",
            path.display()
        )));
    }
    let link_end = suffix
        .find("]]")
        .map(|end| link_start + end + 2)
        .ok_or_else(|| {
            invalid_request(format!(
                "remediation action found an unterminated Org link at {}:{line}:{column}",
                path.display()
            ))
        })?;
    let link = &line_text[link_start..link_end];
    let (destination_id, label) = org_id_link_target_and_label(link).ok_or_else(|| {
        invalid_request(format!(
            "remediation action no longer points at an id link at {}:{line}:{column}",
            path.display()
        ))
    })?;
    if destination_id != missing_explicit_id {
        return Err(invalid_request(format!(
            "remediation action expected missing id {missing_explicit_id} but found {destination_id}"
        )));
    }
    let expected_replacement = label.unwrap_or(destination_id);
    if replacement_text != expected_replacement {
        return Err(invalid_request(format!(
            "unlink-dangling-link replacement_text must match the current link label: {expected_replacement}"
        )));
    }

    content.replace_range(
        line_start + link_start..line_start + link_end,
        replacement_text,
    );
    fs::write(path, content)
        .map_err(|error| internal_error(anyhow!("failed to write remediation target: {error}")))?;
    Ok(())
}

fn line_bounds(content: &str, line_number: u32) -> Option<(usize, usize)> {
    if line_number == 0 {
        return None;
    }
    let mut line_start = 0_usize;
    for (index, segment) in content.split_inclusive('\n').enumerate() {
        if index as u32 + 1 == line_number {
            let mut line_end = line_start + segment.len();
            if segment.ends_with('\n') {
                line_end -= 1;
                if line_end > line_start && content.as_bytes()[line_end - 1] == b'\r' {
                    line_end -= 1;
                }
            }
            return Some((line_start, line_end));
        }
        line_start += segment.len();
    }
    None
}

fn byte_index_for_column(line: &str, column: u32) -> Option<usize> {
    if column == 0 {
        return None;
    }
    if column == 1 {
        return Some(0);
    }
    line.char_indices()
        .nth(column as usize - 1)
        .map(|(index, _)| index)
}

fn org_id_link_target_and_label(link: &str) -> Option<(&str, Option<&str>)> {
    let inner = link.strip_prefix("[[")?.strip_suffix("]]")?;
    let (target, label) = inner
        .split_once("][")
        .map_or((inner, None), |(target, label)| (target, Some(label)));
    let destination_id = target.trim().strip_prefix("id:")?.trim();
    (!destination_id.is_empty()).then_some((destination_id, label))
}

pub(crate) fn list_review_runs(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let _params: ListReviewRunsParams = parse_params(params)?;
    let reviews = state
        .database
        .list_review_runs()
        .map_err(|error| internal_error(error.context("failed to list review runs")))?;
    to_value(ListReviewRunsResult {
        reviews: reviews.iter().map(ReviewRunSummary::from).collect(),
    })
}

pub(crate) fn delete_review_run(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewRunIdParams = parse_params(params)?;
    validate_review_id_params(&params)?;
    if !state
        .database
        .delete_review_run(&params.review_id)
        .map_err(|error| internal_error(error.context("failed to delete review run")))?
    {
        return Err(invalid_request(format!(
            "unknown review run: {}",
            params.review_id
        )));
    }
    to_value(DeleteReviewRunResult {
        review_id: params.review_id,
    })
}

pub(crate) fn mark_review_finding(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: MarkReviewFindingParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let mut review = known_review_run(state, &params.review_id)?;
    let finding = review
        .findings
        .iter_mut()
        .find(|finding| finding.finding_id == params.finding_id)
        .ok_or_else(|| {
            invalid_request(format!(
                "unknown review finding {} in review run {}",
                params.finding_id, params.review_id
            ))
        })?;
    let from_status = finding.status;
    let transition = ReviewFindingStatusTransition {
        review_id: params.review_id.clone(),
        finding_id: params.finding_id.clone(),
        from_status,
        to_status: params.status,
    };
    if let Some(message) = transition.validation_error() {
        return Err(invalid_request(message));
    }
    finding.status = params.status;
    state
        .database
        .save_review_run(&review)
        .map_err(|error| internal_error(error.context("failed to save review run")))?;
    to_value(MarkReviewFindingResult { transition })
}
pub(crate) fn corpus_audit(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: CorpusAuditParams = parse_params(params)?;
    to_value(execute_corpus_audit_query(state, &params)?)
}

pub(super) fn execute_corpus_audit_query(
    state: &mut ServerState,
    params: &CorpusAuditParams,
) -> Result<CorpusAuditResult, JsonRpcError> {
    let entries = match params.audit {
        CorpusAuditKind::DanglingLinks => state
            .database
            .audit_dangling_links(params.normalized_limit())
            .map_err(|error| internal_error(error.context("failed to query dangling link audit")))?
            .into_iter()
            .map(|record| CorpusAuditEntry::DanglingLink {
                record: Box::new(record),
            })
            .collect(),
        CorpusAuditKind::DuplicateTitles => state
            .database
            .audit_duplicate_titles(params.normalized_limit())
            .map_err(|error| {
                internal_error(error.context("failed to query duplicate title audit"))
            })?
            .into_iter()
            .map(|record| CorpusAuditEntry::DuplicateTitle {
                record: Box::new(record),
            })
            .collect(),
        CorpusAuditKind::OrphanNotes => state
            .database
            .audit_orphan_notes(params.normalized_limit())
            .map_err(|error| internal_error(error.context("failed to query orphan note audit")))?
            .into_iter()
            .map(|record| CorpusAuditEntry::OrphanNote {
                record: Box::new(record),
            })
            .collect(),
        CorpusAuditKind::WeaklyIntegratedNotes => state
            .database
            .audit_weakly_integrated_notes(params.normalized_limit())
            .map_err(|error| {
                internal_error(error.context("failed to query weakly integrated note audit"))
            })?
            .into_iter()
            .map(|record| CorpusAuditEntry::WeaklyIntegratedNote {
                record: Box::new(record),
            })
            .collect(),
    };
    Ok(CorpusAuditResult {
        audit: params.audit,
        entries,
    })
}

pub(crate) fn save_corpus_audit_review(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SaveCorpusAuditReviewParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let result = execute_corpus_audit_query(state, &params.audit_params())?;
    let review_run = review_from_audit_result(&params, &result)?;
    let review = save_review_run_with_policy(state, &review_run, params.overwrite)?;
    to_value(SaveCorpusAuditReviewResult { result, review })
}
