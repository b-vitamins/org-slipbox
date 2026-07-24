use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use slipbox_core::{
    BacklinksParams, ForwardLinksParams, GlossaryDueParams, GlossaryTermParams,
    ListGlossaryTermsParams, NodeFromIdParams, NodeFromKeyParams, NodeFromTitleOrAliasParams,
    NoteContextParams, ReflinksParams, SearchGlossaryParams, SearchNodesParams,
    UnlinkedReferencesParams,
};
use slipbox_daemon_client::DaemonServeConfig;
use slipbox_index::scan_root;
use slipbox_store::Database;
use slipbox_web::ReadingBridge;
use tempfile::{TempDir, tempdir};

fn daemon_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

/// Four files: a linked note pair, a weak peer sharing a ref, and one glossary
/// term. The index is built here, so the daemon under test only serves it.
fn build_reading_fixture() -> Result<(TempDir, PathBuf, PathBuf)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        r#":PROPERTIES:
:ID: alpha-id
:ROAM_REFS: cite:shared2024 cite:alpha2024
:END:
#+title: Alpha
#+filetags: :alpha:shared:

See [[id:beta-id][Beta]].
"#,
    )?;
    fs::write(
        root.join("beta.org"),
        r#":PROPERTIES:
:ID: beta-id
:ROAM_REFS: cite:shared2024 cite:beta2024
:END:
#+title: Beta

Beta stands on its own.
"#,
    )?;
    fs::write(
        root.join("weak.org"),
        r#":PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:shared2024
:END:
#+title: Weak

Weakly integrated peer with shared references and no direct links.
Alpha appears here as unlinked text.
"#,
    )?;
    fs::write(
        root.join("riemann.org"),
        r#"#+title: Riemann integral
#+glossary: t
:PROPERTIES:
:ID:              riemann-id
:GLOSSARY_STATUS: confirmed
:SR_DUE:          2026-08-01
:SR_EASE:         2.50
:SR_INTERVAL:     6
:SR_REPS:         3
:SR_LAST:         2026-07-26
:END:

A definite integral defined as the limit of Riemann sums.
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;

    Ok((workspace, root, db))
}

