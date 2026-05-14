use crate::{
    nodes::{AnchorRecord, NodeKind, NodeRecord},
    packs::{WorkbenchPackIssue, WorkbenchPackIssueKind, WorkbenchPackManifest},
    reports::{ReportJsonlLineKind, ReportProfileMode, ReportProfileSpec, ReportProfileSubject},
    review::{
        ReviewFinding, ReviewFindingPayload, ReviewFindingStatus, ReviewRunDiffBucket,
        ReviewRunPayload,
    },
    routine::{ReviewRoutineSource, ReviewRoutineSpec, built_in_review_routines},
    workflow::{
        WorkflowArtifactSaveSource, WorkflowExploreFocus, WorkflowInputAssignment,
        WorkflowInputKind, WorkflowInputSpec, WorkflowResolveTarget, WorkflowStepKind,
        WorkflowStepPayload, WorkflowSummary, built_in_workflows,
    },
    write::{
        StructuralWriteAffectedFiles, StructuralWriteOperationKind, StructuralWritePreviewResult,
        StructuralWriteResult,
    },
};

pub(crate) const fn default_search_limit() -> usize {
    50
}

pub(crate) const fn default_backlink_limit() -> usize {
    200
}

pub(crate) const fn default_artifact_overwrite() -> bool {
    true
}

pub(crate) const fn default_review_overwrite() -> bool {
    true
}

pub(crate) const fn default_audit_limit() -> usize {
    200
}

pub(crate) const fn default_tag_limit() -> usize {
    200
}

pub(crate) const fn default_agenda_limit() -> usize {
    200
}

pub(crate) const fn default_ref_limit() -> usize {
    50
}

pub(crate) const fn default_graph_max_title_length() -> usize {
    100
}

pub(crate) const fn default_heading_level() -> u32 {
    1
}

#[must_use]
pub fn normalize_reference(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if let Some(inner) = trimmed
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    {
        let target = inner.split_once("][").map_or(inner, |(path, _)| path);
        return normalize_reference(target);
    }

    if let Some(key) = trimmed.strip_prefix('@') {
        return normalize_cite_keys(key);
    }

    if let Some(inner) = trimmed
        .strip_prefix("[cite:")
        .and_then(|value| value.strip_suffix(']'))
    {
        return extract_org_cite_keys(inner);
    }

    if let Some(path) = trimmed.strip_prefix("cite:") {
        return normalize_cite_keys(path);
    }

    vec![trimmed.to_owned()]
}

fn normalize_cite_keys(input: &str) -> Vec<String> {
    input
        .split([',', ';'])
        .filter_map(|part| {
            let key = part
                .trim()
                .trim_start_matches('@')
                .trim_start_matches("cite:")
                .trim();
            if key.is_empty() {
                None
            } else {
                Some(format!("@{key}"))
            }
        })
        .collect()
}

fn extract_org_cite_keys(input: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut current = String::new();
    let mut collecting = false;

    for character in input.chars() {
        if collecting {
            if is_cite_key_char(character) {
                current.push(character);
                continue;
            }
            if !current.is_empty() {
                refs.push(format!("@{current}"));
                current.clear();
            }
            collecting = false;
        }

        if character == '@' {
            collecting = true;
        }
    }

    if collecting && !current.is_empty() {
        refs.push(format!("@{current}"));
    }

    if refs.is_empty() {
        normalize_cite_keys(input)
    } else {
        refs
    }
}

fn is_cite_key_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':' | '.')
}

pub(crate) fn normalize_string_values(values: &[String], nocase: bool) -> Vec<String> {
    let mut normalized = Vec::new();

    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty()
            || normalized.iter().any(|existing: &String| {
                if nocase {
                    existing.eq_ignore_ascii_case(trimmed)
                } else {
                    existing == trimmed
                }
            })
        {
            continue;
        }

        normalized.push(trimmed.to_owned());
    }

    normalized
}

pub(crate) fn validate_required_text_field(value: &str, field: &str) -> Option<String> {
    value
        .trim()
        .is_empty()
        .then(|| format!("{field} must not be empty"))
}

pub(crate) fn validate_positive_position(value: u32, field: &str) -> Option<String> {
    (value == 0).then(|| format!("{field} must be a positive 1-based position"))
}

