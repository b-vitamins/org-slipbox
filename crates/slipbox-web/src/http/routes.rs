use serde::Serialize;
use serde_json::{Value, json};
use slipbox_core::{
    BacklinksParams, ExplorationLens, ExploreParams, ForwardLinksParams, GlossaryDueParams,
    GlossaryTermParams, ListGlossaryTermsParams, MIN_SEARCH_TERM_CHARACTERS, NodeFromIdParams,
    NodeFromKeyParams, NodeFromTitleOrAliasParams, NoteContextParams, ReflinksParams,
    SearchGlossaryParams, SearchNodeContentParams, SearchNodesParams, SearchNodesSort,
    UnlinkedReferencesParams,
};

use crate::ReadingBridge;
use crate::http::query::Query;
use crate::http::response::ApiError;

/// The already-serialized body of a resolved route.
pub(crate) struct ApiResponse {
    pub(crate) body: String,
}

impl ApiResponse {
    /// Serialize a result value into a response body.
    fn json<T: Serialize>(value: &T) -> Result<Self, ApiError> {
        serde_json::to_string(value)
            .map(|body| Self { body })
            .map_err(|error| ApiError::internal(format!("failed to serialize response: {error}")))
    }
}

/// Dispatch one already-split request to the matching read route. `path` is the
/// URL path with its query removed; `raw_query` is the portion after `?`. Only
/// `/api/` paths reach here, so an unknown route is an API not-found.
pub(crate) fn dispatch(
    bridge: &ReadingBridge,
    path: &str,
    raw_query: &str,
) -> Result<ApiResponse, ApiError> {
    let query = Query::parse(raw_query)?;
    match path {
        "/api/healthz" => healthz(bridge),
        "/api/status" => status(bridge),
        "/api/node" => node(bridge, &query),
        "/api/note/context" => note_context(bridge, &query),
        "/api/search/nodes" => search_nodes(bridge, &query),
        "/api/search/content" => search_content(bridge, &query),
        "/api/random" => random(bridge),
        "/api/backlinks" => backlinks(bridge, &query),
        "/api/forward-links" => forward_links(bridge, &query),
        "/api/reflinks" => reflinks(bridge, &query),
        "/api/unlinked-references" => unlinked_references(bridge, &query),
        "/api/explore" => explore(bridge, &query),
        "/api/glossary/terms" => glossary_terms(bridge, &query),
        "/api/glossary/search" => glossary_search(bridge, &query),
        "/api/glossary/term" => glossary_term(bridge, &query),
        "/api/glossary/due" => glossary_due(bridge, &query),
        other => Err(ApiError::not_found(format!(
            "no reading route for `{other}`"
        ))),
    }
}

/// Liveness: confirm the daemon answers, and echo the served root.
fn healthz(bridge: &ReadingBridge) -> Result<ApiResponse, ApiError> {
    let ping = bridge.ping()?;
    ApiResponse::json(&json!({ "status": "ok", "root": ping.root, "version": ping.version }))
}

/// The served root, database path, and derived-index counts.
fn status(bridge: &ReadingBridge) -> Result<ApiResponse, ApiError> {
    ApiResponse::json(&bridge.status()?)
}

/// Resolve a single note by `id`, `key`, or `title`, in that order of
/// precedence. A selector that matches nothing is a 404.
fn node(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let resolved = if let Some(id) = query.optional("id")? {
        bridge.node_from_id(&NodeFromIdParams { id })?
    } else if let Some(node_key) = query.optional("key")? {
        bridge.node_from_key(&NodeFromKeyParams { node_key })?
    } else if let Some(title_or_alias) = query.optional("title")? {
        let nocase = query.optional_bool("nocase")?.unwrap_or(false);
        bridge.node_from_title_or_alias(&NodeFromTitleOrAliasParams {
            title_or_alias,
            nocase,
        })?
    } else {
        return Err(ApiError::bad_request(
            "one of `id`, `key`, or `title` is required",
        ));
    };
    let node = resolved.ok_or_else(|| ApiError::not_found("no note matched the given selector"))?;
    ApiResponse::json(&node)
}

/// A note's source slice together with its place in the filing order and its
/// immediate backlinks and forward links.
fn note_context(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    // Each shaping parameter stays `None` when absent, so the RPC layer's own
    // documented defaults apply rather than a second set restated here.
    let params = NoteContextParams {
        node_key: query.require("key")?,
        source_context_before: query.optional_bounded("before", 0, MAX_CONTEXT_LINES)?,
        source_context_after: query.optional_bounded("after", 0, MAX_CONTEXT_LINES)?,
        source_max_lines: query.optional_bounded("max_lines", 1, MAX_SOURCE_LINES)?,
        relation_limit: query.optional_bounded("relations", 1, MAX_LIMIT)?,
    };
    ApiResponse::json(&bridge.note_context(&params)?)
}

