use std::fs;
use std::thread::sleep;
use std::time::Duration;

use serde_json::json;
use slipbox_core::{
    AnchorRecord, AuditRemediationApplyAction, AuditRemediationConfidence,
    AuditRemediationPreviewPayload, BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID,
    BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID, BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID,
    BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID, BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID,
    CompareNotesParams, ComparisonConnectorDirection, CorpusAuditEntry, CorpusAuditKind,
    CorpusAuditResult, DanglingLinkAuditRecord, DeleteExplorationArtifactResult,
    DeleteReviewRunResult, DeleteWorkbenchPackResult, ExecuteExplorationArtifactResult,
    ExecutedExplorationArtifactPayload, ExplorationArtifactMetadata, ExplorationArtifactPayload,
    ExplorationArtifactResult, ExplorationEntry, ExplorationExplanation, ExplorationLens,
    ExplorationSectionKind, ExploreParams, ExploreResult, GraphParams, ImportWorkbenchPackResult,
    ListExplorationArtifactsResult, ListReviewRoutinesResult, ListReviewRunsResult,
    ListWorkbenchPacksResult, ListWorkflowsResult, MarkReviewFindingResult, NodeKind,
    NoteComparisonEntry, NoteComparisonExplanation, NoteComparisonGroup, NoteComparisonResult,
    NoteComparisonSectionKind, ReportJsonlLineKind, ReportProfileMetadata, ReportProfileMode,
    ReportProfileSpec, ReportProfileSubject, ReviewFinding, ReviewFindingPayload,
    ReviewFindingRemediationApplyParams, ReviewFindingRemediationApplyResult,
    ReviewFindingRemediationPreviewResult, ReviewFindingStatus, ReviewRoutineComparePolicy,
    ReviewRoutineCompareTarget, ReviewRoutineMetadata, ReviewRoutineReportLine,
    ReviewRoutineResult, ReviewRoutineSaveReviewPolicy, ReviewRoutineSource,
    ReviewRoutineSourceExecutionResult, ReviewRoutineSpec, ReviewRun, ReviewRunDiffBucket,
    ReviewRunDiffResult, ReviewRunMetadata, ReviewRunPayload, ReviewRunResult,
    RunReviewRoutineResult, RunWorkflowResult, SaveCorpusAuditReviewResult,
    SaveExplorationArtifactResult, SaveReviewRunResult, SaveWorkflowReviewResult,
    SavedComparisonArtifact, SavedExplorationArtifact, SavedLensViewArtifact, SavedTrailArtifact,
    SavedTrailStep, TrailReplayStepResult, ValidateWorkbenchPackResult, WorkbenchPackCompatibility,
    WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackMetadata, WorkbenchPackResult,
    WorkflowInputAssignment, WorkflowMetadata, WorkflowResolveTarget, WorkflowResult, WorkflowSpec,
    WorkflowSpecCompatibility, WorkflowStepPayload, WorkflowStepReport, WorkflowStepReportPayload,
    WorkflowStepSpec,
};
use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};
use tempfile::TempDir;

use super::{
    compare_notes, corpus_audit, delete_exploration_artifact, delete_review_run,
    delete_workbench_pack, diff_review_runs, execute_compare_notes_query,
    execute_exploration_artifact, execute_explore_query, execute_saved_exploration_artifact,
    execute_saved_exploration_artifact_by_id, execute_workflow_spec, exploration_artifact, explore,
    export_workbench_pack, import_workbench_pack, list_exploration_artifacts, list_review_routines,
    list_review_runs, list_workbench_packs, list_workflows, mark_review_finding,
    review_finding_remediation_apply, review_finding_remediation_preview, review_routine,
    review_run, run_review_routine, run_workflow, save_corpus_audit_review,
    save_exploration_artifact, save_review_run, save_workflow_review, validate_workbench_pack,
    workbench_pack, workflow,
};
use crate::server::state::ServerState;

#[test]
fn explore_dispatches_declared_lenses() {
    let (_workspace, mut state, target_key) = indexed_state();

    let structure: ExploreResult = serde_json::from_value(
        explore(
            &mut state,
            json!({
                "node_key": target_key.as_str(),
                "lens": "structure",
                "limit": 20
            }),
        )
        .expect("structure lens should succeed"),
    )
    .expect("structure result should deserialize");
    assert_eq!(
        structure
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            ExplorationSectionKind::Backlinks,
            ExplorationSectionKind::ForwardLinks
        ]
    );
    assert!(!structure.sections[0].entries.is_empty());

    let refs: ExploreResult = serde_json::from_value(
        explore(
            &mut state,
            json!({
                "node_key": target_key.as_str(),
                "lens": "refs",
                "limit": 20
            }),
        )
        .expect("refs lens should succeed"),
    )
    .expect("refs result should deserialize");
    assert_eq!(
        refs.sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            ExplorationSectionKind::Reflinks,
            ExplorationSectionKind::UnlinkedReferences
        ]
    );
    assert!(
        refs.sections[0]
            .entries
            .iter()
            .any(|entry| matches!(entry, ExplorationEntry::Reflink { .. }))
    );
    assert!(
        refs.sections[1]
            .entries
            .iter()
            .any(|entry| matches!(entry, ExplorationEntry::UnlinkedReference { .. }))
    );

    let time: ExploreResult = serde_json::from_value(
        explore(
            &mut state,
            json!({
                "node_key": target_key.as_str(),
                "lens": "time",
                "limit": 20
            }),
        )
        .expect("time lens should succeed"),
    )
    .expect("time result should deserialize");
    assert_eq!(time.sections.len(), 1);
    assert_eq!(time.sections[0].kind, ExplorationSectionKind::TimeNeighbors);
    assert!(
        time.sections[0]
            .entries
            .iter()
            .any(|entry| matches!(entry, ExplorationEntry::Anchor { .. }))
    );

    let tasks: ExploreResult = serde_json::from_value(
        explore(
            &mut state,
            json!({
                "node_key": target_key.as_str(),
                "lens": "tasks",
                "limit": 20
            }),
        )
        .expect("tasks lens should succeed"),
    )
    .expect("tasks result should deserialize");
    assert_eq!(tasks.sections.len(), 1);
    assert_eq!(
        tasks.sections[0].kind,
        ExplorationSectionKind::TaskNeighbors
    );
    assert!(
        tasks.sections[0]
            .entries
            .iter()
            .any(|entry| matches!(entry, ExplorationEntry::Anchor { .. }))
    );
}

#[test]
fn explore_rejects_unique_outside_structure() {
    let (_workspace, mut state, target_key) = indexed_state();

    let error = explore(
        &mut state,
        json!({
            "node_key": target_key.as_str(),
            "lens": "refs",
            "limit": 20,
            "unique": true
        }),
    )
    .expect_err("refs lens should reject unique");

    assert_eq!(
        error.into_inner().message,
        "explore unique is only supported for the structure lens"
    );
}

#[test]
fn compare_notes_dispatches_structured_sections() {
    let (_workspace, mut state, left_key, right_key) = comparison_state();

    let comparison: NoteComparisonResult = serde_json::from_value(
        compare_notes(
            &mut state,
            json!({
                "left_node_key": left_key.as_str(),
                "right_node_key": right_key.as_str(),
                "limit": 20
            }),
        )
        .expect("comparison should succeed"),
    )
    .expect("comparison result should deserialize");

    assert_eq!(
        comparison
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            NoteComparisonSectionKind::SharedRefs,
            NoteComparisonSectionKind::SharedPlanningDates,
            NoteComparisonSectionKind::LeftOnlyRefs,
            NoteComparisonSectionKind::RightOnlyRefs,
            NoteComparisonSectionKind::SharedBacklinks,
            NoteComparisonSectionKind::SharedForwardLinks,
            NoteComparisonSectionKind::ContrastingTaskStates,
            NoteComparisonSectionKind::PlanningTensions,
            NoteComparisonSectionKind::IndirectConnectors,
        ]
    );

    assert!(comparison.sections[0].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::Reference { record }
        if record.reference == "@shared2024"
            && record.explanation == NoteComparisonExplanation::SharedReference
    )));
    assert!(comparison.sections[1].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::PlanningRelation { record }
        if record.date == "2026-05-01T00:00:00"
            && record.explanation == NoteComparisonExplanation::SharedPlanningDate
    )));
    assert!(comparison.sections[2].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::Reference { record }
        if record.reference == "@left2024"
            && record.explanation == NoteComparisonExplanation::LeftOnlyReference
    )));
    assert!(comparison.sections[3].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::Reference { record }
        if record.reference == "@right2024"
            && record.explanation == NoteComparisonExplanation::RightOnlyReference
    )));
    assert!(comparison.sections[4].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::Node { record }
        if record.node.title == "Shared Backlink"
            && record.explanation == NoteComparisonExplanation::SharedBacklink
    )));
    assert!(comparison.sections[5].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::Node { record }
        if record.node.title == "Shared Forward"
            && record.explanation == NoteComparisonExplanation::SharedForwardLink
    )));
    assert!(comparison.sections[6].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::TaskState { record }
        if record.left_todo_keyword == "TODO"
            && record.right_todo_keyword == "NEXT"
            && record.explanation == NoteComparisonExplanation::ContrastingTaskState
    )));
    assert!(comparison.sections[7].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::PlanningRelation { record }
        if record.date == "2026-05-01T00:00:00"
            && record.explanation == NoteComparisonExplanation::PlanningTension
    )));
    assert!(comparison.sections[8].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::Node { record }
        if record.node.title == "Left To Right Bridge"
            && record.explanation == NoteComparisonExplanation::IndirectConnector {
                direction: ComparisonConnectorDirection::LeftToRight,
            }
    )));
    assert!(comparison.sections[8].entries.iter().any(|entry| matches!(
        entry,
        NoteComparisonEntry::Node { record }
        if record.node.title == "Right To Left Bridge"
            && record.explanation == NoteComparisonExplanation::IndirectConnector {
                direction: ComparisonConnectorDirection::RightToLeft,
            }
    )));
}

#[test]
fn explore_dispatches_non_obvious_lenses() {
    let (_workspace, mut state, focus_key) = non_obvious_state();

    let bridges: ExploreResult = serde_json::from_value(
        explore(
            &mut state,
            json!({
                "node_key": focus_key.as_str(),
                "lens": "bridges",
                "limit": 20
            }),
        )
        .expect("bridges lens should succeed"),
    )
    .expect("bridges result should deserialize");
    assert_eq!(bridges.sections.len(), 1);
    assert_eq!(
        bridges.sections[0].kind,
        ExplorationSectionKind::BridgeCandidates
    );
    assert!(bridges.sections[0].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Anchor { record }
        if record.anchor.title == "Dormant Bridge"
            && matches!(
                record.explanation,
                ExplorationExplanation::BridgeCandidate { ref references, ref via_notes }
                if references == &vec!["@shared2024".to_owned()]
                    && via_notes.len() == 1
                    && via_notes[0].title == "Neighbor"
                    && via_notes[0].explicit_id.as_deref() == Some("neighbor-id")
            )
    )));

    let dormant: ExploreResult = serde_json::from_value(
        explore(
            &mut state,
            json!({
                "node_key": focus_key.as_str(),
                "lens": "dormant",
                "limit": 20
            }),
        )
        .expect("dormant lens should succeed"),
    )
    .expect("dormant result should deserialize");
    assert_eq!(dormant.sections.len(), 1);
    assert_eq!(
        dormant.sections[0].kind,
        ExplorationSectionKind::DormantNotes
    );
    assert!(dormant.sections[0].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Anchor { record }
        if record.anchor.title == "Dormant Bridge"
            && matches!(
                record.explanation,
                ExplorationExplanation::DormantSharedReference { ref references, .. }
                if references == &vec!["@shared2024".to_owned()]
            )
    )));

    let unresolved: ExploreResult = serde_json::from_value(
        explore(
            &mut state,
            json!({
                "node_key": focus_key.as_str(),
                "lens": "unresolved",
                "limit": 20
            }),
        )
        .expect("unresolved lens should succeed"),
    )
    .expect("unresolved result should deserialize");
    assert_eq!(
        unresolved
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            ExplorationSectionKind::UnresolvedTasks,
            ExplorationSectionKind::WeaklyIntegratedNotes,
        ]
    );
    assert!(unresolved.sections[0].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Anchor { record }
        if record.anchor.title == "Unresolved Thread"
            && record.explanation == ExplorationExplanation::UnresolvedSharedReference {
                references: vec!["@shared2024".to_owned()],
                todo_keyword: "TODO".to_owned(),
            }
    )));
    assert!(unresolved.sections[1].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Anchor { record }
        if record.anchor.title == "Weak Thread"
            && record.explanation
                == ExplorationExplanation::WeaklyIntegratedSharedReference {
                    references: vec!["@shared2024".to_owned()],
                    structural_link_count: 0,
                }
    )));
}

#[test]
fn saved_lens_artifacts_execute_like_live_queries() {
    let (_workspace, mut state, target_key) = indexed_state();

    let cases = [
        (
            "saved-structure",
            saved_lens_artifact(
                "saved-structure",
                "Saved Structure",
                &target_key,
                ExplorationLens::Structure,
            ),
            ExploreParams {
                node_key: target_key.clone(),
                lens: ExplorationLens::Structure,
                limit: 20,
                unique: false,
            },
        ),
        (
            "saved-refs",
            saved_lens_artifact(
                "saved-refs",
                "Saved Refs",
                &target_key,
                ExplorationLens::Refs,
            ),
            ExploreParams {
                node_key: target_key.clone(),
                lens: ExplorationLens::Refs,
                limit: 20,
                unique: false,
            },
        ),
        (
            "saved-time",
            saved_lens_artifact(
                "saved-time",
                "Saved Time",
                &target_key,
                ExplorationLens::Time,
            ),
            ExploreParams {
                node_key: target_key.clone(),
                lens: ExplorationLens::Time,
                limit: 20,
                unique: false,
            },
        ),
        (
            "saved-tasks",
            saved_lens_artifact(
                "saved-tasks",
                "Saved Tasks",
                &target_key,
                ExplorationLens::Tasks,
            ),
            ExploreParams {
                node_key: target_key.clone(),
                lens: ExplorationLens::Tasks,
                limit: 20,
                unique: false,
            },
        ),
    ];

    for (artifact_id, artifact, params) in cases {
        state
            .database
            .save_exploration_artifact(&artifact)
            .expect("artifact should save");
        let live = execute_explore_query(&mut state, &params).expect("live explore should succeed");
        let executed = execute_saved_exploration_artifact_by_id(&mut state, artifact_id)
            .expect("saved artifact execution should succeed")
            .expect("saved artifact should exist");

        assert_eq!(executed.metadata, artifact.metadata);
        match executed.payload {
            ExecutedExplorationArtifactPayload::LensView {
                artifact: executed_artifact,
                result,
                ..
            } => {
                match artifact.payload {
                    ExplorationArtifactPayload::LensView { artifact } => {
                        assert_eq!(executed_artifact, artifact);
                    }
                    _ => panic!("expected saved lens-view artifact"),
                }
                assert_eq!(*result, live);
            }
            payload => panic!("expected lens-view execution, got {:?}", payload.kind()),
        }
    }
}

