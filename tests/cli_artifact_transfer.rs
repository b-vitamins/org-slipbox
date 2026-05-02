use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{
    ExplorationArtifactKind, ExplorationArtifactPayload, SaveExplorationArtifactResult,
    SavedExplorationArtifact, SavedLensViewArtifact,
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

fn build_seeded_fixture() -> Result<(tempfile::TempDir, String, String, SavedExplorationArtifact)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("focus.org"),
        r#"#+title: Focus

* Focus Node
:PROPERTIES:
:ID: focus-id
:ROAM_REFS: cite:focus2024
:END:
Focus body.
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
    let artifact = SavedExplorationArtifact {
        metadata: slipbox_core::ExplorationArtifactMetadata {
            artifact_id: "artifact/exportable".to_owned(),
            title: "Exportable Artifact".to_owned(),
            summary: Some("Round-trip me".to_owned()),
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: focus_key.clone(),
                current_node_key: focus_key,
                lens: slipbox_core::ExplorationLens::Structure,
                limit: 17,
                unique: true,
                frozen_context: false,
            }),
        },
    };
    database.save_exploration_artifact(&artifact)?;

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
        artifact,
    ))
}

fn build_empty_fixture() -> Result<(tempfile::TempDir, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("focus.org"),
        r#"#+title: Focus

* Focus Node
:PROPERTIES:
:ID: focus-id
:END:
Focus body.
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

fn artifact_export_command(
    root: &str,
    db: &str,
    artifact_id: &str,
    extra_args: &[&str],
    json: bool,
) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "artifact",
        "export",
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
    command.args(extra_args);
    Ok(command.output()?)
}

fn artifact_import_command(
    root: &str,
    db: &str,
    input: &str,
    extra_args: &[&str],
    json: bool,
) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "artifact",
        "import",
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
    command.args(extra_args);
    command.arg(input);
    Ok(command.output()?)
}

