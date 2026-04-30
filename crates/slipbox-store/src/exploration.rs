use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params, params_from_iter};

use slipbox_core::{AnchorExplorationRecord, AnchorRecord, ExplorationExplanation, NodeRecord};

use crate::Database;
use crate::nodes::{
    ANCHOR_SELECT_COLUMN_COUNT, anchor_select_columns, note_where, row_to_anchor,
    row_to_anchor_with_offset,
};

const EXPLORATION_NEIGHBOR_LIMIT: usize = 1_000;
const RELATED_REF_SCAN_FACTOR: usize = 5;

struct SharedRefCandidate {
    anchor: AnchorRecord,
    reference: String,
}

impl Database {
    pub fn time_neighbors(
        &self,
        anchor: &AnchorRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        let scheduled_for = anchor.scheduled_for.as_deref();
        let deadline_for = anchor.deadline_for.as_deref();
        if scheduled_for.is_none() && deadline_for.is_none() {
            return Ok(Vec::new());
        }

        let sql = format!(
            "SELECT {},
                    CASE
                        WHEN ?2 IS NOT NULL AND n.scheduled_for = ?2 THEN 'scheduled'
                        WHEN ?3 IS NOT NULL AND n.deadline_for = ?3 THEN 'deadline'
                    END AS match_kind,
                    CASE
                        WHEN ?2 IS NOT NULL AND n.scheduled_for = ?2 THEN n.scheduled_for
                        WHEN ?3 IS NOT NULL AND n.deadline_for = ?3 THEN n.deadline_for
                    END AS match_date
               FROM nodes AS n
              WHERE n.node_key <> ?1
                AND ((?2 IS NOT NULL AND n.scheduled_for = ?2)
                  OR (?3 IS NOT NULL AND n.deadline_for = ?3))
              ORDER BY COALESCE(n.scheduled_for, n.deadline_for), n.file_path, n.line
              LIMIT ?4",
            anchor_select_columns("n")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(
            params![
                anchor.node_key,
                scheduled_for,
                deadline_for,
                limit.clamp(1, 1_000) as i64
            ],
            row_to_time_neighbor,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read time neighbors")
    }

    pub fn task_neighbors(
        &self,
        anchor: &AnchorRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        let Some(todo_keyword) = anchor.todo_keyword.as_deref() else {
            return Ok(Vec::new());
        };

        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE n.todo_keyword = ?1
                AND n.node_key <> ?2
              ORDER BY n.file_path, n.line
              LIMIT ?3",
            anchor_select_columns("n")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(
            params![todo_keyword, anchor.node_key, limit.clamp(1, 1_000) as i64],
            |row| {
                Ok(AnchorExplorationRecord {
                    anchor: row_to_anchor(row)?,
                    explanation: ExplorationExplanation::SharedTodoKeyword {
                        todo_keyword: todo_keyword.to_owned(),
                    },
                })
            },
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read task neighbors")
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

        let mut results = Vec::new();
        for candidate in candidates {
            if let Some(via_title) = self.first_bridge_title(
                candidate.anchor.node_key.as_str(),
                candidate.anchor.explicit_id.as_deref(),
                &direct_neighbor_key_list,
                &direct_neighbor_explicit_ids,
            )? {
                results.push(AnchorExplorationRecord {
                    anchor: candidate.anchor,
                    explanation: ExplorationExplanation::BridgeCandidate {
                        reference: candidate.reference,
                        via_title,
                    },
                });
            }
            if results.len() == limit.clamp(1, 1_000) {
                break;
            }
        }

        Ok(results)
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
                    reference: candidate.reference,
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
        let candidates = self.shared_ref_candidates(
            note,
            &excluded_keys(note, &direct_neighbor_keys),
            widened_limit(limit),
        )?;

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
                            reference: candidate.reference,
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
                    reference: candidate.reference,
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
                    matches.shared_ref
               FROM (
                     SELECT n.node_key AS candidate_node_key,
                            MIN(r.ref) AS shared_ref
                       FROM refs AS r
                       JOIN nodes AS n ON n.node_key = r.node_key
                      WHERE r.ref IN ({})
                        AND r.node_key NOT IN ({})
                        AND {}
                      GROUP BY n.node_key
                    ) AS matches
               JOIN nodes AS n ON n.node_key = matches.candidate_node_key
              ORDER BY n.file_path, n.line
              LIMIT ?{}",
            anchor_select_columns("n"),
            ref_placeholders,
            excluded_placeholders,
            note_where("n"),
            limit_placeholder,
        );
        let mut values = references;
        values.extend(excluded);
        values.push(limit.clamp(1, 1_000).to_string());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok(SharedRefCandidate {
                anchor: row_to_anchor_with_offset(row, 0)?,
                reference: row.get(ANCHOR_SELECT_COLUMN_COUNT)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read shared-ref exploration candidates")
    }

    fn first_bridge_title(
        &self,
        candidate_node_key: &str,
        candidate_explicit_id: Option<&str>,
        neighbor_node_keys: &[String],
        neighbor_explicit_ids: &[String],
    ) -> Result<Option<String>> {
        let outgoing =
            self.first_outgoing_bridge_title(candidate_node_key, neighbor_explicit_ids)?;
        let incoming =
            self.first_incoming_bridge_title(candidate_explicit_id, neighbor_node_keys)?;
        Ok(match (outgoing, incoming) {
            (Some(outgoing), Some(incoming)) => Some(if outgoing <= incoming {
                outgoing
            } else {
                incoming
            }),
            (Some(outgoing), None) => Some(outgoing),
            (None, Some(incoming)) => Some(incoming),
            (None, None) => None,
        })
    }

    fn first_outgoing_bridge_title(
        &self,
        candidate_node_key: &str,
        neighbor_explicit_ids: &[String],
    ) -> Result<Option<String>> {
        if neighbor_explicit_ids.is_empty() {
            return Ok(None);
        }

        let placeholders = numbered_placeholders(2, neighbor_explicit_ids.len());
        let sql = format!(
            "SELECT dest.title
               FROM links AS l
               JOIN nodes AS dest ON dest.explicit_id = l.destination_explicit_id
              WHERE l.source_node_key = ?1
                AND l.destination_explicit_id IN ({})
                AND {}
              ORDER BY dest.file_path, dest.line
              LIMIT 1",
            placeholders,
            note_where("dest"),
        );
        let mut values = vec![candidate_node_key.to_owned()];
        values.extend(neighbor_explicit_ids.iter().cloned());
        let mut statement = self.connection.prepare(&sql)?;
        statement
            .query_row(params_from_iter(values.iter()), |row| row.get(0))
            .optional()
            .context("failed to read outgoing bridge candidate")
    }

    fn first_incoming_bridge_title(
        &self,
        candidate_explicit_id: Option<&str>,
        neighbor_node_keys: &[String],
    ) -> Result<Option<String>> {
        let Some(candidate_explicit_id) = candidate_explicit_id else {
            return Ok(None);
        };
        if neighbor_node_keys.is_empty() {
            return Ok(None);
        }

        let placeholders = numbered_placeholders(2, neighbor_node_keys.len());
        let sql = format!(
            "SELECT src.title
               FROM links AS l
               JOIN nodes AS src ON src.node_key = l.source_node_key
              WHERE l.destination_explicit_id = ?1
                AND l.source_node_key IN ({})
                AND {}
              ORDER BY src.file_path, src.line
              LIMIT 1",
            placeholders,
            note_where("src"),
        );
        let mut values = vec![candidate_explicit_id.to_owned()];
        values.extend(neighbor_node_keys.iter().cloned());
        let mut statement = self.connection.prepare(&sql)?;
        statement
            .query_row(params_from_iter(values.iter()), |row| row.get(0))
            .optional()
            .context("failed to read incoming bridge candidate")
    }
}

fn row_to_time_neighbor(row: &rusqlite::Row<'_>) -> rusqlite::Result<AnchorExplorationRecord> {
    let match_kind: String = row.get(ANCHOR_SELECT_COLUMN_COUNT)?;
    let match_date: String = row.get(ANCHOR_SELECT_COLUMN_COUNT + 1)?;
    let explanation = match match_kind.as_str() {
        "scheduled" => ExplorationExplanation::SharedScheduledDate { date: match_date },
        "deadline" => ExplorationExplanation::SharedDeadlineDate { date: match_date },
        other => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                ANCHOR_SELECT_COLUMN_COUNT,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(format!(
                    "unexpected time-neighbor match kind {other}"
                ))),
            ));
        }
    };
    Ok(AnchorExplorationRecord {
        anchor: row_to_anchor(row)?,
        explanation,
    })
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

fn structural_link_count(anchor: &AnchorRecord) -> u64 {
    anchor.backlink_count + anchor.forward_link_count
}

fn compare_anchor_records(left: &AnchorRecord, right: &AnchorRecord) -> Ordering {
    left.file_path
        .cmp(&right.file_path)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.node_key.cmp(&right.node_key))
}

