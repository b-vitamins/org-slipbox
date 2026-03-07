use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use slipbox_core::{CaptureContentType, CaptureTemplateParams, NodeKind, NodeRecord};
use uuid::Uuid;

use crate::CaptureOutcome;
use crate::document::{
    OrgDocument, format_property_values, heading_level, looks_like_checkitem, looks_like_list_item,
    render_lines,
};
use crate::path::{
    default_capture_file_title, next_available_path, next_available_relative_path,
    normalize_relative_org_path, normalized_head_source, normalized_title, slugify,
};

enum CaptureTargetSelection {
    File {
        relative_path: String,
        node_key: String,
    },
    Heading {
        relative_path: String,
        line_number: usize,
        level: usize,
        node_key: String,
    },
}

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

pub fn capture_template(
    root: &Path,
    target_node: Option<&NodeRecord>,
    params: &CaptureTemplateParams,
) -> Result<CaptureOutcome> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let refs = params.normalized_refs();
    let relative_path = resolve_template_relative_path(root, target_node, params)?;
    let absolute_path = root.join(&relative_path);
    let existed = absolute_path.exists();
    let source = if existed {
        fs::read_to_string(&absolute_path)
            .with_context(|| format!("failed to read {}", absolute_path.display()))?
    } else {
        normalized_head_source(params.head.as_deref())
    };
    let mut document = OrgDocument::from_source(&source);

    if !existed {
        if params.head.is_none() {
            document.set_file_keyword(
                "title",
                Some(default_capture_file_title(&relative_path, &params.title)),
            );
        }
        document.ensure_file_identity_with_refs(&refs)?;
    }

    let target = resolve_capture_target(&mut document, &relative_path, target_node, params)?;
    let node_key = match params.capture_type {
        CaptureContentType::Entry => capture_entry(
            &mut document,
            &target,
            &params.content,
            &params.title,
            params.prepend,
            params.normalized_empty_lines_before(),
            params.normalized_empty_lines_after(),
        )?,
        CaptureContentType::Plain => {
            capture_body(
                &mut document,
                &target,
                &params.content,
                CaptureContentType::Plain,
                params.prepend,
                params.normalized_empty_lines_before(),
                params.normalized_empty_lines_after(),
            )?;
            capture_target_node_key(&target)
        }
        CaptureContentType::Item => {
            capture_body(
                &mut document,
                &target,
                &params.content,
                CaptureContentType::Item,
                params.prepend,
                params.normalized_empty_lines_before(),
                params.normalized_empty_lines_after(),
            )?;
            capture_target_node_key(&target)
        }
        CaptureContentType::Checkitem => {
            capture_body(
                &mut document,
                &target,
                &params.content,
                CaptureContentType::Checkitem,
                params.prepend,
                params.normalized_empty_lines_before(),
                params.normalized_empty_lines_after(),
            )?;
            capture_target_node_key(&target)
        }
        CaptureContentType::TableLine => {
            capture_body(
                &mut document,
                &target,
                &params.content,
                CaptureContentType::TableLine,
                params.prepend,
                params.normalized_empty_lines_before(),
                params.normalized_empty_lines_after(),
            )?;
            capture_target_node_key(&target)
        }
    };

    if let Some(parent) = absolute_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    fs::write(&absolute_path, document.render())
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;

    Ok(CaptureOutcome {
        absolute_path,
        node_key,
    })
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

fn resolve_template_relative_path(
    root: &Path,
    target_node: Option<&NodeRecord>,
    params: &CaptureTemplateParams,
) -> Result<String> {
    if let Some(target) = target_node {
        return Ok(target.file_path.clone());
    }

    if let Some(file_path) = params.file_path.as_deref() {
        return normalize_relative_org_path(file_path);
    }

    let title = params.title.trim();
    let slug = if title.is_empty() {
        "note".to_owned()
    } else {
        slugify(title)
    };
    Ok(next_available_path(root, &slug))
}

fn resolve_capture_target(
    document: &mut OrgDocument,
    relative_path: &str,
    target_node: Option<&NodeRecord>,
    params: &CaptureTemplateParams,
) -> Result<CaptureTargetSelection> {
    if let Some(target) = target_node {
        return Ok(match target.kind {
            NodeKind::File => CaptureTargetSelection::File {
                relative_path: relative_path.to_owned(),
                node_key: target.node_key.clone(),
            },
            NodeKind::Heading => CaptureTargetSelection::Heading {
                relative_path: relative_path.to_owned(),
                line_number: target.line as usize,
                level: target.level as usize,
                node_key: target.node_key.clone(),
            },
        });
    }

    let outline_path = params.normalized_outline_path();
    if let Some((line_number, level)) = document.ensure_outline_path(&outline_path)? {
        Ok(CaptureTargetSelection::Heading {
            relative_path: relative_path.to_owned(),
            line_number,
            level,
            node_key: format!("heading:{}:{line_number}", relative_path.replace('\\', "/")),
        })
    } else {
        Ok(CaptureTargetSelection::File {
            relative_path: relative_path.to_owned(),
            node_key: format!("file:{}", relative_path.replace('\\', "/")),
        })
    }
}

fn capture_target_node_key(target: &CaptureTargetSelection) -> String {
    match target {
        CaptureTargetSelection::File { node_key, .. }
        | CaptureTargetSelection::Heading { node_key, .. } => node_key.clone(),
    }
}

