use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use crate::Database;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccurrenceDocumentRecord {
    pub file_path: String,
    pub search_text: String,
    pub line_rows: Vec<u32>,
}

impl Database {
    pub fn search_occurrence_document_paths(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<String>> {
        let Some(fts_query) = build_occurrence_fts_query(query) else {
            return Ok(Vec::new());
        };

        let limit = limit.clamp(1, 1_000) as i64;
        let offset = offset as i64;
        let mut statement = self.connection.prepare(
            "SELECT od.file_path
               FROM occurrence_document_fts
               JOIN occurrence_documents AS od ON od.id = occurrence_document_fts.rowid
              WHERE occurrence_document_fts MATCH ?1
              ORDER BY od.file_path COLLATE NOCASE, od.file_path
              LIMIT ?2
             OFFSET ?3",
        )?;
        let rows = statement.query_map(params![fts_query, limit, offset], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read occurrence search candidate paths")
    }

    pub fn occurrence_document(&self, file_path: &str) -> Result<Option<OccurrenceDocumentRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT od.file_path,
                    od.search_text,
                    od.line_rows_json
               FROM occurrence_documents AS od
              WHERE od.file_path = ?1
              LIMIT 1",
        )?;
        statement
            .query_row(params![file_path], row_to_occurrence)
            .optional()
            .context("failed to read occurrence document")
    }
}

fn row_to_occurrence(row: &rusqlite::Row<'_>) -> rusqlite::Result<OccurrenceDocumentRecord> {
    let line_rows_json: String = row.get(2)?;
    Ok(OccurrenceDocumentRecord {
        file_path: row.get(0)?,
        search_text: row.get(1)?,
        line_rows: parse_line_rows(&line_rows_json)?,
    })
}

fn build_occurrence_fts_query(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.chars().count() < 3 {
        None
    } else {
        Some(format!("\"{}\"", trimmed.replace('"', "\"\"")))
    }
}

fn parse_line_rows(input: &str) -> rusqlite::Result<Vec<u32>> {
    serde_json::from_str(input).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            input.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}
