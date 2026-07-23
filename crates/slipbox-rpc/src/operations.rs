//! Operation classification for the JSON-RPC surface.
//!
//! Every method the daemon answers is classified here by family, mutation class,
//! and index-freshness behavior, ahead of the handler that runs it.

use crate::{
    METHOD_AGENDA, METHOD_ANCHOR_AT_POINT, METHOD_ANCHOR_FROM_KEY, METHOD_APPEND_HEADING,
    METHOD_APPEND_HEADING_AT_OUTLINE_PATH, METHOD_APPEND_HEADING_TO_NODE, METHOD_BACKLINKS,
    METHOD_CAPTURE_NODE, METHOD_CAPTURE_TEMPLATE, METHOD_CAPTURE_TEMPLATE_PREVIEW,
    METHOD_COMPARE_NOTES, METHOD_CORPUS_AUDIT, METHOD_DELETE_EXPLORATION_ARTIFACT,
    METHOD_DELETE_REVIEW_RUN, METHOD_DELETE_WORKBENCH_PACK, METHOD_DEMOTE_ENTIRE_FILE,
    METHOD_DIAGNOSE_FILE, METHOD_DIAGNOSE_INDEX, METHOD_DIAGNOSE_NODE, METHOD_DIFF_REVIEW_RUNS,
    METHOD_ENSURE_FILE_NODE, METHOD_ENSURE_NODE_ID, METHOD_EXECUTE_EXPLORATION_ARTIFACT,
    METHOD_EXPLORATION_ARTIFACT, METHOD_EXPLORE, METHOD_EXPORT_WORKBENCH_PACK,
    METHOD_EXTRACT_SUBTREE, METHOD_FORWARD_LINKS, METHOD_GLOSSARY_DUE, METHOD_GLOSSARY_TERM,
    METHOD_GRADE_TERM, METHOD_GRAPH_DOT, METHOD_IMPORT_WORKBENCH_PACK, METHOD_INDEX,
    METHOD_INDEX_FILE, METHOD_INDEXED_FILES, METHOD_LIST_EXPLORATION_ARTIFACTS,
    METHOD_LIST_GLOSSARY_TERMS, METHOD_LIST_REVIEW_ROUTINES, METHOD_LIST_REVIEW_RUNS,
    METHOD_LIST_WORKBENCH_PACKS, METHOD_LIST_WORKFLOWS, METHOD_MARK_GLOSSARY_TERM,
    METHOD_MARK_REVIEW_FINDING, METHOD_NODE_AT_POINT, METHOD_NODE_FROM_ID, METHOD_NODE_FROM_KEY,
    METHOD_NODE_FROM_REF, METHOD_NODE_FROM_TITLE_OR_ALIAS, METHOD_NOTE_CONTEXT, METHOD_PING,
    METHOD_PROMOTE_ENTIRE_FILE, METHOD_RANDOM_NODE, METHOD_READ_FILE_SOURCE,
    METHOD_READ_NODE_SOURCE, METHOD_REFILE_REGION, METHOD_REFILE_SUBTREE, METHOD_REFLINKS,
    METHOD_REVIEW_FINDING_REMEDIATION_APPLY, METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW,
    METHOD_REVIEW_ROUTINE, METHOD_REVIEW_RUN, METHOD_RUN_REVIEW_ROUTINE, METHOD_RUN_WORKFLOW,
    METHOD_SAVE_CORPUS_AUDIT_REVIEW, METHOD_SAVE_EXPLORATION_ARTIFACT, METHOD_SAVE_REVIEW_RUN,
    METHOD_SAVE_WORKFLOW_REVIEW, METHOD_SEARCH_FILES, METHOD_SEARCH_GLOSSARY,
    METHOD_SEARCH_NODE_CONTENT, METHOD_SEARCH_NODES, METHOD_SEARCH_OCCURRENCES, METHOD_SEARCH_REFS,
    METHOD_SEARCH_TAGS, METHOD_SLIPBOX_LINK_REWRITE_APPLY, METHOD_SLIPBOX_LINK_REWRITE_PREVIEW,
    METHOD_STATUS, METHOD_UNLINKED_REFERENCES, METHOD_UPDATE_NODE_METADATA,
    METHOD_VALIDATE_WORKBENCH_PACK, METHOD_WORKBENCH_PACK, METHOD_WORKFLOW,
};

