use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use slipbox_core::ReviewRun;
use urlencoding::encode;

use crate::Database;

const REVIEW_STORE_DIR_SUFFIX: &str = ".review-runs";
const REVIEW_STORE_LAYOUT_VERSION: &str = "v1";
const REVIEW_FILE_EXTENSION: &str = "json";

pub(crate) struct ReviewRunStore {
    root: PathBuf,
}

impl ReviewRunStore {
    pub(crate) fn for_database_path(path: &Path) -> Self {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("index.sqlite3");
        Self {
            root: path.with_file_name(format!("{file_name}{REVIEW_STORE_DIR_SUFFIX}")),
        }
    }

    pub(crate) fn migrate(&self) -> Result<()> {
        fs::create_dir_all(self.version_dir()).with_context(|| {
            format!(
                "failed to create review run store {}",
                self.version_dir().display()
            )
        })?;
        Ok(())
    }

    fn version_dir(&self) -> PathBuf {
        self.root.join(REVIEW_STORE_LAYOUT_VERSION)
    }

    fn review_path(&self, review_id: &str) -> PathBuf {
        self.version_dir()
            .join(format!("{}.{}", encode(review_id), REVIEW_FILE_EXTENSION))
    }

    fn temporary_review_path(&self, review_id: &str) -> PathBuf {
        self.version_dir().join(format!(
            ".{}.tmp-{}-{}",
            encode(review_id),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn validate_review_id(&self, review_id: &str) -> Result<()> {
        if review_id.trim().is_empty() {
            anyhow::bail!("review_id must not be empty");
        }
        if review_id.trim() != review_id {
            anyhow::bail!("review_id must not have leading or trailing whitespace");
        }
        Ok(())
    }

    fn load_review_file(&self, path: &Path) -> Result<ReviewRun> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read review run {}", path.display()))?;
        let review = serde_json::from_str::<ReviewRun>(&contents)
            .with_context(|| format!("failed to parse review run {}", path.display()))?;
        if let Some(error) = review.validation_error() {
            anyhow::bail!("stored review run {} is invalid: {}", path.display(), error);
        }
        let expected_path = self.review_path(&review.metadata.review_id);
        if expected_path != path {
            anyhow::bail!(
                "stored review run {} does not match review_id {}",
                path.display(),
                review.metadata.review_id
            );
        }
        Ok(review)
    }

    fn list_review_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        if !self.version_dir().exists() {
            return Ok(paths);
        }

        for entry in fs::read_dir(self.version_dir()).with_context(|| {
            format!(
                "failed to list review runs in {}",
                self.version_dir().display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some(REVIEW_FILE_EXTENSION) {
                paths.push(path);
            }
        }

        paths.sort();
        Ok(paths)
    }

    fn write_temporary_review_file(&self, review: &ReviewRun) -> Result<PathBuf> {
        let temporary_path = self.temporary_review_path(&review.metadata.review_id);
        let json = serde_json::to_vec_pretty(review).context("failed to serialize review run")?;
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => file,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to create temporary review run {}",
                        temporary_path.display()
                    )
                });
            }
        };
        if let Err(error) = file.write_all(&json).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&temporary_path);
            return Err(error).with_context(|| {
                format!(
                    "failed to write temporary review run {}",
                    temporary_path.display()
                )
            });
        }
        Ok(temporary_path)
    }

    fn save(&self, review: &ReviewRun) -> Result<()> {
        if let Some(error) = review.validation_error() {
            anyhow::bail!("review run is invalid: {error}");
        }

        let path = self.review_path(&review.metadata.review_id);
        let temporary_path = self.write_temporary_review_file(review)?;
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("failed to replace existing review run {}", path.display())
            })?;
        }
        if let Err(error) = fs::rename(&temporary_path, &path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error)
                .with_context(|| format!("failed to finalize review run {}", path.display()));
        }
        Ok(())
    }

    fn save_if_absent(&self, review: &ReviewRun) -> Result<bool> {
        if let Some(error) = review.validation_error() {
            anyhow::bail!("review run is invalid: {error}");
        }

        let path = self.review_path(&review.metadata.review_id);
        let temporary_path = self.write_temporary_review_file(review)?;
        match fs::hard_link(&temporary_path, &path) {
            Ok(()) => {
                fs::remove_file(&temporary_path).with_context(|| {
                    format!(
                        "failed to clean up temporary review run {}",
                        temporary_path.display()
                    )
                })?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temporary_path);
                Ok(false)
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary_path);
                Err(error).with_context(|| {
                    format!("failed to finalize new review run {}", path.display())
                })
            }
        }
    }

    fn load(&self, review_id: &str) -> Result<Option<ReviewRun>> {
        self.validate_review_id(review_id)?;
        let path = self.review_path(review_id);
        if !path.exists() {
            return Ok(None);
        }
        self.load_review_file(&path).map(Some)
    }

    fn list(&self) -> Result<Vec<ReviewRun>> {
        let mut reviews = self
            .list_review_paths()?
            .into_iter()
            .map(|path| self.load_review_file(&path))
            .collect::<Result<Vec<_>>>()?;
        reviews.sort_by(|left, right| {
            let left_title = left.metadata.title.to_ascii_lowercase();
            let right_title = right.metadata.title.to_ascii_lowercase();
            left_title
                .cmp(&right_title)
                .then_with(|| left.metadata.title.cmp(&right.metadata.title))
                .then_with(|| left.metadata.review_id.cmp(&right.metadata.review_id))
        });
        Ok(reviews)
    }

    fn list_newest_first(&self) -> Result<Vec<ReviewRun>> {
        let mut reviews = self
            .list_review_paths()?
            .into_iter()
            .map(|path| {
                let modified = fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(UNIX_EPOCH);
                self.load_review_file(&path)
                    .map(|review| (modified, review))
            })
            .collect::<Result<Vec<_>>>()?;
        reviews.sort_by(|(left_modified, left), (right_modified, right)| {
            right_modified
                .cmp(left_modified)
                .then_with(|| right.metadata.review_id.cmp(&left.metadata.review_id))
        });
        Ok(reviews.into_iter().map(|(_, review)| review).collect())
    }

    fn delete(&self, review_id: &str) -> Result<bool> {
        self.validate_review_id(review_id)?;
        let path = self.review_path(review_id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path)
            .with_context(|| format!("failed to delete review run {}", path.display()))?;
        Ok(true)
    }
}

