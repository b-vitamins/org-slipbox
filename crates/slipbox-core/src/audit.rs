use serde::{Deserialize, Serialize};

use crate::{
    nodes::{AnchorRecord, NodeRecord},
    validation::{default_audit_limit, validate_required_text_field},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CorpusAuditKind {
    DanglingLinks,
    DuplicateTitles,
    OrphanNotes,
    WeaklyIntegratedNotes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusAuditParams {
    pub audit: CorpusAuditKind,
    #[serde(default = "default_audit_limit")]
    pub limit: usize,
}

impl CorpusAuditParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DanglingLinkAuditRecord {
    pub source: AnchorRecord,
    pub missing_explicit_id: String,
    pub line: u32,
    pub column: u32,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateTitleAuditRecord {
    pub title: String,
    pub notes: Vec<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteConnectivityAuditRecord {
    pub note: NodeRecord,
    pub reference_count: usize,
    pub backlink_count: usize,
    pub forward_link_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CorpusAuditEntry {
    DanglingLink {
        record: Box<DanglingLinkAuditRecord>,
    },
    DuplicateTitle {
        record: Box<DuplicateTitleAuditRecord>,
    },
    OrphanNote {
        record: Box<NoteConnectivityAuditRecord>,
    },
    WeaklyIntegratedNote {
        record: Box<NoteConnectivityAuditRecord>,
    },
}

impl CorpusAuditEntry {
    #[must_use]
    pub const fn kind(&self) -> CorpusAuditKind {
        match self {
            Self::DanglingLink { .. } => CorpusAuditKind::DanglingLinks,
            Self::DuplicateTitle { .. } => CorpusAuditKind::DuplicateTitles,
            Self::OrphanNote { .. } => CorpusAuditKind::OrphanNotes,
            Self::WeaklyIntegratedNote { .. } => CorpusAuditKind::WeaklyIntegratedNotes,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::DanglingLink { record } => {
                validate_required_text_field(&record.source.node_key, "source.node_key")
                    .or_else(|| {
                        validate_required_text_field(
                            &record.missing_explicit_id,
                            "missing_explicit_id",
                        )
                    })
                    .or_else(|| validate_required_text_field(&record.preview, "preview"))
            }
            Self::DuplicateTitle { record } => validate_required_text_field(&record.title, "title")
                .or_else(|| {
                    (record.notes.len() < 2).then(|| {
                        "duplicate-title findings must include at least two notes".to_owned()
                    })
                }),
            Self::OrphanNote { record } | Self::WeaklyIntegratedNote { record } => {
                validate_required_text_field(&record.note.node_key, "note.node_key")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusAuditResult {
    pub audit: CorpusAuditKind,
    pub entries: Vec<CorpusAuditEntry>,
}

impl CorpusAuditResult {
    #[must_use]
    pub fn report_lines(&self) -> Vec<CorpusAuditReportLine> {
        let mut lines = Vec::with_capacity(self.entries.len() + 1);
        lines.push(CorpusAuditReportLine::Audit { audit: self.audit });
        lines.extend(
            self.entries
                .iter()
                .cloned()
                .map(|entry| CorpusAuditReportLine::Entry { entry }),
        );
        lines
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CorpusAuditReportLine {
    Audit { audit: CorpusAuditKind },
    Entry { entry: CorpusAuditEntry },
}
