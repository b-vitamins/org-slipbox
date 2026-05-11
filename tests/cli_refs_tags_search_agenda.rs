use std::fs;
use std::process::Command;

use anyhow::Result;
use chrono::{Local, NaiveDate};
use serde::Deserialize;
use slipbox_core::{
    AgendaResult, NodeRecord, SearchOccurrencesResult, SearchRefsResult, SearchTagsResult,
};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[derive(Debug, Deserialize)]
struct ErrorPayload {
    error: ErrorMessage,
}

#[derive(Debug, Deserialize)]
struct ErrorMessage {
    message: String,
}

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

fn org_date(date: NaiveDate) -> String {
    format!("<{} {}>", date.format("%Y-%m-%d"), date.format("%a"))
}

fn build_indexed_fixture() -> Result<(tempfile::TempDir, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let today = Local::now().date_naive();
    fs::write(
        root.join("alpha.org"),
        format!(
            r#":PROPERTIES:
:ID: alpha-id
:ROAM_REFS: cite:alpha2024 [cite:@smith2024]
:END:
#+title: Alpha
#+filetags: :shared:alpha:

Needle appears in Alpha.
See [[id:beta-id][Beta]].

* TODO Today Focus :task:
SCHEDULED: {}
Needle appears today.

* TODO Range Start
DEADLINE: <2026-06-01 Mon>

* TODO Range End
SCHEDULED: <2026-06-03 Wed>
"#,
            org_date(today)
        ),
    )?;
    fs::write(
        root.join("beta.org"),
        r#":PROPERTIES:
:ID: beta-id
:ROAM_REFS: cite:beta2024
:END:
#+title: Beta
#+filetags: :shared:beta:

Unrelated body.
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
    ))
}

fn scoped_args(root: &str, db: &str) -> Vec<String> {
    vec![
        "--root".to_owned(),
        root.to_owned(),
        "--db".to_owned(),
        db.to_owned(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--json".to_owned(),
    ]
}

fn run_slipbox(args: &[String]) -> Result<std::process::Output> {
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

#[test]
fn ref_commands_search_and_resolve_refs_as_json() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let mut search_args = vec!["ref".to_owned(), "search".to_owned(), "alpha".to_owned()];
    search_args.extend(scoped_args(&root, &db));
    search_args.extend(["--limit".to_owned(), "1".to_owned()]);
    let search_output = run_slipbox(&search_args)?;

    assert!(search_output.status.success(), "{search_output:?}");
    let refs: SearchRefsResult = serde_json::from_slice(&search_output.stdout)?;
    assert_eq!(refs.refs.len(), 1);
    assert_eq!(refs.refs[0].reference, "@alpha2024");
    assert_eq!(refs.refs[0].node.title, "Alpha");
    assert!(search_output.stderr.is_empty());

    let mut show_args = vec![
        "ref".to_owned(),
        "show".to_owned(),
        "cite:alpha2024".to_owned(),
    ];
    show_args.extend(scoped_args(&root, &db));
    let show_output = run_slipbox(&show_args)?;

    assert!(show_output.status.success(), "{show_output:?}");
    let node: NodeRecord = serde_json::from_slice(&show_output.stdout)?;
    assert_eq!(node.node_key, "file:alpha.org");
    assert_eq!(node.refs, vec!["@alpha2024", "@smith2024"]);
    assert!(show_output.stderr.is_empty());

    Ok(())
}

#[test]
fn tag_and_occurrence_search_cover_limits_and_empty_results() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let mut tag_args = vec!["tag".to_owned(), "search".to_owned(), "sh".to_owned()];
    tag_args.extend(scoped_args(&root, &db));
    let tag_output = run_slipbox(&tag_args)?;
    assert!(tag_output.status.success(), "{tag_output:?}");
    let tags: SearchTagsResult = serde_json::from_slice(&tag_output.stdout)?;
    assert_eq!(tags.tags, vec!["shared".to_owned()]);

    let mut empty_tag_args = vec!["tag".to_owned(), "search".to_owned(), "missing".to_owned()];
    empty_tag_args.extend(scoped_args(&root, &db));
    let empty_tag_output = run_slipbox(&empty_tag_args)?;
    assert!(empty_tag_output.status.success(), "{empty_tag_output:?}");
    let empty_tags: SearchTagsResult = serde_json::from_slice(&empty_tag_output.stdout)?;
    assert!(empty_tags.tags.is_empty());

    let mut occurrence_args = vec![
        "search".to_owned(),
        "occurrences".to_owned(),
        "Needle".to_owned(),
    ];
    occurrence_args.extend(scoped_args(&root, &db));
    occurrence_args.extend(["--limit".to_owned(), "1".to_owned()]);
    let occurrence_output = run_slipbox(&occurrence_args)?;
    assert!(occurrence_output.status.success(), "{occurrence_output:?}");
    let occurrences: SearchOccurrencesResult = serde_json::from_slice(&occurrence_output.stdout)?;
    assert_eq!(occurrences.occurrences.len(), 1);
    assert_eq!(occurrences.occurrences[0].matched_text, "Needle");
    assert_eq!(occurrences.occurrences[0].file_path, "alpha.org");
    assert!(occurrences.occurrences[0].owning_anchor.is_some());

    let mut empty_occurrence_args = vec![
        "search".to_owned(),
        "occurrences".to_owned(),
        "missing".to_owned(),
    ];
    empty_occurrence_args.extend(scoped_args(&root, &db));
    let empty_occurrence_output = run_slipbox(&empty_occurrence_args)?;
    assert!(
        empty_occurrence_output.status.success(),
        "{empty_occurrence_output:?}"
    );
    let empty_occurrences: SearchOccurrencesResult =
        serde_json::from_slice(&empty_occurrence_output.stdout)?;
    assert!(empty_occurrences.occurrences.is_empty());

    Ok(())
}

