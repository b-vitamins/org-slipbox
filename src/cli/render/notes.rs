use super::explorations::{render_anchor_identity, render_node_identity};
use slipbox_core::{
    AnchorRecord, BacklinksResult, CaptureTemplatePreviewResult, ForwardLinksResult, NodeRecord,
    RandomNodeResult, SearchNodesResult, StructuralWriteReport, StructuralWriteResult,
};

pub(crate) fn render_structural_write_report(report: &StructuralWriteReport) -> String {
    let mut output = String::new();
    output.push_str(&format!("operation: {}\n", report.operation.label()));
    output.push_str(&format!(
        "index refresh: {}\n",
        render_structural_index_refresh(report.index_refresh)
    ));
    output.push_str(&format!(
        "changed files: {}\n",
        report.affected_files.changed_files.len()
    ));
    for file_path in &report.affected_files.changed_files {
        output.push_str(&format!("- {file_path}\n"));
    }
    output.push_str(&format!(
        "removed files: {}\n",
        report.affected_files.removed_files.len()
    ));
    for file_path in &report.affected_files.removed_files {
        output.push_str(&format!("- {file_path}\n"));
    }
    match &report.result {
        Some(StructuralWriteResult::Node { node }) => {
            output.push_str("result: node\n");
            output.push_str(&render_node_summary(node));
        }
        Some(StructuralWriteResult::Anchor { anchor }) => {
            output.push_str("result: anchor\n");
            output.push_str(&render_anchor_summary(anchor));
        }
        None => output.push_str("result: none\n"),
    }
    output
}

pub(crate) fn render_structural_index_refresh(
    status: slipbox_core::StructuralWriteIndexRefreshStatus,
) -> &'static str {
    match status {
        slipbox_core::StructuralWriteIndexRefreshStatus::Refreshed => "refreshed",
        slipbox_core::StructuralWriteIndexRefreshStatus::Pending => "pending",
    }
}

pub(crate) fn render_node_summary(node: &NodeRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("node key: {}\n", node.node_key));
    if let Some(explicit_id) = &node.explicit_id {
        output.push_str(&format!("id: {explicit_id}\n"));
    }
    output.push_str(&format!("title: {}\n", node.title));
    output.push_str(&format!("kind: {}\n", node.kind.as_str()));
    output.push_str(&format!("file: {}\n", node.file_path));
    output.push_str(&format!("line: {}\n", node.line));
    if !node.outline_path.is_empty() {
        output.push_str(&format!("outline path: {}\n", node.outline_path));
    }
    if !node.aliases.is_empty() {
        output.push_str(&format!("aliases: {}\n", node.aliases.join(", ")));
    }
    if !node.refs.is_empty() {
        output.push_str(&format!("refs: {}\n", node.refs.join(", ")));
    }
    if !node.tags.is_empty() {
        output.push_str(&format!("tags: {}\n", node.tags.join(", ")));
    }
    if let Some(todo_keyword) = &node.todo_keyword {
        output.push_str(&format!("todo: {todo_keyword}\n"));
    }
    if let Some(scheduled_for) = &node.scheduled_for {
        output.push_str(&format!("scheduled: {scheduled_for}\n"));
    }
    if let Some(deadline_for) = &node.deadline_for {
        output.push_str(&format!("deadline: {deadline_for}\n"));
    }
    if let Some(closed_at) = &node.closed_at {
        output.push_str(&format!("closed: {closed_at}\n"));
    }
    output
}

pub(crate) fn render_anchor_summary(anchor: &AnchorRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("anchor key: {}\n", anchor.node_key));
    if let Some(explicit_id) = &anchor.explicit_id {
        output.push_str(&format!("id: {explicit_id}\n"));
    }
    output.push_str(&format!("title: {}\n", anchor.title));
    output.push_str(&format!("kind: {}\n", anchor.kind.as_str()));
    output.push_str(&format!("file: {}\n", anchor.file_path));
    output.push_str(&format!("line: {}\n", anchor.line));
    if !anchor.outline_path.is_empty() {
        output.push_str(&format!("outline path: {}\n", anchor.outline_path));
    }
    if !anchor.aliases.is_empty() {
        output.push_str(&format!("aliases: {}\n", anchor.aliases.join(", ")));
    }
    if !anchor.refs.is_empty() {
        output.push_str(&format!("refs: {}\n", anchor.refs.join(", ")));
    }
    if !anchor.tags.is_empty() {
        output.push_str(&format!("tags: {}\n", anchor.tags.join(", ")));
    }
    if let Some(todo_keyword) = &anchor.todo_keyword {
        output.push_str(&format!("todo: {todo_keyword}\n"));
    }
    if let Some(scheduled_for) = &anchor.scheduled_for {
        output.push_str(&format!("scheduled: {scheduled_for}\n"));
    }
    if let Some(deadline_for) = &anchor.deadline_for {
        output.push_str(&format!("deadline: {deadline_for}\n"));
    }
    if let Some(closed_at) = &anchor.closed_at {
        output.push_str(&format!("closed: {closed_at}\n"));
    }
    output
}

pub(crate) fn render_node_search_result(result: &SearchNodesResult) -> String {
    let mut output = format!("nodes: {}\n", result.nodes.len());
    for node in &result.nodes {
        output.push_str(&format!("- {}\n", render_node_identity(node)));
    }
    output
}

pub(crate) fn render_random_node_result(result: &RandomNodeResult) -> String {
    match &result.node {
        Some(node) => render_node_summary(node),
        None => "node: none\n".to_owned(),
    }
}

pub(crate) fn render_backlinks_result(result: &BacklinksResult) -> String {
    let mut output = format!("backlinks: {}\n", result.backlinks.len());
    for record in &result.backlinks {
        output.push_str(&format!(
            "- {} at {}:{}\n",
            render_node_identity(&record.source_note),
            record.row,
            record.col
        ));
        if let Some(anchor) = &record.source_anchor {
            output.push_str(&format!("  anchor: {}\n", render_anchor_identity(anchor)));
        }
        output.push_str(&format!("  preview: {}\n", record.preview));
    }
    output
}

pub(crate) fn render_forward_links_result(result: &ForwardLinksResult) -> String {
    let mut output = format!("forward links: {}\n", result.forward_links.len());
    for record in &result.forward_links {
        output.push_str(&format!(
            "- {} at {}:{}\n",
            render_node_identity(&record.destination_note),
            record.row,
            record.col
        ));
        output.push_str(&format!("  preview: {}\n", record.preview));
    }
    output
}

pub(crate) fn render_capture_preview(preview: &CaptureTemplatePreviewResult) -> String {
    let mut output = format!("preview file: {}\n", preview.file_path);
    output.push_str(&format!(
        "preview node: {} | {} | line {}\n",
        preview.preview_node.node_key, preview.preview_node.title, preview.preview_node.line
    ));
    output.push_str("--- content ---\n");
    output.push_str(&preview.content);
    if !preview.content.ends_with('\n') {
        output.push('\n');
    }
    output
}
