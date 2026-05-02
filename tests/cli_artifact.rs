use std::fs;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{
    DeleteExplorationArtifactResult, ExecuteExplorationArtifactResult,
    ExecutedExplorationArtifactPayload, ExplorationArtifactKind, ExplorationArtifactMetadata,
    ExplorationArtifactPayload, ExplorationArtifactResult, ExplorationLens,
    ListExplorationArtifactsResult, NoteComparisonGroup, NoteComparisonSectionKind,
    SavedComparisonArtifact, SavedExplorationArtifact, SavedLensViewArtifact, SavedTrailArtifact,
    SavedTrailStep,
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
        root.join("artifacts.org"),
        r#"#+title: Artifacts

* TODO Focus
:PROPERTIES:
:ID: focus-id
:ROAM_REFS: cite:shared2024 cite:focus2024
:END:
SCHEDULED: <2026-05-01 Thu>
Links to [[id:shared-forward-id]] and [[id:focus-bridge-id]].

* NEXT Neighbor
:PROPERTIES:
:ID: neighbor-id
:ROAM_REFS: cite:shared2024 cite:neighbor2024
:END:
DEADLINE: <2026-05-01 Thu>
Links to [[id:shared-forward-id]] and [[id:neighbor-bridge-id]].

* Shared Forward
:PROPERTIES:
:ID: shared-forward-id
:END:
Forward target body.

* Focus Bridge
:PROPERTIES:
:ID: focus-bridge-id
:END:
Links to [[id:neighbor-id]].

* Neighbor Bridge
:PROPERTIES:
:ID: neighbor-bridge-id
:END:
Links to [[id:focus-id]].

* Shared Backlink
:PROPERTIES:
:ID: shared-backlink-id
:END:
Links to [[id:focus-id]] and [[id:neighbor-id]].
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;

    let focus_key = database
        .node_from_id("focus-id")?
        .expect("focus note should exist")
        .node_key;
    let neighbor_key = database
        .node_from_id("neighbor-id")?
        .expect("neighbor note should exist")
        .node_key;

    seed_saved_artifacts(&database, &focus_key, &neighbor_key)?;

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
    ))
}

fn seed_saved_artifacts(database: &Database, focus_key: &str, neighbor_key: &str) -> Result<()> {
    let structure = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "artifact/structure".to_owned(),
            title: "Artifact Structure".to_owned(),
            summary: Some("Saved structure lens".to_owned()),
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: focus_key.to_owned(),
                current_node_key: focus_key.to_owned(),
                lens: ExplorationLens::Structure,
                limit: 25,
                unique: true,
                frozen_context: false,
            }),
        },
    };
    let comparison = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "artifact/comparison".to_owned(),
            title: "Artifact Comparison".to_owned(),
            summary: Some("Saved comparison state".to_owned()),
        },
        payload: ExplorationArtifactPayload::Comparison {
            artifact: Box::new(SavedComparisonArtifact {
                root_node_key: focus_key.to_owned(),
                left_node_key: focus_key.to_owned(),
                right_node_key: neighbor_key.to_owned(),
                active_lens: ExplorationLens::Structure,
                structure_unique: false,
                comparison_group: NoteComparisonGroup::Tension,
                limit: 10,
                frozen_context: false,
            }),
        },
    };
    let trail = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "artifact/trail".to_owned(),
            title: "Artifact Trail".to_owned(),
            summary: Some("Saved trail replay".to_owned()),
        },
        payload: ExplorationArtifactPayload::Trail {
            artifact: Box::new(SavedTrailArtifact {
                steps: vec![
                    SavedTrailStep::LensView {
                        artifact: Box::new(SavedLensViewArtifact {
                            root_node_key: focus_key.to_owned(),
                            current_node_key: focus_key.to_owned(),
                            lens: ExplorationLens::Structure,
                            limit: 25,
                            unique: true,
                            frozen_context: false,
                        }),
                    },
                    SavedTrailStep::Comparison {
                        artifact: Box::new(SavedComparisonArtifact {
                            root_node_key: focus_key.to_owned(),
                            left_node_key: focus_key.to_owned(),
                            right_node_key: neighbor_key.to_owned(),
                            active_lens: ExplorationLens::Structure,
                            structure_unique: false,
                            comparison_group: NoteComparisonGroup::Tension,
                            limit: 10,
                            frozen_context: false,
                        }),
                    },
                ],
                cursor: 1,
                detached_step: Some(Box::new(SavedTrailStep::LensView {
                    artifact: Box::new(SavedLensViewArtifact {
                        root_node_key: neighbor_key.to_owned(),
                        current_node_key: neighbor_key.to_owned(),
                        lens: ExplorationLens::Refs,
                        limit: 5,
                        unique: false,
                        frozen_context: true,
                    }),
                })),
            }),
        },
    };

    database.save_exploration_artifact(&structure)?;
    database.save_exploration_artifact(&comparison)?;
    database.save_exploration_artifact(&trail)?;
    Ok(())
}

