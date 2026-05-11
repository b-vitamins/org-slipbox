use std::fs;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;
use slipbox_core::{
    AnchorRecord, BacklinksResult, ForwardLinksResult, NodeKind, NodeRecord, RandomNodeResult,
    SearchNodesResult,
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

fn build_indexed_fixture() -> Result<(tempfile::TempDir, String, String, AnchorRecord)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("alpha.org"),
        r#":PROPERTIES:
:ID: alpha-id
:ROAM_ALIASES: Apex
:ROAM_REFS: cite:alpha2024
:END:
#+title: Alpha

See [[id:beta-id][Beta]].
"#,
    )?;
    fs::write(
        root.join("beta.org"),
        r#":PROPERTIES:
:ID: beta-id
:END:
#+title: Beta

* Child
:PROPERTIES:
:ID: beta-child-id
:END:
Child body.
** Anonymous Grandchild
Anonymous body.
"#,
    )?;
    fs::write(
        root.join("context.org"),
        r#"#+title: Context

* Source
:PROPERTIES:
:ID: source-id
:END:
Links to [[id:alpha-id][Alpha]] and [[id:beta-id][Beta]].
"#,
    )?;
    fs::write(root.join("shared-one.org"), "#+title: Shared Title\n")?;
    fs::write(root.join("shared-two.org"), "#+title: Shared Title\n")?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;
    let anonymous_anchor = database
        .anchors_in_file("beta.org")?
        .into_iter()
        .find(|anchor| anchor.title == "Anonymous Grandchild")
        .context("anonymous anchor should be indexed")?;

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
        anonymous_anchor,
    ))
}

fn node_command(
    root: &str,
    db: &str,
    subcommand: &str,
    extra_args: &[String],
) -> Result<std::process::Output> {
    let mut args = vec![
        "node".to_owned(),
        subcommand.to_owned(),
        "--root".to_owned(),
        root.to_owned(),
        "--db".to_owned(),
        db.to_owned(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--json".to_owned(),
    ];
    args.extend_from_slice(extra_args);
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

#[test]
fn node_show_search_and_random_use_canonical_json_shapes() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor) = build_indexed_fixture()?;

    let show = node_command(
        &root,
        &db,
        "show",
        &["--id".to_owned(), "alpha-id".to_owned()],
    )?;
    assert!(show.status.success(), "{show:?}");
    let shown: NodeRecord = serde_json::from_slice(&show.stdout)?;
    assert_eq!(shown.node_key, "file:alpha.org");
    assert_eq!(shown.title, "Alpha");
    assert_eq!(shown.aliases, vec!["Apex"]);
    assert!(show.stderr.is_empty());

    let search = node_command(&root, &db, "search", &["Alpha".to_owned()])?;
    assert!(search.status.success(), "{search:?}");
    let search_result: SearchNodesResult = serde_json::from_slice(&search.stdout)?;
    assert!(
        search_result
            .nodes
            .iter()
            .any(|node| node.node_key == "file:alpha.org")
    );
    assert!(search.stderr.is_empty());

    let random = node_command(&root, &db, "random", &[])?;
    assert!(random.status.success(), "{random:?}");
    let random_result: RandomNodeResult = serde_json::from_slice(&random.stdout)?;
    assert!(random_result.node.is_some());
    assert!(random.stderr.is_empty());

    Ok(())
}

#[test]
fn node_link_commands_resolve_note_targets_and_return_canonical_json() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor) = build_indexed_fixture()?;

    let backlinks = node_command(
        &root,
        &db,
        "backlinks",
        &["--id".to_owned(), "alpha-id".to_owned()],
    )?;
    assert!(backlinks.status.success(), "{backlinks:?}");
    let backlinks_result: BacklinksResult = serde_json::from_slice(&backlinks.stdout)?;
    assert_eq!(backlinks_result.backlinks.len(), 1);
    assert_eq!(
        backlinks_result.backlinks[0].source_note.node_key,
        "heading:context.org:3"
    );
    assert_eq!(
        backlinks_result.backlinks[0].preview,
        "Links to [[id:alpha-id][Alpha]] and [[id:beta-id][Beta]]."
    );
    assert!(backlinks.stderr.is_empty());

    let forward_links = node_command(
        &root,
        &db,
        "forward-links",
        &["--id".to_owned(), "alpha-id".to_owned()],
    )?;
    assert!(forward_links.status.success(), "{forward_links:?}");
    let forward_result: ForwardLinksResult = serde_json::from_slice(&forward_links.stdout)?;
    assert_eq!(forward_result.forward_links.len(), 1);
    assert_eq!(
        forward_result.forward_links[0].destination_note.node_key,
        "file:beta.org"
    );
    assert!(forward_links.stderr.is_empty());

    Ok(())
}

#[test]
fn node_at_point_returns_anonymous_anchor_semantics() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor) = build_indexed_fixture()?;

    let output = node_command(
        &root,
        &db,
        "at-point",
        &[
            "--file".to_owned(),
            "beta.org".to_owned(),
            "--line".to_owned(),
            anonymous_anchor.line.to_string(),
        ],
    )?;

    assert!(output.status.success(), "{output:?}");
    let parsed: AnchorRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.node_key, anonymous_anchor.node_key);
    assert_eq!(parsed.title, "Anonymous Grandchild");
    assert_eq!(parsed.explicit_id, None);
    assert_eq!(parsed.kind, NodeKind::Heading);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn node_target_commands_report_structured_json_errors() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor) = build_indexed_fixture()?;

    for (subcommand, args, expected_message) in [
        (
            "show",
            vec!["--title".to_owned(), "Shared Title".to_owned()],
            "multiple nodes match Shared Title",
        ),
        (
            "show",
            vec!["--id".to_owned(), "missing-id".to_owned()],
            "unknown node id: missing-id",
        ),
        (
            "show",
            vec!["--key".to_owned(), anonymous_anchor.node_key.clone()],
            "unknown node key:",
        ),
    ] {
        let output = node_command(&root, &db, subcommand, &args)?;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
        assert!(
            parsed.error.message.contains(expected_message),
            "expected {expected_message:?}, got {:?}",
            parsed.error.message
        );
    }

    Ok(())
}

#[test]
fn node_human_link_output_includes_location_and_preview() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor) = build_indexed_fixture()?;
    let output = Command::new(slipbox_binary())
        .args([
            "node",
            "backlinks",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            slipbox_binary(),
            "--id",
            "alpha-id",
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("backlinks: 1"));
    assert!(stdout.contains("Source [heading:context.org:3] context.org:3"));
    assert!(stdout.contains("preview: Links to [[id:alpha-id][Alpha]]"));
    assert!(output.stderr.is_empty());

    Ok(())
}
