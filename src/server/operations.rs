use slipbox_rpc::{
    JsonRpcError, METHOD_AGENDA, METHOD_ANCHOR_AT_POINT, METHOD_ANCHOR_FROM_KEY,
    METHOD_APPEND_HEADING, METHOD_APPEND_HEADING_AT_OUTLINE_PATH, METHOD_APPEND_HEADING_TO_NODE,
    METHOD_BACKLINKS, METHOD_CAPTURE_NODE, METHOD_CAPTURE_TEMPLATE,
    METHOD_CAPTURE_TEMPLATE_PREVIEW, METHOD_COMPARE_NOTES, METHOD_CORPUS_AUDIT,
    METHOD_DELETE_EXPLORATION_ARTIFACT, METHOD_DELETE_REVIEW_RUN, METHOD_DELETE_WORKBENCH_PACK,
    METHOD_DEMOTE_ENTIRE_FILE, METHOD_DIAGNOSE_FILE, METHOD_DIAGNOSE_INDEX, METHOD_DIAGNOSE_NODE,
    METHOD_DIFF_REVIEW_RUNS, METHOD_ENSURE_FILE_NODE, METHOD_ENSURE_NODE_ID,
    METHOD_EXECUTE_EXPLORATION_ARTIFACT, METHOD_EXPLORATION_ARTIFACT, METHOD_EXPLORE,
    METHOD_EXPORT_WORKBENCH_PACK, METHOD_EXTRACT_SUBTREE, METHOD_FORWARD_LINKS,
    METHOD_GLOSSARY_DUE, METHOD_GLOSSARY_TERM, METHOD_GRADE_TERM, METHOD_GRAPH_DOT,
    METHOD_IMPORT_WORKBENCH_PACK, METHOD_INDEX, METHOD_INDEX_FILE, METHOD_INDEXED_FILES,
    METHOD_LIST_EXPLORATION_ARTIFACTS, METHOD_LIST_GLOSSARY_TERMS, METHOD_LIST_REVIEW_ROUTINES,
    METHOD_LIST_REVIEW_RUNS, METHOD_LIST_WORKBENCH_PACKS, METHOD_LIST_WORKFLOWS,
    METHOD_MARK_GLOSSARY_TERM, METHOD_MARK_REVIEW_FINDING, METHOD_NODE_AT_POINT,
    METHOD_NODE_FROM_ID, METHOD_NODE_FROM_KEY, METHOD_NODE_FROM_REF,
    METHOD_NODE_FROM_TITLE_OR_ALIAS, METHOD_NOTE_CONTEXT, METHOD_PING, METHOD_PROMOTE_ENTIRE_FILE,
    METHOD_RANDOM_NODE, METHOD_READ_FILE_SOURCE, METHOD_READ_NODE_SOURCE, METHOD_REFILE_REGION,
    METHOD_REFILE_SUBTREE, METHOD_REFLINKS, METHOD_REVIEW_FINDING_REMEDIATION_APPLY,
    METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW, METHOD_REVIEW_ROUTINE, METHOD_REVIEW_RUN,
    METHOD_RUN_REVIEW_ROUTINE, METHOD_RUN_WORKFLOW, METHOD_SAVE_CORPUS_AUDIT_REVIEW,
    METHOD_SAVE_EXPLORATION_ARTIFACT, METHOD_SAVE_REVIEW_RUN, METHOD_SAVE_WORKFLOW_REVIEW,
    METHOD_SEARCH_FILES, METHOD_SEARCH_GLOSSARY, METHOD_SEARCH_NODE_CONTENT, METHOD_SEARCH_NODES,
    METHOD_SEARCH_OCCURRENCES, METHOD_SEARCH_REFS, METHOD_SEARCH_TAGS,
    METHOD_SLIPBOX_LINK_REWRITE_APPLY, METHOD_SLIPBOX_LINK_REWRITE_PREVIEW, METHOD_STATUS,
    METHOD_UNLINKED_REFERENCES, METHOD_UPDATE_NODE_METADATA, METHOD_VALIDATE_WORKBENCH_PACK,
    METHOD_WORKBENCH_PACK, METHOD_WORKFLOW,
};

use crate::server::handlers::{query, write};
use crate::server::state::ServerState;

// Operation classification lives in `slipbox-rpc`; this module pairs each
// classified method with its handler. These re-exports keep the
// `crate::server::operations::{..}` paths `service.rs` uses.
pub use slipbox_rpc::{
    FreshnessBehavior, OperationDescriptor, OperationFamily, OperationMutation,
    operation_descriptor_by_method, operation_descriptors,
};

type OperationHandler =
    fn(&mut ServerState, serde_json::Value) -> Result<serde_json::Value, JsonRpcError>;

#[derive(Clone, Copy)]
pub struct Operation {
    method: &'static str,
    handler: OperationHandler,
}