/// Public bucket a method belongs to.
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

/// What durable state, if any, a method may change, ordered from least to most
/// invasive. A read-only front-end admits only [`OperationMutation::ReadOnly`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMutation {
    ReadOnly,
    DerivedIndex,
    DurableState,
    OrgContent,
}

/// How a method interacts with derived-index freshness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessBehavior {
    ReportsState,
    ReadsDerivedIndex,
    ReadsSourceFiles,
    RefreshesRootIndex,
    RefreshesOneFile,
    RefreshesAffectedFiles,
}

/// Immutable classification of one JSON-RPC method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationDescriptor {
    pub method: &'static str,
    pub family: OperationFamily,
    pub mutation: OperationMutation,
    pub freshness: FreshnessBehavior,
    pub description: &'static str,
}

macro_rules! descriptor {
    ($method:expr, $family:ident, $mutation:ident, $freshness:ident, $description:expr) => {
        OperationDescriptor {
            method: $method,
            family: OperationFamily::$family,
            mutation: OperationMutation::$mutation,
            freshness: FreshnessBehavior::$freshness,
            description: $description,
        }
    };
}

/// Compatibility inventory of every classified JSON-RPC method. The order is
/// stable and pinned by tests.
const DESCRIPTORS: &[OperationDescriptor] = &[
    descriptor!(
        METHOD_PING,
        System,
        ReadOnly,
        ReportsState,
        "Check daemon liveness and root identity."
    ),
    descriptor!(
        METHOD_STATUS,
        System,
        ReadOnly,
        ReportsState,
        "Return daemon root, database path, and index counts."
    ),
    descriptor!(
        METHOD_INDEX,
        System,
        DerivedIndex,
        RefreshesRootIndex,
        "Refresh the derived index from root discovery."
    ),
    descriptor!(
        METHOD_INDEX_FILE,
        Files,
        DerivedIndex,
        RefreshesOneFile,
        "Refresh one file in the derived index without pruning unrelated files."
    ),
    descriptor!(
        METHOD_INDEXED_FILES,
        Files,
        ReadOnly,
        ReadsDerivedIndex,
        "List files currently represented in the derived index."
    ),
    descriptor!(
        METHOD_SEARCH_FILES,
        Files,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed file records."
    ),
    descriptor!(
        METHOD_DIAGNOSE_FILE,
        Diagnostics,
        ReadOnly,
        ReportsState,
        "Explain discovery and index state for one file."
    ),
    descriptor!(
        METHOD_DIAGNOSE_NODE,
        Diagnostics,
        ReadOnly,
        ReportsState,
        "Explain indexed source-location state for one node."
    ),
    descriptor!(
        METHOD_DIAGNOSE_INDEX,
        Diagnostics,
        ReadOnly,
        ReportsState,
        "Report drift between root discovery and the derived index."
    ),
    descriptor!(
        METHOD_SEARCH_NODES,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed note records."
    ),
    descriptor!(
        METHOD_SEARCH_NODE_CONTENT,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed note content and return a highlighted excerpt per hit."
    ),
    descriptor!(
        METHOD_RANDOM_NODE,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Return one random indexed note."
    ),
    descriptor!(
        METHOD_NODE_FROM_ID,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve a note by exact Org ID."
    ),
    descriptor!(
        METHOD_NODE_FROM_KEY,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve a note by exact slipbox key."
    ),
    descriptor!(
        METHOD_NODE_FROM_TITLE_OR_ALIAS,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve notes by exact title or alias."
    ),
    descriptor!(
        METHOD_NODE_AT_POINT,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve the indexed note at a source position."
    ),
    descriptor!(
        METHOD_ANCHOR_AT_POINT,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve the indexed anchor at a source position."
    ),
    descriptor!(
        METHOD_ANCHOR_FROM_KEY,
        Notes,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve an indexed anchor by exact slipbox key."
    ),
    descriptor!(
        METHOD_READ_FILE_SOURCE,
        Files,
        ReadOnly,
        ReadsSourceFiles,
        "Read a bounded slice from an eligible source file."
    ),
    descriptor!(
        METHOD_READ_NODE_SOURCE,
        Notes,
        ReadOnly,
        ReadsSourceFiles,
        "Read a bounded slice for an indexed source anchor."
    ),
    descriptor!(
        METHOD_NOTE_CONTEXT,
        Notes,
        ReadOnly,
        ReadsSourceFiles,
        "Read a source slice and immediate relation context for an indexed note."
    ),
    descriptor!(
        METHOD_SEARCH_OCCURRENCES,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Search raw indexed text occurrences."
    ),
    descriptor!(
        METHOD_SEARCH_TAGS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed Org tags."
    ),
    descriptor!(
        METHOD_SEARCH_REFS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed reference strings."
    ),
    descriptor!(
        METHOD_NODE_FROM_REF,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve the note owning an exact indexed reference."
    ),
    descriptor!(
        METHOD_BACKLINKS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Return incoming links for an indexed note."
    ),
    descriptor!(
        METHOD_FORWARD_LINKS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Return outgoing links for an indexed note."
    ),
    descriptor!(
        METHOD_REFLINKS,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Return links to indexed references."
    ),
    descriptor!(
        METHOD_UNLINKED_REFERENCES,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Find mention candidates for indexed references."
    ),
    descriptor!(
        METHOD_AGENDA,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Query indexed planning entries."
    ),
    descriptor!(
        METHOD_GRAPH_DOT,
        Relations,
        ReadOnly,
        ReadsDerivedIndex,
        "Render the indexed relation graph as DOT."
    ),
    descriptor!(
        METHOD_EXPLORE,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "Run a live exploration lens."
    ),
    descriptor!(
        METHOD_COMPARE_NOTES,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "Compare two indexed notes."
    ),
    descriptor!(
        METHOD_SAVE_EXPLORATION_ARTIFACT,
        Exploration,
        DurableState,
        ReadsDerivedIndex,
        "Persist an exploration artifact record."
    ),
    descriptor!(
        METHOD_EXPLORATION_ARTIFACT,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "Read one persisted exploration artifact."
    ),
    descriptor!(
        METHOD_LIST_EXPLORATION_ARTIFACTS,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "List persisted exploration artifacts."
    ),
    descriptor!(
        METHOD_DELETE_EXPLORATION_ARTIFACT,
        Exploration,
        DurableState,
        ReadsDerivedIndex,
        "Delete one persisted exploration artifact."
    ),
    descriptor!(
        METHOD_EXECUTE_EXPLORATION_ARTIFACT,
        Exploration,
        ReadOnly,
        ReadsDerivedIndex,
        "Execute one persisted exploration artifact."
    ),
    descriptor!(
        METHOD_CORPUS_AUDIT,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "Run a read-only corpus audit."
    ),
    descriptor!(
        METHOD_SAVE_REVIEW_RUN,
        Reviews,
        DurableState,
        ReadsDerivedIndex,
        "Persist a review run."
    ),
    descriptor!(
        METHOD_REVIEW_RUN,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "Read one persisted review run."
    ),
    descriptor!(
        METHOD_DIFF_REVIEW_RUNS,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "Diff two persisted review runs."
    ),
    descriptor!(
        METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "Preview remediation for one review finding."
    ),
    descriptor!(
        METHOD_REVIEW_FINDING_REMEDIATION_APPLY,
        Reviews,
        OrgContent,
        RefreshesAffectedFiles,
        "Apply remediation for one review finding and refresh affected files."
    ),
    descriptor!(
        METHOD_LIST_REVIEW_RUNS,
        Reviews,
        ReadOnly,
        ReadsDerivedIndex,
        "List persisted review runs."
    ),
    descriptor!(
        METHOD_DELETE_REVIEW_RUN,
        Reviews,
        DurableState,
        ReadsDerivedIndex,
        "Delete one persisted review run."
    ),
    descriptor!(
        METHOD_MARK_REVIEW_FINDING,
        Reviews,
        DurableState,
        ReadsDerivedIndex,
        "Mark the state of one persisted review finding."
    ),
    descriptor!(
        METHOD_SAVE_CORPUS_AUDIT_REVIEW,
        Reviews,
        DurableState,
        ReadsDerivedIndex,
        "Run an audit and persist its review record."
    ),
    descriptor!(
        METHOD_LIST_WORKFLOWS,
        Assets,
        ReadOnly,
        ReportsState,
        "List discovered workflows."
    ),
    descriptor!(
        METHOD_WORKFLOW,
        Assets,
        ReadOnly,
        ReportsState,
        "Read one discovered workflow."
    ),
    // A workflow's own steps decide what it writes: an `artifactSave` step
    // persists an exploration artifact.
    descriptor!(
        METHOD_RUN_WORKFLOW,
        Assets,
        DurableState,
        ReadsDerivedIndex,
        "Run one workflow without persisting a review."
    ),
    descriptor!(
        METHOD_SAVE_WORKFLOW_REVIEW,
        Assets,
        DurableState,
        ReadsDerivedIndex,
        "Run one workflow and persist its review record."
    ),
    descriptor!(
        METHOD_LIST_REVIEW_ROUTINES,
        Assets,
        ReadOnly,
        ReportsState,
        "List discovered review routines."
    ),
    descriptor!(
        METHOD_REVIEW_ROUTINE,
        Assets,
        ReadOnly,
        ReportsState,
        "Read one discovered review routine."
    ),
    // A routine with `save_review` enabled writes a durable review run, and its
    // source may be a writing workflow.
    descriptor!(
        METHOD_RUN_REVIEW_ROUTINE,
        Assets,
        DurableState,
        ReadsDerivedIndex,
        "Run one review routine."
    ),
    descriptor!(
        METHOD_IMPORT_WORKBENCH_PACK,
        Assets,
        DurableState,
        ReportsState,
        "Import a durable workbench pack."
    ),
    descriptor!(
        METHOD_WORKBENCH_PACK,
        Assets,
        ReadOnly,
        ReportsState,
        "Read one durable workbench pack."
    ),
    descriptor!(
        METHOD_VALIDATE_WORKBENCH_PACK,
        Assets,
        ReadOnly,
        ReportsState,
        "Validate a workbench pack payload."
    ),
    descriptor!(
        METHOD_EXPORT_WORKBENCH_PACK,
        Assets,
        ReadOnly,
        ReportsState,
        "Export one durable workbench pack."
    ),
    descriptor!(
        METHOD_LIST_WORKBENCH_PACKS,
        Assets,
        ReadOnly,
        ReportsState,
        "List durable workbench packs."
    ),
    descriptor!(
        METHOD_DELETE_WORKBENCH_PACK,
        Assets,
        DurableState,
        ReportsState,
        "Delete one durable workbench pack."
    ),
    descriptor!(
        METHOD_LIST_GLOSSARY_TERMS,
        Glossary,
        ReadOnly,
        ReadsDerivedIndex,
        "List indexed glossary terms."
    ),
    descriptor!(
        METHOD_SEARCH_GLOSSARY,
        Glossary,
        ReadOnly,
        ReadsDerivedIndex,
        "Search indexed glossary terms."
    ),
    descriptor!(
        METHOD_GLOSSARY_DUE,
        Glossary,
        ReadOnly,
        ReadsDerivedIndex,
        "List glossary terms due for review."
    ),
    descriptor!(
        METHOD_GLOSSARY_TERM,
        Glossary,
        ReadOnly,
        ReadsDerivedIndex,
        "Resolve one glossary term by slipbox key."
    ),
    descriptor!(
        METHOD_GRADE_TERM,
        Glossary,
        OrgContent,
        RefreshesAffectedFiles,
        "Grade a glossary term, rewrite its review drawer, and refresh it."
    ),
    descriptor!(
        METHOD_MARK_GLOSSARY_TERM,
        Glossary,
        OrgContent,
        RefreshesAffectedFiles,
        "Mark a note as a glossary term and refresh it."
    ),
    descriptor!(
        METHOD_CAPTURE_NODE,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Create a file note and refresh its index record."
    ),
    descriptor!(
        METHOD_CAPTURE_TEMPLATE,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Capture from a template and refresh the affected index record."
    ),
    descriptor!(
        METHOD_CAPTURE_TEMPLATE_PREVIEW,
        Capture,
        ReadOnly,
        ReportsState,
        "Preview a template capture without writing files."
    ),
    descriptor!(
        METHOD_ENSURE_FILE_NODE,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Ensure a file-level note exists and refresh it."
    ),
    descriptor!(
        METHOD_APPEND_HEADING,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Append a heading to a file and refresh it."
    ),
    descriptor!(
        METHOD_APPEND_HEADING_TO_NODE,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Append a heading below a resolved note and refresh it."
    ),
    descriptor!(
        METHOD_APPEND_HEADING_AT_OUTLINE_PATH,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Append a heading at an outline path and refresh it."
    ),
    descriptor!(
        METHOD_ENSURE_NODE_ID,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Ensure an anchor has an Org ID and refresh it."
    ),
    descriptor!(
        METHOD_UPDATE_NODE_METADATA,
        Capture,
        OrgContent,
        RefreshesAffectedFiles,
        "Update note metadata and refresh the affected file."
    ),
    descriptor!(
        METHOD_REFILE_SUBTREE,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Move a subtree to another target and refresh affected files."
    ),
    descriptor!(
        METHOD_REFILE_REGION,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Refile a source region and refresh affected files."
    ),
    descriptor!(
        METHOD_EXTRACT_SUBTREE,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Extract a subtree into a target file and refresh affected files."
    ),
    descriptor!(
        METHOD_PROMOTE_ENTIRE_FILE,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Promote every heading in a file and refresh it."
    ),
    descriptor!(
        METHOD_DEMOTE_ENTIRE_FILE,
        StructuralEdit,
        OrgContent,
        RefreshesAffectedFiles,
        "Demote every heading in a file and refresh it."
    ),
    descriptor!(
        METHOD_SLIPBOX_LINK_REWRITE_PREVIEW,
        LinkRewrite,
        ReadOnly,
        ReadsDerivedIndex,
        "Preview a slipbox link rewrite without writing files."
    ),
    descriptor!(
        METHOD_SLIPBOX_LINK_REWRITE_APPLY,
        LinkRewrite,
        OrgContent,
        RefreshesAffectedFiles,
        "Apply a slipbox link rewrite and refresh affected files."
    ),
];

