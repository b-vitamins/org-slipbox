use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use slipbox_core::{AnchorRecord, GlossaryStatus, NodeKind, NodeRecord, SrState};
use uuid::Uuid;

use crate::MetadataUpdate;
use crate::document::{OrgDocument, keyword_value, property_value};
use crate::path::checked_absolute_org_path;
use crate::transaction::FileRewriteTransaction;

pub struct EnsureNodeIdOutcome {
    pub absolute_path: PathBuf,
    pub explicit_id: String,
}

pub fn ensure_node_id(root: &Path, node: &AnchorRecord) -> Result<PathBuf> {
    Ok(ensure_node_id_with_value(root, node)?.absolute_path)
}

pub fn ensure_node_id_with_value(root: &Path, node: &AnchorRecord) -> Result<EnsureNodeIdOutcome> {
    let (_, absolute_path) = checked_absolute_org_path(root, &node.file_path)?;
    if let Some(explicit_id) = &node.explicit_id {
        return Ok(EnsureNodeIdOutcome {
            absolute_path,
            explicit_id: explicit_id.clone(),
        });
    }

    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let explicit_id = Uuid::new_v4().to_string();
    let updated = match node.kind {
        NodeKind::File => insert_file_id(&source, &explicit_id),
        NodeKind::Heading => insert_heading_id(&source, node.line as usize, &explicit_id)?,
    };
    let mut transaction = FileRewriteTransaction::new();
    transaction.write(&absolute_path, updated);
    transaction.commit()?;
    Ok(EnsureNodeIdOutcome {
        absolute_path,
        explicit_id,
    })
}

pub fn update_node_metadata(
    root: &Path,
    node: &NodeRecord,
    update: &MetadataUpdate,
) -> Result<PathBuf> {
    let (_, absolute_path) = checked_absolute_org_path(root, &node.file_path)?;
    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let mut document = OrgDocument::from_source(&source);

    match node.kind {
        NodeKind::File => {
            if let Some(aliases) = &update.aliases {
                document.set_file_property("ROAM_ALIASES", property_value(aliases));
            }
            if let Some(refs) = &update.refs {
                document.set_file_property("ROAM_REFS", property_value(refs));
            }
            if let Some(tags) = &update.tags {
                document.set_file_keyword("filetags", keyword_value(tags));
            }
        }
        NodeKind::Heading => {
            if let Some(aliases) = &update.aliases {
                document.set_heading_property(
                    node.line as usize,
                    "ROAM_ALIASES",
                    property_value(aliases),
                )?;
            }
            if let Some(refs) = &update.refs {
                document.set_heading_property(
                    node.line as usize,
                    "ROAM_REFS",
                    property_value(refs),
                )?;
            }
            if let Some(tags) = &update.tags {
                document.set_heading_tags(node.line as usize, tags)?;
            }
        }
    }

    let mut transaction = FileRewriteTransaction::new();
    transaction.write(&absolute_path, document.render());
    transaction.commit()?;
    Ok(absolute_path)
}

/// Mark an existing file note as a glossary term by adding `#+glossary: t`.
///
/// Idempotent: a note that already carries the marker is left untouched. Terms
/// are file nodes by contract, so a non-file node is rejected rather than
/// silently mis-targeted.
pub fn mark_glossary_term(root: &Path, node: &NodeRecord) -> Result<PathBuf> {
    require_file_node(node, "mark a glossary term")?;
    edit_file_document(root, &node.file_path, |document| {
        document.set_file_keyword("glossary", Some("t".to_owned()));
        Ok(())
    })
}

/// Record a glossary term's confirmation status in the `GLOSSARY_STATUS` drawer key.
///
/// Only `GLOSSARY_STATUS` is written; the rest of the file is preserved.
pub fn set_glossary_status(
    root: &Path,
    node: &NodeRecord,
    status: GlossaryStatus,
) -> Result<PathBuf> {
    require_file_node(node, "set glossary status")?;
    edit_file_document(root, &node.file_path, |document| {
        document.set_file_property("GLOSSARY_STATUS", Some(status.as_str().to_owned()));
        Ok(())
    })
}

