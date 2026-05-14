use super::{
    AnchorRecord, AuditRemediationApplyAction, AuditRemediationPreviewIdentity,
    BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID, BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID,
    BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID, BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
    BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID, BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
    BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID, BacklinkRecord, BridgeEvidenceRecord,
    CaptureNodeParams, CaptureTemplatePreviewResult, CompareNotesParams,
    ComparisonConnectorDirection, ComparisonPlanningRecord, ComparisonReferenceRecord,
    ComparisonTaskStateRecord, CorpusAuditEntry, CorpusAuditKind, CorpusAuditParams,
    CorpusAuditReportLine, CorpusAuditResult, DanglingLinkAuditRecord,
    DeleteExplorationArtifactResult, DeleteWorkbenchPackResult, DuplicateTitleAuditRecord,
    ExecuteExplorationArtifactResult, ExecutedExplorationArtifact,
    ExecutedExplorationArtifactPayload, ExplorationArtifactIdParams, ExplorationArtifactKind,
    ExplorationArtifactMetadata, ExplorationArtifactPayload, ExplorationArtifactResult,
    ExplorationArtifactSummary, ExplorationEntry, ExplorationExplanation, ExplorationLens,
    ExplorationSection, ExplorationSectionKind, ExploreParams, ExploreResult,
    ImportWorkbenchPackParams, ImportWorkbenchPackResult, ListExplorationArtifactsResult,
    ListWorkbenchPacksResult, MarkReviewFindingParams, NodeFromKeyParams,
    NodeFromTitleOrAliasParams, NodeKind, NodeRecord, NoteComparisonEntry,
    NoteComparisonExplanation, NoteComparisonGroup, NoteComparisonResult, NoteComparisonSection,
    NoteComparisonSectionKind, NoteConnectivityAuditRecord, PlanningField, PlanningRelationRecord,
    PreviewNodeRecord, ReportJsonlLineKind, ReportProfileCatalog, ReportProfileMetadata,
    ReportProfileMode, ReportProfileSpec, ReportProfileSubject, ReviewFinding,
    ReviewFindingPayload, ReviewFindingRemediationApplication, ReviewFindingRemediationApplyParams,
    ReviewFindingRemediationApplyResult, ReviewFindingRemediationPreview,
    ReviewFindingRemediationPreviewParams, ReviewFindingRemediationPreviewResult,
    ReviewFindingStatus, ReviewFindingStatusTransition, ReviewRoutineCatalog,
    ReviewRoutineComparePolicy, ReviewRoutineCompareTarget, ReviewRoutineMetadata,
    ReviewRoutineSaveReviewPolicy, ReviewRoutineSource, ReviewRoutineSpec, ReviewRun,
    ReviewRunDiff, ReviewRunDiffBucket, ReviewRunDiffParams, ReviewRunDiffResult,
    ReviewRunIdParams, ReviewRunMetadata, ReviewRunPayload, ReviewRunResult, ReviewRunSummary,
    SaveCorpusAuditReviewParams, SaveCorpusAuditReviewResult, SaveExplorationArtifactParams,
    SaveExplorationArtifactResult, SaveReviewRunParams, SaveReviewRunResult,
    SaveWorkflowReviewParams, SaveWorkflowReviewResult, SavedComparisonArtifact,
    SavedExplorationArtifact, SavedLensViewArtifact, SavedTrailArtifact, SavedTrailStep,
    SearchNodesParams, SearchNodesSort, SlipboxLinkRewriteApplication,
    SlipboxLinkRewriteAppliedEntry, SlipboxLinkRewriteApplyParams, SlipboxLinkRewriteApplyResult,
    SlipboxLinkRewritePreview, SlipboxLinkRewritePreviewEntry, SlipboxLinkRewritePreviewResult,
    StructuralWriteAffectedFiles, StructuralWriteIndexRefreshStatus, StructuralWriteOperationKind,
    StructuralWritePreview, StructuralWritePreviewResult, StructuralWriteReport,
    StructuralWriteResult, TrailReplayResult, TrailReplayStepResult, UnlinkedReferencesParams,
    UpdateNodeMetadataParams, ValidateWorkbenchPackParams, ValidateWorkbenchPackResult,
    WorkbenchPackCompatibility, WorkbenchPackCompatibilityEnvelope, WorkbenchPackIdParams,
    WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackMetadata, WorkbenchPackResult,
    WorkbenchPackSummary, WorkflowArtifactSaveSource, WorkflowExecutionResult,
    WorkflowExploreFocus, WorkflowInputAssignment, WorkflowInputKind, WorkflowInputSpec,
    WorkflowMetadata, WorkflowReportLine, WorkflowResolveTarget, WorkflowSpec,
    WorkflowSpecCompatibility, WorkflowSpecCompatibilityEnvelope, WorkflowStepPayload,
    WorkflowStepRef, WorkflowStepReport, WorkflowStepReportPayload, WorkflowStepSpec,
    WorkflowSummary, built_in_review_routine, built_in_review_routine_summaries,
    built_in_review_routines, built_in_workflow, built_in_workflow_summaries, built_in_workflows,
    normalize_reference,
};
use serde_json::json;

fn sample_node(node_key: &str, title: &str) -> NodeRecord {
    NodeRecord {
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

fn sample_pack_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/pack/context-review".to_owned(),
            title: "Pack Context Review".to_owned(),
            summary: Some("Collect context for a reusable routine".to_owned()),
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

fn sample_pack_report_profiles() -> Vec<ReportProfileSpec> {
    vec![
        ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: "profile/routine-detail".to_owned(),
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
        },
        ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: "profile/diff-focus".to_owned(),
                title: "Diff Focus".to_owned(),
                summary: None,
            },
            subjects: vec![ReportProfileSubject::Diff],
            mode: ReportProfileMode::Detail,
            status_filters: None,
            diff_buckets: Some(vec![
                ReviewRunDiffBucket::Added,
                ReviewRunDiffBucket::ContentChanged,
            ]),
            jsonl_line_kinds: Some(vec![
                ReportJsonlLineKind::Diff,
                ReportJsonlLineKind::Added,
                ReportJsonlLineKind::ContentChanged,
            ]),
        },
    ]
}

fn sample_pack_workflow_routine() -> ReviewRoutineSpec {
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
        compare: Some(ReviewRoutineComparePolicy {
            target: ReviewRoutineCompareTarget::LatestCompatibleReview,
            report_profile_id: Some("profile/diff-focus".to_owned()),
        }),
        report_profile_ids: vec!["profile/routine-detail".to_owned()],
    }
}

fn sample_pack_audit_routine() -> ReviewRoutineSpec {
    ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: "routine/pack/duplicate-title-review".to_owned(),
            title: "Duplicate Title Review".to_owned(),
            summary: None,
        },
        source: ReviewRoutineSource::Audit {
            audit: CorpusAuditKind::DuplicateTitles,
            limit: 100,
        },
        inputs: Vec::new(),
        save_review: ReviewRoutineSaveReviewPolicy::default(),
        compare: None,
        report_profile_ids: vec!["profile/routine-detail".to_owned()],
    }
}

fn sample_workbench_pack_manifest() -> WorkbenchPackManifest {
    WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: "pack/research-review".to_owned(),
            title: "Research Review Pack".to_owned(),
            summary: Some("Reusable review routines and output profiles".to_owned()),
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: vec![sample_pack_workflow()],
        review_routines: vec![sample_pack_workflow_routine(), sample_pack_audit_routine()],
        report_profiles: sample_pack_report_profiles(),
        entrypoint_routine_ids: vec![
            "routine/pack/context-review".to_owned(),
            "routine/pack/duplicate-title-review".to_owned(),
        ],
    }
}

fn sample_dangling_finding(
    finding_id: &str,
    missing_explicit_id: &str,
    status: ReviewFindingStatus,
) -> ReviewFinding {
    ReviewFinding {
        finding_id: finding_id.to_owned(),
        status,
        payload: ReviewFindingPayload::Audit {
            entry: Box::new(CorpusAuditEntry::DanglingLink {
                record: Box::new(DanglingLinkAuditRecord {
                    source: sample_anchor("heading:source.org:3", "Source Heading"),
                    missing_explicit_id: missing_explicit_id.to_owned(),
                    line: 12,
                    column: 7,
                    preview: format!("[[id:{missing_explicit_id}][Missing]]"),
                }),
            }),
        },
    }
}

fn sample_dangling_review(review_id: &str, findings: Vec<ReviewFinding>) -> ReviewRun {
    ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: review_id.to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: Some("Review missing id links".to_owned()),
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 200,
        },
        findings,
    }
}

fn sample_dangling_preview_identity(
    source_node_key: &str,
    missing_explicit_id: &str,
) -> AuditRemediationPreviewIdentity {
    AuditRemediationPreviewIdentity::DanglingLink {
        source_node_key: source_node_key.to_owned(),
        missing_explicit_id: missing_explicit_id.to_owned(),
        file_path: "sample.org".to_owned(),
        line: 12,
        column: 7,
        preview: format!("[[id:{missing_explicit_id}][Missing]]"),
    }
}

fn sample_unlink_dangling_action(
    missing_explicit_id: &str,
    replacement_text: &str,
) -> AuditRemediationApplyAction {
    AuditRemediationApplyAction::UnlinkDanglingLink {
        source_node_key: "heading:source.org:3".to_owned(),
        missing_explicit_id: missing_explicit_id.to_owned(),
        file_path: "sample.org".to_owned(),
        line: 12,
        column: 7,
        preview: format!("[[id:{missing_explicit_id}][Missing]]"),
        replacement_text: replacement_text.to_owned(),
    }
}

#[test]
fn normalizes_common_reference_forms() {
    assert_eq!(normalize_reference("@thrun2005"), vec!["@thrun2005"]);
    assert_eq!(normalize_reference("cite:thrun2005"), vec!["@thrun2005"]);
    assert_eq!(
        normalize_reference("[cite:@thrun2005; @smith2024]"),
        vec!["@thrun2005", "@smith2024"]
    );
    assert_eq!(
        normalize_reference("[[https://example.test/path][Example]]"),
        vec!["https://example.test/path"]
    );
}

#[test]
fn capture_params_normalize_and_deduplicate_refs() {
    let params = CaptureNodeParams {
        title: "Note".to_owned(),
        file_path: None,
        head: None,
        refs: vec![
            "cite:smith2024".to_owned(),
            "@smith2024".to_owned(),
            "https://example.test".to_owned(),
        ],
    };

    assert_eq!(
        params.normalized_refs(),
        vec!["@smith2024".to_owned(), "https://example.test".to_owned()]
    );
}

#[test]
fn metadata_params_normalize_fields() {
    let params = UpdateNodeMetadataParams {
        node_key: "heading:note.org:3".to_owned(),
        aliases: Some(vec![
            " Bruce ".to_owned(),
            "Bruce".to_owned(),
            String::new(),
        ]),
        refs: Some(vec!["cite:smith2024".to_owned(), "@smith2024".to_owned()]),
        tags: Some(vec![
            "alpha".to_owned(),
            " alpha ".to_owned(),
            "beta".to_owned(),
        ]),
    };

    assert_eq!(params.normalized_aliases(), Some(vec!["Bruce".to_owned()]));
    assert_eq!(
        params.normalized_refs(),
        Some(vec!["@smith2024".to_owned()])
    );
    assert_eq!(
        params.normalized_tags(),
        Some(vec!["alpha".to_owned(), "beta".to_owned()])
    );
}

#[test]
fn node_record_serialization_includes_metadata_fields() {
    let node = NodeRecord {
        node_key: "heading:note.org:3".to_owned(),
        explicit_id: Some("note-id".to_owned()),
        file_path: "note.org".to_owned(),
        title: "Note".to_owned(),
        outline_path: "Parent".to_owned(),
        aliases: vec!["Alias".to_owned()],
        tags: vec!["tag".to_owned()],
        refs: vec!["@smith2024".to_owned()],
        todo_keyword: None,
        scheduled_for: None,
        deadline_for: None,
        closed_at: None,
        level: 1,
        line: 3,
        kind: NodeKind::Heading,
        file_mtime_ns: 123,
        backlink_count: 2,
        forward_link_count: 4,
    };

    assert_eq!(
        serde_json::to_value(node).expect("node record should serialize"),
        json!({
            "node_key": "heading:note.org:3",
            "explicit_id": "note-id",
            "file_path": "note.org",
            "title": "Note",
            "outline_path": "Parent",
            "aliases": ["Alias"],
            "tags": ["tag"],
            "refs": ["@smith2024"],
            "todo_keyword": null,
            "scheduled_for": null,
            "deadline_for": null,
            "closed_at": null,
            "level": 1,
            "line": 3,
            "kind": "heading",
            "file_mtime_ns": 123,
            "backlink_count": 2,
            "forward_link_count": 4
        })
    );
}

#[test]
fn preview_node_serialization_omits_indexed_metadata_fields() {
    let preview = PreviewNodeRecord {
        node_key: "heading:note.org:3".to_owned(),
        explicit_id: Some("note-id".to_owned()),
        file_path: "note.org".to_owned(),
        title: "Note".to_owned(),
        outline_path: "Parent".to_owned(),
        aliases: vec!["Alias".to_owned()],
        tags: vec!["tag".to_owned()],
        refs: vec!["@smith2024".to_owned()],
        todo_keyword: None,
        scheduled_for: None,
        deadline_for: None,
        closed_at: None,
        level: 1,
        line: 3,
        kind: NodeKind::Heading,
    };

    assert_eq!(
        serde_json::to_value(preview).expect("preview node should serialize"),
        json!({
            "node_key": "heading:note.org:3",
            "explicit_id": "note-id",
            "file_path": "note.org",
            "title": "Note",
            "outline_path": "Parent",
            "aliases": ["Alias"],
            "tags": ["tag"],
            "refs": ["@smith2024"],
            "todo_keyword": null,
            "scheduled_for": null,
            "deadline_for": null,
            "closed_at": null,
            "level": 1,
            "line": 3,
            "kind": "heading"
        })
    );
}

#[test]
fn capture_template_preview_result_serializes_preview_node_field() {
    let result = CaptureTemplatePreviewResult {
        file_path: "note.org".to_owned(),
        content: "* Note\n".to_owned(),
        preview_node: PreviewNodeRecord {
            node_key: "heading:note.org:1".to_owned(),
            explicit_id: None,
            file_path: "note.org".to_owned(),
            title: "Note".to_owned(),
            outline_path: "Note".to_owned(),
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
        },
    };

    assert_eq!(
        serde_json::to_value(result).expect("preview result should serialize"),
        json!({
            "file_path": "note.org",
            "content": "* Note\n",
            "preview_node": {
                "node_key": "heading:note.org:1",
                "explicit_id": null,
                "file_path": "note.org",
                "title": "Note",
                "outline_path": "Note",
                "aliases": [],
                "tags": [],
                "refs": [],
                "todo_keyword": null,
                "scheduled_for": null,
                "deadline_for": null,
                "closed_at": null,
                "level": 1,
                "line": 1,
                "kind": "heading"
            }
        })
    );
}

#[test]
fn structural_write_reports_round_trip_all_operation_kinds() {
    let result_node = NodeRecord {
        explicit_id: Some("result-id".to_owned()),
        file_path: "extracted.org".to_owned(),
        ..sample_node("heading:extracted.org:1", "Result")
    };
    let promoted_file = NodeRecord {
        node_key: "file:promote.org".to_owned(),
        file_path: "promote.org".to_owned(),
        kind: NodeKind::File,
        ..sample_node("file:promote.org", "Promoted")
    };
    let reports = vec![
        StructuralWriteReport {
            operation: StructuralWriteOperationKind::RefileSubtree,
            affected_files: StructuralWriteAffectedFiles {
                changed_files: vec!["target.org".to_owned(), "source.org".to_owned()],
                removed_files: vec!["old-source.org".to_owned()],
            },
            index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
            result: Some(StructuralWriteResult::Anchor {
                anchor: Box::new(AnchorRecord {
                    file_path: "target.org".to_owned(),
                    ..sample_anchor("heading:target.org:7", "Moved")
                }),
            }),
        },
        StructuralWriteReport {
            operation: StructuralWriteOperationKind::RefileRegion,
            affected_files: StructuralWriteAffectedFiles {
                changed_files: vec!["notes/target.org".to_owned()],
                removed_files: Vec::new(),
            },
            index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
            result: None,
        },
        StructuralWriteReport {
            operation: StructuralWriteOperationKind::ExtractSubtree,
            affected_files: StructuralWriteAffectedFiles {
                changed_files: vec!["source.org".to_owned(), "extracted.org".to_owned()],
                removed_files: Vec::new(),
            },
            index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
            result: Some(StructuralWriteResult::Node {
                node: Box::new(result_node.clone()),
            }),
        },
        StructuralWriteReport {
            operation: StructuralWriteOperationKind::PromoteFile,
            affected_files: StructuralWriteAffectedFiles {
                changed_files: vec!["promote.org".to_owned()],
                removed_files: Vec::new(),
            },
            index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
            result: Some(StructuralWriteResult::Node {
                node: Box::new(promoted_file),
            }),
        },
        StructuralWriteReport {
            operation: StructuralWriteOperationKind::DemoteFile,
            affected_files: StructuralWriteAffectedFiles {
                changed_files: vec!["demote.org".to_owned()],
                removed_files: Vec::new(),
            },
            index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
            result: Some(StructuralWriteResult::Anchor {
                anchor: Box::new(AnchorRecord {
                    file_path: "demote.org".to_owned(),
                    ..sample_anchor("heading:demote.org:1", "Demoted")
                }),
            }),
        },
    ];

    for report in reports {
        assert_eq!(report.validation_error(), None);
        let serialized = serde_json::to_value(&report).expect("structural report should serialize");
        assert_eq!(
            serde_json::from_value::<StructuralWriteReport>(serialized)
                .expect("structural report should deserialize"),
            report
        );
    }
}

