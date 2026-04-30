use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use rusqlite::params_from_iter;

use slipbox_core::{
    AnchorExplorationRecord, AnchorRecord, BridgeEvidenceRecord, ExplorationExplanation,
    NodeRecord, PlanningField, PlanningRelationRecord,
};

use crate::Database;
use crate::nodes::{
    ANCHOR_SELECT_COLUMN_COUNT, anchor_select_columns, note_where, row_to_anchor,
    row_to_anchor_with_offset,
};

const EXPLORATION_NEIGHBOR_LIMIT: usize = 1_000;
const RELATED_REF_SCAN_FACTOR: usize = 5;

struct SharedRefCandidate {
    anchor: AnchorRecord,
    references: Vec<String>,
}

impl SharedRefCandidate {
    fn shared_reference_count(&self) -> usize {
        self.references.len()
    }
}

struct BridgeCandidate {
    candidate: SharedRefCandidate,
    via_notes: Vec<BridgeEvidenceRecord>,
}

impl BridgeCandidate {
    fn bridge_count(&self) -> usize {
        self.via_notes.len()
    }
}

impl Database {
    pub fn time_neighbors(
        &self,
        anchor: &AnchorRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        if distinct_planning_dates(anchor).is_empty() {
            return Ok(Vec::new());
        }

        Ok(self
            .ranked_time_neighbor_candidates(anchor.node_key.as_str(), anchor, limit)?
            .into_iter()
            .map(|candidate| {
                let relations = planning_relations(anchor, &candidate);
                debug_assert!(!relations.is_empty());
                AnchorExplorationRecord {
                    anchor: candidate,
                    explanation: ExplorationExplanation::TimeNeighbor { relations },
                }
            })
            .collect())
    }

    pub fn task_neighbors(
        &self,
        anchor: &AnchorRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        let shared_todo_keyword = anchor.todo_keyword.clone();
        if shared_todo_keyword.is_none() && distinct_planning_dates(anchor).is_empty() {
            return Ok(Vec::new());
        }

        Ok(self
            .ranked_task_neighbor_candidates(anchor.node_key.as_str(), anchor, limit)?
            .into_iter()
            .map(|candidate| {
                let planning_relations = planning_relations(anchor, &candidate);
                let shared_todo_keyword = shared_todo_keyword
                    .as_deref()
                    .filter(|todo_keyword| candidate.todo_keyword.as_deref() == Some(*todo_keyword))
                    .map(ToOwned::to_owned);
                debug_assert!(shared_todo_keyword.is_some() || !planning_relations.is_empty());
                AnchorExplorationRecord {
                    anchor: candidate,
                    explanation: ExplorationExplanation::TaskNeighbor {
                        shared_todo_keyword,
                        planning_relations,
                    },
                }
            })
            .collect())
    }

    pub fn bridge_candidates(
        &self,
        note: &NodeRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        let direct_neighbors = self.direct_neighbor_map(note)?;
        if note.refs.is_empty() || direct_neighbors.is_empty() {
            return Ok(Vec::new());
        }

        let direct_neighbor_keys = direct_neighbors.keys().cloned().collect::<BTreeSet<_>>();
        let direct_neighbor_key_list = direct_neighbor_keys.iter().cloned().collect::<Vec<_>>();
        let direct_neighbor_explicit_ids = direct_neighbors
            .values()
            .filter_map(|neighbor| neighbor.explicit_id.clone())
            .collect::<Vec<_>>();
        let candidates = self.shared_ref_candidates(
            note,
            &excluded_keys(note, &direct_neighbor_keys),
            widened_limit(limit),
        )?;

        let mut bridge_candidates = Vec::new();
        for candidate in candidates {
            let via_notes = self.bridge_notes(
                candidate.anchor.node_key.as_str(),
                candidate.anchor.explicit_id.as_deref(),
                &direct_neighbor_key_list,
                &direct_neighbor_explicit_ids,
            )?;
            if !via_notes.is_empty() {
                bridge_candidates.push(BridgeCandidate {
                    candidate,
                    via_notes,
                });
            }
        }
        bridge_candidates.sort_by(compare_bridge_candidates);

        Ok(bridge_candidates
            .into_iter()
            .take(limit.clamp(1, 1_000))
            .map(|bridge| AnchorExplorationRecord {
                anchor: bridge.candidate.anchor,
                explanation: ExplorationExplanation::BridgeCandidate {
                    references: bridge.candidate.references,
                    via_notes: bridge.via_notes,
                },
            })
            .collect())
    }

    pub fn dormant_related(
        &self,
        note: &NodeRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        if note.refs.is_empty() || note.file_mtime_ns <= 0 {
            return Ok(Vec::new());
        }

        let direct_neighbor_keys = self
            .direct_neighbor_map(note)?
            .into_keys()
            .collect::<BTreeSet<_>>();
        let mut candidates = self
            .shared_ref_candidates(
                note,
                &excluded_keys(note, &direct_neighbor_keys),
                widened_limit(limit),
            )?
            .into_iter()
            .filter(|candidate| {
                candidate.anchor.file_mtime_ns > 0
                    && candidate.anchor.file_mtime_ns < note.file_mtime_ns
            })
            .collect::<Vec<_>>();
        candidates.sort_by(compare_dormant_candidates);

        Ok(candidates
            .into_iter()
            .take(limit.clamp(1, 1_000))
            .map(|candidate| AnchorExplorationRecord {
                anchor: candidate.anchor.clone(),
                explanation: ExplorationExplanation::DormantSharedReference {
                    references: candidate.references,
                    modified_at_ns: candidate.anchor.file_mtime_ns,
                },
            })
            .collect())
    }

