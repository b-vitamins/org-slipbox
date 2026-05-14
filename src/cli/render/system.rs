use super::explorations::render_anchor_identity;
use slipbox_core::{
    FileDiagnosticIssue, FileDiagnostics, FileRecord, IndexDiagnostics, IndexStats,
    IndexedFilesResult, NodeDiagnosticIssue, NodeDiagnostics, SearchFilesResult,
};

pub(crate) fn render_index_stats(stats: &IndexStats) -> String {
    format!(
        "files indexed: {}\nnodes indexed: {}\nlinks indexed: {}\n",
        stats.files_indexed, stats.nodes_indexed, stats.links_indexed
    )
}

pub(crate) fn render_indexed_files(result: &IndexedFilesResult) -> String {
    let mut output = format!("indexed files: {}\n", result.files.len());
    for file_path in &result.files {
        output.push_str(&format!("- {file_path}\n"));
    }
    output
}

pub(crate) fn render_file_search_result(result: &SearchFilesResult) -> String {
    let mut output = format!("files: {}\n", result.files.len());
    for file in &result.files {
        output.push_str(&render_file_record(file));
    }
    output
}

pub(crate) fn render_file_record(file: &FileRecord) -> String {
    format!(
        "- {} | {} | nodes: {}\n",
        file.file_path, file.title, file.node_count
    )
}

pub(crate) fn render_file_diagnostics(diagnostic: &FileDiagnostics) -> String {
    let mut output = String::new();
    output.push_str(&format!("file: {}\n", diagnostic.file_path));
    output.push_str(&format!("absolute path: {}\n", diagnostic.absolute_path));
    output.push_str(&format!("exists: {}\n", yes_no(diagnostic.exists)));
    output.push_str(&format!("eligible: {}\n", yes_no(diagnostic.eligible)));
    output.push_str(&format!("indexed: {}\n", yes_no(diagnostic.indexed)));
    if let Some(record) = &diagnostic.index_record {
        output.push_str(&format!("title: {}\n", record.title));
        output.push_str(&format!("nodes: {}\n", record.node_count));
        output.push_str(&format!("mtime ns: {}\n", record.mtime_ns));
    }
    output.push_str("issues:\n");
    render_file_diagnostic_issues(&mut output, &diagnostic.issues, "  ");
    output
}

pub(crate) fn render_node_diagnostics(diagnostic: &NodeDiagnostics) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "node: {}\n",
        render_anchor_identity(&diagnostic.node)
    ));
    output.push_str(&format!("file: {}\n", diagnostic.file.file_path));
    output.push_str(&format!("line: {}\n", diagnostic.node.line));
    output.push_str(&format!(
        "line present: {}\n",
        yes_no(diagnostic.line_present)
    ));
    output.push_str("file issues:\n");
    render_file_diagnostic_issues(&mut output, &diagnostic.file.issues, "  ");
    output.push_str("node issues:\n");
    render_node_diagnostic_issues(&mut output, &diagnostic.issues, "  ");
    output
}

pub(crate) fn render_index_diagnostics(diagnostic: &IndexDiagnostics) -> String {
    let mut output = String::new();
    output.push_str(&format!("root: {}\n", diagnostic.root));
    output.push_str(&format!(
        "eligible files: {}\n",
        diagnostic.eligible_files.len()
    ));
    output.push_str(&format!(
        "indexed files: {}\n",
        diagnostic.indexed_files.len()
    ));
    output.push_str(&format!(
        "status consistent: {}\n",
        yes_no(diagnostic.status_consistent)
    ));
    output.push_str(&format!(
        "index current: {}\n",
        yes_no(diagnostic.index_current)
    ));
    output.push_str(&format!(
        "status counts: files={} nodes={} links={}\n",
        diagnostic.status.files_indexed,
        diagnostic.status.nodes_indexed,
        diagnostic.status.links_indexed
    ));
    render_path_list(
        &mut output,
        "missing from index",
        &diagnostic.missing_from_index,
    );
    render_path_list(
        &mut output,
        "indexed but missing",
        &diagnostic.indexed_but_missing,
    );
    render_path_list(
        &mut output,
        "indexed but ineligible",
        &diagnostic.indexed_but_ineligible,
    );
    output
}

pub(crate) fn render_file_diagnostic_issues(
    output: &mut String,
    issues: &[FileDiagnosticIssue],
    indent: &str,
) {
    if issues.is_empty() {
        output.push_str(indent);
        output.push_str("(none)\n");
        return;
    }
    for issue in issues {
        output.push_str(indent);
        output.push_str(match issue {
            FileDiagnosticIssue::MissingFromIndex => "missing-from-index",
            FileDiagnosticIssue::IndexedButMissing => "indexed-but-missing",
            FileDiagnosticIssue::IndexedButIneligible => "indexed-but-ineligible",
        });
        output.push('\n');
    }
}

pub(crate) fn render_node_diagnostic_issues(
    output: &mut String,
    issues: &[NodeDiagnosticIssue],
    indent: &str,
) {
    if issues.is_empty() {
        output.push_str(indent);
        output.push_str("(none)\n");
        return;
    }
    for issue in issues {
        output.push_str(indent);
        output.push_str(match issue {
            NodeDiagnosticIssue::SourceFileMissing => "source-file-missing",
            NodeDiagnosticIssue::SourceFileIneligible => "source-file-ineligible",
            NodeDiagnosticIssue::SourceFileUnindexed => "source-file-unindexed",
            NodeDiagnosticIssue::LineOutOfRange => "line-out-of-range",
        });
        output.push('\n');
    }
}

pub(crate) fn render_path_list(output: &mut String, label: &str, paths: &[String]) {
    output.push_str(&format!("{label}: {}\n", paths.len()));
    for path in paths {
        output.push_str(&format!("- {path}\n"));
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