#[test]
fn structural_write_previews_round_trip_expected_results() {
    let previews = vec![
        StructuralWritePreview {
            operation: StructuralWriteOperationKind::RefileSubtree,
            affected_files: StructuralWriteAffectedFiles {
                changed_files: vec!["target.org".to_owned()],
                removed_files: vec!["source.org".to_owned()],
            },
            result: Some(StructuralWritePreviewResult::ExistingAnchor {
                node_key: "heading:target.org:7".to_owned(),
            }),
        },
        StructuralWritePreview {
            operation: StructuralWriteOperationKind::RefileRegion,
            affected_files: StructuralWriteAffectedFiles {
                changed_files: vec!["target.org".to_owned()],
                removed_files: Vec::new(),
            },
            result: None,
        },
        StructuralWritePreview {
            operation: StructuralWriteOperationKind::ExtractSubtree,
            affected_files: StructuralWriteAffectedFiles {
                changed_files: vec!["source.org".to_owned(), "new/extracted.org".to_owned()],
                removed_files: Vec::new(),
            },
            result: Some(StructuralWritePreviewResult::File {
                file_path: "new/extracted.org".to_owned(),
            }),
        },
    ];

    for preview in previews {
        assert_eq!(preview.validation_error(), None);
        let serialized =
            serde_json::to_value(&preview).expect("structural preview should serialize");
        assert_eq!(
            serde_json::from_value::<StructuralWritePreview>(serialized)
                .expect("structural preview should deserialize"),
            preview
        );
    }
}

#[test]
fn slipbox_link_rewrite_contracts_round_trip_and_validate() {
    let target = NodeRecord {
        explicit_id: Some("target-id".to_owned()),
        file_path: "target.org".to_owned(),
        kind: NodeKind::File,
        ..sample_node("file:target.org", "Target")
    };
    let preview = SlipboxLinkRewritePreview {
        file_path: "source.org".to_owned(),
        rewrites: vec![SlipboxLinkRewritePreviewEntry {
            line: 3,
            column: 7,
            preview: "See [[slipbox:Target][Target Label]].".to_owned(),
            link_text: "[[slipbox:Target][Target Label]]".to_owned(),
            title_or_alias: "Target".to_owned(),
            description: "Target Label".to_owned(),
            target,
            target_explicit_id: Some("target-id".to_owned()),
            replacement: Some("[[id:target-id][Target Label]]".to_owned()),
        }],
    };
    assert_eq!(preview.validation_error(), None);
    let preview_result = SlipboxLinkRewritePreviewResult {
        preview: preview.clone(),
    };
    let serialized =
        serde_json::to_value(&preview_result).expect("preview result should serialize");
    assert_eq!(
        serde_json::from_value::<SlipboxLinkRewritePreviewResult>(serialized)
            .expect("preview result should deserialize"),
        preview_result
    );

    let apply_params = SlipboxLinkRewriteApplyParams {
        expected_preview: preview,
    };
    assert_eq!(apply_params.validation_error(), None);
    let application = SlipboxLinkRewriteApplication {
        file_path: "source.org".to_owned(),
        rewrites: vec![SlipboxLinkRewriteAppliedEntry {
            line: 3,
            column: 7,
            title_or_alias: "Target".to_owned(),
            target_node_key: "file:target.org".to_owned(),
            target_explicit_id: "target-id".to_owned(),
            replacement: "[[id:target-id][Target Label]]".to_owned(),
        }],
        affected_files: StructuralWriteAffectedFiles {
            changed_files: vec!["source.org".to_owned(), "target.org".to_owned()],
            removed_files: Vec::new(),
        },
        index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
    };
    assert_eq!(application.validation_error(), None);
    let apply_result = SlipboxLinkRewriteApplyResult { application };
    let serialized = serde_json::to_value(&apply_result).expect("apply result should serialize");
    assert_eq!(
        serde_json::from_value::<SlipboxLinkRewriteApplyResult>(serialized)
            .expect("apply result should deserialize"),
        apply_result
    );

    let empty_apply = SlipboxLinkRewriteApplyParams {
        expected_preview: SlipboxLinkRewritePreview {
            file_path: "source.org".to_owned(),
            rewrites: Vec::new(),
        },
    };
    assert!(
        empty_apply
            .validation_error()
            .expect("empty apply should be invalid")
            .contains("at least one previewed rewrite")
    );
}

#[test]
fn structural_write_report_serializes_stable_machine_shape() {
    let report = StructuralWriteReport {
        operation: StructuralWriteOperationKind::ExtractSubtree,
        affected_files: StructuralWriteAffectedFiles {
            changed_files: vec!["source.org".to_owned(), "extracted.org".to_owned()],
            removed_files: Vec::new(),
        },
        index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
        result: Some(StructuralWriteResult::Node {
            node: Box::new(NodeRecord {
                explicit_id: Some("extracted-id".to_owned()),
                file_path: "extracted.org".to_owned(),
                ..sample_node("heading:extracted.org:1", "Extracted")
            }),
        }),
    };

    let value = serde_json::to_value(report).expect("report should serialize");
    assert_eq!(
        value,
        json!({
            "operation": "extract-subtree",
            "changed_files": ["source.org", "extracted.org"],
            "removed_files": [],
            "index_refresh": "refreshed",
            "result": {
                "kind": "node",
            "node": {
                    "node_key": "heading:extracted.org:1",
                    "explicit_id": "extracted-id",
                    "file_path": "extracted.org",
                    "title": "Extracted",
                    "outline_path": "Extracted",
                    "aliases": [],
                    "tags": [],
                    "refs": [],
                    "todo_keyword": null,
                    "scheduled_for": null,
                    "deadline_for": null,
                    "closed_at": null,
                    "level": 1,
                    "line": 1,
                    "kind": "heading",
                    "file_mtime_ns": 0,
                    "backlink_count": 0,
                    "forward_link_count": 0
                }
            }
        })
    );
}

#[test]
fn structural_write_validation_rejects_contradictory_reports_and_previews() {
    let base_report = StructuralWriteReport {
        operation: StructuralWriteOperationKind::RefileSubtree,
        affected_files: StructuralWriteAffectedFiles {
            changed_files: vec!["source.org".to_owned()],
            removed_files: Vec::new(),
        },
        index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
        result: Some(StructuralWriteResult::Anchor {
            anchor: Box::new(AnchorRecord {
                file_path: "source.org".to_owned(),
                ..sample_anchor("heading:source.org:1", "Source")
            }),
        }),
    };

    let mut missing_changed = base_report.clone();
    missing_changed.affected_files.changed_files.clear();
    assert_eq!(
        missing_changed.validation_error(),
        Some("structural writes must include at least one changed file".to_owned())
    );

    let mut duplicate_changed = base_report.clone();
    duplicate_changed.affected_files.changed_files =
        vec!["source.org".to_owned(), "source.org".to_owned()];
    assert_eq!(
        duplicate_changed.validation_error(),
        Some("changed_files entry 1 is duplicate: source.org".to_owned())
    );

    let mut overlapping_files = base_report.clone();
    overlapping_files.affected_files.removed_files = vec!["source.org".to_owned()];
    assert_eq!(
        overlapping_files.validation_error(),
        Some("structural write file source.org cannot be both changed and removed".to_owned())
    );

    let mut pending_refresh = base_report.clone();
    pending_refresh.index_refresh = StructuralWriteIndexRefreshStatus::Pending;
    assert_eq!(
        pending_refresh.validation_error(),
        Some("structural write reports must be returned after index refresh".to_owned())
    );

    let mut missing_result = base_report.clone();
    missing_result.result = None;
    assert_eq!(
        missing_result.validation_error(),
        Some(
            "refile-subtree structural write reports must include a resulting node or anchor"
                .to_owned()
        )
    );

    let unexpected_result = StructuralWriteReport {
        operation: StructuralWriteOperationKind::RefileRegion,
        result: base_report.result.clone(),
        ..base_report.clone()
    };
    assert_eq!(
        unexpected_result.validation_error(),
        Some(
            "refile-region structural write reports must not include a resulting node or anchor"
                .to_owned()
        )
    );

    let removed_from_promote = StructuralWriteReport {
        operation: StructuralWriteOperationKind::PromoteFile,
        affected_files: StructuralWriteAffectedFiles {
            changed_files: vec!["promote.org".to_owned()],
            removed_files: vec!["old.org".to_owned()],
        },
        result: base_report.result.clone(),
        ..base_report.clone()
    };
    assert_eq!(
        removed_from_promote.validation_error(),
        Some("promote-file structural writes must not include removed files".to_owned())
    );

    let malformed_node = StructuralWriteReport {
        result: Some(StructuralWriteResult::Node {
            node: Box::new(sample_node("heading:source.org:3", "Source")),
        }),
        ..base_report.clone()
    };
    assert_eq!(
        malformed_node.validation_error(),
        Some(
            "structural write result nodes must be file notes or headings with explicit IDs"
                .to_owned()
        )
    );

    let result_outside_changed_files = StructuralWriteReport {
        result: Some(StructuralWriteResult::Anchor {
            anchor: Box::new(AnchorRecord {
                file_path: "other.org".to_owned(),
                ..sample_anchor("heading:other.org:1", "Other")
            }),
        }),
        ..base_report.clone()
    };
    assert_eq!(
        result_outside_changed_files.validation_error(),
        Some("structural write result file other.org must be in changed_files".to_owned())
    );

    let invalid_preview = StructuralWritePreview {
        operation: StructuralWriteOperationKind::ExtractSubtree,
        affected_files: StructuralWriteAffectedFiles {
            changed_files: vec!["source.org".to_owned()],
            removed_files: Vec::new(),
        },
        result: Some(StructuralWritePreviewResult::File {
            file_path: "../escape.org".to_owned(),
        }),
    };
    assert_eq!(
        invalid_preview.validation_error(),
        Some("file_path must be a normalized relative path".to_owned())
    );
}

#[test]
fn exploration_explanation_serializes_with_tagged_kinds() {
    assert_eq!(
        serde_json::to_value(ExplorationExplanation::Backlink)
            .expect("backlink explanation should serialize"),
        json!({ "kind": "backlink" })
    );

    assert_eq!(
        serde_json::to_value(ExplorationExplanation::SharedReference {
            reference: "cite:smith2024".to_owned(),
        })
        .expect("shared reference explanation should serialize"),
        json!({
            "kind": "shared-reference",
            "reference": "cite:smith2024"
        })
    );

    assert_eq!(
        serde_json::to_value(ExplorationExplanation::UnlinkedReference {
            matched_text: "Project Atlas".to_owned(),
        })
        .expect("unlinked reference explanation should serialize"),
        json!({
            "kind": "unlinked-reference",
            "matched_text": "Project Atlas"
        })
    );

    assert_eq!(
        serde_json::to_value(ExplorationExplanation::TimeNeighbor {
            relations: vec![
                PlanningRelationRecord {
                    source_field: PlanningField::Scheduled,
                    candidate_field: PlanningField::Scheduled,
                    date: "2026-05-01".to_owned(),
                },
                PlanningRelationRecord {
                    source_field: PlanningField::Deadline,
                    candidate_field: PlanningField::Scheduled,
                    date: "2026-05-03".to_owned(),
                },
            ],
        })
        .expect("time-neighbor explanation should serialize"),
        json!({
            "kind": "time-neighbor",
            "relations": [
                {
                    "source_field": "scheduled",
                    "candidate_field": "scheduled",
                    "date": "2026-05-01"
                },
                {
                    "source_field": "deadline",
                    "candidate_field": "scheduled",
                    "date": "2026-05-03"
                }
            ]
        })
    );

    assert_eq!(
        serde_json::to_value(ExplorationExplanation::TaskNeighbor {
            shared_todo_keyword: Some("TODO".to_owned()),
            planning_relations: vec![PlanningRelationRecord {
                source_field: PlanningField::Scheduled,
                candidate_field: PlanningField::Deadline,
                date: "2026-05-01".to_owned(),
            }],
        })
        .expect("task-neighbor explanation should serialize"),
        json!({
            "kind": "task-neighbor",
            "shared_todo_keyword": "TODO",
            "planning_relations": [
                {
                    "source_field": "scheduled",
                    "candidate_field": "deadline",
                    "date": "2026-05-01"
                }
            ]
        })
    );

    assert_eq!(
        serde_json::to_value(ExplorationExplanation::BridgeCandidate {
            references: vec!["@shared2024".to_owned(), "@shared2025".to_owned()],
            via_notes: vec![
                BridgeEvidenceRecord {
                    node_key: "heading:neighbor.org:3".to_owned(),
                    explicit_id: Some("neighbor-id".to_owned()),
                    title: "Neighbor".to_owned(),
                },
                BridgeEvidenceRecord {
                    node_key: "heading:support.org:7".to_owned(),
                    explicit_id: Some("support-id".to_owned()),
                    title: "Support".to_owned(),
                },
            ],
        })
        .expect("bridge explanation should serialize"),
        json!({
            "kind": "bridge-candidate",
            "references": ["@shared2024", "@shared2025"],
            "via_notes": [
                {
                    "node_key": "heading:neighbor.org:3",
                    "explicit_id": "neighbor-id",
                    "title": "Neighbor"
                },
                {
                    "node_key": "heading:support.org:7",
                    "explicit_id": "support-id",
                    "title": "Support"
                }
            ]
        })
    );

    assert_eq!(
        serde_json::to_value(ExplorationExplanation::DormantSharedReference {
            references: vec!["@shared2024".to_owned(), "@shared2025".to_owned()],
            modified_at_ns: 42,
        })
        .expect("dormant explanation should serialize"),
        json!({
            "kind": "dormant-shared-reference",
            "references": ["@shared2024", "@shared2025"],
            "modified_at_ns": 42
        })
    );

    assert_eq!(
        serde_json::to_value(ExplorationExplanation::UnresolvedSharedReference {
            references: vec!["@shared2024".to_owned(), "@shared2025".to_owned()],
            todo_keyword: "TODO".to_owned(),
        })
        .expect("unresolved explanation should serialize"),
        json!({
            "kind": "unresolved-shared-reference",
            "references": ["@shared2024", "@shared2025"],
            "todo_keyword": "TODO"
        })
    );

    assert_eq!(
        serde_json::to_value(ExplorationExplanation::WeaklyIntegratedSharedReference {
            references: vec!["@shared2024".to_owned(), "@shared2025".to_owned()],
            structural_link_count: 1,
        })
        .expect("weak integration explanation should serialize"),
        json!({
            "kind": "weakly-integrated-shared-reference",
            "references": ["@shared2024", "@shared2025"],
            "structural_link_count": 1
        })
    );
}

#[test]
fn explore_params_round_trip_and_validate() {
    let params: ExploreParams = serde_json::from_value(json!({
        "node_key": "heading:alpha.org:3",
        "lens": "structure",
        "limit": 0,
        "unique": true
    }))
    .expect("explore params should deserialize");

    assert_eq!(params.node_key, "heading:alpha.org:3");
    assert_eq!(params.lens, ExplorationLens::Structure);
    assert_eq!(params.normalized_limit(), 1);
    assert_eq!(params.validation_error(), None);

    assert_eq!(
        serde_json::to_value(&params).expect("explore params should serialize"),
        json!({
            "node_key": "heading:alpha.org:3",
            "lens": "structure",
            "limit": 0,
            "unique": true
        })
    );
}

#[test]
fn explore_params_reject_unique_outside_structure() {
    let params = ExploreParams {
        node_key: "heading:alpha.org:3".to_owned(),
        lens: ExplorationLens::Refs,
        limit: 25,
        unique: true,
    };

    assert_eq!(
        params.validation_error().as_deref(),
        Some("explore unique is only supported for the structure lens")
    );
}

#[test]
fn explore_result_serializes_declared_sections() {
    let result = ExploreResult {
        lens: ExplorationLens::Structure,
        sections: vec![ExplorationSection {
            kind: ExplorationSectionKind::Backlinks,
            entries: vec![ExplorationEntry::Backlink {
                record: Box::new(BacklinkRecord {
                    source_note: NodeRecord {
                        node_key: "heading:source.org:5".to_owned(),
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
                        level: 1,
                        line: 5,
                        kind: NodeKind::Heading,
                        file_mtime_ns: 0,
                        backlink_count: 0,
                        forward_link_count: 0,
                    },
                    source_anchor: None,
                    row: 5,
                    col: 2,
                    preview: "[[id:target]]".to_owned(),
                    explanation: ExplorationExplanation::Backlink,
                }),
            }],
        }],
    };

    assert_eq!(
        serde_json::to_value(result).expect("explore result should serialize"),
        json!({
            "lens": "structure",
            "sections": [{
                "kind": "backlinks",
                "entries": [{
                    "kind": "backlink",
                    "source_note": {
                        "node_key": "heading:source.org:5",
                        "explicit_id": "source-id",
                        "file_path": "source.org",
                        "title": "Source",
                        "outline_path": "Source",
                        "aliases": [],
                        "tags": [],
                        "refs": [],
                        "todo_keyword": null,
                        "scheduled_for": null,
                        "deadline_for": null,
                        "closed_at": null,
                        "level": 1,
                        "line": 5,
                        "kind": "heading",
                        "file_mtime_ns": 0,
                        "backlink_count": 0,
                        "forward_link_count": 0
                    },
                    "source_anchor": null,
                    "row": 5,
                    "col": 2,
                    "preview": "[[id:target]]",
                    "explanation": { "kind": "backlink" }
                }]
            }]
        })
    );
}

#[test]
fn compare_notes_params_round_trip() {
    let params: CompareNotesParams = serde_json::from_value(json!({
        "left_node_key": "heading:left.org:3",
        "right_node_key": "heading:right.org:7",
        "limit": 0
    }))
    .expect("compare-notes params should deserialize");

    assert_eq!(params.left_node_key, "heading:left.org:3");
    assert_eq!(params.right_node_key, "heading:right.org:7");
    assert_eq!(params.normalized_limit(), 1);

    assert_eq!(
        serde_json::to_value(&params).expect("compare-notes params should serialize"),
        json!({
            "left_node_key": "heading:left.org:3",
            "right_node_key": "heading:right.org:7",
            "limit": 0
        })
    );
}

#[test]
fn note_comparison_explanation_serializes_connectors() {
    assert_eq!(
        serde_json::to_value(NoteComparisonExplanation::IndirectConnector {
            direction: ComparisonConnectorDirection::Bidirectional,
        })
        .expect("connector explanation should serialize"),
        json!({
            "kind": "indirect-connector",
            "direction": "bidirectional"
        })
    );

    assert_eq!(
        serde_json::to_value(NoteComparisonExplanation::PlanningTension)
            .expect("planning-tension explanation should serialize"),
        json!({
            "kind": "planning-tension"
        })
    );
}

