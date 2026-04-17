use std::fs;
use std::path::Path;

use anyhow::Result;
use slipbox_core::{CaptureContentType, CaptureTemplateParams};
use slipbox_index::{scan_path, scan_root, scan_source};
use slipbox_store::Database;
use slipbox_write::{
    MetadataUpdate, RegionRewriteOutcome, RewriteOutcome, append_heading,
    append_heading_at_outline_path, append_heading_to_node, capture_file_note,
    capture_file_note_at, capture_file_note_at_with_head_and_refs, capture_file_note_with_refs,
    capture_template, demote_entire_file, ensure_file_note, ensure_node_id, extract_subtree,
    preview_capture_template, promote_entire_file, refile_region, refile_subtree,
    update_node_metadata,
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
    database.sync_file_index(&indexed)?;

    let node = database
        .note_by_key(&captured.node_key)?
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
        .search_anchors("unidentified", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Unidentified heading")
        .expect("heading node should exist");
    assert!(node.explicit_id.is_none());

    let updated_path = ensure_node_id(&root, &node)?;
    let indexed = scan_path(&root, &updated_path)?;
    database.sync_file_index(&indexed)?;

    let refreshed = database
        .note_by_key(&node.node_key)?
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
    database.sync_file_index(&indexed)?;

    let node = database
        .note_by_key(&ensured.node_key)?
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
        .anchor_by_key(&captured.node_key)?
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

#[test]
fn capture_file_note_at_with_head_preserves_head_and_assigns_identity() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(root.join("projects"))?;

    let captured = capture_file_note_at_with_head_and_refs(
        &root,
        "projects/seed.org",
        "Seed",
        "#+title: Seed\n#+filetags: :seed:",
        &[String::from("https://example.test/seed")],
    )?;
    let indexed = scan_path(&root, &captured.absolute_path)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_file_index(&indexed)?;

    let source = fs::read_to_string(&captured.absolute_path)?;
    assert!(source.contains("#+title: Seed"));
    assert!(source.contains("#+filetags: :seed:"));
    assert!(source.contains(":ID: "));
    assert!(source.contains(":ROAM_REFS: https://example.test/seed"));

    let node = database
        .note_by_key(&captured.node_key)?
        .expect("captured file node should exist");
    assert_eq!(node.title, "Seed");
    assert_eq!(node.tags, vec!["seed"]);
    assert_eq!(node.refs, vec!["https://example.test/seed"]);
    assert!(node.explicit_id.is_some());

    Ok(())
}

#[test]
fn append_heading_at_outline_path_creates_missing_outline_chain() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    let captured = append_heading_at_outline_path(
        &root,
        "daily/2026-03-07.org",
        "Meeting",
        &[String::from("Inbox"), String::from("Calls")],
        Some("#+title: 2026-03-07"),
    )?;
    let indexed = scan_path(&root, &captured.absolute_path)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_file_index(&indexed)?;

    let source = fs::read_to_string(&captured.absolute_path)?;
    assert!(source.starts_with("#+title: 2026-03-07\n"));
    assert!(source.contains("* Inbox"));
    assert!(source.contains("** Calls"));
    assert!(source.contains("*** Meeting"));

    let node = database
        .anchor_by_key(&captured.node_key)?
        .expect("captured outline heading should exist");
    assert_eq!(node.title, "Meeting");
    assert_eq!(node.outline_path, "Inbox / Calls / Meeting");
    assert_eq!(node.level, 3);

    Ok(())
}

#[test]
fn capture_with_refs_writes_property_and_indexes_reference() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    let captured = capture_file_note_with_refs(
        &root,
        "Captured Note",
        &[String::from("https://example.test/ref")],
    )?;
    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let source = fs::read_to_string(&captured.absolute_path)?;
    assert!(source.contains(":ROAM_REFS: https://example.test/ref"));

    let node = database
        .node_from_ref("https://example.test/ref")?
        .expect("captured ref node should exist");
    assert_eq!(node.title, "Captured Note");

    Ok(())
}