#[test]
fn saved_non_obvious_lens_artifacts_execute_like_live_queries() {
    let (_workspace, mut state, focus_key) = non_obvious_state();

    let cases = [
        ("saved-bridges", ExplorationLens::Bridges),
        ("saved-dormant", ExplorationLens::Dormant),
        ("saved-unresolved", ExplorationLens::Unresolved),
    ];

    for (artifact_id, lens) in cases {
        let artifact = saved_lens_artifact(artifact_id, artifact_id, &focus_key, lens);
        state
            .database
            .save_exploration_artifact(&artifact)
            .expect("artifact should save");
        let live = execute_explore_query(
            &mut state,
            &ExploreParams {
                node_key: focus_key.clone(),
                lens,
                limit: 20,
                unique: false,
            },
        )
        .expect("live non-obvious explore should succeed");
        let executed = execute_saved_exploration_artifact_by_id(&mut state, artifact_id)
            .expect("saved artifact execution should succeed")
            .expect("saved artifact should exist");

        match executed.payload {
            ExecutedExplorationArtifactPayload::LensView { result, .. } => {
                assert_eq!(*result, live);
            }
            payload => panic!("expected lens-view execution, got {:?}", payload.kind()),
        }
    }
}

#[test]
fn saved_comparison_artifact_executes_like_live_queries() {
    let (_workspace, mut state, left_key, right_key) = comparison_state();
    let artifact = saved_comparison_artifact(
        "saved-comparison",
        "Saved Comparison",
        &left_key,
        &right_key,
    );
    state
        .database
        .save_exploration_artifact(&artifact)
        .expect("artifact should save");

    let live = execute_compare_notes_query(
        &mut state,
        &CompareNotesParams {
            left_node_key: left_key.clone(),
            right_node_key: right_key.clone(),
            limit: 20,
        },
    )
    .expect("live comparison should succeed");
    let executed = execute_saved_exploration_artifact_by_id(&mut state, "saved-comparison")
        .expect("saved comparison should execute")
        .expect("saved comparison should exist");

    assert_eq!(executed.metadata, artifact.metadata);
    match executed.payload {
        ExecutedExplorationArtifactPayload::Comparison {
            artifact: executed_artifact,
            result,
            ..
        } => {
            match artifact.payload {
                ExplorationArtifactPayload::Comparison { artifact } => {
                    assert_eq!(executed_artifact, artifact);
                }
                _ => panic!("expected saved comparison artifact"),
            }
            assert_eq!(*result, live);
        }
        payload => panic!("expected comparison execution, got {:?}", payload.kind()),
    }
}

#[test]
fn saved_trail_artifacts_replay_live_step_results() {
    let (_workspace, mut state, left_key, right_key) = comparison_state();
    let lens_step = SavedLensViewArtifact {
        root_node_key: left_key.clone(),
        current_node_key: left_key.clone(),
        lens: ExplorationLens::Structure,
        limit: 20,
        unique: false,
        frozen_context: false,
    };
    let comparison_step = SavedComparisonArtifact {
        root_node_key: left_key.clone(),
        left_node_key: left_key.clone(),
        right_node_key: right_key.clone(),
        active_lens: ExplorationLens::Structure,
        structure_unique: false,
        comparison_group: Default::default(),
        limit: 20,
        frozen_context: false,
    };
    let detached_step = SavedLensViewArtifact {
        root_node_key: right_key.clone(),
        current_node_key: right_key.clone(),
        lens: ExplorationLens::Structure,
        limit: 20,
        unique: false,
        frozen_context: false,
    };
    let artifact = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "saved-trail".to_owned(),
            title: "Saved Trail".to_owned(),
            summary: Some("Mixed trail replay".to_owned()),
        },
        payload: ExplorationArtifactPayload::Trail {
            artifact: Box::new(SavedTrailArtifact {
                steps: vec![
                    SavedTrailStep::LensView {
                        artifact: Box::new(lens_step.clone()),
                    },
                    SavedTrailStep::Comparison {
                        artifact: Box::new(comparison_step.clone()),
                    },
                ],
                cursor: 1,
                detached_step: Some(Box::new(SavedTrailStep::LensView {
                    artifact: Box::new(detached_step.clone()),
                })),
            }),
        },
    };
    state
        .database
        .save_exploration_artifact(&artifact)
        .expect("trail artifact should save");

    let expected_lens = execute_explore_query(&mut state, &lens_step.explore_params())
        .expect("live lens replay should succeed");
    let expected_comparison =
        execute_compare_notes_query(&mut state, &comparison_step.compare_notes_params())
            .expect("live comparison replay should succeed");
    let expected_detached = execute_explore_query(&mut state, &detached_step.explore_params())
        .expect("live detached replay should succeed");

    let executed = execute_saved_exploration_artifact_by_id(&mut state, "saved-trail")
        .expect("saved trail should execute")
        .expect("saved trail should exist");

    assert_eq!(executed.metadata, artifact.metadata);
    match executed.payload {
        ExecutedExplorationArtifactPayload::Trail {
            artifact: executed_artifact,
            replay,
        } => {
            match artifact.payload {
                ExplorationArtifactPayload::Trail { artifact } => {
                    assert_eq!(executed_artifact, artifact);
                }
                _ => panic!("expected saved trail artifact"),
            }
            assert_eq!(replay.cursor, 1);
            assert_eq!(replay.steps.len(), 2);
            match &replay.steps[0] {
                TrailReplayStepResult::LensView {
                    artifact, result, ..
                } => {
                    assert_eq!(artifact.as_ref(), &lens_step);
                    assert_eq!(result.as_ref(), &expected_lens);
                }
                other => panic!(
                    "expected first replay step to be lens-view, got {:?}",
                    other
                ),
            }
            match &replay.steps[1] {
                TrailReplayStepResult::Comparison {
                    artifact, result, ..
                } => {
                    assert_eq!(artifact.as_ref(), &comparison_step);
                    assert_eq!(result.as_ref(), &expected_comparison);
                }
                other => {
                    panic!(
                        "expected second replay step to be comparison, got {:?}",
                        other
                    )
                }
            }
            match replay.detached_step.as_deref() {
                Some(TrailReplayStepResult::LensView {
                    artifact, result, ..
                }) => {
                    assert_eq!(artifact.as_ref(), &detached_step);
                    assert_eq!(result.as_ref(), &expected_detached);
                }
                other => panic!("expected detached replay step, got {:?}", other),
            }
        }
        payload => panic!("expected trail execution, got {:?}", payload.kind()),
    }
}

#[test]
fn saved_artifact_execution_returns_none_when_id_is_missing() {
    let (_workspace, mut state, target_key) = indexed_state();
    let _ = target_key;
    assert_eq!(
        execute_saved_exploration_artifact_by_id(&mut state, "missing-artifact")
            .expect("lookup should succeed"),
        None
    );
}

#[test]
fn direct_saved_artifact_execution_rejects_invalid_artifacts() {
    let (_workspace, mut state, target_key) = indexed_state();
    let invalid = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "invalid-trail".to_owned(),
            title: "Invalid Trail".to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::Trail {
            artifact: Box::new(SavedTrailArtifact {
                steps: vec![SavedTrailStep::LensView {
                    artifact: Box::new(SavedLensViewArtifact {
                        root_node_key: target_key.clone(),
                        current_node_key: target_key,
                        lens: ExplorationLens::Structure,
                        limit: 20,
                        unique: false,
                        frozen_context: false,
                    }),
                }],
                cursor: 1,
                detached_step: None,
            }),
        },
    };

    let error = execute_saved_exploration_artifact(&mut state, &invalid)
        .expect_err("direct execution should reject malformed artifacts");
    assert_eq!(
        error.into_inner().message,
        "trail cursor must point to an existing step"
    );
}

#[test]
fn artifact_rpc_operations_round_trip_saved_artifacts() {
    let (_workspace, mut state, focus_key) = non_obvious_state();
    let artifact = saved_lens_artifact(
        "saved-unresolved",
        "Saved Unresolved",
        &focus_key,
        ExplorationLens::Unresolved,
    );

    let saved: SaveExplorationArtifactResult = serde_json::from_value(
        save_exploration_artifact(
            &mut state,
            json!({ "artifact": artifact.clone(), "overwrite": true }),
        )
        .expect("save artifact RPC should succeed"),
    )
    .expect("save result should decode");
    assert_eq!(saved.artifact.metadata, artifact.metadata);
    assert_eq!(saved.artifact.kind, artifact.kind());

    let listed: ListExplorationArtifactsResult = serde_json::from_value(
        list_exploration_artifacts(&mut state, json!({}))
            .expect("list artifacts RPC should succeed"),
    )
    .expect("list result should decode");
    assert_eq!(listed.artifacts.len(), 1);
    assert_eq!(listed.artifacts[0], saved.artifact);

    let inspected: ExplorationArtifactResult = serde_json::from_value(
        exploration_artifact(&mut state, json!({ "artifact_id": "saved-unresolved" }))
            .expect("inspect artifact RPC should succeed"),
    )
    .expect("inspect result should decode");
    assert_eq!(inspected.artifact, artifact);

    let live = execute_explore_query(
        &mut state,
        &ExploreParams {
            node_key: focus_key,
            lens: ExplorationLens::Unresolved,
            limit: 20,
            unique: false,
        },
    )
    .expect("live explore should succeed");
    let executed: ExecuteExplorationArtifactResult = serde_json::from_value(
        execute_exploration_artifact(&mut state, json!({ "artifact_id": "saved-unresolved" }))
            .expect("execute artifact RPC should succeed"),
    )
    .expect("execute result should decode");
    assert_eq!(executed.artifact.metadata, artifact.metadata);
    match executed.artifact.payload {
        ExecutedExplorationArtifactPayload::LensView {
            artifact: executed_artifact,
            result,
            ..
        } => {
            match artifact.payload {
                ExplorationArtifactPayload::LensView { artifact } => {
                    assert_eq!(executed_artifact, artifact);
                }
                _ => panic!("expected saved lens-view artifact"),
            }
            assert_eq!(*result, live);
        }
        payload => panic!("expected lens-view execution, got {:?}", payload.kind()),
    }

    let deleted: DeleteExplorationArtifactResult = serde_json::from_value(
        delete_exploration_artifact(&mut state, json!({ "artifact_id": "saved-unresolved" }))
            .expect("delete artifact RPC should succeed"),
    )
    .expect("delete result should decode");
    assert_eq!(deleted.artifact_id, "saved-unresolved");

    let listed_after_delete: ListExplorationArtifactsResult = serde_json::from_value(
        list_exploration_artifacts(&mut state, json!({}))
            .expect("list after delete should succeed"),
    )
    .expect("list after delete should decode");
    assert!(listed_after_delete.artifacts.is_empty());
}

#[test]
fn artifact_rpc_replays_saved_comparisons_and_trails_after_reopen() {
    let (_workspace, mut state, left_key, right_key) = comparison_state();
    let comparison = saved_comparison_artifact(
        "saved-comparison",
        "Saved Comparison",
        &left_key,
        &right_key,
    );
    let trail = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "saved-trail".to_owned(),
            title: "Saved Trail".to_owned(),
            summary: Some("Persisted replay".to_owned()),
        },
        payload: ExplorationArtifactPayload::Trail {
            artifact: Box::new(SavedTrailArtifact {
                steps: vec![
                    SavedTrailStep::LensView {
                        artifact: Box::new(SavedLensViewArtifact {
                            root_node_key: left_key.clone(),
                            current_node_key: left_key.clone(),
                            lens: ExplorationLens::Structure,
                            limit: 20,
                            unique: false,
                            frozen_context: false,
                        }),
                    },
                    SavedTrailStep::Comparison {
                        artifact: Box::new(SavedComparisonArtifact {
                            root_node_key: left_key.clone(),
                            left_node_key: left_key.clone(),
                            right_node_key: right_key.clone(),
                            active_lens: ExplorationLens::Structure,
                            structure_unique: false,
                            comparison_group: Default::default(),
                            limit: 20,
                            frozen_context: false,
                        }),
                    },
                ],
                cursor: 1,
                detached_step: Some(Box::new(SavedTrailStep::LensView {
                    artifact: Box::new(SavedLensViewArtifact {
                        root_node_key: right_key.clone(),
                        current_node_key: right_key.clone(),
                        lens: ExplorationLens::Structure,
                        limit: 20,
                        unique: false,
                        frozen_context: false,
                    }),
                })),
            }),
        },
    };

    for artifact in [comparison.clone(), trail.clone()] {
        let _: SaveExplorationArtifactResult = serde_json::from_value(
            save_exploration_artifact(
                &mut state,
                json!({ "artifact": artifact, "overwrite": true }),
            )
            .expect("save artifact RPC should succeed"),
        )
        .expect("save result should decode");
    }

    let root = state.root.clone();
    let db_path = state.db_path.clone();
    let discovery = state.discovery.clone();
    drop(state);

    let mut reopened = ServerState::new(root, db_path, Vec::new(), discovery)
        .expect("state should reopen cleanly");

    let listed: ListExplorationArtifactsResult = serde_json::from_value(
        list_exploration_artifacts(&mut reopened, json!({}))
            .expect("list after reopen should succeed"),
    )
    .expect("list after reopen should decode");
    let mut ids = listed
        .artifacts
        .into_iter()
        .map(|summary| summary.metadata.artifact_id)
        .collect::<Vec<_>>();
    ids.sort();
    assert_eq!(
        ids,
        vec!["saved-comparison".to_owned(), "saved-trail".to_owned()]
    );

    let expected_left_structure = execute_explore_query(
        &mut reopened,
        &ExploreParams {
            node_key: left_key.clone(),
            lens: ExplorationLens::Structure,
            limit: 20,
            unique: false,
        },
    )
    .expect("live left structure should succeed");
    let expected_right_structure = execute_explore_query(
        &mut reopened,
        &ExploreParams {
            node_key: right_key.clone(),
            lens: ExplorationLens::Structure,
            limit: 20,
            unique: false,
        },
    )
    .expect("live right structure should succeed");
    let expected_comparison = execute_compare_notes_query(
        &mut reopened,
        &CompareNotesParams {
            left_node_key: left_key.clone(),
            right_node_key: right_key.clone(),
            limit: 20,
        },
    )
    .expect("live comparison should succeed");

    let executed_comparison: ExecuteExplorationArtifactResult = serde_json::from_value(
        execute_exploration_artifact(&mut reopened, json!({ "artifact_id": "saved-comparison" }))
            .expect("comparison execution after reopen should succeed"),
    )
    .expect("comparison execution result should decode");
    match executed_comparison.artifact.payload {
        ExecutedExplorationArtifactPayload::Comparison {
            artifact: executed_artifact,
            result,
            ..
        } => {
            match comparison.payload {
                ExplorationArtifactPayload::Comparison { artifact } => {
                    assert_eq!(executed_artifact, artifact);
                }
                _ => panic!("expected saved comparison artifact"),
            }
            assert_eq!(*result, expected_comparison);
        }
        payload => panic!("expected comparison execution, got {:?}", payload.kind()),
    }

    let executed_trail: ExecuteExplorationArtifactResult = serde_json::from_value(
        execute_exploration_artifact(&mut reopened, json!({ "artifact_id": "saved-trail" }))
            .expect("trail execution after reopen should succeed"),
    )
    .expect("trail execution result should decode");
    match executed_trail.artifact.payload {
        ExecutedExplorationArtifactPayload::Trail {
            artifact: executed_artifact,
            replay,
        } => {
            match trail.payload {
                ExplorationArtifactPayload::Trail { artifact } => {
                    assert_eq!(executed_artifact, artifact);
                }
                _ => panic!("expected saved trail artifact"),
            }
            assert_eq!(replay.cursor, 1);
            assert_eq!(replay.steps.len(), 2);
            match &replay.steps[0] {
                TrailReplayStepResult::LensView {
                    artifact, result, ..
                } => {
                    match &executed_artifact.steps[0] {
                        SavedTrailStep::LensView {
                            artifact: expected_artifact,
                        } => {
                            assert_eq!(artifact.as_ref(), expected_artifact.as_ref());
                        }
                        _ => panic!("expected first trail step artifact to be lens-view"),
                    }
                    assert_eq!(result.as_ref(), &expected_left_structure);
                }
                other => panic!(
                    "expected first replay step to be lens-view, got {:?}",
                    other
                ),
            }
            match &replay.steps[1] {
                TrailReplayStepResult::Comparison {
                    artifact, result, ..
                } => {
                    match &executed_artifact.steps[1] {
                        SavedTrailStep::Comparison {
                            artifact: expected_artifact,
                        } => {
                            assert_eq!(artifact.as_ref(), expected_artifact.as_ref());
                        }
                        _ => panic!("expected second trail step artifact to be comparison"),
                    }
                    assert_eq!(result.as_ref(), &expected_comparison);
                }
                other => panic!(
                    "expected second replay step to be comparison, got {:?}",
                    other
                ),
            }
            match replay.detached_step.as_deref() {
                Some(TrailReplayStepResult::LensView {
                    artifact, result, ..
                }) => {
                    match executed_artifact.detached_step.as_deref() {
                        Some(SavedTrailStep::LensView {
                            artifact: expected_artifact,
                        }) => {
                            assert_eq!(artifact.as_ref(), expected_artifact.as_ref());
                        }
                        _ => panic!("expected detached trail step artifact to be lens-view"),
                    }
                    assert_eq!(result.as_ref(), &expected_right_structure);
                }
                other => panic!("expected detached replay step, got {:?}", other),
            }
        }
        payload => panic!("expected trail execution, got {:?}", payload.kind()),
    }
}