#[test]
fn note_comparison_result_serializes_declared_sections() {
    let result = NoteComparisonResult {
        left_note: NodeRecord {
            node_key: "heading:left.org:3".to_owned(),
            explicit_id: Some("left-id".to_owned()),
            file_path: "left.org".to_owned(),
            title: "Left".to_owned(),
            outline_path: "Left".to_owned(),
            aliases: Vec::new(),
            tags: Vec::new(),
            refs: vec!["@shared2024".to_owned()],
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            level: 1,
            line: 3,
            kind: NodeKind::Heading,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        },
        right_note: NodeRecord {
            node_key: "heading:right.org:7".to_owned(),
            explicit_id: Some("right-id".to_owned()),
            file_path: "right.org".to_owned(),
            title: "Right".to_owned(),
            outline_path: "Right".to_owned(),
            aliases: Vec::new(),
            tags: Vec::new(),
            refs: vec!["@shared2024".to_owned()],
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            level: 1,
            line: 7,
            kind: NodeKind::Heading,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        },
        sections: vec![
            NoteComparisonSection {
                kind: NoteComparisonSectionKind::SharedRefs,
                entries: vec![NoteComparisonEntry::Reference {
                    record: Box::new(ComparisonReferenceRecord {
                        reference: "@shared2024".to_owned(),
                        explanation: NoteComparisonExplanation::SharedReference,
                    }),
                }],
            },
            NoteComparisonSection {
                kind: NoteComparisonSectionKind::PlanningTensions,
                entries: vec![
                    NoteComparisonEntry::PlanningRelation {
                        record: Box::new(ComparisonPlanningRecord {
                            date: "2026-05-01".to_owned(),
                            left_field: PlanningField::Scheduled,
                            right_field: PlanningField::Deadline,
                            explanation: NoteComparisonExplanation::PlanningTension,
                        }),
                    },
                    NoteComparisonEntry::TaskState {
                        record: Box::new(ComparisonTaskStateRecord {
                            left_todo_keyword: "TODO".to_owned(),
                            right_todo_keyword: "NEXT".to_owned(),
                            explanation: NoteComparisonExplanation::ContrastingTaskState,
                        }),
                    },
                ],
            },
        ],
    };

    assert_eq!(
        serde_json::to_value(result).expect("comparison result should serialize"),
        json!({
            "left_note": {
                "node_key": "heading:left.org:3",
                "explicit_id": "left-id",
                "file_path": "left.org",
                "title": "Left",
                "outline_path": "Left",
                "aliases": [],
                "tags": [],
                "refs": ["@shared2024"],
                "todo_keyword": null,
                "scheduled_for": null,
                "deadline_for": null,
                "closed_at": null,
                "level": 1,
                "line": 3,
                "kind": "heading",
                "file_mtime_ns": 0,
                "backlink_count": 0,
                "forward_link_count": 0
            },
            "right_note": {
                "node_key": "heading:right.org:7",
                "explicit_id": "right-id",
                "file_path": "right.org",
                "title": "Right",
                "outline_path": "Right",
                "aliases": [],
                "tags": [],
                "refs": ["@shared2024"],
                "todo_keyword": null,
                "scheduled_for": null,
                "deadline_for": null,
                "closed_at": null,
                "level": 1,
                "line": 7,
                "kind": "heading",
                "file_mtime_ns": 0,
                "backlink_count": 0,
                "forward_link_count": 0
            },
            "sections": [
                {
                    "kind": "shared-refs",
                    "entries": [{
                        "kind": "reference",
                        "reference": "@shared2024",
                        "explanation": { "kind": "shared-reference" }
                    }]
                },
                {
                    "kind": "planning-tensions",
                    "entries": [
                        {
                            "kind": "planning-relation",
                            "date": "2026-05-01",
                            "left_field": "scheduled",
                            "right_field": "deadline",
                            "explanation": { "kind": "planning-tension" }
                        },
                        {
                            "kind": "task-state",
                            "left_todo_keyword": "TODO",
                            "right_todo_keyword": "NEXT",
                            "explanation": { "kind": "contrasting-task-state" }
                        }
                    ]
                }
            ]
        })
    );
}

#[test]
fn note_comparison_group_filters_declared_sections() {
    let result = NoteComparisonResult {
        left_note: NodeRecord {
            node_key: "heading:left.org:3".to_owned(),
            explicit_id: Some("left-id".to_owned()),
            file_path: "left.org".to_owned(),
            title: "Left".to_owned(),
            outline_path: "Left".to_owned(),
            aliases: Vec::new(),
            tags: Vec::new(),
            refs: Vec::new(),
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            level: 1,
            line: 3,
            kind: NodeKind::Heading,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        },
        right_note: NodeRecord {
            node_key: "heading:right.org:7".to_owned(),
            explicit_id: Some("right-id".to_owned()),
            file_path: "right.org".to_owned(),
            title: "Right".to_owned(),
            outline_path: "Right".to_owned(),
            aliases: Vec::new(),
            tags: Vec::new(),
            refs: Vec::new(),
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            level: 1,
            line: 7,
            kind: NodeKind::Heading,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        },
        sections: vec![
            NoteComparisonSection {
                kind: NoteComparisonSectionKind::SharedRefs,
                entries: Vec::new(),
            },
            NoteComparisonSection {
                kind: NoteComparisonSectionKind::LeftOnlyRefs,
                entries: Vec::new(),
            },
            NoteComparisonSection {
                kind: NoteComparisonSectionKind::ContrastingTaskStates,
                entries: Vec::new(),
            },
        ],
    };

    assert_eq!(
        result
            .filtered_to_group(NoteComparisonGroup::Overlap)
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![NoteComparisonSectionKind::SharedRefs]
    );
    assert_eq!(
        result
            .filtered_to_group(NoteComparisonGroup::Divergence)
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![NoteComparisonSectionKind::LeftOnlyRefs]
    );
    assert_eq!(
        result
            .filtered_to_group(NoteComparisonGroup::Tension)
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![NoteComparisonSectionKind::ContrastingTaskStates]
    );
}

#[test]
fn saved_lens_view_artifact_round_trips_and_reuses_explore_validation() {
    let artifact = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "lens-focus".to_owned(),
            title: "Focus refs".to_owned(),
            summary: Some("Pinned refs view".to_owned()),
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "file:focus.org".to_owned(),
                current_node_key: "heading:focus.org:3".to_owned(),
                lens: ExplorationLens::Refs,
                limit: 0,
                unique: false,
                frozen_context: true,
            }),
        },
    };

    assert_eq!(artifact.kind(), ExplorationArtifactKind::LensView);
    assert_eq!(artifact.validation_error(), None);

    let serialized =
        serde_json::to_value(&artifact).expect("saved lens-view artifact should serialize");
    assert_eq!(
        serialized,
        json!({
            "artifact_id": "lens-focus",
            "title": "Focus refs",
            "summary": "Pinned refs view",
            "kind": "lens-view",
            "root_node_key": "file:focus.org",
            "current_node_key": "heading:focus.org:3",
            "lens": "refs",
            "limit": 0,
            "unique": false,
            "frozen_context": true
        })
    );

    let round_trip: SavedExplorationArtifact =
        serde_json::from_value(serialized).expect("saved lens-view artifact should deserialize");
    assert_eq!(round_trip, artifact);

    let invalid = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "invalid-lens".to_owned(),
            title: "Invalid".to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "heading:focus.org:3".to_owned(),
                current_node_key: "heading:focus.org:3".to_owned(),
                lens: ExplorationLens::Refs,
                limit: 25,
                unique: true,
                frozen_context: false,
            }),
        },
    };

    assert_eq!(
        invalid.validation_error().as_deref(),
        Some("explore unique is only supported for the structure lens")
    );

    let non_frozen_root_mismatch = SavedLensViewArtifact {
        root_node_key: "file:other.org".to_owned(),
        current_node_key: "heading:focus.org:3".to_owned(),
        lens: ExplorationLens::Refs,
        limit: 25,
        unique: false,
        frozen_context: false,
    };

    assert_eq!(
        non_frozen_root_mismatch.validation_error().as_deref(),
        Some("non-frozen lens-view artifacts must use current_node_key as root_node_key")
    );
}

#[test]
fn saved_comparison_artifact_round_trips_with_group_semantics() {
    let artifact = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "compare-focus-neighbor".to_owned(),
            title: "Focus vs Neighbor".to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::Comparison {
            artifact: Box::new(SavedComparisonArtifact {
                root_node_key: "file:focus.org".to_owned(),
                left_node_key: "heading:focus.org:3".to_owned(),
                right_node_key: "heading:neighbor.org:7".to_owned(),
                active_lens: ExplorationLens::Tasks,
                structure_unique: false,
                comparison_group: NoteComparisonGroup::Tension,
                limit: 0,
                frozen_context: true,
            }),
        },
    };

    assert_eq!(artifact.kind(), ExplorationArtifactKind::Comparison);
    assert_eq!(artifact.validation_error(), None);
    let serialized =
        serde_json::to_value(&artifact).expect("saved comparison artifact should serialize");
    assert_eq!(
        serialized,
        json!({
            "artifact_id": "compare-focus-neighbor",
            "title": "Focus vs Neighbor",
            "summary": null,
            "kind": "comparison",
            "root_node_key": "file:focus.org",
            "left_node_key": "heading:focus.org:3",
            "right_node_key": "heading:neighbor.org:7",
            "active_lens": "tasks",
            "structure_unique": false,
            "comparison_group": "tension",
            "limit": 0,
            "frozen_context": true
        })
    );
    let round_trip: SavedExplorationArtifact =
        serde_json::from_value(serialized).expect("saved comparison artifact should deserialize");
    assert_eq!(round_trip, artifact);

    let invalid = SavedComparisonArtifact {
        root_node_key: "heading:focus.org:3".to_owned(),
        left_node_key: "heading:focus.org:3".to_owned(),
        right_node_key: "heading:focus.org:3".to_owned(),
        active_lens: ExplorationLens::Structure,
        structure_unique: false,
        comparison_group: NoteComparisonGroup::All,
        limit: 25,
        frozen_context: false,
    };

    assert_eq!(
        invalid.validation_error().as_deref(),
        Some("left_node_key and right_node_key must differ")
    );

    let non_frozen_root_mismatch = SavedComparisonArtifact {
        root_node_key: "heading:previous.org:1".to_owned(),
        left_node_key: "heading:focus.org:3".to_owned(),
        right_node_key: "heading:neighbor.org:7".to_owned(),
        active_lens: ExplorationLens::Structure,
        structure_unique: false,
        comparison_group: NoteComparisonGroup::All,
        limit: 25,
        frozen_context: false,
    };

    assert_eq!(
        non_frozen_root_mismatch.validation_error().as_deref(),
        Some("non-frozen comparison artifacts must use left_node_key as root_node_key")
    );
}

#[test]
fn saved_trail_artifact_round_trips_and_preserves_detached_step() {
    let artifact = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "trail-focus".to_owned(),
            title: "Focus trail".to_owned(),
            summary: Some("Detached comparison branch".to_owned()),
        },
        payload: ExplorationArtifactPayload::Trail {
            artifact: Box::new(SavedTrailArtifact {
                steps: vec![
                    SavedTrailStep::LensView {
                        artifact: Box::new(SavedLensViewArtifact {
                            root_node_key: "file:focus.org".to_owned(),
                            current_node_key: "heading:focus.org:3".to_owned(),
                            lens: ExplorationLens::Unresolved,
                            limit: 200,
                            unique: false,
                            frozen_context: true,
                        }),
                    },
                    SavedTrailStep::Comparison {
                        artifact: Box::new(SavedComparisonArtifact {
                            root_node_key: "file:focus.org".to_owned(),
                            left_node_key: "heading:focus.org:3".to_owned(),
                            right_node_key: "heading:neighbor.org:7".to_owned(),
                            active_lens: ExplorationLens::Refs,
                            structure_unique: false,
                            comparison_group: NoteComparisonGroup::Overlap,
                            limit: 100,
                            frozen_context: true,
                        }),
                    },
                ],
                cursor: 0,
                detached_step: Some(Box::new(SavedTrailStep::Comparison {
                    artifact: Box::new(SavedComparisonArtifact {
                        root_node_key: "file:focus.org".to_owned(),
                        left_node_key: "heading:focus.org:3".to_owned(),
                        right_node_key: "heading:tension.org:9".to_owned(),
                        active_lens: ExplorationLens::Structure,
                        structure_unique: true,
                        comparison_group: NoteComparisonGroup::Tension,
                        limit: 100,
                        frozen_context: true,
                    }),
                })),
            }),
        },
    };

    assert_eq!(artifact.kind(), ExplorationArtifactKind::Trail);
    assert_eq!(artifact.validation_error(), None);

    let serialized =
        serde_json::to_value(&artifact).expect("saved trail artifact should serialize");
    let round_trip: SavedExplorationArtifact = serde_json::from_value(serialized.clone())
        .expect("saved trail artifact should deserialize");
    assert_eq!(round_trip, artifact);
    assert_eq!(serialized["kind"], json!("trail"));
    assert_eq!(serialized["steps"][0]["kind"], json!("lens-view"));
    assert_eq!(serialized["steps"][1]["kind"], json!("comparison"));
    assert_eq!(serialized["detached_step"]["kind"], json!("comparison"));
}

#[test]
fn executed_exploration_artifacts_round_trip_with_trail_replay() {
    let executed = ExecutedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "executed-trail".to_owned(),
            title: "Executed trail".to_owned(),
            summary: Some("Replay result".to_owned()),
        },
        payload: ExecutedExplorationArtifactPayload::Trail {
            artifact: Box::new(SavedTrailArtifact {
                steps: vec![SavedTrailStep::LensView {
                    artifact: Box::new(SavedLensViewArtifact {
                        root_node_key: "file:focus.org".to_owned(),
                        current_node_key: "file:focus.org".to_owned(),
                        lens: ExplorationLens::Structure,
                        limit: 5,
                        unique: false,
                        frozen_context: false,
                    }),
                }],
                cursor: 0,
                detached_step: None,
            }),
            replay: Box::new(TrailReplayResult {
                steps: vec![TrailReplayStepResult::LensView {
                    artifact: Box::new(SavedLensViewArtifact {
                        root_node_key: "file:focus.org".to_owned(),
                        current_node_key: "file:focus.org".to_owned(),
                        lens: ExplorationLens::Structure,
                        limit: 5,
                        unique: false,
                        frozen_context: false,
                    }),
                    root_note: Box::new(NodeRecord {
                        node_key: "file:focus.org".to_owned(),
                        explicit_id: Some("focus-id".to_owned()),
                        file_path: "focus.org".to_owned(),
                        title: "Focus".to_owned(),
                        outline_path: String::new(),
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
                        file_mtime_ns: 123,
                        backlink_count: 1,
                        forward_link_count: 0,
                    }),
                    current_note: Box::new(NodeRecord {
                        node_key: "file:focus.org".to_owned(),
                        explicit_id: Some("focus-id".to_owned()),
                        file_path: "focus.org".to_owned(),
                        title: "Focus".to_owned(),
                        outline_path: String::new(),
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
                        file_mtime_ns: 123,
                        backlink_count: 1,
                        forward_link_count: 0,
                    }),
                    result: Box::new(ExploreResult {
                        lens: ExplorationLens::Structure,
                        sections: vec![ExplorationSection {
                            kind: ExplorationSectionKind::Backlinks,
                            entries: vec![ExplorationEntry::Backlink {
                                record: Box::new(BacklinkRecord {
                                    source_note: NodeRecord {
                                        node_key: "file:focus.org".to_owned(),
                                        explicit_id: Some("focus-id".to_owned()),
                                        file_path: "focus.org".to_owned(),
                                        title: "Focus".to_owned(),
                                        outline_path: String::new(),
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
                                        file_mtime_ns: 123,
                                        backlink_count: 1,
                                        forward_link_count: 0,
                                    },
                                    source_anchor: None,
                                    row: 3,
                                    col: 9,
                                    preview: "Links to focus".to_owned(),
                                    explanation: ExplorationExplanation::Backlink,
                                }),
                            }],
                        }],
                    }),
                }],
                cursor: 0,
                detached_step: None,
            }),
        },
    };

    assert_eq!(executed.kind(), ExplorationArtifactKind::Trail);
    let serialized =
        serde_json::to_value(&executed).expect("executed exploration artifact should serialize");
    assert_eq!(serialized["kind"], json!("trail"));
    assert_eq!(serialized["replay"]["steps"][0]["kind"], json!("lens-view"));

    let round_trip: ExecutedExplorationArtifact = serde_json::from_value(serialized)
        .expect("executed exploration artifact should deserialize");
    assert_eq!(round_trip, executed);
}

