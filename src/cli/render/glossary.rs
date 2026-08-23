use super::explorations::render_node_identity;
use slipbox_core::{
    GlossaryDueResult, GlossaryTermResult, GradeTermResult, ListGlossaryTermsResult,
    MarkGlossaryTermResult, NodeRecord, SearchGlossaryResult,
};

pub(crate) fn render_glossary_term_list(result: &ListGlossaryTermsResult) -> String {
    render_term_lines("terms", &result.terms, result.total)
}

pub(crate) fn render_glossary_search_result(result: &SearchGlossaryResult) -> String {
    render_term_lines("terms", &result.terms, result.total)
}

pub(crate) fn render_glossary_due_result(result: &GlossaryDueResult) -> String {
    render_term_lines("due terms", &result.terms, result.total)
}

pub(crate) fn render_glossary_term_result(result: &GlossaryTermResult) -> String {
    match &result.term {
        Some(term) => render_glossary_term_summary(term),
        None => "term: none\n".to_owned(),
    }
}

pub(crate) fn render_grade_result(result: &GradeTermResult) -> String {
    render_glossary_term_summary(&result.term)
}

pub(crate) fn render_mark_result(result: &MarkGlossaryTermResult) -> String {
    render_glossary_term_summary(&result.term)
}

fn render_term_lines(label: &str, terms: &[NodeRecord], total: usize) -> String {
    // A page cut at `--limit` states what it was cut from, so a partial listing
    // does not read as the whole glossary.
    let mut output = if total > terms.len() {
        format!("{label}: {} of {total}\n", terms.len())
    } else {
        format!("{label}: {}\n", terms.len())
    };
    for term in terms {
        output.push_str(&format!("- {}\n", render_term_line(term)));
    }
    output
}

fn render_term_line(term: &NodeRecord) -> String {
    let mut line = render_node_identity(term);
    line.push_str(&format!(" (status: {}", glossary_status_label(term)));
    if let Some(due) = &term.sr_due {
        line.push_str(&format!(", due: {due}"));
    }
    line.push(')');
    line
}

fn glossary_status_label(term: &NodeRecord) -> &str {
    term.glossary_status.as_deref().unwrap_or("stub")
}

pub(crate) fn render_glossary_term_summary(term: &NodeRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("node key: {}\n", term.node_key));
    if let Some(explicit_id) = &term.explicit_id {
        output.push_str(&format!("id: {explicit_id}\n"));
    }
    output.push_str(&format!("title: {}\n", term.title));
    output.push_str(&format!("file: {}\n", term.file_path));
    output.push_str(&format!("line: {}\n", term.line));
    output.push_str(&format!("status: {}\n", glossary_status_label(term)));
    if !term.aliases.is_empty() {
        output.push_str(&format!("synonyms: {}\n", term.aliases.join(", ")));
    }
    if let Some(due) = &term.sr_due {
        output.push_str(&format!("due: {due}\n"));
    }
    if let Some(ease) = &term.sr_ease {
        output.push_str(&format!("ease: {ease}\n"));
    }
    if let Some(interval) = &term.sr_interval {
        output.push_str(&format!("interval: {interval}\n"));
    }
    if let Some(reps) = &term.sr_reps {
        output.push_str(&format!("reps: {reps}\n"));
    }
    if let Some(last) = &term.sr_last {
        output.push_str(&format!("last: {last}\n"));
    }
    output
}