#[test]
fn reading_bridge_serves_the_read_only_note_and_glossary_surface() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let bridge = ReadingBridge::spawn(daemon_binary(), DaemonServeConfig::new(&root, &db))?;

    // The bridge always spawns a read-only daemon, so every answer below is also
    // evidence the method passes the read-only dispatch guard.
    let ping = bridge.ping()?;
    assert_eq!(ping.root, root.canonicalize()?.display().to_string());

    let status = bridge.status()?;
    assert_eq!(status.files_indexed, 4);
    assert_eq!(status.nodes_indexed, 4);

    let alpha = bridge
        .search_nodes(&SearchNodesParams {
            query: "Alpha".to_owned(),
            limit: 10,
            sort: None,
        })?
        .nodes
        .into_iter()
        .find(|node| node.title == "Alpha")
        .context("Alpha should resolve from search")?;

    let random = bridge.random_node()?;
    assert!(random.node.is_some());

    let by_id = bridge
        .node_from_id(&NodeFromIdParams {
            id: "alpha-id".to_owned(),
        })?
        .context("Alpha should resolve by ID")?;
    assert_eq!(by_id.node_key, alpha.node_key);

    let by_key = bridge
        .node_from_key(&NodeFromKeyParams {
            node_key: alpha.node_key.clone(),
        })?
        .context("Alpha should resolve by key")?;
    assert_eq!(by_key.node_key, alpha.node_key);

    let by_title = bridge
        .node_from_title_or_alias(&NodeFromTitleOrAliasParams {
            title_or_alias: "Beta".to_owned(),
            nocase: false,
        })?
        .context("Beta should resolve by title")?;
    assert_eq!(by_title.title, "Beta");

    let context = bridge.note_context(&NoteContextParams {
        node_key: alpha.node_key.clone(),
        source_context_before: Some(0),
        source_context_after: Some(0),
        source_max_lines: Some(20),
        relation_limit: Some(10),
    })?;
    assert_eq!(context.note.title, "Alpha");
    assert!(context.source.content.contains("#+title: Alpha"));
    assert_eq!(context.forward_links.len(), 1);

    let backlinks = bridge.backlinks(&BacklinksParams {
        node_key: by_title.node_key.clone(),
        limit: 10,
        unique: false,
    })?;
    assert_eq!(backlinks.backlinks.len(), 1);
    assert_eq!(backlinks.backlinks[0].source_note.title, "Alpha");

    let forward_links = bridge.forward_links(&ForwardLinksParams {
        node_key: alpha.node_key.clone(),
        limit: 10,
        unique: false,
    })?;
    assert_eq!(forward_links.forward_links.len(), 1);
    assert_eq!(
        forward_links.forward_links[0].destination_note.title,
        "Beta"
    );

    let reflinks = bridge.reflinks(&ReflinksParams {
        node_key: alpha.node_key.clone(),
        limit: 10,
    })?;
    assert!(
        reflinks
            .reflinks
            .iter()
            .any(|record| record.source_anchor.title == "Weak")
    );

    let unlinked = bridge.unlinked_references(&UnlinkedReferencesParams {
        node_key: alpha.node_key.clone(),
        limit: 10,
    })?;
    assert!(
        unlinked
            .unlinked_references
            .iter()
            .any(|record| record.source_anchor.title == "Weak")
    );

    let terms = bridge.list_glossary_terms(&ListGlossaryTermsParams { limit: 50 })?;
    assert_eq!(terms.terms.len(), 1);
    let riemann_key = terms.terms[0].node_key.clone();
    assert_eq!(terms.terms[0].title, "Riemann integral");

    let searched = bridge.search_glossary(&SearchGlossaryParams {
        query: "Riemann".to_owned(),
        limit: 50,
    })?;
    assert_eq!(searched.terms.len(), 1);
    assert_eq!(searched.terms[0].node_key, riemann_key);

    let term = bridge
        .glossary_term(&GlossaryTermParams {
            node_key: riemann_key.clone(),
        })?
        .term
        .context("known glossary term should resolve")?;
    assert_eq!(term.sr_due.as_deref(), Some("2026-08-01"));

    // The fixture term's `SR_DUE` is 2026-08-01, so these two dates straddle it.
    let not_yet = bridge.glossary_due(&GlossaryDueParams {
        today: Some("2026-07-26".to_owned()),
        limit: 50,
    })?;
    assert!(not_yet.terms.is_empty());
    let due = bridge.glossary_due(&GlossaryDueParams {
        today: Some("2026-08-02".to_owned()),
        limit: 50,
    })?;
    assert_eq!(due.terms.len(), 1);
    assert_eq!(due.terms[0].node_key, riemann_key);

    bridge.shutdown()?;
    Ok(())
}

#[test]
fn reading_bridge_serves_concurrent_readers_over_one_pipe() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let bridge = Arc::new(ReadingBridge::spawn(
        daemon_binary(),
        DaemonServeConfig::new(&root, &db),
    )?);

    // Each reader asks for a different note, so an answer delivered to the wrong
    // thread fails an assertion. Readers all asking for the same note would pass
    // even if the pipe handed every response to whichever thread arrived next.
    let asked = [
        ("alpha-id", "Alpha"),
        ("beta-id", "Beta"),
        ("weak-id", "Weak"),
        ("riemann-id", "Riemann integral"),
    ];
    let mut handles = Vec::new();
    for reader in 0..16 {
        let bridge = Arc::clone(&bridge);
        let (id, title) = asked[reader % asked.len()];
        handles.push(thread::spawn(move || -> Result<()> {
            for _ in 0..8 {
                let node = bridge
                    .node_from_id(&NodeFromIdParams { id: id.to_owned() })?
                    .with_context(|| format!("{id} should resolve by ID"))?;
                assert_eq!(node.title, title, "reader asked for {id}");
            }
            Ok(())
        }));
    }
    for handle in handles {
        handle.join().expect("reader thread should not panic")?;
    }

    Arc::into_inner(bridge)
        .context("all reader threads have finished, so the bridge is uniquely owned")?
        .shutdown()?;
    Ok(())
}
