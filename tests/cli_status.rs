use std::fs;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[derive(Debug, Deserialize)]
struct StatusInfo {
    version: String,
    root: String,
    db: String,
    files_indexed: u64,
    nodes_indexed: u64,
    links_indexed: u64,
}

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
        root.join("alpha.org"),
        "#+title: Alpha\nSee [[id:beta-id][Beta]].\n",
    )?;
    fs::write(
        root.join("beta.org"),
        ":PROPERTIES:\n:ID: beta-id\n:END:\n#+title: Beta\n",
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

#[test]
fn status_command_prints_human_output_over_shared_runtime() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = Command::new(slipbox_binary())
        .args([
            "status",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            slipbox_binary(),
            "--file-extension",
            "org",
            "--exclude-regexp",
            "^$",
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("version: 0.9.0"));
    assert!(stdout.contains(&format!("root: {}", fs::canonicalize(&root)?.display())));
    assert!(stdout.contains(&format!("db: {db}")));
    assert!(stdout.contains("files indexed: 2"));
    assert!(stdout.contains("nodes indexed: 2"));
    assert!(stdout.contains("links indexed: 1"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn status_command_supports_json_output() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = Command::new(slipbox_binary())
        .args([
            "status",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            slipbox_binary(),
            "--json",
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    let parsed: StatusInfo = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.files_indexed, 2);
    assert_eq!(parsed.nodes_indexed, 2);
    assert_eq!(parsed.links_indexed, 1);
    assert_eq!(parsed.root, fs::canonicalize(&root)?.display().to_string());
    assert_eq!(parsed.db, db);
    assert_eq!(parsed.version, "0.9.0");
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn status_command_reports_connection_failures_with_json_error_and_exit_one() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = Command::new(slipbox_binary())
        .args([
            "status",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            "/definitely/not/a/real/slipbox-binary",
            "--json",
        ])
        .output()?;

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
