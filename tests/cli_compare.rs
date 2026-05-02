use std::fs;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{NoteComparisonResult, NoteComparisonSectionKind};
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

fn build_indexed_fixture() -> Result<(tempfile::TempDir, String, String, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("comparison.org"),
        r#"#+title: Comparison

* TODO Left
:PROPERTIES:
:ID: left-id
:ROAM_REFS: cite:shared2024 cite:sharedtwo2024 cite:left2024
:END:
SCHEDULED: <2026-05-01 Thu>
Links to [[id:shared-forward-id]] and [[id:left-right-bridge-id]].

* NEXT Right
:PROPERTIES:
:ID: right-id
:ROAM_REFS: cite:shared2024 cite:sharedtwo2024 cite:right2024
:END:
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-01 Thu>
Links to [[id:shared-forward-id]] and [[id:right-left-bridge-id]].

* Shared Forward
:PROPERTIES:
:ID: shared-forward-id
:END:
Forward target body.

* Left To Right Bridge
:PROPERTIES:
:ID: left-right-bridge-id
:END:
Connects to [[id:right-id]].

* Right To Left Bridge
:PROPERTIES:
:ID: right-left-bridge-id
:END:
Connects to [[id:left-id]].

* Shared Backlink
:PROPERTIES:
:ID: shared-backlink-id
:END:
Links to [[id:left-id]] and [[id:right-id]].
"#,
    )?;
    fs::write(root.join("dup-one.org"), "#+title: Shared Target\n")?;
    fs::write(root.join("dup-two.org"), "#+title: Shared Target\n")?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;
    let left_key = database
        .node_from_id("left-id")?
        .expect("left note should exist")
        .node_key;
    let right_key = database
        .node_from_id("right-id")?
        .expect("right note should exist")
        .node_key;

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
        left_key,
        right_key,
    ))
}

fn compare_command(
    root: &str,
    db: &str,
    target_args: &[&str],
    extra_args: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec![
        "compare",
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
        "--json",
    ];
    args.extend_from_slice(target_args);
    args.extend_from_slice(extra_args);
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

#[test]
fn compare_command_supports_all_group_and_limit() -> Result<()> {
    let (_workspace, root, db, _, _) = build_indexed_fixture()?;

    let output = compare_command(
        &root,
        &db,
        &["--left-id", "left-id", "--right-id", "right-id"],
        &["--limit", "1"],
    )?;

    assert!(output.status.success(), "{output:?}");
    let parsed: NoteComparisonResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            NoteComparisonSectionKind::SharedRefs,
            NoteComparisonSectionKind::SharedPlanningDates,
            NoteComparisonSectionKind::LeftOnlyRefs,
            NoteComparisonSectionKind::RightOnlyRefs,
            NoteComparisonSectionKind::SharedBacklinks,
            NoteComparisonSectionKind::SharedForwardLinks,
            NoteComparisonSectionKind::ContrastingTaskStates,
            NoteComparisonSectionKind::PlanningTensions,
            NoteComparisonSectionKind::IndirectConnectors,
        ]
    );
    assert_eq!(parsed.sections[0].entries.len(), 1);
    assert_eq!(parsed.left_note.title, "Left");
    assert_eq!(parsed.right_note.title, "Right");

    Ok(())
}

#[test]
fn compare_command_filters_overlap_group() -> Result<()> {
    let (_workspace, root, db, _, _) = build_indexed_fixture()?;

    let output = compare_command(
        &root,
        &db,
        &["--left-title", "Left", "--right-ref", "cite:right2024"],
        &["--group", "overlap"],
    )?;

    assert!(output.status.success(), "{output:?}");
    let parsed: NoteComparisonResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            NoteComparisonSectionKind::SharedRefs,
            NoteComparisonSectionKind::SharedPlanningDates,
            NoteComparisonSectionKind::SharedBacklinks,
            NoteComparisonSectionKind::SharedForwardLinks,
        ]
    );

    Ok(())
}

#[test]
fn compare_command_filters_divergence_group() -> Result<()> {
    let (_workspace, root, db, left_key, _) = build_indexed_fixture()?;

    let output = compare_command(
        &root,
        &db,
        &["--left-key", &left_key, "--right-id", "right-id"],
        &["--group", "divergence"],
    )?;

    assert!(output.status.success(), "{output:?}");
    let parsed: NoteComparisonResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            NoteComparisonSectionKind::LeftOnlyRefs,
            NoteComparisonSectionKind::RightOnlyRefs,
        ]
    );

    Ok(())
}

#[test]
fn compare_command_filters_tension_group() -> Result<()> {
    let (_workspace, root, db, _, right_key) = build_indexed_fixture()?;

    let output = compare_command(
        &root,
        &db,
        &["--left-id", "left-id", "--right-key", &right_key],
        &["--group", "tension"],
    )?;

    assert!(output.status.success(), "{output:?}");
    let parsed: NoteComparisonResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            NoteComparisonSectionKind::ContrastingTaskStates,
            NoteComparisonSectionKind::PlanningTensions,
            NoteComparisonSectionKind::IndirectConnectors,
        ]
    );

    Ok(())
}

#[test]
fn compare_command_reports_unknown_targets() -> Result<()> {
    let (_workspace, root, db, _, _) = build_indexed_fixture()?;

    let output = compare_command(
        &root,
        &db,
        &["--left-id", "missing-id", "--right-id", "right-id"],
        &[],
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(parsed.error.message.contains("unknown node id: missing-id"));

    Ok(())
}

#[test]
fn compare_command_reports_ambiguous_titles() -> Result<()> {
    let (_workspace, root, db, _, _) = build_indexed_fixture()?;

    let output = compare_command(
        &root,
        &db,
        &["--left-id", "left-id", "--right-title", "Shared Target"],
        &[],
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("multiple nodes match Shared Target")
    );

    Ok(())
}

#[test]
fn compare_command_prints_human_output_with_group_identity() -> Result<()> {
    let (_workspace, root, db, _, _) = build_indexed_fixture()?;

    let output = Command::new(slipbox_binary())
        .args([
            "compare",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            slipbox_binary(),
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--group",
            "tension",
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("group: tension"));
    assert!(stdout.contains("left: Left ["));
    assert!(stdout.contains("right: Right ["));
    assert!(stdout.contains("[planning tensions]"));
    assert!(stdout.contains("why: planning tension"));
    assert!(output.stderr.is_empty());

    Ok(())
}
