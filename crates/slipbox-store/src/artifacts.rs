use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use slipbox_core::SavedExplorationArtifact;
use urlencoding::encode;

use crate::Database;

const ARTIFACT_STORE_DIR_SUFFIX: &str = ".exploration-artifacts";
const ARTIFACT_STORE_LAYOUT_VERSION: &str = "v1";
const ARTIFACT_FILE_EXTENSION: &str = "json";

pub(crate) struct ExplorationArtifactStore {
    root: PathBuf,
}

impl ExplorationArtifactStore {
    pub(crate) fn for_database_path(path: &Path) -> Self {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("index.sqlite3");
        Self {
            root: path.with_file_name(format!("{file_name}{ARTIFACT_STORE_DIR_SUFFIX}")),
        }
    }

    pub(crate) fn migrate(&self) -> Result<()> {
        fs::create_dir_all(self.version_dir()).with_context(|| {
            format!(
                "failed to create exploration artifact store {}",
                self.version_dir().display()
            )
        })?;
        Ok(())
    }

    fn version_dir(&self) -> PathBuf {
        self.root.join(ARTIFACT_STORE_LAYOUT_VERSION)
    }

    fn artifact_path(&self, artifact_id: &str) -> PathBuf {
        self.version_dir().join(format!(
            "{}.{}",
            encode(artifact_id),
            ARTIFACT_FILE_EXTENSION
        ))
    }

    fn validate_artifact_id(&self, artifact_id: &str) -> Result<()> {
        if artifact_id.trim().is_empty() {
            anyhow::bail!("artifact_id must not be empty");
        }
        if artifact_id.trim() != artifact_id {
            anyhow::bail!("artifact_id must not have leading or trailing whitespace");
        }
        Ok(())
    }

    fn load_artifact_file(&self, path: &Path) -> Result<SavedExplorationArtifact> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read exploration artifact {}", path.display()))?;
        let artifact = serde_json::from_str::<SavedExplorationArtifact>(&contents)
            .with_context(|| format!("failed to parse exploration artifact {}", path.display()))?;
        if let Some(error) = artifact.validation_error() {
            anyhow::bail!(
                "stored exploration artifact {} is invalid: {}",
                path.display(),
                error
            );
        }
        let expected_path = self.artifact_path(&artifact.metadata.artifact_id);
        if expected_path != path {
            anyhow::bail!(
                "stored exploration artifact {} does not match artifact_id {}",
                path.display(),
                artifact.metadata.artifact_id
            );
        }
        Ok(artifact)
    }

    fn list_artifact_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        if !self.version_dir().exists() {
            return Ok(paths);
        }

        for entry in fs::read_dir(self.version_dir()).with_context(|| {
            format!(
                "failed to list exploration artifacts in {}",
                self.version_dir().display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some(ARTIFACT_FILE_EXTENSION) {
                paths.push(path);
            }
        }

        paths.sort();
        Ok(paths)
    }

    fn save(&self, artifact: &SavedExplorationArtifact) -> Result<()> {
        if let Some(error) = artifact.validation_error() {
            anyhow::bail!("exploration artifact is invalid: {error}");
        }

        let path = self.artifact_path(&artifact.metadata.artifact_id);
        let temporary_path = self.version_dir().join(format!(
            ".{}.tmp-{}-{}",
            encode(&artifact.metadata.artifact_id),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let json = serde_json::to_string_pretty(artifact)
            .context("failed to serialize exploration artifact")?;
        fs::write(&temporary_path, json).with_context(|| {
            format!(
                "failed to write temporary exploration artifact {}",
                temporary_path.display()
            )
        })?;
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!(
                    "failed to replace existing exploration artifact {}",
                    path.display()
                )
            })?;
        }
        fs::rename(&temporary_path, &path).with_context(|| {
            format!("failed to finalize exploration artifact {}", path.display())
        })?;
        Ok(())
    }

    fn load(&self, artifact_id: &str) -> Result<Option<SavedExplorationArtifact>> {
        self.validate_artifact_id(artifact_id)?;
        let path = self.artifact_path(artifact_id);
        if !path.exists() {
            return Ok(None);
        }
        self.load_artifact_file(&path).map(Some)
    }

    fn list(&self) -> Result<Vec<SavedExplorationArtifact>> {
        let mut artifacts = self
            .list_artifact_paths()?
            .into_iter()
            .map(|path| self.load_artifact_file(&path))
            .collect::<Result<Vec<_>>>()?;
        artifacts.sort_by(|left, right| {
            let left_title = left.metadata.title.to_ascii_lowercase();
            let right_title = right.metadata.title.to_ascii_lowercase();
            left_title
                .cmp(&right_title)
                .then_with(|| left.metadata.title.cmp(&right.metadata.title))
                .then_with(|| left.metadata.artifact_id.cmp(&right.metadata.artifact_id))
        });
        Ok(artifacts)
    }

    fn delete(&self, artifact_id: &str) -> Result<bool> {
        self.validate_artifact_id(artifact_id)?;
        let path = self.artifact_path(artifact_id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path)
            .with_context(|| format!("failed to delete exploration artifact {}", path.display()))?;
        Ok(true)
    }
}