impl Database {
    pub fn save_review_run(&self, review: &ReviewRun) -> Result<()> {
        self.review_store.save(review)
    }

    pub fn save_review_run_if_absent(&self, review: &ReviewRun) -> Result<bool> {
        self.review_store.save_if_absent(review)
    }

    pub fn review_run(&self, review_id: &str) -> Result<Option<ReviewRun>> {
        self.review_store.load(review_id)
    }

    pub fn list_review_runs(&self) -> Result<Vec<ReviewRun>> {
        self.review_store.list()
    }

    pub fn list_review_runs_newest_first(&self) -> Result<Vec<ReviewRun>> {
        self.review_store.list_newest_first()
    }

    pub fn delete_review_run(&self, review_id: &str) -> Result<bool> {
        self.review_store.delete(review_id)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use slipbox_core::{
        AnchorRecord, CorpusAuditEntry, CorpusAuditKind, DanglingLinkAuditRecord, GraphParams,
        NodeKind, ReviewFinding, ReviewFindingPayload, ReviewFindingStatus, ReviewRun,
        ReviewRunMetadata, ReviewRunPayload,
    };

    use crate::{Database, test_support::indexed_database};

    use super::{REVIEW_STORE_LAYOUT_VERSION, ReviewRunStore};

    #[test]
    fn review_runs_round_trip_and_support_update_delete() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;
        let mut review = audit_review(
            "review/audit/dangling-links",
            "Dangling Link Review",
            Some("Review missing links".to_owned()),
        );

        database.save_review_run(&review)?;

        let review_path = ReviewRunStore::for_database_path(&db_path)
            .version_dir()
            .join("review%2Faudit%2Fdangling-links.json");
        assert!(review_path.exists());
        let raw = fs::read_to_string(&review_path)?;
        assert!(raw.contains("\"review_id\": \"review/audit/dangling-links\""));
        assert!(raw.contains("\"kind\": \"audit\""));

        assert_eq!(
            database.review_run("review/audit/dangling-links")?,
            Some(review.clone())
        );
        assert_eq!(database.list_review_runs()?, vec![review.clone()]);

        review.metadata.title = "Dangling Link Review Updated".to_owned();
        review.metadata.summary = Some("Updated summary".to_owned());
        database.save_review_run(&review)?;
        assert_eq!(
            database.review_run("review/audit/dangling-links")?,
            Some(review.clone())
        );

        assert!(database.delete_review_run("review/audit/dangling-links")?);
        assert_eq!(database.review_run("review/audit/dangling-links")?, None);
        assert!(database.list_review_runs()?.is_empty());
        assert!(!database.delete_review_run("review/audit/dangling-links")?);

        Ok(())
    }

