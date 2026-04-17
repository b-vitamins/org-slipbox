#[path = "../src/occurrences_query.rs"]
mod occurrences_query;
#[path = "../src/reflinks_query.rs"]
mod reflinks_query;
#[path = "../src/text_query.rs"]
mod text_query;
#[path = "../src/unlinked_references_query.rs"]
mod unlinked_references_query;

use std::fs;
use std::{thread, time::Duration};

use anyhow::Result;
use occurrences_query::query_occurrences;
use reflinks_query::query_reflinks;
use slipbox_core::{AnchorRecord, SearchNodesSort};
use slipbox_index::{
    DiscoveryPolicy, scan_path, scan_path_with_policy, scan_root, scan_root_with_policy,
};
use slipbox_store::Database;
use tempfile::tempdir;
use unlinked_references_query::query_unlinked_references;

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

    let results = database.search_nodes("target", 10, None)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Target heading");

    let backlinks = database.backlinks(&results[0].node_key, 10, false)?;
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].source_note.title, "First heading");
    assert_eq!(backlinks[0].row, 7);
    assert_eq!(backlinks[0].col, 5);
    assert_eq!(backlinks[0].preview, "See [[id:beta-target][Beta]].");

    Ok(())
}

#[test]
fn node_queries_return_indexed_metadata_and_graph_counts() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\n\n* First heading\n:PROPERTIES:\n:ID: alpha-first\n:END:\nSee [[id:beta-target][Beta]].\n",
    )?;
    fs::write(
        root.join("gamma.org"),
        "#+title: Gamma\n\n* Second heading\n:PROPERTIES:\n:ID: gamma-second\n:END:\nSee [[id:beta-target][Beta again]].\n",
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
        .search_nodes("target", 10, None)?
        .into_iter()
        .next()
        .expect("expected target node");
    assert!(target.file_mtime_ns > 0);
    assert_eq!(target.backlink_count, 2);
    assert_eq!(target.forward_link_count, 0);

    let source = database
        .search_nodes("first", 10, None)?
        .into_iter()
        .next()
        .expect("expected source node");
    assert!(source.file_mtime_ns > 0);
    assert_eq!(source.backlink_count, 0);
    assert_eq!(source.forward_link_count, 1);

    let backlinks = database.backlinks(&target.node_key, 10, false)?;
    assert_eq!(backlinks.len(), 2);
    assert_eq!(backlinks[0].source_note.title, "First heading");
    assert!(backlinks[0].source_note.file_mtime_ns > 0);
    assert_eq!(backlinks[0].source_note.backlink_count, 0);
    assert_eq!(backlinks[0].source_note.forward_link_count, 1);

    Ok(())
}

#[test]
fn search_nodes_support_named_sorts() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("zeta.org"),
        ":PROPERTIES:\n:ID: alpha-id\n:END:\n#+title: Common Zulu\n\nSee [[id:beta-id][Alpha]].\nSee [[id:gamma-id][Beta]].\n",
    )?;
    thread::sleep(Duration::from_millis(20));
    fs::write(
        root.join("alpha.org"),
        ":PROPERTIES:\n:ID: beta-id\n:END:\n#+title: Common Alpha\n\nTarget body.\n",
    )?;
    thread::sleep(Duration::from_millis(20));
    fs::write(
        root.join("beta.org"),
        ":PROPERTIES:\n:ID: gamma-id\n:END:\n#+title: Common Beta\n\nSee [[id:beta-id][Alpha again]].\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let default = database.search_nodes("common", 10, None)?;
    let relevance = database.search_nodes("common", 10, Some(SearchNodesSort::Relevance))?;
    assert_eq!(
        default
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        relevance
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>()
    );

    let title_sorted = database.search_nodes("common", 10, Some(SearchNodesSort::Title))?;
    assert_eq!(
        title_sorted
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Common Alpha", "Common Beta", "Common Zulu"]
    );

    let file_sorted = database.search_nodes("common", 10, Some(SearchNodesSort::File))?;
    assert_eq!(
        file_sorted
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Common Alpha", "Common Beta", "Common Zulu"]
    );

    let mtime_sorted = database.search_nodes("common", 10, Some(SearchNodesSort::FileMtime))?;
    assert_eq!(
        mtime_sorted
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Common Beta", "Common Alpha", "Common Zulu"]
    );

    let backlink_sorted =
        database.search_nodes("common", 10, Some(SearchNodesSort::BacklinkCount))?;
    assert_eq!(
        backlink_sorted
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Common Alpha", "Common Beta", "Common Zulu"]
    );

    let forward_sorted =
        database.search_nodes("common", 10, Some(SearchNodesSort::ForwardLinkCount))?;
    assert_eq!(
        forward_sorted
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Common Zulu", "Common Beta", "Common Alpha"]
    );

    Ok(())
}

