use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{IndexFileResult, IndexStats, IndexedFilesResult, SearchFilesResult};
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

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

fn build_fixture() -> Result<(TempDir, PathBuf, PathBuf)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(root.join("archive"))?;
    fs::write(root.join("alpha.org"), "#+title: Alpha File\n")?;
    fs::write(root.join("beta.org"), "#+title: Beta File\n\n* Beta Task\n")?;
    fs::write(root.join("archive").join("hidden.org"), "#+title: Hidden\n")?;
    let db = workspace.path().join("slipbox.sqlite");
    Ok((workspace, root, db))
}

fn index_fixture(root: &Path, db: &Path) -> Result<()> {
    let files = scan_root(root)?;
    let mut database = Database::open(db)?;
    database.sync_index(&files)?;
    Ok(())
}

fn scoped_args(root: &Path, db: &Path) -> Vec<String> {
    vec![
        "--root".to_owned(),
        root.display().to_string(),
        "--db".to_owned(),
        db.display().to_string(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--file-extension".to_owned(),
        "org".to_owned(),
    ]
}

fn run_slipbox(args: &[String]) -> Result<std::process::Output> {
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

#[test]
fn sync_root_refreshes_discovered_files_as_json() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let mut args = vec!["sync".to_owned(), "root".to_owned()];
    args.extend(scoped_args(&root, &db));
    args.extend([
        "--exclude-regexp".to_owned(),
        "^archive/".to_owned(),
        "--json".to_owned(),
    ]);

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: IndexStats = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.files_indexed, 2);
    assert_eq!(parsed.nodes_indexed, 3);
    assert_eq!(parsed.links_indexed, 0);
    assert!(output.stderr.is_empty());

    let mut list_args = vec!["file".to_owned(), "list".to_owned()];
    list_args.extend(scoped_args(&root, &db));
    list_args.push("--json".to_owned());
    let list_output = run_slipbox(&list_args)?;
    assert!(list_output.status.success(), "{list_output:?}");
    let files: IndexedFilesResult = serde_json::from_slice(&list_output.stdout)?;
    assert_eq!(
        files.files,
        vec!["alpha.org".to_owned(), "beta.org".to_owned()]
    );

    Ok(())
}

#[test]
fn sync_file_removes_excluded_file_without_full_root_prune() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    index_fixture(&root, &db)?;
    fs::remove_file(root.join("beta.org"))?;

    let mut args = vec![
        "sync".to_owned(),
        "file".to_owned(),
        "archive/hidden.org".to_owned(),
    ];
    args.extend(scoped_args(&root, &db));
    args.extend([
        "--exclude-regexp".to_owned(),
        "^archive/".to_owned(),
        "--json".to_owned(),
    ]);

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let refreshed: IndexFileResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(refreshed.file_path, "archive/hidden.org");
    assert!(output.stderr.is_empty());

    let mut list_args = vec!["file".to_owned(), "list".to_owned()];
    list_args.extend(scoped_args(&root, &db));
    list_args.push("--json".to_owned());
    let list_output = run_slipbox(&list_args)?;
    assert!(list_output.status.success(), "{list_output:?}");
    let files: IndexedFilesResult = serde_json::from_slice(&list_output.stdout)?;
    assert_eq!(
        files.files,
        vec!["alpha.org".to_owned(), "beta.org".to_owned()]
    );

    Ok(())
}

#[test]
fn file_list_prints_compact_human_output() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    index_fixture(&root, &db)?;

    let mut args = vec!["file".to_owned(), "list".to_owned()];
    args.extend(scoped_args(&root, &db));
    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("indexed files: 3"));
    assert!(stdout.contains("- alpha.org"));
    assert!(stdout.contains("- archive/hidden.org"));
    assert!(stdout.contains("- beta.org"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn file_search_emits_canonical_json_result() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    index_fixture(&root, &db)?;

    let mut args = vec!["file".to_owned(), "search".to_owned(), "Alpha".to_owned()];
    args.extend(scoped_args(&root, &db));
    args.push("--json".to_owned());
    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: SearchFilesResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.files.len(), 1);
    assert_eq!(parsed.files[0].file_path, "alpha.org");
    assert_eq!(parsed.files[0].title, "Alpha File");
    assert_eq!(parsed.files[0].node_count, 1);
    assert!(parsed.files[0].mtime_ns > 0);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn sync_and_file_commands_report_daemon_failures_as_json() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    for command in [
        vec!["sync".to_owned(), "root".to_owned()],
        vec!["file".to_owned(), "list".to_owned()],
    ] {
        let mut args = command;
        args.extend([
            "--root".to_owned(),
            root.display().to_string(),
            "--db".to_owned(),
            db.display().to_string(),
            "--server-program".to_owned(),
            "/definitely/not/a/real/slipbox-binary".to_owned(),
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
                .contains("failed to start slipbox daemon")
        );
    }

    Ok(())
}