pub(crate) fn validate_review_node_key_set(
    node_keys: &[String],
    field: &str,
    minimum_len: usize,
) -> Option<String> {
    if node_keys.len() < minimum_len {
        return Some(format!(
            "{field} must include at least {minimum_len} node keys"
        ));
    }

    let mut seen: Vec<&str> = Vec::with_capacity(node_keys.len());
    for (index, node_key) in node_keys.iter().enumerate() {
        if let Some(error) = validate_required_text_field(node_key, field) {
            return Some(format!("{field} entry {index} is invalid: {error}"));
        }
        if seen.contains(&node_key.as_str()) {
            return Some(format!("{field} entry {index} is duplicate: {node_key}"));
        }
        seen.push(node_key);
    }
    None
}

pub(crate) fn validate_artifact_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "artifact_id").or_else(|| {
        (value.trim() != value)
            .then(|| "artifact_id must not have leading or trailing whitespace".to_owned())
    })
}

pub(crate) fn validate_workflow_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "workflow_id").or_else(|| {
        (value.trim() != value)
            .then(|| "workflow_id must not have leading or trailing whitespace".to_owned())
    })
}

pub(crate) fn validate_workflow_input_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "input_id").or_else(|| {
        (value.trim() != value)
            .then(|| "input_id must not have leading or trailing whitespace".to_owned())
    })
}

pub(crate) fn validate_workflow_step_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "step_id").or_else(|| {
        (value.trim() != value)
            .then(|| "step_id must not have leading or trailing whitespace".to_owned())
    })
}

pub(crate) fn validate_review_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "review_id").or_else(|| {
        (value.trim() != value)
            .then(|| "review_id must not have leading or trailing whitespace".to_owned())
    })
}

pub(crate) fn validate_optional_review_id_field(
    value: Option<&str>,
    field: &str,
) -> Option<String> {
    value.and_then(|review_id| {
        validate_required_text_field(review_id, field).or_else(|| {
            (review_id.trim() != review_id)
                .then(|| format!("{field} must not have leading or trailing whitespace"))
        })
    })
}

pub(crate) fn validate_review_finding_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "finding_id").or_else(|| {
        (value.trim() != value)
            .then(|| "finding_id must not have leading or trailing whitespace".to_owned())
    })
}

pub(crate) fn validate_review_routine_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "routine_id").or_else(|| {
        (value.trim() != value)
            .then(|| "routine_id must not have leading or trailing whitespace".to_owned())
    })
}

pub(crate) fn validate_workbench_pack_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "pack_id").or_else(|| {
        (value.trim() != value)
            .then(|| "pack_id must not have leading or trailing whitespace".to_owned())
    })
}

pub(crate) fn validate_report_profile_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "profile_id").or_else(|| {
        (value.trim() != value)
            .then(|| "profile_id must not have leading or trailing whitespace".to_owned())
    })
}

pub(crate) fn validate_optional_report_profile_id_field(
    value: Option<&str>,
    field: &str,
) -> Option<String> {
    value.and_then(|profile_id| {
        validate_required_text_field(profile_id, field).or_else(|| {
            (profile_id.trim() != profile_id)
                .then(|| format!("{field} must not have leading or trailing whitespace"))
        })
    })
}

pub(crate) fn validate_optional_text_field(value: Option<&str>, field: &str) -> Option<String> {
    value.and_then(|text| validate_required_text_field(text, field))
}

pub(crate) fn validate_structural_write_file_path_field(
    value: &str,
    field: &str,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(format!("{field} must not be empty"));
    }
    if trimmed != value {
        return Some(format!(
            "{field} must not have leading or trailing whitespace"
        ));
    }
    if value.starts_with('/') || value.starts_with('\\') {
        return Some(format!("{field} must be relative to the slipbox root"));
    }
    if value.contains('\\') {
        return Some(format!("{field} must use forward slashes"));
    }
    if !value.ends_with(".org") {
        return Some(format!("{field} must target an .org file"));
    }
    if value
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Some(format!("{field} must be a normalized relative path"));
    }
    None
}

