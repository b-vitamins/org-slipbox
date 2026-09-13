use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
use slipbox_core::{ReportProfileCatalog, ReviewRoutineCatalog};
use slipbox_core::{
    ReportProfileSpec, ReviewRoutineSource, ReviewRoutineSpec, WORKFLOW_SPEC_COMPATIBILITY_VERSION,
    WorkbenchPackManifest, WorkflowCatalogIssue, WorkflowCatalogIssueKind, WorkflowInputSpec,
    WorkflowSpec, WorkflowSpecCompatibilityEnvelope, WorkflowSummary, built_in_review_routines,
    built_in_workflows,
};

pub(super) struct WorkflowCatalog {
    workflows: Vec<WorkflowSpec>,
    review_routines: Vec<ReviewRoutineSpec>,
    report_profiles: Vec<ReportProfileSpec>,
    issues: Vec<WorkflowCatalogIssue>,
}

struct PackCatalogState<'a> {
    workflows: &'a mut Vec<WorkflowSpec>,
    review_routines: &'a mut Vec<ReviewRoutineSpec>,
    report_profiles: &'a mut Vec<ReportProfileSpec>,
    workflow_sources: &'a mut HashMap<String, String>,
    routine_sources: &'a mut HashMap<String, String>,
    profile_sources: &'a mut HashMap<String, String>,
    issues: &'a mut Vec<WorkflowCatalogIssue>,
}

impl WorkflowCatalog {
    pub(super) fn summaries(&self) -> Vec<WorkflowSummary> {
        self.workflows
            .iter()
            .map(WorkflowSummary::from)
            .collect::<Vec<_>>()
    }

    pub(super) fn workflow(&self, workflow_id: &str) -> Option<WorkflowSpec> {
        self.workflows
            .iter()
            .find(|workflow| workflow.metadata.workflow_id == workflow_id)
            .cloned()
    }

    pub(super) fn review_routine(&self, routine_id: &str) -> Option<ReviewRoutineSpec> {
        self.review_routines
            .iter()
            .find(|routine| routine.metadata.routine_id == routine_id)
            .cloned()
    }

    pub(super) fn review_routine_summaries(&self) -> Vec<slipbox_core::ReviewRoutineSummary> {
        self.review_routines
            .iter()
            .map(slipbox_core::ReviewRoutineSummary::from)
            .collect::<Vec<_>>()
    }

    pub(super) fn report_profile(&self, profile_id: &str) -> Option<ReportProfileSpec> {
        self.report_profiles
            .iter()
            .find(|profile| profile.metadata.profile_id == profile_id)
            .cloned()
    }

    #[cfg(test)]
    pub(super) fn review_routine_catalog(&self) -> ReviewRoutineCatalog {
        ReviewRoutineCatalog {
            routines: self.review_routines.clone(),
        }
    }

    #[cfg(test)]
    pub(super) fn report_profile_catalog(&self) -> ReportProfileCatalog {
        ReportProfileCatalog {
            profiles: self.report_profiles.clone(),
        }
    }

    pub(super) fn issues(&self) -> &[WorkflowCatalogIssue] {
        &self.issues
    }
}

