use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use slipbox_core::{
    CorpusAuditEntry, CorpusAuditKind, CorpusAuditResult, ReviewRunPayload, ReviewRunResult,
    SaveCorpusAuditReviewResult,
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

fn build_indexed_fixture() -> Result<(tempfile::TempDir, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("duplicate-a.org"),
        r#":PROPERTIES:
:ID: dup-a-id
:END:
#+title: Shared Title

Links to [[id:dup-b-id][Other duplicate]].
"#,
    )?;
    fs::write(
        root.join("duplicate-b.org"),
        r#":PROPERTIES:
:ID: dup-b-id
:END:
#+title: shared title

Links to [[id:dup-a-id][Other duplicate]].
"#,
    )?;
    fs::write(
        root.join("dangling-source.org"),
        r#":PROPERTIES:
:ID: dangling-source-id
:END:
#+title: Dangling Source

Points to [[id:missing-id][Missing]].
"#,
    )?;
    fs::write(
        root.join("orphan.org"),
        r#":PROPERTIES:
:ID: orphan-id
:END:
#+title: Orphan

Just an orphan note.
"#,
    )?;
    fs::write(
        root.join("weak.org"),
        r#":PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:weak2024
:END:
#+title: Weak

Has refs but no structural links.
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

fn audit_command(
    root: &str,
    db: &str,
    subcommand: &str,
    json: bool,
) -> Result<std::process::Output> {
    audit_command_with_server(root, db, subcommand, json, Path::new(slipbox_binary()))
}

fn audit_command_with_server(
    root: &str,
    db: &str,
    subcommand: &str,
    json: bool,
    server_program: &Path,
) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "audit",
        subcommand,
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        server_program
            .to_str()
            .expect("server program path should be valid utf-8"),
    ]);
    if json {
        command.arg("--json");
    }
    command.output().map_err(Into::into)
}

fn audit_command_with_args(
    root: &str,
    db: &str,
    subcommand: &str,
    args: &[&str],
) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "audit",
        subcommand,
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
    ]);
    command.args(args);
    command.output().map_err(Into::into)
}

fn review_show_command(root: &str, db: &str, review_id: &str) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "review",
        "show",
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
        review_id,
        "--json",
    ]);
    command.output().map_err(Into::into)
}

#[test]
fn audit_dangling_links_command_returns_typed_json() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = audit_command(&root, &db, "dangling-links", true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: CorpusAuditResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.audit, CorpusAuditKind::DanglingLinks);
    assert_eq!(parsed.entries.len(), 1);
    match &parsed.entries[0] {
        CorpusAuditEntry::DanglingLink { record } => {
            assert_eq!(record.source.title, "Dangling Source");
            assert_eq!(record.missing_explicit_id, "missing-id");
        }
        other => panic!("expected dangling-link entry, got {:?}", other.kind()),
    }

    Ok(())
}

#[test]
fn audit_duplicate_titles_command_returns_typed_json() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = audit_command(&root, &db, "duplicate-titles", true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: CorpusAuditResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.audit, CorpusAuditKind::DuplicateTitles);
    assert_eq!(parsed.entries.len(), 1);
    match &parsed.entries[0] {
        CorpusAuditEntry::DuplicateTitle { record } => {
            assert_eq!(record.title, "Shared Title");
            assert_eq!(record.notes.len(), 2);
        }
        other => panic!("expected duplicate-title entry, got {:?}", other.kind()),
    }

    Ok(())
}

#[test]
fn audit_orphan_notes_command_returns_typed_json() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = audit_command(&root, &db, "orphan-notes", true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: CorpusAuditResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.audit, CorpusAuditKind::OrphanNotes);
    assert_eq!(parsed.entries.len(), 1);
    match &parsed.entries[0] {
        CorpusAuditEntry::OrphanNote { record } => {
            assert_eq!(record.note.title, "Orphan");
            assert_eq!(record.reference_count, 0);
        }
        other => panic!("expected orphan-note entry, got {:?}", other.kind()),
    }

    Ok(())
}

#[test]
fn audit_weakly_integrated_notes_command_returns_typed_json() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = audit_command(&root, &db, "weakly-integrated-notes", true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: CorpusAuditResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.audit, CorpusAuditKind::WeaklyIntegratedNotes);
    assert_eq!(parsed.entries.len(), 1);
    match &parsed.entries[0] {
        CorpusAuditEntry::WeaklyIntegratedNote { record } => {
            assert_eq!(record.note.title, "Weak");
            assert_eq!(record.reference_count, 1);
        }
        other => panic!(
            "expected weakly-integrated-note entry, got {:?}",
            other.kind()
        ),
    }

    Ok(())
}

#[test]
fn audit_commands_print_human_output() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let dangling = audit_command(&root, &db, "dangling-links", false)?;
    assert!(dangling.status.success(), "{dangling:?}");
    let dangling = String::from_utf8(dangling.stdout)?;
    assert!(dangling.contains("audit: dangling-links"));
    assert!(dangling.contains("Dangling Source"));
    assert!(dangling.contains("missing id missing-id"));

    let duplicates = audit_command(&root, &db, "duplicate-titles", false)?;
    assert!(duplicates.status.success(), "{duplicates:?}");
    let duplicates = String::from_utf8(duplicates.stdout)?;
    assert!(duplicates.contains("audit: duplicate-titles"));
    assert!(duplicates.contains("duplicate title: Shared Title"));

    let orphans = audit_command(&root, &db, "orphan-notes", false)?;
    assert!(orphans.status.success(), "{orphans:?}");
    let orphans = String::from_utf8(orphans.stdout)?;
    assert!(orphans.contains("audit: orphan-notes"));
    assert!(orphans.contains("orphan note: Orphan"));

    let weak = audit_command(&root, &db, "weakly-integrated-notes", false)?;
    assert!(weak.status.success(), "{weak:?}");
    let weak = String::from_utf8(weak.stdout)?;
    assert!(weak.contains("audit: weakly-integrated-notes"));
    assert!(weak.contains("weakly integrated note: Weak"));

    Ok(())
}