#[test]
fn exploration_artifact_rpc_contracts_round_trip() {
    let artifact = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "lens/focus".to_owned(),
            title: "Lens Focus".to_owned(),
            summary: Some("Saved structure lens".to_owned()),
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "file:focus.org".to_owned(),
                current_node_key: "file:focus.org".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 20,
                unique: false,
                frozen_context: false,
            }),
        },
    };
    let summary = ExplorationArtifactSummary::from(&artifact);
    let save_params = SaveExplorationArtifactParams {
        artifact: artifact.clone(),
        overwrite: false,
    };
    let save_result = SaveExplorationArtifactResult {
        artifact: summary.clone(),
    };
    let list_result = ListExplorationArtifactsResult {
        artifacts: vec![summary.clone()],
    };
    let inspect_result = ExplorationArtifactResult {
        artifact: artifact.clone(),
    };
    let execute_result = ExecuteExplorationArtifactResult {
        artifact: ExecutedExplorationArtifact {
            metadata: artifact.metadata.clone(),
            payload: ExecutedExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "file:focus.org".to_owned(),
                    current_node_key: "file:focus.org".to_owned(),
                    lens: ExplorationLens::Structure,
                    limit: 20,
                    unique: false,
                    frozen_context: false,
                }),
                root_note: Box::new(NodeRecord {
                    node_key: "file:focus.org".to_owned(),
                    explicit_id: None,
                    file_path: "focus.org".to_owned(),
                    title: "Focus".to_owned(),
                    outline_path: String::new(),
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
                }),
                current_note: Box::new(NodeRecord {
                    node_key: "file:focus.org".to_owned(),
                    explicit_id: None,
                    file_path: "focus.org".to_owned(),
                    title: "Focus".to_owned(),
                    outline_path: String::new(),
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
                }),
                result: Box::new(ExploreResult {
                    lens: ExplorationLens::Structure,
                    sections: Vec::new(),
                }),
            },
        },
    };
    let delete_result = DeleteExplorationArtifactResult {
        artifact_id: "lens/focus".to_owned(),
    };
    let id_params = ExplorationArtifactIdParams {
        artifact_id: "lens/focus".to_owned(),
    };

    let save_json = serde_json::to_value(&save_params).expect("save params should serialize");
    assert_eq!(save_json["artifact"]["artifact_id"], json!("lens/focus"));
    assert_eq!(save_json["artifact"]["kind"], json!("lens-view"));
    assert_eq!(save_json["overwrite"], json!(false));

    let save_round_trip: SaveExplorationArtifactParams =
        serde_json::from_value(save_json).expect("save params should deserialize");
    assert_eq!(save_round_trip, save_params);

    let legacy_round_trip: SaveExplorationArtifactParams =
        serde_json::from_value(json!({ "artifact": artifact.clone() }))
            .expect("legacy save params should deserialize");
    assert!(legacy_round_trip.overwrite);
    assert_eq!(legacy_round_trip.artifact, artifact);

    let save_result_round_trip: SaveExplorationArtifactResult = serde_json::from_value(
        serde_json::to_value(&save_result).expect("save result should serialize"),
    )
    .expect("save result should deserialize");
    assert_eq!(save_result_round_trip, save_result);

    let summary_json = serde_json::to_value(&summary).expect("summary should serialize");
    assert_eq!(summary_json["kind"], json!("lens-view"));

    let list_round_trip: ListExplorationArtifactsResult = serde_json::from_value(
        serde_json::to_value(&list_result).expect("list result should serialize"),
    )
    .expect("list result should deserialize");
    assert_eq!(list_round_trip, list_result);

    let inspect_round_trip: ExplorationArtifactResult = serde_json::from_value(
        serde_json::to_value(&inspect_result).expect("inspect result should serialize"),
    )
    .expect("inspect result should deserialize");
    assert_eq!(inspect_round_trip, inspect_result);

    let execute_round_trip: ExecuteExplorationArtifactResult = serde_json::from_value(
        serde_json::to_value(&execute_result).expect("execute result should serialize"),
    )
    .expect("execute result should deserialize");
    assert_eq!(execute_round_trip, execute_result);

    let delete_round_trip: DeleteExplorationArtifactResult = serde_json::from_value(
        serde_json::to_value(&delete_result).expect("delete result should serialize"),
    )
    .expect("delete result should deserialize");
    assert_eq!(delete_round_trip, delete_result);

    let id_round_trip: ExplorationArtifactIdParams = serde_json::from_value(
        serde_json::to_value(&id_params).expect("id params should serialize"),
    )
    .expect("id params should deserialize");
    assert_eq!(id_round_trip, id_params);
}

#[test]
fn workflow_specs_round_trip_and_compose_settled_headless_steps() {
    let workflow = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/research-routine".to_owned(),
            title: "Research Routine".to_owned(),
            summary: Some("Resolve, explore, compare, and save".to_owned()),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Id {
                        id: "focus-id".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "resolve-neighbor".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Title {
                        title: "Neighbor".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "run-saved-context".to_owned(),
                payload: WorkflowStepPayload::ArtifactRun {
                    artifact_id: "artifact/context".to_owned(),
                },
            },
            WorkflowStepSpec {
                step_id: "explore-dormant".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Dormant,
                    limit: 0,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-context".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-neighbor".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "compare-focus-neighbor".to_owned(),
                payload: WorkflowStepPayload::Compare {
                    left: WorkflowStepRef {
                        step_id: "resolve-focus".to_owned(),
                    },
                    right: WorkflowStepRef {
                        step_id: "resolve-neighbor".to_owned(),
                    },
                    group: NoteComparisonGroup::Tension,
                    limit: 10,
                },
            },
            WorkflowStepSpec {
                step_id: "save-comparison".to_owned(),
                payload: WorkflowStepPayload::ArtifactSave {
                    source: WorkflowArtifactSaveSource::CompareStep {
                        step_id: "compare-focus-neighbor".to_owned(),
                    },
                    metadata: ExplorationArtifactMetadata {
                        artifact_id: "artifact/focus-vs-neighbor".to_owned(),
                        title: "Focus vs Neighbor".to_owned(),
                        summary: Some("Pinned comparison".to_owned()),
                    },
                    overwrite: false,
                },
            },
        ],
    };

    assert_eq!(workflow.validation_error(), None);
    assert_eq!(WorkflowSummary::from(&workflow).step_count, 7);

    let serialized = serde_json::to_value(&workflow).expect("workflow spec should serialize");
    assert_eq!(
        serialized["workflow_id"],
        json!("workflow/research-routine")
    );
    assert_eq!(serialized["compatibility"]["version"], json!(1));
    assert_eq!(serialized["steps"][0]["kind"], json!("resolve"));
    assert_eq!(serialized["steps"][3]["kind"], json!("explore"));
    assert_eq!(serialized["steps"][5]["kind"], json!("compare"));
    assert_eq!(serialized["steps"][6]["kind"], json!("artifact-save"));
    assert_eq!(
        serialized["steps"][6]["artifact_id"],
        json!("artifact/focus-vs-neighbor")
    );

    let round_trip: WorkflowSpec =
        serde_json::from_value(serialized).expect("workflow spec should deserialize");
    assert_eq!(round_trip, workflow);
}

#[test]
fn workflow_specs_default_legacy_compatibility_and_reject_future_versions() {
    let legacy_spec = json!({
        "workflow_id": "workflow/legacy",
        "title": "Legacy",
        "summary": null,
        "inputs": [],
        "steps": [{
            "step_id": "resolve-focus",
            "kind": "resolve",
            "target": {
                "kind": "id",
                "id": "focus-id"
            }
        }]
    });
    let legacy: WorkflowSpec =
        serde_json::from_value(legacy_spec).expect("legacy workflow spec should deserialize");
    assert_eq!(legacy.compatibility, WorkflowSpecCompatibility::default());
    assert_eq!(legacy.validation_error(), None);

    let future_spec = json!({
        "workflow_id": "workflow/future",
        "title": "Future",
        "summary": null,
        "compatibility": {
            "version": 2
        },
        "inputs": [],
        "steps": [{
            "step_id": "resolve-focus",
            "kind": "future-step",
            "future_field": true
        }]
    });
    let envelope: WorkflowSpecCompatibilityEnvelope = serde_json::from_value(future_spec.clone())
        .expect("future compatibility envelope should deserialize");
    assert_eq!(envelope.workflow_id.as_deref(), Some("workflow/future"));
    assert_eq!(
        envelope.compatibility.validation_error().as_deref(),
        Some("unsupported workflow spec compatibility version 2; supported version is 1")
    );
    serde_json::from_value::<WorkflowSpec>(future_spec)
        .expect_err("future workflow syntax should not deserialize as current spec");
}

#[test]
fn workflow_execution_results_round_trip_with_typed_step_reports() {
    let workflow = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/round-trip".to_owned(),
            title: "Round Trip".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Id {
                        id: "focus-id".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-focus".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::NodeKey {
                        node_key: "heading:focus.org:3".to_owned(),
                    },
                    lens: ExplorationLens::Structure,
                    limit: 10,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "compare-focus-neighbor".to_owned(),
                payload: WorkflowStepPayload::Compare {
                    left: WorkflowStepRef {
                        step_id: "resolve-focus".to_owned(),
                    },
                    right: WorkflowStepRef {
                        step_id: "resolve-focus-other".to_owned(),
                    },
                    group: NoteComparisonGroup::All,
                    limit: 10,
                },
            },
        ],
    };

    let executed_artifact = ExecutedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "artifact/focus".to_owned(),
            title: "Saved Focus".to_owned(),
            summary: None,
        },
        payload: ExecutedExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "file:focus.org".to_owned(),
                current_node_key: "heading:focus.org:3".to_owned(),
                lens: ExplorationLens::Refs,
                limit: 25,
                unique: false,
                frozen_context: true,
            }),
            root_note: Box::new(sample_node("file:focus.org", "Focus")),
            current_note: Box::new(sample_node("heading:focus.org:3", "Focus Heading")),
            result: Box::new(ExploreResult {
                lens: ExplorationLens::Refs,
                sections: Vec::new(),
            }),
        },
    };

    let result = WorkflowExecutionResult {
        workflow: WorkflowSummary::from(&workflow),
        steps: vec![
            WorkflowStepReport {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepReportPayload::Resolve {
                    node: Box::new(sample_node("heading:focus.org:3", "Focus")),
                },
            },
            WorkflowStepReport {
                step_id: "explore-focus".to_owned(),
                payload: WorkflowStepReportPayload::Explore {
                    focus_node_key: "heading:focus.org:3".to_owned(),
                    result: Box::new(ExploreResult {
                        lens: ExplorationLens::Structure,
                        sections: Vec::new(),
                    }),
                },
            },
            WorkflowStepReport {
                step_id: "compare-focus-neighbor".to_owned(),
                payload: WorkflowStepReportPayload::Compare {
                    left_node: Box::new(sample_node("heading:focus.org:3", "Focus")),
                    right_node: Box::new(sample_node("heading:neighbor.org:7", "Neighbor")),
                    result: Box::new(NoteComparisonResult {
                        left_note: sample_node("heading:focus.org:3", "Focus"),
                        right_note: sample_node("heading:neighbor.org:7", "Neighbor"),
                        sections: Vec::new(),
                    }),
                },
            },
            WorkflowStepReport {
                step_id: "run-artifact".to_owned(),
                payload: WorkflowStepReportPayload::ArtifactRun {
                    artifact: Box::new(executed_artifact),
                },
            },
            WorkflowStepReport {
                step_id: "save-artifact".to_owned(),
                payload: WorkflowStepReportPayload::ArtifactSave {
                    artifact: Box::new(ExplorationArtifactSummary {
                        metadata: ExplorationArtifactMetadata {
                            artifact_id: "artifact/focus".to_owned(),
                            title: "Saved Focus".to_owned(),
                            summary: None,
                        },
                        kind: ExplorationArtifactKind::LensView,
                    }),
                },
            },
        ],
    };

    let serialized =
        serde_json::to_value(&result).expect("workflow execution result should serialize");
    assert_eq!(
        serialized["workflow"]["workflow_id"],
        json!("workflow/round-trip")
    );
    assert_eq!(serialized["steps"][0]["kind"], json!("resolve"));
    assert_eq!(serialized["steps"][1]["kind"], json!("explore"));
    assert_eq!(serialized["steps"][2]["kind"], json!("compare"));
    assert_eq!(serialized["steps"][3]["kind"], json!("artifact-run"));
    assert_eq!(serialized["steps"][4]["kind"], json!("artifact-save"));

    let round_trip: WorkflowExecutionResult =
        serde_json::from_value(serialized).expect("workflow execution result should deserialize");
    assert_eq!(round_trip, result);

    let lines = result.report_lines();
    assert_eq!(lines.len(), 6);
    assert_eq!(
        serde_json::to_value(&lines[0]).expect("workflow report line should serialize"),
        json!({
            "kind": "workflow",
            "workflow": result.workflow
        })
    );
    assert_eq!(
        serde_json::to_value(&lines[1]).expect("workflow report line should serialize")["kind"],
        json!("step")
    );

    let round_trip_lines: Vec<WorkflowReportLine> = serde_json::from_value(
        serde_json::to_value(&lines).expect("workflow report lines should serialize"),
    )
    .expect("workflow report lines should deserialize");
    assert_eq!(round_trip_lines, lines);
}

#[test]
fn corpus_audit_results_round_trip_with_typed_entries() {
    let result = CorpusAuditResult {
        audit: CorpusAuditKind::DanglingLinks,
        entries: vec![
            CorpusAuditEntry::DanglingLink {
                record: Box::new(DanglingLinkAuditRecord {
                    source: sample_anchor("heading:source.org:3", "Source Heading"),
                    missing_explicit_id: "missing-id".to_owned(),
                    line: 12,
                    column: 7,
                    preview: "[[id:missing-id][Missing]]".to_owned(),
                }),
            },
            CorpusAuditEntry::DuplicateTitle {
                record: Box::new(DuplicateTitleAuditRecord {
                    title: "Shared Title".to_owned(),
                    notes: vec![
                        sample_node("file:left.org", "Shared Title"),
                        sample_node("file:right.org", "Shared Title"),
                    ],
                }),
            },
            CorpusAuditEntry::OrphanNote {
                record: Box::new(NoteConnectivityAuditRecord {
                    note: sample_node("file:orphan.org", "Orphan"),
                    reference_count: 0,
                    backlink_count: 0,
                    forward_link_count: 0,
                }),
            },
            CorpusAuditEntry::WeaklyIntegratedNote {
                record: Box::new(NoteConnectivityAuditRecord {
                    note: sample_node("file:weak.org", "Weak"),
                    reference_count: 2,
                    backlink_count: 0,
                    forward_link_count: 1,
                }),
            },
        ],
    };

    let serialized = serde_json::to_value(&result).expect("audit result should serialize");
    assert_eq!(serialized["audit"], json!("dangling-links"));
    assert_eq!(serialized["entries"][0]["kind"], json!("dangling-link"));
    assert_eq!(serialized["entries"][1]["kind"], json!("duplicate-title"));
    assert_eq!(serialized["entries"][2]["kind"], json!("orphan-note"));
    assert_eq!(
        serialized["entries"][3]["kind"],
        json!("weakly-integrated-note")
    );

    let round_trip: CorpusAuditResult =
        serde_json::from_value(serialized).expect("audit result should deserialize");
    assert_eq!(round_trip, result);

    let lines = result.report_lines();
    assert_eq!(lines.len(), 5);
    assert_eq!(
        serde_json::to_value(&lines[0]).expect("audit report line should serialize"),
        json!({
            "kind": "audit",
            "audit": "dangling-links"
        })
    );
    assert_eq!(
        serde_json::to_value(&lines[1]).expect("audit report line should serialize")["kind"],
        json!("entry")
    );

    let round_trip_lines: Vec<CorpusAuditReportLine> = serde_json::from_value(
        serde_json::to_value(&lines).expect("audit report lines should serialize"),
    )
    .expect("audit report lines should deserialize");
    assert_eq!(round_trip_lines, lines);
}

#[test]
fn report_profile_specs_round_trip_with_bounded_selections() {
    let profile = ReportProfileSpec {
        metadata: ReportProfileMetadata {
            profile_id: "profile/review-diff-focus".to_owned(),
            title: "Review Diff Focus".to_owned(),
            summary: Some("Show open review details and selected diff buckets.".to_owned()),
        },
        subjects: vec![ReportProfileSubject::Review, ReportProfileSubject::Diff],
        mode: ReportProfileMode::Detail,
        status_filters: Some(vec![
            ReviewFindingStatus::Open,
            ReviewFindingStatus::Reviewed,
        ]),
        diff_buckets: Some(vec![
            ReviewRunDiffBucket::Added,
            ReviewRunDiffBucket::StatusChanged,
        ]),
        jsonl_line_kinds: Some(vec![
            ReportJsonlLineKind::Review,
            ReportJsonlLineKind::Finding,
            ReportJsonlLineKind::Diff,
            ReportJsonlLineKind::Added,
            ReportJsonlLineKind::StatusChanged,
        ]),
    };

    assert_eq!(profile.validation_error(), None);
    let serialized = serde_json::to_value(&profile).expect("profile should serialize");
    assert_eq!(serialized["profile_id"], json!("profile/review-diff-focus"));
    assert_eq!(serialized["subjects"], json!(["review", "diff"]));
    assert_eq!(serialized["mode"], json!("detail"));
    assert_eq!(serialized["status_filters"], json!(["open", "reviewed"]));
    assert_eq!(
        serialized["diff_buckets"],
        json!(["added", "status-changed"])
    );
    assert_eq!(
        serialized["jsonl_line_kinds"],
        json!(["review", "finding", "diff", "added", "status-changed"])
    );

    let round_trip: ReportProfileSpec =
        serde_json::from_value(serialized).expect("profile should deserialize");
    assert_eq!(round_trip, profile);

    let catalog = ReportProfileCatalog {
        profiles: vec![
            profile,
            ReportProfileSpec {
                metadata: ReportProfileMetadata {
                    profile_id: "profile/workflow-summary".to_owned(),
                    title: "Workflow Summary".to_owned(),
                    summary: None,
                },
                subjects: vec![ReportProfileSubject::Workflow],
                mode: ReportProfileMode::Summary,
                status_filters: None,
                diff_buckets: None,
                jsonl_line_kinds: Some(vec![ReportJsonlLineKind::Workflow]),
            },
        ],
    };
    assert_eq!(catalog.validation_error(), None);
    let catalog_round_trip: ReportProfileCatalog =
        serde_json::from_value(serde_json::to_value(&catalog).expect("catalog should serialize"))
            .expect("catalog should deserialize");
    assert_eq!(catalog_round_trip, catalog);
}

