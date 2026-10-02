use anyhow::{Context, Result};
use rusqlite::{params, params_from_iter};

use slipbox_core::{DirectedRelationDirection, DirectedRelationRecord, NodeRecord};

use crate::Database;
use crate::nodes::{
    ANCHOR_SELECT_COLUMN_COUNT, POSITION_SEPARATOR, anchor_select_columns, decode_field,
    encode_field, note_where, row_to_note_with_offset,
};

const POSITION_TAG: &str = "directed-relation";

/// Opaque position in a note's filing-ordered relation inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectedRelationPosition {
    file_path: String,
    line: u32,
    node_key: String,
}

impl DirectedRelationPosition {
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        let mut fields = token.split(POSITION_SEPARATOR);
        if decode_field(fields.next()?)? != POSITION_TAG {
            return None;
        }
        let line = decode_field(fields.next()?)?.parse().ok()?;
        let file_path = decode_field(fields.next()?)?;
        let node_key = decode_field(fields.next()?)?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self {
            file_path,
            line,
            node_key,
        })
    }

    fn after(record: &DirectedRelationRecord) -> Self {
        Self {
            file_path: record.note.file_path.clone(),
            line: record.note.line,
            node_key: record.note.node_key.clone(),
        }
    }

    fn token(&self) -> String {
        [
            encode_field(POSITION_TAG),
            encode_field(&self.line.to_string()),
            encode_field(&self.file_path),
            encode_field(&self.node_key),
        ]
        .join(&POSITION_SEPARATOR.to_string())
    }
}

pub struct DirectedRelationPage {
    pub relations: Vec<DirectedRelationRecord>,
    pub total: u64,
    pub incoming_total: u64,
    pub outgoing_total: u64,
    pub has_more: bool,
    pub next_position: Option<String>,
}

