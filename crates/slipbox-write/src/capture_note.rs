use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use slipbox_core::NodeKind;
use slipbox_core::NodeRecord;
use uuid::Uuid;

use crate::CaptureOutcome;
use crate::document::{OrgDocument, format_property_values, heading_level, render_lines};
use crate::path::{
    next_available_path, next_available_relative_path, normalize_relative_org_path,
    normalized_head_source, normalized_title, slugify,
};

pub fn capture_file_note(root: &Path, title: &str) -> Result<CaptureOutcome> {
    capture_file_note_with_refs(root, title, &[])
}

pub fn capture_file_note_with_refs(
    root: &Path,
    title: &str,
    refs: &[String],
) -> Result<CaptureOutcome> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let title = normalized_title(title)?;
    let slug = slugify(title);
    let relative_path = next_available_path(root, &slug);
    create_file_note(root, &relative_path, title, refs)
}

pub fn capture_file_note_at(root: &Path, file_path: &str, title: &str) -> Result<CaptureOutcome> {
    capture_file_note_at_with_refs(root, file_path, title, &[])
}

pub fn capture_file_note_at_with_refs(
    root: &Path,
    file_path: &str,
    title: &str,
    refs: &[String],
) -> Result<CaptureOutcome> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let title = normalized_title(title)?;
    let relative_path = next_available_relative_path(root, file_path)?;
    create_file_note(root, &relative_path, title, refs)
}

pub fn ensure_file_note(root: &Path, file_path: &str, title: &str) -> Result<CaptureOutcome> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let title = normalized_title(title)?;
    let relative_path = normalize_relative_org_path(file_path)?;
    let absolute_path = root.join(&relative_path);
    if !absolute_path.exists() {
        write_file_note(&absolute_path, title, &[])?;
    }

    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("file:{}", relative_path.replace('\\', "/")),
    })
}

pub fn append_heading(
    root: &Path,
    file_path: &str,
    title: &str,
    heading: &str,
    level: usize,
) -> Result<CaptureOutcome> {
    let heading = heading.trim();
    if heading.is_empty() {
        bail!("capture heading must not be empty");
    }

    let file_note = ensure_file_note(root, file_path, title)?;
    let source = fs::read_to_string(&file_note.absolute_path)
        .with_context(|| format!("failed to read {}", file_note.absolute_path.display()))?;
    let (updated, line_number) = append_heading_to_source(&source, heading, level.max(1));
    fs::write(&file_note.absolute_path, updated)
        .with_context(|| format!("failed to write {}", file_note.absolute_path.display()))?;

    Ok(CaptureOutcome {
        absolute_path: file_note.absolute_path,
        node_key: format!(
            "heading:{}:{line_number}",
            file_note.node_key.trim_start_matches("file:")
        ),
    })
}

pub fn append_heading_to_node(
    root: &Path,
    node: &NodeRecord,
    heading: &str,
) -> Result<CaptureOutcome> {
    let heading = heading.trim();
    if heading.is_empty() {
        bail!("capture heading must not be empty");
    }

    let absolute_path = root.join(&node.file_path);
    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let (updated, line_number) = match node.kind {
        NodeKind::File => append_heading_to_source(&source, heading, 1),
        NodeKind::Heading => {
            append_heading_under_node(&source, node.line as usize, node.level as usize, heading)?
        }
    };
    fs::write(&absolute_path, updated)
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;

    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("heading:{}:{line_number}", node.file_path),
    })
}

pub fn append_heading_at_outline_path(
    root: &Path,
    file_path: &str,
    heading: &str,
    outline_path: &[String],
    head: Option<&str>,
) -> Result<CaptureOutcome> {
    let heading = heading.trim();
    if heading.is_empty() {
        bail!("capture heading must not be empty");
    }

    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let relative_path = normalize_relative_org_path(file_path)?;
    let absolute_path = root.join(&relative_path);
    let source = if absolute_path.exists() {
        fs::read_to_string(&absolute_path)
            .with_context(|| format!("failed to read {}", absolute_path.display()))?
    } else {
        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
        normalized_head_source(head)
    };
    let mut document = OrgDocument::from_source(&source);
    let (rendered, line_number) =
        if let Some((line_number, level)) = document.ensure_outline_path(outline_path)? {
            append_heading_under_node(&document.render(), line_number, level, heading)?
        } else {
            append_heading_to_source(&document.render(), heading, 1)
        };
    fs::write(&absolute_path, rendered)
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;

    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("heading:{}:{line_number}", relative_path.replace('\\', "/")),
    })
}

pub fn capture_file_note_at_with_head_and_refs(
    root: &Path,
    file_path: &str,
    title: &str,
    head: &str,
    refs: &[String],
) -> Result<CaptureOutcome> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let _ = normalized_title(title)?;
    let relative_path = next_available_relative_path(root, file_path)?;
    let absolute_path = root.join(&relative_path);
    let mut document = OrgDocument::from_source(&normalized_head_source(Some(head)));

    if refs.is_empty() {
        document.ensure_file_identity()?;
    } else {
        document.ensure_file_identity_with_refs(refs)?;
    }

    fs::write(&absolute_path, document.render())
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;

    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("file:{}", relative_path.replace('\\', "/")),
    })
}

fn create_file_note(
    root: &Path,
    file_path: &str,
    title: &str,
    refs: &[String],
) -> Result<CaptureOutcome> {
    let relative_path = normalize_relative_org_path(file_path)?;
    let absolute_path = root.join(&relative_path);
    write_file_note(&absolute_path, title, refs)?;

    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("file:{}", relative_path.replace('\\', "/")),
    })
}

fn write_file_note(path: &Path, title: &str, refs: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let explicit_id = Uuid::new_v4().to_string();
    let mut content = format!("#+title: {title}\n:PROPERTIES:\n:ID: {explicit_id}\n");
    if !refs.is_empty() {
        content.push_str(&format!(":ROAM_REFS: {}\n", format_property_values(refs)));
    }
    content.push_str(":END:\n\n");
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

fn append_heading_to_source(source: &str, heading: &str, level: usize) -> (String, usize) {
    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    if !lines.is_empty() && lines.last().is_some_and(|line| !line.trim().is_empty()) {
        lines.push(String::new());
    }

    let line_number = lines.len() + 1;
    lines.push(format!("{} {}", "*".repeat(level), heading));

    (render_lines(&lines, source.ends_with('\n')), line_number)
}

fn append_heading_under_node(
    source: &str,
    line_number: usize,
    level: usize,
    heading: &str,
) -> Result<(String, usize)> {
    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    if line_number == 0 || line_number > lines.len() {
        bail!("heading line {line_number} is out of range");
    }

    let heading_index = line_number - 1;
    let mut insert_index = lines.len();
    for (index, line) in lines.iter().enumerate().skip(heading_index + 1) {
        if heading_level(line).is_some_and(|candidate| candidate <= level) {
            insert_index = index;
            break;
        }
    }

    if insert_index > 0 && !lines[insert_index - 1].trim().is_empty() {
        lines.insert(insert_index, String::new());
        insert_index += 1;
    }

    let line_number = insert_index + 1;
    lines.insert(
        insert_index,
        format!("{} {}", "*".repeat(level + 1), heading),
    );

    Ok((render_lines(&lines, source.ends_with('\n')), line_number))
}