impl Operation {
    pub(crate) fn invoke(
        self,
        state: &mut ServerState,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        (self.handler)(state, params)
    }
}

macro_rules! operation {
    ($method:expr, $handler:path) => {
        Operation {
            method: $method,
            handler: $handler,
        }
    };
}

/// Method-to-handler table for the JSON-RPC surface, ordered to mirror the
/// `slipbox-rpc` descriptor inventory that `handler_table_matches_rpc_classification`
/// pins it against.
const OPERATIONS: &[Operation] = &[
    operation!(METHOD_PING, ping),
    operation!(METHOD_STATUS, status),
    operation!(METHOD_INDEX, index),
    operation!(METHOD_INDEX_FILE, query::index_file),
    operation!(METHOD_INDEXED_FILES, indexed_files),
    operation!(METHOD_SEARCH_FILES, search_files),
    operation!(METHOD_DIAGNOSE_FILE, diagnose_file),
    operation!(METHOD_DIAGNOSE_NODE, query::diagnose_node),
    operation!(METHOD_DIAGNOSE_INDEX, diagnose_index),
    operation!(METHOD_SEARCH_NODES, query::search_nodes),
    operation!(METHOD_SEARCH_NODE_CONTENT, query::search_node_content),
    operation!(METHOD_RANDOM_NODE, query::random_node),
    operation!(METHOD_NODE_FROM_ID, query::node_from_id),
    operation!(METHOD_NODE_FROM_KEY, query::node_from_key),
    operation!(
        METHOD_NODE_FROM_TITLE_OR_ALIAS,
        query::node_from_title_or_alias
    ),
    operation!(METHOD_NODE_AT_POINT, query::node_at_point),
    operation!(METHOD_ANCHOR_AT_POINT, query::anchor_at_point),
    operation!(METHOD_ANCHOR_FROM_KEY, query::anchor_from_key),
    operation!(METHOD_READ_FILE_SOURCE, query::read_file_source),
    operation!(METHOD_READ_NODE_SOURCE, query::read_node_source),
    operation!(METHOD_NOTE_CONTEXT, query::note_context),
    operation!(METHOD_SEARCH_OCCURRENCES, search_occurrences),
    operation!(METHOD_SEARCH_TAGS, query::search_tags),
    operation!(METHOD_SEARCH_REFS, query::search_refs),
    operation!(METHOD_NODE_FROM_REF, query::node_from_ref),
    operation!(METHOD_BACKLINKS, query::backlinks),
    operation!(METHOD_FORWARD_LINKS, query::forward_links),
    operation!(METHOD_REFLINKS, query::reflinks),
    operation!(METHOD_UNLINKED_REFERENCES, query::unlinked_references),
    operation!(METHOD_AGENDA, query::agenda),
    operation!(METHOD_GRAPH_DOT, query::graph_dot),
    operation!(METHOD_EXPLORE, query::explore),
    operation!(METHOD_COMPARE_NOTES, query::compare_notes),
    operation!(
        METHOD_SAVE_EXPLORATION_ARTIFACT,
        query::save_exploration_artifact
    ),
    operation!(METHOD_EXPLORATION_ARTIFACT, query::exploration_artifact),
    operation!(
        METHOD_LIST_EXPLORATION_ARTIFACTS,
        query::list_exploration_artifacts
    ),
    operation!(
        METHOD_DELETE_EXPLORATION_ARTIFACT,
        query::delete_exploration_artifact
    ),
    operation!(
        METHOD_EXECUTE_EXPLORATION_ARTIFACT,
        query::execute_exploration_artifact
    ),
    operation!(METHOD_CORPUS_AUDIT, query::corpus_audit),
    operation!(METHOD_SAVE_REVIEW_RUN, query::save_review_run),
    operation!(METHOD_REVIEW_RUN, query::review_run),
    operation!(METHOD_DIFF_REVIEW_RUNS, query::diff_review_runs),
    operation!(
        METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW,
        query::review_finding_remediation_preview
    ),
    operation!(
        METHOD_REVIEW_FINDING_REMEDIATION_APPLY,
        query::review_finding_remediation_apply
    ),
    operation!(METHOD_LIST_REVIEW_RUNS, query::list_review_runs),
    operation!(METHOD_DELETE_REVIEW_RUN, query::delete_review_run),
    operation!(METHOD_MARK_REVIEW_FINDING, query::mark_review_finding),
    operation!(
        METHOD_SAVE_CORPUS_AUDIT_REVIEW,
        query::save_corpus_audit_review
    ),
    operation!(METHOD_LIST_WORKFLOWS, query::list_workflows),
    operation!(METHOD_WORKFLOW, query::workflow),
    operation!(METHOD_RUN_WORKFLOW, query::run_workflow),
    operation!(METHOD_SAVE_WORKFLOW_REVIEW, query::save_workflow_review),
    operation!(METHOD_LIST_REVIEW_ROUTINES, query::list_review_routines),
    operation!(METHOD_REVIEW_ROUTINE, query::review_routine),
    operation!(METHOD_RUN_REVIEW_ROUTINE, query::run_review_routine),
    operation!(METHOD_IMPORT_WORKBENCH_PACK, query::import_workbench_pack),
    operation!(METHOD_WORKBENCH_PACK, query::workbench_pack),
    operation!(
        METHOD_VALIDATE_WORKBENCH_PACK,
        query::validate_workbench_pack
    ),
    operation!(METHOD_EXPORT_WORKBENCH_PACK, query::export_workbench_pack),
    operation!(METHOD_LIST_WORKBENCH_PACKS, query::list_workbench_packs),
    operation!(METHOD_DELETE_WORKBENCH_PACK, query::delete_workbench_pack),
    operation!(METHOD_LIST_GLOSSARY_TERMS, query::list_glossary_terms),
    operation!(METHOD_SEARCH_GLOSSARY, query::search_glossary),
    operation!(METHOD_GLOSSARY_DUE, query::glossary_due),
    operation!(METHOD_GLOSSARY_TERM, query::glossary_term),
    operation!(METHOD_GRADE_TERM, write::grade_term),
    operation!(METHOD_MARK_GLOSSARY_TERM, write::mark_glossary_term),
    operation!(METHOD_CAPTURE_NODE, write::capture_node),
    operation!(METHOD_CAPTURE_TEMPLATE, write::capture_template),
    operation!(
        METHOD_CAPTURE_TEMPLATE_PREVIEW,
        write::capture_template_preview
    ),
    operation!(METHOD_ENSURE_FILE_NODE, write::ensure_file_node),
    operation!(METHOD_APPEND_HEADING, write::append_heading),
    operation!(METHOD_APPEND_HEADING_TO_NODE, write::append_heading_to_node),
    operation!(
        METHOD_APPEND_HEADING_AT_OUTLINE_PATH,
        write::append_heading_at_outline_path
    ),
    operation!(METHOD_ENSURE_NODE_ID, write::ensure_node_id),
    operation!(METHOD_UPDATE_NODE_METADATA, write::update_node_metadata),
    operation!(METHOD_REFILE_SUBTREE, write::refile_subtree),
    operation!(METHOD_REFILE_REGION, write::refile_region),
    operation!(METHOD_EXTRACT_SUBTREE, write::extract_subtree),
    operation!(METHOD_PROMOTE_ENTIRE_FILE, write::promote_entire_file),
    operation!(METHOD_DEMOTE_ENTIRE_FILE, write::demote_entire_file),
    operation!(
        METHOD_SLIPBOX_LINK_REWRITE_PREVIEW,
        write::slipbox_link_rewrite_preview
    ),
    operation!(
        METHOD_SLIPBOX_LINK_REWRITE_APPLY,
        write::slipbox_link_rewrite_apply
    ),
];

