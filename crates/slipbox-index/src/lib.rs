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
    let file_id = parse_file_id(&lines);
    let file_node_key = format!("file:{file_path}");
    let mut nodes = vec![IndexedNode {
        node_key: file_node_key.clone(),
        explicit_id: file_id,
        file_path: file_path.to_owned(),
        title: file_title,
        outline_path: String::new(),
        level: 0,
        line: 1,
        kind: NodeKind::File,
    }];
    let mut links = Vec::new();
    let mut current_source_node_key = file_node_key;
    let mut outline_stack: Vec<String> = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if let Some((level, title)) = parse_heading(line) {
            outline_stack.truncate(level.saturating_sub(1));
            outline_stack.push(title.clone());

            let node_key = format!("heading:{file_path}:{}", index + 1);
            current_source_node_key = node_key.clone();
            nodes.push(IndexedNode {
                node_key,
                explicit_id: parse_immediate_id(&lines, index + 1),
                file_path: file_path.to_owned(),
                title,
                outline_path: outline_stack.join(" / "),
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

fn parse_file_id(lines: &[&str]) -> Option<String> {
    let mut index = 0;
    while let Some(line) = lines.get(index) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("#+") {
            index += 1;
            continue;
        }

        if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
            for property_line in &lines[index + 1..] {
                let property = property_line.trim();
                if property.eq_ignore_ascii_case(":END:") {
                    break;
                }
                if let Some(value) = strip_keyword(property, ":ID:") {
                    let id = value.trim();
                    if !id.is_empty() {
                        return Some(id.to_owned());
                    }
                }
            }
        }
        break;
    }

    None
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

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let level = line
        .chars()
        .take_while(|character| *character == '*')
        .count();
    if level == 0 || !line[level..].starts_with(' ') {
        return None;
    }

    let title = line[level + 1..].trim();
    if title.is_empty() {
        None
    } else {
        Some((level, title.to_owned()))
    }
}

fn parse_immediate_id(lines: &[&str], start_index: usize) -> Option<String> {
    let start_line = lines.get(start_index)?.trim();
    if !start_line.eq_ignore_ascii_case(":PROPERTIES:") {
        return None;
    }

    for line in &lines[start_index + 1..] {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case(":END:") {
            break;
        }
        if let Some(value) = strip_keyword(trimmed, ":ID:") {
            let id = value.trim();
            if !id.is_empty() {
                return Some(id.to_owned());
            }
        }
    }

    None
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