#[test]
fn artifact_export_command_writes_saved_artifact_json_to_stdout() -> Result<()> {
    let (_workspace, root, db, artifact) = build_seeded_fixture()?;

    let output = artifact_export_command(&root, &db, "artifact/exportable", &[], true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: SavedExplorationArtifact = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed, artifact);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_export_command_writes_to_file_and_reports_destination() -> Result<()> {
    let (_workspace, root, db, artifact) = build_seeded_fixture()?;
    let export_dir = tempdir()?;
    let output_path = export_dir.path().join("artifact.json");

    let output = artifact_export_command(
        &root,
        &db,
        "artifact/exportable",
        &["--output", output_path.to_str().expect("utf-8 path")],
        false,
    )?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("exported artifact: artifact/exportable ->"));
    let parsed: SavedExplorationArtifact =
        serde_json::from_slice(&fs::read(output_path).expect("export file should exist"))?;
    assert_eq!(parsed, artifact);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_import_command_round_trips_from_exported_file() -> Result<()> {
    let (_seeded_workspace, seeded_root, seeded_db, artifact) = build_seeded_fixture()?;
    let export_dir = tempdir()?;
    let output_path = export_dir.path().join("artifact.json");
    let export = artifact_export_command(
        &seeded_root,
        &seeded_db,
        "artifact/exportable",
        &["--output", output_path.to_str().expect("utf-8 path")],
        false,
    )?;
    assert!(export.status.success(), "{export:?}");

    let (_import_workspace, import_root, import_db) = build_empty_fixture()?;
    let import = artifact_import_command(
        &import_root,
        &import_db,
        output_path.to_str().expect("utf-8 path"),
        &[],
        true,
    )?;

    assert!(import.status.success(), "{import:?}");
    let parsed: SaveExplorationArtifactResult = serde_json::from_slice(&import.stdout)?;
    assert_eq!(parsed.artifact.metadata, artifact.metadata);
    assert_eq!(parsed.artifact.kind, ExplorationArtifactKind::LensView);
    let imported =
        Database::open(Path::new(&import_db))?.exploration_artifact("artifact/exportable")?;
    assert_eq!(imported, Some(artifact));
    assert!(import.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_import_command_reads_json_from_stdin() -> Result<()> {
    let (_workspace, root, db, artifact) = build_seeded_fixture()?;
    let json = serde_json::to_vec(&artifact)?;
    let (_import_workspace, import_root, import_db) = build_empty_fixture()?;

    let mut child = Command::new(slipbox_binary())
        .args([
            "artifact",
            "import",
            "--root",
            &import_root,
            "--db",
            &import_db,
            "--server-program",
            slipbox_binary(),
            "--json",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(&json)?;
    drop(child.stdin.take());
    let output = child.wait_with_output()?;

    assert!(output.status.success(), "{output:?}");
    let parsed: SaveExplorationArtifactResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.artifact.metadata.artifact_id, "artifact/exportable");
    assert!(output.stderr.is_empty());

    let imported =
        Database::open(Path::new(&import_db))?.exploration_artifact("artifact/exportable")?;
    assert_eq!(imported, Some(artifact));
    let _ = (root, db); // keep seeded fixture alive until after assertion

    Ok(())
}

#[test]
fn artifact_import_command_refuses_overwrite_without_flag() -> Result<()> {
    let (_workspace, root, db, artifact) = build_seeded_fixture()?;
    let export_dir = tempdir()?;
    let output_path = export_dir.path().join("artifact.json");
    fs::write(&output_path, serde_json::to_vec_pretty(&artifact)?)?;

    let output = artifact_import_command(
        &root,
        &db,
        output_path.to_str().expect("utf-8 path"),
        &[],
        true,
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("exploration artifact already exists: artifact/exportable")
    );

    Ok(())
}

#[test]
fn artifact_import_command_overwrites_when_explicitly_requested() -> Result<()> {
    let (_workspace, root, db, mut artifact) = build_seeded_fixture()?;
    artifact.metadata.title = "Updated Exportable Artifact".to_owned();
    artifact.metadata.summary = Some("Updated by import".to_owned());
    let export_dir = tempdir()?;
    let output_path = export_dir.path().join("artifact.json");
    fs::write(&output_path, serde_json::to_vec_pretty(&artifact)?)?;

    let output = artifact_import_command(
        &root,
        &db,
        output_path.to_str().expect("utf-8 path"),
        &["--overwrite"],
        false,
    )?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("imported artifact: artifact/exportable [lens-view]"));
    let imported = Database::open(Path::new(&db))?.exploration_artifact("artifact/exportable")?;
    assert_eq!(imported, Some(artifact));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn artifact_import_command_reports_malformed_json() -> Result<()> {
    let (_workspace, root, db) = build_empty_fixture()?;
    let input_dir = tempdir()?;
    let input_path = input_dir.path().join("broken.json");
    fs::write(&input_path, b"{ not valid json")?;

    let output = artifact_import_command(
        &root,
        &db,
        input_path.to_str().expect("utf-8 path"),
        &[],
        true,
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("failed to parse saved exploration artifact JSON")
    );

    Ok(())
}

#[test]
fn artifact_import_command_reports_invalid_artifacts_without_normalizing() -> Result<()> {
    let (_workspace, root, db) = build_empty_fixture()?;
    let invalid = serde_json::json!({
        "artifact_id": " artifact/exportable ",
        "title": "Invalid Artifact",
        "kind": "lens-view",
        "root_node_key": "file:focus.org",
        "current_node_key": "file:focus.org",
        "lens": "structure",
        "limit": 10,
        "unique": false,
        "frozen_context": false
    });
    let input_dir = tempdir()?;
    let input_path = input_dir.path().join("invalid.json");
    fs::write(&input_path, serde_json::to_vec_pretty(&invalid)?)?;

    let output = artifact_import_command(
        &root,
        &db,
        input_path.to_str().expect("utf-8 path"),
        &[],
        true,
    )?;

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
