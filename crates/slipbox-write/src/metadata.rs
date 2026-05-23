use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use slipbox_core::{AnchorRecord, NodeKind, NodeRecord};
use uuid::Uuid;

use crate::MetadataUpdate;
use crate::document::{OrgDocument, keyword_value, property_value};
use crate::path::checked_absolute_org_path;
use crate::transaction::FileRewriteTransaction;

pub fn ensure_node_id(root: &Path, node: &AnchorRecord) -> Result<PathBuf> {
    let (_, absolute_path) = checked_absolute_org_path(root, &node.file_path)?;
    if node.explicit_id.is_some() {
        return Ok(absolute_path);
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
    Ok(absolute_path)
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
