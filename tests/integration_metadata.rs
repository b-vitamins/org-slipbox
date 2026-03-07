use std::fs;

use anyhow::Result;
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[test]
fn indexes_aliases_and_tags_and_searches_by_them() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("wayne.org"),
        ":PROPERTIES:\n:ID: 57ff3ce7-5bda-4825-8fca-c09f523e87ba\n:ROAM_ALIASES: Batman \"The Dark Knight\"\n:END:\n#+FILETAGS: :hero:gotham:\n#+title: Bruce Wayne\n\n* Patrol Log :night:city:\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let alias_results = database.search_nodes("dark knight", 10)?;
    assert_eq!(alias_results.len(), 1);
    assert_eq!(alias_results[0].title, "Bruce Wayne");
    assert_eq!(alias_results[0].aliases, vec!["Batman", "The Dark Knight"]);
    assert_eq!(alias_results[0].tags, vec!["hero", "gotham"]);

    let tag_results = database.search_nodes("night", 10)?;
    assert_eq!(tag_results.len(), 1);
    assert_eq!(tag_results[0].title, "Patrol Log");
    assert_eq!(tag_results[0].tags, vec!["hero", "gotham", "night", "city"]);

    Ok(())
}