#[test]
fn report_profile_specs_reject_malformed_and_contradictory_selections() {
    let valid = ReportProfileSpec {
        metadata: ReportProfileMetadata {
            profile_id: "profile/review-open".to_owned(),
            title: "Review Open".to_owned(),
            summary: None,
        },
        subjects: vec![ReportProfileSubject::Review],
        mode: ReportProfileMode::Detail,
        status_filters: Some(vec![ReviewFindingStatus::Open]),
        diff_buckets: None,
        jsonl_line_kinds: Some(vec![ReportJsonlLineKind::Review]),
    };

    let mut padded_id = valid.clone();
    padded_id.metadata.profile_id = " profile/review-open".to_owned();
    assert_eq!(
        padded_id.validation_error().as_deref(),
        Some("profile_id must not have leading or trailing whitespace")
    );

    let mut empty_subjects = valid.clone();
    empty_subjects.subjects.clear();
    assert_eq!(
        empty_subjects.validation_error().as_deref(),
        Some("report profiles must select at least one subject")
    );

    let mut duplicate_subjects = valid.clone();
    duplicate_subjects.subjects = vec![ReportProfileSubject::Review, ReportProfileSubject::Review];
    assert_eq!(
        duplicate_subjects.validation_error().as_deref(),
        Some("report profile subject 1 is duplicate: review")
    );

    let mut empty_status_filters = valid.clone();
    empty_status_filters.status_filters = Some(Vec::new());
    assert_eq!(
        empty_status_filters.validation_error().as_deref(),
        Some("report profile status_filters must not be empty when present")
    );

    let mut duplicate_status_filters = valid.clone();
    duplicate_status_filters.status_filters =
        Some(vec![ReviewFindingStatus::Open, ReviewFindingStatus::Open]);
    assert_eq!(
        duplicate_status_filters.validation_error().as_deref(),
        Some("report profile status_filters entry 1 is duplicate: open")
    );

    let mut status_without_review_surface = valid.clone();
    status_without_review_surface.subjects = vec![ReportProfileSubject::Workflow];
    status_without_review_surface.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Workflow]);
    assert_eq!(
        status_without_review_surface.validation_error().as_deref(),
        Some("report profile status_filters require a review, routine, or diff subject")
    );

    let mut empty_diff_buckets = valid.clone();
    empty_diff_buckets.subjects = vec![ReportProfileSubject::Diff];
    empty_diff_buckets.status_filters = None;
    empty_diff_buckets.diff_buckets = Some(Vec::new());
    empty_diff_buckets.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Diff]);
    assert_eq!(
        empty_diff_buckets.validation_error().as_deref(),
        Some("report profile diff_buckets must not be empty when present")
    );

    let mut diff_without_diff_subject = valid.clone();
    diff_without_diff_subject.diff_buckets = Some(vec![ReviewRunDiffBucket::Added]);
    assert_eq!(
        diff_without_diff_subject.validation_error().as_deref(),
        Some("report profile diff_buckets require a diff subject")
    );

    let mut duplicate_diff_buckets = valid.clone();
    duplicate_diff_buckets.subjects = vec![ReportProfileSubject::Diff];
    duplicate_diff_buckets.status_filters = None;
    duplicate_diff_buckets.diff_buckets =
        Some(vec![ReviewRunDiffBucket::Added, ReviewRunDiffBucket::Added]);
    duplicate_diff_buckets.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Diff]);
    assert_eq!(
        duplicate_diff_buckets.validation_error().as_deref(),
        Some("report profile diff_buckets entry 1 is duplicate: added")
    );

    let mut empty_line_kinds = valid.clone();
    empty_line_kinds.jsonl_line_kinds = Some(Vec::new());
    assert_eq!(
        empty_line_kinds.validation_error().as_deref(),
        Some("report profile jsonl_line_kinds must not be empty when present")
    );

    let unsupported_line_kind: ReportProfileSpec = serde_json::from_value(json!({
        "profile_id": "profile/unsupported-line",
        "title": "Unsupported Line",
        "subjects": ["workflow"],
        "mode": "detail",
        "status_filters": null,
        "diff_buckets": null,
        "jsonl_line_kinds": ["workflow", "template-snippet"]
    }))
    .expect("unsupported line kind should deserialize for validation");
    assert_eq!(
        unsupported_line_kind.validation_error().as_deref(),
        Some("report profile jsonl_line_kinds entry 1 is unsupported: template-snippet")
    );

    let mut incompatible_line_kind = valid.clone();
    incompatible_line_kind.subjects = vec![ReportProfileSubject::Audit];
    incompatible_line_kind.status_filters = None;
    incompatible_line_kind.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Workflow]);
    assert_eq!(
        incompatible_line_kind.validation_error().as_deref(),
        Some(
            "report profile jsonl_line_kinds entry 0 is not supported by selected subjects: workflow"
        )
    );

    let mut summary_with_detail_line = valid.clone();
    summary_with_detail_line.mode = ReportProfileMode::Summary;
    summary_with_detail_line.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Finding]);
    assert_eq!(
        summary_with_detail_line.validation_error().as_deref(),
        Some("report profile summary mode cannot select detail JSONL line kind: finding")
    );

    let catalog = ReportProfileCatalog {
        profiles: vec![
            valid.clone(),
            ReportProfileSpec {
                metadata: ReportProfileMetadata {
                    profile_id: valid.metadata.profile_id.clone(),
                    title: "Duplicate".to_owned(),
                    summary: None,
                },
                subjects: vec![ReportProfileSubject::Workflow],
                mode: ReportProfileMode::Summary,
                status_filters: None,
                diff_buckets: None,
                jsonl_line_kinds: Some(vec![ReportJsonlLineKind::Workflow]),
            },
        ],
    };
    assert_eq!(
        catalog.validation_error().as_deref(),
        Some("report profile 1 reuses duplicate profile_id profile/review-open")
    );
}

#[test]
fn review_routine_specs_round_trip_over_audit_and_workflow_sources() {
    let audit_routine = ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: "routine/audit/duplicate-title-review".to_owned(),
            title: "Duplicate Title Review".to_owned(),
            summary: Some("Review title collisions and compare to the last run".to_owned()),
        },
        source: ReviewRoutineSource::Audit {
            audit: CorpusAuditKind::DuplicateTitles,
            limit: 100,
        },
        inputs: Vec::new(),
        save_review: ReviewRoutineSaveReviewPolicy {
            enabled: true,
            review_id: Some("review/routine/duplicate-title-review".to_owned()),
            title: Some("Duplicate Title Review".to_owned()),
            summary: Some("Generated by a declarative routine".to_owned()),
            overwrite: false,
        },
        compare: Some(ReviewRoutineComparePolicy {
            target: ReviewRoutineCompareTarget::LatestCompatibleReview,
            report_profile_id: Some("profile/diff-focus".to_owned()),
        }),
        report_profile_ids: vec![
            "profile/audit-detail".to_owned(),
            "profile/diff-focus".to_owned(),
        ],
    };
    assert_eq!(audit_routine.validation_error(), None);
    let serialized = serde_json::to_value(&audit_routine).expect("audit routine should serialize");
    assert_eq!(
        serialized["routine_id"],
        json!("routine/audit/duplicate-title-review")
    );
    assert_eq!(serialized["source"]["kind"], json!("audit"));
    assert_eq!(serialized["source"]["audit"], json!("duplicate-titles"));
    assert_eq!(
        serialized["compare"]["target"],
        json!("latest-compatible-review")
    );
    let round_trip: ReviewRoutineSpec =
        serde_json::from_value(serialized).expect("audit routine should deserialize");
    assert_eq!(round_trip, audit_routine);

    let workflow_routine = ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: "routine/workflow/periodic-review".to_owned(),
            title: "Periodic Workflow Review".to_owned(),
            summary: None,
        },
        source: ReviewRoutineSource::Workflow {
            workflow_id: BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID.to_owned(),
        },
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Review focus".to_owned(),
            summary: Some("Note or anchor focus for the review".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        save_review: ReviewRoutineSaveReviewPolicy::default(),
        compare: None,
        report_profile_ids: vec!["profile/workflow-summary".to_owned()],
    };
    assert_eq!(workflow_routine.validation_error(), None);

    let catalog = ReviewRoutineCatalog {
        routines: vec![audit_routine, workflow_routine],
    };
    assert_eq!(catalog.validation_error(), None);
    let catalog_round_trip: ReviewRoutineCatalog = serde_json::from_value(
        serde_json::to_value(&catalog).expect("routine catalog should serialize"),
    )
    .expect("routine catalog should deserialize");
    assert_eq!(catalog_round_trip, catalog);
}

#[test]
fn review_routine_specs_reject_invalid_references_and_policy_conflicts() {
    let valid = ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: "routine/workflow/context-review".to_owned(),
            title: "Context Review".to_owned(),
            summary: None,
        },
        source: ReviewRoutineSource::Workflow {
            workflow_id: BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned(),
        },
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus".to_owned(),
            summary: None,
            kind: WorkflowInputKind::FocusTarget,
        }],
        save_review: ReviewRoutineSaveReviewPolicy::default(),
        compare: Some(ReviewRoutineComparePolicy {
            target: ReviewRoutineCompareTarget::LatestCompatibleReview,
            report_profile_id: Some("profile/diff-focus".to_owned()),
        }),
        report_profile_ids: vec!["profile/workflow-detail".to_owned()],
    };
    assert_eq!(valid.validation_error(), None);

    let mut padded_routine_id = valid.clone();
    padded_routine_id.metadata.routine_id = " routine/workflow/context-review".to_owned();
    assert_eq!(
        padded_routine_id.validation_error().as_deref(),
        Some("routine_id must not have leading or trailing whitespace")
    );

    let unsupported_source: ReviewRoutineSpec = serde_json::from_value(json!({
        "routine_id": "routine/future/source",
        "title": "Future Source",
        "source": {
            "kind": "script",
            "command": "external"
        }
    }))
    .expect("unsupported source kind should deserialize for validation");
    assert_eq!(
        unsupported_source.validation_error().as_deref(),
        Some("review routine source kind is unsupported")
    );

    let mut missing_workflow_reference = valid.clone();
    missing_workflow_reference.source = ReviewRoutineSource::Workflow {
        workflow_id: " ".to_owned(),
    };
    assert_eq!(
        missing_workflow_reference.validation_error().as_deref(),
        Some("workflow_id must not be empty")
    );

    let mut audit_with_inputs = valid.clone();
    audit_with_inputs.source = ReviewRoutineSource::Audit {
        audit: CorpusAuditKind::OrphanNotes,
        limit: 25,
    };
    assert_eq!(
        audit_with_inputs.validation_error().as_deref(),
        Some("audit review routines cannot declare workflow inputs")
    );

    let mut duplicate_inputs = valid.clone();
    duplicate_inputs.inputs.push(WorkflowInputSpec {
        input_id: "focus".to_owned(),
        title: "Duplicate Focus".to_owned(),
        summary: None,
        kind: WorkflowInputKind::FocusTarget,
    });
    assert_eq!(
        duplicate_inputs.validation_error().as_deref(),
        Some("review routine input 1 reuses duplicate input_id focus")
    );

    let mut disabled_save_with_metadata = valid.clone();
    disabled_save_with_metadata.save_review = ReviewRoutineSaveReviewPolicy {
        enabled: false,
        review_id: Some("review/routine/context".to_owned()),
        title: None,
        summary: None,
        overwrite: false,
    };
    disabled_save_with_metadata.compare = None;
    assert_eq!(
        disabled_save_with_metadata.validation_error().as_deref(),
        Some("disabled save_review policy cannot set review_id, title, summary, or overwrite")
    );

    let mut compare_without_save = valid.clone();
    compare_without_save.save_review = ReviewRoutineSaveReviewPolicy {
        enabled: false,
        review_id: None,
        title: None,
        summary: None,
        overwrite: false,
    };
    assert_eq!(
        compare_without_save.validation_error().as_deref(),
        Some("review routine compare policy requires save_review to be enabled")
    );

    let unsupported_compare: ReviewRoutineSpec = serde_json::from_value(json!({
        "routine_id": "routine/future/compare",
        "title": "Future Compare",
        "source": {
            "kind": "workflow",
            "workflow_id": BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID
        },
        "inputs": [],
        "compare": {
            "target": "scripted-baseline"
        }
    }))
    .expect("unsupported compare target should deserialize for validation");
    assert_eq!(
        unsupported_compare.validation_error().as_deref(),
        Some("review routine compare target is unsupported")
    );

    let mut padded_compare_profile = valid.clone();
    padded_compare_profile.compare = Some(ReviewRoutineComparePolicy {
        target: ReviewRoutineCompareTarget::LatestCompatibleReview,
        report_profile_id: Some(" profile/diff-focus".to_owned()),
    });
    assert_eq!(
        padded_compare_profile.validation_error().as_deref(),
        Some("report_profile_id must not have leading or trailing whitespace")
    );

    let mut padded_report_profile_ref = valid.clone();
    padded_report_profile_ref.report_profile_ids = vec![" profile/workflow-detail".to_owned()];
    assert_eq!(
        padded_report_profile_ref.validation_error().as_deref(),
        Some(
            "review routine report_profile_ids entry 0 is invalid: profile_id must not have leading or trailing whitespace"
        )
    );

    let mut duplicate_report_profile_ref = valid.clone();
    duplicate_report_profile_ref.report_profile_ids = vec![
        "profile/workflow-detail".to_owned(),
        "profile/workflow-detail".to_owned(),
    ];
    assert_eq!(
        duplicate_report_profile_ref.validation_error().as_deref(),
        Some("review routine report_profile_ids entry 1 is duplicate: profile/workflow-detail")
    );

    let catalog = ReviewRoutineCatalog {
        routines: vec![
            valid.clone(),
            ReviewRoutineSpec {
                metadata: ReviewRoutineMetadata {
                    routine_id: valid.metadata.routine_id.clone(),
                    title: "Duplicate Routine".to_owned(),
                    summary: None,
                },
                source: ReviewRoutineSource::Audit {
                    audit: CorpusAuditKind::DanglingLinks,
                    limit: 200,
                },
                inputs: Vec::new(),
                save_review: ReviewRoutineSaveReviewPolicy::default(),
                compare: None,
                report_profile_ids: Vec::new(),
            },
        ],
    };
    assert_eq!(
        catalog.validation_error().as_deref(),
        Some("review routine 1 reuses duplicate routine_id routine/workflow/context-review")
    );
}

#[test]
fn built_in_review_routines_are_valid_named_specs() {
    let routines = built_in_review_routines();
    assert_eq!(routines.len(), 2);
    assert!(
        routines
            .iter()
            .all(|routine| routine.validation_error().is_none())
    );
    assert!(routines.iter().any(|routine| {
        routine.metadata.routine_id == BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID
            && routine.source
                == ReviewRoutineSource::Workflow {
                    workflow_id: BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned(),
                }
            && routine.inputs.len() == 1
    }));
    assert!(routines.iter().any(|routine| {
        routine.metadata.routine_id == BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID
            && routine.source
                == ReviewRoutineSource::Audit {
                    audit: CorpusAuditKind::DuplicateTitles,
                    limit: 200,
                }
            && routine.inputs.is_empty()
    }));

    let context = built_in_review_routine(BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID)
        .expect("context routine should exist");
    assert_eq!(context.metadata.title, "Context Sweep Review");
    assert!(built_in_review_routine("routine/builtin/missing").is_none());

    let summaries = built_in_review_routine_summaries();
    assert_eq!(summaries.len(), routines.len());
    assert!(summaries.iter().any(|summary| {
        summary.metadata.routine_id == BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID
            && summary.input_count == 1
    }));
}

#[test]
fn workbench_pack_manifests_round_trip_with_bundled_assets() {
    let manifest = sample_workbench_pack_manifest();

    assert_eq!(manifest.validation_error(), None);
    assert!(manifest.validation_issues().is_empty());
    let summary = manifest.summary();
    assert_eq!(
        summary,
        WorkbenchPackSummary {
            metadata: manifest.metadata.clone(),
            compatibility: WorkbenchPackCompatibility::default(),
            workflow_count: 1,
            review_routine_count: 2,
            report_profile_count: 2,
            entrypoint_routine_ids: manifest.entrypoint_routine_ids.clone(),
        }
    );

    let serialized = serde_json::to_value(&manifest).expect("pack should serialize");
    assert_eq!(serialized["pack_id"], json!("pack/research-review"));
    assert_eq!(serialized["compatibility"]["version"], json!(1));
    assert_eq!(
        serialized["workflows"][0]["workflow_id"],
        json!("workflow/pack/context-review")
    );
    assert_eq!(
        serialized["review_routines"][0]["routine_id"],
        json!("routine/pack/context-review")
    );
    assert_eq!(
        serialized["report_profiles"][0]["profile_id"],
        json!("profile/routine-detail")
    );
    assert_eq!(
        serialized["entrypoint_routine_ids"],
        json!([
            "routine/pack/context-review",
            "routine/pack/duplicate-title-review"
        ])
    );

    let round_trip: WorkbenchPackManifest =
        serde_json::from_value(serialized).expect("pack should deserialize");
    assert_eq!(round_trip, manifest);

    let envelope: WorkbenchPackCompatibilityEnvelope = serde_json::from_value(json!({
        "pack_id": "pack/future",
        "compatibility": {
            "version": 2
        },
        "future_assets": [{
            "kind": "unknown"
        }]
    }))
    .expect("compatibility envelope should deserialize independently of future assets");
    assert_eq!(envelope.pack_id.as_deref(), Some("pack/future"));
    assert_eq!(
        envelope.compatibility.validation_error().as_deref(),
        Some("unsupported workbench pack compatibility version 2; supported version is 1")
    );
}

#[test]
fn workbench_pack_rpc_contracts_round_trip() {
    let manifest = sample_workbench_pack_manifest();
    let summary = WorkbenchPackSummary::from(&manifest);

    let import_params: ImportWorkbenchPackParams = serde_json::from_value(json!({
        "pack": manifest.clone()
    }))
    .expect("import params should deserialize with default overwrite");
    assert!(!import_params.overwrite);
    assert_eq!(import_params.validation_error(), None);
    assert_eq!(import_params.pack, manifest);

    let explicit_overwrite: ImportWorkbenchPackParams = serde_json::from_value(json!({
        "pack": manifest.clone(),
        "overwrite": true
    }))
    .expect("explicit overwrite import params should deserialize");
    assert!(explicit_overwrite.overwrite);

    let validate_params = ValidateWorkbenchPackParams {
        pack: manifest.clone(),
    };
    let import_result = ImportWorkbenchPackResult {
        pack: summary.clone(),
    };
    let show_result = WorkbenchPackResult {
        pack: manifest.clone(),
    };
    let validate_result = ValidateWorkbenchPackResult {
        pack: Some(summary.clone()),
        valid: true,
        issues: Vec::new(),
    };
    let list_result = ListWorkbenchPacksResult {
        packs: vec![summary.clone()],
        issues: Vec::new(),
    };
    let delete_result = DeleteWorkbenchPackResult {
        pack_id: manifest.metadata.pack_id.clone(),
    };
    let id_params = WorkbenchPackIdParams {
        pack_id: manifest.metadata.pack_id.clone(),
    };
    assert_eq!(id_params.validation_error(), None);

    assert_eq!(
        serde_json::from_value::<ValidateWorkbenchPackParams>(
            serde_json::to_value(&validate_params).expect("validate params should serialize")
        )
        .expect("validate params should deserialize"),
        validate_params
    );
    assert_eq!(
        serde_json::from_value::<ImportWorkbenchPackResult>(
            serde_json::to_value(&import_result).expect("import result should serialize")
        )
        .expect("import result should deserialize"),
        import_result
    );
    assert_eq!(
        serde_json::from_value::<WorkbenchPackResult>(
            serde_json::to_value(&show_result).expect("show result should serialize")
        )
        .expect("show result should deserialize"),
        show_result
    );
    assert_eq!(
        serde_json::from_value::<ValidateWorkbenchPackResult>(
            serde_json::to_value(&validate_result).expect("validate result should serialize")
        )
        .expect("validate result should deserialize"),
        validate_result
    );
    assert_eq!(
        serde_json::from_value::<ListWorkbenchPacksResult>(
            serde_json::to_value(&list_result).expect("list result should serialize")
        )
        .expect("list result should deserialize"),
        list_result
    );
    assert_eq!(
        serde_json::from_value::<DeleteWorkbenchPackResult>(
            serde_json::to_value(&delete_result).expect("delete result should serialize")
        )
        .expect("delete result should deserialize"),
        delete_result
    );
    assert_eq!(
        serde_json::from_value::<WorkbenchPackIdParams>(
            serde_json::to_value(&id_params).expect("id params should serialize")
        )
        .expect("id params should deserialize"),
        id_params
    );
}

