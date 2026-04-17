use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use slipbox_core::{AnchorRecord, NodeKind, NodeRecord};

use crate::document::{OrgDocument, shift_subtree_levels};
use crate::path::normalize_relative_org_path;
use crate::{CaptureOutcome, RewriteOutcome};

pub struct RegionRewriteOutcome {
    pub changed_paths: Vec<std::path::PathBuf>,
    pub removed_paths: Vec<std::path::PathBuf>,
}

pub fn refile_subtree(
    root: &Path,
    source: &AnchorRecord,
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

pub fn refile_region(
    root: &Path,
    source_file_path: &str,
    start: usize,
    end: usize,
    target: &NodeRecord,
) -> Result<RegionRewriteOutcome> {
    if start == end {
        bail!("active region must not be empty");
    }

    let relative_source_path = normalize_relative_org_path(source_file_path)?;
    let source_path = root.join(&relative_source_path);
    let target_path = root.join(&target.file_path);
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

    let selection_start = byte_offset_for_char_position(&source_source, start)?;
    let selection_end = byte_offset_for_char_position(&source_source, end)?;
    if selection_start >= selection_end {
        bail!("active region must not be empty");
    }

    let region_text = &source_source[selection_start..selection_end];
    let target_root_level = match target.kind {
        NodeKind::File => 1,
        NodeKind::Heading => target.level as usize + 1,
    };
    let region_text = normalize_region_for_target(region_text, target_root_level)?;

    if source_path == target_path {
        let target_insert = insertion_byte_offset(&source_source, target)?;
        if matches!(target.kind, NodeKind::Heading) {
            let target_heading_start =
                line_index_byte_offset(&source_source, target.line.saturating_sub(1) as usize)?;
            if (selection_start..selection_end).contains(&target_heading_start) {
                bail!("target is inside the current region");
            }
        }

        let mut rewritten_source = String::new();
        rewritten_source.push_str(&source_source[..selection_start]);
        rewritten_source.push_str(&source_source[selection_end..]);
        let adjusted_insert = if target_insert > selection_start {
            target_insert - (selection_end - selection_start)
        } else {
            target_insert
        };
        let rewritten_source = insert_region_text(rewritten_source, adjusted_insert, &region_text);
        fs::write(&source_path, &rewritten_source)
            .with_context(|| format!("failed to write {}", source_path.display()))?;
        return Ok(RegionRewriteOutcome {
            changed_paths: vec![source_path],
            removed_paths: Vec::new(),
        });
    }

    let target_insert = insertion_byte_offset(target_source.as_deref().unwrap_or(""), target)?;
    let rewritten_target = insert_region_text(
        target_source.unwrap_or_default(),
        target_insert,
        &region_text,
    );

    let mut changed_paths = vec![target_path.clone()];
    let mut removed_paths = Vec::new();

    let mut rewritten_source = String::new();
    rewritten_source.push_str(&source_source[..selection_start]);
    rewritten_source.push_str(&source_source[selection_end..]);
    if rewritten_source.trim().is_empty() {
        fs::remove_file(&source_path)
            .with_context(|| format!("failed to remove {}", source_path.display()))?;
        removed_paths.push(source_path);
    } else {
        fs::write(&source_path, &rewritten_source)
            .with_context(|| format!("failed to write {}", source_path.display()))?;
        changed_paths.push(source_path);
    }

    fs::write(&target_path, rewritten_target)
        .with_context(|| format!("failed to write {}", target_path.display()))?;

    Ok(RegionRewriteOutcome {
        changed_paths,
        removed_paths,
    })
}

pub fn extract_subtree(
    root: &Path,
    source: &AnchorRecord,
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

fn normalize_region_for_target(region_text: &str, desired_root_level: usize) -> Result<String> {
    let had_trailing_newline = region_text.ends_with('\n');
    let mut lines = region_text
        .lines()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    shift_region_heading_levels(&mut lines, desired_root_level)?;
    let mut rendered = lines.join("\n");
    if had_trailing_newline && !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn shift_region_heading_levels(lines: &mut [String], desired_root_level: usize) -> Result<()> {
    let Some(min_level) = lines
        .iter()
        .filter_map(|line| crate::document::heading_level(line))
        .min()
    else {
        return Ok(());
    };
    let delta = desired_root_level as isize - min_level as isize;
    for line in lines {
        if let Some(level) = crate::document::heading_level(line) {
            let trimmed = line.trim_start();
            let new_level = (level as isize + delta).max(1) as usize;
            *line = format!("{}{}", "*".repeat(new_level), &trimmed[level..]);
        }
    }
    Ok(())
}

fn insertion_byte_offset(source: &str, target: &NodeRecord) -> Result<usize> {
    let document = OrgDocument::from_source(source);
    let line_index = document.insertion_index(target)?;
    line_index_byte_offset(source, line_index)
}

fn line_index_byte_offset(source: &str, line_index: usize) -> Result<usize> {
    if line_index == 0 {
        return Ok(0);
    }

    let mut current_line = 0usize;
    for (offset, character) in source.char_indices() {
        if current_line == line_index {
            return Ok(offset);
        }
        if character == '\n' {
            current_line += 1;
            if current_line == line_index {
                return Ok(offset + 1);
            }
        }
    }

    if current_line == line_index || line_index >= source.lines().count() {
        Ok(source.len())
    } else {
        bail!("line index {line_index} is out of range")
    }
}

fn byte_offset_for_char_position(source: &str, position: usize) -> Result<usize> {
    if position == 0 {
        bail!("character positions must be 1-based");
    }

    let wanted = position - 1;
    let char_count = source.chars().count();
    if wanted > char_count {
        bail!("character position {position} is out of range");
    }
    if wanted == char_count {
        return Ok(source.len());
    }

    source
        .char_indices()
        .nth(wanted)
        .map(|(offset, _)| offset)
        .context("failed to resolve region byte offset")
}

fn insert_region_text(mut source: String, insert_offset: usize, region_text: &str) -> String {
    let needs_leading_newline =
        insert_offset > 0 && !source[..insert_offset].ends_with('\n') && !region_text.is_empty();
    if needs_leading_newline {
        source.insert(insert_offset, '\n');
        let adjusted_offset = insert_offset + 1;
        source.insert_str(adjusted_offset, region_text);
    } else {
        source.insert_str(insert_offset, region_text);
    }
    source
}