#[test]
fn public_node_search_excludes_anonymous_headings() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha Target\n\n* Anonymous Target\n* Identified Target\n:PROPERTIES:\n:ID: identified-target\n:END:\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let selectable = database.search_nodes("target", 10, None)?;
    assert_eq!(
        selectable
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha Target", "Identified Target"]
    );

    let indexed = database.search_anchors("target", 10, None)?;
    assert_eq!(
        indexed
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha Target", "Anonymous Target", "Identified Target"]
    );

    Ok(())
}

#[test]
fn exact_title_lookup_excludes_anonymous_headings() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Target\n\n* Target\nAnonymous heading.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let selectable = database.node_from_title_or_alias("Target", true)?;
    assert_eq!(selectable.len(), 1);
    assert_eq!(selectable[0].kind.as_str(), "file");

    let indexed = database.search_anchors("Target", 10, Some(SearchNodesSort::Title))?;
    assert_eq!(indexed.len(), 2);
    assert_eq!(indexed[0].kind.as_str(), "file");
    assert_eq!(indexed[1].kind.as_str(), "heading");
    assert!(indexed[1].explicit_id.is_none());

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
        .search_nodes("target", 10, None)?
        .into_iter()
        .next()
        .expect("expected target node");

    let backlinks = database.backlinks(&target.node_key, 10, false)?;
    assert_eq!(backlinks.len(), 3);

    let unique_backlinks = database.backlinks(&target.node_key, 10, true)?;
    assert_eq!(unique_backlinks.len(), 2);
    assert_eq!(unique_backlinks[0].source_note.title, "First heading");
    assert_eq!(unique_backlinks[0].row, 7);
    assert_eq!(unique_backlinks[1].source_note.title, "Second heading");

    Ok(())
}

#[test]
fn forward_links_support_unique_destinations_and_skip_missing_targets() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\n\n* Source heading\n:PROPERTIES:\n:ID: alpha-source\n:END:\nSee [[id:beta-target][Beta]].\nSee [[id:missing-target][Missing]].\nSee [[id:beta-target][Beta again]].\nSee [[id:gamma-target][Gamma]].\n",
    )?;
    fs::write(
        root.join("beta.org"),
        "#+title: Beta\n\n* Target heading\n:PROPERTIES:\n:ID: beta-target\n:END:\nTarget body.\n",
    )?;
    fs::write(
        root.join("gamma.org"),
        "#+title: Gamma\n\n* Another target\n:PROPERTIES:\n:ID: gamma-target\n:END:\nTarget body.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let source = database
        .search_nodes("source", 10, None)?
        .into_iter()
        .next()
        .expect("expected source node");

    let forward_links = database.forward_links(&source.node_key, 10, false)?;
    assert_eq!(forward_links.len(), 3);
    assert_eq!(forward_links[0].destination_note.title, "Target heading");
    assert_eq!(forward_links[0].row, 7);
    assert_eq!(forward_links[0].col, 5);
    assert_eq!(forward_links[1].destination_note.title, "Target heading");
    assert_eq!(forward_links[1].row, 9);
    assert_eq!(forward_links[2].destination_note.title, "Another target");
    assert_eq!(forward_links[2].row, 10);
    assert!(
        forward_links
            .iter()
            .all(|record| record.destination_note.title != "Missing")
    );

    let unique_forward_links = database.forward_links(&source.node_key, 10, true)?;
    assert_eq!(unique_forward_links.len(), 2);
    assert_eq!(
        unique_forward_links[0].destination_note.title,
        "Target heading"
    );
    assert_eq!(unique_forward_links[0].row, 7);
    assert_eq!(
        unique_forward_links[1].destination_note.title,
        "Another target"
    );
    assert_eq!(unique_forward_links[1].row, 10);

    Ok(())
}

