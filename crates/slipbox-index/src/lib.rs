use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use slipbox_core::{IndexedFile, IndexedLink, IndexedNode, NodeKind};
use walkdir::WalkDir;

pub fn scan_root(root: &Path) -> Result<Vec<IndexedFile>> {
    let mut paths = WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(entry) if entry.file_type().is_file() && is_org_file(entry.path()) => {
                Some(Ok(entry.into_path()))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed while traversing Org files")?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| parse_path(root, &path))
        .collect()
}

pub fn scan_path(root: &Path, path: &Path) -> Result<IndexedFile> {
    parse_path(root, path)
}

fn parse_path(root: &Path, path: &Path) -> Result<IndexedFile> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let file_path = relative_path(root, path)?;
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to stat {}", path.display()))?;
    let mtime_ns = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as i64)
        .unwrap_or_default();
    Ok(parse_document(&file_path, mtime_ns, &source))
}

fn parse_document(file_path: &str, mtime_ns: i64, source: &str) -> IndexedFile {
    let lines = source.lines().collect::<Vec<_>>();
    let file_title = parse_file_title(&lines).unwrap_or_else(|| default_file_title(file_path));
    let file_properties = parse_file_properties(&lines);
    let file_tags = parse_filetags(&lines);
    let file_node_key = format!("file:{file_path}");
    let mut nodes = vec![IndexedNode {
        node_key: file_node_key.clone(),
        explicit_id: file_properties.explicit_id,
        file_path: file_path.to_owned(),
        title: file_title,
        outline_path: String::new(),
        aliases: file_properties.aliases,
        tags: file_tags.clone(),
        todo_keyword: None,
        scheduled_for: None,
        deadline_for: None,
        closed_at: None,
        level: 0,
        line: 1,
        kind: NodeKind::File,
    }];
    let mut links = Vec::new();
    let mut current_source_node_key = file_node_key;
    let mut outline_stack: Vec<String> = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if let Some((level, todo_keyword, title, heading_tags)) = parse_heading(line) {
            outline_stack.truncate(level.saturating_sub(1));
            outline_stack.push(title.clone());

            let node_key = format!("heading:{file_path}:{}", index + 1);
            current_source_node_key = node_key.clone();
            let heading_metadata = parse_heading_metadata(&lines, index + 1);
            nodes.push(IndexedNode {
                node_key,
                explicit_id: heading_metadata.explicit_id,
                file_path: file_path.to_owned(),
                title,
                outline_path: outline_stack.join(" / "),
                aliases: heading_metadata.aliases,
                tags: unique_strings(
                    file_tags
                        .iter()
                        .chain(heading_tags.iter())
                        .cloned()
                        .collect(),
                ),
                todo_keyword,
                scheduled_for: heading_metadata.scheduled_for,
                deadline_for: heading_metadata.deadline_for,
                closed_at: heading_metadata.closed_at,
                level: level as u32,
                line: (index + 1) as u32,
                kind: NodeKind::Heading,
            });
        }

        extract_id_links(line, &current_source_node_key, &mut links);
    }

    IndexedFile {
        file_path: file_path.to_owned(),
        mtime_ns,
        nodes,
        links,
    }
}

fn is_org_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("org"))
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} is not under {}", path.display(), root.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn parse_file_title(lines: &[&str]) -> Option<String> {
    lines.iter().find_map(|line| {
        strip_keyword(line, "#+title:")
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn parse_file_properties(lines: &[&str]) -> NodeProperties {
    let mut index = 0;
    while let Some(line) = lines.get(index) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("#+") {
            index += 1;
            continue;
        }

        if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
            return parse_property_drawer(lines, index);
        }
        break;
    }

    NodeProperties::default()
}

fn default_file_title(file_path: &str) -> String {
    Path::new(file_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_path)
        .to_owned()
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

fn parse_heading(line: &str) -> Option<(usize, Option<String>, String, Vec<String>)> {
    let level = line
        .chars()
        .take_while(|character| *character == '*')
        .count();
    if level == 0 || !line[level..].starts_with(' ') {
        return None;
    }

    let heading_text = line[level + 1..].trim();
    let (title, tags) = split_heading_tags(heading_text);
    let (todo_keyword, title) = split_todo_keyword(title);
    if title.is_empty() {
        None
    } else {
        Some((level, todo_keyword, title.to_owned(), tags))
    }
}

fn parse_property_drawer(lines: &[&str], start_index: usize) -> NodeProperties {
    let mut properties = NodeProperties::default();
    for line in &lines[start_index + 1..] {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case(":END:") {
            break;
        }
        if let Some(value) = strip_keyword(trimmed, ":ID:") {
            let id = value.trim();
            if !id.is_empty() {
                properties.explicit_id = Some(id.to_owned());
            }
        }
        if let Some(value) = strip_keyword(trimmed, ":ROAM_ALIASES:") {
            properties.aliases = parse_aliases(value);
        }
    }

    properties
}

fn parse_heading_metadata(lines: &[&str], start_index: usize) -> HeadingMetadata {
    let mut metadata = HeadingMetadata::default();
    let mut in_property_drawer = false;

    for line in &lines[start_index..] {
        let trimmed = line.trim();
        if parse_heading(line).is_some() {
            break;
        }

        if in_property_drawer {
            if trimmed.eq_ignore_ascii_case(":END:") {
                in_property_drawer = false;
                continue;
            }
            if let Some(value) = strip_keyword(trimmed, ":ID:") {
                let id = value.trim();
                if !id.is_empty() {
                    metadata.explicit_id = Some(id.to_owned());
                }
            }
            if let Some(value) = strip_keyword(trimmed, ":ROAM_ALIASES:") {
                metadata.aliases = parse_aliases(value);
            }
            continue;
        }

        if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
            in_property_drawer = true;
            continue;
        }

        if parse_planning_line(trimmed, &mut metadata) || trimmed.is_empty() {
            continue;
        }

        break;
    }

    metadata
}

