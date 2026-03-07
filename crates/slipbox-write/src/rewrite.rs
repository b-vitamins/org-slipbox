use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use slipbox_core::{NodeKind, NodeRecord};

use crate::document::{OrgDocument, shift_subtree_levels};
use crate::path::normalize_relative_org_path;
use crate::{CaptureOutcome, RewriteOutcome};

pub fn refile_subtree(
    root: &Path,
    source: &NodeRecord,
    target: &NodeRecord,
) -> Result<RewriteOutcome> {
    if source.node_key == target.node_key {
        bail!("target is the same as current node");
    }

    let source_path = root.join(&source.file_path);
    let target_path = root.join(&target.file_path);
    if source.kind == NodeKind::File && source_path == target_path {
        bail!("target is inside the current subtree");
    }

    let source_source = fs::read_to_string(&source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    let target_source = if source_path == target_path {
        None
    } else {
        Some(
            fs::read_to_string(&target_path)
                .with_context(|| format!("failed to read {}", target_path.display()))?,
        )
    };

    let mut source_document = OrgDocument::from_source(&source_source);
    let (mut subtree_lines, explicit_id) = source_document.subtree_lines(source)?;
    let target_root_level = match target.kind {
        NodeKind::File => 1,
        NodeKind::Heading => target.level as usize + 1,
    };
    shift_subtree_levels(&mut subtree_lines, target_root_level)?;

    if source_path == target_path {
        if source.kind != NodeKind::Heading {
            bail!("target is inside the current subtree");
        }

        let (source_start, source_end) = source_document.subtree_range(source.line as usize)?;
        if target.kind == NodeKind::Heading {
            let target_index = target.line as usize - 1;
            if (source_start..source_end).contains(&target_index) {
                bail!("target is inside the current subtree");
            }
        }

        let insert_index = source_document.insertion_index(target)?;
        source_document.remove_range(source_start, source_end);
        let adjusted_insert = if insert_index > source_start {
            insert_index - (source_end - source_start)
        } else {
            insert_index
        };
        source_document.insert_subtree(adjusted_insert, target.kind, subtree_lines);

        fs::write(&source_path, source_document.render())
            .with_context(|| format!("failed to write {}", source_path.display()))?;

        return Ok(RewriteOutcome {
            changed_paths: vec![source_path],
            removed_paths: Vec::new(),
            explicit_id,
        });
    }

    let mut target_document = OrgDocument::from_source(target_source.as_deref().unwrap_or(""));
    target_document.insert_subtree(
        target_document.insertion_index(target)?,
        target.kind,
        subtree_lines,
    );

    let mut changed_paths = vec![target_path.clone()];
    let mut removed_paths = Vec::new();

    if source.kind == NodeKind::File {
        fs::remove_file(&source_path)
            .with_context(|| format!("failed to remove {}", source_path.display()))?;
        removed_paths.push(source_path);
    } else {
        let (source_start, source_end) = source_document.subtree_range(source.line as usize)?;
        source_document.remove_range(source_start, source_end);
        if source_document.has_meaningful_content() {
            fs::write(&source_path, source_document.render())
                .with_context(|| format!("failed to write {}", source_path.display()))?;
            changed_paths.push(source_path);
        } else {
            fs::remove_file(&source_path)
                .with_context(|| format!("failed to remove {}", source_path.display()))?;
            removed_paths.push(source_path);
        }
    }

    fs::write(&target_path, target_document.render())
        .with_context(|| format!("failed to write {}", target_path.display()))?;

    Ok(RewriteOutcome {
        changed_paths,
        removed_paths,
        explicit_id,
    })
}

pub fn extract_subtree(
    root: &Path,
    source: &NodeRecord,
    file_path: &str,
) -> Result<RewriteOutcome> {
    if source.kind != NodeKind::Heading {
        bail!("only heading nodes can be extracted");
    }

    let relative_path = normalize_relative_org_path(file_path)?;
    let source_path = root.join(&source.file_path);
    let target_path = root.join(&relative_path);
    if source_path == target_path {
        bail!("target file must differ from the source file");
    }
    if target_path.exists() {
        bail!("{} exists. Aborting", target_path.display());
    }

    let source_source = fs::read_to_string(&source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    let mut source_document = OrgDocument::from_source(&source_source);
    let (subtree_lines, explicit_id) = source_document.subtree_lines(source)?;
    let target_document = OrgDocument::from_extracted_subtree(&subtree_lines, &explicit_id)?;
    let (source_start, source_end) = source_document.subtree_range(source.line as usize)?;
    source_document.remove_range(source_start, source_end);

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    fs::write(&source_path, source_document.render())
        .with_context(|| format!("failed to write {}", source_path.display()))?;
    fs::write(&target_path, target_document.render())
        .with_context(|| format!("failed to write {}", target_path.display()))?;

    Ok(RewriteOutcome {
        changed_paths: vec![source_path, target_path],
        removed_paths: Vec::new(),
        explicit_id,
    })
}

pub fn demote_entire_file(root: &Path, file_path: &str) -> Result<CaptureOutcome> {
    let relative_path = normalize_relative_org_path(file_path)?;
    let absolute_path = root.join(&relative_path);
    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let mut document = OrgDocument::from_source(&source);
    document.demote_entire_file(&relative_path);
    fs::write(&absolute_path, document.render())
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;
    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("heading:{}:1", relative_path.replace('\\', "/")),
    })
}

pub fn promote_entire_file(root: &Path, file_path: &str) -> Result<CaptureOutcome> {
    let relative_path = normalize_relative_org_path(file_path)?;
    let absolute_path = root.join(&relative_path);
    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let mut document = OrgDocument::from_source(&source);
    document.promote_entire_file()?;
    fs::write(&absolute_path, document.render())
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;
    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("file:{}", relative_path.replace('\\', "/")),
    })
}
