use std::fs;

use anyhow::Result;
use slipbox_index::{
    DiscoveryPolicy, scan_path, scan_path_with_policy, scan_root, scan_root_with_policy,
};
use slipbox_store::Database;
use tempfile::tempdir;

#[test]
fn indexes_nodes_searches_and_returns_backlinks() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\n\n* First heading\n:PROPERTIES:\n:ID: alpha-first\n:END:\nSee [[id:beta-target][Beta]].\n",
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

    let backlinks = database.backlinks(&results[0].node_key, 10, false)?;
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].source_node.title, "First heading");
    assert_eq!(backlinks[0].row, 7);
    assert_eq!(backlinks[0].col, 5);
    assert_eq!(backlinks[0].preview, "See [[id:beta-target][Beta]].");

    Ok(())
}

#[test]
fn backlinks_support_unique_sources() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\n\n* First heading\n:PROPERTIES:\n:ID: alpha-first\n:END:\nSee [[id:beta-target][Beta]].\nSee [[id:beta-target][Beta again]].\n",
    )?;
    fs::write(
        root.join("gamma.org"),
        "#+title: Gamma\n\n* Second heading\n:PROPERTIES:\n:ID: gamma-second\n:END:\nSee [[id:beta-target][Beta third]].\n",
    )?;
    fs::write(
        root.join("beta.org"),
        "#+title: Beta\n\n* Target heading\n:PROPERTIES:\n:ID: beta-target\n:END:\nTarget body.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let target = database
        .search_nodes("target", 10)?
        .into_iter()
        .next()
        .expect("expected target node");

    let backlinks = database.backlinks(&target.node_key, 10, false)?;
    assert_eq!(backlinks.len(), 3);

    let unique_backlinks = database.backlinks(&target.node_key, 10, true)?;
    assert_eq!(unique_backlinks.len(), 2);
    assert_eq!(unique_backlinks[0].source_node.title, "First heading");
    assert_eq!(unique_backlinks[0].row, 7);
    assert_eq!(unique_backlinks[1].source_node.title, "Second heading");

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

#[test]
fn incremental_file_sync_keeps_unrelated_files_indexed() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let alpha = root.join("alpha.org");
    let beta = root.join("beta.org");

    fs::write(&alpha, "#+title: Alpha\n")?;
    fs::write(&beta, "#+title: Beta\n")?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    fs::write(&alpha, "#+title: Alpha Updated\n")?;
    let indexed = scan_path(&root, &alpha)?;
    database.sync_file_index(&indexed)?;

    assert_eq!(
        database
            .node_by_key("file:alpha.org")?
            .expect("alpha node should still exist")
            .title,
        "Alpha Updated"
    );
    assert_eq!(
        database
            .node_by_key("file:beta.org")?
            .expect("beta node should still exist")
            .title,
        "Beta"
    );

    Ok(())
}

#[test]
fn scan_root_respects_configured_discovery_policy() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(root.join("archive"))?;

    fs::write(root.join("alpha.md"), "#+title: Alpha\n")?;
    fs::write(root.join("archive").join("hidden.md"), "#+title: Hidden\n")?;
    fs::write(root.join("beta.org"), "#+title: Beta\n")?;

    let policy = DiscoveryPolicy::new(["md"], ["^archive/"])?;
    let files = scan_root_with_policy(&root, &policy)?;

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_path, "alpha.md");
    assert_eq!(files[0].nodes[0].title, "Alpha");

    Ok(())
}

#[test]
fn scan_path_rejects_files_excluded_by_discovery_policy() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    let archived = root.join("archive").join("hidden.org");
    fs::create_dir_all(archived.parent().expect("archive parent"))?;
    fs::write(&archived, "#+title: Hidden\n")?;

    let policy = DiscoveryPolicy::new(["org"], ["^archive/"])?;
    let error = scan_path_with_policy(&root, &archived, &policy)
        .expect_err("excluded file should not be scanned");

    assert!(
        error
            .to_string()
            .contains("excluded by the current discovery policy")
    );

    Ok(())
}
