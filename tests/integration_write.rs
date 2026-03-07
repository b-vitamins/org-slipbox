use std::fs;

use anyhow::Result;
use slipbox_index::{scan_path, scan_root};
use slipbox_store::Database;
use slipbox_write::{
    append_heading, capture_file_note, capture_file_note_at, ensure_file_note, ensure_node_id,
};
use tempfile::tempdir;

#[test]
fn capture_creates_file_node_with_explicit_id() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    let captured = capture_file_note(&root, "Captured Note")?;
    let indexed = scan_path(&root, &captured.absolute_path)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&[indexed])?;

    let node = database
        .node_by_key(&captured.node_key)?
        .expect("captured node should exist");
    assert_eq!(node.title, "Captured Note");
    assert!(node.explicit_id.is_some());

    Ok(())
}

#[test]
fn ensure_node_id_updates_heading_in_place() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("heading.org");
    fs::write(
        &note_path,
        "#+title: Heading Test\n\n* Unidentified heading\nBody.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let node = database
        .search_nodes("unidentified", 10)?
        .into_iter()
        .find(|candidate| candidate.title == "Unidentified heading")
        .expect("heading node should exist");
    assert!(node.explicit_id.is_none());

    let updated_path = ensure_node_id(&root, &node)?;
    let indexed = scan_path(&root, &updated_path)?;
    database.sync_index(&[indexed])?;

    let refreshed = database
        .node_by_key(&node.node_key)?
        .expect("refreshed node should exist");
    assert!(refreshed.explicit_id.is_some());

    Ok(())
}

#[test]
fn ensure_file_note_creates_nested_org_file_with_explicit_id() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    let ensured = ensure_file_note(&root, "daily/2026-03-07.org", "2026-03-07")?;
    let indexed = scan_path(&root, &ensured.absolute_path)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&[indexed])?;

    let node = database
        .node_by_key(&ensured.node_key)?
        .expect("ensured daily note should exist");
    assert_eq!(node.title, "2026-03-07");
    assert!(node.explicit_id.is_some());

    Ok(())
}

#[test]
fn append_heading_creates_indexed_heading_node() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    let captured = append_heading(&root, "daily/2026-03-07.org", "2026-03-07", "Meeting", 1)?;
    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let node = database
        .node_by_key(&captured.node_key)?
        .expect("captured heading should exist");
    assert_eq!(node.title, "Meeting");
    assert_eq!(node.file_path, "daily/2026-03-07.org");
    assert_eq!(node.line, 6);

    Ok(())
}

#[test]
fn capture_file_note_at_chooses_unique_path_when_target_exists() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(root.join("projects"))?;
    fs::write(
        root.join("projects").join("sample.org"),
        "#+title: Existing\n",
    )?;

    let captured = capture_file_note_at(&root, "projects/sample.org", "Sample")?;
    assert_eq!(captured.node_key, "file:projects/sample-1.org");
    assert!(captured.absolute_path.ends_with("projects/sample-1.org"));

    Ok(())
}