/// Full-text search over indexed notes.
fn search_nodes(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = SearchNodesParams {
        query: search_term(query)?,
        limit: query.bounded("limit", DEFAULT_LIMIT, 1, MAX_LIMIT)?,
        sort: parse_sort(query)?,
    };
    ApiResponse::json(&bridge.search_nodes(&params)?)
}

/// Ranked note-content search, with a highlighted excerpt per hit.
fn search_content(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = SearchNodeContentParams {
        query: search_term(query)?,
        limit: query.bounded("limit", DEFAULT_LIMIT, 1, MAX_LIMIT)?,
    };
    ApiResponse::json(&bridge.search_node_content(&params)?)
}

/// One random indexed note.
fn random(bridge: &ReadingBridge) -> Result<ApiResponse, ApiError> {
    ApiResponse::json(&bridge.random_node()?)
}

/// Incoming links to a note.
fn backlinks(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = BacklinksParams {
        node_key: query.require("key")?,
        limit: query.bounded("limit", DEFAULT_RELATION_LIMIT, 1, MAX_RELATION_LIMIT)?,
        unique: query.optional_bool("unique")?.unwrap_or(false),
    };
    ApiResponse::json(&bridge.backlinks(&params)?)
}

/// Outgoing links from a note.
fn forward_links(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = ForwardLinksParams {
        node_key: query.require("key")?,
        limit: query.bounded("limit", DEFAULT_RELATION_LIMIT, 1, MAX_RELATION_LIMIT)?,
        unique: query.optional_bool("unique")?.unwrap_or(false),
    };
    ApiResponse::json(&bridge.forward_links(&params)?)
}

/// Links to the bibliographic references a note carries.
fn reflinks(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = ReflinksParams {
        node_key: query.require("key")?,
        limit: query.bounded("limit", DEFAULT_RELATION_LIMIT, 1, MAX_RELATION_LIMIT)?,
    };
    ApiResponse::json(&bridge.reflinks(&params)?)
}

/// Unlinked mention candidates for a note's references.
fn unlinked_references(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = UnlinkedReferencesParams {
        node_key: query.require("key")?,
        limit: query.bounded("limit", DEFAULT_RELATION_LIMIT, 1, MAX_RELATION_LIMIT)?,
    };
    ApiResponse::json(&bridge.unlinked_references(&params)?)
}

/// One note read through one exploration lens, which decides the sections the
/// answer carries. A section the lens defines is served empty, not omitted.
fn explore(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = ExploreParams {
        node_key: query.require("key")?,
        lens: require_lens(query)?,
        limit: query.bounded("limit", DEFAULT_RELATION_LIMIT, 1, MAX_RELATION_LIMIT)?,
        // Defined for the structure lens alone, and unused here.
        unique: false,
    };
    ApiResponse::json(&bridge.explore(&params)?)
}

/// The glossary as a dictionary listing.
fn glossary_terms(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = ListGlossaryTermsParams {
        limit: query.bounded("limit", DEFAULT_LIMIT, 1, MAX_LIMIT)?,
    };
    ApiResponse::json(&bridge.list_glossary_terms(&params)?)
}

/// Search over glossary terms.
fn glossary_search(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = SearchGlossaryParams {
        query: search_term(query)?,
        limit: query.bounded("limit", DEFAULT_LIMIT, 1, MAX_LIMIT)?,
    };
    ApiResponse::json(&bridge.search_glossary(&params)?)
}

/// One glossary term's definition by slipbox key.
fn glossary_term(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = GlossaryTermParams {
        node_key: query.require("key")?,
    };
    let result = bridge.glossary_term(&params)?;
    if result.term.is_none() {
        return Err(ApiError::not_found(
            "no glossary term found for the given key",
        ));
    }
    ApiResponse::json(&result)
}

/// Glossary terms due for review as of a reference day.
fn glossary_due(bridge: &ReadingBridge, query: &Query) -> Result<ApiResponse, ApiError> {
    let params = GlossaryDueParams {
        // The due predicate compares ISO date strings, so a value that is not a
        // calendar date would not fail downstream: it would answer with the
        // wrong set of terms.
        today: query.optional_date("today")?,
        limit: query.bounded("limit", DEFAULT_LIMIT, 1, MAX_LIMIT)?,
    };
    ApiResponse::json(&bridge.glossary_due(&params)?)
}

