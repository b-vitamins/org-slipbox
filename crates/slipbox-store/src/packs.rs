use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use slipbox_core::WorkbenchPackManifest;
use urlencoding::encode;

use crate::Database;

const PACK_STORE_DIR_SUFFIX: &str = ".workbench-packs";
const PACK_STORE_LAYOUT_VERSION: &str = "v1";
const PACK_FILE_EXTENSION: &str = "json";

pub(crate) struct WorkbenchPackStore {
    root: PathBuf,
}

impl WorkbenchPackStore {
    pub(crate) fn for_database_path(path: &Path) -> Self {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("index.sqlite3");
        Self {
            root: path.with_file_name(format!("{file_name}{PACK_STORE_DIR_SUFFIX}")),
        }
    }

    pub(crate) fn migrate(&self) -> Result<()> {
        fs::create_dir_all(self.version_dir()).with_context(|| {
            format!(
                "failed to create workbench pack store {}",
                self.version_dir().display()
            )
        })?;
        Ok(())
    }

    fn version_dir(&self) -> PathBuf {
        self.root.join(PACK_STORE_LAYOUT_VERSION)
    }

    fn pack_path(&self, pack_id: &str) -> PathBuf {
        self.version_dir()
            .join(format!("{}.{}", encode(pack_id), PACK_FILE_EXTENSION))
    }

    fn temporary_pack_path(&self, pack_id: &str) -> PathBuf {
        self.version_dir().join(format!(
            ".{}.tmp-{}-{}",
            encode(pack_id),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn validate_pack_id(&self, pack_id: &str) -> Result<()> {
        if pack_id.trim().is_empty() {
            anyhow::bail!("pack_id must not be empty");
        }
        if pack_id.trim() != pack_id {
            anyhow::bail!("pack_id must not have leading or trailing whitespace");
        }
        Ok(())
    }

    fn load_pack_file(&self, path: &Path) -> Result<WorkbenchPackManifest> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read workbench pack {}", path.display()))?;
        let pack = serde_json::from_str::<WorkbenchPackManifest>(&contents)
            .with_context(|| format!("failed to parse workbench pack {}", path.display()))?;
        if let Some(error) = pack.validation_error() {
            anyhow::bail!(
                "stored workbench pack {} is invalid: {}",
                path.display(),
                error
            );
        }
        let expected_path = self.pack_path(&pack.metadata.pack_id);
        if expected_path != path {
            anyhow::bail!(
                "stored workbench pack {} does not match pack_id {}",
                path.display(),
                pack.metadata.pack_id
            );
        }
        Ok(pack)
    }

    fn list_pack_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        if !self.version_dir().exists() {
            return Ok(paths);
        }

        for entry in fs::read_dir(self.version_dir()).with_context(|| {
            format!(
                "failed to list workbench packs in {}",
                self.version_dir().display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some(PACK_FILE_EXTENSION) {
                paths.push(path);
            }
        }

        paths.sort();
        Ok(paths)
    }

    fn write_temporary_pack_file(&self, pack: &WorkbenchPackManifest) -> Result<PathBuf> {
        let temporary_path = self.temporary_pack_path(&pack.metadata.pack_id);
        let json = serde_json::to_vec_pretty(pack).context("failed to serialize workbench pack")?;
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => file,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to create temporary workbench pack {}",
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
                    "failed to write temporary workbench pack {}",
                    temporary_path.display()
                )
            });
        }
        Ok(temporary_path)
    }

    fn save(&self, pack: &WorkbenchPackManifest) -> Result<()> {
        if let Some(error) = pack.validation_error() {
            anyhow::bail!("workbench pack is invalid: {error}");
        }

        let path = self.pack_path(&pack.metadata.pack_id);
        let temporary_path = self.write_temporary_pack_file(pack)?;
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!(
                    "failed to replace existing workbench pack {}",
                    path.display()
                )
            })?;
        }
        if let Err(error) = fs::rename(&temporary_path, &path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error)
                .with_context(|| format!("failed to finalize workbench pack {}", path.display()));
        }
        Ok(())
    }

    fn save_if_absent(&self, pack: &WorkbenchPackManifest) -> Result<bool> {
        if let Some(error) = pack.validation_error() {
            anyhow::bail!("workbench pack is invalid: {error}");
        }

        let path = self.pack_path(&pack.metadata.pack_id);
        let temporary_path = self.write_temporary_pack_file(pack)?;
        match fs::hard_link(&temporary_path, &path) {
            Ok(()) => {
                fs::remove_file(&temporary_path).with_context(|| {
                    format!(
                        "failed to clean up temporary workbench pack {}",
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
                    format!("failed to finalize new workbench pack {}", path.display())
                })
            }
        }
    }

    fn load(&self, pack_id: &str) -> Result<Option<WorkbenchPackManifest>> {
        self.validate_pack_id(pack_id)?;
        let path = self.pack_path(pack_id);
        if !path.exists() {
            return Ok(None);
        }
        self.load_pack_file(&path).map(Some)
    }

    fn list(&self) -> Result<Vec<WorkbenchPackManifest>> {
        let mut packs = self
            .list_pack_paths()?
            .into_iter()
            .map(|path| self.load_pack_file(&path))
            .collect::<Result<Vec<_>>>()?;
        packs.sort_by(|left, right| {
            let left_title = left.metadata.title.to_ascii_lowercase();
            let right_title = right.metadata.title.to_ascii_lowercase();
            left_title
                .cmp(&right_title)
                .then_with(|| left.metadata.title.cmp(&right.metadata.title))
                .then_with(|| left.metadata.pack_id.cmp(&right.metadata.pack_id))
        });
        Ok(packs)
    }

    fn delete(&self, pack_id: &str) -> Result<bool> {
        self.validate_pack_id(pack_id)?;
        let path = self.pack_path(pack_id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path)
            .with_context(|| format!("failed to delete workbench pack {}", path.display()))?;
        Ok(true)
    }
}