#[test]
fn capture_template_entry_inserts_child_under_outline_target() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    let captured = capture_template(
        &root,
        None,
        &CaptureTemplateParams {
            title: String::from("Meeting"),
            file_path: Some(String::from("daily/2026-03-07.org")),
            node_key: None,
            head: Some(String::from("#+title: 2026-03-07")),
            outline_path: vec![String::from("Inbox")],
            capture_type: CaptureContentType::Entry,
            content: String::from("* Meeting\nCaptured.\n"),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    let indexed = scan_path(&root, &captured.absolute_path)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_file_index(&indexed)?;

    let source = fs::read_to_string(&captured.absolute_path)?;
    assert!(source.starts_with("#+title: 2026-03-07\n"));
    assert!(source.contains("* Inbox"));
    assert!(source.contains("** Meeting\nCaptured.\n"));

    let node = database
        .anchor_by_key(&captured.node_key)?
        .expect("captured entry should exist");
    assert_eq!(node.title, "Meeting");
    assert_eq!(node.level, 2);
    assert_eq!(node.outline_path, "Inbox / Meeting");

    Ok(())
}

#[test]
fn capture_template_entry_uses_title_for_blank_heading_templates() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    let captured = capture_template(
        &root,
        None,
        &CaptureTemplateParams {
            title: String::from("Daily substitution entry"),
            file_path: Some(String::from("daily/2026-03-08.org")),
            node_key: None,
            head: Some(String::from("#+title: 2026-03-08")),
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Entry,
            content: String::from("* "),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    let indexed = scan_path(&root, &captured.absolute_path)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_file_index(&indexed)?;

    let source = fs::read_to_string(&captured.absolute_path)?;
    assert!(source.starts_with("#+title: 2026-03-08\n"));
    assert!(source.contains("* Daily substitution entry\n"));

    let node = database
        .anchor_by_key(&captured.node_key)?
        .expect("captured daily entry should exist");
    assert_eq!(node.title, "Daily substitution entry");
    assert_eq!(node.file_path, "daily/2026-03-08.org");
    assert_eq!(node.level, 1);

    Ok(())
}

#[test]
fn capture_template_item_appends_inside_existing_list_body() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("project.org");
    fs::write(
        &note_path,
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n- First\n- Second\n\n** Child\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let parent = database
        .search_nodes("parent", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Parent")
        .expect("parent node should exist");

    let captured = capture_template(
        &root,
        Some(&parent),
        &CaptureTemplateParams {
            title: String::new(),
            file_path: None,
            node_key: Some(parent.node_key.clone()),
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Item,
            content: String::from("Third"),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    let indexed = scan_path(&root, &captured.absolute_path)?;
    database.sync_file_index(&indexed)?;

    let source = fs::read_to_string(&note_path)?;
    assert!(source.contains(
        "* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n- First\n- Second\n- Third\n\n** Child"
    ));
    assert_eq!(captured.node_key, parent.node_key);

    Ok(())
}

#[test]
fn capture_template_table_line_uses_existing_table_block() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("project.org");
    fs::write(
        &note_path,
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n| Name | Value |\n|------+-------|\n| One  | 1     |\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let parent = database
        .search_nodes("parent", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Parent")
        .expect("parent node should exist");

    let captured = capture_template(
        &root,
        Some(&parent),
        &CaptureTemplateParams {
            title: String::new(),
            file_path: None,
            node_key: Some(parent.node_key.clone()),
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::TableLine,
            content: String::from("Two | 2"),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    let indexed = scan_path(&root, &captured.absolute_path)?;
    database.sync_file_index(&indexed)?;

    let source = fs::read_to_string(&note_path)?;
    assert!(source.contains("| One  | 1     |\n| Two | 2 |\n"));
    assert_eq!(captured.node_key, parent.node_key);

    Ok(())
}

#[test]
fn capture_template_table_line_honors_explicit_table_line_position() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("project.org");
    fs::write(
        &note_path,
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n| Name | Value |\n|------+-------|\n| One  | 1     |\n| Two  | 2     |\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let parent = database
        .search_nodes("parent", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Parent")
        .expect("parent node should exist");

    capture_template(
        &root,
        Some(&parent),
        &CaptureTemplateParams {
            title: String::new(),
            file_path: None,
            node_key: Some(parent.node_key.clone()),
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::TableLine,
            content: String::from("Zero | 0"),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: Some(String::from("I+1")),
        },
    )?;

    assert_eq!(
        fs::read_to_string(&note_path)?,
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n| Name | Value |\n|------+-------|\n| Zero | 0 |\n| One  | 1     |\n| Two  | 2     |\n"
    );

    Ok(())
}

#[test]
fn capture_template_plain_file_target_appends_after_existing_headings() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    fs::write(&note_path, "#+title: Note\n\n* Existing\nBody.\n")?;

    let captured = capture_template(
        &root,
        None,
        &CaptureTemplateParams {
            title: String::from("Note"),
            file_path: Some(String::from("note.org")),
            node_key: None,
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Plain,
            content: String::from("Tail"),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    assert_eq!(captured.node_key, "file:note.org");
    assert_eq!(
        fs::read_to_string(&note_path)?,
        "#+title: Note\n\n* Existing\nBody.\nTail\n"
    );

    Ok(())
}

#[test]
fn capture_template_plain_file_prepend_skips_top_file_metadata() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    fs::write(
        &note_path,
        "#+title: Note\n:PROPERTIES:\n:ID: note-id\n:END:\n# Comment\n-----\nBody.\n",
    )?;

    capture_template(
        &root,
        None,
        &CaptureTemplateParams {
            title: String::from("Note"),
            file_path: Some(String::from("note.org")),
            node_key: None,
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Plain,
            content: String::from("Prepended"),
            refs: Vec::new(),
            prepend: true,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    assert_eq!(
        fs::read_to_string(&note_path)?,
        "#+title: Note\n:PROPERTIES:\n:ID: note-id\n:END:\n# Comment\n-----\nPrepended\nBody.\n"
    );

    Ok(())
}

#[test]
fn capture_template_item_file_target_finds_first_list_in_whole_buffer() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    fs::write(&note_path, "#+title: Note\n\n* Existing\n- One\n- Two\n")?;

    capture_template(
        &root,
        None,
        &CaptureTemplateParams {
            title: String::from("Note"),
            file_path: Some(String::from("note.org")),
            node_key: None,
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Item,
            content: String::from("Three"),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    assert_eq!(
        fs::read_to_string(&note_path)?,
        "#+title: Note\n\n* Existing\n- One\n- Two\n- Three\n"
    );

    Ok(())
}

#[test]
fn capture_template_item_prepend_renumbers_existing_ordered_lists() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("project.org");
    fs::write(
        &note_path,
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n1. One\n2. Two\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;
    let parent = database
        .search_nodes("parent", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Parent")
        .expect("parent node should exist");

    capture_template(
        &root,
        Some(&parent),
        &CaptureTemplateParams {
            title: String::new(),
            file_path: None,
            node_key: Some(parent.node_key.clone()),
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Item,
            content: String::from("Zero"),
            refs: Vec::new(),
            prepend: true,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    assert_eq!(
        fs::read_to_string(&note_path)?,
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n1. Zero\n2. One\n3. Two\n"
    );

    Ok(())
}

#[test]
fn capture_template_table_line_creates_table_when_missing() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("project.org");
    fs::write(
        &note_path,
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\nText\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;
    let parent = database
        .search_nodes("parent", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Parent")
        .expect("parent node should exist");

    capture_template(
        &root,
        Some(&parent),
        &CaptureTemplateParams {
            title: String::new(),
            file_path: None,
            node_key: Some(parent.node_key.clone()),
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::TableLine,
            content: String::from("Two | 2"),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    assert_eq!(
        fs::read_to_string(&note_path)?,
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\nText\n|   |\n|---|\n| Two | 2 |\n"
    );

    Ok(())
}

#[test]
fn capture_template_table_line_file_target_finds_existing_table_in_whole_buffer() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    fs::write(
        &note_path,
        "#+title: Note\n\n* Existing\n| Name |\n|------|\n| One  |\n",
    )?;

    capture_template(
        &root,
        None,
        &CaptureTemplateParams {
            title: String::from("Note"),
            file_path: Some(String::from("note.org")),
            node_key: None,
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::TableLine,
            content: String::from("Two"),
            refs: Vec::new(),
            prepend: true,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
    )?;

    assert_eq!(
        fs::read_to_string(&note_path)?,
        "#+title: Note\n\n* Existing\n| Name |\n|------|\n| Two |\n| One  |\n"
    );

    Ok(())
}

#[test]
fn preview_capture_template_leaves_new_target_unsaved() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");

    let preview = preview_capture_template(
        &root,
        None,
        &CaptureTemplateParams {
            title: String::from("Note"),
            file_path: Some(String::from("note.org")),
            node_key: None,
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Entry,
            content: String::from("* Note"),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
        None,
        false,
    )?;

    assert_eq!(preview.relative_path, "note.org");
    assert_eq!(preview.node_key, "heading:note.org:6");
    assert!(!note_path.exists());
    assert!(preview.content.contains("* Note"));

    Ok(())
}

#[test]
fn preview_capture_template_uses_source_override_and_can_assign_ids() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    fs::write(&note_path, "#+title: Note\n")?;

    let preview = preview_capture_template(
        &root,
        None,
        &CaptureTemplateParams {
            title: String::from("Note"),
            file_path: Some(String::from("note.org")),
            node_key: None,
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Entry,
            content: String::from("* Captured"),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
        Some("#+title: Note\nLocal edits\n"),
        true,
    )?;

    assert_eq!(fs::read_to_string(&note_path)?, "#+title: Note\n");
    assert!(preview.content.contains("Local edits"));

    let indexed = scan_source(&preview.relative_path, &preview.content);
    let node = indexed
        .nodes
        .into_iter()
        .find(|candidate| candidate.node_key == preview.node_key)
        .expect("preview node should be indexed from rendered content");
    assert!(node.explicit_id.is_some());

    Ok(())
}

#[test]
fn append_heading_to_existing_node_inserts_child_before_next_sibling() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("project.org");
    fs::write(
        &note_path,
        "#+title: Project\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\nBody.\n* Sibling\nSibling body.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;
    let parent = database
        .search_nodes("parent", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Parent")
        .expect("parent node should exist");

    let captured = append_heading_to_node(&root, &parent, "Child Task")?;
    let indexed = scan_path(&root, &captured.absolute_path)?;
    database.sync_file_index(&indexed)?;

    let source = fs::read_to_string(&note_path)?;
    assert!(source.contains(
        "* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\nBody.\n\n** Child Task\n* Sibling"
    ));

    let child = database
        .anchor_by_key(&captured.node_key)?
        .expect("captured child should exist");
    assert_eq!(child.title, "Child Task");
    assert_eq!(child.level, 2);

    Ok(())
}

#[test]
fn update_node_metadata_rewrites_file_level_refs_and_tags() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    fs::write(&note_path, "#+title: Note\n\n")?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let node = database
        .note_by_key("file:note.org")?
        .expect("file node should exist");
    let updated_path = update_node_metadata(
        &root,
        &node,
        &MetadataUpdate {
            aliases: None,
            refs: Some(vec!["https://example.test/ref".to_owned()]),
            tags: Some(vec!["beta".to_owned()]),
        },
    )?;
    let indexed = scan_path(&root, &updated_path)?;
    database.sync_file_index(&indexed)?;

    let source = fs::read_to_string(&note_path)?;
    assert!(source.contains("#+filetags: :beta:"));
    assert!(source.contains(":ROAM_REFS: https://example.test/ref"));

    let refreshed = database
        .note_by_key("file:note.org")?
        .expect("updated file node should exist");
    assert_eq!(refreshed.tags, vec!["beta"]);
    assert_eq!(refreshed.refs, vec!["https://example.test/ref"]);

    Ok(())
}

#[test]
fn update_node_metadata_rewrites_heading_aliases_and_tags() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    fs::write(
        &note_path,
        "#+title: Note\n\n* Heading :one:two:\n:PROPERTIES:\n:ID: heading-id\n:END:\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let node = database
        .search_nodes("heading", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Heading")
        .expect("heading node should exist");
    let updated_path = update_node_metadata(
        &root,
        &node,
        &MetadataUpdate {
            aliases: Some(vec!["Batman".to_owned()]),
            refs: None,
            tags: Some(vec!["two".to_owned()]),
        },
    )?;
    let indexed = scan_path(&root, &updated_path)?;
    database.sync_file_index(&indexed)?;

    let source = fs::read_to_string(&note_path)?;
    assert!(source.contains("* Heading :two:"));
    assert!(source.contains(":ROAM_ALIASES: Batman"));

    let refreshed = database
        .note_by_key(&node.node_key)?
        .expect("updated heading should exist");
    assert_eq!(refreshed.aliases, vec!["Batman"]);
    assert_eq!(refreshed.tags, vec!["two"]);

    Ok(())
}

#[test]
fn demote_entire_file_converts_file_metadata_into_a_root_heading() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    fs::write(
        &note_path,
        "#+title: Note\n#+filetags: :alpha:\n:PROPERTIES:\n:ID: note-id\n:END:\n\nFile body.\n\n* Child\nBody.\n",
    )?;

    let outcome = demote_entire_file(&root, "note.org")?;
    let source = fs::read_to_string(&note_path)?;
    assert!(source.starts_with("* Note :alpha:\n:PROPERTIES:\n:ID: note-id\n:END:\n"));
    assert!(source.contains("\nFile body.\n\n** Child\nBody.\n"));
    assert_eq!(outcome.node_key, "heading:note.org:1");

    let indexed = scan_path(&root, &note_path)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_file_index(&indexed)?;
    let node = database
        .note_by_key("heading:note.org:1")?
        .expect("demoted heading should exist");
    assert_eq!(node.title, "Note");
    assert_eq!(node.tags, vec!["alpha"]);
    assert_eq!(node.explicit_id.as_deref(), Some("note-id"));

    Ok(())
}

#[test]
fn promote_entire_file_converts_a_single_root_heading_into_a_file_node() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    fs::write(
        &note_path,
        "* Note :alpha:\n:PROPERTIES:\n:ID: note-id\n:END:\nBody.\n\n** Child\nBody.\n",
    )?;

    let outcome = promote_entire_file(&root, "note.org")?;
    let source = fs::read_to_string(&note_path)?;
    assert!(
        source
            .starts_with("#+title: Note\n#+filetags: :alpha:\n:PROPERTIES:\n:ID: note-id\n:END:\n")
    );
    assert!(source.contains("\nBody.\n\n* Child\nBody.\n"));
    assert_eq!(outcome.node_key, "file:note.org");

    let indexed = scan_path(&root, &note_path)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_file_index(&indexed)?;
    let node = database
        .note_by_key("file:note.org")?
        .expect("promoted file node should exist");
    assert_eq!(node.title, "Note");
    assert_eq!(node.tags, vec!["alpha"]);
    assert_eq!(node.explicit_id.as_deref(), Some("note-id"));

    Ok(())
}

#[test]
fn refile_subtree_moves_heading_between_files_and_preserves_source_file_note() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let source_path = root.join("source.org");
    let target_path = root.join("target.org");
    fs::write(&source_path, "#+title: Source\n\n* Move Me\nBody\n")?;
    fs::write(
        &target_path,
        "#+title: Target\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let source = database
        .search_anchors("move me", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Move Me")
        .expect("source heading should exist");
    let target = database
        .search_nodes("parent", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Parent")
        .expect("target heading should exist");

    let outcome = refile_subtree(&root, &source, &target)?;
    sync_rewrite_outcome(&mut database, &root, &outcome)?;

    assert_eq!(fs::read_to_string(&source_path)?, "#+title: Source\n\n");
    let target_source = fs::read_to_string(&target_path)?;
    assert!(target_source.contains(
        "* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n** Move Me\n:PROPERTIES:\n:ID: "
    ));
    assert!(target_source.contains("\nBody\n"));

    let moved = database
        .node_from_id(&outcome.explicit_id)?
        .expect("moved heading should be indexed");
    assert_eq!(moved.file_path, "target.org");
    assert_eq!(moved.level, 2);

    Ok(())
}

#[test]
fn refile_region_moves_selected_text_under_target_heading() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let source_path = root.join("source.org");
    let target_path = root.join("target.org");
    let source_body = "#+title: Source\nBody line one.\nBody line two.\n";
    fs::write(&source_path, source_body)?;
    fs::write(
        &target_path,
        "#+title: Target\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let target = database
        .search_nodes("parent", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Parent")
        .expect("target heading should exist");

    let body_start = source_body.find("Body").expect("body should exist");
    let body_end = source_body.len();
    let outcome = refile_region(
        &root,
        "source.org",
        source_body[..body_start].chars().count() + 1,
        source_body[..body_end].chars().count() + 1,
        &target,
    )?;
    sync_region_rewrite_outcome(&mut database, &root, &outcome)?;

    assert_eq!(fs::read_to_string(&source_path)?, "#+title: Source\n");
    assert_eq!(
        fs::read_to_string(&target_path)?,
        "#+title: Target\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\nBody line one.\nBody line two.\n"
    );

    Ok(())
}

#[test]
fn refile_region_same_file_adjusts_insertion_and_heading_levels() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let note_path = root.join("note.org");
    let source = "#+title: Note\n\n* Parent\n** Keep\n:PROPERTIES:\n:ID: keep-id\n:END:\nText\n** Move\nBody\n";
    fs::write(&note_path, source)?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let keep = database
        .search_nodes("keep", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Keep")
        .expect("target heading should exist");

    let region_start = source.find("** Move").expect("move heading should exist");
    let outcome = refile_region(
        &root,
        "note.org",
        source[..region_start].chars().count() + 1,
        source.chars().count() + 1,
        &keep,
    )?;
    sync_region_rewrite_outcome(&mut database, &root, &outcome)?;

    assert_eq!(
        fs::read_to_string(&note_path)?,
        "#+title: Note\n\n* Parent\n** Keep\n:PROPERTIES:\n:ID: keep-id\n:END:\nText\n*** Move\nBody\n"
    );

    let moved = database
        .search_anchors("move", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Move")
        .expect("moved heading should still be indexed");
    assert_eq!(moved.file_path, "note.org");
    assert_eq!(moved.level, 3);

    Ok(())
}

#[test]
fn refile_region_removes_source_file_when_selection_empties_it() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let source_path = root.join("source.org");
    let target_path = root.join("target.org");
    let source = "* Move Me\nBody\n";
    fs::write(&source_path, source)?;
    fs::write(
        &target_path,
        "#+title: Target\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let target = database
        .search_nodes("parent", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Parent")
        .expect("target heading should exist");

    let outcome = refile_region(&root, "source.org", 1, source.chars().count() + 1, &target)?;
    sync_region_rewrite_outcome(&mut database, &root, &outcome)?;

    assert!(!source_path.exists());
    let target_source = fs::read_to_string(&target_path)?;
    assert!(
        target_source.contains("* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n** Move Me\nBody\n")
    );

    Ok(())
}

#[test]
fn extract_subtree_promotes_heading_into_a_file_node() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let source_path = root.join("source.org");
    fs::write(
        &source_path,
        "#+title: Source\n\n* Move Me :tag:\nBody\n** Child\nMore\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let source = database
        .search_anchors("move me", 10, None)?
        .into_iter()
        .find(|candidate| candidate.title == "Move Me")
        .expect("source heading should exist");

    let outcome = extract_subtree(&root, &source, "moved.org")?;
    sync_rewrite_outcome(&mut database, &root, &outcome)?;

    assert_eq!(fs::read_to_string(&source_path)?, "#+title: Source\n\n");
    let moved_source = fs::read_to_string(root.join("moved.org"))?;
    assert!(moved_source.starts_with("#+title: Move Me\n#+filetags: :tag:\n"));
    assert!(moved_source.contains(":PROPERTIES:\n:ID: "));
    assert!(moved_source.contains("\nBody\n* Child\nMore\n"));

    let moved = database
        .node_from_id(&outcome.explicit_id)?
        .expect("extracted node should be indexed");
    assert_eq!(moved.kind.as_str(), "file");
    assert_eq!(moved.file_path, "moved.org");
    assert_eq!(moved.title, "Move Me");

    Ok(())
}

fn sync_rewrite_outcome(
    database: &mut Database,
    root: &Path,
    outcome: &RewriteOutcome,
) -> Result<()> {
    for path in &outcome.changed_paths {
        let indexed = scan_path(root, path)?;
        database.sync_file_index(&indexed)?;
    }

    for path in &outcome.removed_paths {
        let relative = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        database.remove_file_index(&relative)?;
    }

    Ok(())
}

fn sync_region_rewrite_outcome(
    database: &mut Database,
    root: &Path,
    outcome: &RegionRewriteOutcome,
) -> Result<()> {
    for path in &outcome.changed_paths {
        let indexed = scan_path(root, path)?;
        database.sync_file_index(&indexed)?;
    }

    for path in &outcome.removed_paths {
        let relative = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        database.remove_file_index(&relative)?;
    }

    Ok(())
}
