use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde_json::json;
use slipbox_core::{
    IndexFileParams, IndexFileResult, IndexStats, IndexedFilesResult, SearchGlossaryParams,
    SearchGlossaryResult, SearchNodeContentParams, SearchNodeContentResult, SearchNodesParams,
    SearchNodesResult, SearchNodesSort,
};
use slipbox_engine::DiscoveryPolicy;
use slipbox_engine::service::SlipboxService;
use slipbox_rpc::{
    METHOD_INDEX, METHOD_INDEX_FILE, METHOD_INDEXED_FILES, METHOD_SEARCH_GLOSSARY,
    METHOD_SEARCH_NODE_CONTENT, METHOD_SEARCH_NODES,
};
use tempfile::{TempDir, tempdir};

const ARCHITECTURE: &str = "#+title: The transformer architecture and its attention blocks\n\nA sequence model assembled from stacked attention blocks.\n";
const LINEAR: &str = "#+title: Linear transformation\n\nA transformation transforms a vector space, one transformation composes with another transformation, and a transformation of a transformation transforms again.\n";
const HEADWORD: &str =
    "#+title: Transformer block\n#+glossary: t\n\nA layer of attention and a feed-forward map.\n";
const STEM_HEADWORD: &str = "#+title: Transformations\n#+glossary: t\n#+filetags: :transformation:transforms:\n:PROPERTIES:\n:ROAM_ALIASES: \"transformation transforms\"\n:END:\n\nRepeated in the metadata rather than the prose.\n";
const ARCHITECTURE_WITHOUT_THE_TERM: &str = "#+title: The attention architecture\n\nA sequence model assembled from stacked attention blocks that it transforms.\n";
const LINEAR_WITH_A_NAMING_ALIAS: &str = "#+title: Linear maps\n:PROPERTIES:\n:ROAM_ALIASES: \"Linear transformer maps\"\n:END:\n\nA transformation transforms a vector space, and one transformation composes with another transformation.\n";

const LITERAL: [&str; 2] = [
    "The transformer architecture and its attention blocks",
    "Transformer block",
];
const STEMMED: [&str; 2] = ["Linear transformation", "Transformations"];

struct Corpus {
    _workspace: TempDir,
    root: PathBuf,
    service: SlipboxService,
}

impl Corpus {
    fn open() -> Result<Self> {
        let workspace = tempdir()?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root)?;
        for (name, contents) in [
            ("architecture.org", ARCHITECTURE),
            ("linear.org", LINEAR),
            ("block.org", HEADWORD),
            ("transformations.org", STEM_HEADWORD),
        ] {
            fs::write(root.join(name), contents)?;
        }
        let mut service = SlipboxService::new(
            root.clone(),
            workspace.path().join("slipbox.sqlite"),
            Vec::new(),
            DiscoveryPolicy::default(),
        )?;
        let stats: IndexStats = service.invoke(METHOD_INDEX, json!({}))?;
        assert_eq!(stats.files_indexed, 4);
        Ok(Self {
            _workspace: workspace,
            root,
            service,
        })
    }

    fn write(&self, name: &str, contents: &str) -> Result<()> {
        fs::write(self.root.join(name), contents)?;
        Ok(())
    }

    fn index_file(&mut self, name: &str) -> Result<String> {
        let refreshed: IndexFileResult = self.service.invoke(
            METHOD_INDEX_FILE,
            IndexFileParams {
                file_path: name.to_owned(),
            },
        )?;
        Ok(refreshed.file_path)
    }

    fn indexed_files(&mut self) -> Result<Vec<String>> {
        let indexed: IndexedFilesResult = self.service.invoke(METHOD_INDEXED_FILES, json!({}))?;
        Ok(indexed.files)
    }

    fn content(&mut self, query: &str, limit: usize) -> Result<Vec<String>> {
        let found: SearchNodeContentResult = self.service.invoke(
            METHOD_SEARCH_NODE_CONTENT,
            SearchNodeContentParams {
                query: query.to_owned(),
                limit,
            },
        )?;
        Ok(found
            .hits
            .into_iter()
            .map(|hit| hit.node.title)
            .collect::<Vec<_>>())
    }

    fn nodes(&mut self, query: &str, sort: Option<SearchNodesSort>) -> Result<Vec<String>> {
        let found: SearchNodesResult = self.service.invoke(
            METHOD_SEARCH_NODES,
            SearchNodesParams {
                query: query.to_owned(),
                limit: 20,
                sort,
            },
        )?;
        Ok(found
            .nodes
            .into_iter()
            .map(|node| node.title)
            .collect::<Vec<_>>())
    }

    fn glossary(&mut self, query: &str) -> Result<SearchGlossaryResult> {
        self.service.invoke(
            METHOD_SEARCH_GLOSSARY,
            SearchGlossaryParams {
                query: query.to_owned(),
                limit: 20,
            },
        )
    }
}

fn sorted(titles: &[String]) -> Vec<&str> {
    let mut sorted = titles.iter().map(String::as_str).collect::<Vec<_>>();
    sorted.sort_unstable();
    sorted
}