pub(crate) fn operation_by_method(method: &str) -> Option<Operation> {
    OPERATIONS
        .iter()
        .copied()
        .find(|operation| operation.method == method)
}

fn ping(
    state: &mut ServerState,
    _params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    query::ping(state)
}

fn status(
    state: &mut ServerState,
    _params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    query::status(state)
}

fn index(
    state: &mut ServerState,
    _params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    query::index(state)
}

fn indexed_files(
    state: &mut ServerState,
    _params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    query::indexed_files(state)
}

fn diagnose_index(
    state: &mut ServerState,
    _params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    query::diagnose_index(state)
}

fn search_files(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    query::search_files(state, params)
}

fn diagnose_file(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    query::diagnose_file(state, params)
}

fn search_occurrences(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    query::search_occurrences(state, params)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use slipbox_rpc::operation_descriptors;

    use super::{OPERATIONS, operation_by_method};

    #[test]
    fn operation_methods_are_unique_and_lookupable() {
        let mut methods = HashSet::new();
        for operation in OPERATIONS {
            assert!(
                methods.insert(operation.method),
                "duplicate operation method {}",
                operation.method
            );
            assert!(
                operation_by_method(operation.method).is_some(),
                "missing lookup for {}",
                operation.method
            );
        }
    }

    #[test]
    fn handler_table_matches_rpc_classification() {
        let handlers: Vec<&str> = OPERATIONS
            .iter()
            .map(|operation| operation.method)
            .collect();
        let classified: Vec<&str> = operation_descriptors()
            .map(|descriptor| descriptor.method)
            .collect();

        assert_eq!(
            handlers, classified,
            "handler table drifted from the slipbox-rpc classification inventory"
        );
    }
}
