use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{
    BacklinksResult, SlipboxLinkRewriteApplyResult, SlipboxLinkRewritePreviewResult,
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

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

struct LinkFixture {
    _workspace: TempDir,
    root: PathBuf,
    db: PathBuf,
}

fn build_link_fixture(extra_files: &[(&str, &str)]) -> Result<LinkFixture> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("source.org"),
        "#+title: Source\n\nSee [[slipbox:Target][Target Label]].\n",
    )?;
    fs::write(root.join("target.org"), "#+title: Target\n\nTarget body.\n")?;
    for (relative_path, source) in extra_files {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)?;
    }

    let db = workspace.path().join("slipbox.sqlite");
    let files = scan_root(&root)?;
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;

    Ok(LinkFixture {
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

fn link_command(root: &Path, db: &Path, subcommand: &str, extra_args: &[String]) -> Vec<String> {
    let mut args = vec![
        "link".to_owned(),
        "rewrite-slipbox".to_owned(),
        subcommand.to_owned(),
    ];
    args.extend(scoped_args(root, db));
    args.extend_from_slice(extra_args);
    args
}

fn run_slipbox(args: &[String]) -> Result<std::process::Output> {
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

#[test]
fn link_rewrite_preview_reports_supported_slipbox_links() -> Result<()> {
    let fixture = build_link_fixture(&[])?;
    let args = link_command(
        &fixture.root,
        &fixture.db,
        "preview",
        &["--file".to_owned(), "source.org".to_owned()],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let result: SlipboxLinkRewritePreviewResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(result.preview.file_path, "source.org");
    assert_eq!(result.preview.rewrites.len(), 1);
    let rewrite = &result.preview.rewrites[0];
    assert_eq!(rewrite.title_or_alias, "Target");
    assert_eq!(rewrite.description, "Target Label");
    assert_eq!(rewrite.target.node_key, "file:target.org");
    assert!(rewrite.target_explicit_id.is_none());
    assert!(rewrite.replacement.is_none());

    Ok(())
}

#[test]
fn link_rewrite_commands_accept_absolute_paths_under_root() -> Result<()> {
    let fixture = build_link_fixture(&[])?;
    let absolute_source = fixture.root.join("source.org");
    let preview_args = link_command(
        &fixture.root,
        &fixture.db,
        "preview",
        &["--file".to_owned(), absolute_source.display().to_string()],
    );

    let preview_output = run_slipbox(&preview_args)?;

    assert!(preview_output.status.success(), "{preview_output:?}");
    let preview: SlipboxLinkRewritePreviewResult = serde_json::from_slice(&preview_output.stdout)?;
    assert_eq!(preview.preview.file_path, "source.org");
    assert_eq!(preview.preview.rewrites.len(), 1);

    let apply_args = link_command(
        &fixture.root,
        &fixture.db,
        "apply",
        &[
            "--file".to_owned(),
            absolute_source.display().to_string(),
            "--confirm-replace-slipbox-links".to_owned(),
        ],
    );
    let apply_output = run_slipbox(&apply_args)?;

    assert!(apply_output.status.success(), "{apply_output:?}");
    let applied: SlipboxLinkRewriteApplyResult = serde_json::from_slice(&apply_output.stdout)?;
    assert_eq!(applied.application.file_path, "source.org");
    assert_eq!(applied.application.rewrites.len(), 1);

    Ok(())
}

#[test]
fn link_rewrite_apply_assigns_ids_and_refreshes_backlinks() -> Result<()> {
    let fixture = build_link_fixture(&[])?;
    let args = link_command(
        &fixture.root,
        &fixture.db,
        "apply",
        &[
            "--file".to_owned(),
            "source.org".to_owned(),
            "--confirm-replace-slipbox-links".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let result: SlipboxLinkRewriteApplyResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(result.application.file_path, "source.org");
    assert_eq!(result.application.rewrites.len(), 1);
    assert!(
        result
            .application
            .affected_files
            .changed_files
            .contains(&"source.org".to_owned())
    );
    assert!(
        result
            .application
            .affected_files
            .changed_files
            .contains(&"target.org".to_owned())
    );
    let explicit_id = &result.application.rewrites[0].target_explicit_id;
    let source = fs::read_to_string(fixture.root.join("source.org"))?;
    assert!(source.contains(&format!("[[id:{explicit_id}][Target Label]]")));
    assert!(!source.contains("slipbox:Target"));
    let target = fs::read_to_string(fixture.root.join("target.org"))?;
    assert!(target.contains(":ID:"));
    assert!(target.contains(explicit_id));

    let mut backlink_args = vec!["node".to_owned(), "backlinks".to_owned()];
    backlink_args.extend(scoped_args(&fixture.root, &fixture.db));
    backlink_args.extend(["--id".to_owned(), explicit_id.clone()]);
    let backlink_output = run_slipbox(&backlink_args)?;
    assert!(backlink_output.status.success(), "{backlink_output:?}");
    assert!(backlink_output.stderr.is_empty(), "{backlink_output:?}");
    let backlinks: BacklinksResult = serde_json::from_slice(&backlink_output.stdout)?;
    assert_eq!(backlinks.backlinks.len(), 1);
    assert_eq!(
        backlinks.backlinks[0].source_note.node_key,
        "file:source.org"
    );

    Ok(())
}

#[test]
fn link_rewrite_preview_refuses_ambiguous_targets() -> Result<()> {
    let fixture = build_link_fixture(&[("duplicate.org", "#+title: Target\n")])?;
    let args = link_command(
        &fixture.root,
        &fixture.db,
        "preview",
        &["--file".to_owned(), "source.org".to_owned()],
    );

    let output = run_slipbox(&args)?;

    assert!(!output.status.success(), "{output:?}");
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("multiple nodes match slipbox link target Target"),
        "{}",
        error.error.message
    );

    Ok(())
}

#[test]
fn link_rewrite_preview_refuses_unresolved_targets() -> Result<()> {
    let fixture = build_link_fixture(&[])?;
    fs::write(
        fixture.root.join("source.org"),
        "#+title: Source\n\nSee [[slipbox:Missing][Missing]].\n",
    )?;
    let files = scan_root(&fixture.root)?;
    let mut database = Database::open(&fixture.db)?;
    database.sync_index(&files)?;
    let args = link_command(
        &fixture.root,
        &fixture.db,
        "preview",
        &["--file".to_owned(), "source.org".to_owned()],
    );

    let output = run_slipbox(&args)?;

    assert!(!output.status.success(), "{output:?}");
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("unresolved slipbox link target Missing"),
        "{}",
        error.error.message
    );

    Ok(())
}

#[test]
fn link_rewrite_apply_requires_confirmation() -> Result<()> {
    let fixture = build_link_fixture(&[])?;
    let args = link_command(
        &fixture.root,
        &fixture.db,
        "apply",
        &["--file".to_owned(), "source.org".to_owned()],
    );

    let output = run_slipbox(&args)?;

    assert!(!output.status.success(), "{output:?}");
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("requires --confirm-replace-slipbox-links"),
        "{}",
        error.error.message
    );

    Ok(())
}