pub(super) fn discover_workflow_catalog(
    root: &Path,
    workflow_dirs: &[PathBuf],
    packs: &[WorkbenchPackManifest],
) -> WorkflowCatalog {
    let mut workflows = built_in_workflows();
    let mut review_routines = built_in_review_routines();
    let mut report_profiles = Vec::new();
    let mut issues = Vec::new();
    let mut workflow_sources: HashMap<String, String> = workflows
        .iter()
        .map(|workflow| {
            (
                workflow.metadata.workflow_id.clone(),
                "built-in workflow".to_owned(),
            )
        })
        .collect();
    let mut routine_sources: HashMap<String, String> = review_routines
        .iter()
        .map(|routine| {
            (
                routine.metadata.routine_id.clone(),
                "built-in routine".to_owned(),
            )
        })
        .collect();
    let mut profile_sources: HashMap<String, String> = HashMap::new();

    for workflow_dir in workflow_dirs {
        let resolved_dir = if workflow_dir.is_absolute() {
            workflow_dir.clone()
        } else {
            root.join(workflow_dir)
        };
        collect_workflows_from_directory(
            &resolved_dir,
            &mut workflows,
            &mut workflow_sources,
            &mut issues,
        );
    }

    let mut sorted_packs = packs.iter().collect::<Vec<_>>();
    sorted_packs.sort_by(|left, right| {
        left.metadata
            .pack_id
            .cmp(&right.metadata.pack_id)
            .then_with(|| left.metadata.title.cmp(&right.metadata.title))
    });
    for pack in sorted_packs {
        let mut state = PackCatalogState {
            workflows: &mut workflows,
            review_routines: &mut review_routines,
            report_profiles: &mut report_profiles,
            workflow_sources: &mut workflow_sources,
            routine_sources: &mut routine_sources,
            profile_sources: &mut profile_sources,
            issues: &mut issues,
        };
        collect_assets_from_pack(pack, &mut state);
    }

    WorkflowCatalog {
        workflows,
        review_routines,
        report_profiles,
        issues,
    }
}

fn base_catalog_issue(
    path: impl Into<String>,
    kind: WorkflowCatalogIssueKind,
    message: impl Into<String>,
) -> WorkflowCatalogIssue {
    WorkflowCatalogIssue {
        path: path.into(),
        kind,
        pack_id: None,
        workflow_id: None,
        routine_id: None,
        profile_id: None,
        message: message.into(),
    }
}

fn workflow_catalog_issue(
    path: impl Into<String>,
    kind: WorkflowCatalogIssueKind,
    workflow_id: Option<String>,
    message: impl Into<String>,
) -> WorkflowCatalogIssue {
    WorkflowCatalogIssue {
        workflow_id,
        ..base_catalog_issue(path, kind, message)
    }
}

fn pack_catalog_issue(
    pack: &WorkbenchPackManifest,
    kind: WorkflowCatalogIssueKind,
    workflow_id: Option<String>,
    routine_id: Option<String>,
    profile_id: Option<String>,
    message: impl Into<String>,
) -> WorkflowCatalogIssue {
    WorkflowCatalogIssue {
        path: pack_source(pack),
        kind,
        pack_id: Some(pack.metadata.pack_id.clone()),
        workflow_id,
        routine_id,
        profile_id,
        message: message.into(),
    }
}

fn pack_source(pack: &WorkbenchPackManifest) -> String {
    format!("workbench pack {}", pack.metadata.pack_id)
}

fn duplicate_message(asset: &str, existing_source: &str) -> String {
    match existing_source {
        "built-in workflow" => format!("{asset} collides with built-in workflow"),
        source => format!("{asset} collides with {source}"),
    }
}

fn collect_workflows_from_directory(
    directory: &Path,
    workflows: &mut Vec<WorkflowSpec>,
    sources: &mut HashMap<String, String>,
    issues: &mut Vec<WorkflowCatalogIssue>,
) {
    let path = directory.display().to_string();
    if !directory.exists() {
        issues.push(base_catalog_issue(
            path,
            WorkflowCatalogIssueKind::Directory,
            "configured workflow directory does not exist",
        ));
        return;
    }
    if !directory.is_dir() {
        issues.push(base_catalog_issue(
            path,
            WorkflowCatalogIssueKind::Directory,
            "configured workflow directory is not a directory",
        ));
        return;
    }

    let mut entries = match fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry.path()),
                Err(error) => {
                    issues.push(base_catalog_issue(
                        directory.display().to_string(),
                        WorkflowCatalogIssueKind::Io,
                        format!("failed to read workflow directory entry: {error}"),
                    ));
                    None
                }
            })
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            })
            .collect::<Vec<_>>(),
        Err(error) => {
            issues.push(base_catalog_issue(
                directory.display().to_string(),
                WorkflowCatalogIssueKind::Io,
                format!("failed to read workflow directory: {error}"),
            ));
            return;
        }
    };
    entries.sort();

    for entry in entries {
        load_workflow_spec_from_file(&entry, workflows, sources, issues);
    }
}

