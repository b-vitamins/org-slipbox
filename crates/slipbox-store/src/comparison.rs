use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use slipbox_core::{
    CompareNotesParams, ComparisonConnectorDirection, ComparisonNodeRecord,
    ComparisonReferenceRecord, NodeRecord, NoteComparisonEntry, NoteComparisonExplanation,
    NoteComparisonResult, NoteComparisonSection, NoteComparisonSectionKind,
};

use crate::Database;

const COMPARISON_NEIGHBOR_LIMIT: usize = 1_000;

impl Database {
    pub fn compare_notes(
        &self,
        left: &NodeRecord,
        right: &NodeRecord,
        params: &CompareNotesParams,
    ) -> Result<NoteComparisonResult> {
        let limit = params.normalized_limit();
        let left_backlinks = self
            .backlinks(&left.node_key, COMPARISON_NEIGHBOR_LIMIT, true)?
            .into_iter()
            .map(|record| record.source_note)
            .collect::<Vec<_>>();
        let right_backlinks = self
            .backlinks(&right.node_key, COMPARISON_NEIGHBOR_LIMIT, true)?
            .into_iter()
            .map(|record| record.source_note)
            .collect::<Vec<_>>();
        let left_forward_links = self
            .forward_links(&left.node_key, COMPARISON_NEIGHBOR_LIMIT, true)?
            .into_iter()
            .map(|record| record.destination_note)
            .collect::<Vec<_>>();
        let right_forward_links = self
            .forward_links(&right.node_key, COMPARISON_NEIGHBOR_LIMIT, true)?
            .into_iter()
            .map(|record| record.destination_note)
            .collect::<Vec<_>>();

        let left_refs = left.refs.iter().cloned().collect::<BTreeSet<_>>();
        let right_refs = right.refs.iter().cloned().collect::<BTreeSet<_>>();
        let left_backlink_map = notes_by_key(left_backlinks);
        let right_backlink_map = notes_by_key(right_backlinks);
        let left_forward_map = notes_by_key(left_forward_links);
        let right_forward_map = notes_by_key(right_forward_links);

        Ok(NoteComparisonResult {
            left_note: left.clone(),
            right_note: right.clone(),
            sections: vec![
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::SharedRefs,
                    entries: shared_references(&left_refs, &right_refs, limit),
                },
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::LeftOnlyRefs,
                    entries: exclusive_references(
                        &left_refs,
                        &right_refs,
                        NoteComparisonExplanation::LeftOnlyReference,
                        limit,
                    ),
                },
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::RightOnlyRefs,
                    entries: exclusive_references(
                        &right_refs,
                        &left_refs,
                        NoteComparisonExplanation::RightOnlyReference,
                        limit,
                    ),
                },
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::SharedBacklinks,
                    entries: shared_nodes(
                        &left_backlink_map,
                        &right_backlink_map,
                        NoteComparisonExplanation::SharedBacklink,
                        limit,
                    ),
                },
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::SharedForwardLinks,
                    entries: shared_nodes(
                        &left_forward_map,
                        &right_forward_map,
                        NoteComparisonExplanation::SharedForwardLink,
                        limit,
                    ),
                },
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::IndirectConnectors,
                    entries: indirect_connectors(
                        &left_backlink_map,
                        &right_backlink_map,
                        &left_forward_map,
                        &right_forward_map,
                        limit,
                    ),
                },
            ],
        })
    }
}

fn shared_references(
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
    limit: usize,
) -> Vec<NoteComparisonEntry> {
    left.intersection(right)
        .take(limit)
        .cloned()
        .map(|reference| {
            comparison_reference_entry(reference, NoteComparisonExplanation::SharedReference)
        })
        .collect()
}

fn exclusive_references(
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
    explanation: NoteComparisonExplanation,
    limit: usize,
) -> Vec<NoteComparisonEntry> {
    left.difference(right)
        .take(limit)
        .cloned()
        .map(|reference| comparison_reference_entry(reference, explanation.clone()))
        .collect()
}

fn shared_nodes(
    left: &BTreeMap<String, NodeRecord>,
    right: &BTreeMap<String, NodeRecord>,
    explanation: NoteComparisonExplanation,
    limit: usize,
) -> Vec<NoteComparisonEntry> {
    let mut shared = left
        .iter()
        .filter_map(|(node_key, node)| right.contains_key(node_key).then_some(node.clone()))
        .collect::<Vec<_>>();
    shared.sort_by(compare_node_records);
    shared
        .into_iter()
        .take(limit)
        .map(|node| comparison_node_entry(node, explanation.clone()))
        .collect()
}

fn indirect_connectors(
    left_backlinks: &BTreeMap<String, NodeRecord>,
    right_backlinks: &BTreeMap<String, NodeRecord>,
    left_forward: &BTreeMap<String, NodeRecord>,
    right_forward: &BTreeMap<String, NodeRecord>,
    limit: usize,
) -> Vec<NoteComparisonEntry> {
    let mut connectors: BTreeMap<String, (NodeRecord, bool, bool)> = BTreeMap::new();

    for (node_key, node) in left_forward {
        if right_backlinks.contains_key(node_key) {
            let entry = connectors
                .entry(node_key.clone())
                .or_insert_with(|| (node.clone(), false, false));
            entry.1 = true;
        }
    }

    for (node_key, node) in right_forward {
        if left_backlinks.contains_key(node_key) {
            let entry = connectors
                .entry(node_key.clone())
                .or_insert_with(|| (node.clone(), false, false));
            entry.2 = true;
        }
    }

    let mut shared = connectors.into_values().collect::<Vec<_>>();
    shared.sort_by(|(left, _, _), (right, _, _)| compare_node_records(left, right));
    shared
        .into_iter()
        .take(limit)
        .map(|(node, left_to_right, right_to_left)| {
            let direction = match (left_to_right, right_to_left) {
                (true, true) => ComparisonConnectorDirection::Bidirectional,
                (true, false) => ComparisonConnectorDirection::LeftToRight,
                (false, true) => ComparisonConnectorDirection::RightToLeft,
                (false, false) => unreachable!("connector entries always have a direction"),
            };
            comparison_node_entry(
                node,
                NoteComparisonExplanation::IndirectConnector { direction },
            )
        })
        .collect()
}

fn notes_by_key(nodes: Vec<NodeRecord>) -> BTreeMap<String, NodeRecord> {
    nodes
        .into_iter()
        .map(|node| (node.node_key.clone(), node))
        .collect()
}

fn comparison_reference_entry(
    reference: String,
    explanation: NoteComparisonExplanation,
) -> NoteComparisonEntry {
    NoteComparisonEntry::Reference {
        record: Box::new(ComparisonReferenceRecord {
            reference,
            explanation,
        }),
    }
}

fn comparison_node_entry(
    node: NodeRecord,
    explanation: NoteComparisonExplanation,
) -> NoteComparisonEntry {
    NoteComparisonEntry::Node {
        record: Box::new(ComparisonNodeRecord { node, explanation }),
    }
}

fn compare_node_records(left: &NodeRecord, right: &NodeRecord) -> Ordering {
    left.file_path
        .cmp(&right.file_path)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.node_key.cmp(&right.node_key))
}
