mod assets;
mod common;
mod exploration;
mod notes;
mod relations;
mod reviews;
mod system;

pub(crate) use assets::{
    delete_workbench_pack, export_workbench_pack, import_workbench_pack, list_review_routines,
    list_workbench_packs, list_workflows, review_routine, run_review_routine, run_workflow,
    save_workflow_review, validate_workbench_pack, workbench_pack, workflow,
};
pub(crate) use exploration::{
    compare_notes, delete_exploration_artifact, execute_exploration_artifact, exploration_artifact,
    explore, list_exploration_artifacts, save_exploration_artifact,
};
pub(crate) use notes::{
    anchor_at_point, node_at_point, node_from_id, node_from_key, node_from_title_or_alias,
    random_node, search_nodes,
};
pub(crate) use relations::{
    agenda, backlinks, forward_links, graph_dot, node_from_ref, reflinks, search_occurrences,
    search_refs, search_tags, unlinked_references,
};
pub(crate) use reviews::{
    corpus_audit, delete_review_run, diff_review_runs, list_review_runs, mark_review_finding,
    review_finding_remediation_apply, review_finding_remediation_preview, review_run,
    save_corpus_audit_review, save_review_run,
};
pub(crate) use system::{
    diagnose_file, diagnose_index, diagnose_node, index, index_file, indexed_files, ping,
    search_files, status,
};

#[cfg(test)]
use assets::execute_workflow_spec;
#[cfg(test)]
use exploration::{
    execute_compare_notes_query, execute_explore_query, execute_saved_exploration_artifact,
    execute_saved_exploration_artifact_by_id,
};

#[cfg(test)]
mod tests;