pub(crate) fn validate_structural_write_file_paths(
    paths: &[String],
    field: &str,
) -> Option<String> {
    let mut seen: Vec<&str> = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        if let Some(error) = validate_structural_write_file_path_field(path, field) {
            return Some(format!("{field} entry {index} is invalid: {error}"));
        }
        if seen.contains(&path.as_str()) {
            return Some(format!("{field} entry {index} is duplicate: {path}"));
        }
        seen.push(path);
    }
    None
}

pub(crate) fn validate_structural_write_operation_files(
    operation: StructuralWriteOperationKind,
    affected_files: &StructuralWriteAffectedFiles,
) -> Option<String> {
    (!operation.permits_removed_files() && !affected_files.removed_files.is_empty()).then(|| {
        format!(
            "{} structural writes must not include removed files",
            operation.label()
        )
    })
}

pub(crate) fn validate_structural_write_result_requirement(
    operation: StructuralWriteOperationKind,
    result: Option<&StructuralWriteResult>,
) -> Option<String> {
    match (operation.requires_result(), result.is_some()) {
        (true, false) => Some(format!(
            "{} structural write reports must include a resulting node or anchor",
            operation.label()
        )),
        (false, true) => Some(format!(
            "{} structural write reports must not include a resulting node or anchor",
            operation.label()
        )),
        _ => None,
    }
}

pub(crate) fn validate_structural_write_preview_result_requirement(
    operation: StructuralWriteOperationKind,
    result: Option<&StructuralWritePreviewResult>,
) -> Option<String> {
    match (operation.requires_result(), result.is_some()) {
        (true, false) => Some(format!(
            "{} structural write previews must include an expected result",
            operation.label()
        )),
        (false, true) => Some(format!(
            "{} structural write previews must not include an expected result",
            operation.label()
        )),
        _ => None,
    }
}

pub(crate) fn validate_structural_write_result_file(
    file_path: &str,
    affected_files: &StructuralWriteAffectedFiles,
) -> Option<String> {
    (!affected_files
        .changed_files
        .iter()
        .any(|changed| changed == file_path))
    .then(|| format!("structural write result file {file_path} must be in changed_files"))
}

pub(crate) fn validate_structural_write_result_node(node: &NodeRecord) -> Option<String> {
    validate_required_text_field(&node.node_key, "node.node_key")
        .or_else(|| validate_structural_write_file_path_field(&node.file_path, "node.file_path"))
        .or_else(|| validate_required_text_field(&node.title, "node.title"))
        .or_else(|| {
            (node.kind == NodeKind::Heading && node.explicit_id.is_none()).then(|| {
                "structural write result nodes must be file notes or headings with explicit IDs"
                    .to_owned()
            })
        })
}

pub(crate) fn validate_structural_write_result_anchor(anchor: &AnchorRecord) -> Option<String> {
    validate_required_text_field(&anchor.node_key, "anchor.node_key")
        .or_else(|| {
            validate_structural_write_file_path_field(&anchor.file_path, "anchor.file_path")
        })
        .or_else(|| validate_required_text_field(&anchor.title, "anchor.title"))
}

pub(crate) fn validate_report_profile_subjects(
    subjects: &[ReportProfileSubject],
) -> Option<String> {
    if subjects.is_empty() {
        return Some("report profiles must select at least one subject".to_owned());
    }

    let mut seen: Vec<ReportProfileSubject> = Vec::with_capacity(subjects.len());
    for (index, subject) in subjects.iter().copied().enumerate() {
        if seen.contains(&subject) {
            return Some(format!(
                "report profile subject {index} is duplicate: {}",
                subject.label()
            ));
        }
        seen.push(subject);
    }
    None
}

pub(crate) fn validate_report_profile_status_filters(
    profile: &ReportProfileSpec,
) -> Option<String> {
    let Some(status_filters) = &profile.status_filters else {
        return None;
    };
    if status_filters.is_empty() {
        return Some("report profile status_filters must not be empty when present".to_owned());
    }
    if !profile
        .subjects
        .iter()
        .any(|subject| report_profile_subject_supports_status_filters(*subject))
    {
        return Some(
            "report profile status_filters require a review, routine, or diff subject".to_owned(),
        );
    }

    let mut seen: Vec<ReviewFindingStatus> = Vec::with_capacity(status_filters.len());
    for (index, status) in status_filters.iter().copied().enumerate() {
        if seen.contains(&status) {
            return Some(format!(
                "report profile status_filters entry {index} is duplicate: {}",
                status.label()
            ));
        }
        seen.push(status);
    }
    None
}

