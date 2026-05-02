use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use slipbox_core::{
    CompareNotesParams, ExecuteExplorationArtifactResult, ExplorationArtifactIdParams,
    ExplorationArtifactMetadata, ExplorationArtifactPayload, ExplorationLens, ExploreParams,
    NodeFromIdParams, NodeFromRefParams, NodeFromTitleOrAliasParams, SaveExplorationArtifactParams,
    SavedExplorationArtifact, SavedLensViewArtifact, SearchNodesParams,
};
use slipbox_daemon_client::{DaemonClient, DaemonServeConfig};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::{TempDir, tempdir};

fn daemon_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

fn build_indexed_fixture() -> Result<(TempDir, PathBuf, PathBuf)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        r#":PROPERTIES:
:ID: alpha-id
:ROAM_REFS: cite:shared2024 cite:alpha2024
:END:
#+title: Alpha

See [[id:beta-id][Beta]].
"#,
    )?;
    fs::write(
        root.join("beta.org"),
        r#":PROPERTIES:
:ID: beta-id
:ROAM_REFS: cite:shared2024 cite:beta2024
:END:
#+title: Beta

* TODO Follow Up
:PROPERTIES:
:ID: beta-task-id
:END:
SCHEDULED: <2026-05-03 Sun>
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;

    Ok((workspace, root, db))
}

#[test]
fn daemon_client_queries_spawned_daemon_and_round_trips_artifacts() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;
    let canonical_root = root.canonicalize()?;
    let mut client = DaemonClient::spawn(daemon_binary(), &DaemonServeConfig::new(&root, &db))?;

    let ping = client.ping()?;
    assert_eq!(ping.root, canonical_root.display().to_string());
    assert_eq!(ping.db, db.display().to_string());

    let status = client.status()?;
    assert_eq!(status.files_indexed, 2);
    assert_eq!(status.nodes_indexed, 3);

    let alpha = client
        .search_nodes(&SearchNodesParams {
            query: "Alpha".to_owned(),
            limit: 10,
            sort: None,
        })?
        .nodes
        .into_iter()
        .find(|node| node.title == "Alpha")
        .context("Alpha note should resolve from search")?;
    let beta = client
        .node_from_id(&NodeFromIdParams {
            id: "beta-id".to_owned(),
        })?
        .context("Beta note should resolve by ID")?;

    let beta_from_title = client
        .node_from_title_or_alias(&NodeFromTitleOrAliasParams {
            title_or_alias: "Beta".to_owned(),
            nocase: false,
        })?
        .context("Beta note should resolve by title")?;
    assert_eq!(beta_from_title.node_key, beta.node_key);

    let beta_from_ref = client
        .node_from_ref(&NodeFromRefParams {
            reference: "cite:beta2024".to_owned(),
        })?
        .context("Beta note should resolve by unique ref")?;
    assert_eq!(beta_from_ref.node_key, beta.node_key);

    let beta_task = client
        .node_at_point(&slipbox_core::NodeAtPointParams {
            file_path: root.join("beta.org").display().to_string(),
            line: 8,
        })?
        .context("Follow Up heading should resolve at point")?;
    assert_eq!(beta_task.title, "Follow Up");

    let explore = client.explore(&ExploreParams {
        node_key: alpha.node_key.clone(),
        lens: ExplorationLens::Structure,
        limit: 10,
        unique: false,
    })?;
    let forward_section = explore
        .sections
        .iter()
        .find(|section| section.kind == slipbox_core::ExplorationSectionKind::ForwardLinks)
        .context("structure lens should include forward links")?;
    assert_eq!(forward_section.entries.len(), 1);

    let comparison = client.compare_notes(&CompareNotesParams {
        left_node_key: alpha.node_key.clone(),
        right_node_key: beta.node_key.clone(),
        limit: 10,
    })?;
    assert_eq!(comparison.left_note.title, "Alpha");
    assert_eq!(comparison.right_note.title, "Beta");
    assert!(
        comparison
            .sections
            .iter()
            .any(|section| { section.kind == slipbox_core::NoteComparisonSectionKind::SharedRefs })
    );

    let saved = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "alpha-structure".to_owned(),
            title: "Alpha structure".to_owned(),
            summary: Some("Forward structure for Alpha".to_owned()),
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: alpha.node_key.clone(),
                current_node_key: alpha.node_key.clone(),
                lens: ExplorationLens::Structure,
                limit: 10,
                unique: false,
                frozen_context: false,
            }),
        },
    };

    let save = client.save_exploration_artifact(&SaveExplorationArtifactParams {
        artifact: saved.clone(),
    })?;
    assert_eq!(save.artifact.metadata.artifact_id, "alpha-structure");

    let list = client.list_exploration_artifacts()?;
    assert_eq!(list.artifacts.len(), 1);
    assert_eq!(list.artifacts[0].metadata.artifact_id, "alpha-structure");

    let loaded = client.exploration_artifact(&ExplorationArtifactIdParams {
        artifact_id: "alpha-structure".to_owned(),
    })?;
    assert_eq!(loaded.artifact, saved);

    let executed = client.execute_exploration_artifact(&ExplorationArtifactIdParams {
        artifact_id: "alpha-structure".to_owned(),
    })?;
    match executed {
        ExecuteExplorationArtifactResult {
            artifact:
                slipbox_core::ExecutedExplorationArtifact {
                    payload:
                        slipbox_core::ExecutedExplorationArtifactPayload::LensView { result, .. },
                    ..
                },
        } => {
            assert_eq!(result.lens, ExplorationLens::Structure);
            assert!(result.sections.iter().any(|section| {
                section.kind == slipbox_core::ExplorationSectionKind::ForwardLinks
            }));
        }
        other => panic!("unexpected executed artifact payload: {other:?}"),
    }

    let deleted = client.delete_exploration_artifact(&ExplorationArtifactIdParams {
        artifact_id: "alpha-structure".to_owned(),
    })?;
    assert_eq!(deleted.artifact_id, "alpha-structure");
    assert!(client.list_exploration_artifacts()?.artifacts.is_empty());

    client.shutdown()?;
    Ok(())
}

#[test]
fn daemon_client_can_attach_to_a_spawned_daemon_child() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(root.join("gamma.org"), "#+title: Gamma\n")?;
    let db = workspace.path().join("slipbox.sqlite");

    let child = Command::new(daemon_binary())
        .arg("serve")
        .arg("--root")
        .arg(&root)
        .arg("--db")
        .arg(&db)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut client = DaemonClient::from_child(child)?;

    let ping = client.ping()?;
    assert_eq!(ping.root, root.canonicalize()?.display().to_string());

    client.shutdown()?;
    Ok(())
}
