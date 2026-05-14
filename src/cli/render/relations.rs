use super::{
    explorations::{render_anchor_identity, render_node_identity},
    notes::render_structural_index_refresh,
};
use slipbox_core::{
    AgendaResult, OccurrenceRecord, RefRecord, SearchOccurrencesResult, SearchRefsResult,
    SearchTagsResult, SlipboxLinkRewriteApplyResult, SlipboxLinkRewritePreview,
};

pub(crate) fn render_slipbox_link_rewrite_preview(preview: &SlipboxLinkRewritePreview) -> String {
    let mut output = String::new();
    output.push_str(&format!("file: {}\n", preview.file_path));
    output.push_str(&format!("rewrites: {}\n", preview.rewrites.len()));
    if preview.rewrites.is_empty() {
        output.push_str("  (none)\n");
        return output;
    }
    for rewrite in &preview.rewrites {
        output.push_str(&format!(
            "- {}:{} slipbox:{} -> ",
            rewrite.line, rewrite.column, rewrite.title_or_alias
        ));
        match &rewrite.target_explicit_id {
            Some(explicit_id) => output.push_str(&format!("id:{explicit_id}\n")),
            None => output.push_str("id will be assigned on apply\n"),
        }
        output.push_str(&format!(
            "  target: {}\n",
            render_node_identity(&rewrite.target)
        ));
        output.push_str(&format!("  description: {}\n", rewrite.description));
        output.push_str(&format!("  preview: {}\n", rewrite.preview));
        if let Some(replacement) = &rewrite.replacement {
            output.push_str(&format!("  replacement: {replacement}\n"));
        }
    }
    output
}

pub(crate) fn render_slipbox_link_rewrite_application(
    result: &SlipboxLinkRewriteApplyResult,
) -> String {
    let application = &result.application;
    let mut output = String::new();
    output.push_str(&format!(
        "rewrote slipbox links: {}\n",
        application.file_path
    ));
    output.push_str(&format!("rewrites: {}\n", application.rewrites.len()));
    for rewrite in &application.rewrites {
        output.push_str(&format!(
            "- {}:{} slipbox:{} -> id:{}\n",
            rewrite.line, rewrite.column, rewrite.title_or_alias, rewrite.target_explicit_id
        ));
        output.push_str(&format!("  target node: {}\n", rewrite.target_node_key));
        output.push_str(&format!("  replacement: {}\n", rewrite.replacement));
    }
    output.push_str(&format!(
        "index: {}\n",
        render_structural_index_refresh(application.index_refresh)
    ));
    output.push_str("changed files:\n");
    for file in &application.affected_files.changed_files {
        output.push_str(&format!("  - {file}\n"));
    }
    output.push_str("removed files:\n");
    if application.affected_files.removed_files.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for file in &application.affected_files.removed_files {
            output.push_str(&format!("  - {file}\n"));
        }
    }
    output
}

pub(crate) fn render_ref_search_result(result: &SearchRefsResult) -> String {
    let mut output = format!("refs: {}\n", result.refs.len());
    for record in &result.refs {
        output.push_str(&render_ref_record(record));
    }
    output
}

pub(crate) fn render_ref_record(record: &RefRecord) -> String {
    format!(
        "- {} -> {}\n",
        record.reference,
        render_node_identity(&record.node)
    )
}

pub(crate) fn render_tag_search_result(result: &SearchTagsResult) -> String {
    let mut output = format!("tags: {}\n", result.tags.len());
    for tag in &result.tags {
        output.push_str(&format!("- {tag}\n"));
    }
    output
}

pub(crate) fn render_occurrence_search_result(result: &SearchOccurrencesResult) -> String {
    let mut output = format!("occurrences: {}\n", result.occurrences.len());
    for record in &result.occurrences {
        output.push_str(&render_occurrence_record(record));
    }
    output
}

pub(crate) fn render_occurrence_record(record: &OccurrenceRecord) -> String {
    let mut output = format!("- {}:{}:{}\n", record.file_path, record.row, record.col);
    if let Some(anchor) = &record.owning_anchor {
        output.push_str(&format!("  anchor: {}\n", render_anchor_identity(anchor)));
    }
    output.push_str(&format!("  matched text: {}\n", record.matched_text));
    output.push_str(&format!("  preview: {}\n", record.preview));
    output
}

pub(crate) fn render_agenda_result(result: &AgendaResult) -> String {
    let mut output = format!("agenda entries: {}\n", result.nodes.len());
    for node in &result.nodes {
        output.push_str(&format!("- {}\n", render_anchor_identity(node)));
        if let Some(todo_keyword) = &node.todo_keyword {
            output.push_str(&format!("  todo: {todo_keyword}\n"));
        }
        if let Some(scheduled_for) = &node.scheduled_for {
            output.push_str(&format!("  scheduled: {scheduled_for}\n"));
        }
        if let Some(deadline_for) = &node.deadline_for {
            output.push_str(&format!("  deadline: {deadline_for}\n"));
        }
        if let Some(closed_at) = &node.closed_at {
            output.push_str(&format!("  closed: {closed_at}\n"));
        }
    }
    output
}