pub(crate) fn validate_report_profile_diff_buckets(profile: &ReportProfileSpec) -> Option<String> {
    let Some(diff_buckets) = &profile.diff_buckets else {
        return None;
    };
    if diff_buckets.is_empty() {
        return Some("report profile diff_buckets must not be empty when present".to_owned());
    }
    if !profile.subjects.contains(&ReportProfileSubject::Diff) {
        return Some("report profile diff_buckets require a diff subject".to_owned());
    }

    let mut seen: Vec<ReviewRunDiffBucket> = Vec::with_capacity(diff_buckets.len());
    for (index, bucket) in diff_buckets.iter().copied().enumerate() {
        if seen.contains(&bucket) {
            return Some(format!(
                "report profile diff_buckets entry {index} is duplicate: {}",
                bucket.label()
            ));
        }
        seen.push(bucket);
    }
    None
}

pub(crate) fn validate_report_profile_jsonl_line_kinds(
    profile: &ReportProfileSpec,
) -> Option<String> {
    let Some(line_kinds) = &profile.jsonl_line_kinds else {
        return None;
    };
    if line_kinds.is_empty() {
        return Some("report profile jsonl_line_kinds must not be empty when present".to_owned());
    }

    let mut seen: Vec<&str> = Vec::with_capacity(line_kinds.len());
    for (index, line_kind) in line_kinds.iter().enumerate() {
        let label = line_kind.label();
        if label.trim().is_empty() {
            return Some(format!(
                "report profile jsonl_line_kinds entry {index} must not be empty"
            ));
        }
        if !line_kind.is_supported() {
            return Some(format!(
                "report profile jsonl_line_kinds entry {index} is unsupported: {label}"
            ));
        }
        if seen.contains(&label) {
            return Some(format!(
                "report profile jsonl_line_kinds entry {index} is duplicate: {label}"
            ));
        }
        if matches!(profile.mode, ReportProfileMode::Summary) && line_kind.is_detail_line() {
            return Some(format!(
                "report profile summary mode cannot select detail JSONL line kind: {label}"
            ));
        }
        if !profile
            .subjects
            .iter()
            .any(|subject| report_profile_subject_supports_line_kind(*subject, line_kind))
        {
            return Some(format!(
                "report profile jsonl_line_kinds entry {index} is not supported by selected subjects: {label}"
            ));
        }
        seen.push(label);
    }
    None
}

const fn report_profile_subject_supports_status_filters(subject: ReportProfileSubject) -> bool {
    matches!(
        subject,
        ReportProfileSubject::Review | ReportProfileSubject::Routine | ReportProfileSubject::Diff
    )
}

const fn report_profile_subject_supports_line_kind(
    subject: ReportProfileSubject,
    line_kind: &ReportJsonlLineKind,
) -> bool {
    subject.supports_line_kind(line_kind)
}

pub(crate) fn validate_review_routine_inputs(routine: &ReviewRoutineSpec) -> Option<String> {
    if matches!(routine.source, ReviewRoutineSource::Audit { .. }) && !routine.inputs.is_empty() {
        return Some("audit review routines cannot declare workflow inputs".to_owned());
    }

    let mut seen: Vec<&str> = Vec::with_capacity(routine.inputs.len());
    for (index, input) in routine.inputs.iter().enumerate() {
        if let Some(error) = input.validation_error() {
            return Some(format!("review routine input {index} is invalid: {error}"));
        }
        if seen.contains(&input.input_id.as_str()) {
            return Some(format!(
                "review routine input {index} reuses duplicate input_id {}",
                input.input_id
            ));
        }
        seen.push(input.input_id.as_str());
    }
    None
}

pub(crate) fn validate_review_routine_compare_policy(
    routine: &ReviewRoutineSpec,
) -> Option<String> {
    let Some(compare) = &routine.compare else {
        return None;
    };
    if !routine.save_review.enabled {
        return Some("review routine compare policy requires save_review to be enabled".to_owned());
    }
    compare.validation_error()
}

