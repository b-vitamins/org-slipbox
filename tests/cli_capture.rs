use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{AnchorRecord, CaptureTemplatePreviewResult, NodeKind, NodeRecord};
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
    fs::create_dir_all(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    Ok((workspace, root, db))
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

fn run_slipbox_with_stdin(args: &[String], stdin: &[u8]) -> Result<std::process::Output> {
    let mut child = Command::new(slipbox_binary())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(stdin)?;
    drop(child.stdin.take());
    Ok(child.wait_with_output()?)
}

fn capture_command(root: &Path, db: &Path, subcommand: &str, extra_args: &[String]) -> Vec<String> {
    let mut args = vec!["capture".to_owned(), subcommand.to_owned()];
    args.extend(scoped_args(root, db));
    args.extend_from_slice(extra_args);
    args
}

fn sync_root(root: &Path, db: &Path) -> Result<()> {
    let mut args = vec!["sync".to_owned(), "root".to_owned()];
    args.extend(scoped_args(root, db));
    args.push("--json".to_owned());
    let output = run_slipbox(&args)?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

#[test]
fn capture_node_command_writes_ref_backed_file_note() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let args = capture_command(
        &root,
        &db,
        "node",
        &[
            "--title".to_owned(),
            "Captured Node".to_owned(),
            "--file".to_owned(),
            "captures/node.org".to_owned(),
            "--ref".to_owned(),
            "cite:node2026".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let node: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(node.file_path, "captures/node.org");
    assert_eq!(node.title, "Captured Node");
    assert_eq!(node.refs, vec!["@node2026"]);
    assert!(node.explicit_id.is_some());
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn capture_template_entry_reads_stdin_and_targets_outline_with_refs() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let args = capture_command(
        &root,
        &db,
        "template",
        &[
            "--file".to_owned(),
            "daily/2026-05-12.org".to_owned(),
            "--head".to_owned(),
            "#+title: 2026-05-12\n".to_owned(),
            "--outline".to_owned(),
            "Inbox".to_owned(),
            "--type".to_owned(),
            "entry".to_owned(),
            "--title".to_owned(),
            "Meeting".to_owned(),
            "--content-stdin".to_owned(),
            "--ref".to_owned(),
            "cite:meeting2026".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox_with_stdin(&args, b"* Meeting\nCaptured from stdin.\n")?;

    assert!(output.status.success(), "{output:?}");
    let captured: AnchorRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(captured.file_path, "daily/2026-05-12.org");
    assert_eq!(captured.title, "Meeting");
    assert_eq!(captured.outline_path, "Inbox / Meeting");
    assert_eq!(captured.level, 2);
    assert!(output.stderr.is_empty());

    let source = fs::read_to_string(root.join("daily/2026-05-12.org"))?;
    assert!(source.contains(":ROAM_REFS: @meeting2026"));
    assert!(source.contains("* Inbox\n** Meeting\nCaptured from stdin."));

    Ok(())
}

#[test]
fn capture_template_item_reads_content_file_and_targets_node_key() -> Result<()> {
    let (workspace, root, db) = build_fixture()?;
    fs::write(
        root.join("project.org"),
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n- First\n- Second\n\n** Child\n",
    )?;
    sync_root(&root, &db)?;
    let content_path = workspace.path().join("item.txt");
    fs::write(&content_path, "Third")?;
    let args = capture_command(
        &root,
        &db,
        "template",
        &[
            "--node-key".to_owned(),
            "heading:project.org:3".to_owned(),
            "--type".to_owned(),
            "item".to_owned(),
            "--content-file".to_owned(),
            content_path.display().to_string(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let captured: AnchorRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(captured.node_key, "heading:project.org:3");
    assert_eq!(captured.title, "Parent");
    assert!(output.stderr.is_empty());

    let source = fs::read_to_string(root.join("project.org"))?;
    assert!(source.contains("- First\n- Second\n- Third\n\n** Child"));

    Ok(())
}

#[test]
fn capture_template_supports_plain_checkitem_and_table_line() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;

    let plain = capture_command(
        &root,
        &db,
        "template",
        &[
            "--file".to_owned(),
            "plain.org".to_owned(),
            "--title".to_owned(),
            "Plain".to_owned(),
            "--type".to_owned(),
            "plain".to_owned(),
            "--content".to_owned(),
            "Plain body".to_owned(),
            "--json".to_owned(),
        ],
    );
    let plain_output = run_slipbox(&plain)?;
    assert!(plain_output.status.success(), "{plain_output:?}");
    let plain_anchor: AnchorRecord = serde_json::from_slice(&plain_output.stdout)?;
    assert_eq!(plain_anchor.kind, NodeKind::File);

    let checkitem = capture_command(
        &root,
        &db,
        "template",
        &[
            "--file".to_owned(),
            "plain.org".to_owned(),
            "--type".to_owned(),
            "checkitem".to_owned(),
            "--content".to_owned(),
            "Follow up".to_owned(),
            "--json".to_owned(),
        ],
    );
    let checkitem_output = run_slipbox(&checkitem)?;
    assert!(checkitem_output.status.success(), "{checkitem_output:?}");

    let table_line = capture_command(
        &root,
        &db,
        "template",
        &[
            "--file".to_owned(),
            "plain.org".to_owned(),
            "--type".to_owned(),
            "table-line".to_owned(),
            "--content".to_owned(),
            "Name | Value".to_owned(),
            "--json".to_owned(),
        ],
    );
    let table_output = run_slipbox(&table_line)?;
    assert!(table_output.status.success(), "{table_output:?}");

    let source = fs::read_to_string(root.join("plain.org"))?;
    assert!(source.contains("Plain body"));
    assert!(source.contains("- [ ] Follow up"));
    assert!(source.contains("| Name | Value |"));

    Ok(())
}

#[test]
fn capture_preview_returns_json_without_writing_files() -> Result<()> {
    let (workspace, root, db) = build_fixture()?;
    let source_path = workspace.path().join("source.org");
    fs::write(&source_path, "#+title: Preview\nLocal edits\n")?;
    let args = capture_command(
        &root,
        &db,
        "preview",
        &[
            "--file".to_owned(),
            "preview.org".to_owned(),
            "--type".to_owned(),
            "entry".to_owned(),
            "--title".to_owned(),
            "Preview Entry".to_owned(),
            "--content".to_owned(),
            "* Preview Entry\nBody.\n".to_owned(),
            "--source-file".to_owned(),
            source_path.display().to_string(),
            "--ensure-node-id".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let preview: CaptureTemplatePreviewResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(preview.file_path, "preview.org");
    assert!(preview.content.contains("Local edits"));
    assert!(preview.content.contains("* Preview Entry"));
    assert!(preview.content.contains("Body."));
    assert!(preview.preview_node.explicit_id.is_some());
    assert!(!root.join("preview.org").exists());
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn capture_commands_report_structured_json_failures() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let args = capture_command(
        &root,
        &db,
        "template",
        &[
            "--node-key".to_owned(),
            "missing-node".to_owned(),
            "--type".to_owned(),
            "plain".to_owned(),
            "--content".to_owned(),
            "Body".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("unknown target node: missing-node")
    );

    Ok(())
}

#[test]
fn capture_template_rejects_mixed_node_and_file_target_fields() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    fs::write(
        root.join("project.org"),
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\nBody.\n",
    )?;
    sync_root(&root, &db)?;

    let template_args = capture_command(
        &root,
        &db,
        "template",
        &[
            "--node-key".to_owned(),
            "heading:project.org:3".to_owned(),
            "--file".to_owned(),
            "other.org".to_owned(),
            "--outline".to_owned(),
            "Ignored".to_owned(),
            "--ref".to_owned(),
            "cite:ignored2026".to_owned(),
            "--type".to_owned(),
            "plain".to_owned(),
            "--content".to_owned(),
            "Body".to_owned(),
            "--json".to_owned(),
        ],
    );
    let template_output = run_slipbox(&template_args)?;
    assert_eq!(template_output.status.code(), Some(1));
    assert!(template_output.stdout.is_empty());
    let template_error: ErrorPayload = serde_json::from_slice(&template_output.stderr)?;
    assert!(
        template_error
            .error
            .message
            .contains("--node-key cannot be combined")
    );
    assert!(!root.join("other.org").exists());

    let preview_args = capture_command(
        &root,
        &db,
        "preview",
        &[
            "--node-key".to_owned(),
            "heading:project.org:3".to_owned(),
            "--head".to_owned(),
            "#+title: Ignored\n".to_owned(),
            "--type".to_owned(),
            "plain".to_owned(),
            "--content".to_owned(),
            "Body".to_owned(),
            "--json".to_owned(),
        ],
    );
    let preview_output = run_slipbox(&preview_args)?;
    assert_eq!(preview_output.status.code(), Some(1));
    assert!(preview_output.stdout.is_empty());
    let preview_error: ErrorPayload = serde_json::from_slice(&preview_output.stderr)?;
    assert!(
        preview_error
            .error
            .message
            .contains("--node-key cannot be combined")
    );

    Ok(())
}
