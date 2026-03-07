use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use slipbox_core::{CaptureContentType, CaptureTemplateParams, NodeKind, NodeRecord};
use uuid::Uuid;

pub struct CaptureOutcome {
    pub absolute_path: PathBuf,
    pub node_key: String,
}

pub struct MetadataUpdate {
    pub aliases: Option<Vec<String>>,
    pub refs: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

pub struct RewriteOutcome {
    pub changed_paths: Vec<PathBuf>,
    pub removed_paths: Vec<PathBuf>,
    pub explicit_id: String,
}

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

fn default_capture_file_title(relative_path: &str, title: &str) -> String {
    let title = title.trim();
    if !title.is_empty() {
        return title.to_owned();
    }

    Path::new(relative_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace('-', " "))
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or_else(|| String::from("Note"))
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

pub fn ensure_node_id(root: &Path, node: &NodeRecord) -> Result<PathBuf> {
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
            let target_index = source_document.heading_index(target.line as usize)?;
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

fn normalized_title(title: &str) -> Result<&str> {
    let title = title.trim();
    if title.is_empty() {
        bail!("capture title must not be empty");
    }
    Ok(title)
}

fn normalize_relative_org_path(file_path: &str) -> Result<String> {
    let candidate = Path::new(file_path);
    if candidate.is_absolute() {
        bail!("file path must be relative to the slipbox root");
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("file path must stay within the slipbox root")
            }
        }
    }

    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if normalized.is_empty() {
        bail!("file path must not be empty");
    }
    if !normalized.ends_with(".org") {
        bail!("file path must end with .org");
    }

    Ok(normalized)
}

fn next_available_path(root: &Path, slug: &str) -> String {
    for suffix in 0.. {
        let filename = if suffix == 0 {
            format!("{slug}.org")
        } else {
            format!("{slug}-{suffix}.org")
        };
        if !root.join(&filename).exists() {
            return filename;
        }
    }

    unreachable!("unbounded path generation must eventually find an unused file name")
}

fn next_available_relative_path(root: &Path, file_path: &str) -> Result<String> {
    let normalized = normalize_relative_org_path(file_path)?;
    let candidate = Path::new(&normalized);
    let stem = candidate
        .file_stem()
        .and_then(|stem| stem.to_str())
        .context("file path must include a valid file name")?;
    let extension = candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .context("file path must include a valid extension")?;
    let parent = candidate
        .parent()
        .filter(|path| !path.as_os_str().is_empty());

    for suffix in 0.. {
        let filename = if suffix == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{suffix}.{extension}")
        };
        let relative = parent
            .map(|path| path.join(&filename))
            .unwrap_or_else(|| PathBuf::from(&filename));
        let absolute = root.join(&relative);
        if !absolute.exists() {
            return Ok(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    unreachable!("unbounded path generation must eventually find an unused file name")
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for character in title.chars() {
        let normalized = character.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        String::from("note")
    } else {
        trimmed.to_owned()
    }
}

fn property_value(values: &[String]) -> Option<String> {
    if values.is_empty() {
        None
    } else {
        Some(format_property_values(values))
    }
}

fn keyword_value(values: &[String]) -> Option<String> {
    if values.is_empty() {
        None
    } else {
        Some(format_colon_tags(values))
    }
}

fn format_property_values(values: &[String]) -> String {
    values
        .iter()
        .map(|value| {
            if value
                .chars()
                .any(|character| character.is_whitespace() || character == '"')
            {
                format!("{value:?}")
            } else {
                value.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_colon_tags(values: &[String]) -> String {
    format!(":{}:", values.join(":"))
}

fn render_lines(lines: &[String], had_trailing_newline: bool) -> String {
    let mut rendered = lines.join("\n");
    if (had_trailing_newline || !rendered.ends_with('\n')) && !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered
}

fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let stars = trimmed
        .chars()
        .take_while(|character| *character == '*')
        .count();
    if stars == 0
        || !trimmed
            .chars()
            .nth(stars)
            .is_some_and(|character| character.is_whitespace())
    {
        return None;
    }
    Some(stars)
}

fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    if line.len() < keyword.len() {
        return None;
    }

    let prefix = &line[..keyword.len()];
    if prefix.eq_ignore_ascii_case(keyword) {
        Some(&line[keyword.len()..])
    } else {
        None
    }
}

fn split_heading_tags(input: &str) -> (&str, Vec<String>) {
    let Some(position) = input.rfind(" :") else {
        return (input.trim(), Vec::new());
    };
    let title = &input[..position];
    let suffix = &input[position + 1..];
    let tags = parse_colon_tags(suffix);
    if tags.is_empty() {
        (input.trim(), Vec::new())
    } else {
        (title.trim_end(), dedup_strings(tags))
    }
}

fn split_todo_keyword(input: &str) -> (Option<String>, &str) {
    let Some((first, rest)) = input.split_once(' ') else {
        return (None, input);
    };

    if looks_like_todo_keyword(first) {
        (Some(first.to_owned()), rest.trim_start())
    } else {
        (None, input)
    }
}

fn looks_like_todo_keyword(token: &str) -> bool {
    matches!(
        token,
        "TODO" | "DONE" | "NEXT" | "WAITING" | "STARTED" | "CANCELLED" | "HOLD"
    )
}

fn is_heading_planning_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("SCHEDULED:")
        || trimmed.starts_with("DEADLINE:")
        || trimmed.starts_with("CLOSED:")
}

fn looks_like_list_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    looks_like_checkitem(trimmed)
        || trimmed.starts_with("- ")
        || trimmed.starts_with("+ ")
        || trimmed.starts_with("* ")
        || trimmed
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
            && (trimmed.contains(". ") || trimmed.contains(") "))
}

fn looks_like_checkitem(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("- [") || trimmed.starts_with("+ [") || trimmed.starts_with("* [")
}

fn parse_colon_tags(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with(':') || !trimmed.ends_with(':') {
        return Vec::new();
    }

    trimmed
        .split(':')
        .filter(|part| !part.is_empty())
        .filter(|part| !part.chars().any(char::is_whitespace))
        .map(ToOwned::to_owned)
        .collect()
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !value.is_empty() && !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

fn shift_subtree_levels(lines: &mut [String], desired_root_level: usize) -> Result<()> {
    let current_root_level = lines
        .first()
        .and_then(|line| heading_level(line))
        .context("subtree must begin with a heading")?;
    let delta = desired_root_level as isize - current_root_level as isize;

    for line in lines {
        if let Some(level) = heading_level(line) {
            let trimmed = line.trim_start();
            let new_level = (level as isize + delta).max(1) as usize;
            *line = format!("{}{}", "*".repeat(new_level), &trimmed[level..]);
        }
    }

    Ok(())
}

fn promote_subtree_lines(lines: &mut [String]) {
    for line in lines {
        if let Some(level) = heading_level(line) {
            let trimmed = line.trim_start();
            let new_level = level.saturating_sub(1).max(1);
            *line = format!("{}{}", "*".repeat(new_level), &trimmed[level..]);
        }
    }
}

fn demote_all_headings(lines: &mut [String]) {
    for line in lines {
        if let Some(level) = heading_level(line) {
            let trimmed = line.trim_start();
            *line = format!("{}{}", "*".repeat(level + 1), &trimmed[level..]);
        }
    }
}

fn format_heading_line(level: usize, title: &str, tags: &[String]) -> String {
    if tags.is_empty() {
        format!("{} {}", "*".repeat(level), title)
    } else {
        format!(
            "{} {} {}",
            "*".repeat(level),
            title,
            format_colon_tags(tags)
        )
    }
}

struct OrgDocument {
    lines: Vec<String>,
    had_trailing_newline: bool,
}

impl OrgDocument {
    fn from_source(source: &str) -> Self {
        Self {
            lines: source.lines().map(ToOwned::to_owned).collect(),
            had_trailing_newline: source.ends_with('\n'),
        }
    }

    fn from_extracted_subtree(subtree_lines: &[String], explicit_id: &str) -> Result<Self> {
        let root_line = subtree_lines
            .first()
            .context("subtree must begin with a heading")?;
        let level = heading_level(root_line).context("subtree must begin with a heading")?;
        let heading_text = root_line.trim_start()[level + 1..].trim();
        let (title_text, tags) = split_heading_tags(heading_text);
        let (_, title) = split_todo_keyword(title_text);
        let title = title.trim();
        if title.is_empty() {
            bail!("subtree heading must include a title");
        }

        let mut lines = vec![format!("#+title: {title}")];
        if !tags.is_empty() {
            lines.push(format!("#+filetags: {}", format_colon_tags(&tags)));
        }
        let mut remainder = subtree_lines[1..].to_vec();
        promote_subtree_lines(&mut remainder);
        lines.extend(remainder);

        let mut document = Self {
            lines,
            had_trailing_newline: true,
        };
        document.set_file_property("ID", Some(explicit_id.to_owned()));
        Ok(document)
    }

    fn render(&self) -> String {
        if self.lines.is_empty() {
            String::new()
        } else {
            render_lines(&self.lines, self.had_trailing_newline)
        }
    }

    fn has_meaningful_content(&self) -> bool {
        self.lines.iter().any(|line| !line.trim().is_empty())
    }

    fn demote_entire_file(&mut self, relative_path: &str) {
        let title = self
            .file_keyword_value("title")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_capture_file_title(relative_path, ""));
        let tags = self.filetags();

        self.set_file_keyword("title", None);
        self.set_file_keyword("filetags", None);
        demote_all_headings(&mut self.lines);
        self.lines.insert(0, format_heading_line(1, &title, &tags));
    }

    fn promote_entire_file(&mut self) -> Result<()> {
        if !self.buffer_promoteable_p() {
            bail!("cannot promote: multiple root headings or there is extra file-level text");
        }

        let heading = self.lines.remove(0);
        let heading_text = heading.trim_start();
        let (title_text, tags) = split_heading_tags(heading_text[2..].trim());
        let (_, title) = split_todo_keyword(title_text);
        let title = title.trim();
        if title.is_empty() {
            bail!("cannot promote: top-level heading must have a title");
        }

        promote_subtree_lines(&mut self.lines);
        self.set_file_keyword("title", Some(title.to_owned()));
        self.set_file_keyword("filetags", keyword_value(&tags));
        Ok(())
    }

    fn heading_index(&self, line_number: usize) -> Result<usize> {
        if line_number == 0 || line_number > self.lines.len() {
            bail!("heading line {line_number} is out of range");
        }
        let index = line_number - 1;
        if heading_level(&self.lines[index]).is_none() {
            bail!("line {line_number} is not a heading");
        }
        Ok(index)
    }

    fn subtree_range(&self, line_number: usize) -> Result<(usize, usize)> {
        let start = self.heading_index(line_number)?;
        let level = heading_level(&self.lines[start]).context("heading line is invalid")?;
        let mut end = self.lines.len();
        for (index, line) in self.lines.iter().enumerate().skip(start + 1) {
            if heading_level(line).is_some_and(|candidate| candidate <= level) {
                end = index;
                break;
            }
        }
        Ok((start, end))
    }

    fn insertion_index(&self, target: &NodeRecord) -> Result<usize> {
        match target.kind {
            NodeKind::File => Ok(self.lines.len()),
            NodeKind::Heading => {
                let start = self.heading_index(target.line as usize)?;
                let level = target.level as usize;
                let mut insert_index = self.lines.len();
                for (index, line) in self.lines.iter().enumerate().skip(start + 1) {
                    if heading_level(line).is_some_and(|candidate| candidate <= level) {
                        insert_index = index;
                        break;
                    }
                }
                Ok(insert_index)
            }
        }
    }

    fn file_entry_insert_index(&self, prepend: bool) -> usize {
        if !prepend {
            return self.lines.len();
        }

        let (_, body_end) = self.file_body_bounds();
        self.lines
            .iter()
            .enumerate()
            .skip(body_end)
            .find_map(|(index, line)| heading_level(line).map(|_| index))
            .unwrap_or(self.lines.len())
    }

    fn heading_entry_insert_index(
        &self,
        line_number: usize,
        level: usize,
        prepend: bool,
    ) -> Result<usize> {
        let (_, subtree_end) = self.subtree_range(line_number)?;
        if !prepend {
            return Ok(subtree_end);
        }

        let (body_start, _) = self.heading_body_bounds(line_number, level)?;
        Ok(self
            .lines
            .iter()
            .enumerate()
            .skip(body_start)
            .take(subtree_end.saturating_sub(body_start))
            .find_map(|(index, line)| {
                heading_level(line).and_then(|candidate| (candidate > level).then_some(index))
            })
            .unwrap_or(subtree_end))
    }

    fn file_body_bounds(&self) -> (usize, usize) {
        let start = self.file_body_start_index();
        let end = self
            .lines
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(index, line)| heading_level(line).map(|_| index))
            .unwrap_or(self.lines.len());
        (start, end)
    }

    fn heading_body_bounds(&self, line_number: usize, level: usize) -> Result<(usize, usize)> {
        let start = self.heading_body_start_index(line_number)?;
        let (subtree_start, subtree_end) = self.subtree_range(line_number)?;
        let search_start = start.max(subtree_start + 1);
        let end = self
            .lines
            .iter()
            .enumerate()
            .skip(search_start)
            .take(subtree_end.saturating_sub(search_start))
            .find_map(|(index, line)| {
                heading_level(line).and_then(|candidate| (candidate > level).then_some(index))
            })
            .unwrap_or(subtree_end);
        Ok((start, end))
    }

    fn list_bounds(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let mut list_start = None;
        for index in start..end {
            let line = self.lines.get(index)?;
            if looks_like_list_item(line) {
                list_start = Some(index);
                break;
            }
        }

        let start = list_start?;
        let mut end_index = end;
        for index in start + 1..end {
            let line = self.lines.get(index)?;
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                end_index = index;
                break;
            }
            if looks_like_list_item(trimmed) || line.starts_with(' ') || line.starts_with('\t') {
                continue;
            }
            end_index = index;
            break;
        }

        Some((start, end_index))
    }

    fn table_bounds(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let mut table_start = None;
        for index in start..end {
            let line = self.lines.get(index)?;
            if line.trim_start().starts_with('|') {
                table_start = Some(index);
                break;
            }
        }

        let start = table_start?;
        let mut end_index = end;
        for index in start + 1..end {
            let line = self.lines.get(index)?;
            if line.trim().is_empty() {
                end_index = index;
                break;
            }
            if line.trim_start().starts_with('|') {
                continue;
            }
            end_index = index;
            break;
        }

        Some((start, end_index))
    }

    fn insert_block(
        &mut self,
        index: usize,
        block: Vec<String>,
        blank_lines_before: usize,
        blank_lines_after: usize,
    ) -> usize {
        let before_existing = self.count_blank_lines_before(index);
        let after_existing = self.count_blank_lines_after(index);
        let add_before = blank_lines_before.saturating_sub(before_existing);
        let add_after = blank_lines_after.saturating_sub(after_existing);
        let content_index = index + add_before;
        let mut insertion = Vec::new();
        insertion.extend(std::iter::repeat_n(String::new(), add_before));
        insertion.extend(block);
        insertion.extend(std::iter::repeat_n(String::new(), add_after));
        self.lines.splice(index..index, insertion);
        content_index + 1
    }

    fn remove_range(&mut self, start: usize, end: usize) {
        self.lines.drain(start..end);
    }

    fn insert_subtree(
        &mut self,
        index: usize,
        target_kind: NodeKind,
        mut subtree_lines: Vec<String>,
    ) {
        if matches!(target_kind, NodeKind::File)
            && index > 0
            && self
                .lines
                .get(index - 1)
                .is_some_and(|line| !line.trim().is_empty())
        {
            subtree_lines.insert(0, String::new());
        }

        self.lines.splice(index..index, subtree_lines);
    }

    fn subtree_lines(&self, node: &NodeRecord) -> Result<(Vec<String>, String)> {
        match node.kind {
            NodeKind::Heading => {
                let (start, end) = self.subtree_range(node.line as usize)?;
                let mut lines = self.lines[start..end].to_vec();
                let explicit_id = node
                    .explicit_id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                ensure_heading_property_value(&mut lines, 0, "ID", Some(explicit_id.clone()))?;
                Ok((lines, explicit_id))
            }
            NodeKind::File => {
                let mut lines = self.lines.clone();
                remove_file_keyword(&mut lines, "title");
                remove_file_keyword(&mut lines, "filetags");
                demote_all_headings(&mut lines);
                lines.insert(0, format_heading_line(1, &node.title, &node.tags));

                let explicit_id = node
                    .explicit_id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                ensure_heading_property_value(&mut lines, 0, "ID", Some(explicit_id.clone()))?;
                Ok((lines, explicit_id))
            }
        }
    }

    fn set_file_property(&mut self, property: &str, value: Option<String>) {
        set_file_property_value(&mut self.lines, property, value);
    }

    fn set_heading_property(
        &mut self,
        line_number: usize,
        property: &str,
        value: Option<String>,
    ) -> Result<()> {
        let heading_index = self.heading_index(line_number)?;
        ensure_heading_property_value(&mut self.lines, heading_index, property, value)
    }

    fn set_file_keyword(&mut self, keyword: &str, value: Option<String>) {
        set_file_keyword_value(&mut self.lines, keyword, value);
    }

    fn set_heading_tags(&mut self, line_number: usize, tags: &[String]) -> Result<()> {
        let index = self.heading_index(line_number)?;
        let level = heading_level(&self.lines[index]).context("heading line is invalid")?;
        let trimmed = self.lines[index].trim_start();
        let heading_text = trimmed[level + 1..].trim();
        let (title, _) = split_heading_tags(heading_text);
        self.lines[index] = format_heading_line(level, title, tags);
        Ok(())
    }

    fn ensure_file_identity(&mut self) -> Result<()> {
        self.ensure_file_identity_with_refs(&[])
    }

    fn ensure_file_identity_with_refs(&mut self, refs: &[String]) -> Result<()> {
        if file_property_value(&self.lines, "ID").is_none() {
            self.set_file_property("ID", Some(Uuid::new_v4().to_string()));
        }

        if !refs.is_empty() {
            self.set_file_property("ROAM_REFS", property_value(refs));
        }

        if self.render().is_empty() {
            bail!("capture file head must not be empty");
        }

        Ok(())
    }

    fn ensure_outline_path(&mut self, outline_path: &[String]) -> Result<Option<(usize, usize)>> {
        if outline_path.is_empty() {
            return Ok(None);
        }

        let mut parent_start = 0usize;
        let mut parent_end = self.lines.len();
        let mut parent_level = 0usize;
        let mut current = None;

        for heading in outline_path {
            let wanted_level = parent_level + 1;
            let mut found = None;

            for (index, line) in self
                .lines
                .iter()
                .enumerate()
                .skip(parent_start)
                .take(parent_end.saturating_sub(parent_start))
            {
                if heading_level(line).is_some_and(|candidate| candidate == wanted_level)
                    && heading_title(line).is_some_and(|title| title == *heading)
                {
                    if found.is_some() {
                        bail!("heading not unique on level {wanted_level}: {heading}");
                    }
                    found = Some(index);
                }
            }

            let heading_index = if let Some(index) = found {
                index
            } else {
                self.insert_outline_heading(parent_end, wanted_level, heading)
            };
            let (_, subtree_end) = self.subtree_range(heading_index + 1)?;
            parent_start = heading_index;
            parent_end = subtree_end;
            parent_level = wanted_level;
            current = Some((heading_index + 1, wanted_level));
        }

        Ok(current)
    }

    fn file_keyword_value(&self, keyword: &str) -> Option<String> {
        file_keyword_value(&self.lines, keyword)
    }

    fn filetags(&self) -> Vec<String> {
        self.file_keyword_value("filetags")
            .map(|value| parse_colon_tags(&value))
            .unwrap_or_default()
    }

    fn h1_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| heading_level(line) == Some(1))
            .count()
    }

    fn buffer_promoteable_p(&self) -> bool {
        self.h1_count() == 1
            && self
                .lines
                .first()
                .is_some_and(|line| heading_level(line) == Some(1))
    }

    fn file_body_start_index(&self) -> usize {
        let mut index = file_property_drawer_bounds(&self.lines)
            .map(|(_, end)| end)
            .unwrap_or_else(|| file_property_insert_index(&self.lines));
        while index < self.lines.len() && self.lines[index].trim().is_empty() {
            index += 1;
        }
        index
    }

    fn heading_body_start_index(&self, line_number: usize) -> Result<usize> {
        let mut index = self.heading_index(line_number)? + 1;
        if self
            .lines
            .get(index)
            .is_some_and(|line| line.trim().eq_ignore_ascii_case(":PROPERTIES:"))
        {
            index += 1;
            while index < self.lines.len()
                && !self.lines[index].trim().eq_ignore_ascii_case(":END:")
            {
                index += 1;
            }
            if index < self.lines.len() {
                index += 1;
            }
        }
        while index < self.lines.len() && is_heading_planning_line(&self.lines[index]) {
            index += 1;
        }
        while index < self.lines.len() && self.lines[index].trim().is_empty() {
            index += 1;
        }
        Ok(index)
    }

    fn count_blank_lines_before(&self, index: usize) -> usize {
        let mut blanks = 0;
        let mut cursor = index;
        while cursor > 0 {
            cursor -= 1;
            if self.lines[cursor].trim().is_empty() {
                blanks += 1;
            } else {
                break;
            }
        }
        blanks
    }

    fn count_blank_lines_after(&self, index: usize) -> usize {
        let mut blanks = 0;
        let mut cursor = index;
        while cursor < self.lines.len() {
            if self.lines[cursor].trim().is_empty() {
                blanks += 1;
                cursor += 1;
            } else {
                break;
            }
        }
        blanks
    }

    fn insert_outline_heading(&mut self, index: usize, level: usize, title: &str) -> usize {
        let mut insertion = Vec::new();
        let needs_blank = index > 0
            && self
                .lines
                .get(index - 1)
                .is_some_and(|line| !line.trim().is_empty());
        if needs_blank {
            insertion.push(String::new());
        }
        insertion.push(format_heading_line(level, title, &[]));
        let heading_index = index + usize::from(needs_blank);
        self.lines.splice(index..index, insertion);
        heading_index
    }
}