#[test]
fn content_search_serves_literal_titles_before_shared_stems() -> Result<()> {
    let mut corpus = Corpus::open()?;

    let ranked = corpus.content("transformer", 20)?;
    assert_eq!(
        ranked.len(),
        4,
        "every stem match keeps its recall: {ranked:?}"
    );
    assert_eq!(
        sorted(&ranked[..2]),
        LITERAL.to_vec(),
        "the notes naming the whole term hold the head of the page: {ranked:?}"
    );
    assert_eq!(
        sorted(&ranked[2..]),
        STEMMED.to_vec(),
        "the stem-only matches follow them: {ranked:?}"
    );

    let single = corpus.content("transformer", 1)?;
    assert!(
        LITERAL.contains(&single[0].as_str()),
        "the limit applies after the ranking, not before it: {single:?}"
    );
    Ok(())
}

#[test]
fn glossary_terms_answer_both_a_content_search_and_a_glossary_search() -> Result<()> {
    let mut corpus = Corpus::open()?;

    let content = corpus.content("Transformer block", 20)?;
    assert_eq!(
        content.first().map(String::as_str),
        Some("Transformer block"),
        "a glossary term is an ordinary content-search candidate: {content:?}"
    );

    let glossary = corpus.glossary("transformer")?;
    assert_eq!(
        glossary
            .terms
            .iter()
            .map(|term| term.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Transformer block", "Transformations"],
        "the headword naming the whole term leads the glossary page"
    );
    assert_eq!(glossary.total, 2);
    assert!(!glossary.has_more);
    assert!(glossary.terms.iter().all(|term| term.glossary));
    Ok(())
}

#[test]
fn node_search_ranks_by_relevance_and_obeys_an_explicit_sort() -> Result<()> {
    let mut corpus = Corpus::open()?;

    let relevance = corpus.nodes("transformer", None)?;
    assert_eq!(
        sorted(&relevance[..2]),
        LITERAL.to_vec(),
        "metadata relevance prefers the titles spelling the term out: {relevance:?}"
    );
    assert_eq!(sorted(&relevance[2..]), STEMMED.to_vec());

    assert_eq!(
        corpus.nodes("transformer", Some(SearchNodesSort::Title))?,
        vec![
            "Linear transformation",
            "The transformer architecture and its attention blocks",
            "Transformations",
            "Transformer block",
        ],
        "an explicit sort states the order and the ranking evidence stays out of it"
    );
    assert_eq!(
        corpus.nodes("transformer", Some(SearchNodesSort::File))?,
        vec![
            "The transformer architecture and its attention blocks",
            "Transformer block",
            "Linear transformation",
            "Transformations",
        ]
    );
    Ok(())
}

#[test]
fn a_body_only_word_stays_out_of_metadata_search() -> Result<()> {
    let mut corpus = Corpus::open()?;

    assert_eq!(
        corpus.content("feed-forward", 20)?,
        vec!["Transformer block"],
        "content search reaches prose"
    );
    assert!(
        corpus.nodes("feed-forward", None)?.is_empty(),
        "metadata search still sees metadata only"
    );
    Ok(())
}

#[test]
fn one_file_refreshed_alone_moves_the_ranking_every_client_sees() -> Result<()> {
    let mut corpus = Corpus::open()?;
    assert_eq!(
        sorted(&corpus.content("transformer", 20)?[..2]),
        LITERAL.to_vec()
    );

    corpus.write("architecture.org", ARCHITECTURE_WITHOUT_THE_TERM)?;
    assert_eq!(corpus.index_file("architecture.org")?, "architecture.org");

    let ranked = corpus.content("transformer", 20)?;
    assert_eq!(
        ranked.first().map(String::as_str),
        Some("Transformer block"),
        "the title that still names the term leads: {ranked:?}"
    );
    assert!(
        !ranked.iter().any(|title| title == LITERAL[0]),
        "the edited title is gone from the page: {ranked:?}"
    );
    assert!(
        ranked.contains(&"The attention architecture".to_owned()),
        "the refreshed file reads back as it was written: {ranked:?}"
    );
    assert!(
        ranked.contains(&"Linear transformation".to_owned()),
        "a file the refresh did not name keeps its rows: {ranked:?}"
    );

    corpus.write("ghost.org", "#+title: Ghost transformer\n")?;
    corpus.write("linear.org", LINEAR_WITH_A_NAMING_ALIAS)?;
    assert_eq!(corpus.index_file("linear.org")?, "linear.org");

    let ranked = corpus.content("transformer", 20)?;
    assert_eq!(
        sorted(&ranked[..2]),
        vec!["Linear maps", "Transformer block"],
        "an alias added to one file ranks as soon as that file is refreshed: {ranked:?}"
    );
    assert_eq!(
        corpus.indexed_files()?,
        vec![
            "architecture.org",
            "block.org",
            "linear.org",
            "transformations.org"
        ],
        "refreshing one file imports no other, however discoverable"
    );
    assert!(
        corpus.content("Ghost", 20)?.is_empty(),
        "a file that was never indexed stays out of the results"
    );

    fs::remove_file(corpus.root.join("block.org"))?;
    assert_eq!(corpus.index_file("block.org")?, "block.org");

    let ranked = corpus.content("transformer", 20)?;
    assert!(
        !ranked.iter().any(|title| title == "Transformer block"),
        "the deleted file's evidence does not linger: {ranked:?}"
    );
    assert_eq!(
        corpus
            .glossary("transformer")?
            .terms
            .iter()
            .map(|term| term.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Transformations"],
        "the glossary page follows the same removal"
    );
    assert_eq!(
        corpus.indexed_files()?,
        vec!["architecture.org", "linear.org", "transformations.org"]
    );
    Ok(())
}