fn load_workflow_spec_from_file(
    path: &Path,
    workflows: &mut Vec<WorkflowSpec>,
    sources: &mut HashMap<String, String>,
    issues: &mut Vec<WorkflowCatalogIssue>,
) {
    let display_path = path.display().to_string();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            issues.push(base_catalog_issue(
                display_path,
                WorkflowCatalogIssueKind::Io,
                format!("failed to read workflow spec JSON: {error}"),
            ));
            return;
        }
    };

    let compatibility: WorkflowSpecCompatibilityEnvelope = match serde_json::from_slice(&bytes) {
        Ok(compatibility) => compatibility,
        Err(error) => {
            issues.push(workflow_catalog_issue(
                display_path,
                WorkflowCatalogIssueKind::MalformedJson,
                None,
                format!("failed to parse workflow spec JSON: {error}"),
            ));
            return;
        }
    };

    if let Some(message) = compatibility.compatibility.validation_error() {
        let kind = if compatibility.compatibility.version > WORKFLOW_SPEC_COMPATIBILITY_VERSION {
            WorkflowCatalogIssueKind::UnsupportedVersion
        } else {
            WorkflowCatalogIssueKind::InvalidSpec
        };
        issues.push(workflow_catalog_issue(
            display_path,
            kind,
            compatibility.workflow_id,
            message,
        ));
        return;
    }

    let workflow: WorkflowSpec = match serde_json::from_slice(&bytes) {
        Ok(workflow) => workflow,
        Err(error) => {
            issues.push(workflow_catalog_issue(
                display_path,
                WorkflowCatalogIssueKind::MalformedJson,
                compatibility.workflow_id,
                format!("failed to parse workflow spec JSON: {error}"),
            ));
            return;
        }
    };

    add_workflow(workflow, display_path, None, workflows, sources, issues);
}

fn collect_assets_from_pack(pack: &WorkbenchPackManifest, state: &mut PackCatalogState<'_>) {
    if let Some(message) = pack.metadata.validation_error() {
        state.issues.push(pack_catalog_issue(
            pack,
            WorkflowCatalogIssueKind::InvalidPack,
            None,
            None,
            None,
            message,
        ));
        return;
    }
    if let Some(message) = pack.compatibility.validation_error() {
        state.issues.push(pack_catalog_issue(
            pack,
            WorkflowCatalogIssueKind::UnsupportedVersion,
            None,
            None,
            None,
            message,
        ));
        return;
    }

    let source = pack_source(pack);
    for workflow in &pack.workflows {
        add_workflow(
            workflow.clone(),
            source.clone(),
            Some(pack),
            state.workflows,
            state.workflow_sources,
            state.issues,
        );
    }
    for profile in &pack.report_profiles {
        add_report_profile(
            profile.clone(),
            pack,
            state.report_profiles,
            state.profile_sources,
            state.issues,
        );
    }
    for routine in &pack.review_routines {
        add_review_routine(
            routine.clone(),
            pack,
            state.workflows,
            state.report_profiles,
            state.review_routines,
            state.routine_sources,
            state.issues,
        );
    }
}

