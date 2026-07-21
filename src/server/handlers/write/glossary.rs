use slipbox_core::{
    GradeTermParams, GradeTermResult, MarkGlossaryTermParams, MarkGlossaryTermResult, SrState,
    sm2_schedule,
};
use slipbox_rpc::JsonRpcError;

use crate::server::handlers::glossary_today;
use crate::server::rpc::{internal_error, not_found, parse_params, to_value};
use crate::server::state::ServerState;

pub(crate) fn grade_term(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: GradeTermParams = parse_params(params)?;
    let term = known_glossary_term(state, &params.node_key)?;

    let today = glossary_today(params.today.clone());
    let scheduled = sm2_schedule(&SrState::from_record(&term), params.quality, &today);
    let updated_path = slipbox_write::set_glossary_schedule(&state.root, &term, &scheduled)
        .map_err(|error| internal_error(error.context("failed to reschedule glossary term")))?;

    let term = state.sync_path_and_read_node(&updated_path, &params.node_key, "graded term")?;
    to_value(GradeTermResult { term })
}

pub(crate) fn mark_glossary_term(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: MarkGlossaryTermParams = parse_params(params)?;
    let node = state.known_note(&params.node_key, "note")?;

    let mut updated_path = slipbox_write::mark_glossary_term(&state.root, &node)
        .map_err(|error| internal_error(error.context("failed to mark glossary term")))?;
    if let Some(status) = params.status {
        updated_path = slipbox_write::set_glossary_status(&state.root, &node, status)
            .map_err(|error| internal_error(error.context("failed to set glossary status")))?;
    }

    let term = state.sync_path_and_read_node(&updated_path, &params.node_key, "marked term")?;
    to_value(MarkGlossaryTermResult { term })
}

/// Resolve a node key to an existing glossary term.
///
/// Grading only makes sense for a marked term, so a plain note is rejected with
/// `NotFound` rather than silently writing an `SR_*` drawer the index would drop.
fn known_glossary_term(
    state: &mut ServerState,
    node_key: &str,
) -> Result<slipbox_core::NodeRecord, JsonRpcError> {
    let term = state.known_note(node_key, "glossary term")?;
    if term.glossary {
        Ok(term)
    } else {
        Err(not_found(format!("unknown glossary term: {node_key}")))
    }
}