fn heading_title(line: &str) -> Option<String> {
    let level = heading_level(line)?;
    let trimmed = line.trim_start();
    let heading_text = trimmed[level + 1..].trim();
    let (title, _) = split_heading_tags(heading_text);
    let (_, title) = split_todo_keyword(title);
    let title = title.trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_owned())
    }
}

fn remove_file_keyword(lines: &mut Vec<String>, keyword: &str) {
    let limit = file_keyword_limit(lines);
    if let Some(index) = (0..limit)
        .find(|index| strip_keyword(lines[*index].trim_start(), &format!("#+{keyword}:")).is_some())
    {
        lines.remove(index);
    }
}

fn file_keyword_limit(lines: &[String]) -> usize {
    lines
        .iter()
        .position(|line| heading_level(line).is_some())
        .unwrap_or(lines.len())
}

fn file_property_insert_index(lines: &[String]) -> usize {
    let mut index = 0;
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }

    while index < lines.len() && lines[index].trim_start().starts_with("#+") {
        index += 1;
    }

    index
}

fn file_keyword_value(lines: &[String], keyword: &str) -> Option<String> {
    let needle = format!("#+{keyword}:");
    (0..file_keyword_limit(lines)).find_map(|index| {
        strip_keyword(lines[index].trim_start(), &needle).map(|value| value.trim().to_owned())
    })
}