fn add_workflow(
    workflow: WorkflowSpec,
    source: String,
    pack: Option<&WorkbenchPackManifest>,
    workflows: &mut Vec<WorkflowSpec>,
    sources: &mut HashMap<String, String>,
    issues: &mut Vec<WorkflowCatalogIssue>,
) {
    if let Some(message) = workflow.validation_error() {
        if let Some(pack) = pack {
            issues.push(pack_catalog_issue(
                pack,
                WorkflowCatalogIssueKind::InvalidSpec,
                Some(workflow.metadata.workflow_id),
                None,
                None,
                message,
            ));
        } else {
            issues.push(workflow_catalog_issue(
                source,
                WorkflowCatalogIssueKind::InvalidSpec,
                Some(workflow.metadata.workflow_id),
                message,
            ));
        }
        return;
    }

    let workflow_id = workflow.metadata.workflow_id.clone();
    if let Some(existing_source) = sources.get(&workflow_id) {
        let message = duplicate_message("workflow_id", existing_source);
        if let Some(pack) = pack {
            issues.push(pack_catalog_issue(
                pack,
                WorkflowCatalogIssueKind::DuplicateWorkflowId,
                Some(workflow_id),
                None,
                None,
                message,
            ));
        } else {
            issues.push(workflow_catalog_issue(
                source,
                WorkflowCatalogIssueKind::DuplicateWorkflowId,
                Some(workflow_id),
                message,
            ));
        }
        return;
    }

    let source_label = pack
        .map(pack_source)
        .unwrap_or_else(|| format!("discovered workflow from {source}"));
    sources.insert(workflow_id, source_label);
    workflows.push(workflow);
}

fn add_report_profile(
    profile: ReportProfileSpec,
    pack: &WorkbenchPackManifest,
    report_profiles: &mut Vec<ReportProfileSpec>,
    sources: &mut HashMap<String, String>,
    issues: &mut Vec<WorkflowCatalogIssue>,
) {
    if let Some(message) = profile.validation_error() {
        issues.push(pack_catalog_issue(
            pack,
            WorkflowCatalogIssueKind::InvalidReportProfile,
            None,
            None,
            Some(profile.metadata.profile_id),
            message,
        ));
        return;
    }

    let profile_id = profile.metadata.profile_id.clone();
    if let Some(existing_source) = sources.get(&profile_id) {
        issues.push(pack_catalog_issue(
            pack,
            WorkflowCatalogIssueKind::DuplicateReportProfileId,
            None,
            None,
            Some(profile_id),
            duplicate_message("profile_id", existing_source),
        ));
        return;
    }

    sources.insert(profile_id, pack_source(pack));
    report_profiles.push(profile);
}

fn add_review_routine(
    routine: ReviewRoutineSpec,
    pack: &WorkbenchPackManifest,
    workflows: &[WorkflowSpec],
    report_profiles: &[ReportProfileSpec],
    review_routines: &mut Vec<ReviewRoutineSpec>,
    sources: &mut HashMap<String, String>,
    issues: &mut Vec<WorkflowCatalogIssue>,
) {
    if let Some(message) = routine.validation_error().or_else(|| {
        validate_review_routine_catalog_references(&routine, workflows, report_profiles)
    }) {
        issues.push(pack_catalog_issue(
            pack,
            WorkflowCatalogIssueKind::InvalidReviewRoutine,
            None,
            Some(routine.metadata.routine_id),
            None,
            message,
        ));
        return;
    }

    let routine_id = routine.metadata.routine_id.clone();
    if let Some(existing_source) = sources.get(&routine_id) {
        issues.push(pack_catalog_issue(
            pack,
            WorkflowCatalogIssueKind::DuplicateReviewRoutineId,
            None,
            Some(routine_id),
            None,
            duplicate_message("routine_id", existing_source),
        ));
        return;
    }

    sources.insert(routine_id, pack_source(pack));
    review_routines.push(routine);
}