/// Read the `q` search term, requiring at least one word the index can match.
///
/// The index builds its match expression from words of
/// [`MIN_SEARCH_TERM_CHARACTERS`] or more, measured in characters after trimming
/// surrounding punctuation. A query with none yields no expression, and the store
/// then answers with an unranked listing of whatever notes the filter admits.
fn search_term(query: &Query) -> Result<String, ApiError> {
    let raw = query.require("q")?;
    let matchable = raw.split_whitespace().any(|word| {
        word.trim_matches(|character: char| !character.is_alphanumeric())
            .chars()
            .count()
            >= MIN_SEARCH_TERM_CHARACTERS
    });
    if matchable {
        Ok(raw)
    } else {
        Err(ApiError::bad_request(format!(
            "query parameter `q` needs a word of at least {MIN_SEARCH_TERM_CHARACTERS} characters to search on, got `{raw}`"
        )))
    }
}

/// Map the `sort` query parameter through the RPC layer's own serde
/// representation, so the kebab-case wire spelling has one declaration.
fn parse_sort(query: &Query) -> Result<Option<SearchNodesSort>, ApiError> {
    let Some(raw) = query.optional("sort")? else {
        return Ok(None);
    };
    serde_json::from_value::<SearchNodesSort>(Value::String(raw.clone()))
        .map(Some)
        .map_err(|_| ApiError::bad_request(format!("unknown sort `{raw}`")))
}

/// Map the required `lens` parameter the same way. An unknown one is refused with
/// the accepted set rather than read as a default.
fn require_lens(query: &Query) -> Result<ExplorationLens, ApiError> {
    let raw = query.require("lens")?;
    serde_json::from_value::<ExplorationLens>(Value::String(raw.clone())).map_err(|_| {
        ApiError::bad_request(format!(
            "query parameter `lens` must be one of {}, got `{raw}`",
            accepted_lenses()
        ))
    })
}

/// The spellings a refusal lists, drawn from serde rather than restated.
fn accepted_lenses() -> String {
    [
        ExplorationLens::Structure,
        ExplorationLens::Refs,
        ExplorationLens::Time,
        ExplorationLens::Tasks,
        ExplorationLens::Bridges,
        ExplorationLens::Dormant,
        ExplorationLens::Unresolved,
    ]
    .iter()
    .map(|lens| {
        serde_json::to_value(lens)
            .expect("a fieldless enum serializes")
            .as_str()
            .expect("a kebab-case variant serializes to a string")
            .to_owned()
    })
    .collect::<Vec<_>>()
    .join(", ")
}

// The RPC layer's own clamps are crate-private to `slipbox-core`, so the HTTP
// contract states its bounds here and refuses a request outside them.

/// Mirrors the RPC layer's `default_search_limit`.
const DEFAULT_LIMIT: usize = 50;
/// Largest page a search or glossary listing will serve.
const MAX_LIMIT: usize = 200;
/// Mirrors the RPC layer's `default_backlink_limit`.
const DEFAULT_RELATION_LIMIT: usize = 200;
/// Largest relation page a link route will serve.
const MAX_RELATION_LIMIT: usize = 1_000;
/// Longest source slice a note context will serve.
const MAX_SOURCE_LINES: usize = 1_000;
/// Most surrounding lines a note context will add on either side.
const MAX_CONTEXT_LINES: u32 = 200;
// The shortest matchable word is absent from this list on purpose: it is the
// index's own floor, so `search_term` measures against the one declaration in
// `slipbox-core` rather than a second number here.

#[cfg(test)]
mod tests {
    use slipbox_core::{ExplorationLens, SearchNodesSort};

    use super::{
        MAX_RELATION_LIMIT, MIN_SEARCH_TERM_CHARACTERS, accepted_lenses, parse_sort, require_lens,
        search_term,
    };
    use crate::http::query::Query;

    fn term(raw: &str) -> Result<String, u16> {
        search_term(&Query::parse(raw).expect("the raw query parses")).map_err(|error| error.status)
    }

    fn sort(raw: &str) -> Result<Option<SearchNodesSort>, u16> {
        parse_sort(&Query::parse(raw).expect("the raw query parses")).map_err(|error| error.status)
    }

    fn lens(raw: &str) -> Result<ExplorationLens, u16> {
        require_lens(&Query::parse(raw).expect("the raw query parses"))
            .map_err(|error| error.status)
    }

    #[test]
    fn a_word_the_index_can_match_is_searched_as_written() {
        assert_eq!(term("q=gibbs"), Ok("gibbs".to_owned()));
        assert_eq!(term("q=a+of+entropy"), Ok("a of entropy".to_owned()));
    }

