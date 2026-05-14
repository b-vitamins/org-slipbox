use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    review::{ReviewFindingStatus, ReviewRunDiffBucket},
    validation::{
        validate_optional_text_field, validate_report_profile_diff_buckets,
        validate_report_profile_id_field, validate_report_profile_jsonl_line_kinds,
        validate_report_profile_status_filters, validate_report_profile_subjects,
        validate_required_text_field,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportProfileMetadata {
    pub profile_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
}

impl ReportProfileMetadata {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_report_profile_id_field(&self.profile_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportProfileSubject {
    Review,
    Routine,
    Audit,
    Workflow,
    Diff,
}

impl ReportProfileSubject {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Routine => "routine",
            Self::Audit => "audit",
            Self::Workflow => "workflow",
            Self::Diff => "diff",
        }
    }

    #[must_use]
    pub const fn supports_line_kind(self, line_kind: &ReportJsonlLineKind) -> bool {
        match self {
            Self::Workflow => {
                matches!(
                    line_kind,
                    ReportJsonlLineKind::Workflow | ReportJsonlLineKind::Step
                )
            }
            Self::Audit => {
                matches!(
                    line_kind,
                    ReportJsonlLineKind::Audit | ReportJsonlLineKind::Entry
                )
            }
            Self::Review => matches!(
                line_kind,
                ReportJsonlLineKind::Review | ReportJsonlLineKind::Finding
            ),
            Self::Routine => matches!(
                line_kind,
                ReportJsonlLineKind::Routine
                    | ReportJsonlLineKind::Step
                    | ReportJsonlLineKind::Review
                    | ReportJsonlLineKind::Finding
            ),
            Self::Diff => matches!(
                line_kind,
                ReportJsonlLineKind::Diff
                    | ReportJsonlLineKind::Added
                    | ReportJsonlLineKind::Removed
                    | ReportJsonlLineKind::Unchanged
                    | ReportJsonlLineKind::ContentChanged
                    | ReportJsonlLineKind::StatusChanged
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ReportProfileMode {
    Summary,
    #[default]
    Detail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportJsonlLineKind {
    Workflow,
    Step,
    Audit,
    Entry,
    Review,
    Finding,
    Routine,
    Diff,
    Added,
    Removed,
    Unchanged,
    ContentChanged,
    StatusChanged,
    Unsupported(String),
}

impl ReportJsonlLineKind {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Workflow => "workflow",
            Self::Step => "step",
            Self::Audit => "audit",
            Self::Entry => "entry",
            Self::Review => "review",
            Self::Finding => "finding",
            Self::Routine => "routine",
            Self::Diff => "diff",
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Unchanged => "unchanged",
            Self::ContentChanged => "content-changed",
            Self::StatusChanged => "status-changed",
            Self::Unsupported(value) => value.as_str(),
        }
    }

    #[must_use]
    pub fn from_label(value: &str) -> Self {
        match value {
            "workflow" => Self::Workflow,
            "step" => Self::Step,
            "audit" => Self::Audit,
            "entry" => Self::Entry,
            "review" => Self::Review,
            "finding" => Self::Finding,
            "routine" => Self::Routine,
            "diff" => Self::Diff,
            "added" => Self::Added,
            "removed" => Self::Removed,
            "unchanged" => Self::Unchanged,
            "content-changed" => Self::ContentChanged,
            "status-changed" => Self::StatusChanged,
            other => Self::Unsupported(other.to_owned()),
        }
    }

    #[must_use]
    pub const fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported(_))
    }

    #[must_use]
    pub const fn is_detail_line(&self) -> bool {
        matches!(
            self,
            Self::Step
                | Self::Entry
                | Self::Finding
                | Self::Added
                | Self::Removed
                | Self::Unchanged
                | Self::ContentChanged
                | Self::StatusChanged
        )
    }
}

impl Serialize for ReportJsonlLineKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.label())
    }
}

impl<'de> Deserialize<'de> for ReportJsonlLineKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from_label(&value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportProfileSpec {
    #[serde(flatten)]
    pub metadata: ReportProfileMetadata,
    #[serde(default)]
    pub subjects: Vec<ReportProfileSubject>,
    #[serde(default)]
    pub mode: ReportProfileMode,
    #[serde(default)]
    pub status_filters: Option<Vec<ReviewFindingStatus>>,
    #[serde(default)]
    pub diff_buckets: Option<Vec<ReviewRunDiffBucket>>,
    #[serde(default)]
    pub jsonl_line_kinds: Option<Vec<ReportJsonlLineKind>>,
}

impl ReportProfileSpec {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.metadata
            .validation_error()
            .or_else(|| validate_report_profile_subjects(&self.subjects))
            .or_else(|| validate_report_profile_status_filters(self))
            .or_else(|| validate_report_profile_diff_buckets(self))
            .or_else(|| validate_report_profile_jsonl_line_kinds(self))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReportProfileCatalog {
    #[serde(default)]
    pub profiles: Vec<ReportProfileSpec>,
}

impl ReportProfileCatalog {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        let mut seen: Vec<&str> = Vec::with_capacity(self.profiles.len());
        for (index, profile) in self.profiles.iter().enumerate() {
            if let Some(error) = profile.validation_error() {
                return Some(format!("report profile {index} is invalid: {error}"));
            }
            if seen.contains(&profile.metadata.profile_id.as_str()) {
                return Some(format!(
                    "report profile {index} reuses duplicate profile_id {}",
                    profile.metadata.profile_id
                ));
            }
            seen.push(profile.metadata.profile_id.as_str());
        }
        None
    }
}