impl Database {
    pub fn save_workbench_pack(&self, pack: &WorkbenchPackManifest) -> Result<()> {
        self.pack_store.save(pack)
    }

    pub fn save_workbench_pack_if_absent(&self, pack: &WorkbenchPackManifest) -> Result<bool> {
        self.pack_store.save_if_absent(pack)
    }

    pub fn workbench_pack(&self, pack_id: &str) -> Result<Option<WorkbenchPackManifest>> {
        self.pack_store.load(pack_id)
    }

    pub fn list_workbench_packs(&self) -> Result<Vec<WorkbenchPackManifest>> {
        self.pack_store.list()
    }

    pub fn delete_workbench_pack(&self, pack_id: &str) -> Result<bool> {
        self.pack_store.delete(pack_id)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use slipbox_core::{
        ExplorationLens, GraphParams, ReportJsonlLineKind, ReportProfileMetadata,
        ReportProfileMode, ReportProfileSpec, ReportProfileSubject, ReviewFindingStatus,
        ReviewRoutineMetadata, ReviewRoutineSaveReviewPolicy, ReviewRoutineSource,
        ReviewRoutineSpec, WorkbenchPackCompatibility, WorkbenchPackManifest,
        WorkbenchPackMetadata, WorkflowExploreFocus, WorkflowInputKind, WorkflowInputSpec,
        WorkflowMetadata, WorkflowSpec, WorkflowSpecCompatibility, WorkflowStepPayload,
        WorkflowStepSpec,
    };

    use crate::{Database, test_support::indexed_database};

    use super::{PACK_STORE_LAYOUT_VERSION, WorkbenchPackStore};

    #[test]
    fn workbench_packs_round_trip_and_support_update_delete() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;
        let mut pack = workbench_pack(
            "pack/research-review",
            "Research Review Pack",
            Some("Reusable review assets".to_owned()),
        );

        database.save_workbench_pack(&pack)?;

        let pack_path = WorkbenchPackStore::for_database_path(&db_path)
            .version_dir()
            .join("pack%2Fresearch-review.json");
        assert!(pack_path.exists());
        let raw = fs::read_to_string(&pack_path)?;
        assert!(raw.contains("\"pack_id\": \"pack/research-review\""));
        assert!(raw.contains("\"workflows\""));
        assert!(raw.contains("\"review_routines\""));
        assert!(raw.contains("\"report_profiles\""));

        assert_eq!(
            database.workbench_pack("pack/research-review")?,
            Some(pack.clone())
        );
        assert_eq!(database.list_workbench_packs()?, vec![pack.clone()]);

        pack.metadata.title = "Research Review Pack Updated".to_owned();
        pack.metadata.summary = Some("Updated summary".to_owned());
        database.save_workbench_pack(&pack)?;
        assert_eq!(
            database.workbench_pack("pack/research-review")?,
            Some(pack.clone())
        );

        assert!(database.delete_workbench_pack("pack/research-review")?);
        assert_eq!(database.workbench_pack("pack/research-review")?, None);
        assert!(database.list_workbench_packs()?.is_empty());
        assert!(!database.delete_workbench_pack("pack/research-review")?);

        Ok(())
    }