#[test]
fn agenda_commands_query_today_date_and_ranges() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let mut today_args = vec!["agenda".to_owned(), "today".to_owned()];
    today_args.extend(scoped_args(&root, &db));
    let today_output = run_slipbox(&today_args)?;
    assert!(today_output.status.success(), "{today_output:?}");
    let today: AgendaResult = serde_json::from_slice(&today_output.stdout)?;
    assert_eq!(today.nodes.len(), 1);
    assert_eq!(today.nodes[0].title, "Today Focus");

    let mut date_args = vec![
        "agenda".to_owned(),
        "date".to_owned(),
        "2026-06-01".to_owned(),
    ];
    date_args.extend(scoped_args(&root, &db));
    date_args.extend(["--limit".to_owned(), "1".to_owned()]);
    let date_output = run_slipbox(&date_args)?;
    assert!(date_output.status.success(), "{date_output:?}");
    let date: AgendaResult = serde_json::from_slice(&date_output.stdout)?;
    assert_eq!(date.nodes.len(), 1);
    assert_eq!(date.nodes[0].title, "Range Start");
    assert_eq!(
        date.nodes[0].deadline_for.as_deref(),
        Some("2026-06-01T00:00:00")
    );

    let output = Command::new(slipbox_binary())
        .args([
            "agenda",
            "range",
            "2026-06-01",
            "2026-06-03",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            slipbox_binary(),
        ])
        .output()?;
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("agenda entries: 2"));
    assert!(stdout.contains("Range Start"));
    assert!(stdout.contains("Range End"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn query_commands_report_structured_json_failures() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let mut missing_ref_args = vec![
        "ref".to_owned(),
        "resolve".to_owned(),
        "cite:missing".to_owned(),
    ];
    missing_ref_args.extend(scoped_args(&root, &db));
    let missing_ref_output = run_slipbox(&missing_ref_args)?;
    assert_eq!(missing_ref_output.status.code(), Some(1));
    assert!(missing_ref_output.stdout.is_empty());
    let missing_ref: ErrorPayload = serde_json::from_slice(&missing_ref_output.stderr)?;
    assert!(
        missing_ref
            .error
            .message
            .contains("unknown node ref: cite:missing")
    );

    let mut invalid_date_args = vec![
        "agenda".to_owned(),
        "date".to_owned(),
        "2026-13-01".to_owned(),
    ];
    invalid_date_args.extend(scoped_args(&root, &db));
    let invalid_date_output = run_slipbox(&invalid_date_args)?;
    assert_eq!(invalid_date_output.status.code(), Some(1));
    assert!(invalid_date_output.stdout.is_empty());
    let invalid_date: ErrorPayload = serde_json::from_slice(&invalid_date_output.stderr)?;
    assert!(
        invalid_date
            .error
            .message
            .contains("expected ISO date YYYY-MM-DD")
    );

    let mut invalid_range_args = vec![
        "agenda".to_owned(),
        "range".to_owned(),
        "2026-06-03".to_owned(),
        "2026-06-01".to_owned(),
    ];
    invalid_range_args.extend(scoped_args(&root, &db));
    let invalid_range_output = run_slipbox(&invalid_range_args)?;
    assert_eq!(invalid_range_output.status.code(), Some(1));
    assert!(invalid_range_output.stdout.is_empty());
    let invalid_range: ErrorPayload = serde_json::from_slice(&invalid_range_output.stderr)?;
    assert!(invalid_range.error.message.contains("is before start"));

    Ok(())
}
