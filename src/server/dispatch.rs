use slipbox_rpc::{
    JsonRpcError, JsonRpcErrorObject, JsonRpcRequest, JsonRpcResponse, METHOD_AGENDA,
    METHOD_ANCHOR_AT_POINT, METHOD_APPEND_HEADING, METHOD_APPEND_HEADING_AT_OUTLINE_PATH,
    METHOD_APPEND_HEADING_TO_NODE, METHOD_BACKLINKS, METHOD_CAPTURE_NODE, METHOD_CAPTURE_TEMPLATE,
    METHOD_CAPTURE_TEMPLATE_PREVIEW, METHOD_COMPARE_NOTES, METHOD_CORPUS_AUDIT,
    METHOD_DELETE_EXPLORATION_ARTIFACT, METHOD_DELETE_REVIEW_RUN, METHOD_DEMOTE_ENTIRE_FILE,
    METHOD_DIFF_REVIEW_RUNS, METHOD_ENSURE_FILE_NODE, METHOD_ENSURE_NODE_ID,
    METHOD_EXECUTE_EXPLORATION_ARTIFACT, METHOD_EXPLORATION_ARTIFACT, METHOD_EXPLORE,
    METHOD_EXTRACT_SUBTREE, METHOD_FORWARD_LINKS, METHOD_GRAPH_DOT, METHOD_INDEX,
    METHOD_INDEX_FILE, METHOD_INDEXED_FILES, METHOD_LIST_EXPLORATION_ARTIFACTS,
    METHOD_LIST_REVIEW_RUNS, METHOD_LIST_WORKFLOWS, METHOD_MARK_REVIEW_FINDING,
    METHOD_NODE_AT_POINT, METHOD_NODE_FROM_ID, METHOD_NODE_FROM_KEY, METHOD_NODE_FROM_REF,
    METHOD_NODE_FROM_TITLE_OR_ALIAS, METHOD_PING, METHOD_PROMOTE_ENTIRE_FILE, METHOD_RANDOM_NODE,
    METHOD_REFILE_REGION, METHOD_REFILE_SUBTREE, METHOD_REFLINKS, METHOD_REVIEW_RUN,
    METHOD_RUN_WORKFLOW, METHOD_SAVE_CORPUS_AUDIT_REVIEW, METHOD_SAVE_EXPLORATION_ARTIFACT,
    METHOD_SAVE_REVIEW_RUN, METHOD_SAVE_WORKFLOW_REVIEW, METHOD_SEARCH_FILES, METHOD_SEARCH_NODES,
    METHOD_SEARCH_OCCURRENCES, METHOD_SEARCH_REFS, METHOD_SEARCH_TAGS, METHOD_STATUS,
    METHOD_UNLINKED_REFERENCES, METHOD_UPDATE_NODE_METADATA, METHOD_WORKFLOW,
};

use crate::server::handlers::{query, write};
use crate::server::state::ServerState;

pub(super) fn handle_request(state: &mut ServerState, request: JsonRpcRequest) -> JsonRpcResponse {
    let JsonRpcRequest { id, method, .. } = request;
    let id = id.unwrap_or(serde_json::Value::Null);

    let response = dispatch_request(state, &method, request.params);

    match response {
        Ok(result) => JsonRpcResponse::success(id, result),
        Err(error) => JsonRpcResponse::error(id, error.into_inner()),
    }
}

