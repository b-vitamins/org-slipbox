mod discovery;

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, anyhow};
use slipbox_core::{
    IndexedFile, IndexedLink, IndexedNode, IndexedOccurrenceDocument, NodeKind, normalize_reference,
};

pub use discovery::DiscoveryPolicy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineNode {
    pub node_key: String,
    pub line: u32,
    pub level: u32,
    pub kind: NodeKind,
    pub excluded: bool,
}

pub fn scan_root(root: &Path) -> Result<Vec<IndexedFile>> {
    scan_root_with_policy(root, &DiscoveryPolicy::default())
}

pub fn scan_root_with_policy(root: &Path, policy: &DiscoveryPolicy) -> Result<Vec<IndexedFile>> {
    policy
        .list_files(root)?
        .into_iter()
        .map(|path| parse_path(root, &path))
        .collect()
}

pub fn scan_path(root: &Path, path: &Path) -> Result<IndexedFile> {
    scan_path_with_policy(root, path, &DiscoveryPolicy::default())
}

pub fn scan_path_with_policy(
    root: &Path,
    path: &Path,
    policy: &DiscoveryPolicy,
) -> Result<IndexedFile> {
    if !policy.matches_path(root, path) {
        return Err(anyhow!(
            "{} is excluded by the current discovery policy",
            path.display()
        ));
    }
    parse_path(root, path)
}

pub fn scan_source(file_path: &str, source: &str) -> IndexedFile {
    parse_document(file_path, 0, source)
}

pub fn scan_source_outline(file_path: &str, source: &str) -> Vec<OutlineNode> {
    let lines = source.lines().collect::<Vec<_>>();
    parse_outline_nodes(file_path, &lines)
}

fn parse_path(root: &Path, path: &Path) -> Result<IndexedFile> {
    let source = read_source(path)?;
    let file_path = discovery::relative_path(root, path)
        .with_context(|| format!("{} is not under {}", path.display(), root.display()))?;
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
    let mut excluded_explicit_ids = HashSet::new();
    let file_excluded = file_properties.roam_exclude.resolve(false);
    let file_explicit_id = file_properties.explicit_id.clone();
    let mut nodes = Vec::new();
    if file_excluded {
        if let Some(explicit_id) = &file_explicit_id {
            excluded_explicit_ids.insert(explicit_id.clone());
        }
    } else {
        nodes.push(IndexedNode {
            node_key: file_node_key.clone(),
            explicit_id: file_explicit_id,
            file_path: file_path.to_owned(),
            title: file_title.clone(),
            outline_path: String::new(),
            aliases: file_properties.aliases,
            tags: file_tags.clone(),
            refs: file_properties.refs,
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            level: 0,
            line: 1,
            kind: NodeKind::File,
        });
    }
    let mut links = Vec::new();
    let mut occurrence_line_rows = Vec::new();
    let mut occurrence_search_lines = Vec::new();
    let mut current_source_node_key = (!file_excluded).then_some(file_node_key);
    let mut outline_stack: Vec<String> = Vec::new();
    let mut exclusion_stack: Vec<bool> = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if let Some((level, todo_keyword, title, heading_tags)) = parse_heading(line) {
            outline_stack.truncate(level.saturating_sub(1));
            exclusion_stack.truncate(level.saturating_sub(1));
            outline_stack.push(title.clone());

            let node_key = format!("heading:{file_path}:{}", index + 1);
            let heading_metadata = parse_heading_metadata(&lines, index + 1);
            let inherited_exclusion = exclusion_stack.last().copied().unwrap_or(file_excluded);
            let heading_excluded = heading_metadata.roam_exclude.resolve(inherited_exclusion);
            exclusion_stack.push(heading_excluded);

            if heading_excluded {
                if let Some(explicit_id) = &heading_metadata.explicit_id {
                    excluded_explicit_ids.insert(explicit_id.clone());
                }
                current_source_node_key = None;
            } else {
                current_source_node_key = Some(node_key.clone());
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
                    refs: heading_metadata.refs,
                    todo_keyword,
                    scheduled_for: heading_metadata.scheduled_for,
                    deadline_for: heading_metadata.deadline_for,
                    closed_at: heading_metadata.closed_at,
                    level: level as u32,
                    line: (index + 1) as u32,
                    kind: NodeKind::Heading,
                });
            }
        }

        if let Some(source_node_key) = current_source_node_key.as_deref() {
            if !line.trim().is_empty() {
                occurrence_line_rows.push((index + 1) as u32);
                occurrence_search_lines.push((*line).to_owned());
            }
            extract_id_links(line, source_node_key, (index + 1) as u32, &mut links);
        }
    }

    if !excluded_explicit_ids.is_empty() {
        links.retain(|link| !excluded_explicit_ids.contains(&link.destination_explicit_id));
    }

    let occurrence_document = if occurrence_search_lines.is_empty() {
        None
    } else {
        Some(IndexedOccurrenceDocument {
            file_path: file_path.to_owned(),
            search_text: occurrence_search_lines.join("\n"),
            line_rows: occurrence_line_rows,
        })
    };

    IndexedFile {
        file_path: file_path.to_owned(),
        title: file_title,
        mtime_ns,
        nodes,
        links,
        occurrence_document,
    }
}