#[test]
fn reflinks_query_returns_structured_hits_and_skips_current_subtree() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("current.org"),
        "#+title: Current\n\n* Source heading\n:PROPERTIES:\n:ID: source-id\n:ROAM_REFS: @smith2024\n:END:\nCurrent cite:smith2024 should stay hidden.\n** Child heading\nChild @smith2024 should stay hidden.\n* Sibling heading\nSibling cite:smith2024 should surface.\n",
    )?;
    fs::write(
        root.join("other.org"),
        "#+title: Other\n\nPreamble @SMITH2024 should surface.\n* Another heading\nBody cite:smith2024 should surface.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let source = database
        .search_nodes("source", 10, None)?
        .into_iter()
        .next()
        .expect("expected source node");

    let source_anchor = AnchorRecord::from(source.clone());
    let reflinks = query_reflinks(&database, &root, &source_anchor, 10)?;
    assert_eq!(reflinks.len(), 3);

    assert_eq!(reflinks[0].source_anchor.title, "Sibling heading");
    assert_eq!(reflinks[0].row, 12);
    assert_eq!(reflinks[0].col, 9);
    assert_eq!(
        reflinks[0].preview,
        "Sibling cite:smith2024 should surface."
    );
    assert_eq!(reflinks[0].matched_reference, "cite:smith2024");

    assert_eq!(reflinks[1].source_anchor.title, "Other");
    assert_eq!(reflinks[1].row, 3);
    assert_eq!(reflinks[1].col, 10);
    assert_eq!(reflinks[1].preview, "Preamble @SMITH2024 should surface.");
    assert_eq!(reflinks[1].matched_reference, "@SMITH2024");

    assert_eq!(reflinks[2].source_anchor.title, "Another heading");
    assert_eq!(reflinks[2].row, 5);
    assert_eq!(reflinks[2].col, 6);
    assert_eq!(reflinks[2].preview, "Body cite:smith2024 should surface.");
    assert_eq!(reflinks[2].matched_reference, "cite:smith2024");

    Ok(())
}

#[test]
fn unlinked_references_query_returns_title_and_alias_hits() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("current.org"),
        "#+title: Current\n\n* Project Atlas\n:PROPERTIES:\n:ID: atlas-id\n:ROAM_ALIASES: AtlasPlan\n:END:\nProject Atlas should stay hidden.\n** Child heading\nAtlasPlan should stay hidden.\n* Sibling heading\nProject Atlas should surface.\nLinked [[id:atlas-id][Project Atlas]] should stay hidden as linked.\nLinked [[id:atlas-id][AtlasPlan]] should stay hidden as linked.\n",
    )?;
    fs::write(
        root.join("other.org"),
        "#+title: Other\n\nproject atlas should surface.\nATLASPLAN should surface.\nAtlasPlanner should stay out.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let source = database
        .search_nodes("project atlas", 10, None)?
        .into_iter()
        .next()
        .expect("expected project atlas node");

    let source_anchor = AnchorRecord::from(source.clone());
    let unlinked_references = query_unlinked_references(&database, &root, &source_anchor, 10)?;
    assert_eq!(unlinked_references.len(), 3);

    assert_eq!(
        unlinked_references[0].source_anchor.title,
        "Sibling heading"
    );
    assert_eq!(unlinked_references[0].row, 12);
    assert_eq!(unlinked_references[0].col, 1);
    assert_eq!(
        unlinked_references[0].preview,
        "Project Atlas should surface."
    );
    assert_eq!(unlinked_references[0].matched_text, "Project Atlas");

    assert_eq!(unlinked_references[1].source_anchor.title, "Other");
    assert_eq!(unlinked_references[1].row, 3);
    assert_eq!(unlinked_references[1].col, 1);
    assert_eq!(
        unlinked_references[1].preview,
        "project atlas should surface."
    );
    assert_eq!(unlinked_references[1].matched_text, "project atlas");

    assert_eq!(unlinked_references[2].source_anchor.title, "Other");
    assert_eq!(unlinked_references[2].row, 4);
    assert_eq!(unlinked_references[2].col, 1);
    assert_eq!(unlinked_references[2].preview, "ATLASPLAN should surface.");
    assert_eq!(unlinked_references[2].matched_text, "ATLASPLAN");

    Ok(())
}