    pub fn unresolved_tasks(
        &self,
        note: &NodeRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        if note.refs.is_empty() {
            return Ok(Vec::new());
        }

        let direct_neighbor_keys = self
            .direct_neighbor_map(note)?
            .into_keys()
            .collect::<BTreeSet<_>>();
        let mut candidates = self.shared_ref_candidates(
            note,
            &excluded_keys(note, &direct_neighbor_keys),
            widened_limit(limit),
        )?;
        candidates.retain(|candidate| candidate.anchor.todo_keyword.is_some());
        candidates.sort_by(compare_unresolved_candidates);

        Ok(candidates
            .into_iter()
            .filter_map(|candidate| {
                candidate
                    .anchor
                    .todo_keyword
                    .clone()
                    .map(|todo_keyword| AnchorExplorationRecord {
                        anchor: candidate.anchor,
                        explanation: ExplorationExplanation::UnresolvedSharedReference {
                            references: candidate.references,
                            todo_keyword,
                        },
                    })
            })
            .take(limit.clamp(1, 1_000))
            .collect())
    }

    pub fn weakly_integrated_notes(
        &self,
        note: &NodeRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        if note.refs.is_empty() {
            return Ok(Vec::new());
        }

        let direct_neighbor_keys = self
            .direct_neighbor_map(note)?
            .into_keys()
            .collect::<BTreeSet<_>>();
        let mut candidates = self
            .shared_ref_candidates(
                note,
                &excluded_keys(note, &direct_neighbor_keys),
                widened_limit(limit),
            )?
            .into_iter()
            .filter(|candidate| {
                candidate.anchor.todo_keyword.is_none()
                    && structural_link_count(&candidate.anchor) <= 1
            })
            .collect::<Vec<_>>();
        candidates.sort_by(compare_weakly_integrated_candidates);

        Ok(candidates
            .into_iter()
            .take(limit.clamp(1, 1_000))
            .map(|candidate| AnchorExplorationRecord {
                anchor: candidate.anchor.clone(),
                explanation: ExplorationExplanation::WeaklyIntegratedSharedReference {
                    references: candidate.references,
                    structural_link_count: structural_link_count(&candidate.anchor),
                },
            })
            .collect())
    }

    fn direct_neighbor_map(&self, note: &NodeRecord) -> Result<BTreeMap<String, NodeRecord>> {
        let backlinks = self.backlinks(note.node_key.as_str(), EXPLORATION_NEIGHBOR_LIMIT, true)?;
        let forward_links =
            self.forward_links(note.node_key.as_str(), EXPLORATION_NEIGHBOR_LIMIT, true)?;
        let mut neighbors = BTreeMap::new();
        for backlink in backlinks {
            neighbors.insert(backlink.source_note.node_key.clone(), backlink.source_note);
        }
        for forward_link in forward_links {
            neighbors.insert(
                forward_link.destination_note.node_key.clone(),
                forward_link.destination_note,
            );
        }
        Ok(neighbors)
    }

