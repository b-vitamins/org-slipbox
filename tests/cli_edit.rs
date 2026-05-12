use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::Value;
use slipbox_core::{
    NodeKind, StructuralWriteOperationKind, StructuralWriteReport, StructuralWriteResult,
};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::{TempDir, tempdir};

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

struct EditFixture {
    _workspace: TempDir,
    root: PathBuf,
    db: PathBuf,
    main_source: String,
}

fn build_edit_fixture() -> Result<EditFixture> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    let main_source = r#":PROPERTIES:
:ID: main-id
:END:
#+title: Main

* Region Source
:PROPERTIES:
:ID: region-source-id
:END:
Region body.

* Region Target
:PROPERTIES:
:ID: region-target-id
:END:
Target body.

* Extract Source
:PROPERTIES:
:ID: extract-source-id
:END:
Extract body.
"#
    .to_owned();
    fs::write(root.join("main.org"), &main_source)?;
    fs::write(
        root.join("cross.org"),
        r#":PROPERTIES:
:ID: cross-id
:END:
#+title: Cross

* Cross Source
:PROPERTIES:
:ID: cross-source-id
:END:
Cross body.
"#,
    )?;
    fs::write(
        root.join("demote.org"),
        r#":PROPERTIES:
:ID: demote-file-id
:END:
#+title: Demote Me

Demote body.
"#,
    )?;

    let db = workspace.path().join("slipbox.sqlite");
    let files = scan_root(&root)?;
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;

    Ok(EditFixture {
        _workspace: workspace,
        root,
        db,
        main_source,
    })
}

fn edit_command(root: &Path, db: &Path, args: &[String]) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.arg("edit");
    command.args(args);
    command.arg("--root").arg(root);
    command.arg("--db").arg(db);
    command.output().context("failed to run slipbox edit")
}

fn edit_json_command(root: &Path, db: &Path, args: &[String]) -> Result<std::process::Output> {
    let mut command_args = args.to_vec();
    command_args.push("--json".to_owned());
    edit_command(root, db, &command_args)
}

fn edit_json_command_in_dir(
    cwd: &Path,
    root: &Path,
    db: &Path,
    args: &[String],
) -> Result<std::process::Output> {
    let mut command_args = args.to_vec();
    command_args.push("--json".to_owned());
    let mut command = Command::new(slipbox_binary());
    command.current_dir(cwd);
    command.arg("edit");
    command.args(&command_args);
    command.arg("--root").arg(root);
    command.arg("--db").arg(db);
    command.output().context("failed to run slipbox edit")
}

fn parse_report(output: std::process::Output) -> Result<StructuralWriteReport> {
    assert!(output.status.success(), "{output:?}");
    let report: StructuralWriteReport = serde_json::from_slice(&output.stdout)?;
    assert!(report.validation_error().is_none());
    Ok(report)
}

fn assert_error_failure(output: &std::process::Output, expected: &str) -> Result<()> {
    assert!(!output.status.success(), "{output:?}");
    let payload: Value = serde_json::from_slice(&output.stderr)?;
    let message = payload["error"]["message"]
        .as_str()
        .context("structured error message should be present")?;
    assert!(
        message.contains(expected),
        "expected {message:?} to contain {expected:?}"
    );
    Ok(())
}

fn region_char_range(source: &str) -> Result<(u32, u32)> {
    let start = source
        .find("* Region Source")
        .context("region source heading should exist")?;
    let end = source
        .find("\n* Region Target")
        .context("region target heading should follow source")?;
    Ok((
        source[..start].chars().count() as u32 + 1,
        source[..end].chars().count() as u32 + 1,
    ))
}

#[test]
fn edit_commands_return_structural_write_reports() -> Result<()> {
    let fixture = build_edit_fixture()?;
    let (region_start, region_end) = region_char_range(&fixture.main_source)?;

    let region_report = parse_report(edit_json_command(
        &fixture.root,
        &fixture.db,
        &[
            "refile-region".to_owned(),
            "--file".to_owned(),
            "main.org".to_owned(),
            "--start".to_owned(),
            region_start.to_string(),
            "--end".to_owned(),
            region_end.to_string(),
            "--target-id".to_owned(),
            "region-target-id".to_owned(),
        ],
    )?)?;
    assert_eq!(
        region_report.operation,
        StructuralWriteOperationKind::RefileRegion
    );
    assert_eq!(
        region_report.affected_files.changed_files,
        vec!["main.org".to_owned()]
    );
    assert!(region_report.result.is_none());

    let subtree_report = parse_report(edit_json_command(
        &fixture.root,
        &fixture.db,
        &[
            "refile-subtree".to_owned(),
            "--source-id".to_owned(),
            "cross-source-id".to_owned(),
            "--target-id".to_owned(),
            "region-target-id".to_owned(),
        ],
    )?)?;
    assert_eq!(
        subtree_report.operation,
        StructuralWriteOperationKind::RefileSubtree
    );
    assert!(
        subtree_report
            .affected_files
            .changed_files
            .contains(&"main.org".to_owned())
    );
    assert!(
        subtree_report
            .affected_files
            .changed_files
            .contains(&"cross.org".to_owned())
    );
    let Some(StructuralWriteResult::Node { node }) = subtree_report.result else {
        panic!("refile subtree should return node result");
    };
    assert_eq!(node.title, "Cross Source");
    assert_eq!(node.outline_path, "Region Target / Cross Source");

    let extract_report = parse_report(edit_json_command(
        &fixture.root,
        &fixture.db,
        &[
            "extract-subtree".to_owned(),
            "--source-id".to_owned(),
            "extract-source-id".to_owned(),
            "--file".to_owned(),
            "extracted.org".to_owned(),
        ],
    )?)?;
    assert_eq!(
        extract_report.operation,
        StructuralWriteOperationKind::ExtractSubtree
    );
    let Some(StructuralWriteResult::Node { node }) = extract_report.result else {
        panic!("extract subtree should return node result");
    };
    assert_eq!(node.kind, NodeKind::File);
    assert_eq!(node.file_path, "extracted.org");

    let demote_report = parse_report(edit_json_command(
        &fixture.root,
        &fixture.db,
        &[
            "demote-file".to_owned(),
            "--file".to_owned(),
            "demote.org".to_owned(),
        ],
    )?)?;
    assert_eq!(
        demote_report.operation,
        StructuralWriteOperationKind::DemoteFile
    );
    let Some(StructuralWriteResult::Anchor { anchor }) = demote_report.result else {
        panic!("demote file should return anchor result");
    };
    assert_eq!(anchor.kind, NodeKind::Heading);
    assert_eq!(anchor.title, "Demote Me");

    let promote_report = parse_report(edit_json_command(
        &fixture.root,
        &fixture.db,
        &[
            "promote-file".to_owned(),
            "--file".to_owned(),
            "demote.org".to_owned(),
        ],
    )?)?;
    assert_eq!(
        promote_report.operation,
        StructuralWriteOperationKind::PromoteFile
    );
    let Some(StructuralWriteResult::Node { node }) = promote_report.result else {
        panic!("promote file should return node result");
    };
    assert_eq!(node.kind, NodeKind::File);
    assert_eq!(node.file_path, "demote.org");

    Ok(())
}

