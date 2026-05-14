use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{AnchorRecord, NodeKind, NodeRecord};
use tempfile::{TempDir, tempdir};

mod support;

use support::{run_slipbox, scoped_server_args_with_file_extension};

#[derive(Debug, Deserialize)]
struct ErrorPayload {
    error: ErrorMessage,
}

#[derive(Debug, Deserialize)]
struct ErrorMessage {
    message: String,
}

fn build_fixture() -> Result<(TempDir, PathBuf, PathBuf)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    Ok((workspace, root, db))
}

fn note_command(root: &Path, db: &Path, subcommand: &str, extra_args: &[String]) -> Vec<String> {
    let mut args = vec!["note".to_owned(), subcommand.to_owned()];
    args.extend(scoped_server_args_with_file_extension(root, db, "org"));
    args.extend_from_slice(extra_args);
    args
}

#[test]
fn note_create_writes_file_note_with_refs_and_json() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let head = "#+title: Captured Note\n#+filetags: :project:\n".to_owned();
    let mut args = note_command(
        &root,
        &db,
        "create",
        &[
            "--title".to_owned(),
            "Captured Note".to_owned(),
            "--file".to_owned(),
            "projects/captured.org".to_owned(),
            "--head".to_owned(),
            head,
            "--ref".to_owned(),
            "cite:captured2026".to_owned(),
            "cite:extra2026".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let created: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(created.file_path, "projects/captured.org");
    assert_eq!(created.title, "Captured Note");
    assert_eq!(created.kind, NodeKind::File);
    assert_eq!(created.refs, vec!["@captured2026", "@extra2026"]);
    assert_eq!(created.tags, vec!["project"]);
    assert!(created.explicit_id.is_some());
    assert!(output.stderr.is_empty());

    let source = fs::read_to_string(root.join("projects/captured.org"))?;
    assert!(source.starts_with("#+title: Captured Note\n#+filetags: :project:\n"));
    assert!(source.contains(":ROAM_REFS: @captured2026 @extra2026"));

    args = vec!["ref".to_owned(), "resolve".to_owned()];
    args.extend(scoped_server_args_with_file_extension(&root, &db, "org"));
    args.extend(["cite:captured2026".to_owned(), "--json".to_owned()]);
    let resolved_output = run_slipbox(&args)?;
    assert!(resolved_output.status.success(), "{resolved_output:?}");
    let resolved: NodeRecord = serde_json::from_slice(&resolved_output.stdout)?;
    assert_eq!(resolved.node_key, created.node_key);
    assert!(resolved_output.stderr.is_empty());

    Ok(())
}

#[test]
fn note_create_uses_existing_duplicate_path_policy() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    fs::create_dir_all(root.join("projects"))?;
    fs::write(root.join("projects/sample.org"), "#+title: Existing\n")?;
    let args = note_command(
        &root,
        &db,
        "create",
        &[
            "--title".to_owned(),
            "Sample".to_owned(),
            "--file".to_owned(),
            "projects/sample.org".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let created: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(created.file_path, "projects/sample-1.org");
    assert!(root.join("projects/sample-1.org").exists());
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn note_ensure_file_creates_nested_file_note_and_is_resolvable() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let args = note_command(
        &root,
        &db,
        "ensure-file",
        &[
            "--file".to_owned(),
            "daily/2026-05-12.org".to_owned(),
            "--title".to_owned(),
            "2026-05-12".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let ensured: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(ensured.file_path, "daily/2026-05-12.org");
    assert_eq!(ensured.title, "2026-05-12");
    assert!(ensured.explicit_id.is_some());
    assert!(root.join("daily/2026-05-12.org").exists());
    assert!(output.stderr.is_empty());

    let mut show_args = vec!["node".to_owned(), "show".to_owned()];
    show_args.extend(scoped_server_args_with_file_extension(&root, &db, "org"));
    show_args.extend([
        "--key".to_owned(),
        "file:daily/2026-05-12.org".to_owned(),
        "--json".to_owned(),
    ]);
    let show_output = run_slipbox(&show_args)?;
    assert!(show_output.status.success(), "{show_output:?}");
    let shown: NodeRecord = serde_json::from_slice(&show_output.stdout)?;
    assert_eq!(shown.node_key, ensured.node_key);
    assert!(show_output.stderr.is_empty());

    Ok(())
}

#[test]
fn note_file_commands_accept_absolute_paths_under_root() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;

    let create_args = note_command(
        &root,
        &db,
        "create",
        &[
            "--title".to_owned(),
            "Absolute Capture".to_owned(),
            "--file".to_owned(),
            root.join("absolute/captured.org").display().to_string(),
            "--json".to_owned(),
        ],
    );
    let create_output = run_slipbox(&create_args)?;
    assert!(create_output.status.success(), "{create_output:?}");
    let created: NodeRecord = serde_json::from_slice(&create_output.stdout)?;
    assert_eq!(created.file_path, "absolute/captured.org");

    let ensure_args = note_command(
        &root,
        &db,
        "ensure-file",
        &[
            "--file".to_owned(),
            root.join("absolute/ensured.org").display().to_string(),
            "--title".to_owned(),
            "Absolute Ensured".to_owned(),
            "--json".to_owned(),
        ],
    );
    let ensure_output = run_slipbox(&ensure_args)?;
    assert!(ensure_output.status.success(), "{ensure_output:?}");
    let ensured: NodeRecord = serde_json::from_slice(&ensure_output.stdout)?;
    assert_eq!(ensured.file_path, "absolute/ensured.org");

    let append_args = note_command(
        &root,
        &db,
        "append-heading",
        &[
            "--file".to_owned(),
            root.join("absolute/ensured.org").display().to_string(),
            "--title".to_owned(),
            "Absolute Ensured".to_owned(),
            "--heading".to_owned(),
            "Absolute Heading".to_owned(),
            "--json".to_owned(),
        ],
    );
    let append_output = run_slipbox(&append_args)?;
    assert!(append_output.status.success(), "{append_output:?}");
    let appended: AnchorRecord = serde_json::from_slice(&append_output.stdout)?;
    assert_eq!(appended.file_path, "absolute/ensured.org");
    assert_eq!(appended.title, "Absolute Heading");

    let outline_args = note_command(
        &root,
        &db,
        "append-outline",
        &[
            "--file".to_owned(),
            root.join("absolute/outline.org").display().to_string(),
            "--heading".to_owned(),
            "Absolute Finding".to_owned(),
            "--outline".to_owned(),
            "Absolute".to_owned(),
            "Review".to_owned(),
            "--head".to_owned(),
            "#+title: Absolute Outline\n".to_owned(),
            "--json".to_owned(),
        ],
    );
    let outline_output = run_slipbox(&outline_args)?;
    assert!(outline_output.status.success(), "{outline_output:?}");
    let outline: AnchorRecord = serde_json::from_slice(&outline_output.stdout)?;
    assert_eq!(outline.file_path, "absolute/outline.org");
    assert_eq!(outline.outline_path, "Absolute / Review / Absolute Finding");
    assert!(outline_output.stderr.is_empty());

    Ok(())
}

#[test]
fn note_append_heading_and_append_to_node_are_immediately_resolvable() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let ensure_args = note_command(
        &root,
        &db,
        "ensure-file",
        &[
            "--file".to_owned(),
            "daily/2026-05-12.org".to_owned(),
            "--title".to_owned(),
            "2026-05-12".to_owned(),
            "--json".to_owned(),
        ],
    );
    let ensure_output = run_slipbox(&ensure_args)?;
    assert!(ensure_output.status.success(), "{ensure_output:?}");

    let append_args = note_command(
        &root,
        &db,
        "append-heading",
        &[
            "--file".to_owned(),
            "daily/2026-05-12.org".to_owned(),
            "--title".to_owned(),
            "2026-05-12".to_owned(),
            "--heading".to_owned(),
            "Standup".to_owned(),
            "--level".to_owned(),
            "2".to_owned(),
            "--json".to_owned(),
        ],
    );
    let append_output = run_slipbox(&append_args)?;
    assert!(append_output.status.success(), "{append_output:?}");
    let appended: AnchorRecord = serde_json::from_slice(&append_output.stdout)?;
    assert_eq!(appended.file_path, "daily/2026-05-12.org");
    assert_eq!(appended.title, "Standup");
    assert_eq!(appended.kind, NodeKind::Heading);
    assert_eq!(appended.level, 2);
    assert!(append_output.stderr.is_empty());

    let mut at_point_args = vec!["node".to_owned(), "at-point".to_owned()];
    at_point_args.extend(scoped_server_args_with_file_extension(&root, &db, "org"));
    at_point_args.extend([
        "--file".to_owned(),
        "daily/2026-05-12.org".to_owned(),
        "--line".to_owned(),
        appended.line.to_string(),
        "--json".to_owned(),
    ]);
    let at_point_output = run_slipbox(&at_point_args)?;
    assert!(at_point_output.status.success(), "{at_point_output:?}");
    let resolved_anchor: AnchorRecord = serde_json::from_slice(&at_point_output.stdout)?;
    assert_eq!(resolved_anchor.node_key, appended.node_key);

    let append_child_args = note_command(
        &root,
        &db,
        "append-to-node",
        &[
            "--key".to_owned(),
            "file:daily/2026-05-12.org".to_owned(),
            "--heading".to_owned(),
            "Child Task".to_owned(),
            "--json".to_owned(),
        ],
    );
    let child_output = run_slipbox(&append_child_args)?;
    assert!(child_output.status.success(), "{child_output:?}");
    let child: AnchorRecord = serde_json::from_slice(&child_output.stdout)?;
    assert_eq!(child.title, "Child Task");
    assert_eq!(child.level, 1);
    assert!(child_output.stderr.is_empty());

    Ok(())
}

#[test]
fn note_append_outline_creates_missing_outline_chain() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let args = note_command(
        &root,
        &db,
        "append-outline",
        &[
            "--file".to_owned(),
            "projects/review.org".to_owned(),
            "--heading".to_owned(),
            "Finding".to_owned(),
            "--outline".to_owned(),
            "Inbox".to_owned(),
            "Reviews".to_owned(),
            "--head".to_owned(),
            "#+title: Review\n".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let appended: AnchorRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(appended.file_path, "projects/review.org");
    assert_eq!(appended.title, "Finding");
    assert_eq!(appended.outline_path, "Inbox / Reviews / Finding");
    assert_eq!(appended.level, 3);
    assert!(output.stderr.is_empty());

    let source = fs::read_to_string(root.join("projects/review.org"))?;
    assert!(source.starts_with("#+title: Review\n"));
    assert!(source.contains("* Inbox"));
    assert!(source.contains("** Reviews"));
    assert!(source.contains("*** Finding"));

    Ok(())
}

#[test]
fn note_commands_report_structured_json_errors() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let args = note_command(
        &root,
        &db,
        "append-to-node",
        &[
            "--id".to_owned(),
            "missing-id".to_owned(),
            "--heading".to_owned(),
            "Child".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(error.error.message.contains("unknown node id: missing-id"));

    Ok(())
}