fn file_property_drawer_bounds(lines: &[String]) -> Option<(usize, usize)> {
    let mut index = file_property_insert_index(lines);
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }

    if lines
        .get(index)
        .is_some_and(|line| line.trim().eq_ignore_ascii_case(":PROPERTIES:"))
    {
        for (end, line) in lines.iter().enumerate().skip(index + 1) {
            if line.trim().eq_ignore_ascii_case(":END:") {
                return Some((index, end + 1));
            }
        }
    }

    None
}

fn file_property_value(lines: &[String], property: &str) -> Option<String> {
    let (start, end) = file_property_drawer_bounds(lines)?;
    let property_line = format!(":{property}:");
    (start + 1..end - 1).find_map(|index| {
        strip_keyword(lines[index].trim(), &property_line).map(|value| value.trim().to_owned())
    })
}

fn normalized_head_source(head: Option<&str>) -> String {
    match head {
        Some(head) if !head.trim().is_empty() => {
            let mut source = head.to_owned();
            if !source.ends_with('\n') {
                source.push('\n');
            }
            source
        }
        _ => String::new(),
    }
}

fn set_file_property_value(lines: &mut Vec<String>, property: &str, value: Option<String>) {
    if let Some((start, end)) = file_property_drawer_bounds(lines) {
        let property_line = format!(":{property}:");
        if let Some(index) = (start + 1..end - 1)
            .find(|index| strip_keyword(lines[*index].trim(), &property_line).is_some())
        {
            if let Some(value) = value {
                lines[index] = format!(":{property}: {value}");
            } else {
                lines.remove(index);
                if start + 1 == end - 2 {
                    lines.drain(start..start + 2);
                }
            }
            return;
        }

        if let Some(value) = value {
            lines.insert(end - 1, format!(":{property}: {value}"));
        }
        return;
    }

    if let Some(value) = value {
        let insert_index = file_property_insert_index(lines);
        let mut drawer = vec![
            String::from(":PROPERTIES:"),
            format!(":{property}: {value}"),
            String::from(":END:"),
        ];
        if insert_index == lines.len()
            || lines
                .get(insert_index)
                .is_some_and(|line| !line.trim().is_empty())
        {
            drawer.push(String::new());
        }
        lines.splice(insert_index..insert_index, drawer);
    }
}

