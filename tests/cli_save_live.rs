use std::fs;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{
    ExecuteExplorationArtifactResult, ExecutedExplorationArtifactPayload, ExplorationArtifactKind,
    ExplorationArtifactSummary, ExplorationLens, ExploreResult, NoteComparisonGroup,
    NoteComparisonResult,
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

#[derive(Debug, Deserialize)]
struct SavedExploreCommandResult {
    result: ExploreResult,
    artifact: ExplorationArtifactSummary,
}

#[derive(Debug, Deserialize)]
struct SavedCompareCommandResult {
    result: NoteComparisonResult,
    artifact: ExplorationArtifactSummary,
}

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

fn build_indexed_fixture() -> Result<(tempfile::TempDir, String, String, String)> {
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
    fs::write(
        root.join("context.org"),
        r#"#+title: Context

* TODO Dual Match Peer
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Matches both planning fields directly.

* NEXT Cross Match Peer
SCHEDULED: <2026-05-03 Sat>
DEADLINE: <2026-05-01 Thu>
Matches both planning dates through opposite fields.

* TODO Keyword Only Peer
Shares only the same task state.

* WAIT Deadline Peer
DEADLINE: <2026-05-03 Sat>
Shares only the focus deadline.

* TODO Anonymous Focus
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
:PROPERTIES:
:ROAM_REFS: cite:shared2024
:END:
Anonymous anchor body.
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;
    let anonymous_anchor_key = database
        .anchors_in_file("context.org")?
        .into_iter()
        .find(|anchor| anchor.title == "Anonymous Focus")
        .map(|anchor| anchor.node_key)
        .expect("anonymous focus anchor should exist");

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
        anonymous_anchor_key,
    ))
}