#[test]
fn artifact_rpc_reports_missing_and_invalid_artifacts() {
    let (_workspace, mut state, _target_key) = indexed_state();

    let padded_error = exploration_artifact(&mut state, json!({ "artifact_id": " missing " }))
        .expect_err("padded artifact id should be rejected");
    assert_eq!(
        padded_error.into_inner().message,
        "artifact_id must not have leading or trailing whitespace"
    );

    for operation in [
        exploration_artifact(&mut state, json!({ "artifact_id": "missing" })),
        execute_exploration_artifact(&mut state, json!({ "artifact_id": "missing" })),
        delete_exploration_artifact(&mut state, json!({ "artifact_id": "missing" })),
    ] {
        let error = operation.expect_err("missing artifact should be rejected");
        assert_eq!(
            error.into_inner().message,
            "unknown exploration artifact: missing"
        );
    }
}

#[test]
fn save_exploration_artifact_rpc_rejects_invalid_artifacts() {
    let (_workspace, mut state, target_key) = indexed_state();
    let invalid = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "invalid-trail".to_owned(),
            title: "Invalid Trail".to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::Trail {
            artifact: Box::new(SavedTrailArtifact {
                steps: vec![SavedTrailStep::LensView {
                    artifact: Box::new(SavedLensViewArtifact {
                        root_node_key: target_key.clone(),
                        current_node_key: target_key,
                        lens: ExplorationLens::Structure,
                        limit: 20,
                        unique: false,
                        frozen_context: false,
                    }),
                }],
                cursor: 1,
                detached_step: None,
            }),
        },
    };

    let error = save_exploration_artifact(&mut state, json!({ "artifact": invalid }))
        .expect_err("save artifact RPC should reject malformed artifacts");
    assert_eq!(
        error.into_inner().message,
        "trail cursor must point to an existing step"
    );
}

#[test]
fn save_exploration_artifact_rpc_respects_non_overwrite_policy() {
    let (_workspace, mut state, focus_key) = non_obvious_state();
    let original = saved_lens_artifact(
        "saved-unresolved",
        "Original",
        &focus_key,
        ExplorationLens::Refs,
    );
    let replacement = saved_lens_artifact(
        "saved-unresolved",
        "Replacement",
        &focus_key,
        ExplorationLens::Unresolved,
    );

    let _: SaveExplorationArtifactResult = serde_json::from_value(
        save_exploration_artifact(
            &mut state,
            json!({ "artifact": original.clone(), "overwrite": true }),
        )
        .expect("initial save should succeed"),
    )
    .expect("save result should decode");

    let error = save_exploration_artifact(
        &mut state,
        json!({ "artifact": replacement, "overwrite": false }),
    )
    .expect_err("non-overwrite save should reject replacement");
    assert_eq!(
        error.into_inner().message,
        "exploration artifact already exists: saved-unresolved"
    );

    let stored = state
        .database
        .exploration_artifact("saved-unresolved")
        .expect("stored artifact lookup should succeed")
        .expect("stored artifact should remain readable");
    assert_eq!(stored, original);
}

#[test]
fn workbench_pack_rpc_round_trips_import_validate_export_delete_after_reopen() {
    let (_workspace, mut state, _target_key) = indexed_state();
    let pack = sample_workbench_pack("pack/research-review", "Research Review Pack");

    let validation: ValidateWorkbenchPackResult = serde_json::from_value(
        validate_workbench_pack(&mut state, json!({ "pack": pack.clone() }))
            .expect("validate pack RPC should succeed"),
    )
    .expect("validation result should decode");
    assert!(validation.valid);
    assert!(validation.issues.is_empty());
    assert_eq!(validation.pack, Some(pack.summary()));
    let listed_after_validation: ListWorkbenchPacksResult = serde_json::from_value(
        list_workbench_packs(&mut state, json!({})).expect("validation should not persist packs"),
    )
    .expect("list result should decode");
    assert!(listed_after_validation.packs.is_empty());

    let imported: ImportWorkbenchPackResult = serde_json::from_value(
        import_workbench_pack(&mut state, json!({ "pack": pack.clone() }))
            .expect("import pack RPC should succeed"),
    )
    .expect("import result should decode");
    assert_eq!(imported.pack, pack.summary());

    let conflict = import_workbench_pack(&mut state, json!({ "pack": pack.clone() }))
        .expect_err("default import should reject existing packs");
    assert_eq!(
        conflict.into_inner().message,
        "workbench pack already exists: pack/research-review"
    );

    let mut replacement = pack.clone();
    replacement.metadata.title = "Research Review Pack Updated".to_owned();
    let overwritten: ImportWorkbenchPackResult = serde_json::from_value(
        import_workbench_pack(
            &mut state,
            json!({ "pack": replacement.clone(), "overwrite": true }),
        )
        .expect("overwrite import should succeed"),
    )
    .expect("overwrite result should decode");
    assert_eq!(overwritten.pack, replacement.summary());

    let root = state.root.clone();
    let db_path = state.db_path.clone();
    let discovery = state.discovery.clone();
    drop(state);

    let mut reopened =
        ServerState::new(root, db_path, Vec::new(), discovery).expect("state should reopen");
    let listed: ListWorkbenchPacksResult = serde_json::from_value(
        list_workbench_packs(&mut reopened, json!({}))
            .expect("list packs after reopen should succeed"),
    )
    .expect("list result should decode");
    assert_eq!(listed.packs, vec![replacement.summary()]);

    let shown: WorkbenchPackResult = serde_json::from_value(
        workbench_pack(&mut reopened, json!({ "pack_id": "pack/research-review" }))
            .expect("show pack after reopen should succeed"),
    )
    .expect("show result should decode");
    assert_eq!(shown.pack, replacement);

    let exported: WorkbenchPackManifest = serde_json::from_value(
        export_workbench_pack(&mut reopened, json!({ "pack_id": "pack/research-review" }))
            .expect("export pack after reopen should succeed"),
    )
    .expect("exported pack should decode as canonical manifest");
    assert_eq!(exported, shown.pack);

    let deleted: DeleteWorkbenchPackResult = serde_json::from_value(
        delete_workbench_pack(&mut reopened, json!({ "pack_id": "pack/research-review" }))
            .expect("delete pack RPC should succeed"),
    )
    .expect("delete result should decode");
    assert_eq!(deleted.pack_id, "pack/research-review");
    let listed_after_delete: ListWorkbenchPacksResult = serde_json::from_value(
        list_workbench_packs(&mut reopened, json!({})).expect("list after delete should succeed"),
    )
    .expect("list result should decode");
    assert!(listed_after_delete.packs.is_empty());
}

#[test]
fn imported_pack_workflows_are_visible_through_live_catalog_handlers() {
    let (_workspace, mut state, target_key) = indexed_state();
    let mut pack = sample_workbench_pack("pack/workflows", "Pack Workflows");
    pack.workflows.push(sample_pack_workflow(
        "workflow/pack/live",
        "Pack Live Workflow",
        &target_key,
    ));
    let _: ImportWorkbenchPackResult = serde_json::from_value(
        import_workbench_pack(&mut state, json!({ "pack": pack }))
            .expect("pack import should succeed"),
    )
    .expect("import result should decode");

    let listed: ListWorkflowsResult = serde_json::from_value(
        list_workflows(&mut state, json!({})).expect("list workflows should succeed"),
    )
    .expect("workflow list should decode");
    assert!(listed.issues.is_empty());
    assert!(listed.workflows.iter().any(|workflow| {
        workflow.metadata.workflow_id == "workflow/pack/live"
            && workflow.metadata.title == "Pack Live Workflow"
    }));

    let shown: WorkflowResult = serde_json::from_value(
        workflow(&mut state, json!({ "workflow_id": "workflow/pack/live" }))
            .expect("pack workflow should be inspectable"),
    )
    .expect("workflow result should decode");
    assert_eq!(shown.workflow.metadata.title, "Pack Live Workflow");

    let run: RunWorkflowResult = serde_json::from_value(
        run_workflow(&mut state, json!({ "workflow_id": "workflow/pack/live" }))
            .expect("pack workflow should execute"),
    )
    .expect("workflow run should decode");
    assert_eq!(
        run.result.workflow.metadata.workflow_id,
        "workflow/pack/live"
    );
    assert_eq!(run.result.steps.len(), 1);
}

#[test]
fn durable_state_survives_reopen_and_forced_index_rebuild_without_surface_pollution() {
    let (_workspace, mut state, target_key) = indexed_state();
    let focus = state
        .database
        .node_from_id("target-id")
        .expect("target lookup should succeed")
        .expect("target note should exist");
    let graph_params = GraphParams {
        root_node_key: None,
        max_distance: None,
        include_orphans: true,
        hidden_link_types: Vec::new(),
        max_title_length: 100,
        shorten_titles: None,
        node_url_prefix: None,
    };
    let before_notes = state
        .database
        .search_nodes("", 50, None)
        .expect("baseline node search should succeed");
    let before_files = state
        .database
        .indexed_files()
        .expect("baseline file list should succeed");
    let before_refs = state
        .database
        .search_refs("", 50)
        .expect("baseline ref search should succeed");
    let before_graph = state
        .database
        .graph_dot(&graph_params)
        .expect("baseline graph should render");

    let artifact = saved_lens_artifact(
        "artifact/rebuild-survivor",
        "Durable Artifact Survivor",
        &target_key,
        ExplorationLens::Refs,
    );
    state
        .database
        .save_exploration_artifact(&artifact)
        .expect("artifact should persist");
    let review = sample_audit_review_run(
        "review/rebuild-survivor",
        "Durable Review Survivor",
        ReviewFindingStatus::Open,
    );
    state
        .database
        .save_review_run(&review)
        .expect("review should persist");
    let mut pack = sample_workbench_pack("pack/rebuild-survivor", "Durable Pack Survivor");
    pack.workflows.push(sample_input_workflow(
        "workflow/pack/rebuild-survivor",
        "Durable Pack Workflow",
    ));
    let mut routine = sample_workflow_review_routine(
        "routine/pack/rebuild-survivor",
        "workflow/pack/rebuild-survivor",
        None,
    );
    routine.report_profile_ids = vec!["pack/rebuild-survivor/profile/detail".to_owned()];
    pack.review_routines.push(routine);
    let imported: ImportWorkbenchPackResult = serde_json::from_value(
        import_workbench_pack(&mut state, json!({ "pack": pack.clone() }))
            .expect("pack import should succeed"),
    )
    .expect("pack import result should decode");
    assert_eq!(imported.pack, pack.summary());

    let root = state.root.clone();
    let db_path = state.db_path.clone();
    let discovery = state.discovery.clone();
    drop(state);
    fs::remove_file(&db_path).expect("derived SQLite database should be removable");

    let mut reopened = ServerState::new(root.clone(), db_path, Vec::new(), discovery)
        .expect("state should reopen after derived database removal");

    let artifacts: ListExplorationArtifactsResult = serde_json::from_value(
        list_exploration_artifacts(&mut reopened, json!({}))
            .expect("artifacts should list after rebuild"),
    )
    .expect("artifact list should decode");
    assert_eq!(artifacts.artifacts.len(), 1);
    assert_eq!(
        artifacts.artifacts[0].metadata.artifact_id,
        "artifact/rebuild-survivor"
    );
    let reviews: ListReviewRunsResult = serde_json::from_value(
        list_review_runs(&mut reopened, json!({})).expect("reviews should list after rebuild"),
    )
    .expect("review list should decode");
    assert_eq!(reviews.reviews.len(), 1);
    assert_eq!(
        reviews.reviews[0].metadata.review_id,
        "review/rebuild-survivor"
    );
    let packs: ListWorkbenchPacksResult = serde_json::from_value(
        list_workbench_packs(&mut reopened, json!({})).expect("packs should list after rebuild"),
    )
    .expect("pack list should decode");
    assert_eq!(packs.packs, vec![pack.summary()]);

    let workflows: ListWorkflowsResult = serde_json::from_value(
        list_workflows(&mut reopened, json!({})).expect("workflow catalog should load packs"),
    )
    .expect("workflow list should decode");
    assert!(workflows.issues.is_empty());
    assert!(workflows.workflows.iter().any(|workflow| {
        workflow.metadata.workflow_id == "workflow/pack/rebuild-survivor"
            && workflow.metadata.title == "Durable Pack Workflow"
    }));
    let shown_workflow: WorkflowResult = serde_json::from_value(
        workflow(
            &mut reopened,
            json!({ "workflow_id": "workflow/pack/rebuild-survivor" }),
        )
        .expect("pack workflow should remain inspectable"),
    )
    .expect("workflow result should decode");
    assert_eq!(
        shown_workflow.workflow.metadata.workflow_id,
        "workflow/pack/rebuild-survivor"
    );

    let routines: ListReviewRoutinesResult = serde_json::from_value(
        list_review_routines(&mut reopened, json!({})).expect("routine catalog should load packs"),
    )
    .expect("routine list should decode");
    assert!(routines.issues.is_empty());
    assert!(routines.routines.iter().any(|routine| {
        routine.metadata.routine_id == "routine/pack/rebuild-survivor"
            && routine.metadata.title == "Workflow Routine"
    }));
    let shown_routine: ReviewRoutineResult = serde_json::from_value(
        review_routine(
            &mut reopened,
            json!({ "routine_id": "routine/pack/rebuild-survivor" }),
        )
        .expect("pack routine should remain inspectable"),
    )
    .expect("routine result should decode");
    assert_eq!(
        shown_routine.routine.report_profile_ids,
        vec!["pack/rebuild-survivor/profile/detail".to_owned()]
    );
    let shown_pack: WorkbenchPackResult = serde_json::from_value(
        workbench_pack(&mut reopened, json!({ "pack_id": "pack/rebuild-survivor" }))
            .expect("pack should remain inspectable"),
    )
    .expect("pack result should decode");
    assert_eq!(
        shown_pack.pack.report_profiles[0].metadata.profile_id,
        "pack/rebuild-survivor/profile/detail"
    );

    let files = scan_root_with_policy(&root, &reopened.discovery).expect("fixture should rescan");
    reopened
        .database
        .sync_index(&files)
        .expect("derived index should rebuild");
    assert_eq!(
        reopened.database.search_nodes("", 50, None).unwrap(),
        before_notes
    );
    assert_eq!(reopened.database.indexed_files().unwrap(), before_files);
    assert_eq!(reopened.database.search_refs("", 50).unwrap(), before_refs);
    assert_eq!(
        reopened.database.graph_dot(&graph_params).unwrap(),
        before_graph
    );
    assert!(
        reopened
            .database
            .search_nodes("Durable Pack Survivor", 20, None)
            .unwrap()
            .is_empty()
    );
    assert!(
        reopened
            .database
            .search_refs("rebuild-survivor", 20)
            .unwrap()
            .is_empty()
    );
    assert!(
        !reopened
            .database
            .graph_dot(&graph_params)
            .unwrap()
            .contains("rebuild-survivor")
    );
    assert_eq!(
        reopened
            .database
            .node_from_id("target-id")
            .expect("rebuilt target lookup should succeed"),
        Some(focus)
    );
}