    #[test]
    fn save_if_absent_refuses_to_replace_existing_workbench_packs() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;
        let original = workbench_pack("pack/research-review", "Original", None);
        let replacement = workbench_pack("pack/research-review", "Replacement", None);

        assert!(database.save_workbench_pack_if_absent(&original)?);
        assert!(!database.save_workbench_pack_if_absent(&replacement)?);
        assert_eq!(
            database.workbench_pack("pack/research-review")?,
            Some(original)
        );
        let version_dir = WorkbenchPackStore::for_database_path(&db_path).version_dir();
        let mut file_names = fs::read_dir(version_dir)?
            .map(|entry| entry.map(|value| value.file_name().to_string_lossy().into_owned()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        file_names.sort();
        assert_eq!(file_names, vec!["pack%2Fresearch-review.json".to_owned()]);

        Ok(())
    }

    #[test]
    fn workbench_pack_operations_reject_padded_pack_ids_before_writes() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;

        let padded = workbench_pack(" pack/research-review ", "Padded", None);
        let save_error = database
            .save_workbench_pack(&padded)
            .expect_err("padded pack_id should be rejected on save");
        assert_eq!(
            save_error.to_string(),
            "workbench pack is invalid: pack_id must not have leading or trailing whitespace"
        );

        let load_error = database
            .workbench_pack(" pack/research-review ")
            .expect_err("padded pack_id should be rejected on load");
        assert_eq!(
            load_error.to_string(),
            "pack_id must not have leading or trailing whitespace"
        );

        let delete_error = database
            .delete_workbench_pack(" pack/research-review ")
            .expect_err("padded pack_id should be rejected on delete");
        assert_eq!(
            delete_error.to_string(),
            "pack_id must not have leading or trailing whitespace"
        );

        assert!(database.list_workbench_packs()?.is_empty());
        let version_dir = WorkbenchPackStore::for_database_path(&db_path).version_dir();
        assert!(fs::read_dir(version_dir)?.next().is_none());

        Ok(())
    }

    #[test]
    fn stored_workbench_packs_reject_malformed_invalid_and_path_mismatched_json() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;
        let store = WorkbenchPackStore::for_database_path(&db_path);

        let malformed_path = store.pack_path("pack/malformed");
        fs::write(&malformed_path, b"{ not json")?;
        let malformed_error = database
            .workbench_pack("pack/malformed")
            .expect_err("malformed stored pack should be rejected");
        assert!(
            malformed_error
                .to_string()
                .starts_with("failed to parse workbench pack ")
        );
        fs::remove_file(&malformed_path)?;

        let invalid_path = store.pack_path("pack/invalid");
        fs::write(
            &invalid_path,
            serde_json::to_vec_pretty(&workbench_pack(" ", "Invalid", None))?,
        )?;
        let invalid_error = database
            .workbench_pack("pack/invalid")
            .expect_err("invalid stored pack should be rejected");
        assert!(
            invalid_error
                .to_string()
                .starts_with("stored workbench pack ")
        );
        assert!(
            invalid_error
                .to_string()
                .contains("is invalid: pack_id must not be empty")
        );
        fs::remove_file(&invalid_path)?;

        let mismatch_path = store.pack_path("pack/mismatch");
        fs::write(
            &mismatch_path,
            serde_json::to_vec_pretty(&workbench_pack("pack/other", "Other", None))?,
        )?;
        let mismatch_error = database
            .workbench_pack("pack/mismatch")
            .expect_err("path-mismatched stored pack should be rejected");
        assert!(
            mismatch_error
                .to_string()
                .contains("does not match pack_id pack/other")
        );