#[test]
fn audit_commands_report_structured_daemon_failures() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = audit_command_with_server(
        &root,
        &db,
        "dangling-links",
        true,
        Path::new("/definitely/missing/slipbox"),
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("failed to start slipbox daemon")
    );

    Ok(())
}

#[test]
fn audit_save_review_flags_persist_typed_reviews_and_enforce_policy() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = audit_command_with_args(
        &root,
        &db,
        "dangling-links",
        &[
            "--save-review",
            "--review-id",
            "review/audit/dangling/custom",
            "--review-title",
            "Dangling Review",
            "--review-summary",
            "Review links to missing IDs.",
            "--json",
        ],
    )?;
    assert!(output.status.success(), "{output:?}");
    let saved: SaveCorpusAuditReviewResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(saved.result.audit, CorpusAuditKind::DanglingLinks);
    assert_eq!(saved.result.entries.len(), 1);
    assert_eq!(
        saved.review.metadata.review_id,
        "review/audit/dangling/custom"
    );
    assert_eq!(saved.review.metadata.title, "Dangling Review");
    assert_eq!(saved.review.finding_count, saved.result.entries.len());

    let shown = review_show_command(&root, &db, "review/audit/dangling/custom")?;
    assert!(shown.status.success(), "{shown:?}");
    let shown: ReviewRunResult = serde_json::from_slice(&shown.stdout)?;
    match shown.review.payload {
        ReviewRunPayload::Audit { audit, limit } => {
            assert_eq!(audit, CorpusAuditKind::DanglingLinks);
            assert_eq!(limit, 200);
        }
        other => panic!("expected audit review, got {:?}", other.kind()),
    }
    assert_eq!(shown.review.findings.len(), saved.result.entries.len());
    match &shown.review.findings[0].payload {
        slipbox_core::ReviewFindingPayload::Audit { entry } => {
            assert_eq!(entry.as_ref(), &saved.result.entries[0]);
        }
        other => panic!("expected audit finding, got {:?}", other.kind()),
    }

    let conflict = audit_command_with_args(
        &root,
        &db,
        "dangling-links",
        &[
            "--save-review",
            "--review-id",
            "review/audit/dangling/custom",
            "--json",
        ],
    )?;
    assert_eq!(conflict.status.code(), Some(1));
    assert!(conflict.stdout.is_empty());
    let error: ErrorPayload = serde_json::from_slice(&conflict.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("review run already exists: review/audit/dangling/custom")
    );

    let overwritten = audit_command_with_args(
        &root,
        &db,
        "dangling-links",
        &[
            "--save-review",
            "--review-id",
            "review/audit/dangling/custom",
            "--review-title",
            "Replacement Dangling Review",
            "--overwrite",
            "--json",
        ],
    )?;
    assert!(overwritten.status.success(), "{overwritten:?}");
    let overwritten: SaveCorpusAuditReviewResult = serde_json::from_slice(&overwritten.stdout)?;
    assert_eq!(
        overwritten.review.metadata.title,
        "Replacement Dangling Review"
    );

    let invalid = audit_command_with_args(
        &root,
        &db,
        "dangling-links",
        &["--save-review", "--review-id", " bad ", "--json"],
    )?;
    assert_eq!(invalid.status.code(), Some(1));
    let error: ErrorPayload = serde_json::from_slice(&invalid.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("review_id must not have leading or trailing whitespace")
    );

    let stray = audit_command_with_args(
        &root,
        &db,
        "dangling-links",
        &["--review-id", "review/audit/stray", "--json"],
    )?;
    assert_eq!(stray.status.code(), Some(1));
    let error: ErrorPayload = serde_json::from_slice(&stray.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("--review-id require --save-review")
    );

    Ok(())
}

#[test]
fn audit_save_review_report_output_acknowledges_review_summary() -> Result<()> {
    let (workspace, root, db) = build_indexed_fixture()?;
    let report_path = workspace.path().join("duplicate-report.json");

    let output = audit_command_with_args(
        &root,
        &db,
        "duplicate-titles",
        &[
            "--save-review",
            "--review-id",
            "review/audit/duplicates/report",
            "--output",
            report_path
                .to_str()
                .expect("report path should be valid utf-8"),
            "--json",
        ],
    )?;
    assert!(output.status.success(), "{output:?}");
    let ack: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(ack["audit"], "duplicate-titles");
    assert_eq!(ack["format"], "json");
    assert_eq!(ack["review"]["review_id"], "review/audit/duplicates/report");
    let report: CorpusAuditResult = serde_json::from_slice(&fs::read(report_path)?)?;
    assert_eq!(report.audit, CorpusAuditKind::DuplicateTitles);
    assert_eq!(report.entries.len(), 1);

    Ok(())
}