#[test]
fn workbench_pack_manifests_report_malformed_assets_and_references() {
    let valid = sample_workbench_pack_manifest();
    assert_eq!(valid.validation_error(), None);

    let mut unsupported_version = valid.clone();
    unsupported_version.compatibility = WorkbenchPackCompatibility { version: 2 };
    let issues = unsupported_version.validation_issues();
    assert_eq!(issues[0].kind, WorkbenchPackIssueKind::UnsupportedVersion);
    assert_eq!(
        issues[0].message,
        "unsupported workbench pack compatibility version 2; supported version is 1"
    );

    let mut malformed_metadata = valid.clone();
    malformed_metadata.metadata.pack_id = " pack/research-review".to_owned();
    let issues = malformed_metadata.validation_issues();
    assert_eq!(issues[0].kind, WorkbenchPackIssueKind::InvalidMetadata);
    assert_eq!(
        issues[0].message,
        "pack_id must not have leading or trailing whitespace"
    );

    let empty_pack = WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: "pack/empty".to_owned(),
            title: "Empty".to_owned(),
            summary: None,
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: Vec::new(),
        review_routines: Vec::new(),
        report_profiles: Vec::new(),
        entrypoint_routine_ids: Vec::new(),
    };
    let issues = empty_pack.validation_issues();
    assert_eq!(issues[0].kind, WorkbenchPackIssueKind::EmptyPack);

    let mut invalid_workflow = valid.clone();
    invalid_workflow.workflows[0].steps.clear();
    let issues = invalid_workflow.validation_issues();
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::InvalidWorkflow)
    );

    let mut invalid_routine = valid.clone();
    invalid_routine.review_routines[0]
        .inputs
        .push(WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Duplicate Focus".to_owned(),
            summary: None,
            kind: WorkflowInputKind::FocusTarget,
        });
    let issues = invalid_routine.validation_issues();
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::InvalidReviewRoutine)
    );

    let mut invalid_profile = valid.clone();
    invalid_profile.report_profiles[0].subjects.clear();
    let issues = invalid_profile.validation_issues();
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::InvalidReportProfile)
    );

    let mut duplicate_ids = valid.clone();
    duplicate_ids
        .workflows
        .push(duplicate_ids.workflows[0].clone());
    duplicate_ids
        .review_routines
        .push(duplicate_ids.review_routines[0].clone());
    duplicate_ids
        .report_profiles
        .push(duplicate_ids.report_profiles[0].clone());
    let issues = duplicate_ids.validation_issues();
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::DuplicateWorkflowId)
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::DuplicateReviewRoutineId)
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::DuplicateReportProfileId)
    );

    let mut built_in_collision = valid.clone();
    built_in_collision.workflows[0].metadata.workflow_id =
        BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned();
    built_in_collision.review_routines[0].source = ReviewRoutineSource::Workflow {
        workflow_id: BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned(),
    };
    built_in_collision.review_routines[0].inputs[0].kind = WorkflowInputKind::NoteTarget;
    let issues = built_in_collision.validation_issues();
    assert!(issues.iter().any(|issue| {
        issue.kind == WorkbenchPackIssueKind::DuplicateWorkflowId
            && issue.message
                == format!(
                    "workbench pack workflow 0 collides with built-in workflow_id {BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID}"
                )
    }));
    assert!(issues.iter().any(|issue| {
        issue.kind == WorkbenchPackIssueKind::InvalidReviewRoutineReference
            && issue
                .message
                .contains("but referenced workflow requires focus-target")
    }));

    let mut built_in_routine_collision = valid.clone();
    built_in_routine_collision.review_routines[0]
        .metadata
        .routine_id = BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID.to_owned();
    let issues = built_in_routine_collision.validation_issues();
    assert!(issues.iter().any(|issue| {
        issue.kind == WorkbenchPackIssueKind::DuplicateReviewRoutineId
            && issue.message
                == format!(
                    "workbench pack review routine 0 collides with built-in routine_id {BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID}"
                )
    }));

    let mut missing_workflow = valid.clone();
    missing_workflow.review_routines[0].source = ReviewRoutineSource::Workflow {
        workflow_id: "workflow/missing".to_owned(),
    };
    let issues = missing_workflow.validation_issues();
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::MissingWorkflowReference)
    );

    let mut mismatched_inputs = valid.clone();
    mismatched_inputs.review_routines[0].inputs[0].kind = WorkflowInputKind::NoteTarget;
    let issues = mismatched_inputs.validation_issues();
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::InvalidReviewRoutineReference)
    );

    let mut missing_profile = valid.clone();
    missing_profile.review_routines[0].report_profile_ids = vec!["profile/missing".to_owned()];
    let issues = missing_profile.validation_issues();
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::MissingReportProfileReference)
    );

    let mut missing_entrypoint = valid.clone();
    missing_entrypoint.entrypoint_routine_ids = vec![
        "routine/pack/context-review".to_owned(),
        "routine/pack/context-review".to_owned(),
        "routine/pack/missing".to_owned(),
    ];
    let issues = missing_entrypoint.validation_issues();
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::DuplicateReviewRoutineReference)
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == WorkbenchPackIssueKind::MissingReviewRoutineReference)
    );
}

#[test]
fn corpus_audit_params_normalize_limit() {
    let params: CorpusAuditParams = serde_json::from_value(json!({
        "audit": "weakly-integrated-notes",
        "limit": 800
    }))
    .expect("audit params should deserialize");
    assert_eq!(params.audit, CorpusAuditKind::WeaklyIntegratedNotes);
    assert_eq!(params.normalized_limit(), 500);
    assert_eq!(
        serde_json::to_value(&params).expect("audit params should serialize"),
        json!({
            "audit": "weakly-integrated-notes",
            "limit": 800
        })
    );
}

#[test]
fn review_runs_round_trip_with_audit_and_workflow_findings() {
    let audit_entry = CorpusAuditEntry::DanglingLink {
        record: Box::new(DanglingLinkAuditRecord {
            source: sample_anchor("heading:source.org:3", "Source Heading"),
            missing_explicit_id: "missing-id".to_owned(),
            line: 12,
            column: 7,
            preview: "[[id:missing-id][Missing]]".to_owned(),
        }),
    };
    let audit_review = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/dangling-links/2026-05-05".to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: Some("Review missing id links".to_owned()),
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 200,
        },
        findings: vec![
            ReviewFinding {
                finding_id: "audit/dangling-links/source/missing-id".to_owned(),
                status: ReviewFindingStatus::Open,
                payload: ReviewFindingPayload::Audit {
                    entry: Box::new(audit_entry.clone()),
                },
            },
            ReviewFinding {
                finding_id: "audit/dangling-links/source/other-missing-id".to_owned(),
                status: ReviewFindingStatus::Dismissed,
                payload: ReviewFindingPayload::Audit {
                    entry: Box::new(CorpusAuditEntry::DanglingLink {
                        record: Box::new(DanglingLinkAuditRecord {
                            source: sample_anchor("heading:source.org:3", "Source Heading"),
                            missing_explicit_id: "other-missing-id".to_owned(),
                            line: 18,
                            column: 3,
                            preview: "[[id:other-missing-id][Missing]]".to_owned(),
                        }),
                    }),
                },
            },
        ],
    };

    assert_eq!(audit_review.validation_error(), None);
    assert_eq!(audit_review.kind(), super::ReviewRunKind::Audit);
    assert_eq!(
        audit_review.findings[0].kind(),
        super::ReviewFindingKind::Audit
    );

    let audit_summary = ReviewRunSummary::from(&audit_review);
    assert_eq!(audit_summary.finding_count, 2);
    assert_eq!(audit_summary.status_counts.open, 1);
    assert_eq!(audit_summary.status_counts.dismissed, 1);

    let serialized = serde_json::to_value(&audit_review).expect("audit review should serialize");
    assert_eq!(
        serialized["review_id"],
        json!("review/audit/dangling-links/2026-05-05")
    );
    assert_eq!(serialized["kind"], json!("audit"));
    assert_eq!(serialized["audit"], json!("dangling-links"));
    assert_eq!(serialized["limit"], json!(200));
    assert_eq!(serialized["findings"][0]["kind"], json!("audit"));
    assert_eq!(serialized["findings"][0]["status"], json!("open"));
    assert_eq!(
        serialized["findings"][0]["entry"]["kind"],
        json!("dangling-link")
    );

    let round_trip: ReviewRun =
        serde_json::from_value(serialized).expect("audit review should deserialize");
    assert_eq!(round_trip, audit_review);

    let workflow = WorkflowSummary {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/research/context".to_owned(),
            title: "Research Context".to_owned(),
            summary: Some("Collect review context".to_owned()),
        },
        step_count: 2,
    };
    let workflow_review = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/workflow/context/2026-05-05".to_owned(),
            title: "Workflow Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Workflow {
            workflow: workflow.clone(),
            inputs: vec![WorkflowInputAssignment {
                input_id: "focus".to_owned(),
                target: WorkflowResolveTarget::NodeKey {
                    node_key: "heading:focus.org:3".to_owned(),
                },
            }],
            step_ids: vec!["resolve-focus".to_owned(), "explore-focus".to_owned()],
        },
        findings: vec![ReviewFinding {
            finding_id: "workflow-step/explore-focus".to_owned(),
            status: ReviewFindingStatus::Reviewed,
            payload: ReviewFindingPayload::WorkflowStep {
                step: Box::new(WorkflowStepReport {
                    step_id: "explore-focus".to_owned(),
                    payload: WorkflowStepReportPayload::Explore {
                        focus_node_key: "heading:focus.org:3".to_owned(),
                        result: Box::new(ExploreResult {
                            lens: ExplorationLens::Unresolved,
                            sections: Vec::new(),
                        }),
                    },
                }),
            },
        }],
    };

    assert_eq!(workflow_review.validation_error(), None);
    let workflow_json =
        serde_json::to_value(&workflow_review).expect("workflow review should serialize");
    assert_eq!(workflow_json["kind"], json!("workflow"));
    assert_eq!(
        workflow_json["workflow"]["workflow_id"],
        json!("workflow/research/context")
    );
    assert_eq!(
        workflow_json["step_ids"],
        json!(["resolve-focus", "explore-focus"])
    );
    assert_eq!(workflow_json["inputs"][0]["input_id"], json!("focus"));
    assert_eq!(
        workflow_json["inputs"][0]["node_key"],
        json!("heading:focus.org:3")
    );
    assert_eq!(workflow_json["findings"][0]["kind"], json!("workflow-step"));
    assert_eq!(workflow_json["findings"][0]["status"], json!("reviewed"));

    let audit_review_params = SaveCorpusAuditReviewParams {
        audit: CorpusAuditKind::DanglingLinks,
        limit: 50,
        review_id: Some("review/audit/dangling-links/custom".to_owned()),
        title: Some("Custom Dangling Review".to_owned()),
        summary: None,
        overwrite: false,
    };
    let workflow_review_params = SaveWorkflowReviewParams {
        workflow_id: "workflow/research/context".to_owned(),
        inputs: vec![WorkflowInputAssignment {
            input_id: "focus".to_owned(),
            target: WorkflowResolveTarget::NodeKey {
                node_key: "heading:focus.org:3".to_owned(),
            },
        }],
        review_id: None,
        title: Some("Custom Workflow Review".to_owned()),
        summary: None,
        overwrite: false,
    };
    let save_params = SaveReviewRunParams {
        review: audit_review.clone(),
        overwrite: false,
    };
    let mark_params = MarkReviewFindingParams {
        review_id: "review/workflow/context/2026-05-05".to_owned(),
        finding_id: "workflow-step/explore-focus".to_owned(),
        status: ReviewFindingStatus::Accepted,
    };
    let preview_params = ReviewFindingRemediationPreviewParams {
        review_id: "review/audit/dangling-links/2026-05-05".to_owned(),
        finding_id: "audit/dangling-links/source/missing-id".to_owned(),
    };
    let diff_params = ReviewRunDiffParams {
        base_review_id: "review/audit/dangling-links/2026-05-04".to_owned(),
        target_review_id: "review/audit/dangling-links/2026-05-05".to_owned(),
    };
    let save_result = SaveReviewRunResult {
        review: ReviewRunSummary::from(&audit_review),
    };
    let review_result = ReviewRunResult {
        review: workflow_review.clone(),
    };
    let list_result = super::ListReviewRunsResult {
        reviews: vec![
            ReviewRunSummary::from(&audit_review),
            ReviewRunSummary::from(&workflow_review),
        ],
    };
    let delete_result = super::DeleteReviewRunResult {
        review_id: "review/workflow/context/2026-05-05".to_owned(),
    };
    let mark_result = super::MarkReviewFindingResult {
        transition: ReviewFindingStatusTransition {
            review_id: "review/workflow/context/2026-05-05".to_owned(),
            finding_id: "workflow-step/explore-focus".to_owned(),
            from_status: ReviewFindingStatus::Open,
            to_status: ReviewFindingStatus::Reviewed,
        },
    };
    let audit_review_result = SaveCorpusAuditReviewResult {
        result: CorpusAuditResult {
            audit: CorpusAuditKind::DanglingLinks,
            entries: vec![audit_entry],
        },
        review: ReviewRunSummary::from(&audit_review),
    };
    let workflow_review_result = SaveWorkflowReviewResult {
        result: WorkflowExecutionResult {
            workflow,
            steps: vec![match &workflow_review.findings[0].payload {
                ReviewFindingPayload::WorkflowStep { step } => step.as_ref().clone(),
                _ => panic!("expected workflow-step finding"),
            }],
        },
        review: ReviewRunSummary::from(&workflow_review),
    };
    let diff_result = ReviewRunDiffResult {
        diff: ReviewRunDiff::between(&audit_review, &audit_review)
            .expect("same audit review should diff"),
    };
    let preview_result = ReviewFindingRemediationPreviewResult {
        preview: ReviewFindingRemediationPreview::from_review_finding(
            "review/audit/dangling-links/2026-05-05",
            &audit_review.findings[0],
        )
        .expect("dangling-link finding should have a preview"),
    };

    assert_eq!(
        serde_json::from_value::<SaveReviewRunResult>(
            serde_json::to_value(&save_result).expect("save result should serialize"),
        )
        .expect("save result should deserialize"),
        save_result
    );
    assert_eq!(
        serde_json::from_value::<ReviewRunResult>(
            serde_json::to_value(&review_result).expect("review result should serialize"),
        )
        .expect("review result should deserialize"),
        review_result
    );
    assert_eq!(
        serde_json::from_value::<super::ListReviewRunsResult>(
            serde_json::to_value(&list_result).expect("list result should serialize"),
        )
        .expect("list result should deserialize"),
        list_result
    );
    assert_eq!(
        serde_json::from_value::<super::DeleteReviewRunResult>(
            serde_json::to_value(&delete_result).expect("delete result should serialize"),
        )
        .expect("delete result should deserialize"),
        delete_result
    );
    assert_eq!(
        serde_json::from_value::<super::MarkReviewFindingResult>(
            serde_json::to_value(&mark_result).expect("mark result should serialize"),
        )
        .expect("mark result should deserialize"),
        mark_result
    );
    assert_eq!(
        serde_json::from_value::<SaveReviewRunParams>(
            serde_json::to_value(&save_params).expect("save params should serialize"),
        )
        .expect("save params should deserialize"),
        save_params
    );
    assert_eq!(
        serde_json::from_value::<MarkReviewFindingParams>(
            serde_json::to_value(&mark_params).expect("mark params should serialize"),
        )
        .expect("mark params should deserialize"),
        mark_params
    );
    assert_eq!(
        serde_json::from_value::<ReviewRunDiffParams>(
            serde_json::to_value(&diff_params).expect("diff params should serialize"),
        )
        .expect("diff params should deserialize"),
        diff_params
    );
    assert_eq!(
        serde_json::from_value::<ReviewFindingRemediationPreviewParams>(
            serde_json::to_value(&preview_params).expect("preview params should serialize"),
        )
        .expect("preview params should deserialize"),
        preview_params
    );
    assert_eq!(
        serde_json::from_value::<SaveCorpusAuditReviewParams>(
            serde_json::to_value(&audit_review_params)
                .expect("audit review params should serialize"),
        )
        .expect("audit review params should deserialize"),
        audit_review_params
    );
    assert_eq!(
        serde_json::from_value::<SaveWorkflowReviewParams>(
            serde_json::to_value(&workflow_review_params)
                .expect("workflow review params should serialize"),
        )
        .expect("workflow review params should deserialize"),
        workflow_review_params
    );
    assert_eq!(
        serde_json::from_value::<SaveCorpusAuditReviewResult>(
            serde_json::to_value(&audit_review_result)
                .expect("audit review result should serialize"),
        )
        .expect("audit review result should deserialize"),
        audit_review_result
    );
    assert_eq!(
        serde_json::from_value::<SaveWorkflowReviewResult>(
            serde_json::to_value(&workflow_review_result)
                .expect("workflow review result should serialize"),
        )
        .expect("workflow review result should deserialize"),
        workflow_review_result
    );
    assert_eq!(
        serde_json::from_value::<ReviewRunDiffResult>(
            serde_json::to_value(&diff_result).expect("diff result should serialize"),
        )
        .expect("diff result should deserialize"),
        diff_result
    );
    assert_eq!(
        serde_json::from_value::<ReviewFindingRemediationPreviewResult>(
            serde_json::to_value(&preview_result).expect("preview result should serialize"),
        )
        .expect("preview result should deserialize"),
        preview_result
    );

    let default_save_params: SaveReviewRunParams = serde_json::from_value(json!({
        "review": audit_review
    }))
    .expect("save params should default overwrite");
    assert!(default_save_params.overwrite);
}

