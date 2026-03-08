use anyhow::{Context, Result, bail};
use slipbox_core::{NodeKind, NodeRecord};

use super::OrgDocument;
use super::properties::{file_property_drawer_bounds, file_property_insert_index};

impl OrgDocument {
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

    pub(super) fn heading_index(&self, line_number: usize) -> Result<usize> {
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

    pub(super) fn buffer_promoteable_p(&self) -> bool {
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

pub(super) fn split_heading_tags(input: &str) -> (&str, Vec<String>) {
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

pub(super) fn split_todo_keyword(input: &str) -> (Option<String>, &str) {
    let Some((first, rest)) = input.split_once(' ') else {
        return (None, input);
    };

    if looks_like_todo_keyword(first) {
        (Some(first.to_owned()), rest.trim_start())
    } else {
        (None, input)
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

pub(super) fn parse_colon_tags(input: &str) -> Vec<String> {
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

pub(super) fn promote_subtree_lines(lines: &mut [String]) {
    for line in lines {
        if let Some(level) = heading_level(line) {
            let trimmed = line.trim_start();
            let new_level = level.saturating_sub(1).max(1);
            *line = format!("{}{}", "*".repeat(new_level), &trimmed[level..]);
        }
    }
}

pub(super) fn demote_all_headings(lines: &mut [String]) {
    for line in lines {
        if let Some(level) = heading_level(line) {
            let trimmed = line.trim_start();
            *line = format!("{}{}", "*".repeat(level + 1), &trimmed[level..]);
        }
    }
}

pub(super) fn format_heading_line(level: usize, title: &str, tags: &[String]) -> String {
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

pub(super) fn format_colon_tags(values: &[String]) -> String {
    format!(":{}:", values.join(":"))
}
