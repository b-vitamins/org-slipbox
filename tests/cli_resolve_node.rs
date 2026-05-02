use std::fs;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::NodeRecord;
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
        root.join("alpha.org"),
        ":PROPERTIES:\n:ID: alpha-id\n:ROAM_ALIASES: Apex\n:ROAM_REFS: cite:alpha2024\n:END:\n#+title: Alpha\n",
    )?;
    fs::write(
        root.join("beta.org"),
        ":PROPERTIES:\n:ID: beta-id\n:ROAM_REFS: cite:beta2024\n:END:\n#+title: Beta\n",
    )?;
    fs::write(root.join("shared-one.org"), "#+title: Shared Title\n")?;
    fs::write(root.join("shared-two.org"), "#+title: Shared Title\n")?;

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

fn resolve_node_command(
    root: &str,
    db: &str,
    target_args: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec![
        "resolve-node",
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
        "--json",
    ];
    args.extend_from_slice(target_args);
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

#[test]
fn resolve_node_command_resolves_exact_id() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = resolve_node_command(&root, &db, &["--id", "alpha-id"])?;

    assert!(output.status.success(), "{output:?}");
    let parsed: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.node_key, "file:alpha.org");
    assert_eq!(parsed.explicit_id.as_deref(), Some("alpha-id"));
    assert_eq!(parsed.title, "Alpha");
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn resolve_node_command_resolves_exact_alias() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = resolve_node_command(&root, &db, &["--title", "Apex"])?;

    assert!(output.status.success(), "{output:?}");
    let parsed: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.node_key, "file:alpha.org");
    assert_eq!(parsed.title, "Alpha");
    assert_eq!(parsed.aliases, vec!["Apex"]);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn resolve_node_command_resolves_exact_ref() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = resolve_node_command(&root, &db, &["--ref", "cite:alpha2024"])?;

    assert!(output.status.success(), "{output:?}");
    let parsed: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.node_key, "file:alpha.org");
    assert_eq!(parsed.refs, vec!["@alpha2024"]);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn resolve_node_command_resolves_exact_node_key() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = resolve_node_command(&root, &db, &["--key", "file:alpha.org"])?;

    assert!(output.status.success(), "{output:?}");
    let parsed: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.node_key, "file:alpha.org");
    assert_eq!(parsed.title, "Alpha");
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn resolve_node_command_reports_ambiguous_titles() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = resolve_node_command(&root, &db, &["--title", "Shared Title"])?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("multiple nodes match Shared Title")
    );

    Ok(())
}

#[test]
fn resolve_node_command_reports_unknown_ids() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let output = resolve_node_command(&root, &db, &["--id", "missing-id"])?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(parsed.error.message.contains("unknown node id: missing-id"));

    Ok(())
}