#[test]
fn review_routine_rpc_executes_audit_routines_with_save_compare_and_profiles() {
    let (_workspace, mut state) = audit_state();
    let _: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "duplicate-titles",
                "limit": 20,
                "review_id": "review/routine/z-old"
            }),
        )
        .expect("previous audit review should save"),
    )
    .expect("previous save result should decode");
    sleep(Duration::from_millis(20));
    let _: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "duplicate-titles",
                "limit": 20,
                "review_id": "review/routine/a-new"
            }),
        )
        .expect("newer audit review should save"),
    )
    .expect("newer save result should decode");

    let mut pack = sample_workbench_pack("pack/routines", "Routine Pack");
    pack.report_profiles
        .push(sample_routine_report_profile("profile/routine/detail"));
    pack.report_profiles
        .push(sample_routine_only_review_profile(
            "profile/routine/review-lines",
        ));
    let mut routine = sample_audit_review_routine("routine/pack/audit", "profile/routine/detail");
    routine
        .report_profile_ids
        .push("profile/routine/review-lines".to_owned());
    pack.review_routines.push(routine);
    let _: ImportWorkbenchPackResult = serde_json::from_value(
        import_workbench_pack(&mut state, json!({ "pack": pack }))
            .expect("routine pack should import"),
    )
    .expect("pack import result should decode");

    let run: RunReviewRoutineResult = serde_json::from_value(
        run_review_routine(&mut state, json!({ "routine_id": "routine/pack/audit" }))
            .expect("routine should run"),
    )
    .expect("routine result should decode");

    assert_eq!(run.result.routine.metadata.routine_id, "routine/pack/audit");
    match &run.result.source {
        ReviewRoutineSourceExecutionResult::Audit { result } => {
            assert_eq!(result.audit, CorpusAuditKind::DuplicateTitles);
            assert_eq!(result.entries.len(), 1);
        }
        other => panic!("expected audit routine source, got {other:?}"),
    }
    assert_eq!(
        run.result
            .saved_review
            .as_ref()
            .expect("routine should save a review")
            .metadata
            .review_id,
        "review/routine/001-current"
    );
    let compare = run
        .result
        .compare
        .as_ref()
        .expect("routine should return compare result");
    assert_eq!(
        compare
            .base_review
            .as_ref()
            .expect("previous compatible review should be selected")
            .metadata
            .review_id,
        "review/routine/a-new"
    );
    assert_eq!(
        compare
            .diff
            .as_ref()
            .expect("compatible reviews should diff")
            .unchanged
            .len(),
        1
    );
    assert!(
        compare
            .report
            .as_ref()
            .expect("compare profile should be applied")
            .lines
            .iter()
            .any(|line| matches!(line, ReviewRoutineReportLine::Unchanged { .. }))
    );

    let profile = run
        .result
        .reports
        .first()
        .expect("routine report profile should be applied");
    assert_eq!(
        profile.profile.metadata.profile_id,
        "profile/routine/detail"
    );
    assert!(
        profile
            .lines
            .iter()
            .any(|line| matches!(line, ReviewRoutineReportLine::Routine { .. }))
    );
    assert!(
        profile
            .lines
            .iter()
            .any(|line| matches!(line, ReviewRoutineReportLine::Entry { .. }))
    );
    assert!(
        profile
            .lines
            .iter()
            .any(|line| matches!(line, ReviewRoutineReportLine::Finding { .. }))
    );
    assert!(
        profile
            .lines
            .iter()
            .any(|line| matches!(line, ReviewRoutineReportLine::Diff { .. }))
    );
    let routine_only_profile = run
        .result
        .reports
        .iter()
        .find(|report| report.profile.metadata.profile_id == "profile/routine/review-lines")
        .expect("routine-only review profile should be applied");
    assert!(
        routine_only_profile
            .lines
            .iter()
            .any(|line| matches!(line, ReviewRoutineReportLine::Review { .. }))
    );
    assert!(
        routine_only_profile
            .lines
            .iter()
            .any(|line| matches!(line, ReviewRoutineReportLine::Finding { .. }))
    );
    assert!(routine_only_profile.lines.iter().all(|line| matches!(
        line,
        ReviewRoutineReportLine::Review { .. } | ReviewRoutineReportLine::Finding { .. }
    )));

    let saved: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/routine/001-current" }),
        )
        .expect("saved routine review should be loadable"),
    )
    .expect("saved review should decode");
    assert_eq!(saved.review.findings.len(), 1);
}

#[test]
fn review_routine_rpc_executes_workflow_routines_with_inputs_and_reports_step_failures() {
    let (_workspace, mut state, target_key) = indexed_state();
    let mut pack = sample_workbench_pack("pack/workflow-routine", "Workflow Routine Pack");
    pack.workflows.push(sample_input_workflow(
        "workflow/pack/input-review",
        "Input Review",
    ));
    pack.review_routines.push(sample_workflow_review_routine(
        "routine/pack/workflow",
        "workflow/pack/input-review",
        Some("review/routine/workflow"),
    ));
    pack.review_routines.push(sample_workflow_review_routine(
        "routine/pack/workflow-failure",
        "workflow/pack/input-review",
        None,
    ));
    let _: ImportWorkbenchPackResult = serde_json::from_value(
        import_workbench_pack(&mut state, json!({ "pack": pack }))
            .expect("workflow routine pack should import"),
    )
    .expect("pack import result should decode");

    let run: RunReviewRoutineResult = serde_json::from_value(
        run_review_routine(
            &mut state,
            json!({
                "routine_id": "routine/pack/workflow",
                "inputs": [{
                    "input_id": "focus",
                    "kind": "node-key",
                    "node_key": target_key
                }]
            }),
        )
        .expect("workflow routine should run"),
    )
    .expect("workflow routine result should decode");
    match &run.result.source {
        ReviewRoutineSourceExecutionResult::Workflow { result } => {
            assert_eq!(
                result.workflow.metadata.workflow_id,
                "workflow/pack/input-review"
            );
            assert_eq!(result.steps.len(), 2);
        }
        other => panic!("expected workflow routine source, got {other:?}"),
    }
    assert_eq!(
        run.result
            .saved_review
            .as_ref()
            .expect("workflow routine should save review")
            .metadata
            .review_id,
        "review/routine/workflow"
    );

    let missing_input = run_review_routine(
        &mut state,
        json!({ "routine_id": "routine/pack/workflow-failure" }),
    )
    .expect_err("missing workflow input should fail before execution");
    assert_eq!(
        missing_input.into_inner().message,
        "workflow input focus must be assigned"
    );

    let step_failure = run_review_routine(
        &mut state,
        json!({
            "routine_id": "routine/pack/workflow-failure",
            "inputs": [{
                "input_id": "focus",
                "kind": "node-key",
                "node_key": "missing:node"
            }]
        }),
    )
    .expect_err("workflow step failure should be surfaced with context");
    assert_eq!(
        step_failure.into_inner().message,
        "workflow step resolve-focus failed: unknown workflow note target: missing:node"
    );
}

#[test]
fn review_routine_save_review_conflicts_prevent_workflow_side_effects() {
    let (_workspace, mut state, target_key) = indexed_state();
    let _: SaveReviewRunResult = serde_json::from_value(
        save_review_run(
            &mut state,
            json!({
                "review": sample_audit_review_run(
                    "review/routine/conflict",
                    "Existing Routine Review",
                    ReviewFindingStatus::Open
                )
            }),
        )
        .expect("existing review should save"),
    )
    .expect("save review result should decode");

    let mut pack = sample_workbench_pack("pack/routine-conflict", "Routine Conflict Pack");
    pack.workflows.push(sample_artifact_save_workflow(
        "workflow/pack/conflict-side-effect",
        &target_key,
    ));
    pack.review_routines.push(ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: "routine/pack/conflict".to_owned(),
            title: "Conflict Routine".to_owned(),
            summary: None,
        },
        source: ReviewRoutineSource::Workflow {
            workflow_id: "workflow/pack/conflict-side-effect".to_owned(),
        },
        inputs: Vec::new(),
        save_review: ReviewRoutineSaveReviewPolicy {
            enabled: true,
            review_id: Some("review/routine/conflict".to_owned()),
            title: None,
            summary: None,
            overwrite: false,
        },
        compare: None,
        report_profile_ids: Vec::new(),
    });
    let _: ImportWorkbenchPackResult = serde_json::from_value(
        import_workbench_pack(&mut state, json!({ "pack": pack }))
            .expect("conflict pack should import"),
    )
    .expect("pack import result should decode");

    let conflict = run_review_routine(&mut state, json!({ "routine_id": "routine/pack/conflict" }))
        .expect_err("review conflict should reject before workflow execution");
    assert_eq!(
        conflict.into_inner().message,
        "review run already exists: review/routine/conflict"
    );
    assert!(
        state
            .database
            .exploration_artifact("routine-conflict-artifact")
            .expect("artifact lookup should succeed")
            .is_none(),
        "workflow artifact-save side effect should not run on review conflict"
    );
}

#[test]
fn review_routine_rpc_reports_unknown_routines() {
    let (_workspace, mut state) = audit_state();
    let error = run_review_routine(&mut state, json!({ "routine_id": "routine/missing" }))
        .expect_err("unknown routine should fail");
    assert_eq!(
        error.into_inner().message,
        "unknown review routine: routine/missing"
    );
}

#[test]
fn workbench_pack_rpc_reports_malformed_unsupported_and_missing_packs() {
    let (_workspace, mut state, _target_key) = indexed_state();
    let valid = sample_workbench_pack("pack/research-review", "Research Review Pack");

    let padded_error = workbench_pack(&mut state, json!({ "pack_id": " pack/research-review " }))
        .expect_err("padded pack id should be rejected");
    assert_eq!(
        padded_error.into_inner().message,
        "pack_id must not have leading or trailing whitespace"
    );

    for operation in [
        workbench_pack(&mut state, json!({ "pack_id": "pack/missing" })),
        export_workbench_pack(&mut state, json!({ "pack_id": "pack/missing" })),
        delete_workbench_pack(&mut state, json!({ "pack_id": "pack/missing" })),
    ] {
        let error = operation.expect_err("missing pack should be rejected");
        assert_eq!(
            error.into_inner().message,
            "unknown workbench pack: pack/missing"
        );
    }

    let mut unsupported = valid.clone();
    unsupported.compatibility = WorkbenchPackCompatibility { version: 2 };
    let validation: ValidateWorkbenchPackResult = serde_json::from_value(
        validate_workbench_pack(&mut state, json!({ "pack": unsupported }))
            .expect("unsupported version should produce validation issues"),
    )
    .expect("validation result should decode");
    assert!(!validation.valid);
    assert_eq!(
        validation.issues[0].kind,
        WorkbenchPackIssueKind::UnsupportedVersion
    );
    assert_eq!(
        validation.issues[0].message,
        "unsupported workbench pack compatibility version 2; supported version is 1"
    );

    let future_syntax_validation: ValidateWorkbenchPackResult = serde_json::from_value(
        validate_workbench_pack(
            &mut state,
            json!({
                "pack": {
                    "pack_id": "pack/future",
                    "title": "Future Pack",
                    "compatibility": { "version": 2 },
                    "workflows": [{
                        "workflow_id": "workflow/future",
                        "title": "Future Workflow",
                        "inputs": [{
                            "input_id": "focus",
                            "title": "Focus",
                            "kind": "future-target"
                        }],
                        "steps": []
                    }]
                }
            }),
        )
        .expect("future pack compatibility should be detected before typed parse"),
    )
    .expect("future syntax validation result should decode");
    assert!(!future_syntax_validation.valid);
    assert_eq!(future_syntax_validation.pack, None);
    assert_eq!(
        future_syntax_validation.issues[0].kind,
        WorkbenchPackIssueKind::UnsupportedVersion
    );

    let future_import_error = import_workbench_pack(
        &mut state,
        json!({
            "pack": {
                "pack_id": "pack/future",
                "title": "Future Pack",
                "compatibility": { "version": 2 },
                "workflows": [{
                    "workflow_id": "workflow/future",
                    "title": "Future Workflow",
                    "inputs": [{
                        "input_id": "focus",
                        "title": "Focus",
                        "kind": "future-target"
                    }],
                    "steps": []
                }]
            }
        }),
    )
    .expect_err("future pack import should fail as unsupported before typed parse");
    assert_eq!(
        future_import_error.into_inner().message,
        "unsupported workbench pack compatibility version 2; supported version is 1"
    );

    let listed_after_validation: ListWorkbenchPacksResult = serde_json::from_value(
        list_workbench_packs(&mut state, json!({}))
            .expect("invalid validation should not persist packs"),
    )
    .expect("list result should decode");
    assert!(listed_after_validation.packs.is_empty());

    let mut empty = valid.clone();
    empty.workflows.clear();
    empty.review_routines.clear();
    empty.report_profiles.clear();
    let save_error = import_workbench_pack(&mut state, json!({ "pack": empty }))
        .expect_err("import should reject invalid packs");
    assert_eq!(
        save_error.into_inner().message,
        "workbench packs must contain at least one workflow, review routine, or report profile"
    );

    let malformed_error = validate_workbench_pack(
        &mut state,
        json!({
            "pack": {
                "pack_id": "pack/malformed",
                "title": "Malformed",
                "report_profiles": [{
                    "profile_id": "profile/malformed",
                    "title": "Malformed",
                    "subjects": ["future-subject"]
                }]
            }
        }),
    )
    .expect_err("malformed pack should fail request parsing");
    assert!(
        malformed_error
            .into_inner()
            .message
            .starts_with("invalid request parameters:"),
        "unexpected malformed error"
    );
}