#[test]
fn unlinked_references_query_supports_quoted_multi_word_aliases() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("current.org"),
        "#+title: Current\n\n* Project Atlas\n:PROPERTIES:\n:ID: atlas-id\n:ROAM_ALIASES: \"Atlas Plan\"\n:END:\nProject Atlas should stay hidden.\n** Child heading\nAtlas Plan should stay hidden.\n* Sibling heading\nAtlas Plan should surface here.\nLinked [[id:atlas-id][Atlas Plan]] should stay hidden as linked.\n",
    )?;
    fs::write(
        root.join("other.org"),
        "#+title: Other\n\nATLAS PLAN should surface.\nAtlas Planner should stay out.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let source = database
        .node_from_title_or_alias("Atlas Plan", true)?
        .into_iter()
        .next()
        .expect("expected project atlas node");

    let source_anchor = AnchorRecord::from(source.clone());
    let unlinked_references = query_unlinked_references(&database, &root, &source_anchor, 10)?;
    assert_eq!(unlinked_references.len(), 2);

    assert_eq!(
        unlinked_references[0].source_anchor.title,
        "Sibling heading"
    );
    assert_eq!(unlinked_references[0].row, 12);
    assert_eq!(unlinked_references[0].col, 1);
    assert_eq!(
        unlinked_references[0].preview,
        "Atlas Plan should surface here."
    );
    assert_eq!(unlinked_references[0].matched_text, "Atlas Plan");

    assert_eq!(unlinked_references[1].source_anchor.title, "Other");
    assert_eq!(unlinked_references[1].row, 3);
    assert_eq!(unlinked_references[1].col, 1);
    assert_eq!(unlinked_references[1].preview, "ATLAS PLAN should surface.");
    assert_eq!(unlinked_references[1].matched_text, "ATLAS PLAN");

    Ok(())
}

#[test]
fn occurrence_query_returns_structured_hits_and_honors_limits() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\n:PROPERTIES:\n:ROAM_REFS: @cite000000\n:END:\n\nNeedle in preamble.\n* First heading\nNeedle in first heading body.\n",
    )?;
    fs::write(
        root.join("beta.org"),
        "#+title: Beta\n\n* Second heading\nNeedle in beta heading body.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    assert!(query_occurrences(&database, "   ", 10)?.is_empty());

    let occurrences = query_occurrences(&database, "needle", 10)?;
    assert_eq!(occurrences.len(), 3);

    assert_eq!(occurrences[0].file_path, "alpha.org");
    assert_eq!(occurrences[0].row, 6);
    assert_eq!(occurrences[0].col, 1);
    assert_eq!(occurrences[0].preview, "Needle in preamble.");
    assert_eq!(occurrences[0].matched_text, "Needle");
    assert_eq!(
        occurrences[0]
            .owning_anchor
            .as_ref()
            .expect("file preamble should resolve file node")
            .title,
        "Alpha"
    );

    assert_eq!(occurrences[1].file_path, "alpha.org");
    assert_eq!(occurrences[1].row, 8);
    assert_eq!(occurrences[1].col, 1);
    assert_eq!(occurrences[1].preview, "Needle in first heading body.");
    assert_eq!(
        occurrences[1]
            .owning_anchor
            .as_ref()
            .expect("heading body should resolve heading node")
            .title,
        "First heading"
    );

    assert_eq!(occurrences[2].file_path, "beta.org");
    assert_eq!(occurrences[2].row, 4);
    assert_eq!(occurrences[2].col, 1);
    assert_eq!(occurrences[2].preview, "Needle in beta heading body.");
    assert_eq!(
        occurrences[2]
            .owning_anchor
            .as_ref()
            .expect("heading body should resolve heading node")
            .title,
        "Second heading"
    );

    let limited = query_occurrences(&database, "NEEDLE", 2)?;
    assert_eq!(limited.len(), 2);
    assert_eq!(limited[0].file_path, "alpha.org");
    assert_eq!(limited[0].row, 6);
    assert_eq!(limited[1].file_path, "alpha.org");
    assert_eq!(limited[1].row, 8);

    let infix = query_occurrences(&database, "eedl", 10)?;
    assert_eq!(infix.len(), 3);
    assert_eq!(infix[0].file_path, "alpha.org");
    assert_eq!(infix[0].row, 6);
    assert_eq!(infix[0].matched_text, "eedl");

    let short = query_occurrences(&database, "NE", 10)?;
    assert!(short.is_empty());

    let punctuated = query_occurrences(&database, "@cite000000", 10)?;
    assert_eq!(punctuated.len(), 1);
    assert_eq!(punctuated[0].file_path, "alpha.org");
    assert_eq!(punctuated[0].row, 3);
    assert_eq!(punctuated[0].preview, ":ROAM_REFS: @cite000000");
    assert_eq!(punctuated[0].matched_text, "@cite000000");

    Ok(())
}

