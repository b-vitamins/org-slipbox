use std::fs;
use std::path::PathBuf;

use serde_json::json;
use slipbox_core::{
    IndexStats, NodeFromIdParams, NodeRecord, SearchNodesParams, SearchNodesResult, SourceId,
    SourceScope, SourceScopedKey,
};
use slipbox_engine::DiscoveryPolicy;
use slipbox_engine::service::SlipboxService;
use slipbox_rpc::{METHOD_INDEX, METHOD_NODE_FROM_ID, METHOD_SEARCH_NODES};
use slipbox_sources::{CatalogRevision, SourceCatalog, SourceCatalogStore};
use tempfile::{TempDir, tempdir};

mod support;

use support::{fixture_id, public_source};

// Both corpora hold this file name and this explicit Org ID, so the engine
// mints the same node key in each and only the source scope separates them.
const FILE_NAME: &str = "notes.org";
const SHARED_ID: &str = "shared-note";

const ALPHA: &str = "#+title: Alpha corpus\n\n* Alpha heading\n:PROPERTIES:\n:ID: shared-note\n:END:\nAlpha body.\n";
const BETA: &str =
    "#+title: Beta corpus\n\n* Beta heading\n:PROPERTIES:\n:ID: shared-note\n:END:\nBeta body.\n";

const ALPHA_SEED: u8 = 0x31;
const BETA_SEED: u8 = 0x32;

struct Corpus {
    _workspace: TempDir,
    database: PathBuf,
    service: SlipboxService,
}

impl Corpus {
    fn open(contents: &str) -> Self {
        let workspace = tempdir().expect("a temporary corpus directory");
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).expect("a corpus root is creatable");
        fs::write(root.join(FILE_NAME), contents).expect("a synthetic note is writable");
        let database = workspace.path().join("slipbox.sqlite");
        let mut service = SlipboxService::new(
            root,
            database.clone(),
            Vec::new(),
            DiscoveryPolicy::default(),
        )
        .expect("a canonical service opens over its own database");
        let stats: IndexStats = service
            .invoke(METHOD_INDEX, json!({}))
            .expect("a synthetic corpus indexes");
        assert_eq!(stats.files_indexed, 1);
        assert_eq!(stats.nodes_indexed, 2);
        Self {
            _workspace: workspace,
            database,
            service,
        }
    }

    fn shared_node(&mut self) -> NodeRecord {
        let node: Option<NodeRecord> = self
            .service
            .invoke(
                METHOD_NODE_FROM_ID,
                NodeFromIdParams {
                    id: SHARED_ID.to_owned(),
                },
            )
            .expect("a node lookup answers");
        node.expect("the shared identity resolves in this corpus")
    }

    fn titles(&mut self, query: &str) -> Vec<String> {
        let found: SearchNodesResult = self
            .service
            .invoke(
                METHOD_SEARCH_NODES,
                SearchNodesParams {
                    query: query.to_owned(),
                    limit: 20,
                    sort: None,
                },
            )
            .expect("a search answers");
        let mut titles: Vec<String> = found.nodes.into_iter().map(|node| node.title).collect();
        titles.sort();
        titles
    }
}

fn two_corpora() -> Vec<(SourceId, Corpus)> {
    vec![
        (fixture_id(ALPHA_SEED), Corpus::open(ALPHA)),
        (fixture_id(BETA_SEED), Corpus::open(BETA)),
    ]
}

fn corpus_of<'a>(corpora: &'a mut [(SourceId, Corpus)], source: &SourceId) -> &'a mut Corpus {
    let (_, corpus) = corpora
        .iter_mut()
        .find(|(id, _)| id == source)
        .expect("the selected source has a corpus");
    corpus
}

fn two_source_catalog() -> SourceCatalog {
    let mut catalog = SourceCatalog::new();
    catalog
        .add(public_source(
            ALPHA_SEED,
            "Alpha",
            "https://git.example.org/owner/alpha.git",
            "",
        ))
        .expect("the first source is new");
    catalog
        .add(public_source(
            BETA_SEED,
            "Beta",
            "https://git.example.org/owner/beta.git",
            "",
        ))
        .expect("the second source is new");
    catalog
}

#[test]
fn two_corpora_collide_on_every_key_the_engine_mints() {
    let mut corpora = two_corpora();
    let alpha = corpus_of(&mut corpora, &fixture_id(ALPHA_SEED)).shared_node();
    let beta = corpus_of(&mut corpora, &fixture_id(BETA_SEED)).shared_node();

    assert_eq!(alpha.explicit_id.as_deref(), Some(SHARED_ID));
    assert_eq!(beta.explicit_id.as_deref(), Some(SHARED_ID));
    assert_eq!(
        alpha.node_key, beta.node_key,
        "the corpora no longer collide, so the source seam is not what separates them"
    );
    assert_eq!(alpha.file_path, beta.file_path);
    assert_ne!(alpha.title, beta.title);
    assert_ne!(
        corpora[0].1.database, corpora[1].1.database,
        "the corpora share a database"
    );
}