#[test]
fn review_finding_remediation_previews_cover_supported_audit_findings() {
    let dangling_finding = ReviewFinding {
        finding_id: "audit/dangling-links/source/missing-id".to_owned(),
        status: ReviewFindingStatus::Open,
        payload: ReviewFindingPayload::Audit {
            entry: Box::new(CorpusAuditEntry::DanglingLink {
                record: Box::new(DanglingLinkAuditRecord {
                    source: sample_anchor("file:source.org", "Source"),
                    missing_explicit_id: "missing-id".to_owned(),
                    line: 12,
                    column: 7,
                    preview: "[[id:missing-id][Missing]]".to_owned(),
                }),
            }),
        },
    };
    let dangling_preview = ReviewFindingRemediationPreview::from_review_finding(
        "review/audit/dangling-links",
        &dangling_finding,
    )
    .expect("dangling link should be previewable");
    assert_eq!(dangling_preview.review_id, "review/audit/dangling-links");
    assert_eq!(
        dangling_preview.finding_id,
        "audit/dangling-links/source/missing-id"
    );
    assert_eq!(dangling_preview.status, ReviewFindingStatus::Open);
    assert_eq!(
        dangling_preview.preview_identity,
        sample_dangling_preview_identity("file:source.org", "missing-id")
    );
    match dangling_preview.payload {
        super::AuditRemediationPreviewPayload::DanglingLink {
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
            assert_eq!(source.node_key, "file:source.org");
            assert_eq!(missing_explicit_id, "missing-id");
            assert_eq!(file_path, "sample.org");
            assert_eq!(line, 12);
            assert_eq!(column, 7);
            assert_eq!(preview, "[[id:missing-id][Missing]]");
            assert!(suggestion.contains("id:missing-id"));
            assert_eq!(confidence, super::AuditRemediationConfidence::Medium);
            assert!(reason.contains("missing-id"));
        }
        other => panic!("expected dangling-link preview, got {other:?}"),
    }

    let duplicate_finding = ReviewFinding {
        finding_id: "audit/duplicate-titles/shared-title".to_owned(),
        status: ReviewFindingStatus::Reviewed,
        payload: ReviewFindingPayload::Audit {
            entry: Box::new(CorpusAuditEntry::DuplicateTitle {
                record: Box::new(DuplicateTitleAuditRecord {
                    title: "Shared Title".to_owned(),
                    notes: vec![
                        sample_node("file:a.org", "Shared Title"),
                        sample_node("file:b.org", "Shared Title"),
                    ],
                }),
            }),
        },
    };
    let duplicate_preview = ReviewFindingRemediationPreview::from_review_finding(
        "review/audit/duplicate-titles",
        &duplicate_finding,
    )
    .expect("duplicate title should be previewable");
    match duplicate_preview.payload {
        super::AuditRemediationPreviewPayload::DuplicateTitle {
            title,
            notes,
            suggestion,
            confidence,
            reason,
        } => {
            assert_eq!(title, "Shared Title");
            assert_eq!(notes.len(), 2);
            assert!(suggestion.contains("Disambiguate"));
            assert_eq!(confidence, super::AuditRemediationConfidence::High);
            assert!(reason.contains("2 notes"));
        }
        other => panic!("expected duplicate-title preview, got {other:?}"),
    }

    let unsupported = ReviewFinding {
        finding_id: "audit/orphan-notes/source".to_owned(),
        status: ReviewFindingStatus::Open,
        payload: ReviewFindingPayload::Audit {
            entry: Box::new(CorpusAuditEntry::OrphanNote {
                record: Box::new(NoteConnectivityAuditRecord {
                    note: sample_node("file:orphan.org", "Orphan"),
                    reference_count: 0,
                    backlink_count: 0,
                    forward_link_count: 0,
                }),
            }),
        },
    };
    assert_eq!(
        ReviewFindingRemediationPreview::from_review_finding(
            "review/audit/orphan-notes",
            &unsupported,
        )
        .expect_err("orphan finding should not be previewable"),
        "review finding has no remediation preview for orphan-note evidence"
    );
}

#[test]
fn review_finding_remediation_apply_contracts_round_trip() {
    let expected_preview = sample_dangling_preview_identity("heading:source.org:3", "missing-id");
    let action = sample_unlink_dangling_action("missing-id", "Missing");
    let params = ReviewFindingRemediationApplyParams {
        review_id: "review/audit/dangling-links".to_owned(),
        finding_id: "audit/dangling-links/source/missing-id".to_owned(),
        expected_preview: expected_preview.clone(),
        action: action.clone(),
    };
    assert_eq!(params.validation_error(), None);
    assert_eq!(action.preview_identity(), expected_preview);

    let application = ReviewFindingRemediationApplication {
        review_id: params.review_id.clone(),
        finding_id: params.finding_id.clone(),
        preview_identity: expected_preview,
        action,
        affected_files: StructuralWriteAffectedFiles {
            changed_files: vec!["sample.org".to_owned()],
            removed_files: Vec::new(),
        },
        index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
    };
    assert_eq!(application.validation_error(), None);
    let result = ReviewFindingRemediationApplyResult {
        application: application.clone(),
    };

    let serialized_params = serde_json::to_value(&params).expect("apply params should serialize");
    assert_eq!(
        serialized_params["expected_preview"]["kind"],
        json!("dangling-link")
    );
    assert_eq!(
        serialized_params["action"]["kind"],
        json!("unlink-dangling-link")
    );
    assert_eq!(
        serde_json::from_value::<ReviewFindingRemediationApplyParams>(serialized_params)
            .expect("apply params should deserialize"),
        params
    );
    assert_eq!(
        serde_json::from_value::<ReviewFindingRemediationApplyResult>(
            serde_json::to_value(&result).expect("apply result should serialize"),
        )
        .expect("apply result should deserialize"),
        result
    );
}

#[test]
fn review_finding_remediation_apply_contracts_reject_mismatches() {
    let expected_preview = sample_dangling_preview_identity("heading:source.org:3", "missing-id");
    let mismatched_action = sample_unlink_dangling_action("other-missing-id", "Missing");
    let params = ReviewFindingRemediationApplyParams {
        review_id: "review/audit/dangling-links".to_owned(),
        finding_id: "audit/dangling-links/source/missing-id".to_owned(),
        expected_preview: expected_preview.clone(),
        action: mismatched_action,
    };
    assert_eq!(
        params.validation_error(),
        Some("remediation action must match the expected preview identity".to_owned())
    );

    let duplicate_title_preview = AuditRemediationPreviewIdentity::DuplicateTitle {
        title: "Shared".to_owned(),
        node_keys: vec!["file:left.org".to_owned(), "file:right.org".to_owned()],
    };
    let params = ReviewFindingRemediationApplyParams {
        review_id: "review/audit/duplicate-titles".to_owned(),
        finding_id: "audit/duplicate-titles/shared".to_owned(),
        expected_preview: duplicate_title_preview,
        action: sample_unlink_dangling_action("missing-id", "Missing"),
    };
    assert_eq!(
        params.validation_error(),
        Some("remediation action must match the expected preview identity".to_owned())
    );

    let mut application = ReviewFindingRemediationApplication {
        review_id: "review/audit/dangling-links".to_owned(),
        finding_id: "audit/dangling-links/source/missing-id".to_owned(),
        preview_identity: expected_preview,
        action: sample_unlink_dangling_action("missing-id", "Missing"),
        affected_files: StructuralWriteAffectedFiles {
            changed_files: vec!["other.org".to_owned()],
            removed_files: Vec::new(),
        },
        index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
    };
    assert_eq!(
        application.validation_error(),
        Some(
            "remediation application affected files must include changed file sample.org"
                .to_owned()
        )
    );

    application.affected_files.changed_files = vec!["sample.org".to_owned()];
    application.affected_files.removed_files = vec!["removed.org".to_owned()];
    assert_eq!(
        application.validation_error(),
        Some("remediation applications must not remove files for supported actions".to_owned())
    );

    application.affected_files.removed_files.clear();
    application.index_refresh = StructuralWriteIndexRefreshStatus::Pending;
    assert_eq!(
        application.validation_error(),
        Some("remediation applications must be returned after index refresh".to_owned())
    );
}

#[test]
fn review_run_diffs_classify_findings_deterministically() {
    let base = sample_dangling_review(
        "review/audit/dangling-links/base",
        vec![
            sample_dangling_finding(
                "audit/dangling-links/added-order/status-changed",
                "status-changed",
                ReviewFindingStatus::Open,
            ),
            sample_dangling_finding(
                "audit/dangling-links/added-order/removed",
                "removed",
                ReviewFindingStatus::Dismissed,
            ),
            sample_dangling_finding(
                "audit/dangling-links/added-order/unchanged",
                "unchanged",
                ReviewFindingStatus::Reviewed,
            ),
        ],
    );
    let target = sample_dangling_review(
        "review/audit/dangling-links/target",
        vec![
            sample_dangling_finding(
                "audit/dangling-links/added-order/unchanged",
                "unchanged",
                ReviewFindingStatus::Reviewed,
            ),
            sample_dangling_finding(
                "audit/dangling-links/added-order/added",
                "added",
                ReviewFindingStatus::Open,
            ),
            sample_dangling_finding(
                "audit/dangling-links/added-order/status-changed",
                "status-changed",
                ReviewFindingStatus::Accepted,
            ),
        ],
    );

    let diff = ReviewRunDiff::between(&base, &target).expect("compatible reviews should diff");

    assert_eq!(diff.base_review.metadata.review_id, base.metadata.review_id);
    assert_eq!(
        diff.target_review.metadata.review_id,
        target.metadata.review_id
    );
    assert_eq!(
        diff.added
            .iter()
            .map(|finding| finding.finding_id.as_str())
            .collect::<Vec<_>>(),
        vec!["audit/dangling-links/added-order/added"]
    );
    assert_eq!(
        diff.removed
            .iter()
            .map(|finding| finding.finding_id.as_str())
            .collect::<Vec<_>>(),
        vec!["audit/dangling-links/added-order/removed"]
    );
    assert_eq!(
        diff.unchanged
            .iter()
            .map(|finding| finding.finding_id.as_str())
            .collect::<Vec<_>>(),
        vec!["audit/dangling-links/added-order/unchanged"]
    );
    assert!(diff.content_changed.is_empty());
    assert_eq!(diff.status_changed.len(), 1);
    assert_eq!(
        diff.status_changed[0].finding_id,
        "audit/dangling-links/added-order/status-changed"
    );
    assert_eq!(
        diff.status_changed[0].from_status,
        ReviewFindingStatus::Open
    );
    assert_eq!(
        diff.status_changed[0].to_status,
        ReviewFindingStatus::Accepted
    );
}

#[test]
fn review_run_diffs_separate_content_changes_from_unchanged_findings() {
    let workflow = WorkflowSummary {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/research/context".to_owned(),
            title: "Research Context".to_owned(),
            summary: None,
        },
        step_count: 1,
    };
    let base = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/workflow/context/base".to_owned(),
            title: "Workflow Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Workflow {
            workflow: workflow.clone(),
            inputs: Vec::new(),
            step_ids: vec!["resolve-focus".to_owned()],
        },
        findings: vec![ReviewFinding {
            finding_id: "workflow-step/resolve-focus".to_owned(),
            status: ReviewFindingStatus::Open,
            payload: ReviewFindingPayload::WorkflowStep {
                step: Box::new(WorkflowStepReport {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepReportPayload::Resolve {
                        node: Box::new(sample_node("heading:focus.org:3", "Old Focus")),
                    },
                }),
            },
        }],
    };
    let target = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/workflow/context/target".to_owned(),
            title: "Workflow Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Workflow {
            workflow,
            inputs: Vec::new(),
            step_ids: vec!["resolve-focus".to_owned()],
        },
        findings: vec![ReviewFinding {
            finding_id: "workflow-step/resolve-focus".to_owned(),
            status: ReviewFindingStatus::Open,
            payload: ReviewFindingPayload::WorkflowStep {
                step: Box::new(WorkflowStepReport {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepReportPayload::Resolve {
                        node: Box::new(sample_node("heading:focus.org:3", "New Focus")),
                    },
                }),
            },
        }],
    };

    let diff = ReviewRunDiff::between(&base, &target).expect("compatible reviews should diff");

    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());
    assert!(diff.unchanged.is_empty());
    assert!(diff.status_changed.is_empty());
    assert_eq!(diff.content_changed.len(), 1);
    assert_eq!(
        diff.content_changed[0].finding_id,
        "workflow-step/resolve-focus"
    );
    match &diff.content_changed[0].base.payload {
        ReviewFindingPayload::WorkflowStep { step } => match &step.payload {
            WorkflowStepReportPayload::Resolve { node } => assert_eq!(node.title, "Old Focus"),
            other => panic!("expected resolve payload, got {:?}", other.kind()),
        },
        other => panic!("expected workflow-step payload, got {:?}", other.kind()),
    }
    match &diff.content_changed[0].target.payload {
        ReviewFindingPayload::WorkflowStep { step } => match &step.payload {
            WorkflowStepReportPayload::Resolve { node } => assert_eq!(node.title, "New Focus"),
            other => panic!("expected resolve payload, got {:?}", other.kind()),
        },
        other => panic!("expected workflow-step payload, got {:?}", other.kind()),
    }
}

#[test]
fn review_run_diffs_reject_incompatible_review_sources() {
    let audit_review = sample_dangling_review(
        "review/audit/dangling-links",
        vec![sample_dangling_finding(
            "audit/dangling-links/source/missing-id",
            "missing-id",
            ReviewFindingStatus::Open,
        )],
    );
    let different_audit = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/orphan-notes".to_owned(),
            title: "Orphan Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::OrphanNotes,
            limit: 200,
        },
        findings: Vec::new(),
    };
    let audit_error = ReviewRunDiff::between(&audit_review, &different_audit)
        .expect_err("different audit kinds should be incompatible");
    assert!(audit_error.contains("different audit kinds"));

    let workflow = WorkflowSummary {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/research/context".to_owned(),
            title: "Research Context".to_owned(),
            summary: None,
        },
        step_count: 1,
    };
    let workflow_review = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/workflow/context".to_owned(),
            title: "Workflow Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Workflow {
            workflow: workflow.clone(),
            inputs: Vec::new(),
            step_ids: vec!["resolve-focus".to_owned()],
        },
        findings: Vec::new(),
    };
    let changed_workflow_review = ReviewRun {
        payload: ReviewRunPayload::Workflow {
            workflow,
            inputs: vec![WorkflowInputAssignment {
                input_id: "focus".to_owned(),
                target: WorkflowResolveTarget::NodeKey {
                    node_key: "heading:focus.org:3".to_owned(),
                },
            }],
            step_ids: vec!["resolve-focus".to_owned()],
        },
        ..workflow_review.clone()
    };
    let workflow_error = ReviewRunDiff::between(&workflow_review, &changed_workflow_review)
        .expect_err("different workflow inputs should be incompatible");
    assert!(workflow_error.contains("different inputs"));

    let cross_kind_error = ReviewRunDiff::between(&audit_review, &workflow_review)
        .expect_err("cross-kind reviews should be incompatible");
    assert_eq!(
        cross_kind_error,
        "cannot diff review runs with different kinds"
    );
}

#[test]
fn review_runs_reject_malformed_records_and_invalid_status_transitions() {
    let valid_finding = ReviewFinding {
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
    };

    let blank_metadata = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: " ".to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 200,
        },
        findings: vec![valid_finding.clone()],
    };
    assert_eq!(
        blank_metadata.validation_error().as_deref(),
        Some("review_id must not be empty")
    );

    let padded_id = ReviewRunIdParams {
        review_id: " review/audit ".to_owned(),
    };
    assert_eq!(
        padded_id.validation_error().as_deref(),
        Some("review_id must not have leading or trailing whitespace")
    );

    let duplicate_findings = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/dangling-links".to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 200,
        },
        findings: vec![valid_finding.clone(), valid_finding.clone()],
    };
    assert_eq!(
        duplicate_findings.validation_error().as_deref(),
        Some("review finding 1 reuses duplicate finding_id audit/dangling-links/source/missing-id")
    );

    let padded_finding = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/dangling-links".to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 200,
        },
        findings: vec![ReviewFinding {
            finding_id: " audit/finding ".to_owned(),
            ..valid_finding.clone()
        }],
    };
    assert_eq!(
        padded_finding.validation_error().as_deref(),
        Some(
            "review finding 0 is invalid: finding_id must not have leading or trailing whitespace"
        )
    );

    let wrong_audit_kind = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/orphans".to_owned(),
            title: "Orphan Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::OrphanNotes,
            limit: 200,
        },
        findings: vec![valid_finding.clone()],
    };
    assert_eq!(
        wrong_audit_kind.validation_error().as_deref(),
        Some("review finding 0 is invalid: audit review findings must match review audit kind")
    );

    let workflow_step_in_audit = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/dangling-links".to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 200,
        },
        findings: vec![ReviewFinding {
            finding_id: "workflow-step/explore-focus".to_owned(),
            status: ReviewFindingStatus::Reviewed,
            payload: ReviewFindingPayload::WorkflowStep {
                step: Box::new(WorkflowStepReport {
                    step_id: "explore-focus".to_owned(),
                    payload: WorkflowStepReportPayload::Explore {
                        focus_node_key: "heading:focus.org:3".to_owned(),
                        result: Box::new(ExploreResult {
                            lens: ExplorationLens::Unresolved,
                            sections: Vec::new(),
                        }),
                    },
                }),
            },
        }],
    };
    assert_eq!(
        workflow_step_in_audit.validation_error().as_deref(),
        Some(
            "review finding 0 is invalid: audit review runs cannot contain workflow-step findings"
        )
    );

    let malformed_audit_entry = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/dangling-links".to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 200,
        },
        findings: vec![ReviewFinding {
            payload: ReviewFindingPayload::Audit {
                entry: Box::new(CorpusAuditEntry::DanglingLink {
                    record: Box::new(DanglingLinkAuditRecord {
                        source: sample_anchor("", "Source Heading"),
                        missing_explicit_id: "missing-id".to_owned(),
                        line: 12,
                        column: 7,
                        preview: "[[id:missing-id][Missing]]".to_owned(),
                    }),
                }),
            },
            ..valid_finding
        }],
    };
    assert_eq!(
        malformed_audit_entry.validation_error().as_deref(),
        Some("review finding 0 is invalid: source.node_key must not be empty")
    );

    let workflow = WorkflowSummary {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/research/context".to_owned(),
            title: "Research Context".to_owned(),
            summary: None,
        },
        step_count: 2,
    };
    let workflow_step = WorkflowStepReport {
        step_id: "explore-focus".to_owned(),
        payload: WorkflowStepReportPayload::Explore {
            focus_node_key: "heading:focus.org:3".to_owned(),
            result: Box::new(ExploreResult {
                lens: ExplorationLens::Unresolved,
                sections: Vec::new(),
            }),
        },
    };

    let mismatched_workflow_source = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/workflow/context".to_owned(),
            title: "Workflow Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Workflow {
            workflow: workflow.clone(),
            inputs: Vec::new(),
            step_ids: vec!["explore-focus".to_owned()],
        },
        findings: Vec::new(),
    };
    assert_eq!(
        mismatched_workflow_source.validation_error().as_deref(),
        Some("workflow review source step_ids must match workflow step_count")
    );

    let duplicate_source_step_ids = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/workflow/context".to_owned(),
            title: "Workflow Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Workflow {
            workflow: workflow.clone(),
            inputs: Vec::new(),
            step_ids: vec!["explore-focus".to_owned(), "explore-focus".to_owned()],
        },
        findings: Vec::new(),
    };
    assert_eq!(
        duplicate_source_step_ids.validation_error().as_deref(),
        Some("workflow review source step_id 1 reuses duplicate step_id explore-focus")
    );

    let unknown_workflow_step = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/workflow/context".to_owned(),
            title: "Workflow Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Workflow {
            workflow: workflow.clone(),
            inputs: Vec::new(),
            step_ids: vec!["resolve-focus".to_owned(), "explore-focus".to_owned()],
        },
        findings: vec![ReviewFinding {
            finding_id: "workflow-step/compare-focus".to_owned(),
            status: ReviewFindingStatus::Open,
            payload: ReviewFindingPayload::WorkflowStep {
                step: Box::new(WorkflowStepReport {
                    step_id: "compare-focus".to_owned(),
                    ..workflow_step.clone()
                }),
            },
        }],
    };
    assert_eq!(
        unknown_workflow_step.validation_error().as_deref(),
        Some(
            "review finding 0 is invalid: workflow-step findings must reference a source workflow step"
        )
    );

    let duplicate_workflow_step_finding = ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/workflow/context".to_owned(),
            title: "Workflow Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Workflow {
            workflow,
            inputs: Vec::new(),
            step_ids: vec!["resolve-focus".to_owned(), "explore-focus".to_owned()],
        },
        findings: vec![
            ReviewFinding {
                finding_id: "workflow-step/explore-focus".to_owned(),
                status: ReviewFindingStatus::Open,
                payload: ReviewFindingPayload::WorkflowStep {
                    step: Box::new(workflow_step.clone()),
                },
            },
            ReviewFinding {
                finding_id: "workflow-step/explore-focus-copy".to_owned(),
                status: ReviewFindingStatus::Reviewed,
                payload: ReviewFindingPayload::WorkflowStep {
                    step: Box::new(workflow_step),
                },
            },
        ],
    };
    assert_eq!(
        duplicate_workflow_step_finding
            .validation_error()
            .as_deref(),
        Some("review finding 1 reuses duplicate workflow step_id explore-focus")
    );

    let no_op_transition = ReviewFindingStatusTransition {
        review_id: "review/audit/dangling-links".to_owned(),
        finding_id: "audit/dangling-links/source/missing-id".to_owned(),
        from_status: ReviewFindingStatus::Open,
        to_status: ReviewFindingStatus::Open,
    };
    assert_eq!(
        no_op_transition.validation_error().as_deref(),
        Some("review finding status transition must change status")
    );
}

