use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::{
    artifacts::{
        ExecutedExplorationArtifact, ExplorationArtifactMetadata, ExplorationArtifactSummary,
    },
    exploration::{
        ExplorationLens, ExploreParams, ExploreResult, NoteComparisonGroup, NoteComparisonResult,
    },
    nodes::NodeRecord,
    validation::{
        default_artifact_overwrite, default_backlink_limit, validate_artifact_id_field,
        validate_optional_text_field, validate_required_text_field, validate_workflow_id_field,
        validate_workflow_input_id_field, validate_workflow_step_id_field,
        validate_workflow_step_references,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    pub workflow_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
}

impl WorkflowMetadata {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_id_field(&self.workflow_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSummary {
    #[serde(flatten)]
    pub metadata: WorkflowMetadata,
    pub step_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowInputKind {
    NoteTarget,
    FocusTarget,
}

impl WorkflowInputKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NoteTarget => "note-target",
            Self::FocusTarget => "focus-target",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowInputSpec {
    pub input_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub kind: WorkflowInputKind,
}

impl WorkflowInputSpec {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_input_id_field(&self.input_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowInputAssignment {
    pub input_id: String,
    #[serde(flatten)]
    pub target: WorkflowResolveTarget,
}

impl WorkflowInputAssignment {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_input_id_field(&self.input_id).or_else(|| match &self.target {
            WorkflowResolveTarget::Input { .. } => Some(
                "workflow input assignments cannot reference another workflow input".to_owned(),
            ),
            WorkflowResolveTarget::Id { id } => validate_required_text_field(id, "id"),
            WorkflowResolveTarget::Title { title } => validate_required_text_field(title, "title"),
            WorkflowResolveTarget::Reference { reference } => {
                validate_required_text_field(reference, "reference")
            }
            WorkflowResolveTarget::NodeKey { node_key } => {
                validate_required_text_field(node_key, "node_key")
            }
        })
    }
}

pub const BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID: &str = "workflow/builtin/context-sweep";
pub const BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID: &str = "workflow/builtin/unresolved-sweep";
pub const BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID: &str = "workflow/builtin/periodic-review";
pub const BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID: &str =
    "workflow/builtin/weak-integration-review";
pub const BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID: &str =
    "workflow/builtin/comparison-tension-review";
pub const BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID: &str = "routine/builtin/context-sweep-review";
pub const BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID: &str =
    "routine/builtin/duplicate-title-review";
pub const WORKFLOW_SPEC_COMPATIBILITY_VERSION: u32 = 1;

#[must_use]
pub const fn default_workflow_spec_compatibility_version() -> u32 {
    WORKFLOW_SPEC_COMPATIBILITY_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSpecCompatibility {
    #[serde(default = "default_workflow_spec_compatibility_version")]
    pub version: u32,
}

impl Default for WorkflowSpecCompatibility {
    fn default() -> Self {
        Self {
            version: WORKFLOW_SPEC_COMPATIBILITY_VERSION,
        }
    }
}

impl WorkflowSpecCompatibility {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        if self.version == 0 {
            return Some(
                "workflow spec compatibility version must be greater than zero".to_owned(),
            );
        }
        (self.version > WORKFLOW_SPEC_COMPATIBILITY_VERSION).then(|| {
            format!(
                "unsupported workflow spec compatibility version {}; supported version is {}",
                self.version, WORKFLOW_SPEC_COMPATIBILITY_VERSION
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSpecCompatibilityEnvelope {
    #[serde(default)]
    pub workflow_id: Option<String>,
    #[serde(default)]
    pub compatibility: WorkflowSpecCompatibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowStepKind {
    Resolve,
    Explore,
    Compare,
    ArtifactRun,
    ArtifactSave,
}

impl WorkflowStepKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Resolve => "resolve",
            Self::Explore => "explore",
            Self::Compare => "compare",
            Self::ArtifactRun => "artifact-run",
            Self::ArtifactSave => "artifact-save",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowResolveTarget {
    Id { id: String },
    Title { title: String },
    Reference { reference: String },
    NodeKey { node_key: String },
    Input { input_id: String },
}

impl WorkflowResolveTarget {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::Id { id } => validate_required_text_field(id, "id"),
            Self::Title { title } => validate_required_text_field(title, "title"),
            Self::Reference { reference } => validate_required_text_field(reference, "reference"),
            Self::NodeKey { node_key } => validate_required_text_field(node_key, "node_key"),
            Self::Input { input_id } => validate_workflow_input_id_field(input_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepRef {
    pub step_id: String,
}

impl WorkflowStepRef {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_step_id_field(&self.step_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowExploreFocus {
    NodeKey { node_key: String },
    Input { input_id: String },
    ResolvedStep { step_id: String },
}

impl WorkflowExploreFocus {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::NodeKey { node_key } => validate_required_text_field(node_key, "node_key"),
            Self::Input { input_id } => validate_workflow_input_id_field(input_id),
            Self::ResolvedStep { step_id } => validate_workflow_step_id_field(step_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowArtifactSaveSource {
    ExploreStep { step_id: String },
    CompareStep { step_id: String },
}

impl WorkflowArtifactSaveSource {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::ExploreStep { step_id } | Self::CompareStep { step_id } => {
                validate_workflow_step_id_field(step_id)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowStepPayload {
    Resolve {
        target: WorkflowResolveTarget,
    },
    Explore {
        focus: WorkflowExploreFocus,
        lens: ExplorationLens,
        #[serde(default = "default_backlink_limit")]
        limit: usize,
        #[serde(default)]
        unique: bool,
    },
    Compare {
        left: WorkflowStepRef,
        right: WorkflowStepRef,
        #[serde(default)]
        group: NoteComparisonGroup,
        #[serde(default = "default_backlink_limit")]
        limit: usize,
    },
    ArtifactRun {
        artifact_id: String,
    },
    ArtifactSave {
        source: WorkflowArtifactSaveSource,
        #[serde(flatten)]
        metadata: ExplorationArtifactMetadata,
        #[serde(default = "default_artifact_overwrite")]
        overwrite: bool,
    },
}

impl WorkflowStepPayload {
    #[must_use]
    pub fn kind(&self) -> WorkflowStepKind {
        match self {
            Self::Resolve { .. } => WorkflowStepKind::Resolve,
            Self::Explore { .. } => WorkflowStepKind::Explore,
            Self::Compare { .. } => WorkflowStepKind::Compare,
            Self::ArtifactRun { .. } => WorkflowStepKind::ArtifactRun,
            Self::ArtifactSave { .. } => WorkflowStepKind::ArtifactSave,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::Resolve { target } => target.validation_error(),
            Self::Explore {
                focus,
                lens,
                limit,
                unique,
            } => focus.validation_error().or_else(|| {
                ExploreParams {
                    node_key: "__workflow_focus__".to_owned(),
                    lens: *lens,
                    limit: *limit,
                    unique: *unique,
                }
                .validation_error()
            }),
            Self::Compare {
                left,
                right,
                group: _,
                limit: _,
            } => left
                .validation_error()
                .or_else(|| right.validation_error())
                .or_else(|| {
                    (left.step_id == right.step_id).then(|| {
                        "compare left and right must reference distinct resolve steps".to_owned()
                    })
                }),
            Self::ArtifactRun { artifact_id } => validate_artifact_id_field(artifact_id),
            Self::ArtifactSave {
                source,
                metadata,
                overwrite: _,
            } => source
                .validation_error()
                .or_else(|| metadata.validation_error()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepSpec {
    pub step_id: String,
    #[serde(flatten)]
    pub payload: WorkflowStepPayload,
}

impl WorkflowStepSpec {
    #[must_use]
    pub fn kind(&self) -> WorkflowStepKind {
        self.payload.kind()
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_step_id_field(&self.step_id).or_else(|| self.payload.validation_error())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSpec {
    #[serde(flatten)]
    pub metadata: WorkflowMetadata,
    #[serde(default)]
    pub compatibility: WorkflowSpecCompatibility,
    #[serde(default)]
    pub inputs: Vec<WorkflowInputSpec>,
    pub steps: Vec<WorkflowStepSpec>,
}

impl WorkflowSpec {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.compatibility
            .validation_error()
            .or_else(|| self.metadata.validation_error())
            .or_else(|| {
                let mut seen_inputs: Vec<(&str, WorkflowInputKind)> =
                    Vec::with_capacity(self.inputs.len());
                for (index, input) in self.inputs.iter().enumerate() {
                    if let Some(error) = input.validation_error() {
                        return Some(format!("workflow input {index} is invalid: {error}"));
                    }
                    if seen_inputs
                        .iter()
                        .any(|(input_id, _)| *input_id == input.input_id)
                    {
                        return Some(format!(
                            "workflow input {index} reuses duplicate input_id {}",
                            input.input_id
                        ));
                    }
                    seen_inputs.push((input.input_id.as_str(), input.kind));
                }

                if self.steps.is_empty() {
                    return Some("workflows must contain at least one step".to_owned());
                }

                let mut seen: Vec<(&str, WorkflowStepKind)> = Vec::with_capacity(self.steps.len());
                for (index, step) in self.steps.iter().enumerate() {
                    if let Some(error) = step.validation_error() {
                        return Some(format!("workflow step {index} is invalid: {error}"));
                    }
                    if seen.iter().any(|(step_id, _)| *step_id == step.step_id) {
                        return Some(format!(
                            "workflow step {index} reuses duplicate step_id {}",
                            step.step_id
                        ));
                    }
                    if let Some(error) =
                        validate_workflow_step_references(&step.payload, &seen, &seen_inputs)
                    {
                        return Some(format!("workflow step {index} is invalid: {error}"));
                    }
                    seen.push((step.step_id.as_str(), step.kind()));
                }
                None
            })
    }

    #[must_use]
    pub fn has_unsupported_compatibility_version(&self) -> bool {
        self.compatibility.version > WORKFLOW_SPEC_COMPATIBILITY_VERSION
    }

    #[must_use]
    pub fn input_assignments_validation_error(
        &self,
        inputs: &[WorkflowInputAssignment],
    ) -> Option<String> {
        let mut seen_assignments: Vec<&str> = Vec::with_capacity(inputs.len());
        for (index, input) in inputs.iter().enumerate() {
            if let Some(error) = input.validation_error() {
                return Some(format!(
                    "workflow input assignment {index} is invalid: {error}"
                ));
            }
            if seen_assignments
                .iter()
                .any(|input_id| *input_id == input.input_id)
            {
                return Some(format!(
                    "workflow input assignment {index} reuses duplicate input_id {}",
                    input.input_id
                ));
            }
            if !self
                .inputs
                .iter()
                .any(|declared| declared.input_id == input.input_id)
            {
                return Some(format!(
                    "workflow input assignment {index} references unknown input_id {}",
                    input.input_id
                ));
            }
            seen_assignments.push(input.input_id.as_str());
        }

        self.inputs
            .iter()
            .find(|input| !seen_assignments.contains(&input.input_id.as_str()))
            .map(|input| format!("workflow input {} must be assigned", input.input_id))
    }
}

impl From<&WorkflowSpec> for WorkflowSummary {
    fn from(workflow: &WorkflowSpec) -> Self {
        Self {
            metadata: workflow.metadata.clone(),
            step_count: workflow.steps.len(),
        }
    }
}

#[must_use]
pub fn built_in_workflows() -> Vec<WorkflowSpec> {
    static WORKFLOWS: LazyLock<Vec<WorkflowSpec>> = LazyLock::new(|| {
        vec![
            built_in_context_sweep_workflow(),
            built_in_unresolved_sweep_workflow(),
            built_in_periodic_review_workflow(),
            built_in_weak_integration_review_workflow(),
            built_in_comparison_tension_workflow(),
        ]
    });
    WORKFLOWS.clone()
}

#[must_use]
pub fn built_in_workflow(workflow_id: &str) -> Option<WorkflowSpec> {
    built_in_workflows()
        .into_iter()
        .find(|workflow| workflow.metadata.workflow_id == workflow_id)
}

#[must_use]
pub fn built_in_workflow_summaries() -> Vec<WorkflowSummary> {
    built_in_workflows()
        .into_iter()
        .map(|workflow| WorkflowSummary::from(&workflow))
        .collect()
}

fn built_in_context_sweep_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned(),
            title: "Context Sweep".to_owned(),
            summary: Some(
                "Resolve a focus note and gather structural, reference, and dormant context."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus target".to_owned(),
            summary: Some("Note or anchor target to inspect".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-structure".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Structure,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-refs".to_owned(),
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
                step_id: "explore-dormant".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Dormant,
                    limit: 25,
                    unique: false,
                },
            },
        ],
    }
}

fn built_in_unresolved_sweep_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID.to_owned(),
            title: "Unresolved Sweep".to_owned(),
            summary: Some(
                "Resolve a focus note and sweep unresolved, task, and time context around it."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus target".to_owned(),
            summary: Some("Note or anchor target to audit for unresolved pressure".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-unresolved".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Unresolved,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-tasks".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Tasks,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-time".to_owned(),
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

fn built_in_periodic_review_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID.to_owned(),
            title: "Periodic Review".to_owned(),
            summary: Some(
                "Run a recurring review around a focus target across unresolved work, planning, references, and dormant context."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Review focus".to_owned(),
            summary: Some("Note or anchor target anchoring the recurring review".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "review-unresolved".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Unresolved,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-time".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Time,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-tasks".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Tasks,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-refs".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-dormant".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Dormant,
                    limit: 50,
                    unique: false,
                },
            },
        ],
    }
}

fn built_in_weak_integration_review_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID.to_owned(),
            title: "Weak Integration Review".to_owned(),
            summary: Some(
                "Review weakly integrated, dormant, and bridgeable material around a focus note."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Integration focus".to_owned(),
            summary: Some(
                "Note or anchor target whose surrounding integration should be reviewed".to_owned(),
            ),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "review-weak-integration".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Unresolved,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-dormant".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Dormant,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-bridges".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Bridges,
                    limit: 50,
                    unique: false,
                },
            },
        ],
    }
}

fn built_in_comparison_tension_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID.to_owned(),
            title: "Comparison Tension Review".to_owned(),
            summary: Some(
                "Resolve two notes and review both tension and overlap together.".to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![
            WorkflowInputSpec {
                input_id: "left".to_owned(),
                title: "Left note".to_owned(),
                summary: Some("First exact note target".to_owned()),
                kind: WorkflowInputKind::NoteTarget,
            },
            WorkflowInputSpec {
                input_id: "right".to_owned(),
                title: "Right note".to_owned(),
                summary: Some("Second exact note target".to_owned()),
                kind: WorkflowInputKind::NoteTarget,
            },
        ],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-left".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "left".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "resolve-right".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "right".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "compare-tension".to_owned(),
                payload: WorkflowStepPayload::Compare {
                    left: WorkflowStepRef {
                        step_id: "resolve-left".to_owned(),
                    },
                    right: WorkflowStepRef {
                        step_id: "resolve-right".to_owned(),
                    },
                    group: NoteComparisonGroup::Tension,
                    limit: 25,
                },
            },
            WorkflowStepSpec {
                step_id: "compare-overlap".to_owned(),
                payload: WorkflowStepPayload::Compare {
                    left: WorkflowStepRef {
                        step_id: "resolve-left".to_owned(),
                    },
                    right: WorkflowStepRef {
                        step_id: "resolve-right".to_owned(),
                    },
                    group: NoteComparisonGroup::Overlap,
                    limit: 25,
                },
            },
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowStepReportPayload {
    Resolve {
        node: Box<NodeRecord>,
    },
    Explore {
        focus_node_key: String,
        result: Box<ExploreResult>,
    },
    Compare {
        left_node: Box<NodeRecord>,
        right_node: Box<NodeRecord>,
        result: Box<NoteComparisonResult>,
    },
    ArtifactRun {
        artifact: Box<ExecutedExplorationArtifact>,
    },
    ArtifactSave {
        artifact: Box<ExplorationArtifactSummary>,
    },
}

impl WorkflowStepReportPayload {
    #[must_use]
    pub fn kind(&self) -> WorkflowStepKind {
        match self {
            Self::Resolve { .. } => WorkflowStepKind::Resolve,
            Self::Explore { .. } => WorkflowStepKind::Explore,
            Self::Compare { .. } => WorkflowStepKind::Compare,
            Self::ArtifactRun { .. } => WorkflowStepKind::ArtifactRun,
            Self::ArtifactSave { .. } => WorkflowStepKind::ArtifactSave,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepReport {
    pub step_id: String,
    #[serde(flatten)]
    pub payload: WorkflowStepReportPayload,
}

impl WorkflowStepReport {
    #[must_use]
    pub fn kind(&self) -> WorkflowStepKind {
        self.payload.kind()
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_step_id_field(&self.step_id).or_else(|| match &self.payload {
            WorkflowStepReportPayload::Resolve { node } => {
                validate_required_text_field(&node.node_key, "node_key")
            }
            WorkflowStepReportPayload::Explore {
                focus_node_key,
                result: _,
            } => validate_required_text_field(focus_node_key, "focus_node_key"),
            WorkflowStepReportPayload::Compare {
                left_node,
                right_node,
                result: _,
            } => validate_required_text_field(&left_node.node_key, "left_node_key")
                .or_else(|| validate_required_text_field(&right_node.node_key, "right_node_key")),
            WorkflowStepReportPayload::ArtifactRun { artifact } => {
                artifact.metadata.validation_error()
            }
            WorkflowStepReportPayload::ArtifactSave { artifact } => {
                artifact.metadata.validation_error()
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionResult {
    pub workflow: WorkflowSummary,
    pub steps: Vec<WorkflowStepReport>,
}

impl WorkflowExecutionResult {
    #[must_use]
    pub fn report_lines(&self) -> Vec<WorkflowReportLine> {
        let mut lines = Vec::with_capacity(self.steps.len() + 1);
        lines.push(WorkflowReportLine::Workflow {
            workflow: self.workflow.clone(),
        });
        lines.extend(
            self.steps
                .iter()
                .cloned()
                .map(|step| WorkflowReportLine::Step { step }),
        );
        lines
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowReportLine {
    Workflow { workflow: WorkflowSummary },
    Step { step: WorkflowStepReport },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowIdParams {
    pub workflow_id: String,
}

impl WorkflowIdParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_id_field(&self.workflow_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListWorkflowsParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCatalogIssue {
    pub path: String,
    pub kind: WorkflowCatalogIssueKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_id: Option<String>,
    #[serde(default)]
    pub workflow_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routine_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowCatalogIssueKind {
    Directory,
    Io,
    MalformedJson,
    UnsupportedVersion,
    InvalidSpec,
    InvalidPack,
    InvalidReviewRoutine,
    InvalidReportProfile,
    DuplicateWorkflowId,
    DuplicateReviewRoutineId,
    DuplicateReportProfileId,
}

impl WorkflowCatalogIssueKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Io => "io",
            Self::MalformedJson => "malformed-json",
            Self::UnsupportedVersion => "unsupported-version",
            Self::InvalidSpec => "invalid-spec",
            Self::InvalidPack => "invalid-pack",
            Self::InvalidReviewRoutine => "invalid-review-routine",
            Self::InvalidReportProfile => "invalid-report-profile",
            Self::DuplicateWorkflowId => "duplicate-workflow-id",
            Self::DuplicateReviewRoutineId => "duplicate-review-routine-id",
            Self::DuplicateReportProfileId => "duplicate-report-profile-id",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListWorkflowsResult {
    pub workflows: Vec<WorkflowSummary>,
    #[serde(default)]
    pub issues: Vec<WorkflowCatalogIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub workflow: WorkflowSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunWorkflowParams {
    pub workflow_id: String,
    #[serde(default)]
    pub inputs: Vec<WorkflowInputAssignment>,
}

impl RunWorkflowParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_id_field(&self.workflow_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunWorkflowResult {
    pub result: WorkflowExecutionResult,
}
