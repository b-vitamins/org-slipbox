use slipbox_core::{
    GlossaryDueParams, GlossaryDueResult, GlossaryTermParams, GlossaryTermResult,
    ListGlossaryTermsParams, ListGlossaryTermsResult, SearchGlossaryParams, SearchGlossaryResult,
};
use slipbox_rpc::JsonRpcError;
use slipbox_store::GlossaryPosition;

use crate::server::rpc::{internal_error, invalid_params, parse_params, to_value};
use crate::server::state::ServerState;

pub(crate) fn list_glossary_terms(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ListGlossaryTermsParams = parse_params(params)?;
    let after = position(params.normalized_after(), GlossaryPosition::parse_term)?;
    let page = state
        .database
        .list_glossary_terms(params.normalized_limit(), after.as_ref())
        .map_err(|error| internal_error(error.context("failed to list glossary terms")))?;
    to_value(ListGlossaryTermsResult {
        terms: page.terms,
        total: page.total,
        has_more: page.has_more,
        next_position: page.next_position,
    })
}

pub(crate) fn search_glossary(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchGlossaryParams = parse_params(params)?;
    let page = state
        .database
        .search_glossary(&params.query, params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to search glossary terms")))?;
    to_value(SearchGlossaryResult {
        terms: page.terms,
        total: page.total,
        has_more: page.has_more,
    })
}

pub(crate) fn glossary_due(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: GlossaryDueParams = parse_params(params)?;
    let today = crate::server::handlers::glossary_today(params.today.clone());
    let after = position(params.normalized_after(), GlossaryPosition::parse_due)?;
    let page = state
        .database
        .glossary_due_terms(
            &today,
            params.normalized_query(),
            params.normalized_limit(),
            after.as_ref(),
        )
        .map_err(|error| internal_error(error.context("failed to list due glossary terms")))?;
    to_value(GlossaryDueResult {
        terms: page.terms,
        total: page.total,
        has_more: page.has_more,
        next_position: page.next_position,
    })
}

pub(crate) fn glossary_term(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: GlossaryTermParams = parse_params(params)?;
    let term = state
        .database
        .note_by_key(&params.node_key)
        .map_err(|error| internal_error(error.context("failed to fetch glossary term")))?
        // Only a marked term resolves through this method; a plain note answers None.
        .filter(|record| record.glossary);
    to_value(GlossaryTermResult { term })
}

/// Read a listing position through the index's own reader, refusing a token this
/// listing did not mint rather than answering a different page.
fn position(
    after: Option<&str>,
    read: impl Fn(&str) -> Option<GlossaryPosition>,
) -> Result<Option<GlossaryPosition>, JsonRpcError> {
    match after {
        None => Ok(None),
        Some(token) => read(token).map(Some).ok_or_else(|| {
            invalid_params(format!(
                "`after` is not a position in this listing: {token}"
            ))
        }),
    }
}