#[test]
fn workflow_specs_reject_malformed_metadata_and_step_references() {
    let blank_metadata = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: " ".to_owned(),
            title: "Workflow".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![WorkflowStepSpec {
            step_id: "resolve-focus".to_owned(),
            payload: WorkflowStepPayload::Resolve {
                target: WorkflowResolveTarget::Id {
                    id: "focus-id".to_owned(),
                },
            },
        }],
    };
    assert_eq!(
        blank_metadata.validation_error().as_deref(),
        Some("workflow_id must not be empty")
    );

    let empty_steps = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/empty".to_owned(),
            title: "Empty".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: Vec::new(),
    };
    assert_eq!(
        empty_steps.validation_error().as_deref(),
        Some("workflows must contain at least one step")
    );

    let duplicate_step_ids = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/duplicate".to_owned(),
            title: "Duplicate".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Id {
                        id: "focus-id".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Title {
                        title: "Other".to_owned(),
                    },
                },
            },
        ],
    };
    assert_eq!(
        duplicate_step_ids.validation_error().as_deref(),
        Some("workflow step 1 reuses duplicate step_id resolve-focus")
    );

    let missing_reference = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/missing-ref".to_owned(),
            title: "Missing Ref".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![WorkflowStepSpec {
            step_id: "explore-focus".to_owned(),
            payload: WorkflowStepPayload::Explore {
                focus: WorkflowExploreFocus::ResolvedStep {
                    step_id: "resolve-focus".to_owned(),
                },
                lens: ExplorationLens::Refs,
                limit: 25,
                unique: false,
            },
        }],
    };
    assert_eq!(
        missing_reference.validation_error().as_deref(),
        Some("workflow step 0 is invalid: focus must reference an earlier resolve step")
    );

    let wrong_reference_kind = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/wrong-ref".to_owned(),
            title: "Wrong Ref".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Id {
                        id: "focus-id".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "save-focus".to_owned(),
                payload: WorkflowStepPayload::ArtifactSave {
                    source: WorkflowArtifactSaveSource::CompareStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    metadata: ExplorationArtifactMetadata {
                        artifact_id: "artifact/focus".to_owned(),
                        title: "Focus".to_owned(),
                        summary: None,
                    },
                    overwrite: true,
                },
            },
        ],
    };
    assert_eq!(
        wrong_reference_kind.validation_error().as_deref(),
        Some("workflow step 1 is invalid: source must reference a compare step, not resolve")
    );

    let same_compare_refs = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/same-compare".to_owned(),
            title: "Same Compare".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Id {
                        id: "focus-id".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "compare-focus".to_owned(),
                payload: WorkflowStepPayload::Compare {
                    left: WorkflowStepRef {
                        step_id: "resolve-focus".to_owned(),
                    },
                    right: WorkflowStepRef {
                        step_id: "resolve-focus".to_owned(),
                    },
                    group: NoteComparisonGroup::All,
                    limit: 25,
                },
            },
        ],
    };
    assert_eq!(
        same_compare_refs.validation_error().as_deref(),
        Some(
            "workflow step 1 is invalid: compare left and right must reference distinct resolve steps"
        )
    );

    let invalid_explore_unique = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/unique-refs".to_owned(),
            title: "Unique Refs".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![WorkflowStepSpec {
            step_id: "explore-focus".to_owned(),
            payload: WorkflowStepPayload::Explore {
                focus: WorkflowExploreFocus::NodeKey {
                    node_key: "heading:focus.org:3".to_owned(),
                },
                lens: ExplorationLens::Refs,
                limit: 25,
                unique: true,
            },
        }],
    };
    assert_eq!(
        invalid_explore_unique.validation_error().as_deref(),
        Some("workflow step 0 is invalid: explore unique is only supported for the structure lens")
    );
}

#[test]
fn built_in_workflows_are_valid_named_specs() {
    let workflows = built_in_workflows();
    assert_eq!(workflows.len(), 5);

    let ids: Vec<&str> = workflows
        .iter()
        .map(|workflow| workflow.metadata.workflow_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![
            BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID,
            BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID,
            BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID,
        ]
    );

    for workflow in &workflows {
        assert_eq!(workflow.validation_error(), None);
        assert!(
            !workflow.inputs.is_empty(),
            "built-in workflow {} should declare inputs",
            workflow.metadata.workflow_id
        );
    }
    assert_eq!(workflows[0].inputs[0].kind, WorkflowInputKind::FocusTarget);
    assert_eq!(workflows[1].inputs[0].kind, WorkflowInputKind::FocusTarget);
    assert_eq!(workflows[2].inputs[0].kind, WorkflowInputKind::FocusTarget);
    assert_eq!(workflows[3].inputs[0].kind, WorkflowInputKind::FocusTarget);
    assert_eq!(workflows[4].inputs[0].kind, WorkflowInputKind::NoteTarget);

    assert_eq!(
        built_in_workflow(BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID),
        Some(workflows[0].clone())
    );
    assert_eq!(built_in_workflow("workflow/builtin/missing"), None);

    let summaries = built_in_workflow_summaries();
    assert_eq!(summaries.len(), workflows.len());
    assert_eq!(summaries[0].step_count, workflows[0].steps.len());
    assert_eq!(
        summaries[4].metadata.workflow_id,
        BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID
    );
}

#[test]
fn built_in_workflows_round_trip_with_input_backed_resolve_targets() {
    let workflow = built_in_workflow(BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID)
        .expect("built-in unresolved sweep should exist");
    let serialized = serde_json::to_value(&workflow).expect("built-in workflow should serialize");
    assert_eq!(serialized["inputs"][0]["kind"], json!("focus-target"));
    assert_eq!(serialized["steps"][0]["kind"], json!("resolve"));
    assert_eq!(serialized["steps"][0]["target"]["kind"], json!("input"));
    assert_eq!(serialized["steps"][0]["target"]["input_id"], json!("focus"));
    assert_eq!(serialized["steps"][2]["kind"], json!("explore"));
    assert_eq!(serialized["steps"][2]["focus"]["kind"], json!("input"));
    assert_eq!(serialized["steps"][2]["focus"]["input_id"], json!("focus"));

    let round_trip: WorkflowSpec =
        serde_json::from_value(serialized).expect("built-in workflow should deserialize");
    assert_eq!(round_trip, workflow);

    let periodic = built_in_workflow(BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID)
        .expect("periodic review workflow should exist");
    assert_eq!(periodic.steps.len(), 6);
    assert_eq!(periodic.steps[1].step_id, "review-unresolved");
    assert_eq!(periodic.steps[4].step_id, "review-refs");
    assert_eq!(periodic.validation_error(), None);

    let weak = built_in_workflow(BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID)
        .expect("weak integration review workflow should exist");
    assert_eq!(weak.steps.len(), 4);
    assert_eq!(weak.steps[1].step_id, "review-weak-integration");
    assert_eq!(weak.validation_error(), None);
}

#[test]
fn workflow_specs_reject_invalid_inputs_and_missing_input_targets() {
    let duplicate_inputs = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/duplicate-inputs".to_owned(),
            title: "Duplicate Inputs".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![
            WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus".to_owned(),
                summary: None,
                kind: WorkflowInputKind::NoteTarget,
            },
            WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus Again".to_owned(),
                summary: None,
                kind: WorkflowInputKind::NoteTarget,
            },
        ],
        steps: vec![WorkflowStepSpec {
            step_id: "resolve-focus".to_owned(),
            payload: WorkflowStepPayload::Resolve {
                target: WorkflowResolveTarget::Input {
                    input_id: "focus".to_owned(),
                },
            },
        }],
    };
    assert_eq!(
        duplicate_inputs.validation_error().as_deref(),
        Some("workflow input 1 reuses duplicate input_id focus")
    );

    let missing_input_target = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/missing-input".to_owned(),
            title: "Missing Input".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![WorkflowStepSpec {
            step_id: "resolve-focus".to_owned(),
            payload: WorkflowStepPayload::Resolve {
                target: WorkflowResolveTarget::Input {
                    input_id: "focus".to_owned(),
                },
            },
        }],
    };
    assert_eq!(
        missing_input_target.validation_error().as_deref(),
        Some("workflow step 0 is invalid: target must reference a declared workflow input")
    );

    let missing_focus_input = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/missing-focus-input".to_owned(),
            title: "Missing Focus Input".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: Vec::new(),
        steps: vec![WorkflowStepSpec {
            step_id: "explore-focus".to_owned(),
            payload: WorkflowStepPayload::Explore {
                focus: WorkflowExploreFocus::Input {
                    input_id: "focus".to_owned(),
                },
                lens: ExplorationLens::Refs,
                limit: 25,
                unique: false,
            },
        }],
    };
    assert_eq!(
        missing_focus_input.validation_error().as_deref(),
        Some("workflow step 0 is invalid: focus must reference a declared workflow input")
    );

    let note_target_used_as_focus_input = WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: "workflow/note-focus-mismatch".to_owned(),
            title: "Note Focus Mismatch".to_owned(),
            summary: None,
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus".to_owned(),
            summary: None,
            kind: WorkflowInputKind::NoteTarget,
        }],
        steps: vec![WorkflowStepSpec {
            step_id: "explore-focus".to_owned(),
            payload: WorkflowStepPayload::Explore {
                focus: WorkflowExploreFocus::Input {
                    input_id: "focus".to_owned(),
                },
                lens: ExplorationLens::Refs,
                limit: 25,
                unique: false,
            },
        }],
    };
    assert_eq!(
        note_target_used_as_focus_input
            .validation_error()
            .as_deref(),
        Some("workflow step 0 is invalid: focus must reference a declared focus-target input")
    );
}

#[test]
fn saved_exploration_artifacts_reject_malformed_metadata_and_trails() {
    let blank_metadata = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: " ".to_owned(),
            title: "Title".to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "file:focus.org".to_owned(),
                current_node_key: "heading:focus.org:3".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 10,
                unique: false,
                frozen_context: false,
            }),
        },
    };

    assert_eq!(
        blank_metadata.validation_error().as_deref(),
        Some("artifact_id must not be empty")
    );

    let padded_metadata = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: " focus ".to_owned(),
            title: "Title".to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "file:focus.org".to_owned(),
                current_node_key: "heading:focus.org:3".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 10,
                unique: false,
                frozen_context: false,
            }),
        },
    };
    assert_eq!(
        padded_metadata.validation_error().as_deref(),
        Some("artifact_id must not have leading or trailing whitespace")
    );

    let empty_trail = SavedTrailArtifact {
        steps: Vec::new(),
        cursor: 0,
        detached_step: None,
    };
    assert_eq!(
        empty_trail.validation_error().as_deref(),
        Some("trail artifacts must contain at least one step")
    );

    let out_of_bounds_cursor = SavedTrailArtifact {
        steps: vec![SavedTrailStep::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "file:focus.org".to_owned(),
                current_node_key: "heading:focus.org:3".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 10,
                unique: false,
                frozen_context: false,
            }),
        }],
        cursor: 1,
        detached_step: None,
    };
    assert_eq!(
        out_of_bounds_cursor.validation_error().as_deref(),
        Some("trail cursor must point to an existing step")
    );

    let invalid_nested_step = SavedTrailArtifact {
        steps: vec![SavedTrailStep::Comparison {
            artifact: Box::new(SavedComparisonArtifact {
                root_node_key: "heading:focus.org:3".to_owned(),
                left_node_key: "heading:focus.org:3".to_owned(),
                right_node_key: "heading:focus.org:3".to_owned(),
                active_lens: ExplorationLens::Structure,
                structure_unique: false,
                comparison_group: NoteComparisonGroup::All,
                limit: 10,
                frozen_context: false,
            }),
        }],
        cursor: 0,
        detached_step: None,
    };
    assert_eq!(
        invalid_nested_step.validation_error().as_deref(),
        Some("trail step 0 is invalid: left_node_key and right_node_key must differ")
    );

    let attached_detached_step = SavedTrailArtifact {
        steps: vec![SavedTrailStep::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "heading:focus.org:3".to_owned(),
                current_node_key: "heading:focus.org:3".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 10,
                unique: false,
                frozen_context: false,
            }),
        }],
        cursor: 0,
        detached_step: Some(Box::new(SavedTrailStep::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "heading:focus.org:3".to_owned(),
                current_node_key: "heading:focus.org:3".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 10,
                unique: false,
                frozen_context: false,
            }),
        })),
    };

    assert_eq!(
        attached_detached_step.validation_error().as_deref(),
        Some("detached trail step must not duplicate any recorded trail step")
    );
}

#[test]
fn exploration_artifact_id_params_reject_padded_ids() {
    let padded = ExplorationArtifactIdParams {
        artifact_id: " lens/focus ".to_owned(),
    };
    assert_eq!(
        padded.validation_error().as_deref(),
        Some("artifact_id must not have leading or trailing whitespace")
    );
}

#[test]
fn search_nodes_params_support_kebab_case_sort_names() {
    let params: SearchNodesParams = serde_json::from_value(json!({
        "query": "alpha",
        "limit": 10,
        "sort": "forward-link-count"
    }))
    .expect("search node params should deserialize");

    assert_eq!(params.query, "alpha");
    assert_eq!(params.limit, 10);
    assert_eq!(params.sort, Some(SearchNodesSort::ForwardLinkCount));

    assert_eq!(
        serde_json::to_value(&params).expect("search node params should serialize"),
        json!({
            "query": "alpha",
            "limit": 10,
            "sort": "forward-link-count"
        })
    );
}

#[test]
fn search_nodes_params_default_to_unspecified_sort() {
    let params: SearchNodesParams =
        serde_json::from_value(json!({ "query": "alpha", "limit": 10 }))
            .expect("search node params should deserialize");

    assert_eq!(params.sort, None);
}

#[test]
fn node_from_title_or_alias_params_round_trip_without_scope() {
    let params: NodeFromTitleOrAliasParams =
        serde_json::from_value(json!({ "title_or_alias": "alpha", "nocase": true }))
            .expect("title-or-alias params should deserialize");

    assert_eq!(params.title_or_alias, "alpha");
    assert!(params.nocase);

    assert_eq!(
        serde_json::to_value(&params).expect("title-or-alias params should serialize"),
        json!({
            "title_or_alias": "alpha",
            "nocase": true
        })
    );
}

#[test]
fn node_from_key_params_round_trip() {
    let params: NodeFromKeyParams = serde_json::from_value(json!({ "node_key": "file:alpha.org" }))
        .expect("node-from-key params should deserialize");

    assert_eq!(params.node_key, "file:alpha.org");

    assert_eq!(
        serde_json::to_value(&params).expect("node-from-key params should serialize"),
        json!({
            "node_key": "file:alpha.org"
        })
    );
}

#[test]
fn unlinked_references_params_normalize_limit() {
    let params: UnlinkedReferencesParams =
        serde_json::from_value(json!({ "node_key": "heading:alpha.org:3", "limit": 0 }))
            .expect("unlinked reference params should deserialize");

    assert_eq!(params.node_key, "heading:alpha.org:3");
    assert_eq!(params.normalized_limit(), 1);

    assert_eq!(
        serde_json::to_value(&params).expect("unlinked reference params should serialize"),
        json!({
            "node_key": "heading:alpha.org:3",
            "limit": 0
        })
    );
}
