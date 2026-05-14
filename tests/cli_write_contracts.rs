use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{
    AgendaResult, AnchorRecord, BacklinksResult, ForwardLinksResult, IndexFileResult, IndexStats,
    NodeKind, NodeRecord, SearchFilesResult, SearchNodesResult, SearchOccurrencesResult,
    SearchTagsResult,
};
use tempfile::{TempDir, tempdir};

mod support;

use support::{run_json, run_slipbox, scoped_server_args_with_file_extension};

#[derive(Debug, Deserialize)]
struct ErrorPayload {
    error: ErrorMessage,
}

#[derive(Debug, Deserialize)]
struct ErrorMessage {
    message: String,
}

fn build_fixture() -> Result<(TempDir, PathBuf, PathBuf)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    Ok((workspace, root, db))
}

fn command(root: &Path, db: &Path, words: &[&str]) -> Vec<String> {
    let mut args = words
        .iter()
        .map(|word| (*word).to_owned())
        .collect::<Vec<_>>();
    args.extend(scoped_server_args_with_file_extension(root, db, "org"));
    args.push("--json".to_owned());
    args
}

#[test]
fn everyday_writes_are_visible_across_separate_cli_invocations() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;

    let mut create_note = command(&root, &db, &["note", "create"]);
    create_note.extend([
        "--title".to_owned(),
        "Seed".to_owned(),
        "--file".to_owned(),
        "seed.org".to_owned(),
        "--ref".to_owned(),
        "cite:seed2026".to_owned(),
    ]);
    let created: NodeRecord = run_json(&create_note)?;
    assert_eq!(created.node_key, "file:seed.org");
    assert_eq!(created.refs, vec!["@seed2026"]);

    let mut show_seed = command(&root, &db, &["node", "show"]);
    show_seed.extend(["--key".to_owned(), "file:seed.org".to_owned()]);
    let shown: NodeRecord = run_json(&show_seed)?;
    assert_eq!(shown.node_key, created.node_key);

    let mut captured_note = command(&root, &db, &["note", "create"]);
    captured_note.extend([
        "--title".to_owned(),
        "Captured Node".to_owned(),
        "--file".to_owned(),
        "captured.org".to_owned(),
        "--ref".to_owned(),
        "cite:captured2026".to_owned(),
    ]);
    let captured: NodeRecord = run_json(&captured_note)?;
    let captured_id = captured
        .explicit_id
        .as_deref()
        .expect("note create assigns an explicit ID")
        .to_owned();

    let mut resolve_captured_ref = command(&root, &db, &["ref", "resolve"]);
    resolve_captured_ref.push("cite:captured2026".to_owned());
    let captured_by_ref: NodeRecord = run_json(&resolve_captured_ref)?;
    assert_eq!(captured_by_ref.node_key, captured.node_key);

    let mut append_heading = command(&root, &db, &["note", "append-heading"]);
    append_heading.extend([
        "--file".to_owned(),
        "seed.org".to_owned(),
        "--title".to_owned(),
        "Seed".to_owned(),
        "--heading".to_owned(),
        format!(
            "TODO Follow-up\nSCHEDULED: <2026-05-13 Wed>\nLinks [[id:{captured_id}][Captured Node]]."
        ),
    ]);
    let appended: AnchorRecord = run_json(&append_heading)?;
    assert_eq!(appended.title, "Follow-up");
    assert_eq!(appended.todo_keyword.as_deref(), Some("TODO"));

    let mut forward_links = command(&root, &db, &["node", "forward-links"]);
    forward_links.extend(["--key".to_owned(), "file:seed.org".to_owned()]);
    let links: ForwardLinksResult = run_json(&forward_links)?;
    assert!(
        links
            .forward_links
            .iter()
            .any(|link| link.destination_note.node_key == captured.node_key)
    );

    let mut backlinks = command(&root, &db, &["node", "backlinks"]);
    backlinks.extend(["--id".to_owned(), captured_id.clone()]);
    let backlinked: BacklinksResult = run_json(&backlinks)?;
    assert!(
        backlinked
            .backlinks
            .iter()
            .any(|link| link.source_note.node_key == created.node_key)
    );

    let mut agenda = command(&root, &db, &["agenda", "date"]);
    agenda.push("2026-05-13".to_owned());
    let agenda_result: AgendaResult = run_json(&agenda)?;
    assert!(
        agenda_result
            .nodes
            .iter()
            .any(|node| node.title == "Follow-up")
    );

    let mut capture_template = command(&root, &db, &["capture", "template"]);
    capture_template.extend([
        "--file".to_owned(),
        "seed.org".to_owned(),
        "--type".to_owned(),
        "plain".to_owned(),
        "--content".to_owned(),
        "durable capture template phrase".to_owned(),
    ]);
    let captured_template: AnchorRecord = run_json(&capture_template)?;
    assert_eq!(captured_template.node_key, "file:seed.org");

    let mut occurrences = command(&root, &db, &["search", "occurrences"]);
    occurrences.push("durable capture template phrase".to_owned());
    let occurrence_result: SearchOccurrencesResult = run_json(&occurrences)?;
    assert!(
        occurrence_result
            .occurrences
            .iter()
            .any(|occurrence| occurrence.file_path == "seed.org")
    );

    let mut daily_append = command(&root, &db, &["daily", "append"]);
    daily_append.extend([
        "--date".to_owned(),
        "2026-05-13".to_owned(),
        "--heading".to_owned(),
        "TODO Daily follow-up\nSCHEDULED: <2026-05-13 Wed>".to_owned(),
    ]);
    let daily_heading: AnchorRecord = run_json(&daily_append)?;
    assert_eq!(daily_heading.file_path, "daily/2026-05-13.org");

    let agenda_after_daily: AgendaResult = run_json(&agenda)?;
    assert!(
        agenda_after_daily
            .nodes
            .iter()
            .any(|node| node.title == "Daily follow-up")
    );

    fs::write(root.join("needs-id.org"), "#+title: Needs ID\n")?;
    let mut sync_file = command(&root, &db, &["sync", "file"]);
    sync_file.push("needs-id.org".to_owned());
    let synced_file: IndexFileResult = run_json(&sync_file)?;
    assert_eq!(synced_file.file_path, "needs-id.org");

    let mut ensure_id = command(&root, &db, &["node", "ensure-id"]);
    ensure_id.extend(["--key".to_owned(), "file:needs-id.org".to_owned()]);
    let identified: AnchorRecord = run_json(&ensure_id)?;
    assert_eq!(identified.kind, NodeKind::File);
    let ensured_id = identified
        .explicit_id
        .as_deref()
        .expect("ensure-id assigns an explicit ID")
        .to_owned();

    let mut show_by_id = command(&root, &db, &["node", "show"]);
    show_by_id.extend(["--id".to_owned(), ensured_id]);
    let identified_node: NodeRecord = run_json(&show_by_id)?;
    assert_eq!(identified_node.node_key, "file:needs-id.org");

    let mut alias_add = command(&root, &db, &["node", "alias", "add"]);
    alias_add.extend([
        "--key".to_owned(),
        "file:seed.org".to_owned(),
        "Seed Alias".to_owned(),
    ]);
    let aliased: NodeRecord = run_json(&alias_add)?;
    assert_eq!(aliased.aliases, vec!["Seed Alias"]);

    let mut search_alias = command(&root, &db, &["node", "search"]);
    search_alias.push("Seed Alias".to_owned());
    let alias_search: SearchNodesResult = run_json(&search_alias)?;
    assert!(
        alias_search
            .nodes
            .iter()
            .any(|node| node.node_key == "file:seed.org")
    );

    let mut ref_set = command(&root, &db, &["node", "ref", "set"]);
    ref_set.extend([
        "--key".to_owned(),
        "file:seed.org".to_owned(),
        "cite:seed-updated".to_owned(),
    ]);
    let refed: NodeRecord = run_json(&ref_set)?;
    assert_eq!(refed.refs, vec!["@seed-updated"]);

    let mut resolve_updated_ref = command(&root, &db, &["ref", "resolve"]);
    resolve_updated_ref.push("cite:seed-updated".to_owned());
    let updated_ref_target: NodeRecord = run_json(&resolve_updated_ref)?;
    assert_eq!(updated_ref_target.node_key, "file:seed.org");

    let mut tag_set = command(&root, &db, &["node", "tag", "set"]);
    tag_set.extend([
        "--key".to_owned(),
        "file:seed.org".to_owned(),
        "project".to_owned(),
    ]);
    let tagged: NodeRecord = run_json(&tag_set)?;
    assert_eq!(tagged.tags, vec!["project"]);

    let mut tag_search = command(&root, &db, &["tag", "search"]);
    tag_search.push("project".to_owned());
    let tags: SearchTagsResult = run_json(&tag_search)?;
    assert!(tags.tags.iter().any(|tag| tag == "project"));

    fs::write(root.join("root-sync.org"), "#+title: Root Synced\n")?;
    let sync_root: IndexStats = run_json(&command(&root, &db, &["sync", "root"]))?;
    assert!(sync_root.files_indexed >= 5);

    let mut file_search = command(&root, &db, &["file", "search"]);
    file_search.push("Root Synced".to_owned());
    let files: SearchFilesResult = run_json(&file_search)?;
    assert!(
        files
            .files
            .iter()
            .any(|file| file.file_path == "root-sync.org")
    );

    Ok(())
}

#[test]
fn everyday_write_failures_return_structured_json_errors() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let mut args = command(&root, &db, &["note", "append-to-node"]);
    args.extend([
        "--id".to_owned(),
        "missing-id".to_owned(),
        "--heading".to_owned(),
        "Should Fail".to_owned(),
    ]);

    let output = run_slipbox(&args)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(error.error.message.contains("unknown node id: missing-id"));

    Ok(())
}