impl DirectedRelationPage {
    fn paged(
        mut relations: Vec<DirectedRelationRecord>,
        counts: DirectedRelationCounts,
        limit: usize,
    ) -> Self {
        let has_more = relations.len() > limit;
        relations.truncate(limit);
        let next_position = if has_more {
            relations
                .last()
                .map(DirectedRelationPosition::after)
                .map(|position| position.token())
        } else {
            None
        };
        Self {
            relations,
            total: counts.total,
            incoming_total: counts.incoming,
            outgoing_total: counts.outgoing,
            has_more,
            next_position,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DirectedRelationCounts {
    total: u64,
    incoming: u64,
    outgoing: u64,
}

impl Database {
    /// Return one keyset page of distinct incoming, outgoing or bidirectional notes.
    pub fn directed_relations(
        &self,
        focus: &NodeRecord,
        limit: usize,
        after: Option<&DirectedRelationPosition>,
    ) -> Result<DirectedRelationPage> {
        let limit = limit.clamp(1, 1_000);
        let counts = self.directed_relation_counts(focus)?;
        let mut arguments: Vec<rusqlite::types::Value> = vec![
            focus.node_key.clone().into(),
            focus.explicit_id.clone().into(),
            (limit as i64 + 1).into(),
        ];
        let seek = match after {
            None => String::new(),
            Some(position) => {
                arguments.push(position.file_path.clone().into());
                arguments.push(i64::from(position.line).into());
                arguments.push(position.node_key.clone().into());
                "AND (n.file_path, n.line, n.node_key) > (?4, ?5, ?6)".to_owned()
            }
        };
        let sql = directed_relations_sql(&seek);
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(arguments), row_to_directed_relation)?;
        let relations = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read directed relations")?;
        Ok(DirectedRelationPage::paged(relations, counts, limit))
    }

    fn directed_relation_counts(&self, focus: &NodeRecord) -> Result<DirectedRelationCounts> {
        self.connection
            .query_row(
                &format!(
                    "WITH {}
                     SELECT COUNT(*),
                            COALESCE(SUM(incoming), 0),
                            COALESCE(SUM(outgoing), 0)
                       FROM directions",
                    directed_relation_ctes(),
                ),
                params![focus.node_key, focus.explicit_id],
                |row| {
                    Ok(DirectedRelationCounts {
                        total: row.get(0)?,
                        incoming: row.get(1)?,
                        outgoing: row.get(2)?,
                    })
                },
            )
            .context("failed to count directed relations")
    }
}

fn directed_relations_sql(seek: &str) -> String {
    format!(
        "WITH {}
         SELECT {},
                d.incoming,
                d.outgoing,
                COALESCE(
                    NULLIF(TRIM((
                        SELECT SUBSTR(content.body, 1, 240)
                          FROM node_content_fts AS content
                         WHERE content.rowid = n.id
                    )), ''),
                    CASE WHEN d.incoming = 1 THEN (
                        SELECT SUBSTR(inbound.preview, 1, 240)
                          FROM links AS inbound
                         WHERE inbound.source_note_key = d.related_node_key
                           AND inbound.destination_explicit_id = ?2
                         ORDER BY inbound.source_file_path,
                                  inbound.line,
                                  inbound.column,
                                  inbound.source_node_key
                         LIMIT 1
                    ) ELSE (
                        SELECT SUBSTR(outbound.preview, 1, 240)
                          FROM links AS outbound
                          JOIN nodes AS destination
                            ON destination.explicit_id = outbound.destination_explicit_id
                         WHERE outbound.source_note_key = ?1
                           AND destination.node_key = d.related_node_key
                         ORDER BY outbound.line,
                                  outbound.column,
                                  outbound.destination_explicit_id
                         LIMIT 1
                    ) END,
                    ''
                ) AS preview
           FROM directions AS d
           JOIN nodes AS n ON n.node_key = d.related_node_key
          WHERE 1 = 1
            {seek}
          ORDER BY n.file_path, n.line, n.node_key
          LIMIT ?3",
        directed_relation_ctes(),
        anchor_select_columns("n"),
    )
}

fn directed_relation_ctes() -> String {
    format!(
        "relation_hits AS (
             SELECT destination.node_key AS related_node_key,
                    0 AS incoming,
                    1 AS outgoing
               FROM links AS link
               JOIN nodes AS destination
                 ON destination.explicit_id = link.destination_explicit_id
              WHERE link.source_note_key = ?1
                AND {}
             UNION ALL
             SELECT source.node_key AS related_node_key,
                    1 AS incoming,
                    0 AS outgoing
               FROM links AS link
               JOIN nodes AS source ON source.node_key = link.source_note_key
              WHERE ?2 IS NOT NULL
                AND link.destination_explicit_id = ?2
                AND {}
         ),
         directions AS (
             SELECT related_node_key,
                    MAX(incoming) AS incoming,
                    MAX(outgoing) AS outgoing
               FROM relation_hits
              GROUP BY related_node_key
         )",
        note_where("destination"),
        note_where("source"),
    )
}

fn row_to_directed_relation(row: &rusqlite::Row<'_>) -> rusqlite::Result<DirectedRelationRecord> {
    let incoming = row.get::<_, bool>(ANCHOR_SELECT_COLUMN_COUNT)?;
    let outgoing = row.get::<_, bool>(ANCHOR_SELECT_COLUMN_COUNT + 1)?;
    let direction = match (incoming, outgoing) {
        (true, true) => DirectedRelationDirection::Bidirectional,
        (true, false) => DirectedRelationDirection::Incoming,
        (false, true) => DirectedRelationDirection::Outgoing,
        (false, false) => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(DirectedRelationRecord {
        note: row_to_note_with_offset(row, 0)?,
        direction,
        preview: row.get(ANCHOR_SELECT_COLUMN_COUNT + 2)?,
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use slipbox_core::DirectedRelationDirection;

    use super::DirectedRelationPosition;
    use crate::test_support::indexed_database;

    #[test]
    fn duplicate_and_reciprocal_links_make_one_directionally_complete_row() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "alpha.org",
                ":PROPERTIES:\n:ID: alpha-id\n:END:\n#+title: Alpha\n\nSee [[id:beta-id][Beta]] twice: [[id:beta-id][Beta]].\nSee [[id:gamma-id][Gamma]].\n",
            ),
            (
                "beta.org",
                ":PROPERTIES:\n:ID: beta-id\n:END:\n#+title: Beta\n\nReturn to [[id:alpha-id][Alpha]].\nAnd again [[id:alpha-id][Alpha]].\n",
            ),
            (
                "gamma.org",
                ":PROPERTIES:\n:ID: gamma-id\n:END:\n#+title: Gamma\n\nGamma gives its own useful excerpt.\n",
            ),
            (
                "zeta.org",
                ":PROPERTIES:\n:ID: zeta-id\n:END:\n#+title: Zeta\n\nPoints to [[id:alpha-id][Alpha]].\n",
            ),
        ])?;
        let alpha = database
            .node_from_id("alpha-id")?
            .expect("alpha is indexed");

        let page = database.directed_relations(&alpha, 10, None)?;

        assert_eq!(page.total, 3);
        assert_eq!(page.incoming_total, 2);
        assert_eq!(page.outgoing_total, 2);
        assert_eq!(
            page.relations
                .iter()
                .map(|record| (record.note.title.as_str(), record.direction))
                .collect::<Vec<_>>(),
            vec![
                ("Beta", DirectedRelationDirection::Bidirectional),
                ("Gamma", DirectedRelationDirection::Outgoing),
                ("Zeta", DirectedRelationDirection::Incoming),
            ],
        );
        assert!(page.relations[0].preview.contains("Return to"));
        assert_eq!(
            page.relations[1].preview,
            "Gamma gives its own useful excerpt.",
        );
        assert!(!page.has_more);
        assert_eq!(page.next_position, None);
        Ok(())
    }