        Ok(())
    }

    #[test]
    fn workbench_packs_survive_fresh_open_and_schema_rebuild() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let pack = workbench_pack(
            "pack/research-review",
            "Research Review Pack",
            Some("Persist across sessions".to_owned()),
        );

        {
            let database = Database::open(&db_path)?;
            database.save_workbench_pack(&pack)?;
        }

        {
            let database = Database::open(&db_path)?;
            assert_eq!(
                database.workbench_pack("pack/research-review")?,
                Some(pack.clone())
            );
            database
                .connection
                .execute_batch("PRAGMA user_version = 0;")?;
        }

        let database = Database::open(&db_path)?;
        assert_eq!(database.workbench_pack("pack/research-review")?, Some(pack));
        assert!(
            WorkbenchPackStore::for_database_path(&db_path)
                .root
                .join(PACK_STORE_LAYOUT_VERSION)
                .exists()
        );

        Ok(())
    }

    #[test]
    fn workbench_packs_do_not_pollute_note_review_or_artifact_surfaces() -> Result<()> {
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
        let graph_params = GraphParams {
            root_node_key: None,
            max_distance: None,
            include_orphans: true,
            hidden_link_types: Vec::new(),
            max_title_length: 100,
            shorten_titles: None,
            node_url_prefix: None,
        };
        let before_graph = database.graph_dot(&graph_params)?;

        database.save_workbench_pack(&workbench_pack(
            "pack/research-review",
            "Workbench Pack",
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
        assert_eq!(database.graph_dot(&graph_params)?, before_graph);
        assert!(
            database
                .search_nodes("Workbench Pack", 20, None)?
                .is_empty()
        );
        assert!(database.list_review_runs()?.is_empty());
        assert!(database.list_exploration_artifacts()?.is_empty());
        assert_eq!(
            database
                .list_workbench_packs()?
                .into_iter()
                .map(|pack| pack.metadata.pack_id)
                .collect::<Vec<_>>(),
            vec!["pack/research-review".to_owned()]
        );
        assert_eq!(database.review_run("pack/research-review")?, None);
        assert_eq!(database.exploration_artifact("pack/research-review")?, None);
        let db_path = root
            .parent()
            .expect("workspace parent")
            .join("index.sqlite3");
        assert!(
            WorkbenchPackStore::for_database_path(&db_path)
                .root
                .join(PACK_STORE_LAYOUT_VERSION)
                .exists()
        );

        Ok(())
    }

    fn workbench_pack(
        pack_id: &str,
        title: &str,
        summary: Option<String>,
    ) -> WorkbenchPackManifest {
        WorkbenchPackManifest {
            metadata: WorkbenchPackMetadata {
                pack_id: pack_id.to_owned(),
                title: title.to_owned(),
                summary,
            },
            compatibility: WorkbenchPackCompatibility::default(),
            workflows: vec![workflow()],
            review_routines: vec![review_routine()],
            report_profiles: vec![report_profile()],
            entrypoint_routine_ids: vec!["routine/pack/context-review".to_owned()],
        }
    }

    fn workflow() -> WorkflowSpec {
        WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/pack/context-review".to_owned(),
                title: "Pack Context Review".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: vec![WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus".to_owned(),
                summary: None,
                kind: WorkflowInputKind::FocusTarget,
            }],
            steps: vec![WorkflowStepSpec {
                step_id: "explore-context".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Bridges,
                    limit: 25,
                    unique: false,
                },
            }],
        }
    }

    fn review_routine() -> ReviewRoutineSpec {
        ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: "routine/pack/context-review".to_owned(),
                title: "Pack Context Review".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Workflow {
                workflow_id: "workflow/pack/context-review".to_owned(),
            },
            inputs: vec![WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus".to_owned(),
                summary: None,
                kind: WorkflowInputKind::FocusTarget,
            }],
            save_review: ReviewRoutineSaveReviewPolicy::default(),
            compare: None,
            report_profile_ids: vec!["profile/pack/routine-detail".to_owned()],
        }
    }

    fn report_profile() -> ReportProfileSpec {
        ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: "profile/pack/routine-detail".to_owned(),
                title: "Routine Detail".to_owned(),
                summary: None,
            },
            subjects: vec![ReportProfileSubject::Routine, ReportProfileSubject::Review],
            mode: ReportProfileMode::Detail,
            status_filters: Some(vec![ReviewFindingStatus::Open]),
            diff_buckets: None,
            jsonl_line_kinds: Some(vec![
                ReportJsonlLineKind::Routine,
                ReportJsonlLineKind::Review,
                ReportJsonlLineKind::Finding,
            ]),
        }
    }
}