fn capture_entry(
    document: &mut OrgDocument,
    target: &CaptureTargetSelection,
    content: &str,
    title: &str,
    prepend: bool,
    empty_lines_before: usize,
    empty_lines_after: usize,
) -> Result<String> {
    let desired_level = match target {
        CaptureTargetSelection::File { .. } => 1,
        CaptureTargetSelection::Heading { level, .. } => level + 1,
    };
    let block = entry_capture_lines(content, title, desired_level)?;
    let insert_index = match target {
        CaptureTargetSelection::File { .. } => document.file_entry_insert_index(prepend),
        CaptureTargetSelection::Heading {
            line_number, level, ..
        } => document.heading_entry_insert_index(*line_number, *level, prepend)?,
    };
    let base_before = usize::from(
        insert_index > 0
            && document
                .lines
                .get(insert_index - 1)
                .is_some_and(|line| !line.trim().is_empty()),
    );
    let line_number = document.insert_block(
        insert_index,
        block,
        empty_lines_before.max(base_before),
        empty_lines_after,
    );

    Ok(format!(
        "heading:{}:{line_number}",
        capture_target_relative_path(target).replace('\\', "/")
    ))
}

fn capture_body(
    document: &mut OrgDocument,
    target: &CaptureTargetSelection,
    content: &str,
    capture_type: CaptureContentType,
    prepend: bool,
    empty_lines_before: usize,
    empty_lines_after: usize,
) -> Result<()> {
    let mut block = body_capture_lines(content, capture_type);
    if block.is_empty() {
        return Ok(());
    }

    let (body_start, body_end) = match target {
        CaptureTargetSelection::File { .. } => document.file_body_bounds(),
        CaptureTargetSelection::Heading {
            line_number, level, ..
        } => document.heading_body_bounds(*line_number, *level)?,
    };
    let index = match capture_type {
        CaptureContentType::Plain => {
            if prepend {
                body_start
            } else {
                body_end
            }
        }
        CaptureContentType::Item | CaptureContentType::Checkitem => {
            if let Some((list_start, list_end)) = document.list_bounds(body_start, body_end) {
                if prepend { list_start } else { list_end }
            } else if prepend {
                body_start
            } else {
                body_end
            }
        }
        CaptureContentType::TableLine => {
            if let Some((table_start, table_end)) = document.table_bounds(body_start, body_end) {
                if prepend { table_start } else { table_end }
            } else if prepend {
                body_start
            } else {
                body_end
            }
        }
        CaptureContentType::Entry => unreachable!("entry capture uses capture_entry"),
    };

    if matches!(capture_type, CaptureContentType::TableLine)
        && index > 0
        && document
            .lines
            .get(index - 1)
            .is_some_and(|line| line.trim_start().starts_with('|'))
        && block.len() == 1
        && !block[0].trim_start().starts_with('|')
    {
        block[0] = format!("| {} |", block[0].trim());
    }

    document.insert_block(index, block, empty_lines_before, empty_lines_after);
    Ok(())
}

fn capture_target_relative_path(target: &CaptureTargetSelection) -> &str {
    match target {
        CaptureTargetSelection::File { relative_path, .. }
        | CaptureTargetSelection::Heading { relative_path, .. } => relative_path,
    }
}

fn entry_capture_lines(content: &str, title: &str, desired_level: usize) -> Result<Vec<String>> {
    let mut lines = trimmed_capture_lines(content);
    if lines.is_empty() {
        lines.push(format!(
            "{} {}",
            "*".repeat(desired_level),
            normalized_title(title)?
        ));
        return Ok(lines);
    }

    if let Some(current_level) = heading_level(&lines[0]) {
        let delta = desired_level as isize - current_level as isize;
        for line in &mut lines {
            if let Some(level) = heading_level(line) {
                let trimmed = line.trim_start();
                let shifted = (level as isize + delta).max(1) as usize;
                *line = format!("{}{}", "*".repeat(shifted), &trimmed[level..]);
            }
        }
        Ok(lines)
    } else {
        let mut entry = vec![format!(
            "{} {}",
            "*".repeat(desired_level),
            normalized_title(title)?
        )];
        entry.extend(lines);
        Ok(entry)
    }
}

fn body_capture_lines(content: &str, capture_type: CaptureContentType) -> Vec<String> {
    let mut lines = trimmed_capture_lines(content);
    if lines.is_empty() {
        return match capture_type {
            CaptureContentType::Plain => Vec::new(),
            CaptureContentType::Item => vec![String::from("- ")],
            CaptureContentType::Checkitem => vec![String::from("- [ ] ")],
            CaptureContentType::TableLine => vec![String::from("|  |")],
            CaptureContentType::Entry => Vec::new(),
        };
    }

    match capture_type {
        CaptureContentType::Item => {
            if !looks_like_list_item(&lines[0]) {
                lines[0] = format!("- {}", lines[0].trim_start());
            }
        }
        CaptureContentType::Checkitem => {
            if !looks_like_checkitem(&lines[0]) {
                lines[0] = format!("- [ ] {}", lines[0].trim_start());
            }
        }
        CaptureContentType::TableLine => {
            if !lines[0].trim_start().starts_with('|') {
                lines[0] = format!("| {} |", lines[0].trim());
            }
        }
        CaptureContentType::Plain | CaptureContentType::Entry => {}
    }

    lines
}

fn trimmed_capture_lines(content: &str) -> Vec<String> {
    let mut lines = content.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines
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
