use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use slipbox_core::{
    WORKFLOW_SPEC_COMPATIBILITY_VERSION, WorkflowCatalogIssue, WorkflowCatalogIssueKind,
    WorkflowSpec, WorkflowSpecCompatibilityEnvelope, WorkflowSummary, built_in_workflows,
};

pub(super) struct WorkflowCatalog {
    workflows: Vec<WorkflowSpec>,
    issues: Vec<WorkflowCatalogIssue>,
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

    pub(super) fn issues(&self) -> &[WorkflowCatalogIssue] {
        &self.issues
    }
}

pub(super) fn discover_workflow_catalog(root: &Path, workflow_dirs: &[PathBuf]) -> WorkflowCatalog {
    let mut workflows = built_in_workflows();
    let mut issues = Vec::new();
    let mut sources: HashMap<String, String> = workflows
        .iter()
        .map(|workflow| {
            (
                workflow.metadata.workflow_id.clone(),
                "built-in workflow".to_owned(),
            )
        })
        .collect();

    for workflow_dir in workflow_dirs {
        let resolved_dir = if workflow_dir.is_absolute() {
            workflow_dir.clone()
        } else {
            root.join(workflow_dir)
        };
        collect_workflows_from_directory(&resolved_dir, &mut workflows, &mut sources, &mut issues);
    }

    WorkflowCatalog { workflows, issues }
}

fn collect_workflows_from_directory(
    directory: &Path,
    workflows: &mut Vec<WorkflowSpec>,
    sources: &mut HashMap<String, String>,
    issues: &mut Vec<WorkflowCatalogIssue>,
) {
    let path = directory.display().to_string();
    if !directory.exists() {
        issues.push(WorkflowCatalogIssue {
            path,
            kind: WorkflowCatalogIssueKind::Directory,
            workflow_id: None,
            message: "configured workflow directory does not exist".to_owned(),
        });
        return;
    }
    if !directory.is_dir() {
        issues.push(WorkflowCatalogIssue {
            path,
            kind: WorkflowCatalogIssueKind::Directory,
            workflow_id: None,
            message: "configured workflow directory is not a directory".to_owned(),
        });
        return;
    }

    let mut entries = match fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry.path()),
                Err(error) => {
                    issues.push(WorkflowCatalogIssue {
                        path: directory.display().to_string(),
                        kind: WorkflowCatalogIssueKind::Io,
                        workflow_id: None,
                        message: format!("failed to read workflow directory entry: {error}"),
                    });
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
            issues.push(WorkflowCatalogIssue {
                path: directory.display().to_string(),
                kind: WorkflowCatalogIssueKind::Io,
                workflow_id: None,
                message: format!("failed to read workflow directory: {error}"),
            });
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
            issues.push(WorkflowCatalogIssue {
                path: display_path,
                kind: WorkflowCatalogIssueKind::Io,
                workflow_id: None,
                message: format!("failed to read workflow spec JSON: {error}"),
            });
            return;
        }
    };

    let compatibility: WorkflowSpecCompatibilityEnvelope = match serde_json::from_slice(&bytes) {
        Ok(compatibility) => compatibility,
        Err(error) => {
            issues.push(WorkflowCatalogIssue {
                path: display_path,
                kind: WorkflowCatalogIssueKind::MalformedJson,
                workflow_id: None,
                message: format!("failed to parse workflow spec JSON: {error}"),
            });
            return;
        }
    };

    if let Some(message) = compatibility.compatibility.validation_error() {
        let kind = if compatibility.compatibility.version > WORKFLOW_SPEC_COMPATIBILITY_VERSION {
            WorkflowCatalogIssueKind::UnsupportedVersion
        } else {
            WorkflowCatalogIssueKind::InvalidSpec
        };
        issues.push(WorkflowCatalogIssue {
            path: display_path,
            kind,
            workflow_id: compatibility.workflow_id,
            message,
        });
        return;
    }

    let workflow: WorkflowSpec = match serde_json::from_slice(&bytes) {
        Ok(workflow) => workflow,
        Err(error) => {
            issues.push(WorkflowCatalogIssue {
                path: display_path,
                kind: WorkflowCatalogIssueKind::MalformedJson,
                workflow_id: compatibility.workflow_id,
                message: format!("failed to parse workflow spec JSON: {error}"),
            });
            return;
        }
    };

    if let Some(message) = workflow.validation_error() {
        issues.push(WorkflowCatalogIssue {
            path: display_path,
            kind: WorkflowCatalogIssueKind::InvalidSpec,
            workflow_id: Some(workflow.metadata.workflow_id.clone()),
            message,
        });
        return;
    }

    let workflow_id = workflow.metadata.workflow_id.clone();
    if let Some(existing_source) = sources.get(&workflow_id) {
        let message = if existing_source == "built-in workflow" {
            "workflow_id collides with built-in workflow".to_owned()
        } else {
            format!("workflow_id collides with discovered workflow from {existing_source}")
        };
        issues.push(WorkflowCatalogIssue {
            path: display_path,
            kind: WorkflowCatalogIssueKind::DuplicateWorkflowId,
            workflow_id: Some(workflow_id),
            message,
        });
        return;
    }

    sources.insert(workflow_id, display_path);
    workflows.push(workflow);
}