pub(crate) fn validate_review_routine_report_profiles(
    report_profile_ids: &[String],
) -> Option<String> {
    let mut seen: Vec<&str> = Vec::with_capacity(report_profile_ids.len());
    for (index, profile_id) in report_profile_ids.iter().enumerate() {
        if let Some(error) = validate_report_profile_id_field(profile_id) {
            return Some(format!(
                "review routine report_profile_ids entry {index} is invalid: {error}"
            ));
        }
        if seen.contains(&profile_id.as_str()) {
            return Some(format!(
                "review routine report_profile_ids entry {index} is duplicate: {profile_id}"
            ));
        }
        seen.push(profile_id.as_str());
    }
    None
}

pub(crate) fn validate_workbench_pack_manifest(
    manifest: &WorkbenchPackManifest,
) -> Vec<WorkbenchPackIssue> {
    let mut issues = Vec::new();

    if let Some(error) = manifest.metadata.validation_error() {
        issues.push(workbench_pack_issue(
            WorkbenchPackIssueKind::InvalidMetadata,
            Some(manifest.metadata.pack_id.clone()),
            error,
        ));
    }
    if let Some(error) = manifest.compatibility.validation_error() {
        issues.push(workbench_pack_issue(
            WorkbenchPackIssueKind::UnsupportedVersion,
            Some(manifest.metadata.pack_id.clone()),
            error,
        ));
    }
    if manifest.workflows.is_empty()
        && manifest.review_routines.is_empty()
        && manifest.report_profiles.is_empty()
    {
        issues.push(workbench_pack_issue(
            WorkbenchPackIssueKind::EmptyPack,
            Some(manifest.metadata.pack_id.clone()),
            "workbench packs must contain at least one workflow, review routine, or report profile",
        ));
    }

    let built_in_workflows = built_in_workflows();
    let built_in_routines = built_in_review_routines();
    let mut workflow_inputs: Vec<(&str, &[WorkflowInputSpec])> = built_in_workflows
        .iter()
        .map(|workflow| {
            (
                workflow.metadata.workflow_id.as_str(),
                workflow.inputs.as_slice(),
            )
        })
        .collect();
    let mut workflow_ids = Vec::with_capacity(manifest.workflows.len());
    for (index, workflow) in manifest.workflows.iter().enumerate() {
        let collides_with_built_in = built_in_workflows
            .iter()
            .any(|built_in| built_in.metadata.workflow_id == workflow.metadata.workflow_id);
        if let Some(error) = workflow.validation_error() {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::InvalidWorkflow,
                Some(workflow.metadata.workflow_id.clone()),
                format!("workbench pack workflow {index} is invalid: {error}"),
            ));
        } else if !collides_with_built_in {
            workflow_inputs.push((
                workflow.metadata.workflow_id.as_str(),
                workflow.inputs.as_slice(),
            ));
        }
        if collides_with_built_in {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateWorkflowId,
                Some(workflow.metadata.workflow_id.clone()),
                format!(
                    "workbench pack workflow {index} collides with built-in workflow_id {}",
                    workflow.metadata.workflow_id
                ),
            ));
        }
        if workflow_ids.contains(&workflow.metadata.workflow_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateWorkflowId,
                Some(workflow.metadata.workflow_id.clone()),
                format!(
                    "workbench pack workflow {index} reuses duplicate workflow_id {}",
                    workflow.metadata.workflow_id
                ),
            ));
        }
        workflow_ids.push(workflow.metadata.workflow_id.as_str());
    }

    let mut report_profile_ids = Vec::with_capacity(manifest.report_profiles.len());
    for (index, profile) in manifest.report_profiles.iter().enumerate() {
        if let Some(error) = profile.validation_error() {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::InvalidReportProfile,
                Some(profile.metadata.profile_id.clone()),
                format!("workbench pack report profile {index} is invalid: {error}"),
            ));
        }
        if report_profile_ids.contains(&profile.metadata.profile_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateReportProfileId,
                Some(profile.metadata.profile_id.clone()),
                format!(
                    "workbench pack report profile {index} reuses duplicate profile_id {}",
                    profile.metadata.profile_id
                ),
            ));
        }
        report_profile_ids.push(profile.metadata.profile_id.as_str());
    }

    let mut routine_ids = Vec::with_capacity(manifest.review_routines.len());
    for (index, routine) in manifest.review_routines.iter().enumerate() {
        let collides_with_built_in = built_in_routines
            .iter()
            .any(|built_in| built_in.metadata.routine_id == routine.metadata.routine_id);
        if let Some(error) = routine.validation_error() {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::InvalidReviewRoutine,
                Some(routine.metadata.routine_id.clone()),
                format!("workbench pack review routine {index} is invalid: {error}"),
            ));
        }
        if collides_with_built_in {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateReviewRoutineId,
                Some(routine.metadata.routine_id.clone()),
                format!(
                    "workbench pack review routine {index} collides with built-in routine_id {}",
                    routine.metadata.routine_id
                ),
            ));
        }
        if routine_ids.contains(&routine.metadata.routine_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateReviewRoutineId,
                Some(routine.metadata.routine_id.clone()),
                format!(
                    "workbench pack review routine {index} reuses duplicate routine_id {}",
                    routine.metadata.routine_id
                ),
            ));
        }
        validate_workbench_pack_routine_references(
            routine,
            &workflow_inputs,
            &report_profile_ids,
            &mut issues,
        );
        routine_ids.push(routine.metadata.routine_id.as_str());
    }

    validate_workbench_pack_entrypoint_routine_references(
        &manifest.entrypoint_routine_ids,
        &routine_ids,
        &mut issues,
    );

    issues
}

