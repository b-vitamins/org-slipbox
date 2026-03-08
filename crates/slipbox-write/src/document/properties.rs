use anyhow::{Context, Result, bail};
use uuid::Uuid;

use super::OrgDocument;
use super::outline::{format_colon_tags, format_heading_line, heading_level, parse_colon_tags};

impl OrgDocument {
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
        let (title, _) = super::outline::split_heading_tags(heading_text);
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

    pub(crate) fn file_keyword_value(&self, keyword: &str) -> Option<String> {
        file_keyword_value(&self.lines, keyword)
    }

    pub(crate) fn heading_property_value(
        &self,
        line_number: usize,
        property: &str,
    ) -> Result<Option<String>> {
        let heading_index = self.heading_index(line_number)?;
        Ok(heading_property_value(&self.lines, heading_index, property))
    }

    pub(super) fn filetags(&self) -> Vec<String> {
        self.file_keyword_value("filetags")
            .map(|value| parse_colon_tags(&value))
            .unwrap_or_default()
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

pub(super) fn remove_file_keyword(lines: &mut Vec<String>, keyword: &str) {
    let limit = file_keyword_limit(lines);
    if let Some(index) = (0..limit)
        .find(|index| strip_keyword(lines[*index].trim_start(), &format!("#+{keyword}:")).is_some())
    {
        lines.remove(index);
    }
}

pub(super) fn file_property_insert_index(lines: &[String]) -> usize {
    let mut index = 0;
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }

    while index < lines.len() && lines[index].trim_start().starts_with("#+") {
        index += 1;
    }

    index
}

pub(super) fn file_property_drawer_bounds(lines: &[String]) -> Option<(usize, usize)> {
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

pub(super) fn file_property_value(lines: &[String], property: &str) -> Option<String> {
    let (start, end) = file_property_drawer_bounds(lines)?;
    let property_line = format!(":{property}:");
    (start + 1..end - 1).find_map(|index| {
        strip_keyword(lines[index].trim(), &property_line).map(|value| value.trim().to_owned())
    })
}

pub(super) fn ensure_heading_property_value(
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

fn file_keyword_limit(lines: &[String]) -> usize {
    lines
        .iter()
        .position(|line| heading_level(line).is_some())
        .unwrap_or(lines.len())
}

fn file_keyword_value(lines: &[String], keyword: &str) -> Option<String> {
    let needle = format!("#+{keyword}:");
    (0..file_keyword_limit(lines)).find_map(|index| {
        strip_keyword(lines[index].trim_start(), &needle).map(|value| value.trim().to_owned())
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

fn heading_property_value(
    lines: &[String],
    heading_index: usize,
    property: &str,
) -> Option<String> {
    let (start, end) = heading_property_drawer_bounds(lines, heading_index)?;
    let property_line = format!(":{property}:");
    (start + 1..end - 1)
        .find_map(|index| strip_keyword(lines[index].trim(), &property_line))
        .map(|value| value.trim().to_owned())
}