#[test]
fn edit_file_paths_accept_absolute_paths_under_relative_root() -> Result<()> {
    let fixture = build_edit_fixture()?;

    let output = edit_json_command_in_dir(
        fixture
            .root
            .parent()
            .context("fixture root should have parent")?,
        Path::new("notes"),
        &fixture.db,
        &[
            "demote-file".to_owned(),
            "--file".to_owned(),
            fixture.root.join("demote.org").display().to_string(),
        ],
    )?;
    let report = parse_report(output)?;

    assert_eq!(report.operation, StructuralWriteOperationKind::DemoteFile);
    assert_eq!(
        report.affected_files.changed_files,
        vec!["demote.org".to_owned()]
    );
    let Some(StructuralWriteResult::Anchor { anchor }) = report.result else {
        panic!("demote file should return anchor result");
    };
    assert_eq!(anchor.title, "Demote Me");

    Ok(())
}

#[test]
fn edit_file_paths_accept_absolute_paths_under_parent_relative_root() -> Result<()> {
    let fixture = build_edit_fixture()?;
    let workspace = fixture
        .root
        .parent()
        .context("fixture root should have parent")?;
    let command_dir = workspace.join("sub");
    fs::create_dir_all(&command_dir)?;

    let output = edit_json_command_in_dir(
        &command_dir,
        Path::new("../notes"),
        &fixture.db,
        &[
            "demote-file".to_owned(),
            "--file".to_owned(),
            fixture.root.join("demote.org").display().to_string(),
        ],
    )?;
    let report = parse_report(output)?;

    assert_eq!(report.operation, StructuralWriteOperationKind::DemoteFile);
    assert_eq!(
        report.affected_files.changed_files,
        vec!["demote.org".to_owned()]
    );

    Ok(())
}

#[test]
fn edit_commands_print_factual_human_report_summaries() -> Result<()> {
    let fixture = build_edit_fixture()?;

    let output = edit_command(
        &fixture.root,
        &fixture.db,
        &[
            "demote-file".to_owned(),
            "--file".to_owned(),
            "demote.org".to_owned(),
        ],
    )?;
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("operation: demote-file"));
    assert!(stdout.contains("index refresh: refreshed"));
    assert!(stdout.contains("changed files: 1"));
    assert!(stdout.contains("- demote.org"));
    assert!(stdout.contains("result: anchor"));
    assert!(stdout.contains("anchor key: heading:demote.org:1"));

    Ok(())
}

#[test]
fn edit_commands_report_structured_json_failures() -> Result<()> {
    let fixture = build_edit_fixture()?;
    let (region_start, _region_end) = region_char_range(&fixture.main_source)?;

    let unknown_source = edit_json_command(
        &fixture.root,
        &fixture.db,
        &[
            "refile-subtree".to_owned(),
            "--source-key".to_owned(),
            "heading:missing.org:1".to_owned(),
            "--target-id".to_owned(),
            "region-target-id".to_owned(),
        ],
    )?;
    assert_error_failure(
        &unknown_source,
        "unknown source node: heading:missing.org:1",
    )?;

    let same_target = edit_json_command(
        &fixture.root,
        &fixture.db,
        &[
            "refile-subtree".to_owned(),
            "--source-id".to_owned(),
            "region-target-id".to_owned(),
            "--target-id".to_owned(),
            "region-target-id".to_owned(),
        ],
    )?;
    assert_error_failure(&same_target, "source and target nodes must be different")?;

    let bad_range = edit_json_command(
        &fixture.root,
        &fixture.db,
        &[
            "refile-region".to_owned(),
            "--file".to_owned(),
            "main.org".to_owned(),
            "--start".to_owned(),
            region_start.to_string(),
            "--end".to_owned(),
            region_start.to_string(),
            "--target-id".to_owned(),
            "region-target-id".to_owned(),
        ],
    )?;
    assert_error_failure(&bad_range, "active region range must not be empty")?;

    let unsafe_path = edit_json_command(
        &fixture.root,
        &fixture.db,
        &[
            "demote-file".to_owned(),
            "--file".to_owned(),
            "../outside.org".to_owned(),
        ],
    )?;
    assert_error_failure(&unsafe_path, "edit file path must stay within --root")?;

    Ok(())
}
