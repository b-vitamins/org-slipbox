use std::path::Path;

use anyhow::Result;
use slipbox_core::SavedExplorationArtifact;

use crate::{
    Database,
    json_store::{JsonFileStore, JsonStoreSpec},
};

const ARTIFACT_STORE_DIR_SUFFIX: &str = ".exploration-artifacts";
const ARTIFACT_STORE_LAYOUT_VERSION: &str = "v1";
const ARTIFACT_FILE_EXTENSION: &str = "json";

pub(crate) struct ExplorationArtifactStore {
    store: JsonFileStore<SavedExplorationArtifact>,
}

impl ExplorationArtifactStore {
    pub(crate) fn for_database_path(path: &Path) -> Self {
        Self {
            store: JsonFileStore::for_database_path(
                path,
                JsonStoreSpec {
                    directory_suffix: ARTIFACT_STORE_DIR_SUFFIX,
                    layout_version: ARTIFACT_STORE_LAYOUT_VERSION,
                    file_extension: ARTIFACT_FILE_EXTENSION,
                    item_name: "exploration artifact",
                    plural_name: "exploration artifacts",
                    id_field_name: "artifact_id",
                    default_database_file_name: "index.sqlite3",
                },
            ),
        }
    }

    pub(crate) fn migrate(&self) -> Result<()> {
        self.store.migrate()
    }

    #[cfg(test)]
    fn version_dir(&self) -> std::path::PathBuf {
        self.store.version_dir()
    }

    fn save(&self, artifact: &SavedExplorationArtifact) -> Result<()> {
        self.store.save(
            &artifact.metadata.artifact_id,
            artifact,
            SavedExplorationArtifact::validation_error,
        )
    }

    fn save_if_absent(&self, artifact: &SavedExplorationArtifact) -> Result<bool> {
        self.store.save_if_absent(
            &artifact.metadata.artifact_id,
            artifact,
            SavedExplorationArtifact::validation_error,
        )
    }

    fn load(&self, artifact_id: &str) -> Result<Option<SavedExplorationArtifact>> {
        self.store.load(
            artifact_id,
            |artifact| artifact.metadata.artifact_id.as_str(),
            SavedExplorationArtifact::validation_error,
        )
    }

    fn list(&self) -> Result<Vec<SavedExplorationArtifact>> {
        let mut artifacts = self.store.list(
            |artifact| artifact.metadata.artifact_id.as_str(),
            SavedExplorationArtifact::validation_error,
        )?;
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
        self.store.delete(artifact_id)
    }
}

impl Database {
    pub fn save_exploration_artifact(&self, artifact: &SavedExplorationArtifact) -> Result<()> {
        self.artifact_store.save(artifact)
    }

    pub fn save_exploration_artifact_if_absent(
        &self,
        artifact: &SavedExplorationArtifact,
    ) -> Result<bool> {
        self.artifact_store.save_if_absent(artifact)
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

    use super::ExplorationArtifactStore;

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
    fn save_if_absent_refuses_to_replace_existing_artifacts() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        let database = Database::open(&db_path)?;
        let original = lens_view_artifact("lens/focus", "Original", None);
        let replacement = lens_view_artifact("lens/focus", "Replacement", None);

        assert!(database.save_exploration_artifact_if_absent(&original)?);
        assert!(!database.save_exploration_artifact_if_absent(&replacement)?);
        assert_eq!(database.exploration_artifact("lens/focus")?, Some(original));
        let version_dir = ExplorationArtifactStore::for_database_path(&db_path).version_dir();
        let mut file_names = fs::read_dir(version_dir)?
            .map(|entry| entry.map(|value| value.file_name().to_string_lossy().into_owned()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        file_names.sort();
        assert_eq!(file_names, vec!["lens%2Ffocus.json".to_owned()]);

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
                .version_dir()
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
                .version_dir()
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