#[test]
fn a_colliding_key_is_distinct_once_it_is_source_bound() {
    let mut corpora = two_corpora();
    let alpha_id = fixture_id(ALPHA_SEED);
    let beta_id = fixture_id(BETA_SEED);
    let node = corpus_of(&mut corpora, &alpha_id).shared_node();

    let alpha_note =
        SourceScopedKey::note(&alpha_id, &node.node_key).expect("a node key is a usable key");
    let beta_note =
        SourceScopedKey::note(&beta_id, &node.node_key).expect("a node key is a usable key");
    assert_ne!(alpha_note, beta_note);
    assert_ne!(alpha_note.storage_key(), beta_note.storage_key());
    assert_eq!(
        alpha_note.key(),
        beta_note.key(),
        "a wrapped key was rewritten"
    );

    let alpha_file =
        SourceScopedKey::file(&alpha_id, &node.file_path).expect("a file path is a usable key");
    let beta_file =
        SourceScopedKey::file(&beta_id, &node.file_path).expect("a file path is a usable key");
    assert_ne!(alpha_file, beta_file);

    let alpha_reading = SourceScopedKey::reading_reference(&alpha_id, &node.node_key)
        .expect("a node key is a usable reading reference");
    let beta_reading = SourceScopedKey::reading_reference(&beta_id, &node.node_key)
        .expect("a node key is a usable reading reference");
    assert_ne!(alpha_reading, beta_reading);
    assert_ne!(
        alpha_reading, alpha_note,
        "one source's reading reference and note key collapsed"
    );

    for (key, source, scope) in [
        (&alpha_note, &alpha_id, SourceScope::Note),
        (&beta_note, &beta_id, SourceScope::Note),
        (&alpha_file, &alpha_id, SourceScope::File),
        (&alpha_reading, &alpha_id, SourceScope::ReadingReference),
    ] {
        let parsed = SourceScopedKey::parse_storage_key(&key.storage_key())
            .expect("a storage key round-trips");
        assert_eq!(&parsed, key);
        assert_eq!(parsed.source(), source);
        assert_eq!(parsed.scope(), scope);
    }
}

#[test]
fn switching_the_selection_changes_the_answer_and_re_keys_nothing() {
    let storage = tempdir().expect("a temporary private storage directory");
    let store = SourceCatalogStore::at(storage.path().join("source-catalog.json"));
    let mut catalog = two_source_catalog();
    let alpha_id = fixture_id(ALPHA_SEED);
    let beta_id = fixture_id(BETA_SEED);
    let revision = store
        .save(CatalogRevision::ABSENT, &catalog)
        .expect("a first write succeeds");

    let mut corpora = two_corpora();
    let before: Vec<String> = corpora
        .iter()
        .map(|(id, _)| {
            SourceScopedKey::note(id, "heading:notes.org:1")
                .expect("a node key is a usable key")
                .storage_key()
        })
        .collect();

    let selected = catalog
        .active_id()
        .expect("the first source added is the first selection")
        .clone();
    assert_eq!(selected, alpha_id);
    assert_eq!(
        corpus_of(&mut corpora, &selected).shared_node().title,
        "Alpha heading"
    );
    assert_eq!(
        corpus_of(&mut corpora, &selected).titles("Alpha"),
        vec!["Alpha corpus", "Alpha heading"]
    );
    assert!(
        corpus_of(&mut corpora, &beta_id).titles("Alpha").is_empty(),
        "the unselected corpus answered for the selected one"
    );

    catalog
        .set_active(&beta_id)
        .expect("the second source is configured");
    store
        .save(revision, &catalog)
        .expect("a switch is writable");
    let reopened = store
        .load()
        .expect("the document is readable")
        .expect("the document exists")
        .into_catalog();
    let selected = reopened
        .active_id()
        .expect("the reopened catalog kept its selection")
        .clone();
    assert_eq!(selected, beta_id);
    assert_eq!(
        corpus_of(&mut corpora, &selected).shared_node().title,
        "Beta heading"
    );
    assert_eq!(
        corpus_of(&mut corpora, &selected).titles("Beta"),
        vec!["Beta corpus", "Beta heading"]
    );

    assert_eq!(
        corpus_of(&mut corpora, &alpha_id).shared_node().title,
        "Alpha heading"
    );
    let after: Vec<String> = corpora
        .iter()
        .map(|(id, _)| {
            SourceScopedKey::note(id, "heading:notes.org:1")
                .expect("a node key is a usable key")
                .storage_key()
        })
        .collect();
    assert_eq!(before, after);
    assert_eq!(
        reopened.sources().len(),
        2,
        "a switch changed what the catalog holds"
    );
}
