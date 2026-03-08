mod blocks;
mod outline;
mod properties;

use anyhow::{Context, Result, bail};
use slipbox_core::{NodeKind, NodeRecord};
use uuid::Uuid;

use crate::path::default_capture_file_title;

pub(crate) use blocks::render_lines;
pub(crate) use outline::{heading_level, shift_subtree_levels};
pub(crate) use properties::{format_property_values, keyword_value, property_value};

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
        let (title_text, tags) = outline::split_heading_tags(heading_text);
        let (_, title) = outline::split_todo_keyword(title_text);
        let title = title.trim();
        if title.is_empty() {
            bail!("subtree heading must include a title");
        }

        let mut lines = vec![format!("#+title: {title}")];
        if !tags.is_empty() {
            lines.push(format!("#+filetags: {}", outline::format_colon_tags(&tags)));
        }
        let mut remainder = subtree_lines[1..].to_vec();
        outline::promote_subtree_lines(&mut remainder);
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
        outline::demote_all_headings(&mut self.lines);
        self.lines
            .insert(0, outline::format_heading_line(1, &title, &tags));
    }

    pub(crate) fn promote_entire_file(&mut self) -> Result<()> {
        if !self.buffer_promoteable_p() {
            bail!("cannot promote: multiple root headings or there is extra file-level text");
        }

        let heading = self.lines.remove(0);
        let heading_text = heading.trim_start();
        let (title_text, tags) = outline::split_heading_tags(heading_text[2..].trim());
        let (_, title) = outline::split_todo_keyword(title_text);
        let title = title.trim();
        if title.is_empty() {
            bail!("cannot promote: top-level heading must have a title");
        }

        outline::promote_subtree_lines(&mut self.lines);
        self.set_file_keyword("title", Some(title.to_owned()));
        self.set_file_keyword("filetags", keyword_value(&tags));
        Ok(())
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
                properties::ensure_heading_property_value(
                    &mut lines,
                    0,
                    "ID",
                    Some(explicit_id.clone()),
                )?;
                Ok((lines, explicit_id))
            }
            NodeKind::File => {
                let mut lines = self.lines.clone();
                properties::remove_file_keyword(&mut lines, "title");
                properties::remove_file_keyword(&mut lines, "filetags");
                outline::demote_all_headings(&mut lines);
                lines.insert(0, outline::format_heading_line(1, &node.title, &node.tags));

                let explicit_id = node
                    .explicit_id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                properties::ensure_heading_property_value(
                    &mut lines,
                    0,
                    "ID",
                    Some(explicit_id.clone()),
                )?;
                Ok((lines, explicit_id))
            }
        }
    }
}