/// Rewrite a term's SM-2 review drawer from computed scheduling state.
///
/// Writes exactly the `SR_DUE`/`SR_EASE`/`SR_INTERVAL`/`SR_REPS`/`SR_LAST` keys
/// and leaves every other line intact. A `None` date on the state clears the
/// corresponding key so a never-scheduled term does not carry a stale value.
pub fn set_glossary_schedule(root: &Path, node: &NodeRecord, state: &SrState) -> Result<PathBuf> {
    require_file_node(node, "set a glossary review schedule")?;
    edit_file_document(root, &node.file_path, |document| {
        document.set_file_property("SR_DUE", state.due.clone());
        document.set_file_property("SR_EASE", Some(state.ease_string()));
        document.set_file_property("SR_INTERVAL", Some(state.interval_string()));
        document.set_file_property("SR_REPS", Some(state.reps_string()));
        document.set_file_property("SR_LAST", state.last.clone());
        Ok(())
    })
}

fn require_file_node(node: &NodeRecord, action: &str) -> Result<()> {
    if node.kind == NodeKind::File {
        Ok(())
    } else {
        bail!("cannot {action}: {} is not a file node", node.node_key)
    }
}

fn edit_file_document(
    root: &Path,
    file_path: &str,
    edit: impl FnOnce(&mut OrgDocument) -> Result<()>,
) -> Result<PathBuf> {
    let (_, absolute_path) = checked_absolute_org_path(root, file_path)?;
    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let mut document = OrgDocument::from_source(&source);
    edit(&mut document)?;
    let mut transaction = FileRewriteTransaction::new();
    transaction.write(&absolute_path, document.render());
    transaction.commit()?;
    Ok(absolute_path)
}

fn insert_file_id(source: &str, explicit_id: &str) -> String {
    let mut document = OrgDocument::from_source(source);
    document.set_file_property("ID", Some(explicit_id.to_owned()));
    document.render()
}