/// Iterate the stable compatibility inventory of operation descriptors.
pub fn operation_descriptors() -> impl ExactSizeIterator<Item = OperationDescriptor> {
    DESCRIPTORS.iter().copied()
}

/// Resolve the descriptor for an exact method name.
#[must_use]
pub fn operation_descriptor_by_method(method: &str) -> Option<OperationDescriptor> {
    DESCRIPTORS
        .iter()
        .copied()
        .find(|descriptor| descriptor.method == method)
}

/// Report whether a method is safe for a read-only caller to invoke. Fail-closed:
/// an unknown method is rejected.
#[must_use]
pub fn is_read_only(method: &str) -> bool {
    operation_descriptor_by_method(method)
        .is_some_and(|descriptor| matches!(descriptor.mutation, OperationMutation::ReadOnly))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        DESCRIPTORS, FreshnessBehavior, OperationFamily, OperationMutation, is_read_only,
        operation_descriptor_by_method, operation_descriptors,
    };

    #[test]
    fn descriptor_methods_are_unique_and_lookupable() {
        let mut methods = HashSet::new();
        for descriptor in DESCRIPTORS {
            assert!(
                methods.insert(descriptor.method),
                "duplicate descriptor method {}",
                descriptor.method
            );
            assert!(
                operation_descriptor_by_method(descriptor.method).is_some(),
                "missing lookup for {}",
                descriptor.method
            );
        }
        assert_eq!(methods.len(), operation_descriptors().len());
    }

    #[test]
    fn is_read_only_admits_only_read_only_methods() {
        for descriptor in DESCRIPTORS {
            let expected = matches!(descriptor.mutation, OperationMutation::ReadOnly);
            assert_eq!(
                is_read_only(descriptor.method),
                expected,
                "read-only classification drifted for {}",
                descriptor.method
            );
        }
    }

    #[test]
    fn is_read_only_rejects_unknown_methods() {
        assert!(!is_read_only("slipbox/doesNotExist"));
        assert!(!is_read_only(""));
    }

    #[test]
    fn descriptors_are_explicit_compatibility_inventory() {
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
            (
                "slipbox/searchNodeContent",
                Notes,
                ReadOnly,
                ReadsDerivedIndex,
            ),
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
            (
                "slipbox/runWorkflow",
                Assets,
                DurableState,
                ReadsDerivedIndex,
            ),
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
                DurableState,
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
        let actual: Vec<_> = DESCRIPTORS
            .iter()
            .map(|descriptor| {
                (
                    descriptor.method,
                    descriptor.family,
                    descriptor.mutation,
                    descriptor.freshness,
                )
            })
            .collect();

        assert_eq!(actual, expected);
    }
}