impl Database {
    pub fn save_exploration_artifact(&self, artifact: &SavedExplorationArtifact) -> Result<()> {
        self.artifact_store.save(artifact)
    }

    pub fn exploration_artifact(
        &self,
        artifact_id: &str,
    ) -> Result<Option<SavedExplorationArtifact>> {
        self.artifact_store.load(artifact_id)
    }

    pub fn list_exploration_artifacts(&self) -> Result<Vec<SavedExplorationArtifact>> {
        self.artifact_store.list()
    }

    pub fn delete_exploration_artifact(&self, artifact_id: &str) -> Result<bool> {
        self.artifact_store.delete(artifact_id)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use slipbox_core::{
        ExplorationArtifactMetadata, ExplorationArtifactPayload, ExplorationLens,
        SavedExplorationArtifact, SavedLensViewArtifact,
    };

    use crate::{Database, test_support::indexed_database};

    use super::{ARTIFACT_STORE_LAYOUT_VERSION, ExplorationArtifactStore};

    #[test]
    fn exploration_artifacts_round_trip_and_support_update_delete() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;
        let mut artifact = lens_view_artifact(
            "lens/focus",
            "Lens Focus",
            Some("Saved structure lens".to_owned()),
        );

        database.save_exploration_artifact(&artifact)?;

        let artifact_path = ExplorationArtifactStore::for_database_path(&db_path)
            .version_dir()
            .join("lens%2Ffocus.json");
        assert!(artifact_path.exists());
        let raw = fs::read_to_string(&artifact_path)?;
        assert!(raw.contains("\"artifact_id\": \"lens/focus\""));
        assert!(raw.contains("\"kind\": \"lens-view\""));

        assert_eq!(
            database.exploration_artifact("lens/focus")?,
            Some(artifact.clone())
        );
        assert_eq!(
            database.list_exploration_artifacts()?,
            vec![artifact.clone()]
        );

        artifact.metadata.title = "Lens Focus Updated".to_owned();
        artifact.metadata.summary = Some("Updated summary".to_owned());
        database.save_exploration_artifact(&artifact)?;
        assert_eq!(
            database.exploration_artifact("lens/focus")?,
            Some(artifact.clone())
        );

        assert!(database.delete_exploration_artifact("lens/focus")?);
        assert_eq!(database.exploration_artifact("lens/focus")?, None);
        assert!(database.list_exploration_artifacts()?.is_empty());
        assert!(!database.delete_exploration_artifact("lens/focus")?);

        Ok(())
    }

    #[test]
    fn exploration_artifact_operations_reject_padded_artifact_ids() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;

