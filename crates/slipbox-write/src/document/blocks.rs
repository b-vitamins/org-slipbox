use super::OrgDocument;

impl OrgDocument {
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
        target_kind: slipbox_core::NodeKind,
        mut subtree_lines: Vec<String>,
    ) {
        if matches!(target_kind, slipbox_core::NodeKind::File)
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
}

pub(crate) fn render_lines(lines: &[String], had_trailing_newline: bool) -> String {
    let mut rendered = lines.join("\n");
    if (had_trailing_newline || !rendered.ends_with('\n')) && !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered
}