fn workbench_pack_issue(
    kind: WorkbenchPackIssueKind,
    asset_id: Option<String>,
    message: impl Into<String>,
) -> WorkbenchPackIssue {
    WorkbenchPackIssue {
        kind,
        asset_id,
        message: message.into(),
    }
}

pub(crate) fn validate_workbench_pack_routine_references(
    routine: &ReviewRoutineSpec,
    workflow_inputs: &[(&str, &[WorkflowInputSpec])],
    report_profile_ids: &[&str],
    issues: &mut Vec<WorkbenchPackIssue>,
) {
    if let ReviewRoutineSource::Workflow { workflow_id } = &routine.source {
        match workflow_inputs
            .iter()
            .find(|(candidate_id, _)| *candidate_id == workflow_id.as_str())
            .map(|(_, inputs)| *inputs)
        {
            Some(inputs) => {
                if let Some(error) = validate_workbench_pack_routine_inputs(routine, inputs) {
                    issues.push(workbench_pack_issue(
                        WorkbenchPackIssueKind::InvalidReviewRoutineReference,
                        Some(routine.metadata.routine_id.clone()),
                        error,
                    ));
                }
            }
            None => issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::MissingWorkflowReference,
                Some(routine.metadata.routine_id.clone()),
                format!(
                    "review routine {} references missing workflow_id {workflow_id}",
                    routine.metadata.routine_id
                ),
            )),
        }
    }

    if let Some(compare) = &routine.compare {
        if let Some(profile_id) = &compare.report_profile_id {
            validate_workbench_pack_report_profile_reference(
                &routine.metadata.routine_id,
                profile_id,
                report_profile_ids,
                issues,
            );
        }
    }
    for profile_id in &routine.report_profile_ids {
        validate_workbench_pack_report_profile_reference(
            &routine.metadata.routine_id,
            profile_id,
            report_profile_ids,
            issues,
        );
    }
}

pub(crate) fn validate_workbench_pack_routine_inputs(
    routine: &ReviewRoutineSpec,
    workflow_inputs: &[WorkflowInputSpec],
) -> Option<String> {
    for input in &routine.inputs {
        let Some(workflow_input) = workflow_inputs
            .iter()
            .find(|workflow_input| workflow_input.input_id == input.input_id)
        else {
            return Some(format!(
                "review routine {} declares input_id {} that referenced workflow does not accept",
                routine.metadata.routine_id, input.input_id
            ));
        };
        if workflow_input.kind != input.kind {
            return Some(format!(
                "review routine {} declares input_id {} as {}, but referenced workflow requires {}",
                routine.metadata.routine_id,
                input.input_id,
                input.kind.label(),
                workflow_input.kind.label()
            ));
        }
    }

    workflow_inputs
        .iter()
        .find(|workflow_input| {
            !routine
                .inputs
                .iter()
                .any(|input| input.input_id == workflow_input.input_id)
        })
        .map(|workflow_input| {
            format!(
                "review routine {} is missing input_id {} required by referenced workflow",
                routine.metadata.routine_id, workflow_input.input_id
            )
        })
}