#[test]
fn review_run_rpc_operations_round_trip_and_mark_review_runs() {
    let (_workspace, mut state) = audit_state();
    let review = sample_audit_review_run(
        "review/audit/dangling-links",
        "Dangling Link Review",
        ReviewFindingStatus::Open,
    );

    let saved: SaveReviewRunResult = serde_json::from_value(
        save_review_run(
            &mut state,
            json!({ "review": review.clone(), "overwrite": true }),
        )
        .expect("save review RPC should succeed"),
    )
    .expect("save review result should decode");
    assert_eq!(saved.review.metadata, review.metadata);
    assert_eq!(saved.review.finding_count, 1);
    assert_eq!(saved.review.status_counts.open, 1);

    let listed: ListReviewRunsResult = serde_json::from_value(
        list_review_runs(&mut state, json!({})).expect("list reviews RPC should succeed"),
    )
    .expect("list reviews result should decode");
    assert_eq!(listed.reviews, vec![saved.review.clone()]);

    let inspected: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/dangling-links" }),
        )
        .expect("inspect review RPC should succeed"),
    )
    .expect("inspect review result should decode");
    assert_eq!(inspected.review, review);

    let marked: MarkReviewFindingResult = serde_json::from_value(
        mark_review_finding(
            &mut state,
            json!({
                "review_id": "review/audit/dangling-links",
                "finding_id": "audit/dangling-links/source/missing-id",
                "status": "reviewed"
            }),
        )
        .expect("mark review finding RPC should succeed"),
    )
    .expect("mark result should decode");
    assert_eq!(marked.transition.from_status, ReviewFindingStatus::Open);
    assert_eq!(marked.transition.to_status, ReviewFindingStatus::Reviewed);

    let root = state.root.clone();
    let db_path = state.db_path.clone();
    let discovery = state.discovery.clone();
    drop(state);

    let mut reopened =
        ServerState::new(root, db_path, Vec::new(), discovery).expect("state should reopen");
    let updated: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut reopened,
            json!({ "review_id": "review/audit/dangling-links" }),
        )
        .expect("marked review should load after reopen"),
    )
    .expect("marked review result should decode");
    assert_eq!(
        updated.review.findings[0].status,
        ReviewFindingStatus::Reviewed
    );

    let deleted: DeleteReviewRunResult = serde_json::from_value(
        delete_review_run(
            &mut reopened,
            json!({ "review_id": "review/audit/dangling-links" }),
        )
        .expect("delete review RPC should succeed"),
    )
    .expect("delete review result should decode");
    assert_eq!(deleted.review_id, "review/audit/dangling-links");

    let listed_after_delete: ListReviewRunsResult = serde_json::from_value(
        list_review_runs(&mut reopened, json!({}))
            .expect("list reviews after delete should succeed"),
    )
    .expect("list reviews after delete should decode");
    assert!(listed_after_delete.reviews.is_empty());
}

#[test]
fn review_run_diff_rpc_classifies_stored_review_runs() {
    let (_workspace, mut state) = audit_state();
    let mut base = sample_audit_review_run(
        "review/audit/dangling-links/base",
        "Base Dangling Review",
        ReviewFindingStatus::Open,
    );
    base.findings.push(ReviewFinding {
        finding_id: "audit/dangling-links/source/removed-id".to_owned(),
        status: ReviewFindingStatus::Dismissed,
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
                    missing_explicit_id: "removed-id".to_owned(),
                    line: 14,
                    column: 3,
                    preview: "[[id:removed-id][Removed]]".to_owned(),
                }),
            }),
        },
    });
    let mut target = sample_audit_review_run(
        "review/audit/dangling-links/target",
        "Target Dangling Review",
        ReviewFindingStatus::Reviewed,
    );
    target.findings.push(ReviewFinding {
        finding_id: "audit/dangling-links/source/added-id".to_owned(),
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
                    missing_explicit_id: "added-id".to_owned(),
                    line: 16,
                    column: 3,
                    preview: "[[id:added-id][Added]]".to_owned(),
                }),
            }),
        },
    });

    state
        .database
        .save_review_run(&base)
        .expect("base review should be saved");
    state
        .database
        .save_review_run(&target)
        .expect("target review should be saved");

    let diff: ReviewRunDiffResult = serde_json::from_value(
        diff_review_runs(
            &mut state,
            json!({
                "base_review_id": "review/audit/dangling-links/base",
                "target_review_id": "review/audit/dangling-links/target"
            }),
        )
        .expect("review diff RPC should succeed"),
    )
    .expect("diff result should decode");

    assert_eq!(diff.diff.added.len(), 1);
    assert_eq!(
        diff.diff.added[0].finding_id,
        "audit/dangling-links/source/added-id"
    );
    assert_eq!(diff.diff.removed.len(), 1);
    assert_eq!(
        diff.diff.removed[0].finding_id,
        "audit/dangling-links/source/removed-id"
    );
    assert!(diff.diff.unchanged.is_empty());
    assert_eq!(diff.diff.status_changed.len(), 1);
    assert_eq!(
        diff.diff.status_changed[0].finding_id,
        "audit/dangling-links/source/missing-id"
    );
    assert_eq!(
        diff.diff.status_changed[0].from_status,
        ReviewFindingStatus::Open
    );
    assert_eq!(
        diff.diff.status_changed[0].to_status,
        ReviewFindingStatus::Reviewed
    );

    let incompatible = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/orphans".to_owned(),
            title: "Orphan Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::OrphanNotes,
            limit: 200,
        },
        findings: Vec::new(),
    };
    state
        .database
        .save_review_run(&incompatible)
        .expect("incompatible review should be saved");
    let error = diff_review_runs(
        &mut state,
        json!({
            "base_review_id": "review/audit/dangling-links/base",
            "target_review_id": "review/audit/orphans"
        }),
    )
    .expect_err("incompatible review diff should be rejected");
    assert!(error.into_inner().message.contains("different audit kinds"));
}

#[test]
fn review_finding_remediation_preview_reports_supported_audit_evidence_without_mutation() {
    let (_workspace, mut state) = audit_state();
    let source_path = state.root.join("dangling-source.org");
    let source_before = fs::read_to_string(&source_path).expect("source file should read");

    let _: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "dangling-links",
                "limit": 20,
                "review_id": "review/audit/dangling-links/custom",
                "overwrite": true
            }),
        )
        .expect("save audit review RPC should succeed"),
    )
    .expect("save audit review result should decode");
    let stored_before: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/dangling-links/custom" }),
        )
        .expect("saved audit review should load"),
    )
    .expect("review result should decode");
    let dangling_finding_id = stored_before.review.findings[0].finding_id.clone();

    let preview: ReviewFindingRemediationPreviewResult = serde_json::from_value(
        review_finding_remediation_preview(
            &mut state,
            json!({
                "review_id": "review/audit/dangling-links/custom",
                "finding_id": dangling_finding_id
            }),
        )
        .expect("dangling-link preview should succeed"),
    )
    .expect("preview result should decode");
    assert_eq!(
        preview.preview.review_id,
        "review/audit/dangling-links/custom"
    );
    assert_eq!(preview.preview.status, ReviewFindingStatus::Open);
    match preview.preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview,
            suggestion,
            confidence,
            reason,
        } => {
            assert_eq!(source.explicit_id.as_deref(), Some("dangling-source-id"));
            assert_eq!(missing_explicit_id, "missing-id");
            assert_eq!(file_path, "dangling-source.org");
            assert_eq!(line, 6);
            assert!(column > 0);
            assert!(preview.contains("missing-id"));
            assert!(suggestion.contains("id:missing-id"));
            assert_eq!(confidence, AuditRemediationConfidence::Medium);
            assert!(reason.contains("dangling-source.org"));
        }
        other => panic!("expected dangling-link preview, got {other:?}"),
    }

    let stored_after: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/dangling-links/custom" }),
        )
        .expect("saved audit review should still load"),
    )
    .expect("review result should decode");
    assert_eq!(stored_after.review, stored_before.review);
    assert_eq!(
        fs::read_to_string(&source_path).expect("source file should still read"),
        source_before
    );

    let _: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "duplicate-titles",
                "limit": 20,
                "review_id": "review/audit/duplicate-titles/custom",
                "overwrite": true
            }),
        )
        .expect("save duplicate-title review RPC should succeed"),
    )
    .expect("duplicate-title review result should decode");
    let duplicate_review: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/duplicate-titles/custom" }),
        )
        .expect("duplicate-title review should load"),
    )
    .expect("duplicate-title review result should decode");
    let duplicate_preview: ReviewFindingRemediationPreviewResult = serde_json::from_value(
        review_finding_remediation_preview(
            &mut state,
            json!({
                "review_id": "review/audit/duplicate-titles/custom",
                "finding_id": duplicate_review.review.findings[0].finding_id
            }),
        )
        .expect("duplicate-title preview should succeed"),
    )
    .expect("duplicate-title preview result should decode");
    match duplicate_preview.preview.payload {
        AuditRemediationPreviewPayload::DuplicateTitle {
            title,
            notes,
            suggestion,
            confidence,
            reason,
        } => {
            assert_eq!(title, "Shared Title");
            assert_eq!(notes.len(), 2);
            assert!(suggestion.contains("Disambiguate"));
            assert_eq!(confidence, AuditRemediationConfidence::High);
            assert!(reason.contains("2 notes"));
        }
        other => panic!("expected duplicate-title preview, got {other:?}"),
    }

    let _: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "orphan-notes",
                "limit": 20,
                "review_id": "review/audit/orphan-notes/custom",
                "overwrite": true
            }),
        )
        .expect("save orphan review RPC should succeed"),
    )
    .expect("orphan review result should decode");
    let orphan_review: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/orphan-notes/custom" }),
        )
        .expect("orphan review should load"),
    )
    .expect("orphan review result should decode");
    let unsupported = review_finding_remediation_preview(
        &mut state,
        json!({
            "review_id": "review/audit/orphan-notes/custom",
            "finding_id": orphan_review.review.findings[0].finding_id
        }),
    )
    .expect_err("orphan preview should be rejected");
    assert_eq!(
        unsupported.into_inner().message,
        "review finding has no remediation preview for orphan-note evidence"
    );
}

#[test]
fn review_finding_remediation_apply_unlinks_dangling_links_without_review_mutation() {
    let (_workspace, mut state) = audit_state();

    let _: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "dangling-links",
                "limit": 20,
                "review_id": "review/audit/dangling-links/custom",
                "overwrite": true
            }),
        )
        .expect("save audit review RPC should succeed"),
    )
    .expect("save audit review result should decode");
    let stored_before: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/dangling-links/custom" }),
        )
        .expect("saved audit review should load"),
    )
    .expect("review result should decode");
    let dangling_finding_id = stored_before.review.findings[0].finding_id.clone();
    let preview: ReviewFindingRemediationPreviewResult = serde_json::from_value(
        review_finding_remediation_preview(
            &mut state,
            json!({
                "review_id": "review/audit/dangling-links/custom",
                "finding_id": dangling_finding_id
            }),
        )
        .expect("dangling-link preview should succeed"),
    )
    .expect("preview result should decode");

    let action = match &preview.preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview,
            ..
        } => AuditRemediationApplyAction::UnlinkDanglingLink {
            source_node_key: source.node_key.clone(),
            missing_explicit_id: missing_explicit_id.clone(),
            file_path: file_path.clone(),
            line: *line,
            column: *column,
            preview: preview.clone(),
            replacement_text: "Missing".to_owned(),
        },
        other => panic!("expected dangling-link preview, got {other:?}"),
    };
    let apply: ReviewFindingRemediationApplyResult = serde_json::from_value(
        review_finding_remediation_apply(
            &mut state,
            serde_json::to_value(ReviewFindingRemediationApplyParams {
                review_id: "review/audit/dangling-links/custom".to_owned(),
                finding_id: dangling_finding_id,
                expected_preview: preview.preview.preview_identity,
                action,
            })
            .expect("apply params should serialize"),
        )
        .expect("remediation apply should succeed"),
    )
    .expect("apply result should decode");
    assert_eq!(
        apply.application.affected_files.changed_files,
        vec!["dangling-source.org"]
    );
    assert!(apply.application.affected_files.removed_files.is_empty());

    let source_after =
        fs::read_to_string(state.root.join("dangling-source.org")).expect("source should read");
    assert!(source_after.contains("Points to Missing."));
    assert!(!source_after.contains("[[id:missing-id][Missing]]"));
    let audit_after: CorpusAuditResult = serde_json::from_value(
        corpus_audit(
            &mut state,
            json!({ "audit": "dangling-links", "limit": 20 }),
        )
        .expect("dangling-link audit should still run"),
    )
    .expect("audit result should decode");
    assert!(audit_after.entries.is_empty());

    let stored_after: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/dangling-links/custom" }),
        )
        .expect("saved audit review should still load"),
    )
    .expect("review result should decode");
    assert_eq!(stored_after.review, stored_before.review);
    assert_eq!(
        state
            .database
            .list_exploration_artifacts()
            .expect("artifact list should load"),
        Vec::new()
    );
    assert_eq!(
        state
            .database
            .list_workbench_packs()
            .expect("pack list should load"),
        Vec::new()
    );
}