        let padded = lens_view_artifact(" focus ", "Padded", None);
        let save_error = database
            .save_exploration_artifact(&padded)
            .expect_err("padded artifact_id should be rejected on save");
        assert_eq!(
            save_error.to_string(),
            "exploration artifact is invalid: artifact_id must not have leading or trailing whitespace"
        );

        let load_error = database
            .exploration_artifact(" focus ")
            .expect_err("padded artifact_id should be rejected on load");
        assert_eq!(
            load_error.to_string(),
            "artifact_id must not have leading or trailing whitespace"
        );

        let delete_error = database
            .delete_exploration_artifact(" focus ")
            .expect_err("padded artifact_id should be rejected on delete");
        assert_eq!(
            delete_error.to_string(),
            "artifact_id must not have leading or trailing whitespace"
        );

        assert!(database.list_exploration_artifacts()?.is_empty());

        Ok(())
    }

    #[test]
    fn exploration_artifacts_survive_fresh_open_and_schema_rebuild() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let artifact = lens_view_artifact(
            "focus-trail",
            "Focus Trail",
            Some("Persist across sessions".to_owned()),
        );

        {
            let database = Database::open(&db_path)?;
            database.save_exploration_artifact(&artifact)?;
        }

        {
            let database = Database::open(&db_path)?;
            assert_eq!(
                database.exploration_artifact("focus-trail")?,
                Some(artifact.clone())
            );
            database
                .connection
                .execute_batch("PRAGMA user_version = 0;")?;
        }

        let database = Database::open(&db_path)?;
        assert_eq!(
            database.exploration_artifact("focus-trail")?,
            Some(artifact)
        );
        assert!(
            ExplorationArtifactStore::for_database_path(&db_path)
                .root
                .join(ARTIFACT_STORE_LAYOUT_VERSION)
                .exists()
        );

        Ok(())
    }

    #[test]
    fn exploration_artifacts_do_not_pollute_note_surfaces() -> Result<()> {
        let (_workspace, database, root) = indexed_database(&[
            (
                "focus.org",
                "#+title: Focus\n:PROPERTIES:\n:ID: focus\n:END:\n* TODO Follow up\n",
            ),
            (
                "neighbor.org",
                "#+title: Neighbor\n:PROPERTIES:\n:ID: neighbor\n:END:\n* DONE Review\n",
            ),
        ])?;
        let before_notes = database.search_nodes("", 20, None)?;
        let before_anchors = database.search_anchors("", 20, None)?;
        let before_files = database.indexed_files()?;

        database.save_exploration_artifact(&lens_view_artifact(
            "workbench/focus",
            "Workbench Artifact",
            Some("Should not leak into note discovery".to_owned()),
        ))?;

        assert_eq!(database.search_nodes("", 20, None)?, before_notes);
        assert_eq!(database.search_anchors("", 20, None)?, before_anchors);
        assert_eq!(database.indexed_files()?, before_files);
        assert!(
            database
                .search_nodes("Workbench Artifact", 20, None)?
                .is_empty()
        );
        assert_eq!(
            database
                .list_exploration_artifacts()?
                .into_iter()
                .map(|artifact| artifact.metadata.artifact_id)
                .collect::<Vec<_>>(),
            vec!["workbench/focus".to_owned()]
        );
        let db_path = root
            .parent()
            .expect("workspace parent")
            .join("index.sqlite3");
        assert!(
            ExplorationArtifactStore::for_database_path(&db_path)
                .root
                .join(ARTIFACT_STORE_LAYOUT_VERSION)
                .exists()
        );

        Ok(())
    }

    fn lens_view_artifact(
        artifact_id: &str,
        title: &str,
        summary: Option<String>,
    ) -> SavedExplorationArtifact {
        SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: artifact_id.to_owned(),
                title: title.to_owned(),
                summary,
            },
            payload: ExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "note:focus".to_owned(),
                    current_node_key: "note:focus".to_owned(),
                    lens: ExplorationLens::Structure,
                    limit: 32,
                    unique: false,
                    frozen_context: false,
                }),
            },
        }
    }
}
