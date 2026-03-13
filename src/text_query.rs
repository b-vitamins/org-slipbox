use slipbox_core::{NodeKind, NodeRecord};

#[derive(Debug, Clone)]
pub(crate) struct NodeRange {
    pub(crate) node: NodeRecord,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
}

pub(crate) fn build_node_ranges(nodes: &[NodeRecord], total_lines: u32) -> Vec<NodeRange> {
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| NodeRange {
            node: node.clone(),
            start_line: node.line.max(1),
            end_line: node_end_line(nodes, index, total_lines),
        })
        .collect()
}

pub(crate) fn node_range_for_key(ranges: &[NodeRange], node_key: &str) -> Option<(u32, u32)> {
    ranges
        .iter()
        .find(|range| range.node.node_key == node_key)
        .map(|range| (range.start_line, range.end_line))
}

pub(crate) fn source_range_for_row<'a>(
    ranges: &'a [NodeRange],
    row: u32,
    source_index: &mut usize,
) -> Option<&'a NodeRange> {
    while *source_index + 1 < ranges.len() && ranges[*source_index + 1].start_line <= row {
        *source_index += 1;
    }

    let mut index = (*source_index).min(ranges.len().saturating_sub(1));
    loop {
        let range = &ranges[index];
        if range.start_line <= row && row <= range.end_line {
            return Some(range);
        }
        if index == 0 {
            return None;
        }
        index -= 1;
    }
}

pub(crate) fn column_number(line: &str, byte_offset: usize) -> u32 {
    line[..byte_offset].chars().count() as u32 + 1
}

pub(crate) fn byte_offset_for_column(line: &str, column: u32) -> Option<usize> {
    if column <= 1 {
        return Some(0);
    }

    line.char_indices()
        .nth(column.saturating_sub(1) as usize)
        .map(|(offset, _)| offset)
}

pub(crate) fn has_phrase_boundaries(line: &str, start: usize, end: usize) -> bool {
    let before = line[..start].chars().next_back();
    let after = line[end..].chars().next();

    !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

fn node_end_line(nodes: &[NodeRecord], index: usize, total_lines: u32) -> u32 {
    let node = &nodes[index];
    if node.kind == NodeKind::File {
        return total_lines.max(node.line);
    }

    for candidate in &nodes[index + 1..] {
        if candidate.line > node.line && candidate.level <= node.level {
            return candidate.line.saturating_sub(1).max(node.line);
        }
    }

    total_lines.max(node.line)
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}
