use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use slipbox_core::{NodeKind, NodeRecord};
use uuid::Uuid;

pub struct CaptureOutcome {
    pub absolute_path: PathBuf,
    pub node_key: String,
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

fn insert_file_id(source: &str, explicit_id: &str) -> String {
    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let mut index = 0;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() || trimmed.starts_with("#+") {
            index += 1;
            continue;
        }

        if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
            let mut property_index = index + 1;
            while property_index < lines.len() {
                if lines[property_index].trim().eq_ignore_ascii_case(":END:") {
                    lines.insert(property_index, format!(":ID: {explicit_id}"));
                    return render_lines(&lines, source.ends_with('\n'));
                }
                property_index += 1;
            }
        }
        break;
    }

    lines.splice(
        index..index,
        [
            String::from(":PROPERTIES:"),
            format!(":ID: {explicit_id}"),
            String::from(":END:"),
            String::new(),
        ],
    );
    render_lines(&lines, source.ends_with('\n'))
}

fn insert_heading_id(source: &str, line_number: usize, explicit_id: &str) -> Result<String> {
    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    if line_number == 0 || line_number > lines.len() {
        bail!("heading line {line_number} is out of range");
    }

    let heading_index = line_number - 1;
    let next_index = heading_index + 1;
    if let Some(next_line) = lines.get(next_index)
        && next_line.trim().eq_ignore_ascii_case(":PROPERTIES:")
    {
        let mut property_index = next_index + 1;
        while property_index < lines.len() {
            if lines[property_index].trim().eq_ignore_ascii_case(":END:") {
                lines.insert(property_index, format!(":ID: {explicit_id}"));
                return Ok(render_lines(&lines, source.ends_with('\n')));
            }
            property_index += 1;
        }
    }

    lines.splice(
        next_index..next_index,
        [
            String::from(":PROPERTIES:"),
            format!(":ID: {explicit_id}"),
            String::from(":END:"),
        ],
    );
    Ok(render_lines(&lines, source.ends_with('\n')))
}

fn render_lines(lines: &[String], had_trailing_newline: bool) -> String {
    let mut rendered = lines.join("\n");
    if had_trailing_newline || !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered
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