fn run_slipbox(args: &[String]) -> Result<std::process::Output> {
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

fn base_headless_args(command: &str, root: &str, db: &str) -> Vec<String> {
    vec![
        command.to_owned(),
        "--root".to_owned(),
        root.to_owned(),
        "--db".to_owned(),
        db.to_owned(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--json".to_owned(),
    ]
}

fn explore_command(
    root: &str,
    db: &str,
    target_args: &[&str],
    extra_args: &[&str],
) -> Result<std::process::Output> {
    let mut args = base_headless_args("explore", root, db);
    args.extend(target_args.iter().map(|value| (*value).to_owned()));
    args.extend(extra_args.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

fn compare_command(root: &str, db: &str, extra_args: &[&str]) -> Result<std::process::Output> {
    let mut args = base_headless_args("compare", root, db);
    args.extend(extra_args.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

fn artifact_run_command(root: &str, db: &str, artifact_id: &str) -> Result<std::process::Output> {
    let args = vec![
        "artifact".to_owned(),
        "run".to_owned(),
        "--root".to_owned(),
        root.to_owned(),
        "--db".to_owned(),
        db.to_owned(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--json".to_owned(),
        artifact_id.to_owned(),
    ];
    run_slipbox(&args)
}

#[test]
fn explore_command_can_save_anchor_scoped_live_results() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let output = explore_command(
        &root,
        &db,
        &["--key", &anonymous_anchor_key],
        &[
            "--lens",
            "time",
            "--limit",
            "1",
            "--save",
            "--artifact-id",
            "artifact/explore-time",
            "--artifact-title",
            "Saved Time View",
            "--artifact-summary",
            "Anchor scoped time lens",
        ],
    )?;

    assert!(output.status.success(), "{output:?}");
    let saved: SavedExploreCommandResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(saved.artifact.metadata.artifact_id, "artifact/explore-time");
    assert_eq!(saved.artifact.kind, ExplorationArtifactKind::LensView);

    let run_output = artifact_run_command(&root, &db, "artifact/explore-time")?;
    assert!(run_output.status.success(), "{run_output:?}");
    let executed: ExecuteExplorationArtifactResult = serde_json::from_slice(&run_output.stdout)?;
    match executed.artifact.payload {
        ExecutedExplorationArtifactPayload::LensView {
            artifact,
            root_note: _,
            current_note: _,
            result,
        } => {
            assert_eq!(artifact.root_node_key, anonymous_anchor_key);
            assert_eq!(artifact.current_node_key, anonymous_anchor_key);
            assert_eq!(artifact.lens, ExplorationLens::Time);
            assert_eq!(*result, saved.result);
        }
        other => panic!("expected executed lens-view artifact, got {other:?}"),
    }

    Ok(())
}

#[test]
fn compare_command_can_save_live_results_with_group_semantics() -> Result<()> {
    let (_workspace, root, db, _) = build_indexed_fixture()?;

    let output = compare_command(
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--group",
            "tension",
            "--save",
            "--artifact-id",
            "artifact/compare-tension",
            "--artifact-title",
            "Saved Tension Comparison",
        ],
    )?;

    assert!(output.status.success(), "{output:?}");
    let saved: SavedCompareCommandResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        saved.artifact.metadata.artifact_id,
        "artifact/compare-tension"
    );
    assert_eq!(saved.artifact.kind, ExplorationArtifactKind::Comparison);

    let run_output = artifact_run_command(&root, &db, "artifact/compare-tension")?;
    assert!(run_output.status.success(), "{run_output:?}");
    let executed: ExecuteExplorationArtifactResult = serde_json::from_slice(&run_output.stdout)?;
    match executed.artifact.payload {
        ExecutedExplorationArtifactPayload::Comparison {
            artifact,
            root_note: _,
            result,
        } => {
            assert_eq!(artifact.comparison_group, NoteComparisonGroup::Tension);
            assert_eq!(
                result.filtered_to_group(artifact.comparison_group),
                saved.result
            );
        }
        other => panic!("expected executed comparison artifact, got {other:?}"),
    }

    Ok(())
}

#[test]
fn live_save_commands_report_conflicts_and_support_overwrite() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let initial = explore_command(
        &root,
        &db,
        &["--key", &anonymous_anchor_key],
        &[
            "--lens",
            "refs",
            "--save",
            "--artifact-id",
            "artifact/conflict",
            "--artifact-title",
            "Conflict Probe",
        ],
    )?;
    assert!(initial.status.success(), "{initial:?}");

    let conflict = compare_command(
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--save",
            "--artifact-id",
            "artifact/conflict",
            "--artifact-title",
            "Conflict Probe Replacement",
        ],
    )?;
    assert_eq!(conflict.status.code(), Some(1));
    assert!(conflict.stdout.is_empty());
    let conflict_error: ErrorPayload = serde_json::from_slice(&conflict.stderr)?;
    assert!(
        conflict_error
            .error
            .message
            .contains("exploration artifact already exists: artifact/conflict")
    );

    let overwrite = compare_command(
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--save",
            "--artifact-id",
            "artifact/conflict",
            "--artifact-title",
            "Conflict Probe Replacement",
            "--overwrite",
        ],
    )?;
    assert!(overwrite.status.success(), "{overwrite:?}");
    let saved: SavedCompareCommandResult = serde_json::from_slice(&overwrite.stdout)?;
    assert_eq!(saved.artifact.metadata.title, "Conflict Probe Replacement");

    Ok(())
}

#[test]
fn live_save_commands_reject_missing_and_invalid_metadata() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let missing = explore_command(
        &root,
        &db,
        &["--key", &anonymous_anchor_key],
        &[
            "--lens",
            "tasks",
            "--save",
            "--artifact-id",
            "artifact/missing-title",
        ],
    )?;
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    let missing_error: ErrorPayload = serde_json::from_slice(&missing.stderr)?;
    assert!(
        missing_error
            .error
            .message
            .contains("--save requires --artifact-title")
    );

    let invalid = compare_command(
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--save",
            "--artifact-id",
            "artifact/blank-title",
            "--artifact-title",
            "",
        ],
    )?;
    assert_eq!(invalid.status.code(), Some(1));
    assert!(invalid.stdout.is_empty());
    let invalid_error: ErrorPayload = serde_json::from_slice(&invalid.stderr)?;
    assert!(
        invalid_error
            .error
            .message
            .contains("title must not be empty")
    );

    Ok(())
}