fn validate_review_routine_catalog_references(
    routine: &ReviewRoutineSpec,
    workflows: &[WorkflowSpec],
    report_profiles: &[ReportProfileSpec],
) -> Option<String> {
    if let ReviewRoutineSource::Workflow { workflow_id } = &routine.source {
        let Some(workflow) = workflows
            .iter()
            .find(|workflow| workflow.metadata.workflow_id == *workflow_id)
        else {
            return Some(format!(
                "review routine {} references missing workflow_id {workflow_id}",
                routine.metadata.routine_id
            ));
        };
        if let Some(message) =
            validate_review_routine_workflow_inputs(routine, workflow.inputs.as_slice())
        {
            return Some(message);
        }
    }

    if let Some(compare) = &routine.compare
        && let Some(profile_id) = &compare.report_profile_id
        && !report_profiles
            .iter()
            .any(|profile| profile.metadata.profile_id == *profile_id)
    {
        return Some(format!(
            "review routine {} references missing profile_id {profile_id}",
            routine.metadata.routine_id
        ));
    }

    for profile_id in &routine.report_profile_ids {
        if !report_profiles
            .iter()
            .any(|profile| profile.metadata.profile_id == *profile_id)
        {
            return Some(format!(
                "review routine {} references missing profile_id {profile_id}",
                routine.metadata.routine_id
            ));
        }
    }

    None
}