pub(crate) fn validate_workbench_pack_report_profile_reference(
    routine_id: &str,
    profile_id: &str,
    report_profile_ids: &[&str],
    issues: &mut Vec<WorkbenchPackIssue>,
) {
    if !report_profile_ids.contains(&profile_id) {
        issues.push(workbench_pack_issue(
            WorkbenchPackIssueKind::MissingReportProfileReference,
            Some(routine_id.to_owned()),
            format!("review routine {routine_id} references missing profile_id {profile_id}"),
        ));
    }
}

pub(crate) fn validate_workbench_pack_entrypoint_routine_references(
    entrypoint_routine_ids: &[String],
    routine_ids: &[&str],
    issues: &mut Vec<WorkbenchPackIssue>,
) {
    let mut seen = Vec::with_capacity(entrypoint_routine_ids.len());
    for (index, routine_id) in entrypoint_routine_ids.iter().enumerate() {
        if let Some(error) = validate_review_routine_id_field(routine_id) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::InvalidReviewRoutineReference,
                Some(routine_id.clone()),
                format!("workbench pack entrypoint_routine_ids entry {index} is invalid: {error}"),
            ));
            continue;
        }
        if seen.contains(&routine_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateReviewRoutineReference,
                Some(routine_id.clone()),
                format!(
                    "workbench pack entrypoint_routine_ids entry {index} is duplicate: {routine_id}"
                ),
            ));
        }
        if !routine_ids.contains(&routine_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::MissingReviewRoutineReference,
                Some(routine_id.clone()),
                format!(
                    "workbench pack entrypoint_routine_ids entry {index} references missing routine_id {routine_id}"
                ),
            ));
        }
        seen.push(routine_id.as_str());
    }
}

fn workflow_step_kind_for_reference(
    seen: &[(&str, WorkflowStepKind)],
    step_id: &str,
) -> Option<WorkflowStepKind> {
    seen.iter()
        .find_map(|(existing_id, kind)| (*existing_id == step_id).then_some(*kind))
}

pub(crate) fn validate_workflow_step_reference(
    seen: &[(&str, WorkflowStepKind)],
    step_id: &str,
    expected: WorkflowStepKind,
    field: &str,
) -> Option<String> {
    match workflow_step_kind_for_reference(seen, step_id) {
        Some(kind) if kind == expected => None,
        Some(kind) => Some(format!(
            "{field} must reference a {} step, not {}",
            expected.label(),
            kind.label()
        )),
        None => Some(format!(
            "{field} must reference an earlier {} step",
            expected.label()
        )),
    }
}

fn workflow_input_kind_for_reference(
    inputs: &[(&str, WorkflowInputKind)],
    input_id: &str,
) -> Option<WorkflowInputKind> {
    inputs
        .iter()
        .find_map(|(existing_id, kind)| (*existing_id == input_id).then_some(*kind))
}

pub(crate) fn validate_workflow_input_reference_kind(
    inputs: &[(&str, WorkflowInputKind)],
    input_id: &str,
    required_kind: WorkflowInputKind,
    role: &str,
) -> Option<String> {
    match workflow_input_kind_for_reference(inputs, input_id) {
        None => Some(format!("{role} must reference a declared workflow input")),
        Some(kind) if kind != required_kind => Some(format!(
            "{role} must reference a declared {} input",
            match required_kind {
                WorkflowInputKind::NoteTarget => "note-target",
                WorkflowInputKind::FocusTarget => "focus-target",
            }
        )),
        Some(_) => None,
    }
}