#[test]
fn review_finding_remediation_apply_rejects_restored_missing_targets_without_writing() {
    let (_workspace, mut state) = audit_state();

    let _: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "dangling-links",
                "limit": 20,
                "review_id": "review/audit/dangling-links/custom",
                "overwrite": true
            }),
        )
        .expect("save audit review RPC should succeed"),
    )
    .expect("save audit review result should decode");
    let stored: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/dangling-links/custom" }),
        )
        .expect("saved audit review should load"),
    )
    .expect("review result should decode");
    let dangling_finding_id = stored.review.findings[0].finding_id.clone();
    let preview: ReviewFindingRemediationPreviewResult = serde_json::from_value(
        review_finding_remediation_preview(
            &mut state,
            json!({
                "review_id": "review/audit/dangling-links/custom",
                "finding_id": dangling_finding_id
            }),
        )
        .expect("dangling-link preview should succeed"),
    )
    .expect("preview result should decode");
    let action = match &preview.preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview,
            ..
        } => AuditRemediationApplyAction::UnlinkDanglingLink {
            source_node_key: source.node_key.clone(),
            missing_explicit_id: missing_explicit_id.clone(),
            file_path: file_path.clone(),
            line: *line,
            column: *column,
            preview: preview.clone(),
            replacement_text: "Missing".to_owned(),
        },
        other => panic!("expected dangling-link preview, got {other:?}"),
    };

    let source_path = state.root.join("dangling-source.org");
    let source_before =
        fs::read_to_string(&source_path).expect("dangling source should read before apply");
    let restored_target_path = state.root.join("restored-target.org");
    fs::write(
        &restored_target_path,
        r#":PROPERTIES:
:ID: missing-id
:END:
#+title: Restored Target

Restored target.
"#,
    )
    .expect("restored target should write");
    state
        .sync_path(&restored_target_path)
        .expect("restored target should sync");
    assert!(
        state
            .database
            .node_from_id("missing-id")
            .expect("restored target lookup should succeed")
            .is_some()
    );

    let restored_target_error = review_finding_remediation_apply(
        &mut state,
        serde_json::to_value(ReviewFindingRemediationApplyParams {
            review_id: "review/audit/dangling-links/custom".to_owned(),
            finding_id: dangling_finding_id,
            expected_preview: preview.preview.preview_identity,
            action,
        })
        .expect("apply params should serialize"),
    )
    .expect_err("restored target should make the remediation stale");
    assert_eq!(
        restored_target_error.into_inner().message,
        "cannot unlink dangling link because target id missing-id now resolves in the current index"
    );
    let source_after =
        fs::read_to_string(&source_path).expect("dangling source should read after rejection");
    assert_eq!(source_after, source_before);
}

#[test]
fn review_finding_remediation_apply_rejects_stale_or_unsupported_actions() {
    let (_workspace, mut state) = audit_state();

    let _: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "dangling-links",
                "limit": 20,
                "review_id": "review/audit/dangling-links/custom",
                "overwrite": true
            }),
        )
        .expect("save audit review RPC should succeed"),
    )
    .expect("save audit review result should decode");
    let stored: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/dangling-links/custom" }),
        )
        .expect("saved audit review should load"),
    )
    .expect("review result should decode");
    let dangling_finding_id = stored.review.findings[0].finding_id.clone();
    let preview: ReviewFindingRemediationPreviewResult = serde_json::from_value(
        review_finding_remediation_preview(
            &mut state,
            json!({
                "review_id": "review/audit/dangling-links/custom",
                "finding_id": dangling_finding_id
            }),
        )
        .expect("dangling-link preview should succeed"),
    )
    .expect("preview result should decode");
    let action = match &preview.preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview,
            ..
        } => AuditRemediationApplyAction::UnlinkDanglingLink {
            source_node_key: source.node_key.clone(),
            missing_explicit_id: missing_explicit_id.clone(),
            file_path: file_path.clone(),
            line: *line,
            column: *column,
            preview: preview.clone(),
            replacement_text: "Missing".to_owned(),
        },
        other => panic!("expected dangling-link preview, got {other:?}"),
    };

    let mut stale_action = action.clone();
    let stale_preview = match &mut stale_action {
        AuditRemediationApplyAction::UnlinkDanglingLink {
            missing_explicit_id,
            preview,
            ..
        } => {
            *missing_explicit_id = "other-missing-id".to_owned();
            *preview = "Points to [[id:other-missing-id][Missing]].".to_owned();
            stale_action.preview_identity()
        }
    };
    let stale_preview_error = review_finding_remediation_apply(
        &mut state,
        serde_json::to_value(ReviewFindingRemediationApplyParams {
            review_id: "review/audit/dangling-links/custom".to_owned(),
            finding_id: dangling_finding_id.clone(),
            expected_preview: stale_preview,
            action: stale_action,
        })
        .expect("stale apply params should serialize"),
    )
    .expect_err("stale stored preview should be rejected");
    assert_eq!(
        stale_preview_error.into_inner().message,
        format!(
            "stale remediation preview for finding {dangling_finding_id} in review run review/audit/dangling-links/custom"
        )
    );

    let mut wrong_replacement = action.clone();
    match &mut wrong_replacement {
        AuditRemediationApplyAction::UnlinkDanglingLink {
            replacement_text, ..
        } => *replacement_text = "Wrong".to_owned(),
    }
    let wrong_replacement_error = review_finding_remediation_apply(
        &mut state,
        serde_json::to_value(ReviewFindingRemediationApplyParams {
            review_id: "review/audit/dangling-links/custom".to_owned(),
            finding_id: dangling_finding_id.clone(),
            expected_preview: preview.preview.preview_identity.clone(),
            action: wrong_replacement,
        })
        .expect("wrong replacement params should serialize"),
    )
    .expect_err("action with wrong replacement should be rejected");
    assert_eq!(
        wrong_replacement_error.into_inner().message,
        "unlink-dangling-link replacement_text must match the current link label: Missing"
    );

    let source_path = state.root.join("dangling-source.org");
    let stale_source = fs::read_to_string(&source_path)
        .expect("dangling source should read")
        .replace("[[id:missing-id][Missing]]", "[[id:missing-id][Stale]]");
    fs::write(&source_path, stale_source).expect("stale fixture should write");
    let stale_file_error = review_finding_remediation_apply(
        &mut state,
        serde_json::to_value(ReviewFindingRemediationApplyParams {
            review_id: "review/audit/dangling-links/custom".to_owned(),
            finding_id: dangling_finding_id,
            expected_preview: preview.preview.preview_identity,
            action: action.clone(),
        })
        .expect("stale file params should serialize"),
    )
    .expect_err("stale file contents should be rejected");
    assert!(
        stale_file_error
            .into_inner()
            .message
            .contains("remediation action no longer matches file contents")
    );

    let _: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "orphan-notes",
                "limit": 20,
                "review_id": "review/audit/orphan-notes/custom",
                "overwrite": true
            }),
        )
        .expect("save orphan review RPC should succeed"),
    )
    .expect("orphan review result should decode");
    let orphan_review: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut state,
            json!({ "review_id": "review/audit/orphan-notes/custom" }),
        )
        .expect("orphan review should load"),
    )
    .expect("orphan review result should decode");
    let unsupported = review_finding_remediation_apply(
        &mut state,
        serde_json::to_value(ReviewFindingRemediationApplyParams {
            review_id: "review/audit/orphan-notes/custom".to_owned(),
            finding_id: orphan_review.review.findings[0].finding_id.clone(),
            expected_preview: action.preview_identity(),
            action,
        })
        .expect("unsupported apply params should serialize"),
    )
    .expect_err("unsupported finding should be rejected");
    assert_eq!(
        unsupported.into_inner().message,
        "review finding has no remediation preview for orphan-note evidence"
    );
}

#[test]
fn review_run_rpc_reports_missing_invalid_and_malformed_reviews() {
    let (_workspace, mut state) = audit_state();
    let review = sample_audit_review_run(
        "review/audit/dangling-links",
        "Dangling Link Review",
        ReviewFindingStatus::Open,
    );

    let padded_error = review_run(&mut state, json!({ "review_id": " missing " }))
        .expect_err("padded review id should be rejected");
    assert_eq!(
        padded_error.into_inner().message,
        "review_id must not have leading or trailing whitespace"
    );
    let padded_diff = diff_review_runs(
        &mut state,
        json!({
            "base_review_id": " missing ",
            "target_review_id": "review/audit/dangling-links"
        }),
    )
    .expect_err("padded review id in diff should be rejected");
    assert_eq!(
        padded_diff.into_inner().message,
        "review_id must not have leading or trailing whitespace"
    );

    for operation in [
        review_run(&mut state, json!({ "review_id": "missing" })),
        delete_review_run(&mut state, json!({ "review_id": "missing" })),
        mark_review_finding(
            &mut state,
            json!({
                "review_id": "missing",
                "finding_id": "finding",
                "status": "reviewed"
            }),
        ),
        diff_review_runs(
            &mut state,
            json!({
                "base_review_id": "missing",
                "target_review_id": "review/audit/dangling-links"
            }),
        ),
    ] {
        let error = operation.expect_err("missing review should be rejected");
        assert_eq!(error.into_inner().message, "unknown review run: missing");
    }

    let invalid_review = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/invalid".to_owned(),
            title: String::new(),
            summary: None,
        },
        ..review.clone()
    };
    let invalid_save = save_review_run(&mut state, json!({ "review": invalid_review }))
        .expect_err("invalid review should be rejected");
    assert_eq!(invalid_save.into_inner().message, "title must not be empty");

    let _: SaveReviewRunResult = serde_json::from_value(
        save_review_run(
            &mut state,
            json!({ "review": review.clone(), "overwrite": true }),
        )
        .expect("initial review save should succeed"),
    )
    .expect("save result should decode");

    let replacement = sample_audit_review_run(
        "review/audit/dangling-links",
        "Replacement",
        ReviewFindingStatus::Dismissed,
    );
    let overwrite_error = save_review_run(
        &mut state,
        json!({ "review": replacement, "overwrite": false }),
    )
    .expect_err("non-overwrite review save should reject replacement");
    assert_eq!(
        overwrite_error.into_inner().message,
        "review run already exists: review/audit/dangling-links"
    );

    let unknown_finding = mark_review_finding(
        &mut state,
        json!({
            "review_id": "review/audit/dangling-links",
            "finding_id": "missing-finding",
            "status": "reviewed"
        }),
    )
    .expect_err("unknown finding should be rejected");
    assert_eq!(
        unknown_finding.into_inner().message,
        "unknown review finding missing-finding in review run review/audit/dangling-links"
    );

    let no_op = mark_review_finding(
        &mut state,
        json!({
            "review_id": "review/audit/dangling-links",
            "finding_id": "audit/dangling-links/source/missing-id",
            "status": "open"
        }),
    )
    .expect_err("no-op mark should be rejected");
    assert_eq!(
        no_op.into_inner().message,
        "review finding status transition must change status"
    );

    let db_file_name = state
        .db_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("db path should have UTF-8 file name");
    let malformed_path = state
        .db_path
        .with_file_name(format!("{db_file_name}.review-runs"))
        .join("v1")
        .join("malformed.json");
    fs::write(
        &malformed_path,
        serde_json::to_string_pretty(&json!({
            "review_id": "",
            "title": "Malformed",
            "kind": "audit",
            "audit": "dangling-links",
            "findings": []
        }))
        .expect("malformed review fixture should serialize"),
    )
    .expect("malformed review fixture should be written");

    let malformed = review_run(&mut state, json!({ "review_id": "malformed" }))
        .expect_err("malformed stored review should be rejected");
    assert!(
        malformed
            .into_inner()
            .message
            .contains("failed to load review run")
    );
}

#[test]
fn save_corpus_audit_review_rpc_persists_typed_audit_evidence() {
    let (_workspace, mut state) = audit_state();

    let saved: SaveCorpusAuditReviewResult = serde_json::from_value(
        save_corpus_audit_review(
            &mut state,
            json!({
                "audit": "dangling-links",
                "limit": 20,
                "review_id": "review/audit/dangling-links/custom",
                "title": "Custom Dangling Review",
                "overwrite": true
            }),
        )
        .expect("save audit review RPC should succeed"),
    )
    .expect("save audit review result should decode");
    assert_eq!(saved.result.audit, CorpusAuditKind::DanglingLinks);
    assert_eq!(saved.result.entries.len(), 1);
    assert_eq!(
        saved.review.metadata.review_id,
        "review/audit/dangling-links/custom"
    );
    assert_eq!(saved.review.finding_count, saved.result.entries.len());
    assert_eq!(saved.review.status_counts.open, saved.result.entries.len());

    let root = state.root.clone();
    let db_path = state.db_path.clone();
    let discovery = state.discovery.clone();
    drop(state);

    let mut reopened =
        ServerState::new(root, db_path, Vec::new(), discovery).expect("state should reopen");
    let inspected: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut reopened,
            json!({ "review_id": "review/audit/dangling-links/custom" }),
        )
        .expect("saved audit review should load after reopen"),
    )
    .expect("review result should decode");
    assert_eq!(inspected.review.metadata.title, "Custom Dangling Review");
    match inspected.review.payload {
        ReviewRunPayload::Audit { audit, limit } => {
            assert_eq!(audit, CorpusAuditKind::DanglingLinks);
            assert_eq!(limit, 20);
        }
        other => panic!("expected audit review payload, got {:?}", other.kind()),
    }
    assert_eq!(inspected.review.findings.len(), saved.result.entries.len());
    match &saved.result.entries[0] {
        CorpusAuditEntry::DanglingLink { record } => {
            assert_eq!(
                inspected.review.findings[0].finding_id,
                format!(
                    "audit/dangling-links/{}/{}/{}/{}",
                    record.source.node_key, record.missing_explicit_id, record.line, record.column
                )
            );
        }
        other => panic!("expected dangling-link result, got {:?}", other.kind()),
    }
    assert_eq!(
        inspected.review.findings[0].status,
        ReviewFindingStatus::Open
    );
    match &inspected.review.findings[0].payload {
        ReviewFindingPayload::Audit { entry } => {
            assert_eq!(entry.as_ref(), &saved.result.entries[0]);
        }
        other => panic!("expected audit finding payload, got {:?}", other.kind()),
    }

    let conflict = save_corpus_audit_review(
        &mut reopened,
        json!({
            "audit": "dangling-links",
            "limit": 20,
            "review_id": "review/audit/dangling-links/custom",
            "overwrite": false
        }),
    )
    .expect_err("non-overwrite audit review save should reject replacement");
    assert_eq!(
        conflict.into_inner().message,
        "review run already exists: review/audit/dangling-links/custom"
    );
}

#[test]
fn save_workflow_review_rpc_persists_typed_workflow_evidence() {
    let (_workspace, mut state, focus_key) = indexed_state();

    let saved: SaveWorkflowReviewResult = serde_json::from_value(
        save_workflow_review(
            &mut state,
            json!({
                "workflow_id": BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
                "inputs": [{
                    "input_id": "focus",
                    "kind": "node-key",
                    "node_key": focus_key
                }],
                "title": "Unresolved Sweep Review",
                "overwrite": true
            }),
        )
        .expect("save workflow review RPC should succeed"),
    )
    .expect("save workflow review result should decode");
    assert_eq!(
        saved.result.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
    );
    assert_eq!(saved.result.steps.len(), 4);
    assert_eq!(saved.review.finding_count, saved.result.steps.len());
    assert_eq!(saved.review.status_counts.open, saved.result.steps.len());
    assert!(
        saved
            .review
            .metadata
            .review_id
            .starts_with("review/workflow/builtin/unresolved-sweep/inputs-")
    );

    let root = state.root.clone();
    let db_path = state.db_path.clone();
    let discovery = state.discovery.clone();
    drop(state);

    let mut reopened =
        ServerState::new(root, db_path, Vec::new(), discovery).expect("state should reopen");
    let inspected: ReviewRunResult = serde_json::from_value(
        review_run(
            &mut reopened,
            json!({ "review_id": saved.review.metadata.review_id }),
        )
        .expect("saved workflow review should load after reopen"),
    )
    .expect("review result should decode");
    assert_eq!(inspected.review.metadata.title, "Unresolved Sweep Review");
    match &inspected.review.payload {
        ReviewRunPayload::Workflow {
            workflow,
            inputs,
            step_ids,
        } => {
            assert_eq!(
                workflow.metadata.workflow_id,
                BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
            );
            assert_eq!(inputs.len(), 1);
            assert_eq!(inputs[0].input_id, "focus");
            assert_eq!(
                step_ids,
                &saved
                    .result
                    .steps
                    .iter()
                    .map(|step| step.step_id.clone())
                    .collect::<Vec<_>>()
            );
        }
        other => panic!("expected workflow review payload, got {:?}", other.kind()),
    }
    assert_eq!(inspected.review.findings.len(), saved.result.steps.len());
    assert_eq!(
        inspected.review.findings[0].finding_id,
        format!("workflow-step/{}", saved.result.steps[0].step_id)
    );
    match &inspected.review.findings[0].payload {
        ReviewFindingPayload::WorkflowStep { step } => {
            assert_eq!(step.as_ref(), &saved.result.steps[0]);
        }
        other => panic!(
            "expected workflow-step finding payload, got {:?}",
            other.kind()
        ),
    }

    let unknown = save_workflow_review(
        &mut reopened,
        json!({
            "workflow_id": "workflow/builtin/missing",
            "overwrite": true
        }),
    )
    .expect_err("unknown workflow should be rejected");
    assert_eq!(
        unknown.into_inner().message,
        "unknown workflow: workflow/builtin/missing"
    );
}

