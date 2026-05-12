use std::fs;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;
use slipbox_core::{
    AnchorRecord, BacklinksResult, ForwardLinksResult, NodeKind, NodeRecord, RandomNodeResult,
    SearchNodesResult, SearchTagsResult,
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
#+filetags: :seed:

See [[id:beta-id][Beta]].
"#,
    )?;
    fs::write(
        root.join("beta.org"),
        r#":PROPERTIES:
:ID: beta-id
:END:
#+title: Beta

* Child :child:
:PROPERTIES:
:ID: beta-child-id
:ROAM_ALIASES: Beta Kid
:ROAM_REFS: cite:beta-child
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
    node_command_path(root, db, &[subcommand], extra_args)
}

fn node_command_path(
    root: &str,
    db: &str,
    subcommands: &[&str],
    extra_args: &[String],
) -> Result<std::process::Output> {
    let mut args = vec!["node".to_owned()];
    args.extend(
        subcommands
            .iter()
            .map(|subcommand| (*subcommand).to_owned()),
    );
    args.extend([
        "--root".to_owned(),
        root.to_owned(),
        "--db".to_owned(),
        db.to_owned(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--json".to_owned(),
    ]);
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
fn node_ensure_id_assigns_file_and_heading_identities() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor) = build_indexed_fixture()?;

    let file_output = node_command(
        &root,
        &db,
        "ensure-id",
        &["--key".to_owned(), "file:shared-one.org".to_owned()],
    )?;
    assert!(file_output.status.success(), "{file_output:?}");
    let file_anchor: AnchorRecord = serde_json::from_slice(&file_output.stdout)?;
    assert_eq!(file_anchor.node_key, "file:shared-one.org");
    assert_eq!(file_anchor.kind, NodeKind::File);
    assert!(file_anchor.explicit_id.is_some());

    let heading_output = node_command(
        &root,
        &db,
        "ensure-id",
        &["--key".to_owned(), anonymous_anchor.node_key.clone()],
    )?;
    assert!(heading_output.status.success(), "{heading_output:?}");
    let heading_anchor: AnchorRecord = serde_json::from_slice(&heading_output.stdout)?;
    assert_eq!(heading_anchor.node_key, anonymous_anchor.node_key);
    assert_eq!(heading_anchor.kind, NodeKind::Heading);
    assert!(heading_anchor.explicit_id.is_some());

    let show_heading = node_command(
        &root,
        &db,
        "show",
        &["--key".to_owned(), anonymous_anchor.node_key.clone()],
    )?;
    assert!(show_heading.status.success(), "{show_heading:?}");
    let shown: NodeRecord = serde_json::from_slice(&show_heading.stdout)?;
    assert_eq!(shown.explicit_id, heading_anchor.explicit_id);
    assert!(show_heading.stderr.is_empty());

    Ok(())
}

#[test]
fn node_metadata_commands_update_file_level_metadata() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor) = build_indexed_fixture()?;

    let show = node_command_path(
        &root,
        &db,
        &["metadata", "show"],
        &["--id".to_owned(), "alpha-id".to_owned()],
    )?;
    assert!(show.status.success(), "{show:?}");
    let shown: NodeRecord = serde_json::from_slice(&show.stdout)?;
    assert_eq!(shown.aliases, vec!["Apex"]);
    assert_eq!(shown.refs, vec!["@alpha2024"]);
    assert_eq!(shown.tags, vec!["seed"]);

    let add_alias = node_command_path(
        &root,
        &db,
        &["alias", "add"],
        &[
            "--id".to_owned(),
            "alpha-id".to_owned(),
            "Apex".to_owned(),
            "Alpha Alt".to_owned(),
        ],
    )?;
    assert!(add_alias.status.success(), "{add_alias:?}");
    let aliased: NodeRecord = serde_json::from_slice(&add_alias.stdout)?;
    assert_eq!(aliased.aliases, vec!["Apex", "Alpha Alt"]);

    let search = node_command(&root, &db, "search", &["Alpha Alt".to_owned()])?;
    assert!(search.status.success(), "{search:?}");
    let search_result: SearchNodesResult = serde_json::from_slice(&search.stdout)?;
    assert!(
        search_result
            .nodes
            .iter()
            .any(|node| node.node_key == "file:alpha.org")
    );

    let remove_alias = node_command_path(
        &root,
        &db,
        &["alias", "remove"],
        &["--id".to_owned(), "alpha-id".to_owned(), "apex".to_owned()],
    )?;
    assert!(remove_alias.status.success(), "{remove_alias:?}");
    let removed: NodeRecord = serde_json::from_slice(&remove_alias.stdout)?;
    assert_eq!(removed.aliases, vec!["Alpha Alt"]);

    let set_alias = node_command_path(
        &root,
        &db,
        &["alias", "set"],
        &[
            "--id".to_owned(),
            "alpha-id".to_owned(),
            "Primary".to_owned(),
            "Primary".to_owned(),
            "Secondary".to_owned(),
        ],
    )?;
    assert!(set_alias.status.success(), "{set_alias:?}");
    let reset: NodeRecord = serde_json::from_slice(&set_alias.stdout)?;
    assert_eq!(reset.aliases, vec!["Primary", "Secondary"]);

    Ok(())
}