fn parse_outline_nodes(file_path: &str, lines: &[&str]) -> Vec<OutlineNode> {
    let file_excluded = parse_file_properties(lines).roam_exclude.resolve(false);
    let mut outline_nodes = vec![OutlineNode {
        node_key: format!("file:{file_path}"),
        line: 1,
        level: 0,
        kind: NodeKind::File,
        excluded: file_excluded,
    }];
    let mut exclusion_stack = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if let Some((level, _, _, _)) = parse_heading(line) {
            exclusion_stack.truncate(level.saturating_sub(1));
            let inherited_exclusion = exclusion_stack.last().copied().unwrap_or(file_excluded);
            let heading_excluded =
                parse_heading_exclusion(lines, index + 1).resolve(inherited_exclusion);
            exclusion_stack.push(heading_excluded);
            outline_nodes.push(OutlineNode {
                node_key: format!("heading:{file_path}:{}", index + 1),
                line: (index + 1) as u32,
                level: level as u32,
                kind: NodeKind::Heading,
                excluded: heading_excluded,
            });
        }
    }

    outline_nodes
}

pub fn read_source(path: &Path) -> Result<String> {
    match discovery::envelope_extension(path).as_deref() {
        Some("gpg") => read_encrypted_source(path, "gpg", &["--quiet", "--batch", "--decrypt"]),
        Some("age") => read_age_source(path),
        _ => fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn read_age_source(path: &Path) -> Result<String> {
    read_encrypted_source(path, "age", &["--decrypt"])
        .or_else(|_| read_encrypted_source(path, "rage", &["--decrypt"]))
        .with_context(|| format!("failed to decrypt {}", path.display()))
}

fn read_encrypted_source(path: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .arg(path)
        .output()
        .with_context(|| format!("failed to execute {program} for {}", path.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("{program} exited with {}", output.status)
        } else {
            stderr
        };
        return Err(anyhow!("failed to decrypt {}: {}", path.display(), message));
    }
    String::from_utf8(output.stdout)
        .with_context(|| format!("failed to decode decrypted output from {}", path.display()))
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
    discovery::default_file_stem(Path::new(file_path)).unwrap_or_else(|| file_path.to_owned())
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
        if let Some(value) = strip_keyword(trimmed, ":ROAM_REFS:") {
            properties.refs = parse_refs(value);
        }
        if let Some(value) = strip_keyword(trimmed, ":ROAM_EXCLUDE:") {
            properties.roam_exclude = parse_roam_exclude(value);
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
            if let Some(value) = strip_keyword(trimmed, ":ROAM_REFS:") {
                metadata.refs = parse_refs(value);
            }
            if let Some(value) = strip_keyword(trimmed, ":ROAM_EXCLUDE:") {
                metadata.roam_exclude = parse_roam_exclude(value);
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

fn parse_heading_exclusion(lines: &[&str], start_index: usize) -> NodeExclusionDirective {
    let mut in_property_drawer = false;

    for line in &lines[start_index..] {
        let trimmed = line.trim();
        if parse_heading(line).is_some() {
            break;
        }

        if in_property_drawer {
            if trimmed.eq_ignore_ascii_case(":END:") {
                break;
            }
            if let Some(value) = strip_keyword(trimmed, ":ROAM_EXCLUDE:") {
                return parse_roam_exclude(value);
            }
            continue;
        }

        if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
            in_property_drawer = true;
            continue;
        }

        if trimmed.is_empty() || is_planning_line(trimmed) {
            continue;
        }

        break;
    }

    NodeExclusionDirective::Inherit
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
    unique_strings(parse_quoted_values(input))
}

fn parse_refs(input: &str) -> Vec<String> {
    unique_strings(
        parse_quoted_values(input)
            .into_iter()
            .flat_map(|value| normalize_reference(&value))
            .collect(),
    )
}

fn parse_roam_exclude(input: &str) -> NodeExclusionDirective {
    let value = input.trim();
    if value.eq_ignore_ascii_case("nil") {
        NodeExclusionDirective::Include
    } else {
        NodeExclusionDirective::Exclude
    }
}

fn parse_quoted_values(input: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut bracket_depth = 0_usize;

    for character in input.chars() {
        match character {
            '"' => {
                if in_quotes {
                    if !current.is_empty() {
                        values.push(std::mem::take(&mut current));
                    }
                    in_quotes = false;
                } else {
                    if !current.trim().is_empty() {
                        values.extend(current.split_whitespace().map(str::to_owned));
                        current.clear();
                    }
                    in_quotes = true;
                }
            }
            '[' if !in_quotes => {
                bracket_depth += 1;
                current.push(character);
            }
            ']' if !in_quotes => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(character);
            }
            character if character.is_whitespace() && !in_quotes && bracket_depth == 0 => {
                if !current.is_empty() {
                    values.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }

    if !current.trim().is_empty() {
        values.push(current.trim().to_owned());
    }

    values
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
    refs: Vec<String>,
    roam_exclude: NodeExclusionDirective,
}

#[derive(Debug, Clone, Default)]
struct HeadingMetadata {
    explicit_id: Option<String>,
    aliases: Vec<String>,
    refs: Vec<String>,
    roam_exclude: NodeExclusionDirective,
    scheduled_for: Option<String>,
    deadline_for: Option<String>,
    closed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
enum NodeExclusionDirective {
    #[default]
    Inherit,
    Exclude,
    Include,
}

impl NodeExclusionDirective {
    fn resolve(self, inherited: bool) -> bool {
        match self {
            Self::Inherit => inherited,
            Self::Exclude => true,
            Self::Include => false,
        }
    }
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

fn is_planning_line(line: &str) -> bool {
    ["SCHEDULED:", "DEADLINE:", "CLOSED:"]
        .iter()
        .any(|keyword| extract_planning_timestamp(line, keyword).is_some())
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

fn extract_id_links(line: &str, source_node_key: &str, row: u32, links: &mut Vec<IndexedLink>) {
    let mut offset = 0_usize;
    while let Some(relative_start) = line[offset..].find("[[") {
        let start = offset + relative_start;
        let suffix = &line[start + 2..];
        let Some(end) = suffix.find("]]") else {
            break;
        };

        let inner = &suffix[..end];
        let target = inner
            .split_once("][")
            .map_or(inner, |(path, _)| path)
            .trim();
        if let Some(destination_id) = target.strip_prefix("id:").map(str::trim)
            && !destination_id.is_empty()
        {
            links.push(IndexedLink {
                source_node_key: source_node_key.to_owned(),
                destination_explicit_id: destination_id.to_owned(),
                line: row,
                column: column_number(line, start),
                preview: preview_snippet(line),
            });
        }
        offset = start + 2 + end + 2;
    }
}

fn column_number(line: &str, byte_offset: usize) -> u32 {
    line[..byte_offset].chars().count() as u32 + 1
}

fn preview_snippet(line: &str) -> String {
    line.trim().to_owned()
}