fn compare_dormant_candidates(left: &SharedRefCandidate, right: &SharedRefCandidate) -> Ordering {
    left.anchor
        .file_mtime_ns
        .cmp(&right.anchor.file_mtime_ns)
        .then_with(|| compare_anchor_records(&left.anchor, &right.anchor))
}

fn compare_weakly_integrated_candidates(
    left: &SharedRefCandidate,
    right: &SharedRefCandidate,
) -> Ordering {
    structural_link_count(&left.anchor)
        .cmp(&structural_link_count(&right.anchor))
        .then_with(|| compare_anchor_records(&left.anchor, &right.anchor))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    use anyhow::{Context, Result};
    use slipbox_core::ExplorationExplanation;
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
        assert_eq!(
            bridges[0].explanation,
            ExplorationExplanation::BridgeCandidate {
                reference: "@shared2024".to_owned(),
                via_title: "Neighbor".to_owned(),
            }
        );

        let dormant = database.dormant_related(&focus, 20)?;
        assert_eq!(dormant.len(), 1);
        assert_eq!(dormant[0].anchor.title, "Dormant Bridge");
        assert!(matches!(
            dormant[0].explanation,
            ExplorationExplanation::DormantSharedReference {
                ref reference,
                modified_at_ns,
            } if reference == "@shared2024" && modified_at_ns < focus.file_mtime_ns
        ));

        let unresolved = database.unresolved_tasks(&focus, 20)?;
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].anchor.title, "Unresolved Thread");
        assert_eq!(
            unresolved[0].explanation,
            ExplorationExplanation::UnresolvedSharedReference {
                reference: "@shared2024".to_owned(),
                todo_keyword: "TODO".to_owned(),
            }
        );

        let weakly_integrated = database.weakly_integrated_notes(&focus, 20)?;
        assert_eq!(weakly_integrated.len(), 1);
        assert_eq!(weakly_integrated[0].anchor.title, "Weak Thread");
        assert_eq!(
            weakly_integrated[0].explanation,
            ExplorationExplanation::WeaklyIntegratedSharedReference {
                reference: "@shared2024".to_owned(),
                structural_link_count: 0,
            }
        );

        Ok(())
    }
}