    #[test]
    fn save_if_absent_refuses_to_replace_existing_review_runs() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;
        let original = audit_review("review/audit/dangling-links", "Original", None);
        let replacement = audit_review("review/audit/dangling-links", "Replacement", None);

        assert!(database.save_review_run_if_absent(&original)?);
        assert!(!database.save_review_run_if_absent(&replacement)?);
        assert_eq!(
            database.review_run("review/audit/dangling-links")?,
            Some(original)
        );
        let version_dir = ReviewRunStore::for_database_path(&db_path).version_dir();
        let mut file_names = fs::read_dir(version_dir)?
            .map(|entry| entry.map(|value| value.file_name().to_string_lossy().into_owned()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        file_names.sort();
        assert_eq!(
            file_names,
            vec!["review%2Faudit%2Fdangling-links.json".to_owned()]
        );

        Ok(())
    }

    #[test]
    fn review_run_operations_reject_padded_review_ids() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;

        let padded = audit_review(" review/audit ", "Padded", None);
        let save_error = database
            .save_review_run(&padded)
            .expect_err("padded review_id should be rejected on save");
        assert_eq!(
            save_error.to_string(),
            "review run is invalid: review_id must not have leading or trailing whitespace"
        );

        let load_error = database
            .review_run(" review/audit ")
            .expect_err("padded review_id should be rejected on load");
        assert_eq!(
            load_error.to_string(),
            "review_id must not have leading or trailing whitespace"
        );

        let delete_error = database
            .delete_review_run(" review/audit ")
            .expect_err("padded review_id should be rejected on delete");
        assert_eq!(
            delete_error.to_string(),
            "review_id must not have leading or trailing whitespace"
        );

        assert!(database.list_review_runs()?.is_empty());

        Ok(())
    }

    #[test]
    fn stored_review_runs_reject_invalid_and_path_mismatched_json() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;
        let store = ReviewRunStore::for_database_path(&db_path);

        let invalid_path = store.review_path("review/invalid");
        fs::write(
            &invalid_path,
            serde_json::to_vec_pretty(&audit_review(" ", "Invalid", None))?,
        )?;
        let invalid_error = database
            .review_run("review/invalid")
            .expect_err("invalid stored review should be rejected");
        assert!(invalid_error.to_string().starts_with("stored review run "));
        assert!(
            invalid_error
                .to_string()
                .contains("is invalid: review_id must not be empty")
        );
        fs::remove_file(&invalid_path)?;

        let mismatch_path = store.review_path("review/mismatch");
        fs::write(
            &mismatch_path,
            serde_json::to_vec_pretty(&audit_review("review/other", "Other", None))?,
        )?;
        let mismatch_error = database
            .review_run("review/mismatch")
            .expect_err("path-mismatched stored review should be rejected");
        assert!(
            mismatch_error
                .to_string()
                .contains("does not match review_id review/other")
        );