fn parse_filetags(lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .find_map(|line| {
            strip_keyword(line, "#+filetags:")
                .or_else(|| strip_keyword(line, "#+FILETAGS:"))
                .map(parse_colon_tags)
        })
        .map(unique_strings)
        .unwrap_or_default()
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
        (title.trim_end(), unique_strings(tags))
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

fn parse_colon_tags(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with(':') || !trimmed.ends_with(':') {
        return Vec::new();
    }

    let tags = trimmed
        .split(':')
        .filter(|part| !part.is_empty())
        .filter(|part| !part.chars().any(char::is_whitespace))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if tags.is_empty() { Vec::new() } else { tags }
}

fn parse_aliases(input: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for character in input.chars() {
        match character {
            '"' => {
                if in_quotes {
                    if !current.is_empty() {
                        aliases.push(std::mem::take(&mut current));
                    }
                    in_quotes = false;
                } else {
                    if !current.trim().is_empty() {
                        aliases.extend(current.split_whitespace().map(str::to_owned));
                        current.clear();
                    }
                    in_quotes = true;
                }
            }
            character if character.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    aliases.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }

    if !current.trim().is_empty() {
        if in_quotes {
            aliases.push(current.trim().to_owned());
        } else {
            aliases.extend(current.split_whitespace().map(str::to_owned));
        }
    }

    unique_strings(aliases)
}

fn unique_strings(values: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !value.is_empty() && !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

#[derive(Debug, Clone, Default)]
struct NodeProperties {
    explicit_id: Option<String>,
    aliases: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct HeadingMetadata {
    explicit_id: Option<String>,
    aliases: Vec<String>,
    scheduled_for: Option<String>,
    deadline_for: Option<String>,
    closed_at: Option<String>,
}

fn parse_planning_line(line: &str, metadata: &mut HeadingMetadata) -> bool {
    let mut matched = false;

    if let Some(timestamp) = extract_planning_timestamp(line, "SCHEDULED:") {
        metadata.scheduled_for = Some(timestamp);
        matched = true;
    }
    if let Some(timestamp) = extract_planning_timestamp(line, "DEADLINE:") {
        metadata.deadline_for = Some(timestamp);
        matched = true;
    }
    if let Some(timestamp) = extract_planning_timestamp(line, "CLOSED:") {
        metadata.closed_at = Some(timestamp);
        matched = true;
    }

    matched
}

fn extract_planning_timestamp(line: &str, keyword: &str) -> Option<String> {
    let position = line.find(keyword)?;
    let value = line[position + keyword.len()..].trim_start();
    let closing = if value.starts_with('<') {
        '>'
    } else if value.starts_with('[') {
        ']'
    } else {
        return None;
    };
    let end = value.find(closing)?;
    parse_org_timestamp(&value[1..end])
}

fn parse_org_timestamp(value: &str) -> Option<String> {
    let date = value.split_whitespace().next()?;
    if date.len() != 10
        || !date
            .chars()
            .enumerate()
            .all(|(index, character)| match index {
                4 | 7 => character == '-',
                _ => character.is_ascii_digit(),
            })
    {
        return None;
    }

    Some(format!("{date}T00:00:00"))
}

fn extract_id_links(line: &str, source_node_key: &str, links: &mut Vec<IndexedLink>) {
    let mut rest = line;
    while let Some(start) = rest.find("[[id:") {
        let suffix = &rest[start + 5..];
        let Some(end) = suffix.find("]]") else {
            break;
        };

        let destination_id = suffix[..end].trim();
        if !destination_id.is_empty() {
            links.push(IndexedLink {
                source_node_key: source_node_key.to_owned(),
                destination_explicit_id: destination_id.to_owned(),
            });
        }
        rest = &suffix[end + 2..];
    }
}
