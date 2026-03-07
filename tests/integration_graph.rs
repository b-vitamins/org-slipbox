use std::fs;

use anyhow::Result;
use slipbox_core::{GraphParams, GraphTitleShortening};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[test]
fn generates_global_graph_dot_with_orphan_nodes() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\nSee [[id:beta-file][Beta]].\n",
    )?;
    fs::write(
        root.join("beta.org"),
        ":PROPERTIES:\n:ID: beta-file\n:END:\n#+title: Beta\n",
    )?;
    fs::write(root.join("gamma.org"), "#+title: Gamma\n")?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let dot = database.graph_dot(&GraphParams {
        root_node_key: None,
        max_distance: None,
        include_orphans: true,
        hidden_link_types: Vec::new(),
        max_title_length: 100,
        shorten_titles: None,
        node_url_prefix: None,
    })?;

    assert!(dot.contains("\"file:alpha.org\" -> \"file:beta.org\";"));
    assert!(dot.contains("\"file:gamma.org\" [label=\"Gamma\""));

    Ok(())
}

#[test]
fn generates_neighborhood_graph_with_distance_limit() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\nSee [[id:beta-file][Beta]].\n",
    )?;
    fs::write(
        root.join("beta.org"),
        ":PROPERTIES:\n:ID: beta-file\n:END:\n#+title: Beta\nSee [[id:gamma-file][Gamma]].\n",
    )?;
    fs::write(
        root.join("gamma.org"),
        ":PROPERTIES:\n:ID: gamma-file\n:END:\n#+title: Gamma\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let dot = database.graph_dot(&GraphParams {
        root_node_key: Some("file:alpha.org".to_owned()),
        max_distance: Some(1),
        include_orphans: false,
        hidden_link_types: Vec::new(),
        max_title_length: 100,
        shorten_titles: None,
        node_url_prefix: None,
    })?;

    assert!(dot.contains("\"file:alpha.org\" [label=\"Alpha\""));
    assert!(dot.contains("\"file:beta.org\" [label=\"Beta\""));
    assert!(!dot.contains("\"file:gamma.org\" [label=\"Gamma\""));
    assert!(dot.contains("\"file:alpha.org\" -> \"file:beta.org\";"));
    assert!(!dot.contains("\"file:beta.org\" -> \"file:gamma.org\";"));

    Ok(())
}

#[test]
fn shortens_long_titles_in_graph_labels() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("long.org"),
        "#+title: A Very Long Graph Title For Testing\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let dot = database.graph_dot(&GraphParams {
        root_node_key: None,
        max_distance: None,
        include_orphans: true,
        hidden_link_types: Vec::new(),
        max_title_length: 12,
        shorten_titles: Some(GraphTitleShortening::Truncate),
        node_url_prefix: None,
    })?;

    assert!(dot.contains("label=\"A Very Lo...\""));

    Ok(())
}

#[test]
fn rejects_unsupported_hidden_link_types() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(root.join("alpha.org"), "#+title: Alpha\n")?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let error = database
        .graph_dot(&GraphParams {
            root_node_key: None,
            max_distance: None,
            include_orphans: true,
            hidden_link_types: vec!["file".to_owned()],
            max_title_length: 100,
            shorten_titles: None,
            node_url_prefix: None,
        })
        .expect_err("unsupported link type should fail");

    assert!(
        error
            .to_string()
            .contains("unsupported graph link type filter: file")
    );

    Ok(())
}

#[test]
fn adds_protocol_urls_for_nodes_with_explicit_ids() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        ":PROPERTIES:\n:ID: alpha id\n:END:\n#+title: Alpha\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let dot = database.graph_dot(&GraphParams {
        root_node_key: None,
        max_distance: None,
        include_orphans: true,
        hidden_link_types: Vec::new(),
        max_title_length: 100,
        shorten_titles: None,
        node_url_prefix: Some("org-protocol://roam-node?node=".to_owned()),
    })?;

    assert!(
        dot.contains(
            "\"file:alpha.org\" [label=\"Alpha\", tooltip=\"alpha.org\", URL=\"org-protocol://roam-node?node=alpha%20id\"]"
        )
    );

    Ok(())
}
