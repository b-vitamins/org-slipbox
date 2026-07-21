use slipbox_core::{
    GlossaryDueParams, GlossaryDueResult, GlossaryTermParams, GlossaryTermResult,
    ListGlossaryTermsParams, ListGlossaryTermsResult, SearchGlossaryParams, SearchGlossaryResult,
};
use slipbox_rpc::JsonRpcError;

use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;

pub(crate) fn list_glossary_terms(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ListGlossaryTermsParams = parse_params(params)?;
    let terms = state
        .database
        .list_glossary_terms(params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to list glossary terms")))?;
    to_value(ListGlossaryTermsResult { terms })
}

pub(crate) fn search_glossary(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchGlossaryParams = parse_params(params)?;
    let terms = state
        .database
        .search_glossary(&params.query, params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to search glossary terms")))?;
    to_value(SearchGlossaryResult { terms })
}

pub(crate) fn glossary_due(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: GlossaryDueParams = parse_params(params)?;
    let today = crate::server::handlers::glossary_today(params.today.clone());
    let terms = state
        .database
        .glossary_due_terms(&today, params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to list due glossary terms")))?;
    to_value(GlossaryDueResult { terms })
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