        Ok(())
    }

    #[test]
    fn review_runs_survive_fresh_open_and_schema_rebuild() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let review = audit_review(
            "review/audit/dangling-links",
            "Dangling Link Review",
            Some("Persist across sessions".to_owned()),
        );

        {
            let database = Database::open(&db_path)?;
            database.save_review_run(&review)?;
        }

        {
            let database = Database::open(&db_path)?;
            assert_eq!(
                database.review_run("review/audit/dangling-links")?,
                Some(review.clone())
            );
            database
                .connection
                .execute_batch("PRAGMA user_version = 0;")?;
        }

        let database = Database::open(&db_path)?;
        assert_eq!(
            database.review_run("review/audit/dangling-links")?,
            Some(review)
        );
        assert!(
            ReviewRunStore::for_database_path(&db_path)
                .root
                .join(REVIEW_STORE_LAYOUT_VERSION)
                .exists()
        );

        Ok(())
    }

    #[test]
    fn review_runs_do_not_pollute_note_surfaces() -> Result<()> {
        let (_workspace, database, root) = indexed_database(&[
            (
                "focus.org",
                ":PROPERTIES:\n:ID: focus\n:ROAM_REFS: cite:focus2026\n:END:\n#+title: Focus\nSee [[id:neighbor][Neighbor]].\n* TODO Follow up\n",
            ),
            (
                "neighbor.org",
                "#+title: Neighbor\n:PROPERTIES:\n:ID: neighbor\n:END:\n* DONE Review\n",
            ),
        ])?;
        let focus = database
            .node_from_id("focus")?
            .expect("focus note should be indexed");
        let neighbor = database
            .node_from_id("neighbor")?
            .expect("neighbor note should be indexed");
        let before_notes = database.search_nodes("", 20, None)?;
        let before_anchors = database.search_anchors("", 20, None)?;
        let before_files = database.indexed_files()?;
        let before_refs = database.search_refs("", 20)?;
        let before_backlinks = database.backlinks(&neighbor.node_key, 20, true)?;
        let before_forward_links = database.forward_links(&focus.node_key, 20, true)?;
        let before_graph = database.graph_dot(&GraphParams {
            root_node_key: None,
            max_distance: None,
            include_orphans: true,
            hidden_link_types: Vec::new(),
            max_title_length: 100,
            shorten_titles: None,
            node_url_prefix: None,
        })?;

        database.save_review_run(&audit_review(
            "review/audit/dangling-links",
            "Workbench Review",
            Some("Should not leak into note discovery".to_owned()),
        ))?;

        assert_eq!(database.search_nodes("", 20, None)?, before_notes);
        assert_eq!(database.search_anchors("", 20, None)?, before_anchors);
        assert_eq!(database.indexed_files()?, before_files);
        assert_eq!(database.search_refs("", 20)?, before_refs);
        assert_eq!(
            database.backlinks(&neighbor.node_key, 20, true)?,
            before_backlinks
        );
        assert_eq!(
            database.forward_links(&focus.node_key, 20, true)?,
            before_forward_links
        );
        assert_eq!(
            database.graph_dot(&GraphParams {
                root_node_key: None,
                max_distance: None,
                include_orphans: true,
                hidden_link_types: Vec::new(),
                max_title_length: 100,
                shorten_titles: None,
                node_url_prefix: None,
            })?,
            before_graph
        );
        assert!(
            database
                .search_nodes("Workbench Review", 20, None)?
                .is_empty()
        );
        assert_eq!(
            database
                .list_review_runs()?
                .into_iter()
                .map(|review| review.metadata.review_id)
                .collect::<Vec<_>>(),
            vec!["review/audit/dangling-links".to_owned()]
        );
        let db_path = root
            .parent()
            .expect("workspace parent")
            .join("index.sqlite3");
        assert!(
            ReviewRunStore::for_database_path(&db_path)
                .root
                .join(REVIEW_STORE_LAYOUT_VERSION)
                .exists()
        );

        Ok(())
    }

    fn audit_review(review_id: &str, title: &str, summary: Option<String>) -> ReviewRun {
        ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: review_id.to_owned(),
                title: title.to_owned(),
                summary,
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::DanglingLinks,
                limit: 200,
            },
            findings: vec![ReviewFinding {
                finding_id: "audit/dangling-links/source/missing-id".to_owned(),
                status: ReviewFindingStatus::Open,
                payload: ReviewFindingPayload::Audit {
                    entry: Box::new(CorpusAuditEntry::DanglingLink {
                        record: Box::new(DanglingLinkAuditRecord {
                            source: sample_anchor("heading:source.org:3", "Source Heading"),
                            missing_explicit_id: "missing-id".to_owned(),
                            line: 12,
                            column: 7,
                            preview: "[[id:missing-id][Missing]]".to_owned(),
                        }),
                    }),
                },
            }],
        }
    }

    fn sample_anchor(node_key: &str, title: &str) -> AnchorRecord {
        AnchorRecord {
            node_key: node_key.to_owned(),
            explicit_id: None,
            file_path: "sample.org".to_owned(),
            title: title.to_owned(),
            outline_path: title.to_owned(),
            aliases: Vec::new(),
            tags: Vec::new(),
            refs: Vec::new(),
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            level: 1,
            line: 1,
            kind: NodeKind::Heading,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        }
    }
}
