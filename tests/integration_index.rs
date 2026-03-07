use std::fs;

use anyhow::Result;
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[test]
fn indexes_nodes_searches_and_returns_backlinks() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\n\n* First heading\n:PROPERTIES:\n:ID: alpha-first\n:END:\nSee [[id:beta-target]].\n",
    )?;
    fs::write(
        root.join("beta.org"),
        "#+title: Beta\n\n* Target heading\n:PROPERTIES:\n:ID: beta-target\n:END:\nTarget body.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    let stats = database.sync_index(&files)?;

    assert_eq!(stats.files_indexed, 2);
    assert_eq!(stats.links_indexed, 1);

    let results = database.search_nodes("target", 10)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Target heading");

    let backlinks = database.backlinks(&results[0].node_key, 10)?;
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].title, "First heading");

    Ok(())
}

#[test]
fn indexes_and_queries_distinct_tags() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\n#+filetags: :global:alpha:\n\n* First heading :beta:\nBody.\n* Second heading :alpha:\nMore body.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    assert_eq!(
        database.search_tags("", 10)?,
        vec!["alpha".to_owned(), "beta".to_owned(), "global".to_owned()]
    );
    assert_eq!(database.search_tags("be", 10)?, vec!["beta".to_owned()]);

    Ok(())
}

#[test]
fn selects_a_random_node_from_the_index() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(root.join("alpha.org"), "#+title: Alpha\n")?;
    fs::write(root.join("beta.org"), "#+title: Beta\n")?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let node = database.random_node()?.expect("expected indexed node");
    assert!(matches!(node.title.as_str(), "Alpha" | "Beta"));

    Ok(())
}
