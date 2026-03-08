use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use slipbox_core::{CaptureContentType, NodeKind, NodeRecord};
use uuid::Uuid;

use crate::CaptureOutcome;
use crate::capture_pipeline::{CaptureTargetSelection, capture_target_node_key};
pub use crate::capture_pipeline::{capture_template, preview_capture_template};
use crate::document::{OrgDocument, format_property_values, heading_level, render_lines};
use crate::path::{
    next_available_path, next_available_relative_path, normalize_relative_org_path,
    normalized_head_source, normalized_title, slugify,
};

#[derive(Clone, Debug)]
struct ListContext {
    start: usize,
    end: usize,
    indent: usize,
    style: ListStyle,
}

#[derive(Clone, Debug)]
enum ListStyle {
    Unordered {
        bullet: char,
    },
    Ordered {
        start: OrderedMarkerValue,
        delimiter: char,
    },
}

#[derive(Clone, Debug)]
enum OrderedMarkerValue {
    Numeric(usize),
    Alpha { codepoint: u32, uppercase: bool },
}

#[derive(Clone, Debug)]
struct TableContext {
    start: usize,
    end: usize,
    hlines: Vec<usize>,
    data_lines: Vec<usize>,
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

pub(crate) fn capture_entry(
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
    let line_number =
        document.insert_block(insert_index, block, empty_lines_before, empty_lines_after);

    Ok(format!(
        "heading:{}:{line_number}",
        capture_target_relative_path(target).replace('\\', "/")
    ))
}

pub(crate) fn capture_plain(
    document: &mut OrgDocument,
    target: &CaptureTargetSelection,
    content: &str,
    prepend: bool,
    empty_lines_before: usize,
    empty_lines_after: usize,
) -> Result<String> {
    let block = plain_capture_lines(content);
    if block.is_empty() {
        return Ok(capture_target_node_key(target));
    }

    let index = match target {
        CaptureTargetSelection::File { .. } => {
            if prepend {
                file_plain_prepend_index(document)
            } else {
                document.lines.len()
            }
        }
        CaptureTargetSelection::Heading {
            line_number, level, ..
        } => {
            let (body_start, body_end) = document.heading_body_bounds(*line_number, *level)?;
            if prepend { body_start } else { body_end }
        }
    };
    document.insert_block(index, block, empty_lines_before, empty_lines_after);
    Ok(capture_target_node_key(target))
}

pub(crate) fn capture_list_item(
    document: &mut OrgDocument,
    target: &CaptureTargetSelection,
    content: &str,
    capture_type: CaptureContentType,
    prepend: bool,
    empty_lines_before: usize,
    empty_lines_after: usize,
) -> Result<String> {
    let (search_start, search_end, fallback_index) = list_search_bounds(document, target, prepend)?;
    let list_context = find_list_context(&document.lines, search_start, search_end);
    let (index, blank_lines_before, blank_lines_after) = if let Some(list) = &list_context {
        (
            if prepend { list.start } else { list.end },
            if prepend {
                0
            } else {
                empty_lines_before.min(1)
            },
            if prepend { empty_lines_after.min(1) } else { 0 },
        )
    } else {
        (fallback_index, empty_lines_before, empty_lines_after)
    };
    let block = list_capture_lines(content, capture_type, list_context.as_ref())?;
    document.insert_block(index, block, blank_lines_before, blank_lines_after);

    if let Some(ListContext {
        start,
        style:
            ListStyle::Ordered {
                start: ordered_start,
                delimiter,
            },
        ..
    }) = list_context
    {
        renumber_ordered_list(document, start.min(index), ordered_start, delimiter);
    }

    Ok(capture_target_node_key(target))
}

pub(crate) fn capture_table_line(
    document: &mut OrgDocument,
    target: &CaptureTargetSelection,
    content: &str,
    prepend: bool,
    table_line_pos: Option<&str>,
) -> Result<String> {
    let (search_start, search_end) = table_search_bounds(document, target)?;
    let table_context =
        if let Some(existing) = find_table_context(&document.lines, search_start, search_end) {
            existing
        } else {
            document.insert_block(
                search_end,
                vec![String::from("|   |"), String::from("|---|")],
                0,
                0,
            );
            find_table_context(&document.lines, search_end, document.lines.len())
                .context("failed to prepare capture table")?
        };
    let index = table_insertion_index(&table_context, prepend, table_line_pos)?;
    let line = table_capture_line(content);
    document.insert_block(index, vec![line], 0, 0);
    Ok(capture_target_node_key(target))
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

fn plain_capture_lines(content: &str) -> Vec<String> {
    trimmed_capture_lines(content)
}

fn file_plain_prepend_index(document: &OrgDocument) -> usize {
    let lines = &document.lines;
    let mut index = 0usize;

    loop {
        while index < lines.len() && lines[index].trim().is_empty() {
            index += 1;
        }
        if index >= lines.len() {
            return index;
        }

        let trimmed = lines[index].trim_start();
        if trimmed.starts_with("#+begin_comment") || trimmed.starts_with("#+BEGIN_COMMENT") {
            index += 1;
            while index < lines.len()
                && !lines[index].trim_start().starts_with("#+end_comment")
                && !lines[index].trim_start().starts_with("#+END_COMMENT")
            {
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            continue;
        }
        if trimmed.starts_with("#+") || is_org_comment_line(trimmed) || is_horizontal_rule(trimmed)
        {
            index += 1;
            continue;
        }
        if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
            index += 1;
            while index < lines.len() && !lines[index].trim().eq_ignore_ascii_case(":END:") {
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            continue;
        }
        return index;
    }
}

fn list_search_bounds(
    document: &OrgDocument,
    target: &CaptureTargetSelection,
    prepend: bool,
) -> Result<(usize, usize, usize)> {
    match target {
        CaptureTargetSelection::File { .. } => Ok((
            0,
            document.lines.len(),
            if prepend { 0 } else { document.lines.len() },
        )),
        CaptureTargetSelection::Heading {
            line_number, level, ..
        } => {
            let (body_start, body_end) = document.heading_body_bounds(*line_number, *level)?;
            Ok((
                body_start,
                body_end,
                if prepend { body_start } else { body_end },
            ))
        }
    }
}

fn table_search_bounds(
    document: &OrgDocument,
    target: &CaptureTargetSelection,
) -> Result<(usize, usize)> {
    match target {
        CaptureTargetSelection::File { .. } => Ok((0, document.lines.len())),
        CaptureTargetSelection::Heading {
            line_number, level, ..
        } => document.heading_body_bounds(*line_number, *level),
    }
}

fn find_list_context(lines: &[String], start: usize, end: usize) -> Option<ListContext> {
    (start..end).find_map(|index| {
        let (indent, style, _, _) = parse_list_item_prefix(lines.get(index)?)?;
        let mut end_index = end;
        for cursor in index + 1..end {
            let line = lines.get(cursor)?;
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                end_index = cursor;
                break;
            }
            if parse_list_item_prefix(line).is_some()
                || leading_spaces(line) > indent
                || line.starts_with('\t')
            {
                continue;
            }
            end_index = cursor;
            break;
        }
        Some(ListContext {
            start: index,
            end: end_index,
            indent,
            style,
        })
    })
}

fn parse_list_item_prefix(line: &str) -> Option<(usize, ListStyle, bool, usize)> {
    let indent = leading_spaces(line);
    let trimmed = line.get(indent..)?.trim_end_matches('\r');
    let mut chars = trimmed.chars();
    let first = chars.next()?;
    match first {
        '-' | '+' | '*' => {
            if first == '*' && indent == 0 {
                return None;
            }
            let rest = trimmed.get(1..)?;
            if !rest.starts_with(' ') {
                return None;
            }
            let rest = rest.trim_start_matches(' ');
            let (checkbox, consumed) = parse_checkbox_prefix(rest);
            Some((
                indent,
                ListStyle::Unordered { bullet: first },
                checkbox,
                indent + 1 + 1 + consumed,
            ))
        }
        '0'..='9' | 'A'..='Z' | 'a'..='z' => {
            let mut marker_len = 1usize;
            if first.is_ascii_digit() {
                marker_len = trimmed
                    .chars()
                    .take_while(|character| character.is_ascii_digit())
                    .count();
            }
            let delimiter = trimmed.chars().nth(marker_len)?;
            if !matches!(delimiter, '.' | ')') {
                return None;
            }
            let spacing = trimmed.chars().nth(marker_len + 1)?;
            if !spacing.is_whitespace() {
                return None;
            }
            let marker = &trimmed[..marker_len];
            let rest = trimmed.get(marker_len + 2..)?;
            let (checkbox, consumed) = parse_checkbox_prefix(rest);
            let style = if first.is_ascii_digit() {
                ListStyle::Ordered {
                    start: OrderedMarkerValue::Numeric(marker.parse().ok()?),
                    delimiter,
                }
            } else {
                ListStyle::Ordered {
                    start: OrderedMarkerValue::Alpha {
                        codepoint: first.to_ascii_lowercase() as u32,
                        uppercase: first.is_ascii_uppercase(),
                    },
                    delimiter,
                }
            };
            Some((indent, style, checkbox, indent + marker_len + 2 + consumed))
        }
        _ => None,
    }
}

fn parse_checkbox_prefix(text: &str) -> (bool, usize) {
    let bytes = text.as_bytes();
    if bytes.len() >= 4 && bytes[0] == b'[' && bytes[2] == b']' && bytes[3] == b' ' {
        return (true, 4);
    }
    if bytes.len() >= 3 && bytes[0] == b'[' && bytes[2] == b']' {
        return (true, 3);
    }
    (false, 0)
}

fn list_capture_lines(
    content: &str,
    capture_type: CaptureContentType,
    existing: Option<&ListContext>,
) -> Result<Vec<String>> {
    let mut lines = trimmed_capture_lines(content);
    if lines.is_empty() {
        lines.push(String::new());
    }
    if let Some(first) = lines.first_mut() {
        *first = strip_list_marker(first);
    }

    let (marker, indent) = match existing {
        Some(ListContext { indent, style, .. }) => {
            (list_marker_prefix(style, capture_type), *indent)
        }
        None => (
            match capture_type {
                CaptureContentType::Checkitem => String::from("- [ ]"),
                CaptureContentType::Item => String::from("-"),
                _ => bail!("unsupported list capture type"),
            },
            0,
        ),
    };
    let continuation_width = indent + marker.chars().count() + 1;
    let mut formatted = Vec::with_capacity(lines.len());
    let first_text = lines.remove(0);
    formatted.push(format_list_line(indent, &marker, &first_text));
    for line in lines {
        formatted.push(format!("{}{}", " ".repeat(continuation_width), line));
    }
    Ok(formatted)
}

fn list_marker_prefix(style: &ListStyle, capture_type: CaptureContentType) -> String {
    match style {
        ListStyle::Unordered { bullet } => match capture_type {
            CaptureContentType::Checkitem => format!("{bullet} [ ]"),
            _ => bullet.to_string(),
        },
        ListStyle::Ordered {
            start, delimiter, ..
        } => format!(
            "{}{}{}",
            ordered_marker_text(start),
            delimiter,
            if matches!(capture_type, CaptureContentType::Checkitem) {
                " [ ]"
            } else {
                ""
            }
        ),
    }
}

fn ordered_marker_text(value: &OrderedMarkerValue) -> String {
    match value {
        OrderedMarkerValue::Numeric(number) => number.to_string(),
        OrderedMarkerValue::Alpha {
            codepoint,
            uppercase,
        } => char::from_u32(*codepoint)
            .map(|character| {
                if *uppercase {
                    character.to_ascii_uppercase()
                } else {
                    character
                }
            })
            .unwrap_or('a')
            .to_string(),
    }
}

fn format_list_line(indent: usize, marker: &str, text: &str) -> String {
    if text.is_empty() {
        format!("{}{} ", " ".repeat(indent), marker)
    } else {
        format!("{}{} {}", " ".repeat(indent), marker, text)
    }
}

fn strip_list_marker(line: &str) -> String {
    if let Some((_, _, _, content_offset)) = parse_list_item_prefix(line) {
        return line
            .get(content_offset..)
            .unwrap_or("")
            .trim_start()
            .to_owned();
    }
    line.trim_start().to_owned()
}

fn renumber_ordered_list(
    document: &mut OrgDocument,
    start_index: usize,
    ordered_start: OrderedMarkerValue,
    delimiter: char,
) {
    let Some(ListContext {
        start,
        end,
        indent,
        style: ListStyle::Ordered { .. },
    }) = find_list_context(&document.lines, start_index, document.lines.len())
    else {
        return;
    };
    let mut current = ordered_start;
    for index in start..end {
        let Some((line_indent, ListStyle::Ordered { .. }, checkbox, content_offset)) =
            parse_list_item_prefix(&document.lines[index])
        else {
            continue;
        };
        if line_indent != indent {
            continue;
        }
        let text = document.lines[index]
            .get(content_offset..)
            .unwrap_or("")
            .trim_start()
            .to_owned();
        let marker = ordered_marker_text(&current);
        let prefix = if checkbox {
            format!("{marker}{delimiter} [ ]")
        } else {
            format!("{marker}{delimiter}")
        };
        document.lines[index] = format_list_line(indent, &prefix, &text);
        advance_ordered_marker(&mut current);
    }
}

fn advance_ordered_marker(value: &mut OrderedMarkerValue) {
    match value {
        OrderedMarkerValue::Numeric(number) => *number += 1,
        OrderedMarkerValue::Alpha { codepoint, .. } => *codepoint += 1,
    }
}

fn find_table_context(lines: &[String], start: usize, end: usize) -> Option<TableContext> {
    let mut table_start = None;
    for index in start..end {
        if lines.get(index)?.trim_start().starts_with('|') {
            table_start = Some(index);
            break;
        }
    }
    let start = table_start?;
    let mut end_index = end;
    let mut hlines = Vec::new();
    let mut data_lines = Vec::new();
    for index in start..end {
        let line = lines.get(index)?;
        if !line.trim_start().starts_with('|') {
            end_index = index;
            break;
        }
        if is_table_hline(line) {
            hlines.push(index);
        } else {
            data_lines.push(index);
        }
    }
    Some(TableContext {
        start,
        end: end_index,
        hlines,
        data_lines,
    })
}

fn table_insertion_index(
    table: &TableContext,
    prepend: bool,
    table_line_pos: Option<&str>,
) -> Result<usize> {
    if let Some(spec) = table_line_pos {
        return table_line_position_index(table, spec);
    }
    if !prepend {
        return Ok(table.end);
    }
    if let Some(first_hline) = table.hlines.first() {
        if let Some(first_data_after) = table
            .data_lines
            .iter()
            .copied()
            .find(|line| *line > *first_hline)
        {
            return Ok(first_data_after);
        }
        return Ok(table.end);
    }
    Ok(table.start)
}

fn table_line_position_index(table: &TableContext, spec: &str) -> Result<usize> {
    let hline_count = spec
        .chars()
        .take_while(|character| *character == 'I')
        .count();
    if hline_count == 0 {
        bail!("invalid table line specification {spec:?}");
    }
    let delta = spec
        .get(hline_count..)
        .context("invalid table line specification")?
        .parse::<isize>()
        .with_context(|| format!("invalid table line specification {spec:?}"))?;
    let hline = table
        .hlines
        .get(hline_count - 1)
        .copied()
        .with_context(|| format!("invalid table line specification {spec:?}"))?;
    let relative = if delta < 0 { delta + 1 } else { delta };
    let index = hline as isize + relative;
    if index < table.start as isize || index > table.end as isize {
        bail!("invalid table line specification {spec:?}");
    }
    Ok(index as usize)
}

fn table_capture_line(content: &str) -> String {
    let line = trimmed_capture_lines(content)
        .into_iter()
        .next()
        .unwrap_or_default();
    if line.trim_start().starts_with('|') {
        line
    } else if line.trim().is_empty() {
        String::from("|  |")
    } else {
        format!("| {} |", line.trim())
    }
}

fn leading_spaces(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn is_org_comment_line(line: &str) -> bool {
    line.starts_with("# ") || line.starts_with("#\t")
}

fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 5 && trimmed.chars().all(|character| character == '-')
}

fn is_table_hline(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(stripped) = trimmed
        .strip_prefix('|')
        .and_then(|value| value.strip_suffix('|'))
    else {
        return false;
    };
    let content = stripped.trim();
    !content.is_empty()
        && content
            .chars()
            .all(|character| matches!(character, '-' | '+' | ':' | ' '))
        && content.contains('-')
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
