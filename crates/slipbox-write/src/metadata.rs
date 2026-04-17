use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use slipbox_core::{AnchorRecord, NodeKind, NodeRecord};
use uuid::Uuid;

use crate::MetadataUpdate;
use crate::document::{OrgDocument, keyword_value, property_value};

pub fn ensure_node_id(root: &Path, node: &AnchorRecord) -> Result<PathBuf> {
    if node.explicit_id.is_some() {
        return Ok(root.join(&node.file_path));
    }

    let absolute_path = root.join(&node.file_path);
    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let explicit_id = Uuid::new_v4().to_string();
    let updated = match node.kind {
        NodeKind::File => insert_file_id(&source, &explicit_id),
        NodeKind::Heading => insert_heading_id(&source, node.line as usize, &explicit_id)?,
    };
    fs::write(&absolute_path, updated)
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;
    Ok(absolute_path)
}

pub fn update_node_metadata(
    root: &Path,
    node: &NodeRecord,
    update: &MetadataUpdate,
) -> Result<PathBuf> {
    let absolute_path = root.join(&node.file_path);
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

    fs::write(&absolute_path, document.render())
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;
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
