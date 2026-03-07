use anyhow::{Context, Result, bail};
use slipbox_core::{NodeKind, NodeRecord};
use uuid::Uuid;

use crate::path::default_capture_file_title;

pub(crate) struct OrgDocument {
    pub(crate) lines: Vec<String>,
    had_trailing_newline: bool,
}

impl OrgDocument {
    pub(crate) fn from_source(source: &str) -> Self {
        Self {
            lines: source.lines().map(ToOwned::to_owned).collect(),
            had_trailing_newline: source.ends_with('\n'),
        }
    }

    pub(crate) fn from_extracted_subtree(
        subtree_lines: &[String],
        explicit_id: &str,
    ) -> Result<Self> {
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

    pub(crate) fn render(&self) -> String {
        if self.lines.is_empty() {
            String::new()
        } else {
            render_lines(&self.lines, self.had_trailing_newline)
        }
    }

    pub(crate) fn has_meaningful_content(&self) -> bool {
        self.lines.iter().any(|line| !line.trim().is_empty())
    }

    pub(crate) fn demote_entire_file(&mut self, relative_path: &str) {
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

    pub(crate) fn promote_entire_file(&mut self) -> Result<()> {
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

    pub(crate) fn subtree_range(&self, line_number: usize) -> Result<(usize, usize)> {
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

    pub(crate) fn insertion_index(&self, target: &NodeRecord) -> Result<usize> {
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

    pub(crate) fn file_entry_insert_index(&self, prepend: bool) -> usize {
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

    pub(crate) fn heading_entry_insert_index(
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

    pub(crate) fn file_body_bounds(&self) -> (usize, usize) {
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

    pub(crate) fn heading_body_bounds(
        &self,
        line_number: usize,
        level: usize,
    ) -> Result<(usize, usize)> {
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

    pub(crate) fn insert_block(
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

    pub(crate) fn remove_range(&mut self, start: usize, end: usize) {
        self.lines.drain(start..end);
    }

    pub(crate) fn insert_subtree(
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

    pub(crate) fn subtree_lines(&self, node: &NodeRecord) -> Result<(Vec<String>, String)> {
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

    pub(crate) fn set_file_property(&mut self, property: &str, value: Option<String>) {
        set_file_property_value(&mut self.lines, property, value);
    }

    pub(crate) fn set_heading_property(
        &mut self,
        line_number: usize,
        property: &str,
        value: Option<String>,
    ) -> Result<()> {
        let heading_index = self.heading_index(line_number)?;
        ensure_heading_property_value(&mut self.lines, heading_index, property, value)
    }

    pub(crate) fn set_file_keyword(&mut self, keyword: &str, value: Option<String>) {
        set_file_keyword_value(&mut self.lines, keyword, value);
    }

    pub(crate) fn set_heading_tags(&mut self, line_number: usize, tags: &[String]) -> Result<()> {
        let index = self.heading_index(line_number)?;
        let level = heading_level(&self.lines[index]).context("heading line is invalid")?;
        let trimmed = self.lines[index].trim_start();
        let heading_text = trimmed[level + 1..].trim();
        let (title, _) = split_heading_tags(heading_text);
        self.lines[index] = format_heading_line(level, title, tags);
        Ok(())
    }

    pub(crate) fn ensure_file_identity(&mut self) -> Result<()> {
        self.ensure_file_identity_with_refs(&[])
    }

    pub(crate) fn ensure_file_identity_with_refs(&mut self, refs: &[String]) -> Result<()> {
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

    pub(crate) fn ensure_outline_path(
        &mut self,
        outline_path: &[String],
    ) -> Result<Option<(usize, usize)>> {
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

    pub(crate) fn file_keyword_value(&self, keyword: &str) -> Option<String> {
        file_keyword_value(&self.lines, keyword)
    }

    fn filetags(&self) -> Vec<String> {
        self.file_keyword_value("filetags")
            .map(|value| parse_colon_tags(&value))
            .unwrap_or_default()
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

pub(crate) fn property_value(values: &[String]) -> Option<String> {
    if values.is_empty() {
        None
    } else {
        Some(format_property_values(values))
    }
}

pub(crate) fn keyword_value(values: &[String]) -> Option<String> {
    if values.is_empty() {
        None
    } else {
        Some(format_colon_tags(values))
    }
}

pub(crate) fn format_property_values(values: &[String]) -> String {
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

pub(crate) fn render_lines(lines: &[String], had_trailing_newline: bool) -> String {
    let mut rendered = lines.join("\n");
    if (had_trailing_newline || !rendered.ends_with('\n')) && !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered
}

pub(crate) fn heading_level(line: &str) -> Option<usize> {
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

pub(crate) fn shift_subtree_levels(lines: &mut [String], desired_root_level: usize) -> Result<()> {
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

fn format_colon_tags(values: &[String]) -> String {
    format!(":{}:", values.join(":"))
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