fn set_file_keyword_value(lines: &mut Vec<String>, keyword: &str, value: Option<String>) {
    let needle = format!("#+{keyword}:");
    let limit = file_keyword_limit(lines);
    if let Some(index) =
        (0..limit).find(|index| strip_keyword(lines[*index].trim_start(), &needle).is_some())
    {
        if let Some(value) = value {
            lines[index] = format!("#+{}: {value}", keyword.to_ascii_lowercase());
        } else {
            lines.remove(index);
        }
        return;
    }

    if let Some(value) = value {
        let insert_index = file_property_insert_index(lines);
        lines.insert(
            insert_index,
            format!("#+{}: {value}", keyword.to_ascii_lowercase()),
        );
    }
}

fn heading_property_drawer_bounds(
    lines: &[String],
    heading_index: usize,
) -> Option<(usize, usize)> {
    let drawer_start = heading_index + 1;
    if lines
        .get(drawer_start)
        .is_some_and(|line| line.trim().eq_ignore_ascii_case(":PROPERTIES:"))
    {
        for (end, line) in lines.iter().enumerate().skip(drawer_start + 1) {
            if line.trim().eq_ignore_ascii_case(":END:") {
                return Some((drawer_start, end + 1));
            }
        }
    }
    None
}

fn ensure_heading_property_value(
    lines: &mut Vec<String>,
    heading_index: usize,
    property: &str,
    value: Option<String>,
) -> Result<()> {
    if heading_index >= lines.len() || heading_level(&lines[heading_index]).is_none() {
        bail!("heading line {} is out of range", heading_index + 1);
    }

    if let Some((start, end)) = heading_property_drawer_bounds(lines, heading_index) {
        let property_line = format!(":{property}:");
        if let Some(index) = (start + 1..end - 1)
            .find(|index| strip_keyword(lines[*index].trim(), &property_line).is_some())
        {
            if let Some(value) = value {
                lines[index] = format!(":{property}: {value}");
            } else {
                lines.remove(index);
                if start + 1 == end - 2 {
                    lines.drain(start..start + 2);
                }
            }
            return Ok(());
        }

        if let Some(value) = value {
            lines.insert(end - 1, format!(":{property}: {value}"));
        }
        return Ok(());
    }

    if let Some(value) = value {
        lines.splice(
            heading_index + 1..heading_index + 1,
            [
                String::from(":PROPERTIES:"),
                format!(":{property}: {value}"),
                String::from(":END:"),
            ],
        );
    }

    Ok(())
}