    #[test]
    fn keyset_pages_reach_beyond_the_initial_relation_page() -> Result<()> {
        let mut owned = vec![(
            "focus.org".to_owned(),
            ":PROPERTIES:\n:ID: focus-id\n:END:\n#+title: Focus\n".to_owned(),
        )];
        for index in 0..205 {
            owned.push((
                format!("note-{index:02}.org"),
                format!(
                    ":PROPERTIES:\n:ID: note-{index:02}-id\n:END:\n#+title: Note {index:02}\n\nSee [[id:focus-id][Focus]].\n"
                ),
            ));
        }
        let borrowed = owned
            .iter()
            .map(|(name, body)| (name.as_str(), body.as_str()))
            .collect::<Vec<_>>();
        let (_workspace, database, _root) = indexed_database(&borrowed)?;
        let focus = database
            .node_from_id("focus-id")?
            .expect("focus is indexed");

        let first = database.directed_relations(&focus, 200, None)?;
        assert_eq!(first.relations.len(), 200);
        assert_eq!(first.total, 205);
        assert!(first.has_more);
        let position = DirectedRelationPosition::parse(
            first
                .next_position
                .as_deref()
                .expect("the first page continues"),
        )
        .expect("the relation cursor parses");

        let second = database.directed_relations(&focus, 200, Some(&position))?;
        assert_eq!(second.relations.len(), 5);
        assert_eq!(second.total, 205);
        assert!(!second.has_more);
        assert!(first.relations.iter().all(|left| {
            second
                .relations
                .iter()
                .all(|right| left.note.node_key != right.note.node_key)
        }),);
        Ok(())
    }

    #[test]
    fn a_relation_cursor_is_listing_specific_and_strict() {
        assert!(DirectedRelationPosition::parse("note.31.61").is_none());
        assert!(DirectedRelationPosition::parse("zz.31.61.62").is_none());
        assert!(
            DirectedRelationPosition::parse("64697265637465642d72656c6174696f6e.x.61.62").is_none()
        );
    }
}
