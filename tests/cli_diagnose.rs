use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{
    FileDiagnosticIssue, FileDiagnosticsResult, IndexDiagnosticsResult, NodeDiagnosticsResult,
};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::{TempDir, tempdir};

#[derive(Debug, Deserialize)]
struct ErrorPayload {
    error: ErrorMessage,
}

#[derive(Debug, Deserialize)]
struct ErrorMessage {
    message: String,
}

struct DiagnoseFixture {
    _workspace: TempDir,
    root: PathBuf,
    db: PathBuf,
}

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

fn build_diagnose_fixture() -> Result<DiagnoseFixture> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("healthy.org"),
        r#":PROPERTIES:
:ID: healthy-id
:END:
#+title: Healthy

Healthy body.
"#,
    )?;
    fs::write(root.join("excluded.org"), "#+title: Excluded\n")?;
    fs::write(root.join("stale.org"), "#+title: Stale\n")?;

    let db = workspace.path().join("slipbox.sqlite");
    let files = scan_root(&root)?;
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;

    fs::remove_file(root.join("stale.org"))?;
    fs::write(root.join("new.org"), "#+title: New\n")?;
    fs::write(root.join("readme.md"), "# Readme\n")?;

    Ok(DiagnoseFixture {
        _workspace: workspace,
        root,
        db,
    })
}

fn scoped_args(root: &Path, db: &Path) -> Vec<String> {
    vec![
        "--root".to_owned(),
        root.display().to_string(),
        "--db".to_owned(),
        db.display().to_string(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--json".to_owned(),
    ]
}

fn diagnose_command(
    root: &Path,
    db: &Path,
    subcommand: &str,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = vec!["diagnose".to_owned(), subcommand.to_owned()];
    args.extend(scoped_args(root, db));
    args.extend_from_slice(extra_args);
    args
}

fn diagnose_command_with_exclude(
    root: &Path,
    db: &Path,
    subcommand: &str,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = diagnose_command(root, db, subcommand, extra_args);
    args.extend([
        "--file-extension".to_owned(),
        "org".to_owned(),
        "--exclude-regexp".to_owned(),
        "excluded\\.org$".to_owned(),
    ]);
    args
}

fn run_slipbox(args: &[String]) -> Result<std::process::Output> {
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

#[test]
fn diagnose_file_reports_eligibility_and_index_state() -> Result<()> {
    let fixture = build_diagnose_fixture()?;
    let new_file = fixture.root.join("new.org");
    let args = diagnose_command(
        &fixture.root,
        &fixture.db,
        "file",
        &["--file".to_owned(), new_file.display().to_string()],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let result: FileDiagnosticsResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(result.diagnostic.file_path, "new.org");
    assert!(result.diagnostic.exists);
    assert!(result.diagnostic.eligible);
    assert!(!result.diagnostic.indexed);
    assert_eq!(
        result.diagnostic.issues,
        vec![FileDiagnosticIssue::MissingFromIndex]
    );

    let readme_args = diagnose_command(
        &fixture.root,
        &fixture.db,
        "file",
        &["--file".to_owned(), "readme.md".to_owned()],
    );
    let readme_output = run_slipbox(&readme_args)?;
    assert!(readme_output.status.success(), "{readme_output:?}");
    let readme: FileDiagnosticsResult = serde_json::from_slice(&readme_output.stdout)?;
    assert_eq!(readme.diagnostic.file_path, "readme.md");
    assert!(readme.diagnostic.exists);
    assert!(!readme.diagnostic.eligible);
    assert!(!readme.diagnostic.indexed);
    assert!(readme.diagnostic.issues.is_empty());

    Ok(())
}

#[test]
fn diagnose_node_reports_source_file_and_line_state() -> Result<()> {
    let fixture = build_diagnose_fixture()?;
    let args = diagnose_command(
        &fixture.root,
        &fixture.db,
        "node",
        &["--id".to_owned(), "healthy-id".to_owned()],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let result: NodeDiagnosticsResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(result.diagnostic.node.node_key, "file:healthy.org");
    assert_eq!(result.diagnostic.file.file_path, "healthy.org");
    assert!(result.diagnostic.file.indexed);
    assert!(result.diagnostic.line_present);
    assert!(result.diagnostic.file.issues.is_empty());
    assert!(result.diagnostic.issues.is_empty());

    Ok(())
}

#[test]
fn diagnose_index_reports_eligible_indexed_drift() -> Result<()> {
    let fixture = build_diagnose_fixture()?;
    let args = diagnose_command_with_exclude(&fixture.root, &fixture.db, "index", &[]);

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let result: IndexDiagnosticsResult = serde_json::from_slice(&output.stdout)?;
    assert!(
        result
            .diagnostic
            .eligible_files
            .contains(&"healthy.org".to_owned())
    );
    assert!(
        result
            .diagnostic
            .eligible_files
            .contains(&"new.org".to_owned())
    );
    assert!(
        !result
            .diagnostic
            .eligible_files
            .contains(&"excluded.org".to_owned())
    );
    assert_eq!(result.diagnostic.missing_from_index, vec!["new.org"]);
    assert_eq!(result.diagnostic.indexed_but_missing, vec!["stale.org"]);
    assert_eq!(
        result.diagnostic.indexed_but_ineligible,
        vec!["excluded.org"]
    );
    assert!(result.diagnostic.status_consistent);
    assert!(!result.diagnostic.index_current);

    Ok(())
}

#[test]
fn diagnose_commands_report_structured_json_failures() -> Result<()> {
    let fixture = build_diagnose_fixture()?;
    let args = diagnose_command(
        &fixture.root,
        &fixture.db,
        "node",
        &["--key".to_owned(), "missing-node".to_owned()],
    );

    let output = run_slipbox(&args)?;

    assert!(!output.status.success(), "{output:?}");
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        error.error.message.contains("unknown diagnostic node"),
        "{}",
        error.error.message
    );

    Ok(())
}
