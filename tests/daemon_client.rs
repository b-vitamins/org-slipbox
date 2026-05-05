use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use slipbox_core::{
    AnchorRecord, BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID, CompareNotesParams, CorpusAuditEntry,
    CorpusAuditKind, DanglingLinkAuditRecord, ExecuteExplorationArtifactResult,
    ExplorationArtifactIdParams, ExplorationArtifactMetadata, ExplorationArtifactPayload,
    ExplorationLens, ExploreParams, NodeFromIdParams, NodeFromRefParams,
    NodeFromTitleOrAliasParams, NodeKind, ReviewFinding, ReviewFindingPayload, ReviewFindingStatus,
    ReviewRun, ReviewRunDiffParams, ReviewRunIdParams, ReviewRunMetadata, ReviewRunPayload,
    RunWorkflowParams, SaveCorpusAuditReviewParams, SaveExplorationArtifactParams,
    SaveReviewRunParams, SaveWorkflowReviewParams, SavedExplorationArtifact, SavedLensViewArtifact,
    SearchNodesParams, WorkflowIdParams, WorkflowInputAssignment, WorkflowResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonServeConfig};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::{TempDir, tempdir};

fn daemon_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

fn build_indexed_fixture() -> Result<(TempDir, PathBuf, PathBuf, String)> {
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

* TODO Anonymous Follow Up
SCHEDULED: <2026-05-04 Mon>
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;
    let anonymous_anchor_key = database
        .anchor_at_point("beta.org", 13)?
        .context("anonymous heading anchor should exist")?
        .node_key;

    Ok((workspace, root, db, anonymous_anchor_key))
}