    fn ranked_time_neighbor_candidates(
        &self,
        anchor_node_key: &str,
        anchor: &AnchorRecord,
        limit: usize,
    ) -> Result<Vec<AnchorRecord>> {
        let planning_dates = distinct_planning_dates(anchor);
        let mut values = vec![anchor_node_key.to_owned()];
        let (match_predicate_sql, next_parameter_index) =
            planning_date_match_predicate_sql("n", &planning_dates, 2, &mut values);
        let (relation_count_sql, limit_placeholder) =
            planning_relation_count_sql(anchor, "n", next_parameter_index, &mut values);
        let sql = format!(
            "WITH ranked AS (
                 SELECT {0},
                        {1} AS relation_count
                   FROM nodes AS n
                  WHERE n.node_key <> ?1
                    AND {2}
             )
             SELECT {3}
               FROM ranked AS r
              ORDER BY r.relation_count DESC, r.file_path, r.line, r.node_key
              LIMIT ?{4}",
            anchor_select_columns("n"),
            relation_count_sql,
            match_predicate_sql,
            anchor_select_columns("r"),
            limit_placeholder,
        );
        values.push(limit.clamp(1, 1_000).to_string());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), row_to_anchor)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read ranked time-neighbor candidates")
    }

    fn ranked_task_neighbor_candidates(
        &self,
        anchor_node_key: &str,
        anchor: &AnchorRecord,
        limit: usize,
    ) -> Result<Vec<AnchorRecord>> {
        let planning_dates = distinct_planning_dates(anchor);
        let mut values = vec![anchor_node_key.to_owned()];
        let mut predicate_parts = Vec::new();
        let mut next_parameter_index = 2;
        let shared_todo_match_sql = if let Some(todo_keyword) = anchor.todo_keyword.as_deref() {
            let placeholder = next_parameter_index;
            values.push(todo_keyword.to_owned());
            next_parameter_index += 1;
            predicate_parts.push(format!("n.todo_keyword = ?{placeholder}"));
            format!("CASE WHEN n.todo_keyword = ?{placeholder} THEN 1 ELSE 0 END")
        } else {
            "0".to_owned()
        };
        if !planning_dates.is_empty() {
            let (planning_predicate_sql, parameter_index) = planning_date_match_predicate_sql(
                "n",
                &planning_dates,
                next_parameter_index,
                &mut values,
            );
            predicate_parts.push(planning_predicate_sql);
            next_parameter_index = parameter_index;
        }
        let (planning_relation_count_sql, limit_placeholder) =
            planning_relation_count_sql(anchor, "n", next_parameter_index, &mut values);
        let evidence_count_sql =
            format!("({shared_todo_match_sql} + {planning_relation_count_sql})");
        let sql = format!(
            "WITH ranked AS (
                 SELECT {0},
                        {1} AS shared_todo_match,
                        {2} AS planning_relation_count,
                        {3} AS evidence_count
                   FROM nodes AS n
                  WHERE n.node_key <> ?1
                    AND n.todo_keyword IS NOT NULL
                    AND ({4})
             )
             SELECT {5}
               FROM ranked AS r
              ORDER BY r.evidence_count DESC,
                       r.shared_todo_match DESC,
                       r.planning_relation_count DESC,
                       r.file_path,
                       r.line,
                       r.node_key
              LIMIT ?{6}",
            anchor_select_columns("n"),
            shared_todo_match_sql,
            planning_relation_count_sql,
            evidence_count_sql,
            predicate_parts.join(" OR "),
            anchor_select_columns("r"),
            limit_placeholder,
        );
        values.push(limit.clamp(1, 1_000).to_string());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), row_to_anchor)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read ranked task-neighbor candidates")
    }

    fn shared_ref_candidates(
        &self,
        note: &NodeRecord,
        excluded_node_keys: &BTreeSet<String>,
        limit: usize,
    ) -> Result<Vec<SharedRefCandidate>> {
        let references = note
            .refs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if references.is_empty() {
            return Ok(Vec::new());
        }

        let excluded = excluded_node_keys.iter().cloned().collect::<Vec<_>>();
        let ref_placeholders = numbered_placeholders(1, references.len());
        let excluded_placeholders = numbered_placeholders(references.len() + 1, excluded.len());
        let limit_placeholder = references.len() + excluded.len() + 1;
        let sql = format!(
            "SELECT {},
                    matches.shared_ref_count
               FROM (
                     SELECT n.node_key AS candidate_node_key,
                            COUNT(DISTINCT r.ref) AS shared_ref_count
                       FROM refs AS r
                       JOIN nodes AS n ON n.node_key = r.node_key
                      WHERE r.ref IN ({})
                        AND r.node_key NOT IN ({})
                        AND {}
                      GROUP BY n.node_key
                      ORDER BY shared_ref_count DESC, n.file_path, n.line, n.node_key
                      LIMIT ?{}
                    ) AS matches
               JOIN nodes AS n ON n.node_key = matches.candidate_node_key
              ORDER BY matches.shared_ref_count DESC, n.file_path, n.line, n.node_key",
            anchor_select_columns("n"),
            ref_placeholders,
            excluded_placeholders,
            note_where("n"),
            limit_placeholder,
        );
        let mut values = references.clone();
        values.extend(excluded);
        values.push(limit.clamp(1, 1_000).to_string());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row_to_anchor_with_offset(row, 0)?,
                row.get::<_, i64>(ANCHOR_SELECT_COLUMN_COUNT)?,
            ))
        })?;
        let ordered_anchors = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read shared-ref exploration candidates")?;
        if ordered_anchors.is_empty() {
            return Ok(Vec::new());
        }

        let candidate_node_keys = ordered_anchors
            .iter()
            .map(|(anchor, _shared_ref_count)| anchor.node_key.clone())
            .collect::<Vec<_>>();
        let candidate_placeholders =
            numbered_placeholders(references.len() + 1, candidate_node_keys.len());
        let sql = format!(
            "SELECT DISTINCT r.node_key, r.ref
               FROM refs AS r
              WHERE r.ref IN ({})
                AND r.node_key IN ({})
              ORDER BY r.node_key, r.ref",
            ref_placeholders, candidate_placeholders,
        );
        let mut values = references;
        values.extend(candidate_node_keys.iter().cloned());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut references_by_node = BTreeMap::<String, Vec<String>>::new();
        for row in rows {
            let (node_key, reference) = row?;
            references_by_node
                .entry(node_key)
                .or_default()
                .push(reference);
        }

        Ok(ordered_anchors
            .into_iter()
            .map(|(anchor, _shared_ref_count)| SharedRefCandidate {
                references: references_by_node
                    .remove(&anchor.node_key)
                    .unwrap_or_default(),
                anchor,
            })
            .collect())
    }

    fn bridge_notes(
        &self,
        candidate_node_key: &str,
        candidate_explicit_id: Option<&str>,
        neighbor_node_keys: &[String],
        neighbor_explicit_ids: &[String],
    ) -> Result<Vec<BridgeEvidenceRecord>> {
        let outgoing = self.outgoing_bridge_notes(candidate_node_key, neighbor_explicit_ids)?;
        let incoming = self.incoming_bridge_notes(candidate_explicit_id, neighbor_node_keys)?;
        let mut notes = Vec::new();
        extend_unique_bridge_notes(&mut notes, outgoing);
        extend_unique_bridge_notes(&mut notes, incoming);
        Ok(notes)
    }

    fn outgoing_bridge_notes(
        &self,
        candidate_node_key: &str,
        neighbor_explicit_ids: &[String],
    ) -> Result<Vec<BridgeEvidenceRecord>> {
        if neighbor_explicit_ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = numbered_placeholders(2, neighbor_explicit_ids.len());
        let sql = format!(
            "SELECT dest.node_key, dest.explicit_id, dest.title
               FROM links AS l
               JOIN nodes AS dest ON dest.explicit_id = l.destination_explicit_id
              WHERE l.source_node_key = ?1
                AND l.destination_explicit_id IN ({})
                AND {}
              ORDER BY dest.file_path, dest.line, dest.node_key",
            placeholders,
            note_where("dest"),
        );
        let mut values = vec![candidate_node_key.to_owned()];
        values.extend(neighbor_explicit_ids.iter().cloned());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok(BridgeEvidenceRecord {
                node_key: row.get(0)?,
                explicit_id: row.get(1)?,
                title: row.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read outgoing bridge candidates")
    }

    fn incoming_bridge_notes(
        &self,
        candidate_explicit_id: Option<&str>,
        neighbor_node_keys: &[String],
    ) -> Result<Vec<BridgeEvidenceRecord>> {
        let Some(candidate_explicit_id) = candidate_explicit_id else {
            return Ok(Vec::new());
        };
        if neighbor_node_keys.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = numbered_placeholders(2, neighbor_node_keys.len());
        let sql = format!(
            "SELECT src.node_key, src.explicit_id, src.title
               FROM links AS l
               JOIN nodes AS src ON src.node_key = l.source_node_key
              WHERE l.destination_explicit_id = ?1
                AND l.source_node_key IN ({})
                AND {}
              ORDER BY src.file_path, src.line, src.node_key",
            placeholders,
            note_where("src"),
        );
        let mut values = vec![candidate_explicit_id.to_owned()];
        values.extend(neighbor_node_keys.iter().cloned());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok(BridgeEvidenceRecord {
                node_key: row.get(0)?,
                explicit_id: row.get(1)?,
                title: row.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read incoming bridge candidates")
    }
}

fn distinct_planning_dates(anchor: &AnchorRecord) -> Vec<String> {
    let mut dates = BTreeSet::new();
    if let Some(scheduled_for) = anchor.scheduled_for.as_ref() {
        dates.insert(scheduled_for.clone());
    }
    if let Some(deadline_for) = anchor.deadline_for.as_ref() {
        dates.insert(deadline_for.clone());
    }
    dates.into_iter().collect()
}

fn planning_field_value(anchor: &AnchorRecord, field: PlanningField) -> Option<&str> {
    match field {
        PlanningField::Scheduled => anchor.scheduled_for.as_deref(),
        PlanningField::Deadline => anchor.deadline_for.as_deref(),
    }
}

fn planning_relations(
    source: &AnchorRecord,
    candidate: &AnchorRecord,
) -> Vec<PlanningRelationRecord> {
    let mut relations = Vec::new();
    for source_field in [PlanningField::Scheduled, PlanningField::Deadline] {
        let Some(source_date) = planning_field_value(source, source_field) else {
            continue;
        };
        for candidate_field in [PlanningField::Scheduled, PlanningField::Deadline] {
            if planning_field_value(candidate, candidate_field) == Some(source_date) {
                relations.push(PlanningRelationRecord {
                    source_field,
                    candidate_field,
                    date: source_date.to_owned(),
                });
            }
        }
    }
    relations
}

fn planning_field_column(field: PlanningField) -> &'static str {
    match field {
        PlanningField::Scheduled => "scheduled_for",
        PlanningField::Deadline => "deadline_for",
    }
}

fn planning_date_match_predicate_sql(
    node_alias: &str,
    planning_dates: &[String],
    next_parameter_index: usize,
    values: &mut Vec<String>,
) -> (String, usize) {
    let placeholders = numbered_placeholders(next_parameter_index, planning_dates.len());
    values.extend(planning_dates.iter().cloned());
    (
        format!(
            "({node_alias}.scheduled_for IN ({placeholders}) OR {node_alias}.deadline_for IN ({placeholders}))"
        ),
        next_parameter_index + planning_dates.len(),
    )
}

fn planning_relation_count_sql(
    anchor: &AnchorRecord,
    node_alias: &str,
    next_parameter_index: usize,
    values: &mut Vec<String>,
) -> (String, usize) {
    let mut parts = Vec::new();
    let mut parameter_index = next_parameter_index;
    for source_field in [PlanningField::Scheduled, PlanningField::Deadline] {
        let Some(source_date) = planning_field_value(anchor, source_field) else {
            continue;
        };
        for candidate_field in [PlanningField::Scheduled, PlanningField::Deadline] {
            let column = planning_field_column(candidate_field);
            parts.push(format!(
                "CASE WHEN {node_alias}.{column} = ?{parameter_index} THEN 1 ELSE 0 END"
            ));
            values.push(source_date.to_owned());
            parameter_index += 1;
        }
    }
    if parts.is_empty() {
        ("0".to_owned(), parameter_index)
    } else {
        (format!("({})", parts.join(" + ")), parameter_index)
    }
}

fn excluded_keys(note: &NodeRecord, direct_neighbor_keys: &BTreeSet<String>) -> BTreeSet<String> {
    let mut excluded = direct_neighbor_keys.clone();
    excluded.insert(note.node_key.clone());
    excluded
}

fn numbered_placeholders(start: usize, count: usize) -> String {
    (start..start + count)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn widened_limit(limit: usize) -> usize {
    limit
        .clamp(1, 1_000)
        .saturating_mul(RELATED_REF_SCAN_FACTOR)
        .min(1_000)
}

fn extend_unique_bridge_notes(
    target: &mut Vec<BridgeEvidenceRecord>,
    values: Vec<BridgeEvidenceRecord>,
) {
    for value in values {
        if !target
            .iter()
            .any(|existing| existing.node_key == value.node_key)
        {
            target.push(value);
        }
    }
}

fn structural_link_count(anchor: &AnchorRecord) -> u64 {
    anchor.backlink_count + anchor.forward_link_count
}

fn compare_anchor_records(left: &AnchorRecord, right: &AnchorRecord) -> Ordering {
    left.file_path
        .cmp(&right.file_path)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.node_key.cmp(&right.node_key))
}

fn compare_shared_reference_support(
    left: &SharedRefCandidate,
    right: &SharedRefCandidate,
) -> Ordering {
    right
        .shared_reference_count()
        .cmp(&left.shared_reference_count())
}

fn compare_shared_ref_candidates_by_evidence(
    left: &SharedRefCandidate,
    right: &SharedRefCandidate,
) -> Ordering {
    compare_shared_reference_support(left, right)
        .then_with(|| compare_anchor_records(&left.anchor, &right.anchor))
}

fn compare_bridge_candidates(left: &BridgeCandidate, right: &BridgeCandidate) -> Ordering {
    right
        .bridge_count()
        .cmp(&left.bridge_count())
        .then_with(|| compare_shared_reference_support(&left.candidate, &right.candidate))
        .then_with(|| compare_anchor_records(&left.candidate.anchor, &right.candidate.anchor))
}

fn compare_dormant_candidates(left: &SharedRefCandidate, right: &SharedRefCandidate) -> Ordering {
    compare_shared_reference_support(left, right)
        .then_with(|| left.anchor.file_mtime_ns.cmp(&right.anchor.file_mtime_ns))
        .then_with(|| compare_anchor_records(&left.anchor, &right.anchor))
}

fn compare_unresolved_candidates(
    left: &SharedRefCandidate,
    right: &SharedRefCandidate,
) -> Ordering {
    compare_shared_ref_candidates_by_evidence(left, right)
}

fn compare_weakly_integrated_candidates(
    left: &SharedRefCandidate,
    right: &SharedRefCandidate,
) -> Ordering {
    structural_link_count(&left.anchor)
        .cmp(&structural_link_count(&right.anchor))
        .then_with(|| compare_shared_ref_candidates_by_evidence(left, right))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    use anyhow::{Context, Result};
    use slipbox_core::{
        AnchorRecord, ExplorationExplanation, PlanningField, PlanningRelationRecord,
    };
    use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};

    use crate::Database;

    #[test]
    fn non_obvious_queries_stay_explainable_and_note_scoped() -> Result<()> {
        let workspace = tempfile::tempdir().context("workspace should be created")?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).context("notes root should be created")?;
        fs::write(
            root.join("older.org"),
            r#"#+title: Older

* Dormant Bridge
:PROPERTIES:
:ID: dormant-bridge-id
:ROAM_REFS: cite:shared2024
:END:
Links to [[id:neighbor-id]] and [[id:support-id]].

* Support
:PROPERTIES:
:ID: support-id
:END:
Support body.
"#,
        )
        .context("older fixture should be written")?;
        sleep(Duration::from_millis(10));
        fs::write(
            root.join("focus.org"),
            r#"#+title: Focus

* Focus
:PROPERTIES:
:ID: focus-id
:ROAM_REFS: cite:shared2024 cite:focus2024
:END:
Links to [[id:neighbor-id]].

* Neighbor
:PROPERTIES:
:ID: neighbor-id
:END:
Neighbor body.
"#,
        )
        .context("focus fixture should be written")?;
        sleep(Duration::from_millis(10));
        fs::write(
            root.join("related.org"),
            r#"#+title: Related

* TODO Unresolved Thread
:PROPERTIES:
:ID: unresolved-id
:ROAM_REFS: cite:shared2024
:END:
Unresolved body.

* Weak Thread
:PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:shared2024
:END:
Weakly integrated body.
"#,
        )
        .context("related fixture should be written")?;

        let mut database = Database::open(&workspace.path().join("index.sqlite3"))?;
        let files =
            scan_root_with_policy(&root, &DiscoveryPolicy::default()).context("fixture scan")?;
        database.sync_index(&files).context("fixture index sync")?;
        let focus = database
            .node_from_id("focus-id")?
            .context("focus note should exist")?;

        let bridges = database.bridge_candidates(&focus, 20)?;
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].anchor.title, "Dormant Bridge");
        assert!(matches!(
            bridges[0].explanation,
            ExplorationExplanation::BridgeCandidate { ref references, ref via_notes }
            if references == &vec!["@shared2024".to_owned()]
                && via_notes.len() == 1
                && via_notes[0].title == "Neighbor"
                && via_notes[0].explicit_id.as_deref() == Some("neighbor-id")
        ));

        let dormant = database.dormant_related(&focus, 20)?;
        assert_eq!(dormant.len(), 1);
        assert_eq!(dormant[0].anchor.title, "Dormant Bridge");
        assert!(matches!(
            dormant[0].explanation,
            ExplorationExplanation::DormantSharedReference {
                ref references,
                modified_at_ns,
            } if references == &vec!["@shared2024".to_owned()]
                && modified_at_ns < focus.file_mtime_ns
        ));

        let unresolved = database.unresolved_tasks(&focus, 20)?;
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].anchor.title, "Unresolved Thread");
        assert_eq!(
            unresolved[0].explanation,
            ExplorationExplanation::UnresolvedSharedReference {
                references: vec!["@shared2024".to_owned()],
                todo_keyword: "TODO".to_owned(),
            }
        );

        let weakly_integrated = database.weakly_integrated_notes(&focus, 20)?;
        assert_eq!(weakly_integrated.len(), 1);
        assert_eq!(weakly_integrated[0].anchor.title, "Weak Thread");
        assert_eq!(
            weakly_integrated[0].explanation,
            ExplorationExplanation::WeaklyIntegratedSharedReference {
                references: vec!["@shared2024".to_owned()],
                structural_link_count: 0,
            }
        );

        Ok(())
    }

    #[test]
    fn non_obvious_queries_rank_by_supporting_evidence() -> Result<()> {
        let workspace = tempfile::tempdir().context("workspace should be created")?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).context("notes root should be created")?;
        fs::write(
            root.join("older.org"),
            r#"#+title: Older

* Bridge Rich
:PROPERTIES:
:ID: bridge-rich-id
:ROAM_REFS: cite:alpha2024 cite:beta2024
:END:
Links to [[id:neighbor-id]] and [[id:support-id]].

* Bridge Sparse
:PROPERTIES:
:ID: bridge-sparse-id
:ROAM_REFS: cite:alpha2024
:END:
Links to [[id:neighbor-id]].

* Support
:PROPERTIES:
:ID: support-id
:END:
Support body.
"#,
        )
        .context("older fixture should be written")?;
        sleep(Duration::from_millis(10));
        fs::write(
            root.join("focus.org"),
            r#"#+title: Focus

* Focus
:PROPERTIES:
:ID: focus-id
:ROAM_REFS: cite:alpha2024 cite:beta2024
:END:
Links to [[id:neighbor-id]] and [[id:support-id]].

* Neighbor
:PROPERTIES:
:ID: neighbor-id
:END:
Neighbor body.
"#,
        )
        .context("focus fixture should be written")?;
        sleep(Duration::from_millis(10));
        fs::write(
            root.join("related.org"),
            r#"#+title: Related

* TODO Ranked Unresolved
:PROPERTIES:
:ID: unresolved-rich-id
:ROAM_REFS: cite:alpha2024 cite:beta2024
:END:
Needs more synthesis.

* TODO Sparse Unresolved
:PROPERTIES:
:ID: unresolved-sparse-id
:ROAM_REFS: cite:alpha2024
:END:
Needs more synthesis.

* Rich Weak
:PROPERTIES:
:ID: weak-rich-id
:ROAM_REFS: cite:alpha2024 cite:beta2024
:END:
Weakly integrated body.

* Sparse Weak
:PROPERTIES:
:ID: weak-sparse-id
:ROAM_REFS: cite:alpha2024
:END:
Weakly integrated body.
"#,
        )
        .context("related fixture should be written")?;

        let mut database = Database::open(&workspace.path().join("index.sqlite3"))?;
        let files =
            scan_root_with_policy(&root, &DiscoveryPolicy::default()).context("fixture scan")?;
        database.sync_index(&files).context("fixture index sync")?;
        let focus = database
            .node_from_id("focus-id")?
            .context("focus note should exist")?;

        let bridges = database.bridge_candidates(&focus, 20)?;
        assert_eq!(
            bridges
                .iter()
                .map(|record| record.anchor.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Bridge Rich", "Bridge Sparse"]
        );
        assert!(matches!(
            bridges[0].explanation,
            ExplorationExplanation::BridgeCandidate { ref references, ref via_notes }
            if references == &vec!["@alpha2024".to_owned(), "@beta2024".to_owned()]
                && via_notes
                    .iter()
                    .map(|note| note.title.as_str())
                    .collect::<Vec<_>>()
                    == vec!["Neighbor", "Support"]
        ));

        let dormant = database.dormant_related(&focus, 20)?;
        assert_eq!(
            dormant
                .iter()
                .map(|record| record.anchor.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Bridge Rich", "Bridge Sparse"]
        );

        let unresolved = database.unresolved_tasks(&focus, 20)?;
        assert_eq!(
            unresolved
                .iter()
                .map(|record| record.anchor.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Ranked Unresolved", "Sparse Unresolved"]
        );

        let weakly_integrated = database.weakly_integrated_notes(&focus, 20)?;
        let weak_titles = weakly_integrated
            .iter()
            .map(|record| record.anchor.title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(&weak_titles[..2], &["Rich Weak", "Sparse Weak"]);

        Ok(())
    }

    #[test]
    fn time_and_task_neighbors_use_explicit_planning_relations() -> Result<()> {
        let workspace = tempfile::tempdir().context("workspace should be created")?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).context("notes root should be created")?;
        fs::write(
            root.join("planning.org"),
            r#"#+title: Planning

* TODO Focus
:PROPERTIES:
:ID: focus-id
:END:
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Focus body.

* TODO Dual Match Peer
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Matches both planning fields directly.

* NEXT Cross Match Peer
SCHEDULED: <2026-05-03 Sat>
DEADLINE: <2026-05-01 Thu>
Matches both planning dates through opposite fields.

* TODO Keyword Only Peer
Shares only the same task state.

* WAIT Deadline Peer
DEADLINE: <2026-05-03 Sat>
Shares only the focus deadline.
"#,
        )
        .context("planning fixture should be written")?;

        let mut database = Database::open(&workspace.path().join("index.sqlite3"))?;
        let files =
            scan_root_with_policy(&root, &DiscoveryPolicy::default()).context("fixture scan")?;
        database.sync_index(&files).context("fixture index sync")?;
        let focus = database
            .node_from_id("focus-id")?
            .context("focus note should exist")?;
        let focus_anchor: AnchorRecord = focus.clone().into();

        let time_neighbors = database.time_neighbors(&focus_anchor, 20)?;
        assert_eq!(
            time_neighbors
                .iter()
                .map(|record| record.anchor.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Dual Match Peer", "Cross Match Peer", "WAIT Deadline Peer",]
        );
        assert_eq!(
            time_neighbors[1].explanation,
            ExplorationExplanation::TimeNeighbor {
                relations: vec![
                    PlanningRelationRecord {
                        source_field: PlanningField::Scheduled,
                        candidate_field: PlanningField::Deadline,
                        date: "2026-05-01T00:00:00".to_owned(),
                    },
                    PlanningRelationRecord {
                        source_field: PlanningField::Deadline,
                        candidate_field: PlanningField::Scheduled,
                        date: "2026-05-03T00:00:00".to_owned(),
                    },
                ],
            }
        );

        let task_neighbors = database.task_neighbors(&focus_anchor, 20)?;
        assert_eq!(
            task_neighbors
                .iter()
                .map(|record| record.anchor.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Dual Match Peer", "Cross Match Peer", "Keyword Only Peer",]
        );
        assert_eq!(
            task_neighbors[0].explanation,
            ExplorationExplanation::TaskNeighbor {
                shared_todo_keyword: Some("TODO".to_owned()),
                planning_relations: vec![
                    PlanningRelationRecord {
                        source_field: PlanningField::Scheduled,
                        candidate_field: PlanningField::Scheduled,
                        date: "2026-05-01T00:00:00".to_owned(),
                    },
                    PlanningRelationRecord {
                        source_field: PlanningField::Deadline,
                        candidate_field: PlanningField::Deadline,
                        date: "2026-05-03T00:00:00".to_owned(),
                    },
                ],
            }
        );
        assert_eq!(
            task_neighbors[1].explanation,
            ExplorationExplanation::TaskNeighbor {
                shared_todo_keyword: None,
                planning_relations: vec![
                    PlanningRelationRecord {
                        source_field: PlanningField::Scheduled,
                        candidate_field: PlanningField::Deadline,
                        date: "2026-05-01T00:00:00".to_owned(),
                    },
                    PlanningRelationRecord {
                        source_field: PlanningField::Deadline,
                        candidate_field: PlanningField::Scheduled,
                        date: "2026-05-03T00:00:00".to_owned(),
                    },
                ],
            }
        );
        assert_eq!(
            task_neighbors[2].explanation,
            ExplorationExplanation::TaskNeighbor {
                shared_todo_keyword: Some("TODO".to_owned()),
                planning_relations: Vec::new(),
            }
        );

        Ok(())
    }

    #[test]
    fn time_and_task_neighbors_rank_full_candidate_sets_by_evidence() -> Result<()> {
        let workspace = tempfile::tempdir().context("workspace should be created")?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).context("notes root should be created")?;
        fs::write(
            root.join("focus.org"),
            r#"* TODO Focus
:PROPERTIES:
:ID: focus-id
:END:
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Focus body.
"#,
        )
        .context("focus fixture should be written")?;
        for (path, contents) in [
            (
                "a-time-1.org",
                "* Time Weak 1\nSCHEDULED: <2026-05-01 Thu>\nWeak time evidence.\n",
            ),
            (
                "a-time-2.org",
                "* Time Weak 2\nDEADLINE: <2026-05-01 Thu>\nWeak time evidence.\n",
            ),
            (
                "a-time-3.org",
                "* Time Weak 3\nSCHEDULED: <2026-05-03 Sat>\nWeak time evidence.\n",
            ),
            (
                "a-time-4.org",
                "* Time Weak 4\nDEADLINE: <2026-05-03 Sat>\nWeak time evidence.\n",
            ),
            (
                "a-time-5.org",
                "* Time Weak 5\nSCHEDULED: <2026-05-01 Thu>\nWeak time evidence.\n",
            ),
            (
                "b-task-1.org",
                "* TODO Task Weak 1\nWeak task state only.\n",
            ),
            (
                "b-task-2.org",
                "* TODO Task Weak 2\nWeak task state only.\n",
            ),
            (
                "b-task-3.org",
                "* TODO Task Weak 3\nWeak task state only.\n",
            ),
            (
                "b-task-4.org",
                "* TODO Task Weak 4\nWeak task state only.\n",
            ),
            (
                "b-task-5.org",
                "* TODO Task Weak 5\nWeak task state only.\n",
            ),
            (
                "z-task-strong.org",
                "* TODO Task Strong\nSCHEDULED: <2026-05-01 Thu>\nStronger task evidence.\n",
            ),
            (
                "z-time-strong.org",
                "* Time Strong\nSCHEDULED: <2026-05-01 Thu>\nDEADLINE: <2026-05-03 Sat>\nStronger time evidence.\n",
            ),
        ] {
            fs::write(root.join(path), contents)
                .with_context(|| format!("fixture {path} should be written"))?;
        }

        let mut database = Database::open(&workspace.path().join("index.sqlite3"))?;
        let files =
            scan_root_with_policy(&root, &DiscoveryPolicy::default()).context("fixture scan")?;
        database.sync_index(&files).context("fixture index sync")?;
        let focus = database
            .node_from_id("focus-id")?
            .context("focus note should exist")?;
        let focus_anchor: AnchorRecord = focus.into();

        let time_neighbors = database.time_neighbors(&focus_anchor, 1)?;
        assert_eq!(time_neighbors.len(), 1);
        assert_eq!(time_neighbors[0].anchor.title, "Time Strong");
        assert_eq!(
            time_neighbors[0].explanation,
            ExplorationExplanation::TimeNeighbor {
                relations: vec![
                    PlanningRelationRecord {
                        source_field: PlanningField::Scheduled,
                        candidate_field: PlanningField::Scheduled,
                        date: "2026-05-01T00:00:00".to_owned(),
                    },
                    PlanningRelationRecord {
                        source_field: PlanningField::Deadline,
                        candidate_field: PlanningField::Deadline,
                        date: "2026-05-03T00:00:00".to_owned(),
                    },
                ],
            }
        );

        let task_neighbors = database.task_neighbors(&focus_anchor, 1)?;
        assert_eq!(task_neighbors.len(), 1);
        assert_eq!(task_neighbors[0].anchor.title, "Task Strong");
        assert_eq!(
            task_neighbors[0].explanation,
            ExplorationExplanation::TaskNeighbor {
                shared_todo_keyword: Some("TODO".to_owned()),
                planning_relations: vec![PlanningRelationRecord {
                    source_field: PlanningField::Scheduled,
                    candidate_field: PlanningField::Scheduled,
                    date: "2026-05-01T00:00:00".to_owned(),
                }],
            }
        );

        Ok(())
    }

    #[test]
    fn dormant_candidates_break_same_support_ties_by_age() -> Result<()> {
        let workspace = tempfile::tempdir().context("workspace should be created")?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).context("notes root should be created")?;
        fs::write(
            root.join("older.org"),
            r#"#+title: Older

* Older Dormant
:PROPERTIES:
:ID: older-dormant-id
:ROAM_REFS: cite:shared2024
:END:
Links to [[id:neighbor-id]].
"#,
        )
        .context("older fixture should be written")?;
        sleep(Duration::from_millis(10));
        fs::write(
            root.join("newer.org"),
            r#"#+title: Newer

* Newer Dormant
:PROPERTIES:
:ID: newer-dormant-id
:ROAM_REFS: cite:shared2024
:END:
Links to [[id:neighbor-id]].
"#,
        )
        .context("newer fixture should be written")?;
        sleep(Duration::from_millis(10));
        fs::write(
            root.join("focus.org"),
            r#"#+title: Focus

* Focus
:PROPERTIES:
:ID: focus-id
:ROAM_REFS: cite:shared2024
:END:
Links to [[id:neighbor-id]].

* Neighbor
:PROPERTIES:
:ID: neighbor-id
:END:
Neighbor body.
"#,
        )
        .context("focus fixture should be written")?;

        let mut database = Database::open(&workspace.path().join("index.sqlite3"))?;
        let files =
            scan_root_with_policy(&root, &DiscoveryPolicy::default()).context("fixture scan")?;
        database.sync_index(&files).context("fixture index sync")?;
        let focus = database
            .node_from_id("focus-id")?
            .context("focus note should exist")?;

        let dormant = database.dormant_related(&focus, 20)?;
        assert_eq!(
            dormant
                .iter()
                .map(|record| record.anchor.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Older Dormant", "Newer Dormant"]
        );

        Ok(())
    }

    #[test]
    fn bridge_candidates_preserve_distinct_same_title_bridge_notes() -> Result<()> {
        let workspace = tempfile::tempdir().context("workspace should be created")?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).context("notes root should be created")?;
        fs::write(
            root.join("focus.org"),
            r#"#+title: Focus

* Focus
:PROPERTIES:
:ID: focus-id
:ROAM_REFS: cite:shared2024
:END:
Links to [[id:neighbor-a-id]] and [[id:neighbor-b-id]].

* Bridge
:PROPERTIES:
:ID: neighbor-a-id
:END:
First bridge note.

* Bridge
:PROPERTIES:
:ID: neighbor-b-id
:END:
Second bridge note.
"#,
        )
        .context("focus fixture should be written")?;
        fs::write(
            root.join("candidate.org"),
            r#"#+title: Candidate

* Candidate
:PROPERTIES:
:ID: candidate-id
:ROAM_REFS: cite:shared2024
:END:
Links to [[id:neighbor-a-id]] and [[id:neighbor-b-id]].
"#,
        )
        .context("candidate fixture should be written")?;

        let mut database = Database::open(&workspace.path().join("index.sqlite3"))?;
        let files =
            scan_root_with_policy(&root, &DiscoveryPolicy::default()).context("fixture scan")?;
        database.sync_index(&files).context("fixture index sync")?;
        let focus = database
            .node_from_id("focus-id")?
            .context("focus note should exist")?;

        let bridges = database.bridge_candidates(&focus, 20)?;
        assert_eq!(bridges.len(), 1);
        assert!(matches!(
            bridges[0].explanation,
            ExplorationExplanation::BridgeCandidate { ref references, ref via_notes }
            if references == &vec!["@shared2024".to_owned()]
                && via_notes.len() == 2
                && via_notes.iter().all(|note| note.title == "Bridge")
                && via_notes[0].node_key != via_notes[1].node_key
        ));

        Ok(())
    }
}