#[test]
fn node_metadata_commands_update_heading_refs_and_tags() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor) = build_indexed_fixture()?;

    let add_ref = node_command_path(
        &root,
        &db,
        &["ref", "add"],
        &[
            "--id".to_owned(),
            "beta-child-id".to_owned(),
            "cite:beta-child".to_owned(),
            "https://example.com/beta".to_owned(),
        ],
    )?;
    assert!(add_ref.status.success(), "{add_ref:?}");
    let refed: NodeRecord = serde_json::from_slice(&add_ref.stdout)?;
    assert_eq!(refed.refs, vec!["@beta-child", "https://example.com/beta"]);

    let ref_show = Command::new(slipbox_binary())
        .args([
            "ref",
            "show",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            slipbox_binary(),
            "--json",
            "https://example.com/beta",
        ])
        .output()?;
    assert!(ref_show.status.success(), "{ref_show:?}");
    let ref_target: NodeRecord = serde_json::from_slice(&ref_show.stdout)?;
    assert_eq!(ref_target.node_key, refed.node_key);

    let remove_ref = node_command_path(
        &root,
        &db,
        &["ref", "remove"],
        &[
            "--id".to_owned(),
            "beta-child-id".to_owned(),
            "cite:beta-child".to_owned(),
        ],
    )?;
    assert!(remove_ref.status.success(), "{remove_ref:?}");
    let ref_removed: NodeRecord = serde_json::from_slice(&remove_ref.stdout)?;
    assert_eq!(ref_removed.refs, vec!["https://example.com/beta"]);

    let set_tags = node_command_path(
        &root,
        &db,
        &["tag", "set"],
        &[
            "--id".to_owned(),
            "beta-child-id".to_owned(),
            "review".to_owned(),
            "review".to_owned(),
            "active".to_owned(),
        ],
    )?;
    assert!(set_tags.status.success(), "{set_tags:?}");
    let tagged: NodeRecord = serde_json::from_slice(&set_tags.stdout)?;
    assert_eq!(tagged.tags, vec!["review", "active"]);

    let tag_search = Command::new(slipbox_binary())
        .args([
            "tag",
            "search",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            slipbox_binary(),
            "--json",
            "review",
        ])
        .output()?;
    assert!(tag_search.status.success(), "{tag_search:?}");
    let tags: SearchTagsResult = serde_json::from_slice(&tag_search.stdout)?;
    assert!(tags.tags.iter().any(|tag| tag == "review"));

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
fn node_metadata_commands_report_structured_json_errors() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor) = build_indexed_fixture()?;
    let output = node_command_path(
        &root,
        &db,
        &["alias", "add"],
        &[
            "--id".to_owned(),
            "missing-id".to_owned(),
            "Ghost".to_owned(),
        ],
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(parsed.error.message.contains("unknown node id: missing-id"));

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