fn validate_review_routine_workflow_inputs(
    routine: &ReviewRoutineSpec,
    workflow_inputs: &[WorkflowInputSpec],
) -> Option<String> {
    for input in &routine.inputs {
        match workflow_inputs
            .iter()
            .find(|workflow_input| workflow_input.input_id == input.input_id)
        {
            Some(workflow_input) if workflow_input.kind == input.kind => {}
            Some(workflow_input) => {
                return Some(format!(
                    "review routine {} declares input_id {} as {}, but referenced workflow requires {}",
                    routine.metadata.routine_id,
                    input.input_id,
                    input.kind.label(),
                    workflow_input.kind.label()
                ));
            }
            None => {
                return Some(format!(
                    "review routine {} declares input_id {} that referenced workflow does not accept",
                    routine.metadata.routine_id, input.input_id
                ));
            }
        }
    }

    for workflow_input in workflow_inputs {
        if !routine
            .inputs
            .iter()
            .any(|input| input.input_id == workflow_input.input_id)
        {
            return Some(format!(
                "review routine {} is missing input_id {} required by referenced workflow",
                routine.metadata.routine_id, workflow_input.input_id
            ));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use slipbox_core::{
        CorpusAuditKind, ReportProfileMetadata, ReportProfileMode, ReportProfileSubject,
        ReviewRoutineComparePolicy, ReviewRoutineCompareTarget, ReviewRoutineMetadata,
        ReviewRoutineSaveReviewPolicy, WorkbenchPackCompatibility, WorkbenchPackMetadata,
        WorkflowMetadata, WorkflowSpecCompatibility, WorkflowStepPayload, WorkflowStepSpec,
    };
    use tempfile::tempdir;

    use super::*;

    fn workflow(workflow_id: &str, title: &str) -> WorkflowSpec {
        WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: workflow_id.to_owned(),
                title: title.to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: slipbox_core::WorkflowResolveTarget::NodeKey {
                        node_key: "file:sample.org".to_owned(),
                    },
                },
            }],
        }
    }

    fn report_profile(profile_id: &str, title: &str) -> ReportProfileSpec {
        ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: profile_id.to_owned(),
                title: title.to_owned(),
                summary: None,
            },
            subjects: vec![ReportProfileSubject::Audit],
            mode: ReportProfileMode::Detail,
            status_filters: None,
            diff_buckets: None,
            jsonl_line_kinds: None,
        }
    }

    fn routine(routine_id: &str, workflow_id: &str, profile_id: &str) -> ReviewRoutineSpec {
        ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: routine_id.to_owned(),
                title: "Routine".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Workflow {
                workflow_id: workflow_id.to_owned(),
            },
            inputs: Vec::new(),
            save_review: ReviewRoutineSaveReviewPolicy::default(),
            compare: None,
            report_profile_ids: vec![profile_id.to_owned()],
        }
    }

    fn audit_routine(routine_id: &str, profile_id: &str) -> ReviewRoutineSpec {
        ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: routine_id.to_owned(),
                title: "Audit Routine".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Audit {
                audit: CorpusAuditKind::DuplicateTitles,
                limit: 200,
            },
            inputs: Vec::new(),
            save_review: ReviewRoutineSaveReviewPolicy::default(),
            compare: Some(ReviewRoutineComparePolicy {
                target: ReviewRoutineCompareTarget::LatestCompatibleReview,
                report_profile_id: Some(profile_id.to_owned()),
            }),
            report_profile_ids: vec![profile_id.to_owned()],
        }
    }

    fn pack(pack_id: &str) -> WorkbenchPackManifest {
        WorkbenchPackManifest {
            metadata: WorkbenchPackMetadata {
                pack_id: pack_id.to_owned(),
                title: pack_id.to_owned(),
                summary: None,
            },
            compatibility: WorkbenchPackCompatibility::default(),
            workflows: Vec::new(),
            review_routines: Vec::new(),
            report_profiles: Vec::new(),
            entrypoint_routine_ids: Vec::new(),
        }
    }

    #[test]
    fn catalog_merges_built_ins_directories_and_packs_with_deterministic_precedence() {
        let workspace = tempdir().expect("workspace should be created");
        let workflow_dir = workspace.path().join("workflows");
        fs::create_dir_all(&workflow_dir).expect("workflow dir should be created");
        let dir_workflow = workflow("workflow/test/context", "Directory Context");
        fs::write(
            workflow_dir.join("context.json"),
            serde_json::to_vec_pretty(&dir_workflow).expect("workflow should serialize"),
        )
        .expect("workflow file should be written");

        let mut later_pack = pack("pack/z-later");
        later_pack.workflows = vec![workflow("workflow/pack/shared", "Later Shared")];
        let mut earlier_pack = pack("pack/a-earlier");
        earlier_pack.workflows = vec![
            workflow("workflow/pack/shared", "Earlier Shared"),
            workflow("workflow/test/context", "Shadowed Directory Context"),
            workflow(
                slipbox_core::BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
                "Shadowed Built-In",
            ),
        ];

        let catalog = discover_workflow_catalog(
            workspace.path(),
            &[workflow_dir],
            &[later_pack.clone(), earlier_pack.clone()],
        );

        assert!(
            catalog
                .workflow(slipbox_core::BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID)
                .is_some()
        );
        assert_eq!(
            catalog
                .workflow("workflow/test/context")
                .expect("directory workflow should win")
                .metadata
                .title,
            "Directory Context"
        );
        assert_eq!(
            catalog
                .workflow("workflow/pack/shared")
                .expect("earlier pack should win")
                .metadata
                .title,
            "Earlier Shared"
        );
        assert!(catalog.issues().iter().any(|issue| {
            issue.kind == WorkflowCatalogIssueKind::DuplicateWorkflowId
                && issue.pack_id.as_deref() == Some("pack/z-later")
                && issue.workflow_id.as_deref() == Some("workflow/pack/shared")
        }));
        assert!(catalog.issues().iter().any(|issue| {
            issue.kind == WorkflowCatalogIssueKind::DuplicateWorkflowId
                && issue.pack_id.as_deref() == Some("pack/a-earlier")
                && issue.workflow_id.as_deref() == Some("workflow/test/context")
        }));
        assert!(catalog.issues().iter().any(|issue| {
            issue.kind == WorkflowCatalogIssueKind::DuplicateWorkflowId
                && issue.pack_id.as_deref() == Some("pack/a-earlier")
                && issue.workflow_id.as_deref()
                    == Some(slipbox_core::BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID)
        }));
    }

    #[test]
    fn pack_entries_populate_routine_and_profile_catalogs_with_isolated_issues() {
        let workspace = tempdir().expect("workspace should be created");
        let mut manifest = pack("pack/catalog");
        manifest.workflows = vec![
            workflow("workflow/pack/valid", "Valid Workflow"),
            WorkflowSpec {
                steps: Vec::new(),
                ..workflow("workflow/pack/invalid", "Invalid Workflow")
            },
        ];
        manifest.report_profiles = vec![
            report_profile("profile/pack/detail", "Detail"),
            ReportProfileSpec {
                subjects: Vec::new(),
                ..report_profile("profile/pack/invalid", "Invalid")
            },
            report_profile("profile/pack/detail", "Duplicate Detail"),
        ];
        manifest.review_routines = vec![
            routine(
                "routine/pack/valid",
                "workflow/pack/valid",
                "profile/pack/detail",
            ),
            routine(
                "routine/pack/missing-workflow",
                "workflow/pack/missing",
                "profile/pack/detail",
            ),
            audit_routine("routine/pack/missing-profile", "profile/pack/missing"),
        ];

        let catalog = discover_workflow_catalog(workspace.path(), &[], &[manifest]);
        assert!(catalog.workflow("workflow/pack/valid").is_some());
        assert!(catalog.workflow("workflow/pack/invalid").is_none());

        let profile_catalog = catalog.report_profile_catalog();
        assert_eq!(profile_catalog.profiles.len(), 1);
        assert_eq!(
            profile_catalog.profiles[0].metadata.profile_id,
            "profile/pack/detail"
        );

        let routine_catalog = catalog.review_routine_catalog();
        assert!(
            routine_catalog
                .routines
                .iter()
                .any(|routine| routine.metadata.routine_id == "routine/pack/valid")
        );
        assert!(
            !routine_catalog
                .routines
                .iter()
                .any(|routine| routine.metadata.routine_id == "routine/pack/missing-workflow")
        );
        assert!(
            !routine_catalog
                .routines
                .iter()
                .any(|routine| routine.metadata.routine_id == "routine/pack/missing-profile")
        );

        assert!(catalog.issues().iter().any(|issue| {
            issue.kind == WorkflowCatalogIssueKind::InvalidSpec
                && issue.workflow_id.as_deref() == Some("workflow/pack/invalid")
        }));
        assert!(catalog.issues().iter().any(|issue| {
            issue.kind == WorkflowCatalogIssueKind::InvalidReportProfile
                && issue.profile_id.as_deref() == Some("profile/pack/invalid")
        }));
        assert!(catalog.issues().iter().any(|issue| {
            issue.kind == WorkflowCatalogIssueKind::DuplicateReportProfileId
                && issue.profile_id.as_deref() == Some("profile/pack/detail")
        }));
        assert!(catalog.issues().iter().any(|issue| {
            issue.kind == WorkflowCatalogIssueKind::InvalidReviewRoutine
                && issue.routine_id.as_deref() == Some("routine/pack/missing-workflow")
                && issue.message.contains("references missing workflow_id")
        }));
        assert!(catalog.issues().iter().any(|issue| {
            issue.kind == WorkflowCatalogIssueKind::InvalidReviewRoutine
                && issue.routine_id.as_deref() == Some("routine/pack/missing-profile")
                && issue.message.contains("references missing profile_id")
        }));

        let serialized_issue = serde_json::to_value(
            catalog
                .issues()
                .iter()
                .find(|issue| issue.profile_id.as_deref() == Some("profile/pack/invalid"))
                .expect("profile issue should exist"),
        )
        .expect("issue should serialize");
        assert_eq!(
            serialized_issue,
            json!({
                "path": "workbench pack pack/catalog",
                "kind": "invalid-report-profile",
                "pack_id": "pack/catalog",
                "workflow_id": null,
                "profile_id": "profile/pack/invalid",
                "message": "report profiles must select at least one subject"
            })
        );
    }
}