#[test]
fn node_exclusion_respects_file_heading_inheritance_and_explicit_clearing() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("target.org"),
        "#+title: Target\n\n* Target heading\n:PROPERTIES:\n:ID: target-id\n:END:\nTarget body.\n",
    )?;
    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\n\n* Visible heading\n:PROPERTIES:\n:ID: visible-id\n:END:\nVisible body.\n\n* Excluded parent\n:PROPERTIES:\n:ID: excluded-parent-id\n:ROAM_EXCLUDE:\n:END:\nThis link should not count [[id:target-id][Target]].\n** Inherited excluded child\n:PROPERTIES:\n:ID: inherited-child-id\n:END:\nStill excluded.\n** Reincluded child\n:PROPERTIES:\n:ID: reincluded-child-id\n:ROAM_EXCLUDE: nil\n:END:\nThis link should count [[id:target-id][Target]].\n*** Included grandchild\n:PROPERTIES:\n:ID: included-grandchild-id\n:END:\nVisible again.\n",
    )?;
    fs::write(
        root.join("excluded-file.org"),
        ":PROPERTIES:\n:ID: excluded-file-id\n:ROAM_EXCLUDE: t\n:END:\n#+title: Excluded File\n\n* Hidden file heading\n:PROPERTIES:\n:ID: hidden-file-heading-id\n:END:\nThis link should not count [[id:target-id][Target]].\n\n* Reincluded file heading\n:PROPERTIES:\n:ID: reincluded-file-heading-id\n:ROAM_EXCLUDE: nil\n:END:\nThis link should count [[id:target-id][Target]].\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    let stats = database.sync_index(&files)?;

    assert_eq!(stats.files_indexed, 3);
    assert_eq!(stats.links_indexed, 2);

    assert!(database.node_from_id("visible-id")?.is_some());
    assert!(database.node_from_id("reincluded-child-id")?.is_some());
    assert!(database.node_from_id("included-grandchild-id")?.is_some());
    assert!(
        database
            .node_from_id("reincluded-file-heading-id")?
            .is_some()
    );
    assert!(database.node_from_id("target-id")?.is_some());

    assert!(database.node_from_id("excluded-file-id")?.is_none());
    assert!(database.node_from_id("excluded-parent-id")?.is_none());
    assert!(database.node_from_id("inherited-child-id")?.is_none());
    assert!(database.node_from_id("hidden-file-heading-id")?.is_none());

    assert!(
        database
            .node_from_title_or_alias("Excluded parent", true)?
            .is_empty()
    );
    assert!(
        database
            .node_from_title_or_alias("Hidden file heading", true)?
            .is_empty()
    );

    let target = database
        .node_from_id("target-id")?
        .expect("target heading should remain indexed");
    let backlinks = database.backlinks(&target.node_key, 10, false)?;
    assert_eq!(backlinks.len(), 2);
    assert_eq!(backlinks[0].source_note.title, "Reincluded child");
    assert_eq!(backlinks[1].source_note.title, "Reincluded file heading");

    let files = database.search_files("", 10)?;
    let excluded_file = files
        .iter()
        .find(|record| record.file_path == "excluded-file.org")
        .expect("excluded file should remain in the file surface");
    assert_eq!(excluded_file.title, "Excluded File");
    assert_eq!(excluded_file.node_count, 1);
    let title_match = database.search_files("excluded file", 10)?;
    assert_eq!(title_match.len(), 1);
    assert_eq!(title_match[0].file_path, "excluded-file.org");

    Ok(())
}