pub(crate) fn validate_workflow_step_references(
    payload: &WorkflowStepPayload,
    seen: &[(&str, WorkflowStepKind)],
    inputs: &[(&str, WorkflowInputKind)],
) -> Option<String> {
    match payload {
        WorkflowStepPayload::Resolve { target } => match target {
            WorkflowResolveTarget::Input { input_id } => {
                (workflow_input_kind_for_reference(inputs, input_id).is_none())
                    .then(|| "target must reference a declared workflow input".to_owned())
            }
            WorkflowResolveTarget::Id { .. }
            | WorkflowResolveTarget::Title { .. }
            | WorkflowResolveTarget::Reference { .. }
            | WorkflowResolveTarget::NodeKey { .. } => None,
        },
        WorkflowStepPayload::ArtifactRun { .. } => None,
        WorkflowStepPayload::Explore { focus, .. } => match focus {
            WorkflowExploreFocus::NodeKey { .. } => None,
            WorkflowExploreFocus::Input { input_id } => validate_workflow_input_reference_kind(
                inputs,
                input_id,
                WorkflowInputKind::FocusTarget,
                "focus",
            ),
            WorkflowExploreFocus::ResolvedStep { step_id } => {
                validate_workflow_step_reference(seen, step_id, WorkflowStepKind::Resolve, "focus")
            }
        },
        WorkflowStepPayload::Compare { left, right, .. } => {
            validate_workflow_step_reference(seen, &left.step_id, WorkflowStepKind::Resolve, "left")
                .or_else(|| {
                    validate_workflow_step_reference(
                        seen,
                        &right.step_id,
                        WorkflowStepKind::Resolve,
                        "right",
                    )
                })
        }
        WorkflowStepPayload::ArtifactSave { source, .. } => match source {
            WorkflowArtifactSaveSource::ExploreStep { step_id } => {
                validate_workflow_step_reference(seen, step_id, WorkflowStepKind::Explore, "source")
            }
            WorkflowArtifactSaveSource::CompareStep { step_id } => {
                validate_workflow_step_reference(seen, step_id, WorkflowStepKind::Compare, "source")
            }
        },
    }
}

pub(crate) fn validate_workflow_review_source(
    workflow: &WorkflowSummary,
    step_ids: &[String],
) -> Option<String> {
    if workflow.step_count == 0 {
        return Some("workflow review source must contain at least one step".to_owned());
    }
    if step_ids.len() != workflow.step_count {
        return Some("workflow review source step_ids must match workflow step_count".to_owned());
    }

    let mut seen: Vec<&str> = Vec::with_capacity(step_ids.len());
    for (index, step_id) in step_ids.iter().enumerate() {
        if let Some(error) = validate_workflow_step_id_field(step_id) {
            return Some(format!(
                "workflow review source step_id {index} is invalid: {error}"
            ));
        }
        if seen.iter().any(|existing| *existing == step_id) {
            return Some(format!(
                "workflow review source step_id {index} reuses duplicate step_id {step_id}"
            ));
        }
        seen.push(step_id.as_str());
    }

    None
}

pub(crate) fn validate_workflow_review_inputs(
    inputs: &[WorkflowInputAssignment],
) -> Option<String> {
    let mut seen: Vec<&str> = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.iter().enumerate() {
        if let Some(error) = input.validation_error() {
            return Some(format!("workflow review input {index} is invalid: {error}"));
        }
        if seen.contains(&input.input_id.as_str()) {
            return Some(format!(
                "workflow review input {index} reuses duplicate input_id {}",
                input.input_id
            ));
        }
        seen.push(input.input_id.as_str());
    }
    None
}

pub(crate) fn validate_review_finding_matches_run(
    payload: &ReviewRunPayload,
    finding: &ReviewFinding,
) -> Option<String> {
    match (payload, &finding.payload) {
        (ReviewRunPayload::Audit { audit, .. }, ReviewFindingPayload::Audit { entry }) => {
            (entry.kind() != *audit)
                .then(|| "audit review findings must match review audit kind".to_owned())
        }
        (
            ReviewRunPayload::Workflow { step_ids, .. },
            ReviewFindingPayload::WorkflowStep { step },
        ) => (!step_ids.iter().any(|step_id| step_id == &step.step_id))
            .then(|| "workflow-step findings must reference a source workflow step".to_owned()),
        (ReviewRunPayload::Audit { .. }, ReviewFindingPayload::WorkflowStep { .. }) => {
            Some("audit review runs cannot contain workflow-step findings".to_owned())
        }
        (ReviewRunPayload::Workflow { .. }, ReviewFindingPayload::Audit { .. }) => {
            Some("workflow review runs cannot contain audit findings".to_owned())
        }
    }
}
