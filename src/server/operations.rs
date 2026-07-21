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
    METHOD_SEARCH_FILES, METHOD_SEARCH_GLOSSARY, METHOD_SEARCH_NODES, METHOD_SEARCH_OCCURRENCES,
    METHOD_SEARCH_REFS, METHOD_SEARCH_TAGS, METHOD_SLIPBOX_LINK_REWRITE_APPLY,
    METHOD_SLIPBOX_LINK_REWRITE_PREVIEW, METHOD_STATUS, METHOD_UNLINKED_REFERENCES,
    METHOD_UPDATE_NODE_METADATA, METHOD_VALIDATE_WORKBENCH_PACK, METHOD_WORKBENCH_PACK,
    METHOD_WORKFLOW,
};

use crate::server::handlers::{query, write};
use crate::server::state::ServerState;

type OperationHandler =
    fn(&mut ServerState, serde_json::Value) -> Result<serde_json::Value, JsonRpcError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationFamily {
    System,
    Diagnostics,
    Files,
    Notes,
    Relations,
    Exploration,
    Reviews,
    Assets,
    Glossary,
    Capture,
    StructuralEdit,
    LinkRewrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMutation {
    ReadOnly,
    DerivedIndex,
    DurableState,
    OrgContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessBehavior {
    ReportsState,
    ReadsDerivedIndex,
    ReadsSourceFiles,
    RefreshesRootIndex,
    RefreshesOneFile,
    RefreshesAffectedFiles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationDescriptor {
    pub method: &'static str,
    pub family: OperationFamily,
    pub mutation: OperationMutation,
    pub freshness: FreshnessBehavior,
    pub description: &'static str,
}

#[derive(Clone, Copy)]
pub struct Operation {
    pub descriptor: OperationDescriptor,
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
    ($method:expr, $family:ident, $mutation:ident, $freshness:ident, $description:expr, $handler:path) => {
        Operation {
            descriptor: OperationDescriptor {
                method: $method,
                family: OperationFamily::$family,
                mutation: OperationMutation::$mutation,
                freshness: FreshnessBehavior::$freshness,
                description: $description,
            },
            handler: $handler,
        }
    };
}

const OPERATIONS: &[Operation] = &[
    operation!(
        METHOD_PING,
        System,
        ReadOnly,
        ReportsState,
        "Check daemon liveness and root identity.",
        ping
    ),
    operation!(
        METHOD_STATUS,
        System,
        ReadOnly,
        ReportsState,
        "Return daemon root, database path, and index counts.",
        status
    ),
    operation!(
        METHOD_INDEX,
        System,
        DerivedIndex,
        RefreshesRootIndex,
        "Refresh the derived index from root discovery.",
        index
    ),
    operation!(
        METHOD_INDEX_FILE,
        Files,
        DerivedIndex,
        RefreshesOneFile,
        "Refresh one file in the derived index without pruning unrelated files.",
        query::index_file
    ),
    operation!(
        METHOD_INDEXED_FILES,
        Files,
        ReadOnly,
        ReadsDerivedIndex,
        "List files currently represented in the derived index.",
        indexed_files
    ),
    operation!(
        METHOD_SEARCH_FILES,
        Files,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed file records.",
        search_files
    ),
    operation!(
        METHOD_DIAGNOSE_FILE,
        Diagnostics,
        ReadOnly,
        ReportsState,
        "Explain discovery and index state for one file.",
        diagnose_file
    ),
    operation!(
        METHOD_DIAGNOSE_NODE,
        Diagnostics,
        ReadOnly,
        ReportsState,
        "Explain indexed source-location state for one node.",
        query::diagnose_node
    ),
    operation!(
        METHOD_DIAGNOSE_INDEX,
        Diagnostics,
        ReadOnly,
        ReportsState,
        "Report drift between root discovery and the derived index.",
        diagnose_index
    ),
    operation!(
        METHOD_SEARCH_NODES,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed note records.",
        query::search_nodes
    ),
    operation!(
        METHOD_RANDOM_NODE,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Return one random indexed note.",
        query::random_node
    ),
    operation!(
        METHOD_NODE_FROM_ID,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve a note by exact Org ID.",
        query::node_from_id
    ),
    operation!(
        METHOD_NODE_FROM_KEY,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve a note by exact slipbox key.",
        query::node_from_key
    ),
    operation!(
        METHOD_NODE_FROM_TITLE_OR_ALIAS,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve notes by exact title or alias.",
        query::node_from_title_or_alias
    ),
    operation!(
        METHOD_NODE_AT_POINT,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve the indexed note at a source position.",
        query::node_at_point
    ),
    operation!(
        METHOD_ANCHOR_AT_POINT,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve the indexed anchor at a source position.",
        query::anchor_at_point
    ),
    operation!(
        METHOD_ANCHOR_FROM_KEY,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve an indexed anchor by exact slipbox key.",
        query::anchor_from_key
    ),
    operation!(
        METHOD_READ_FILE_SOURCE,
        Files,
        ReadOnly,
        ReadsSourceFiles,
        "Read a bounded slice from an eligible source file.",
        query::read_file_source
    ),
    operation!(
        METHOD_READ_NODE_SOURCE,
        Notes,
        ReadOnly,
        ReadsSourceFiles,
        "Read a bounded slice for an indexed source anchor.",
        query::read_node_source
    ),
    operation!(
        METHOD_NOTE_CONTEXT,
        Notes,
        ReadOnly,
        ReadsSourceFiles,
        "Read a source slice and immediate relation context for an indexed note.",
        query::note_context
    ),
    operation!(
        METHOD_SEARCH_OCCURRENCES,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Search raw indexed text occurrences.",
        search_occurrences
    ),
    operation!(
        METHOD_SEARCH_TAGS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed Org tags.",
        query::search_tags
    ),
    operation!(
        METHOD_SEARCH_REFS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed reference strings.",
        query::search_refs
    ),
    operation!(
        METHOD_NODE_FROM_REF,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve the note owning an exact indexed reference.",
        query::node_from_ref
    ),
    operation!(
        METHOD_BACKLINKS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Return incoming links for an indexed note.",
        query::backlinks
    ),
    operation!(
        METHOD_FORWARD_LINKS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Return outgoing links for an indexed note.",
        query::forward_links
    ),
    operation!(
        METHOD_REFLINKS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Return links to indexed references.",
        query::reflinks
    ),
    operation!(
        METHOD_UNLINKED_REFERENCES,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Find mention candidates for indexed references.",
        query::unlinked_references
    ),
    operation!(
        METHOD_AGENDA,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Query indexed planning entries.",
        query::agenda
    ),
    operation!(
        METHOD_GRAPH_DOT,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Render the indexed relation graph as DOT.",
        query::graph_dot
    ),
    operation!(
        METHOD_EXPLORE,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "Run a live exploration lens.",
        query::explore
    ),
    operation!(
        METHOD_COMPARE_NOTES,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "Compare two indexed notes.",
        query::compare_notes
    ),
    operation!(
        METHOD_SAVE_EXPLORATION_ARTIFACT,
        Exploration,
        DurableState,
        ReadsDerivedIndex,
        "Persist an exploration artifact record.",
        query::save_exploration_artifact
    ),
    operation!(
        METHOD_EXPLORATION_ARTIFACT,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "Read one persisted exploration artifact.",
        query::exploration_artifact
    ),
    operation!(
        METHOD_LIST_EXPLORATION_ARTIFACTS,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "List persisted exploration artifacts.",
        query::list_exploration_artifacts
    ),
    operation!(
        METHOD_DELETE_EXPLORATION_ARTIFACT,
        Exploration,
        DurableState,
        ReadsDerivedIndex,
        "Delete one persisted exploration artifact.",
        query::delete_exploration_artifact
    ),
    operation!(
        METHOD_EXECUTE_EXPLORATION_ARTIFACT,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "Execute one persisted exploration artifact.",
        query::execute_exploration_artifact
    ),
    operation!(
        METHOD_CORPUS_AUDIT,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "Run a read-only corpus audit.",
        query::corpus_audit
    ),
    operation!(
        METHOD_SAVE_REVIEW_RUN,
        Reviews,
        DurableState,
        ReadsDerivedIndex,
        "Persist a review run.",
        query::save_review_run
    ),
    operation!(
        METHOD_REVIEW_RUN,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "Read one persisted review run.",
        query::review_run
    ),
    operation!(
        METHOD_DIFF_REVIEW_RUNS,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "Diff two persisted review runs.",
        query::diff_review_runs
    ),
    operation!(
        METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "Preview remediation for one review finding.",
        query::review_finding_remediation_preview
    ),
    operation!(
        METHOD_REVIEW_FINDING_REMEDIATION_APPLY,
        Reviews,
        OrgContent,
        RefreshesAffectedFiles,
        "Apply remediation for one review finding and refresh affected files.",
        query::review_finding_remediation_apply
    ),
    operation!(
        METHOD_LIST_REVIEW_RUNS,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "List persisted review runs.",
        query::list_review_runs
    ),
    operation!(
        METHOD_DELETE_REVIEW_RUN,
        Reviews,
        DurableState,
        ReadsDerivedIndex,
        "Delete one persisted review run.",
        query::delete_review_run
    ),
    operation!(
        METHOD_MARK_REVIEW_FINDING,
        Reviews,
        DurableState,
        ReadsDerivedIndex,
        "Mark the state of one persisted review finding.",
        query::mark_review_finding
    ),
    operation!(
        METHOD_SAVE_CORPUS_AUDIT_REVIEW,
        Reviews,
        DurableState,
        ReadsDerivedIndex,
        "Run an audit and persist its review record.",
        query::save_corpus_audit_review
    ),
    operation!(
        METHOD_LIST_WORKFLOWS,
        Assets,
        ReadOnly,
        ReportsState,
        "List discovered workflows.",
        query::list_workflows
    ),
    operation!(
        METHOD_WORKFLOW,
        Assets,
        ReadOnly,
        ReportsState,
        "Read one discovered workflow.",
        query::workflow
    ),
    operation!(
        METHOD_RUN_WORKFLOW,
        Assets,
        ReadOnly,
        ReadsDerivedIndex,
        "Run one workflow without persisting a review.",
        query::run_workflow
    ),
    operation!(
        METHOD_SAVE_WORKFLOW_REVIEW,
        Assets,
        DurableState,
        ReadsDerivedIndex,
        "Run one workflow and persist its review record.",
        query::save_workflow_review
    ),
    operation!(
        METHOD_LIST_REVIEW_ROUTINES,
        Assets,
        ReadOnly,
        ReportsState,
        "List discovered review routines.",
        query::list_review_routines
    ),
    operation!(
        METHOD_REVIEW_ROUTINE,
        Assets,
        ReadOnly,
        ReportsState,
        "Read one discovered review routine.",
        query::review_routine
    ),
    operation!(
        METHOD_RUN_REVIEW_ROUTINE,
        Assets,
        ReadOnly,
        ReadsDerivedIndex,
        "Run one review routine.",
        query::run_review_routine
    ),
    operation!(
        METHOD_IMPORT_WORKBENCH_PACK,
        Assets,
        DurableState,
        ReportsState,
        "Import a durable workbench pack.",
        query::import_workbench_pack
    ),
    operation!(
        METHOD_WORKBENCH_PACK,
        Assets,
        ReadOnly,
        ReportsState,
        "Read one durable workbench pack.",
        query::workbench_pack
    ),
    operation!(
        METHOD_VALIDATE_WORKBENCH_PACK,
        Assets,
        ReadOnly,
        ReportsState,
        "Validate a workbench pack payload.",
        query::validate_workbench_pack
    ),
    operation!(
        METHOD_EXPORT_WORKBENCH_PACK,
        Assets,
        ReadOnly,
        ReportsState,
        "Export one durable workbench pack.",
        query::export_workbench_pack
    ),
    operation!(
        METHOD_LIST_WORKBENCH_PACKS,
        Assets,
        ReadOnly,
        ReportsState,
        "List durable workbench packs.",
        query::list_workbench_packs
    ),
    operation!(
        METHOD_DELETE_WORKBENCH_PACK,
        Assets,
        DurableState,
        ReportsState,
        "Delete one durable workbench pack.",
        query::delete_workbench_pack
    ),
    operation!(
        METHOD_LIST_GLOSSARY_TERMS,
        Glossary,
        ReadOnly,
        ReadsDerivedIndex,
        "List indexed glossary terms.",
        query::list_glossary_terms
    ),
    operation!(
        METHOD_SEARCH_GLOSSARY,
        Glossary,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed glossary terms.",
        query::search_glossary
    ),
    operation!(
        METHOD_GLOSSARY_DUE,
        Glossary,
        ReadOnly,
        ReadsDerivedIndex,
        "List glossary terms due for review.",
        query::glossary_due
    ),
    operation!(
        METHOD_GLOSSARY_TERM,
        Glossary,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve one glossary term by slipbox key.",
        query::glossary_term
    ),
    operation!(
        METHOD_GRADE_TERM,
        Glossary,
        OrgContent,
        RefreshesAffectedFiles,
        "Grade a glossary term, rewrite its review drawer, and refresh it.",
        write::grade_term
    ),
    operation!(
        METHOD_MARK_GLOSSARY_TERM,
        Glossary,
        OrgContent,
        RefreshesAffectedFiles,
        "Mark a note as a glossary term and refresh it.",
        write::mark_glossary_term
    ),
    operation!(
        METHOD_CAPTURE_NODE,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Create a file note and refresh its index record.",
        write::capture_node
    ),
    operation!(
        METHOD_CAPTURE_TEMPLATE,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Capture from a template and refresh the affected index record.",
        write::capture_template
    ),
    operation!(
        METHOD_CAPTURE_TEMPLATE_PREVIEW,
        Capture,
        ReadOnly,
        ReportsState,
        "Preview a template capture without writing files.",
        write::capture_template_preview
    ),
    operation!(
        METHOD_ENSURE_FILE_NODE,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Ensure a file-level note exists and refresh it.",
        write::ensure_file_node
    ),
    operation!(
        METHOD_APPEND_HEADING,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Append a heading to a file and refresh it.",
        write::append_heading
    ),
    operation!(
        METHOD_APPEND_HEADING_TO_NODE,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Append a heading below a resolved note and refresh it.",
        write::append_heading_to_node
    ),
    operation!(
        METHOD_APPEND_HEADING_AT_OUTLINE_PATH,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Append a heading at an outline path and refresh it.",
        write::append_heading_at_outline_path
    ),
    operation!(
        METHOD_ENSURE_NODE_ID,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Ensure an anchor has an Org ID and refresh it.",
        write::ensure_node_id
    ),
    operation!(
        METHOD_UPDATE_NODE_METADATA,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Update note metadata and refresh the affected file.",
        write::update_node_metadata
    ),
    operation!(
        METHOD_REFILE_SUBTREE,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Move a subtree to another target and refresh affected files.",
        write::refile_subtree
    ),
    operation!(
        METHOD_REFILE_REGION,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Refile a source region and refresh affected files.",
        write::refile_region
    ),
    operation!(
        METHOD_EXTRACT_SUBTREE,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Extract a subtree into a target file and refresh affected files.",
        write::extract_subtree
    ),
    operation!(
        METHOD_PROMOTE_ENTIRE_FILE,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Promote every heading in a file and refresh it.",
        write::promote_entire_file
    ),
    operation!(
        METHOD_DEMOTE_ENTIRE_FILE,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Demote every heading in a file and refresh it.",
        write::demote_entire_file
    ),
    operation!(
        METHOD_SLIPBOX_LINK_REWRITE_PREVIEW,
        LinkRewrite,
        ReadOnly,
        ReadsDerivedIndex,
        "Preview a slipbox link rewrite without writing files.",
        write::slipbox_link_rewrite_preview
    ),
    operation!(
        METHOD_SLIPBOX_LINK_REWRITE_APPLY,
        LinkRewrite,
        OrgContent,
        RefreshesAffectedFiles,
        "Apply a slipbox link rewrite and refresh affected files.",
        write::slipbox_link_rewrite_apply
    ),
];

pub fn operation_descriptors() -> impl ExactSizeIterator<Item = OperationDescriptor> {
    OPERATIONS.iter().map(|operation| operation.descriptor)
}

pub fn operation_descriptor_by_method(method: &str) -> Option<OperationDescriptor> {
    operation_by_method(method).map(|operation| operation.descriptor)
}

pub(crate) fn operation_by_method(method: &str) -> Option<Operation> {
    OPERATIONS
        .iter()
        .copied()
        .find(|operation| operation.descriptor.method == method)
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

    use super::{
        FreshnessBehavior, OPERATIONS, OperationFamily, OperationMutation, operation_by_method,
    };

    #[test]
    fn operation_methods_are_unique_and_lookupable() {
        let mut methods = HashSet::new();
        for operation in OPERATIONS {
            assert!(
                methods.insert(operation.descriptor.method),
                "duplicate operation method {}",
                operation.descriptor.method
            );
            assert!(
                operation_by_method(operation.descriptor.method).is_some(),
                "missing lookup for {}",
                operation.descriptor.method
            );
        }
    }

    #[test]
    fn operation_descriptors_are_explicit_compatibility_inventory() {
        use FreshnessBehavior::*;
        use OperationFamily::*;
        use OperationMutation::*;

        let expected: &[(&str, OperationFamily, OperationMutation, FreshnessBehavior)] = &[
            ("slipbox/ping", System, ReadOnly, ReportsState),
            ("slipbox/status", System, ReadOnly, ReportsState),
            ("slipbox/index", System, DerivedIndex, RefreshesRootIndex),
            ("slipbox/indexFile", Files, DerivedIndex, RefreshesOneFile),
            ("slipbox/indexedFiles", Files, ReadOnly, ReadsDerivedIndex),
            ("slipbox/searchFiles", Files, ReadOnly, ReadsDerivedIndex),
            ("slipbox/diagnoseFile", Diagnostics, ReadOnly, ReportsState),
            ("slipbox/diagnoseNode", Diagnostics, ReadOnly, ReportsState),
            ("slipbox/diagnoseIndex", Diagnostics, ReadOnly, ReportsState),
            ("slipbox/searchNodes", Notes, ReadOnly, ReadsDerivedIndex),
            ("slipbox/randomNode", Notes, ReadOnly, ReadsDerivedIndex),
            ("slipbox/nodeFromId", Notes, ReadOnly, ReadsDerivedIndex),
            ("slipbox/nodeFromKey", Notes, ReadOnly, ReadsDerivedIndex),
            (
                "slipbox/nodeFromTitleOrAlias",
                Notes,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            ("slipbox/nodeAtPoint", Notes, ReadOnly, ReadsDerivedIndex),
            ("slipbox/anchorAtPoint", Notes, ReadOnly, ReadsDerivedIndex),
            ("slipbox/anchorFromKey", Notes, ReadOnly, ReadsDerivedIndex),
            ("slipbox/readFileSource", Files, ReadOnly, ReadsSourceFiles),
            ("slipbox/readNodeSource", Notes, ReadOnly, ReadsSourceFiles),
            ("slipbox/noteContext", Notes, ReadOnly, ReadsSourceFiles),
            (
                "slipbox/searchOccurrences",
                Relations,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            ("slipbox/searchTags", Relations, ReadOnly, ReadsDerivedIndex),
            ("slipbox/searchRefs", Relations, ReadOnly, ReadsDerivedIndex),
            (
                "slipbox/nodeFromRef",
                Relations,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            ("slipbox/backlinks", Relations, ReadOnly, ReadsDerivedIndex),
            (
                "slipbox/forwardLinks",
                Relations,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            ("slipbox/reflinks", Relations, ReadOnly, ReadsDerivedIndex),
            (
                "slipbox/unlinkedReferences",
                Relations,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            ("slipbox/agenda", Relations, ReadOnly, ReadsDerivedIndex),
            ("slipbox/graphDot", Relations, ReadOnly, ReadsDerivedIndex),
            ("slipbox/explore", Exploration, ReadOnly, ReadsDerivedIndex),
            (
                "slipbox/compareNotes",
                Exploration,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/saveExplorationArtifact",
                Exploration,
                DurableState,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/explorationArtifact",
                Exploration,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/listExplorationArtifacts",
                Exploration,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/deleteExplorationArtifact",
                Exploration,
                DurableState,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/executeExplorationArtifact",
                Exploration,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            ("slipbox/corpusAudit", Reviews, ReadOnly, ReadsDerivedIndex),
            (
                "slipbox/saveReviewRun",
                Reviews,
                DurableState,
                ReadsDerivedIndex,
            ),
            ("slipbox/reviewRun", Reviews, ReadOnly, ReadsDerivedIndex),
            (
                "slipbox/diffReviewRuns",
                Reviews,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/reviewFindingRemediationPreview",
                Reviews,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/reviewFindingRemediationApply",
                Reviews,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/listReviewRuns",
                Reviews,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/deleteReviewRun",
                Reviews,
                DurableState,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/markReviewFinding",
                Reviews,
                DurableState,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/saveCorpusAuditReview",
                Reviews,
                DurableState,
                ReadsDerivedIndex,
            ),
            ("slipbox/listWorkflows", Assets, ReadOnly, ReportsState),
            ("slipbox/workflow", Assets, ReadOnly, ReportsState),
            ("slipbox/runWorkflow", Assets, ReadOnly, ReadsDerivedIndex),
            (
                "slipbox/saveWorkflowReview",
                Assets,
                DurableState,
                ReadsDerivedIndex,
            ),
            ("slipbox/listReviewRoutines", Assets, ReadOnly, ReportsState),
            ("slipbox/reviewRoutine", Assets, ReadOnly, ReportsState),
            (
                "slipbox/runReviewRoutine",
                Assets,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/importWorkbenchPack",
                Assets,
                DurableState,
                ReportsState,
            ),
            ("slipbox/workbenchPack", Assets, ReadOnly, ReportsState),
            (
                "slipbox/validateWorkbenchPack",
                Assets,
                ReadOnly,
                ReportsState,
            ),
            (
                "slipbox/exportWorkbenchPack",
                Assets,
                ReadOnly,
                ReportsState,
            ),
            ("slipbox/listWorkbenchPacks", Assets, ReadOnly, ReportsState),
            (
                "slipbox/deleteWorkbenchPack",
                Assets,
                DurableState,
                ReportsState,
            ),
            (
                "slipbox/listGlossaryTerms",
                Glossary,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/searchGlossary",
                Glossary,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            ("slipbox/glossaryDue", Glossary, ReadOnly, ReadsDerivedIndex),
            (
                "slipbox/glossaryTerm",
                Glossary,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/gradeTerm",
                Glossary,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/markGlossaryTerm",
                Glossary,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/captureNode",
                Capture,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/captureTemplate",
                Capture,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/captureTemplatePreview",
                Capture,
                ReadOnly,
                ReportsState,
            ),
            (
                "slipbox/ensureFileNode",
                Capture,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/appendHeading",
                Capture,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/appendHeadingToNode",
                Capture,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/appendHeadingAtOutlinePath",
                Capture,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/ensureNodeId",
                Capture,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/updateNodeMetadata",
                Capture,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/refileSubtree",
                StructuralEdit,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/refileRegion",
                StructuralEdit,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/extractSubtree",
                StructuralEdit,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/promoteEntireFile",
                StructuralEdit,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/demoteEntireFile",
                StructuralEdit,
                OrgContent,
                RefreshesAffectedFiles,
            ),
            (
                "slipbox/slipboxLinkRewritePreview",
                LinkRewrite,
                ReadOnly,
                ReadsDerivedIndex,
            ),
            (
                "slipbox/slipboxLinkRewriteApply",
                LinkRewrite,
                OrgContent,
                RefreshesAffectedFiles,
            ),
        ];
        let actual: Vec<_> = OPERATIONS
            .iter()
            .map(|operation| {
                (
                    operation.descriptor.method,
                    operation.descriptor.family,
                    operation.descriptor.mutation,
                    operation.descriptor.freshness,
                )
            })
            .collect();

        assert_eq!(actual, expected);
    }
}
