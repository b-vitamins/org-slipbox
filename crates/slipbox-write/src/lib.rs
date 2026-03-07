use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use slipbox_core::{NodeKind, NodeRecord};
use uuid::Uuid;

pub struct CaptureOutcome {
    pub absolute_path: PathBuf,
    pub node_key: String,
}

pub fn capture_file_note(root: &Path, title: &str) -> Result<CaptureOutcome> {
    let title = title.trim();
    if title.is_empty() {
        bail!("capture title must not be empty");
    }

    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let slug = slugify(title);
    let relative_path = next_available_path(root, &slug);
    let absolute_path = root.join(&relative_path);
    let explicit_id = Uuid::new_v4().to_string();
    let content = format!("#+title: {title}\n:PROPERTIES:\n:ID: {explicit_id}\n:END:\n\n");
    fs::write(&absolute_path, content)
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;

    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("file:{}", relative_path.replace('\\', "/")),
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