fn insert_heading_id(source: &str, line_number: usize, explicit_id: &str) -> Result<String> {
    let mut document = OrgDocument::from_source(source);
    document.set_heading_property(line_number, "ID", Some(explicit_id.to_owned()))?;
    Ok(document.render())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slipbox_core::sm2_schedule;
    use tempfile::TempDir;

    fn term_node(file_path: &str) -> NodeRecord {
        NodeRecord {
            node_key: format!("file:{file_path}"),
            explicit_id: Some("term-id".to_owned()),
            file_path: file_path.to_owned(),
            title: "Riemann integral".to_owned(),
            outline_path: String::new(),
            aliases: Vec::new(),
            tags: Vec::new(),
            refs: Vec::new(),
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            glossary: true,
            glossary_status: None,
            sr_due: None,
            sr_ease: None,
            sr_interval: None,
            sr_reps: None,
            sr_last: None,
            level: 0,
            line: 1,
            kind: NodeKind::File,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        }
    }

    fn write_root(body: &str) -> (TempDir, NodeRecord) {
        let root = tempfile::tempdir().expect("temp root");
        std::fs::write(root.path().join("term.org"), body).expect("write fixture");
        (root, term_node("term.org"))
    }

    fn read_back(root: &TempDir) -> String {
        std::fs::read_to_string(root.path().join("term.org")).expect("read fixture")
    }

    #[test]
    fn mark_adds_the_glossary_keyword() {
        let (root, node) = write_root("#+title: Riemann integral\n\nA definite integral.\n");
        mark_glossary_term(root.path(), &node).expect("mark");
        let text = read_back(&root);
        assert!(text.contains("#+glossary: t"));
        assert!(text.contains("#+title: Riemann integral"));
        assert!(text.contains("A definite integral."));
    }

    #[test]
    fn mark_is_idempotent_when_already_present() {
        let (root, node) = write_root("#+title: Riemann integral\n#+glossary: t\n\nBody.\n");
        mark_glossary_term(root.path(), &node).expect("mark");
        let text = read_back(&root);
        assert_eq!(text.matches("#+glossary:").count(), 1);
    }

    #[test]
    fn set_status_writes_only_glossary_status() {
        let (root, node) = write_root(
            "#+title: Riemann integral\n#+glossary: t\n:PROPERTIES:\n:ID: term-id\n:END:\n\nBody.\n",
        );
        set_glossary_status(root.path(), &node, GlossaryStatus::Confirmed).expect("status");
        let text = read_back(&root);
        assert!(text.contains(":GLOSSARY_STATUS: confirmed"));
        assert!(text.contains(":ID: term-id"));
        assert!(!text.contains(":SR_"));

        // Updating status rewrites the existing key rather than duplicating it.
        set_glossary_status(root.path(), &node, GlossaryStatus::Stub).expect("status");
        let text = read_back(&root);
        assert_eq!(text.matches(":GLOSSARY_STATUS:").count(), 1);
        assert!(text.contains(":GLOSSARY_STATUS: stub"));
    }

    #[test]
    fn set_schedule_writes_all_sr_keys_and_preserves_the_rest() {
        let (root, node) = write_root(
            "#+title: Riemann integral\n#+glossary: t\n:PROPERTIES:\n:ID: term-id\n:END:\n\nA definite integral.\n",
        );
        let state = sm2_schedule(&SrState::default(), 5, "2026-07-21");
        set_glossary_schedule(root.path(), &node, &state).expect("schedule");

        let text = read_back(&root);
        assert!(text.contains(":SR_DUE: 2026-07-22"));
        assert!(text.contains(":SR_EASE: 2.60"));
        assert!(text.contains(":SR_INTERVAL: 1"));
        assert!(text.contains(":SR_REPS: 1"));
        assert!(text.contains(":SR_LAST: 2026-07-21"));
        // Existing identity and body survive untouched.
        assert!(text.contains(":ID: term-id"));
        assert!(text.contains("A definite integral."));
        assert!(text.contains("#+glossary: t"));
    }

    #[test]
    fn set_schedule_is_idempotent_in_key_count() {
        let (root, node) = write_root(
            "#+title: Riemann integral\n#+glossary: t\n:PROPERTIES:\n:ID: term-id\n:END:\n\nBody.\n",
        );
        let first = sm2_schedule(&SrState::default(), 5, "2026-07-21");
        set_glossary_schedule(root.path(), &node, &first).expect("first");
        let second = sm2_schedule(&first, 5, "2026-07-22");
        set_glossary_schedule(root.path(), &node, &second).expect("second");

        let text = read_back(&root);
        for key in [":SR_DUE:", ":SR_EASE:", ":SR_INTERVAL:", ":SR_REPS:", ":SR_LAST:"] {
            assert_eq!(
                text.matches(key).count(),
                1,
                "{key} should appear exactly once"
            );
        }
        // The second grade's values replaced the first.
        assert!(text.contains(":SR_INTERVAL: 6"));
        assert!(text.contains(":SR_REPS: 2"));
        assert!(text.contains(":SR_DUE: 2026-07-28"));
    }

    #[test]
    fn writers_reject_non_file_nodes() {
        let (root, mut node) = write_root("#+title: Term\n#+glossary: t\n");
        node.kind = NodeKind::Heading;
        assert!(mark_glossary_term(root.path(), &node).is_err());
        assert!(set_glossary_status(root.path(), &node, GlossaryStatus::Stub).is_err());
        assert!(
            set_glossary_schedule(root.path(), &node, &SrState::default()).is_err()
        );
    }
}