    #[test]
    fn a_two_character_acronym_is_a_searchable_term() {
        for raw in ["q=KL", "q=EM", "q=ML"] {
            assert_eq!(term(raw), Ok(raw["q=".len()..].to_owned()), "{raw}");
        }
    }

    #[test]
    fn a_query_with_nothing_matchable_is_refused() {
        for raw in ["q=a", "q=%2D", "q=%2D%2D", "q=a+I+%2C"] {
            assert_eq!(term(raw), Err(400), "`{raw}` should not reach the index");
        }
    }

    #[test]
    fn punctuation_does_not_pad_a_word_to_length() {
        // The index trims non-alphanumerics before measuring.
        assert_eq!(term("q=%22a%22"), Err(400));
    }

    #[test]
    fn a_multibyte_word_is_measured_in_characters_like_any_other() {
        // Each word below is at least three bytes long but two, one, and two
        // characters, so counting bytes would admit the one-character word.
        assert_eq!(term("q=a%C3%B1"), Ok("añ".to_owned()));
        assert_eq!(term("q=%E7%8C%AB"), Err(400));
        assert_eq!(term("q=%E6%9D%B1%E4%BA%AC"), Ok("東京".to_owned()));
    }

    #[test]
    fn the_refusal_states_the_bound_it_applied() {
        // The reading client shows this number, so it is on the wire by contract.
        let refusal = search_term(&Query::parse("q=a").expect("the raw query parses"))
            .expect_err("a one-character query is refused");
        assert_eq!(refusal.status, 400);
        let body = refusal.body();
        assert!(
            body.contains(&format!("at least {MIN_SEARCH_TERM_CHARACTERS} characters")),
            "{body}"
        );
    }

    #[test]
    fn every_sort_order_is_spelled_the_way_the_rpc_layer_spells_it() {
        // The kebab-case spelling is the wire contract a client writes into a URL.
        for (raw, expected) in [
            ("sort=relevance", SearchNodesSort::Relevance),
            ("sort=title", SearchNodesSort::Title),
            ("sort=file", SearchNodesSort::File),
            ("sort=file-mtime", SearchNodesSort::FileMtime),
            ("sort=backlink-count", SearchNodesSort::BacklinkCount),
            ("sort=forward-link-count", SearchNodesSort::ForwardLinkCount),
        ] {
            assert_eq!(sort(raw), Ok(Some(expected)), "{raw}");
        }
    }

    #[test]
    fn an_absent_sort_leaves_the_order_to_the_index() {
        assert_eq!(sort(""), Ok(None));
        assert_eq!(sort("sort=sideways"), Err(400));
        assert_eq!(sort("sort=Title"), Err(400));
    }

    #[test]
    fn every_lens_is_spelled_the_way_the_rpc_layer_spells_it() {
        for (raw, expected) in [
            ("lens=structure", ExplorationLens::Structure),
            ("lens=refs", ExplorationLens::Refs),
            ("lens=time", ExplorationLens::Time),
            ("lens=tasks", ExplorationLens::Tasks),
            ("lens=bridges", ExplorationLens::Bridges),
            ("lens=dormant", ExplorationLens::Dormant),
            ("lens=unresolved", ExplorationLens::Unresolved),
        ] {
            assert_eq!(lens(raw), Ok(expected), "{raw}");
        }
    }

    #[test]
    fn an_absent_or_unknown_lens_is_refused_rather_than_defaulted() {
        for raw in ["", "lens=", "lens=sideways", "lens=Structure"] {
            assert_eq!(lens(raw), Err(400), "`{raw}` should not reach the index");
        }
    }

    #[test]
    fn the_lens_refusal_names_every_spelling_it_would_have_taken() {
        let refusal = require_lens(&Query::parse("lens=sideways").expect("the raw query parses"))
            .expect_err("an unknown lens is refused");
        assert_eq!(refusal.status, 400);
        let body = refusal.body();
        for spelling in [
            "structure",
            "refs",
            "time",
            "tasks",
            "bridges",
            "dormant",
            "unresolved",
        ] {
            assert!(body.contains(spelling), "{spelling} missing from {body}");
        }
    }

    #[test]
    fn the_accepted_lenses_are_read_off_serde_not_restated() {
        // A renamed variant changes the message with it.
        assert_eq!(
            accepted_lenses(),
            "structure, refs, time, tasks, bridges, dormant, unresolved"
        );
    }

    #[test]
    fn the_explore_limit_admits_the_whole_range_the_operation_accepts() {
        // `ExploreParams::normalized_limit` clamps to `1..=1_000`.
        assert_eq!(MAX_RELATION_LIMIT, 1_000);
    }
}
