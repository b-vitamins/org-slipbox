use std::fs;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::GraphResult;
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
struct GraphDotFileResult {
    output_path: String,
    format: String,
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
        ":PROPERTIES:\n:ID: alpha-id\n:END:\n#+title: Alpha\nSee [[id:beta-id][Beta]].\n",
    )?;
    fs::write(
        root.join("beta.org"),
        ":PROPERTIES:\n:ID: beta-id\n:END:\n#+title: Beta\nSee [[id:gamma-id][Gamma]].\n",
    )?;
    fs::write(
        root.join("gamma.org"),
        ":PROPERTIES:\n:ID: gamma-id\n:END:\n#+title: Gamma\n",
    )?;
    fs::write(root.join("orphan.org"), "#+title: Orphan\n")?;

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
    ]
}

fn run_slipbox(args: &[String]) -> Result<std::process::Output> {
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

#[test]
fn graph_dot_command_emits_global_dot_to_stdout() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;
    let mut args = vec!["graph".to_owned(), "dot".to_owned()];
    args.extend(scoped_args(&root, &db));
    args.push("--include-orphans".to_owned());

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("digraph \"org-slipbox\""));
    assert!(stdout.contains("\"file:alpha.org\" -> \"file:beta.org\";"));
    assert!(stdout.contains("\"file:orphan.org\" [label=\"Orphan\""));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn graph_dot_command_supports_neighborhood_json_and_graph_options() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;
    let mut args = vec!["graph".to_owned(), "dot".to_owned()];
    args.extend(scoped_args(&root, &db));
    args.extend([
        "--root-node-key".to_owned(),
        "file:alpha.org".to_owned(),
        "--max-distance".to_owned(),
        "1".to_owned(),
        "--shorten-titles".to_owned(),
        "truncate".to_owned(),
        "--max-title-length".to_owned(),
        "8".to_owned(),
        "--node-url-prefix".to_owned(),
        "org-protocol://roam-node?node=".to_owned(),
        "--json".to_owned(),
    ]);

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let result: GraphResult = serde_json::from_slice(&output.stdout)?;
    assert!(result.dot.contains("\"file:alpha.org\" [label=\"Alpha\""));
    assert!(result.dot.contains("\"file:beta.org\" [label=\"Beta\""));
    assert!(!result.dot.contains("\"file:gamma.org\" [label=\"Gamma\""));
    assert!(
        result
            .dot
            .contains("URL=\"org-protocol://roam-node?node=alpha-id\"")
    );
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn graph_dot_command_writes_dot_to_file_and_reports_ack() -> Result<()> {
    let (workspace, root, db) = build_indexed_fixture()?;
    let output_path = workspace.path().join("graph.dot");
    let mut args = vec!["graph".to_owned(), "dot".to_owned()];
    args.extend(scoped_args(&root, &db));
    args.extend([
        "--include-orphans".to_owned(),
        "--output".to_owned(),
        output_path.display().to_string(),
        "--json".to_owned(),
    ]);

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let ack: GraphDotFileResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(ack.output_path, output_path.display().to_string());
    assert_eq!(ack.format, "dot");
    let dot = fs::read_to_string(&output_path)?;
    assert!(dot.contains("\"file:alpha.org\" -> \"file:beta.org\";"));
    assert!(dot.contains("\"file:orphan.org\" [label=\"Orphan\""));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn graph_dot_command_reports_unsupported_link_types_as_json_errors() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;
    let mut args = vec!["graph".to_owned(), "dot".to_owned()];
    args.extend(scoped_args(&root, &db));
    args.extend([
        "--hide-link-type".to_owned(),
        "file".to_owned(),
        "--json".to_owned(),
    ]);

    let output = run_slipbox(&args)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("unsupported graph link type filter: file")
    );

    Ok(())
}