#[test]
fn save_workflow_review_rejects_existing_review_before_artifact_save_side_effects() {
    let (_workspace, mut state, left_key, right_key) = comparison_state();
    let workflow_dir = state.root.join("workflows");
    fs::create_dir_all(&workflow_dir).expect("workflow dir should be created");
    state.workflow_dirs = vec![workflow_dir.clone()];

    let workflow_id = "workflow/test/review-save-side-effect";
    let artifact_id = "workflow-review-side-effect";
    let workflow = WorkflowSpec {
        metadata: slipbox_core::WorkflowMetadata {
            workflow_id: workflow_id.to_owned(),
            title: "Review Save Side Effect".to_owned(),
            summary: Some("Would save an artifact if it reaches execution".to_owned()),
        },
        compatibility: slipbox_core::WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![
            slipbox_core::WorkflowStepSpec {
                step_id: "resolve-left".to_owned(),
                payload: slipbox_core::WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::NodeKey {
                        node_key: left_key.clone(),
                    },
                },
            },
            slipbox_core::WorkflowStepSpec {
                step_id: "resolve-right".to_owned(),
                payload: slipbox_core::WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::NodeKey {
                        node_key: right_key.clone(),
                    },
                },
            },
            slipbox_core::WorkflowStepSpec {
                step_id: "compare".to_owned(),
                payload: slipbox_core::WorkflowStepPayload::Compare {
                    left: slipbox_core::WorkflowStepRef {
                        step_id: "resolve-left".to_owned(),
                    },
                    right: slipbox_core::WorkflowStepRef {
                        step_id: "resolve-right".to_owned(),
                    },
                    group: NoteComparisonGroup::Overlap,
                    limit: 10,
                },
            },
            slipbox_core::WorkflowStepSpec {
                step_id: "save-artifact".to_owned(),
                payload: slipbox_core::WorkflowStepPayload::ArtifactSave {
                    source: slipbox_core::WorkflowArtifactSaveSource::CompareStep {
                        step_id: "compare".to_owned(),
                    },
                    metadata: ExplorationArtifactMetadata {
                        artifact_id: artifact_id.to_owned(),
                        title: "Workflow Review Side Effect".to_owned(),
                        summary: None,
                    },
                    overwrite: true,
                },
            },
        ],
    };
    fs::write(
        workflow_dir.join("side-effect.json"),
        serde_json::to_vec_pretty(&workflow).expect("workflow should serialize"),
    )
    .expect("workflow spec should be written");

    let existing_review_id = "review/workflow/side-effect";
    state
        .database
        .save_review_run(&sample_audit_review_run(
            existing_review_id,
            "Existing Review",
            ReviewFindingStatus::Open,
        ))
        .expect("existing review should be saved");
    assert!(
        state
            .database
            .exploration_artifact(artifact_id)
            .expect("artifact lookup should succeed")
            .is_none()
    );

    let conflict = save_workflow_review(
        &mut state,
        json!({
            "workflow_id": workflow_id,
            "review_id": existing_review_id,
            "overwrite": false
        }),
    )
    .expect_err("existing review should be rejected before workflow execution");
    assert_eq!(
        conflict.into_inner().message,
        format!("review run already exists: {existing_review_id}")
    );
    assert!(
        state
            .database
            .exploration_artifact(artifact_id)
            .expect("artifact lookup should still succeed")
            .is_none()
    );
}

#[test]
fn execute_workflow_spec_runs_all_supported_step_kinds() {
    let (_workspace, mut state, left_key, right_key) = comparison_state();
    let workflow = WorkflowSpec {
        metadata: slipbox_core::WorkflowMetadata {
            workflow_id: "workflow/test-all-kinds".to_owned(),
            title: "All Kinds".to_owned(),
            summary: Some("Exercise all workflow step kinds".to_owned()),
        },
        compatibility: slipbox_core::WorkflowSpecCompatibility::default(),
        inputs: vec![
            slipbox_core::WorkflowInputSpec {
                input_id: "left".to_owned(),
                title: "Left".to_owned(),
                summary: None,
                kind: slipbox_core::WorkflowInputKind::NoteTarget,
            },
            slipbox_core::WorkflowInputSpec {
                input_id: "right".to_owned(),
                title: "Right".to_owned(),
                summary: None,
                kind: slipbox_core::WorkflowInputKind::NoteTarget,
            },
        ],
        steps: vec![
            slipbox_core::WorkflowStepSpec {
                step_id: "resolve-left".to_owned(),
                payload: slipbox_core::WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "left".to_owned(),
                    },
                },
            },
            slipbox_core::WorkflowStepSpec {
                step_id: "resolve-right".to_owned(),
                payload: slipbox_core::WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "right".to_owned(),
                    },
                },
            },
            slipbox_core::WorkflowStepSpec {
                step_id: "compare".to_owned(),
                payload: slipbox_core::WorkflowStepPayload::Compare {
                    left: slipbox_core::WorkflowStepRef {
                        step_id: "resolve-left".to_owned(),
                    },
                    right: slipbox_core::WorkflowStepRef {
                        step_id: "resolve-right".to_owned(),
                    },
                    group: NoteComparisonGroup::Tension,
                    limit: 10,
                },
            },
            slipbox_core::WorkflowStepSpec {
                step_id: "save".to_owned(),
                payload: slipbox_core::WorkflowStepPayload::ArtifactSave {
                    source: slipbox_core::WorkflowArtifactSaveSource::CompareStep {
                        step_id: "compare".to_owned(),
                    },
                    metadata: ExplorationArtifactMetadata {
                        artifact_id: "workflow-saved-comparison".to_owned(),
                        title: "Workflow Saved Comparison".to_owned(),
                        summary: None,
                    },
                    overwrite: false,
                },
            },
            slipbox_core::WorkflowStepSpec {
                step_id: "run-saved".to_owned(),
                payload: slipbox_core::WorkflowStepPayload::ArtifactRun {
                    artifact_id: "workflow-saved-comparison".to_owned(),
                },
            },
        ],
    };

    let result = execute_workflow_spec(
        &mut state,
        &workflow,
        &[
            WorkflowInputAssignment {
                input_id: "left".to_owned(),
                target: WorkflowResolveTarget::NodeKey {
                    node_key: left_key.clone(),
                },
            },
            WorkflowInputAssignment {
                input_id: "right".to_owned(),
                target: WorkflowResolveTarget::NodeKey {
                    node_key: right_key.clone(),
                },
            },
        ],
    )
    .expect("workflow execution should succeed");

    assert_eq!(
        result.workflow.metadata.workflow_id,
        "workflow/test-all-kinds"
    );
    assert_eq!(
        result
            .steps
            .iter()
            .map(WorkflowStepReport::kind)
            .collect::<Vec<_>>(),
        vec![
            slipbox_core::WorkflowStepKind::Resolve,
            slipbox_core::WorkflowStepKind::Resolve,
            slipbox_core::WorkflowStepKind::Compare,
            slipbox_core::WorkflowStepKind::ArtifactSave,
            slipbox_core::WorkflowStepKind::ArtifactRun,
        ]
    );
    match &result.steps[4].payload {
        WorkflowStepReportPayload::ArtifactRun { artifact } => {
            assert_eq!(artifact.metadata.artifact_id, "workflow-saved-comparison");
            assert!(matches!(
                artifact.payload,
                ExecutedExplorationArtifactPayload::Comparison { .. }
            ));
        }
        other => panic!("expected artifact-run report, got {:?}", other.kind()),
    }
}

#[test]
fn workflow_rpc_lists_shows_and_runs_built_ins() {
    let (_workspace, mut state, _target_key) = indexed_state();
    let anchor_key = state
        .database
        .anchor_at_point("alpha.org", 31)
        .expect("anchor lookup should succeed")
        .expect("anonymous heading anchor should exist")
        .node_key;

    let listed: ListWorkflowsResult = serde_json::from_value(
        list_workflows(&mut state, json!({})).expect("list workflows RPC should succeed"),
    )
    .expect("list workflows result should decode");
    assert_eq!(listed.workflows.len(), 5);
    assert_eq!(
        listed.workflows[0].metadata.workflow_id,
        BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID
    );
    assert_eq!(
        listed.workflows[2].metadata.workflow_id,
        BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID
    );
    assert_eq!(
        listed.workflows[3].metadata.workflow_id,
        BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID
    );
    assert_eq!(
        listed.workflows[4].metadata.workflow_id,
        BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID
    );

    let shown: WorkflowResult = serde_json::from_value(
        workflow(
            &mut state,
            json!({ "workflow_id": BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID }),
        )
        .expect("workflow RPC should succeed"),
    )
    .expect("workflow result should decode");
    assert_eq!(
        shown.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
    );
    assert_eq!(shown.workflow.inputs.len(), 1);

    let executed_anchor_key = anchor_key.clone();
    let executed: RunWorkflowResult = serde_json::from_value(
        run_workflow(
            &mut state,
            json!({
                "workflow_id": BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
                "inputs": [
                    {
                        "input_id": "focus",
                        "kind": "node-key",
                        "node_key": anchor_key,
                    }
                ]
            }),
        )
        .expect("run workflow RPC should succeed"),
    )
    .expect("run workflow result should decode");
    assert_eq!(
        executed.result.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
    );
    assert_eq!(executed.result.steps.len(), 4);
    assert_eq!(executed.result.steps[0].kind().label(), "resolve");
    match &executed.result.steps[1].payload {
        WorkflowStepReportPayload::Explore { result, .. } => {
            assert_eq!(result.lens, ExplorationLens::Unresolved);
        }
        other => panic!("expected unresolved explore report, got {:?}", other.kind()),
    }
    match &executed.result.steps[2].payload {
        WorkflowStepReportPayload::Explore {
            focus_node_key,
            result,
        } => {
            assert_eq!(focus_node_key, &executed_anchor_key);
            assert_eq!(result.lens, ExplorationLens::Tasks);
        }
        other => panic!("expected tasks explore report, got {:?}", other.kind()),
    }

    let weak: RunWorkflowResult = serde_json::from_value(
        run_workflow(
            &mut state,
            json!({
                "workflow_id": BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID,
                "inputs": [
                    {
                        "input_id": "focus",
                        "kind": "node-key",
                        "node_key": executed_anchor_key,
                    }
                ]
            }),
        )
        .expect("weak integration review workflow should run"),
    )
    .expect("weak integration workflow result should decode");
    assert_eq!(
        weak.result.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID
    );
    assert_eq!(weak.result.steps.len(), 4);
    match &weak.result.steps[1].payload {
        WorkflowStepReportPayload::Explore { result, .. } => {
            assert_eq!(result.lens, ExplorationLens::Unresolved);
        }
        other => panic!(
            "expected weak integration explore report, got {:?}",
            other.kind()
        ),
    }
}

#[test]
fn workflow_rpc_reports_lookup_and_step_failures_with_context() {
    let (_workspace, mut state, target_key) = indexed_state();
    let anchor_key = state
        .database
        .anchor_at_point("alpha.org", 18)
        .expect("anchor lookup should succeed")
        .expect("anonymous heading anchor should exist")
        .node_key;

    let unknown_workflow = workflow(
        &mut state,
        json!({ "workflow_id": "workflow/builtin/missing" }),
    )
    .expect_err("unknown workflow should fail");
    assert_eq!(
        unknown_workflow.into_inner().message,
        "unknown workflow: workflow/builtin/missing"
    );

    let step_failure = run_workflow(
        &mut state,
        json!({
            "workflow_id": BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
            "inputs": [
                {
                    "input_id": "focus",
                    "kind": "id",
                    "id": "missing-id"
                }
            ]
        }),
    )
    .expect_err("missing workflow input target should fail");
    assert_eq!(
        step_failure.into_inner().message,
        "workflow step resolve-focus failed: unknown workflow focus target: missing-id"
    );

    let note_target_anchor_failure = run_workflow(
        &mut state,
        json!({
            "workflow_id": BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID,
            "inputs": [
                {
                    "input_id": "left",
                    "kind": "node-key",
                    "node_key": anchor_key,
                },
                {
                    "input_id": "right",
                    "kind": "node-key",
                    "node_key": target_key,
                }
            ]
        }),
    )
    .expect_err("note-target workflow inputs should reject anchor node keys");
    assert_eq!(
        note_target_anchor_failure.into_inner().message,
        format!("workflow step resolve-left failed: unknown workflow note target: {anchor_key}")
    );
}