#[test]
fn node_exclusion_keeps_text_queries_out_of_excluded_subtrees() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("target.org"),
        "#+title: Target\n\n* Project Atlas\n:PROPERTIES:\n:ID: atlas-id\n:ROAM_ALIASES: AtlasPlan\n:ROAM_REFS: @smith2024\n:END:\nTarget body.\n",
    )?;
    fs::write(
        root.join("mentions.org"),
        "#+title: Mentions\n\n* Visible heading\nVisible cite:smith2024 should surface.\nVisible AtlasPlan should surface.\n\n* Excluded heading\n:PROPERTIES:\n:ROAM_EXCLUDE:\n:END:\nExcluded cite:smith2024 should stay hidden.\nExcluded AtlasPlan should stay hidden.\n** Excluded child\nChild AtlasPlan should stay hidden.\n\n* Reincluded heading\n:PROPERTIES:\n:ROAM_EXCLUDE: nil\n:END:\nReincluded cite:smith2024 should surface.\nReincluded AtlasPlan should surface.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let target = database
        .node_from_id("atlas-id")?
        .expect("target node should remain indexed");

    let target_anchor = AnchorRecord::from(target.clone());
    let reflinks = query_reflinks(&database, &root, &target_anchor, 10)?;
    assert_eq!(reflinks.len(), 2);
    assert_eq!(reflinks[0].source_anchor.title, "Visible heading");
    assert_eq!(
        reflinks[0].preview,
        "Visible cite:smith2024 should surface."
    );
    assert_eq!(reflinks[1].source_anchor.title, "Reincluded heading");
    assert_eq!(
        reflinks[1].preview,
        "Reincluded cite:smith2024 should surface."
    );

    let target_anchor = AnchorRecord::from(target.clone());
    let unlinked_references = query_unlinked_references(&database, &root, &target_anchor, 10)?;
    assert_eq!(unlinked_references.len(), 2);
    assert_eq!(
        unlinked_references[0].source_anchor.title,
        "Visible heading"
    );
    assert_eq!(
        unlinked_references[0].preview,
        "Visible AtlasPlan should surface."
    );
    assert_eq!(
        unlinked_references[1].source_anchor.title,
        "Reincluded heading"
    );
    assert_eq!(
        unlinked_references[1].preview,
        "Reincluded AtlasPlan should surface."
    );

    Ok(())
}

#[test]
fn reports_index_stats_and_indexed_files() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(root.join("alpha.org"), "#+title: Alpha\n")?;
    fs::write(root.join("beta.org"), "#+title: Beta\n\n* Heading\n")?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let stats = database.stats()?;
    assert_eq!(stats.files_indexed, 2);
    assert_eq!(stats.nodes_indexed, 3);
    assert_eq!(stats.links_indexed, 0);

    assert_eq!(
        database.indexed_files()?,
        vec!["alpha.org".to_owned(), "beta.org".to_owned()]
    );

    Ok(())
}

#[test]
fn search_files_returns_indexed_records_and_matches_path_and_title() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(root.join("nested"))?;

    fs::write(root.join("alpha.org"), "#+title: Alpha File\n")?;
    fs::write(
        root.join("nested").join("beta.org"),
        "#+title: Project Beta\n\n* Heading\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let all_files = database.search_files("", 10)?;
    assert_eq!(
        all_files
            .iter()
            .map(|file| file.file_path.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha.org", "nested/beta.org"]
    );
    assert_eq!(all_files[0].title, "Alpha File");
    assert!(all_files[0].mtime_ns > 0);
    assert_eq!(all_files[0].node_count, 1);
    assert_eq!(all_files[1].title, "Project Beta");
    assert!(all_files[1].mtime_ns > 0);
    assert_eq!(all_files[1].node_count, 2);

    let path_match = database.search_files("nested/beta", 10)?;
    assert_eq!(path_match.len(), 1);
    assert_eq!(path_match[0].file_path, "nested/beta.org");

    let title_match = database.search_files("project beta", 10)?;
    assert_eq!(title_match.len(), 1);
    assert_eq!(title_match[0].title, "Project Beta");

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
fn random_node_excludes_anonymous_headings() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        "#+title: Alpha\n\n* Hidden random heading\nAnonymous heading.\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let node = database
        .random_node()?
        .expect("expected selectable random node");
    assert_eq!(node.title, "Alpha");
    assert_eq!(node.kind.as_str(), "file");

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
            .note_by_key("file:alpha.org")?
            .expect("alpha node should still exist")
            .title,
        "Alpha Updated"
    );
    assert_eq!(
        database
            .note_by_key("file:beta.org")?
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