fn sample_review_run() -> ReviewRun {
    ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/dangling-links".to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: Some("Review dangling links".to_owned()),
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
                        source: AnchorRecord {
                            node_key: "file:source.org".to_owned(),
                            explicit_id: Some("source-id".to_owned()),
                            file_path: "source.org".to_owned(),
                            title: "Source".to_owned(),
                            outline_path: "Source".to_owned(),
                            aliases: Vec::new(),
                            tags: Vec::new(),
                            refs: Vec::new(),
                            todo_keyword: None,
                            scheduled_for: None,
                            deadline_for: None,
                            closed_at: None,
                            level: 0,
                            line: 1,
                            kind: NodeKind::File,
                            file_mtime_ns: 0,
                            backlink_count: 0,
                            forward_link_count: 0,
                        },
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

#[test]
fn daemon_client_queries_spawned_daemon_and_round_trips_artifacts() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;
    let canonical_root = root.canonicalize()?;
    let mut client = DaemonClient::spawn(daemon_binary(), &DaemonServeConfig::new(&root, &db))?;

    let ping = client.ping()?;
    assert_eq!(ping.root, canonical_root.display().to_string());
    assert_eq!(ping.db, db.display().to_string());

    let status = client.status()?;
    assert_eq!(status.files_indexed, 2);
    assert_eq!(status.nodes_indexed, 4);

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

    let workflows = client.list_workflows()?;
    assert_eq!(workflows.workflows.len(), 3);

    let comparison_workflow: WorkflowResult = client.workflow(&WorkflowIdParams {
        workflow_id: BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID.to_owned(),
    })?;
    assert_eq!(
        comparison_workflow.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID
    );
    assert_eq!(comparison_workflow.workflow.inputs.len(), 2);

    let workflow_run = client.run_workflow(&RunWorkflowParams {
        workflow_id: BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID.to_owned(),
        inputs: vec![
            WorkflowInputAssignment {
                input_id: "left".to_owned(),
                target: slipbox_core::WorkflowResolveTarget::NodeKey {
                    node_key: alpha.node_key.clone(),
                },
            },
            WorkflowInputAssignment {
                input_id: "right".to_owned(),
                target: slipbox_core::WorkflowResolveTarget::NodeKey {
                    node_key: beta.node_key.clone(),
                },
            },
        ],
    })?;
    assert_eq!(workflow_run.result.steps.len(), 4);
    assert_eq!(workflow_run.result.steps[2].kind().label(), "compare");

    let unresolved_workflow_run = client.run_workflow(&RunWorkflowParams {
        workflow_id: slipbox_core::BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID.to_owned(),
        inputs: vec![WorkflowInputAssignment {
            input_id: "focus".to_owned(),
            target: slipbox_core::WorkflowResolveTarget::NodeKey {
                node_key: anonymous_anchor_key.clone(),
            },
        }],
    })?;
    match &unresolved_workflow_run.result.steps[2].payload {
        slipbox_core::WorkflowStepReportPayload::Explore {
            focus_node_key,
            result,
        } => {
            assert_eq!(focus_node_key, &anonymous_anchor_key);
            assert_eq!(result.lens, ExplorationLens::Tasks);
        }
        other => panic!("expected tasks explore step, got {:?}", other.kind()),
    }

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
        overwrite: true,
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

    let review = sample_review_run();
    let saved_review = client.save_review_run(&SaveReviewRunParams {
        review: review.clone(),
        overwrite: true,
    })?;
    assert_eq!(
        saved_review.review.metadata.review_id,
        "review/audit/dangling-links"
    );
    assert_eq!(saved_review.review.status_counts.open, 1);

    let listed_reviews = client.list_review_runs()?;
    assert_eq!(listed_reviews.reviews, vec![saved_review.review.clone()]);

    let loaded_review = client.review_run(&ReviewRunIdParams {
        review_id: "review/audit/dangling-links".to_owned(),
    })?;
    assert_eq!(loaded_review.review, review);

    let mut target_review = review.clone();
    target_review.metadata.review_id = "review/audit/dangling-links/target".to_owned();
    target_review.findings[0].status = ReviewFindingStatus::Reviewed;
    let saved_target_review = client.save_review_run(&SaveReviewRunParams {
        review: target_review,
        overwrite: true,
    })?;
    assert_eq!(
        saved_target_review.review.metadata.review_id,
        "review/audit/dangling-links/target"
    );
    let diff = client.diff_review_runs(&ReviewRunDiffParams {
        base_review_id: "review/audit/dangling-links".to_owned(),
        target_review_id: "review/audit/dangling-links/target".to_owned(),
    })?;
    assert!(diff.diff.added.is_empty());
    assert!(diff.diff.removed.is_empty());
    assert!(diff.diff.unchanged.is_empty());
    assert_eq!(diff.diff.status_changed.len(), 1);
    assert_eq!(
        diff.diff.status_changed[0].finding_id,
        "audit/dangling-links/source/missing-id"
    );

    let marked = client.mark_review_finding(&slipbox_core::MarkReviewFindingParams {
        review_id: "review/audit/dangling-links".to_owned(),
        finding_id: "audit/dangling-links/source/missing-id".to_owned(),
        status: ReviewFindingStatus::Reviewed,
    })?;
    assert_eq!(marked.transition.from_status, ReviewFindingStatus::Open);
    assert_eq!(marked.transition.to_status, ReviewFindingStatus::Reviewed);

    let marked_review = client.review_run(&ReviewRunIdParams {
        review_id: "review/audit/dangling-links".to_owned(),
    })?;
    assert_eq!(
        marked_review.review.findings[0].status,
        ReviewFindingStatus::Reviewed
    );

    let deleted_review = client.delete_review_run(&ReviewRunIdParams {
        review_id: "review/audit/dangling-links".to_owned(),
    })?;
    assert_eq!(deleted_review.review_id, "review/audit/dangling-links");
    let deleted_target_review = client.delete_review_run(&ReviewRunIdParams {
        review_id: "review/audit/dangling-links/target".to_owned(),
    })?;
    assert_eq!(
        deleted_target_review.review_id,
        "review/audit/dangling-links/target"
    );
    assert!(client.list_review_runs()?.reviews.is_empty());

    let audit_review = client.save_corpus_audit_review(&SaveCorpusAuditReviewParams {
        audit: CorpusAuditKind::WeaklyIntegratedNotes,
        limit: 20,
        review_id: Some("review/audit/weakly-integrated-notes".to_owned()),
        title: Some("Weak Integration Review".to_owned()),
        summary: None,
        overwrite: true,
    })?;
    assert_eq!(
        audit_review.result.audit,
        CorpusAuditKind::WeaklyIntegratedNotes
    );
    assert_eq!(
        audit_review.review.metadata.review_id,
        "review/audit/weakly-integrated-notes"
    );

    let workflow_review = client.save_workflow_review(&SaveWorkflowReviewParams {
        workflow_id: slipbox_core::BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID.to_owned(),
        inputs: vec![WorkflowInputAssignment {
            input_id: "focus".to_owned(),
            target: slipbox_core::WorkflowResolveTarget::NodeKey {
                node_key: anonymous_anchor_key,
            },
        }],
        review_id: Some("review/workflow/unresolved-sweep".to_owned()),
        title: Some("Unresolved Sweep Review".to_owned()),
        summary: None,
        overwrite: true,
    })?;
    assert_eq!(
        workflow_review.result.workflow.metadata.workflow_id,
        slipbox_core::BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
    );
    assert_eq!(
        workflow_review.review.finding_count,
        workflow_review.result.steps.len()
    );

    let loaded_workflow_review = client.review_run(&ReviewRunIdParams {
        review_id: "review/workflow/unresolved-sweep".to_owned(),
    })?;
    match loaded_workflow_review.review.payload {
        ReviewRunPayload::Workflow {
            workflow,
            inputs,
            step_ids,
        } => {
            assert_eq!(
                workflow.metadata.workflow_id,
                slipbox_core::BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
            );
            assert_eq!(inputs.len(), 1);
            assert_eq!(step_ids.len(), workflow_review.result.steps.len());
        }
        other => panic!("expected workflow review payload, got {:?}", other.kind()),
    }

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