fn artifact_list_command(root: &str, db: &str, json: bool) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "artifact",
        "list",
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
    ]);
    if json {
        command.arg("--json");
    }
    Ok(command.output()?)
}

fn artifact_id_command(
    root: &str,
    db: &str,
    subcommand: &str,
    artifact_id: &str,
    json: bool,
) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "artifact",
        subcommand,
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
    ]);
    if json {
        command.arg("--json");
    }
    command.arg(artifact_id);
    Ok(command.output()?)
}

#[test]
fn artifact_list_command_lists_saved_artifacts_as_summaries() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_list_command(&root, &db, true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: ListExplorationArtifactsResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.artifacts.len(), 3);
    assert_eq!(parsed.artifacts[0].metadata.title, "Artifact Comparison");
    assert_eq!(
        parsed.artifacts[0].kind,
        ExplorationArtifactKind::Comparison
    );
    assert_eq!(
        parsed.artifacts[1].metadata.artifact_id,
        "artifact/structure"
    );
    assert_eq!(parsed.artifacts[2].kind, ExplorationArtifactKind::Trail);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_list_command_prints_human_summaries() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_list_command(&root, &db, false)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("- Artifact Comparison [comparison]"));
    assert!(stdout.contains("artifact id: artifact/structure"));
    assert!(stdout.contains("summary: Saved trail replay"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_show_command_returns_saved_definition() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_id_command(&root, &db, "show", "artifact/structure", true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: ExplorationArtifactResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.artifact.metadata.artifact_id, "artifact/structure");
    assert_eq!(parsed.artifact.kind(), ExplorationArtifactKind::LensView);
    match parsed.artifact.payload {
        ExplorationArtifactPayload::LensView { artifact } => {
            assert_eq!(artifact.lens, ExplorationLens::Structure);
            assert!(artifact.unique);
            assert_eq!(artifact.limit, 25);
        }
        other => panic!("unexpected saved artifact payload: {other:?}"),
    }
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_show_command_prints_human_saved_definition() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_id_command(&root, &db, "show", "artifact/comparison", false)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("artifact id: artifact/comparison"));
    assert!(stdout.contains("kind: comparison"));
    assert!(stdout.contains("comparison group: tension"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_run_command_executes_saved_live_semantics() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_id_command(&root, &db, "run", "artifact/comparison", true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: ExecuteExplorationArtifactResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.artifact.metadata.artifact_id, "artifact/comparison");
    match parsed.artifact.payload {
        ExecutedExplorationArtifactPayload::Comparison { result, .. } => {
            assert_eq!(result.left_note.title, "Focus");
            assert_eq!(result.right_note.title, "Neighbor");
            assert_eq!(
                result
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
        }
        other => panic!("unexpected executed artifact payload: {other:?}"),
    }
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_run_command_prints_human_executed_trail() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_id_command(&root, &db, "run", "artifact/trail", false)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("artifact id: artifact/trail"));
    assert!(stdout.contains("kind: trail"));
    assert!(stdout.contains("[replay]"));
    assert!(stdout.contains("[step 1]"));
    assert!(stdout.contains("kind: comparison"));
    assert!(stdout.contains("group: all"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_delete_command_acknowledges_and_removes_artifacts() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let delete = artifact_id_command(&root, &db, "delete", "artifact/structure", true)?;
    assert!(delete.status.success(), "{delete:?}");
    let parsed: DeleteExplorationArtifactResult = serde_json::from_slice(&delete.stdout)?;
    assert_eq!(parsed.artifact_id, "artifact/structure");

    let listed = artifact_list_command(&root, &db, true)?;
    assert!(listed.status.success(), "{listed:?}");
    let parsed_list: ListExplorationArtifactsResult = serde_json::from_slice(&listed.stdout)?;
    assert_eq!(parsed_list.artifacts.len(), 2);
    assert!(
        parsed_list
            .artifacts
            .iter()
            .all(|artifact| artifact.metadata.artifact_id != "artifact/structure")
    );

    Ok(())
}

#[test]
fn artifact_delete_command_prints_human_acknowledgement() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_id_command(&root, &db, "delete", "artifact/trail", false)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert_eq!(stdout, "deleted artifact: artifact/trail\n");
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_show_command_reports_missing_artifacts() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_id_command(&root, &db, "show", "missing-artifact", true)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("unknown exploration artifact: missing-artifact")
    );

    Ok(())
}

#[test]
fn artifact_run_command_reports_missing_artifacts() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_id_command(&root, &db, "run", "missing-artifact", true)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("unknown exploration artifact: missing-artifact")
    );

    Ok(())
}

#[test]
fn artifact_delete_command_reports_missing_artifacts() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_id_command(&root, &db, "delete", "missing-artifact", true)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("unknown exploration artifact: missing-artifact")
    );

    Ok(())
}

#[test]
fn artifact_delete_command_reports_invalid_artifact_ids() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = artifact_id_command(&root, &db, "delete", " artifact/structure ", true)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("artifact_id must not have leading or trailing whitespace")
    );

    Ok(())
}