#[test]
fn corpus_audit_rpc_dispatches_index_backed_audit_kinds() {
    let (_workspace, mut state) = audit_state();

    let dangling: CorpusAuditResult = serde_json::from_value(
        corpus_audit(
            &mut state,
            json!({ "audit": "dangling-links", "limit": 20 }),
        )
        .expect("dangling link audit should succeed"),
    )
    .expect("dangling audit result should decode");
    assert_eq!(dangling.audit, CorpusAuditKind::DanglingLinks);
    assert_eq!(dangling.entries.len(), 1);
    match &dangling.entries[0] {
        CorpusAuditEntry::DanglingLink { record } => {
            assert_eq!(record.source.title, "Dangling Source");
            assert_eq!(record.missing_explicit_id, "missing-id");
        }
        other => panic!("expected dangling link audit entry, got {:?}", other.kind()),
    }

    let duplicates: CorpusAuditResult = serde_json::from_value(
        corpus_audit(
            &mut state,
            json!({ "audit": "duplicate-titles", "limit": 20 }),
        )
        .expect("duplicate title audit should succeed"),
    )
    .expect("duplicate audit result should decode");
    assert_eq!(duplicates.audit, CorpusAuditKind::DuplicateTitles);
    assert_eq!(duplicates.entries.len(), 1);
    match &duplicates.entries[0] {
        CorpusAuditEntry::DuplicateTitle { record } => {
            assert_eq!(record.title, "Shared Title");
            assert_eq!(record.notes.len(), 2);
        }
        other => panic!(
            "expected duplicate title audit entry, got {:?}",
            other.kind()
        ),
    }

    let orphans: CorpusAuditResult = serde_json::from_value(
        corpus_audit(&mut state, json!({ "audit": "orphan-notes", "limit": 20 }))
            .expect("orphan note audit should succeed"),
    )
    .expect("orphan audit result should decode");
    assert_eq!(orphans.audit, CorpusAuditKind::OrphanNotes);
    assert_eq!(orphans.entries.len(), 1);
    match &orphans.entries[0] {
        CorpusAuditEntry::OrphanNote { record } => {
            assert_eq!(record.note.title, "Orphan");
            assert_eq!(record.reference_count, 0);
            assert_eq!(record.backlink_count, 0);
            assert_eq!(record.forward_link_count, 0);
        }
        other => panic!("expected orphan note audit entry, got {:?}", other.kind()),
    }

    let weak: CorpusAuditResult = serde_json::from_value(
        corpus_audit(
            &mut state,
            json!({ "audit": "weakly-integrated-notes", "limit": 20 }),
        )
        .expect("weakly integrated note audit should succeed"),
    )
    .expect("weak audit result should decode");
    assert_eq!(weak.audit, CorpusAuditKind::WeaklyIntegratedNotes);
    assert_eq!(weak.entries.len(), 1);
    match &weak.entries[0] {
        CorpusAuditEntry::WeaklyIntegratedNote { record } => {
            assert_eq!(record.note.title, "Weak");
            assert_eq!(record.reference_count, 1);
            assert_eq!(record.backlink_count, 0);
            assert_eq!(record.forward_link_count, 0);
        }
        other => panic!(
            "expected weakly integrated note audit entry, got {:?}",
            other.kind()
        ),
    }
}

fn saved_lens_artifact(
    artifact_id: &str,
    title: &str,
    node_key: &str,
    lens: ExplorationLens,
) -> SavedExplorationArtifact {
    SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: artifact_id.to_owned(),
            title: title.to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: node_key.to_owned(),
                current_node_key: node_key.to_owned(),
                lens,
                limit: 20,
                unique: false,
                frozen_context: false,
            }),
        },
    }
}

fn saved_comparison_artifact(
    artifact_id: &str,
    title: &str,
    left_node_key: &str,
    right_node_key: &str,
) -> SavedExplorationArtifact {
    SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: artifact_id.to_owned(),
            title: title.to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::Comparison {
            artifact: Box::new(SavedComparisonArtifact {
                root_node_key: left_node_key.to_owned(),
                left_node_key: left_node_key.to_owned(),
                right_node_key: right_node_key.to_owned(),
                active_lens: ExplorationLens::Structure,
                structure_unique: false,
                comparison_group: Default::default(),
                limit: 20,
                frozen_context: false,
            }),
        },
    }
}

fn sample_pack_workflow(workflow_id: &str, title: &str, node_key: &str) -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: workflow_id.to_owned(),
            title: title.to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![WorkflowStepSpec {
            step_id: "explore-pack-focus".to_owned(),
            payload: WorkflowStepPayload::Explore {
                focus: slipbox_core::WorkflowExploreFocus::NodeKey {
                    node_key: node_key.to_owned(),
                },
                lens: ExplorationLens::Refs,
                limit: 20,
                unique: false,
            },
        }],
    }
}

fn sample_input_workflow(workflow_id: &str, title: &str) -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: workflow_id.to_owned(),
            title: title.to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![slipbox_core::WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus".to_owned(),
            summary: None,
            kind: slipbox_core::WorkflowInputKind::NoteTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-focus".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: slipbox_core::WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 20,
                    unique: false,
                },
            },
        ],
    }
}

fn sample_artifact_save_workflow(workflow_id: &str, node_key: &str) -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: workflow_id.to_owned(),
            title: "Artifact Save Workflow".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![
            WorkflowStepSpec {
                step_id: "explore-focus".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: slipbox_core::WorkflowExploreFocus::NodeKey {
                        node_key: node_key.to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 20,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "save-artifact".to_owned(),
                payload: WorkflowStepPayload::ArtifactSave {
                    source: slipbox_core::WorkflowArtifactSaveSource::ExploreStep {
                        step_id: "explore-focus".to_owned(),
                    },
                    metadata: ExplorationArtifactMetadata {
                        artifact_id: "routine-conflict-artifact".to_owned(),
                        title: "Routine Conflict Artifact".to_owned(),
                        summary: None,
                    },
                    overwrite: false,
                },
            },
        ],
    }
}

fn sample_routine_report_profile(profile_id: &str) -> ReportProfileSpec {
    ReportProfileSpec {
        metadata: ReportProfileMetadata {
            profile_id: profile_id.to_owned(),
            title: "Routine Detail".to_owned(),
            summary: None,
        },
        subjects: vec![
            ReportProfileSubject::Routine,
            ReportProfileSubject::Audit,
            ReportProfileSubject::Review,
            ReportProfileSubject::Diff,
        ],
        mode: ReportProfileMode::Detail,
        status_filters: Some(vec![ReviewFindingStatus::Open]),
        diff_buckets: Some(vec![ReviewRunDiffBucket::Unchanged]),
        jsonl_line_kinds: Some(vec![
            ReportJsonlLineKind::Routine,
            ReportJsonlLineKind::Audit,
            ReportJsonlLineKind::Entry,
            ReportJsonlLineKind::Review,
            ReportJsonlLineKind::Finding,
            ReportJsonlLineKind::Diff,
            ReportJsonlLineKind::Unchanged,
        ]),
    }
}

fn sample_routine_only_review_profile(profile_id: &str) -> ReportProfileSpec {
    ReportProfileSpec {
        metadata: ReportProfileMetadata {
            profile_id: profile_id.to_owned(),
            title: "Routine Review Lines".to_owned(),
            summary: None,
        },
        subjects: vec![ReportProfileSubject::Routine],
        mode: ReportProfileMode::Detail,
        status_filters: None,
        diff_buckets: None,
        jsonl_line_kinds: Some(vec![
            ReportJsonlLineKind::Review,
            ReportJsonlLineKind::Finding,
        ]),
    }
}

fn sample_audit_review_routine(routine_id: &str, profile_id: &str) -> ReviewRoutineSpec {
    ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: routine_id.to_owned(),
            title: "Duplicate Title Routine".to_owned(),
            summary: None,
        },
        source: ReviewRoutineSource::Audit {
            audit: CorpusAuditKind::DuplicateTitles,
            limit: 20,
        },
        inputs: Vec::new(),
        save_review: ReviewRoutineSaveReviewPolicy {
            enabled: true,
            review_id: Some("review/routine/001-current".to_owned()),
            title: Some("Routine Duplicate Title Review".to_owned()),
            summary: None,
            overwrite: false,
        },
        compare: Some(ReviewRoutineComparePolicy {
            target: ReviewRoutineCompareTarget::LatestCompatibleReview,
            report_profile_id: Some(profile_id.to_owned()),
        }),
        report_profile_ids: vec![profile_id.to_owned()],
    }
}

fn sample_workflow_review_routine(
    routine_id: &str,
    workflow_id: &str,
    review_id: Option<&str>,
) -> ReviewRoutineSpec {
    ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: routine_id.to_owned(),
            title: "Workflow Routine".to_owned(),
            summary: None,
        },
        source: ReviewRoutineSource::Workflow {
            workflow_id: workflow_id.to_owned(),
        },
        inputs: vec![slipbox_core::WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus".to_owned(),
            summary: None,
            kind: slipbox_core::WorkflowInputKind::NoteTarget,
        }],
        save_review: ReviewRoutineSaveReviewPolicy {
            enabled: review_id.is_some(),
            review_id: review_id.map(str::to_owned),
            title: None,
            summary: None,
            overwrite: false,
        },
        compare: None,
        report_profile_ids: Vec::new(),
    }
}

fn sample_workbench_pack(pack_id: &str, title: &str) -> WorkbenchPackManifest {
    WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: pack_id.to_owned(),
            title: title.to_owned(),
            summary: Some("Reusable workbench assets".to_owned()),
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: Vec::new(),
        review_routines: Vec::new(),
        report_profiles: vec![ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: format!("{pack_id}/profile/detail"),
                title: "Detail Report".to_owned(),
                summary: None,
            },
            subjects: vec![ReportProfileSubject::Audit],
            mode: ReportProfileMode::Detail,
            status_filters: None,
            diff_buckets: None,
            jsonl_line_kinds: None,
        }],
        entrypoint_routine_ids: Vec::new(),
    }
}

fn sample_audit_review_run(review_id: &str, title: &str, status: ReviewFindingStatus) -> ReviewRun {
    ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: review_id.to_owned(),
            title: title.to_owned(),
            summary: Some("Review dangling links".to_owned()),
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 200,
        },
        findings: vec![ReviewFinding {
            finding_id: "audit/dangling-links/source/missing-id".to_owned(),
            status,
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

fn indexed_state() -> (TempDir, ServerState, String) {
    let workspace = tempfile::tempdir().expect("workspace should be created");
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root).expect("notes root should be created");
    fs::write(
        root.join("alpha.org"),
        r#"#+title: Alpha

* Source
:PROPERTIES:
:ID: source-id
:END:
Points to [[id:target-id]].

* TODO Target
:PROPERTIES:
:ID: target-id
:ROAM_REFS: cite:smith2024
:END:
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Target body.

* Reflink Source
This mentions cite:smith2024 near Target.

* TODO Dual Match Peer
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Shares both planning dates and task state.

* NEXT Cross Match Peer
SCHEDULED: <2026-05-03 Sat>
DEADLINE: <2026-05-01 Thu>
Shares both planning dates through opposite fields.

* TODO Keyword Only Peer
Shares only the same task state.

* WAIT Deadline Peer
DEADLINE: <2026-05-03 Sat>
Shares only the target deadline.
"#,
    )
    .expect("fixture should be written");

    let db_path = workspace.path().join("index.sqlite3");
    let discovery = DiscoveryPolicy::default();
    let mut state = ServerState::new(root.clone(), db_path, Vec::new(), discovery)
        .expect("state should be created");
    let files = scan_root_with_policy(&root, &state.discovery).expect("fixture should be indexed");
    state
        .database
        .sync_index(&files)
        .expect("fixture index should sync");
    let target_key = state
        .database
        .node_from_id("target-id")
        .expect("target note lookup should succeed")
        .expect("target note should exist")
        .node_key;

    (workspace, state, target_key)
}

fn comparison_state() -> (TempDir, ServerState, String, String) {
    let workspace = tempfile::tempdir().expect("workspace should be created");
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root).expect("notes root should be created");
    fs::write(
        root.join("comparison.org"),
        r#"#+title: Comparison

* TODO Left
:PROPERTIES:
:ID: left-id
:ROAM_REFS: cite:shared2024 cite:left2024
:END:
SCHEDULED: <2026-05-01 Thu>
Links to [[id:shared-forward-id]] and [[id:left-right-bridge-id]].

* NEXT Right
:PROPERTIES:
:ID: right-id
:ROAM_REFS: cite:shared2024 cite:right2024
:END:
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-01 Thu>
Links to [[id:shared-forward-id]] and [[id:right-left-bridge-id]].

* Shared Forward
:PROPERTIES:
:ID: shared-forward-id
:END:
Forward target body.

* Left To Right Bridge
:PROPERTIES:
:ID: left-right-bridge-id
:END:
Connects to [[id:right-id]].

* Right To Left Bridge
:PROPERTIES:
:ID: right-left-bridge-id
:END:
Connects to [[id:left-id]].

* Shared Backlink
:PROPERTIES:
:ID: shared-backlink-id
:END:
Links to [[id:left-id]] and [[id:right-id]].
"#,
    )
    .expect("fixture should be written");

    let db_path = workspace.path().join("index.sqlite3");
    let discovery = DiscoveryPolicy::default();
    let mut state = ServerState::new(root.clone(), db_path, Vec::new(), discovery)
        .expect("state should be created");
    let files = scan_root_with_policy(&root, &state.discovery).expect("fixture should be indexed");
    state
        .database
        .sync_index(&files)
        .expect("fixture index should sync");
    let left_key = state
        .database
        .node_from_id("left-id")
        .expect("left note lookup should succeed")
        .expect("left note should exist")
        .node_key;
    let right_key = state
        .database
        .node_from_id("right-id")
        .expect("right note lookup should succeed")
        .expect("right note should exist")
        .node_key;

    (workspace, state, left_key, right_key)
}

fn audit_state() -> (TempDir, ServerState) {
    let workspace = tempfile::tempdir().expect("workspace should be created");
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root).expect("notes root should be created");
    fs::write(
        root.join("duplicate-a.org"),
        r#":PROPERTIES:
:ID: dup-a-id
:END:
#+title: Shared Title

Links to [[id:dup-b-id][Other duplicate]].
"#,
    )
    .expect("fixture should be written");
    fs::write(
        root.join("duplicate-b.org"),
        r#":PROPERTIES:
:ID: dup-b-id
:END:
#+title: shared title

Links to [[id:dup-a-id][Other duplicate]].
"#,
    )
    .expect("fixture should be written");
    fs::write(
        root.join("dangling-source.org"),
        r#":PROPERTIES:
:ID: dangling-source-id
:END:
#+title: Dangling Source

Points to [[id:missing-id][Missing]].
"#,
    )
    .expect("fixture should be written");
    fs::write(
        root.join("orphan.org"),
        r#":PROPERTIES:
:ID: orphan-id
:END:
#+title: Orphan

Just an orphan note.
"#,
    )
    .expect("fixture should be written");
    fs::write(
        root.join("weak.org"),
        r#":PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:weak2024
:END:
#+title: Weak

Has refs but no structural links.
"#,
    )
    .expect("fixture should be written");

    let db_path = workspace.path().join("index.sqlite3");
    let discovery = DiscoveryPolicy::default();
    let mut state = ServerState::new(root.clone(), db_path, Vec::new(), discovery)
        .expect("state should be created");
    let files = scan_root_with_policy(&root, &state.discovery).expect("fixture should be indexed");
    state
        .database
        .sync_index(&files)
        .expect("fixture index should sync");

    (workspace, state)
}

fn non_obvious_state() -> (TempDir, ServerState, String) {
    let workspace = tempfile::tempdir().expect("workspace should be created");
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root).expect("notes root should be created");
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
    .expect("older fixture should be written");
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
    .expect("focus fixture should be written");
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
    .expect("related fixture should be written");

    let db_path = workspace.path().join("index.sqlite3");
    let discovery = DiscoveryPolicy::default();
    let mut state = ServerState::new(root.clone(), db_path, Vec::new(), discovery)
        .expect("state should be created");
    let files = scan_root_with_policy(&root, &state.discovery).expect("fixture should be indexed");
    state
        .database
        .sync_index(&files)
        .expect("fixture index should sync");
    let focus_key = state
        .database
        .node_from_id("focus-id")
        .expect("focus note lookup should succeed")
        .expect("focus note should exist")
        .node_key;

    (workspace, state, focus_key)
}