fn dispatch_request(
    state: &mut ServerState,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    match method {
        METHOD_PING => query::ping(state),
        METHOD_STATUS => query::status(state),
        METHOD_INDEX => query::index(state),
        METHOD_GRAPH_DOT => query::graph_dot(state, params),
        METHOD_INDEXED_FILES => query::indexed_files(state),
        METHOD_SEARCH_FILES => query::search_files(state, params),
        METHOD_SEARCH_OCCURRENCES => query::search_occurrences(state, params),
        METHOD_SEARCH_NODES => query::search_nodes(state, params),
        METHOD_RANDOM_NODE => query::random_node(state, params),
        METHOD_SEARCH_TAGS => query::search_tags(state, params),
        METHOD_NODE_FROM_ID => query::node_from_id(state, params),
        METHOD_NODE_FROM_KEY => query::node_from_key(state, params),
        METHOD_NODE_FROM_TITLE_OR_ALIAS => query::node_from_title_or_alias(state, params),
        METHOD_NODE_AT_POINT => query::node_at_point(state, params),
        METHOD_ANCHOR_AT_POINT => query::anchor_at_point(state, params),
        METHOD_BACKLINKS => query::backlinks(state, params),
        METHOD_FORWARD_LINKS => query::forward_links(state, params),
        METHOD_REFLINKS => query::reflinks(state, params),
        METHOD_UNLINKED_REFERENCES => query::unlinked_references(state, params),
        METHOD_EXPLORE => query::explore(state, params),
        METHOD_SEARCH_REFS => query::search_refs(state, params),
        METHOD_NODE_FROM_REF => query::node_from_ref(state, params),
        METHOD_AGENDA => query::agenda(state, params),
        METHOD_COMPARE_NOTES => query::compare_notes(state, params),
        METHOD_CORPUS_AUDIT => query::corpus_audit(state, params),
        METHOD_SAVE_EXPLORATION_ARTIFACT => query::save_exploration_artifact(state, params),
        METHOD_EXPLORATION_ARTIFACT => query::exploration_artifact(state, params),
        METHOD_LIST_EXPLORATION_ARTIFACTS => query::list_exploration_artifacts(state, params),
        METHOD_DELETE_EXPLORATION_ARTIFACT => query::delete_exploration_artifact(state, params),
        METHOD_EXECUTE_EXPLORATION_ARTIFACT => query::execute_exploration_artifact(state, params),
        METHOD_SAVE_REVIEW_RUN => query::save_review_run(state, params),
        METHOD_REVIEW_RUN => query::review_run(state, params),
        METHOD_DIFF_REVIEW_RUNS => query::diff_review_runs(state, params),
        METHOD_LIST_REVIEW_RUNS => query::list_review_runs(state, params),
        METHOD_DELETE_REVIEW_RUN => query::delete_review_run(state, params),
        METHOD_MARK_REVIEW_FINDING => query::mark_review_finding(state, params),
        METHOD_SAVE_CORPUS_AUDIT_REVIEW => query::save_corpus_audit_review(state, params),
        METHOD_SAVE_WORKFLOW_REVIEW => query::save_workflow_review(state, params),
        METHOD_LIST_WORKFLOWS => query::list_workflows(state, params),
        METHOD_WORKFLOW => query::workflow(state, params),
        METHOD_RUN_WORKFLOW => query::run_workflow(state, params),
        METHOD_CAPTURE_NODE => write::capture_node(state, params),
        METHOD_CAPTURE_TEMPLATE => write::capture_template(state, params),
        METHOD_CAPTURE_TEMPLATE_PREVIEW => write::capture_template_preview(state, params),
        METHOD_ENSURE_FILE_NODE => write::ensure_file_node(state, params),
        METHOD_APPEND_HEADING => write::append_heading(state, params),
        METHOD_APPEND_HEADING_TO_NODE => write::append_heading_to_node(state, params),
        METHOD_APPEND_HEADING_AT_OUTLINE_PATH => {
            write::append_heading_at_outline_path(state, params)
        }
        METHOD_ENSURE_NODE_ID => write::ensure_node_id(state, params),
        METHOD_UPDATE_NODE_METADATA => write::update_node_metadata(state, params),
        METHOD_REFILE_SUBTREE => write::refile_subtree(state, params),
        METHOD_REFILE_REGION => write::refile_region(state, params),
        METHOD_EXTRACT_SUBTREE => write::extract_subtree(state, params),
        METHOD_PROMOTE_ENTIRE_FILE => write::promote_entire_file(state, params),
        METHOD_DEMOTE_ENTIRE_FILE => write::demote_entire_file(state, params),
        METHOD_INDEX_FILE => query::index_file(state, params),
        _ => Err(JsonRpcError::new(JsonRpcErrorObject::method_not_found(
            format!("unsupported method: {method}"),
        ))),
    }
}
