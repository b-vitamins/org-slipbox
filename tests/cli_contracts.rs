use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json::Value;
use slipbox_core::{
    BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID, BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
    BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID, CorpusAuditKind, ExplorationArtifactMetadata,
    ExplorationArtifactPayload, ExplorationLens, NodeKind, NodeRecord, ReportJsonlLineKind,
    ReportProfileMetadata, ReportProfileMode, ReportProfileSpec, ReportProfileSubject,
    ReviewFinding, ReviewFindingPayload, ReviewFindingStatus, ReviewRoutineComparePolicy,
    ReviewRoutineCompareTarget, ReviewRoutineMetadata, ReviewRoutineSaveReviewPolicy,
    ReviewRoutineSource, ReviewRoutineSpec, ReviewRun, ReviewRunDiffBucket, ReviewRunMetadata,
    ReviewRunPayload, SavedComparisonArtifact, SavedExplorationArtifact, SavedLensViewArtifact,
    WorkbenchPackCompatibility, WorkbenchPackManifest, WorkbenchPackMetadata, WorkflowMetadata,
    WorkflowSpec, WorkflowStepReport, WorkflowStepReportPayload, WorkflowSummary,
    built_in_workflow,
};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

mod support;

use support::{
    assert_anchor_record_keys, assert_error_failure, assert_exact_object_keys,
    assert_file_record_keys, assert_node_record_keys, assert_occurrence_record_keys, json_command,
    json_command_path, json_command_path_with_bad_server, run_slipbox, run_slipbox_with_stdin,
    scoped_server_args, slipbox_binary,
};

fn build_indexed_fixture() -> Result<(tempfile::TempDir, String, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("comparison.org"),
        r#"#+title: Comparison

* TODO Left
:PROPERTIES:
:ID: left-id
:ROAM_REFS: cite:shared2024 cite:sharedtwo2024 cite:left2024
:END:
SCHEDULED: <2026-05-01 Thu>
Links to [[id:shared-forward-id]] and [[id:left-right-bridge-id]].

* NEXT Right
:PROPERTIES:
:ID: right-id
:ROAM_REFS: cite:shared2024 cite:sharedtwo2024 cite:right2024
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
    )?;
    fs::write(
        root.join("context.org"),
        r#"#+title: Context

* TODO Dual Match Peer
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Matches both planning fields directly.

* NEXT Cross Match Peer
SCHEDULED: <2026-05-03 Sat>
DEADLINE: <2026-05-01 Thu>
Matches both planning dates through opposite fields.

* TODO Anonymous Focus
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
:PROPERTIES:
:ROAM_REFS: cite:shared2024
:END:
Anonymous anchor body.
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;
    let anonymous_anchor_key = database
        .anchors_in_file("context.org")?
        .into_iter()
        .find(|anchor| anchor.title == "Anonymous Focus")
        .map(|anchor| anchor.node_key)
        .expect("anonymous focus anchor should exist");

    let left_key = database
        .node_from_id("left-id")?
        .expect("left note should exist")
        .node_key;
    let right_key = database
        .node_from_id("right-id")?
        .expect("right note should exist")
        .node_key;

    let structure = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "artifact/structure".to_owned(),
            title: "Artifact Structure".to_owned(),
            summary: Some("Saved structure lens".to_owned()),
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: left_key.clone(),
                current_node_key: left_key.clone(),
                lens: ExplorationLens::Structure,
                limit: 25,
                unique: true,
                frozen_context: false,
            }),
        },
    };
    let comparison = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "artifact/comparison".to_owned(),
            title: "Artifact Comparison".to_owned(),
            summary: Some("Saved comparison state".to_owned()),
        },
        payload: ExplorationArtifactPayload::Comparison {
            artifact: Box::new(SavedComparisonArtifact {
                root_node_key: left_key.clone(),
                left_node_key: left_key,
                right_node_key: right_key,
                active_lens: ExplorationLens::Structure,
                structure_unique: false,
                comparison_group: slipbox_core::NoteComparisonGroup::Tension,
                limit: 10,
                frozen_context: false,
            }),
        },
    };
    database.save_exploration_artifact(&structure)?;
    database.save_exploration_artifact(&comparison)?;
    database.save_review_run(&contract_review_run(
        "review/workflow/base",
        "Review Base",
        ReviewFindingStatus::Open,
        "Focus",
    ))?;
    database.save_review_run(&contract_review_run(
        "review/workflow/target",
        "Review Target",
        ReviewFindingStatus::Reviewed,
        "Focus",
    ))?;

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
        anonymous_anchor_key,
    ))
}

fn contract_review_run(
    review_id: &str,
    title: &str,
    status: ReviewFindingStatus,
    node_title: &str,
) -> ReviewRun {
    ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: review_id.to_owned(),
            title: title.to_owned(),
            summary: Some("Contract review fixture".to_owned()),
        },
        payload: ReviewRunPayload::Workflow {
            workflow: WorkflowSummary {
                metadata: WorkflowMetadata {
                    workflow_id: "workflow/contract/review".to_owned(),
                    title: "Contract Review Workflow".to_owned(),
                    summary: Some("Review contract workflow".to_owned()),
                },
                step_count: 1,
            },
            inputs: Vec::new(),
            step_ids: vec!["resolve-focus".to_owned()],
        },
        findings: vec![ReviewFinding {
            finding_id: "workflow-step/resolve-focus".to_owned(),
            status,
            payload: ReviewFindingPayload::WorkflowStep {
                step: Box::new(WorkflowStepReport {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepReportPayload::Resolve {
                        node: Box::new(contract_review_node(node_title)),
                    },
                }),
            },
        }],
    }
}

fn contract_review_node(title: &str) -> NodeRecord {
    NodeRecord {
        node_key: "heading:contract.org:1".to_owned(),
        explicit_id: Some("contract-focus".to_owned()),
        file_path: "contract.org".to_owned(),
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

fn base_args(root: &str, db: &str) -> Vec<String> {
    scoped_server_args(root, db)
}

fn artifact_json_command(
    subcommand: &str,
    root: &str,
    db: &str,
    extra: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec!["artifact".to_owned(), subcommand.to_owned()];
    args.extend(base_args(root, db));
    args.push("--json".to_owned());
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

fn artifact_json_command_with_stdin(
    subcommand: &str,
    root: &str,
    db: &str,
    extra: &[&str],
    stdin: &[u8],
) -> Result<std::process::Output> {
    let mut args = vec!["artifact".to_owned(), subcommand.to_owned()];
    args.extend(base_args(root, db));
    args.push("--json".to_owned());
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox_with_stdin(&args, stdin)
}

fn review_json_command(
    subcommand: &str,
    root: &str,
    db: &str,
    extra: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec!["review".to_owned(), subcommand.to_owned()];
    args.extend(base_args(root, db));
    args.push("--json".to_owned());
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

fn workflow_json_command(
    subcommand: &str,
    root: &str,
    db: &str,
    extra: &[&str],
) -> Result<std::process::Output> {
    workflow_json_command_with_dirs(subcommand, root, db, &[], extra)
}

fn workflow_json_command_with_dirs(
    subcommand: &str,
    root: &str,
    db: &str,
    workflow_dirs: &[&Path],
    extra: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec!["workflow".to_owned(), subcommand.to_owned()];
    args.extend(base_args(root, db));
    for workflow_dir in workflow_dirs {
        args.push("--workflow-dir".to_owned());
        args.push(
            workflow_dir
                .to_str()
                .expect("workflow dir path should be valid utf-8")
                .to_owned(),
        );
    }
    args.push("--json".to_owned());
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

fn workflow_json_command_with_stdin(
    subcommand: &str,
    extra: &[&str],
    stdin: &[u8],
) -> Result<std::process::Output> {
    let mut args = vec![
        "workflow".to_owned(),
        subcommand.to_owned(),
        "--json".to_owned(),
    ];
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox_with_stdin(&args, stdin)
}

fn audit_json_command(
    subcommand: &str,
    root: &str,
    db: &str,
    extra: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec!["audit".to_owned(), subcommand.to_owned()];
    args.extend(base_args(root, db));
    args.push("--json".to_owned());
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

fn pack_json_command(
    subcommand: &str,
    root: &str,
    db: &str,
    extra: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec!["pack".to_owned(), subcommand.to_owned()];
    args.extend(base_args(root, db));
    args.push("--json".to_owned());
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

fn pack_json_command_with_stdin(
    subcommand: &str,
    root: &str,
    db: &str,
    extra: &[&str],
    stdin: &[u8],
) -> Result<std::process::Output> {
    let mut args = vec!["pack".to_owned(), subcommand.to_owned()];
    args.extend(base_args(root, db));
    args.push("--json".to_owned());
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox_with_stdin(&args, stdin)
}

fn pack_validate_json_command(input: &str) -> Result<std::process::Output> {
    run_slipbox(&[
        "pack".to_owned(),
        "validate".to_owned(),
        "--json".to_owned(),
        input.to_owned(),
    ])
}

fn pack_validate_json_command_with_stdin(stdin: &[u8]) -> Result<std::process::Output> {
    run_slipbox_with_stdin(
        &[
            "pack".to_owned(),
            "validate".to_owned(),
            "--json".to_owned(),
            "-".to_owned(),
        ],
        stdin,
    )
}

fn routine_json_command(
    subcommand: &str,
    root: &str,
    db: &str,
    extra: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec!["routine".to_owned(), subcommand.to_owned()];
    args.extend(base_args(root, db));
    args.push("--json".to_owned());
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

fn with_bad_server_program(
    mut args: Vec<String>,
    root: &str,
    db: &str,
    insert_at: usize,
) -> Vec<String> {
    let mut global = vec![
        "--root".to_owned(),
        root.to_owned(),
        "--db".to_owned(),
        db.to_owned(),
        "--server-program".to_owned(),
        "/definitely/not/a/real/slipbox-binary".to_owned(),
        "--json".to_owned(),
    ];
    args.splice(insert_at..insert_at, global.drain(..));
    args
}

fn assert_preview_node_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "node_key",
            "explicit_id",
            "file_path",
            "title",
            "outline_path",
            "aliases",
            "tags",
            "refs",
            "todo_keyword",
            "scheduled_for",
            "deadline_for",
            "closed_at",
            "level",
            "line",
            "kind",
        ],
    );
}

fn assert_saved_artifact_summary_keys(value: &Value) {
    assert_exact_object_keys(value, &["artifact_id", "title", "summary", "kind"]);
}

fn assert_review_summary_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "review_id",
            "title",
            "summary",
            "kind",
            "finding_count",
            "status_counts",
        ],
    );
    assert_exact_object_keys(
        &value["status_counts"],
        &["open", "reviewed", "dismissed", "accepted"],
    );
}

fn assert_workflow_review_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "review_id",
            "title",
            "summary",
            "kind",
            "workflow",
            "inputs",
            "step_ids",
            "findings",
        ],
    );
}

fn assert_audit_review_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "review_id",
            "title",
            "summary",
            "kind",
            "audit",
            "limit",
            "findings",
        ],
    );
}

fn assert_review_finding_keys(value: &Value, payload_key: &str) {
    assert_exact_object_keys(value, &["finding_id", "status", "kind", payload_key]);
}

fn assert_pack_summary_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "pack_id",
            "title",
            "summary",
            "compatibility",
            "workflow_count",
            "review_routine_count",
            "report_profile_count",
            "entrypoint_routine_ids",
        ],
    );
    assert_exact_object_keys(&value["compatibility"], &["version"]);
}

fn assert_pack_manifest_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "pack_id",
            "title",
            "summary",
            "compatibility",
            "workflows",
            "review_routines",
            "report_profiles",
            "entrypoint_routine_ids",
        ],
    );
    assert_exact_object_keys(&value["compatibility"], &["version"]);
}

fn assert_workbench_pack_issue_keys(value: &Value) {
    assert_exact_object_keys(value, &["kind", "asset_id", "message"]);
}

fn assert_workflow_catalog_issue_keys(value: &Value, expected_optional_keys: &[&str]) {
    let mut keys = vec!["path", "kind", "workflow_id", "message"];
    keys.extend_from_slice(expected_optional_keys);
    assert_exact_object_keys(value, &keys);
}

fn assert_review_routine_summary_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "routine_id",
            "title",
            "summary",
            "source_kind",
            "input_count",
            "report_profile_count",
        ],
    );
}

fn assert_review_routine_spec_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "routine_id",
            "title",
            "summary",
            "source",
            "inputs",
            "save_review",
            "compare",
            "report_profile_ids",
        ],
    );
    assert_exact_object_keys(
        &value["save_review"],
        &["enabled", "review_id", "title", "summary", "overwrite"],
    );
}

fn assert_report_profile_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "profile_id",
            "title",
            "summary",
            "subjects",
            "mode",
            "status_filters",
            "diff_buckets",
            "jsonl_line_kinds",
        ],
    );
}

fn assert_applied_report_profile_keys(value: &Value) {
    assert_exact_object_keys(value, &["profile", "lines"]);
    assert_report_profile_keys(&value["profile"]);
}

fn seed_duplicate_title_audit_fixture(root: &str, db: &str) -> Result<()> {
    fs::write(
        Path::new(root).join("duplicate-a.org"),
        r#":PROPERTIES:
:ID: dup-a-id
:END:
#+title: Shared Title
"#,
    )?;
    fs::write(
        Path::new(root).join("duplicate-b.org"),
        r#":PROPERTIES:
:ID: dup-b-id
:END:
#+title: shared title
"#,
    )?;
    let files = scan_root(Path::new(root))?;
    let mut database = Database::open(Path::new(db))?;
    database.sync_index(&files)?;
    Ok(())
}

fn parse_jsonl_values(bytes: &[u8]) -> Vec<Value> {
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("jsonl line should be valid JSON"))
        .collect()
}

fn discovered_workflow(workflow_id: &str, title: &str, summary: &str) -> WorkflowSpec {
    let mut workflow = built_in_workflow(BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID)
        .expect("built-in workflow should exist");
    workflow.metadata.workflow_id = workflow_id.to_owned();
    workflow.metadata.title = title.to_owned();
    workflow.metadata.summary = Some(summary.to_owned());
    workflow
}

fn contract_workbench_pack(
    pack_id: &str,
    workflow_id: &str,
    routine_id: &str,
    review_id: &str,
) -> WorkbenchPackManifest {
    let mut workflow =
        built_in_workflow(BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID).expect("built-in workflow exists");
    workflow.metadata.workflow_id = workflow_id.to_owned();
    workflow.metadata.title = "Contract Pack Workflow".to_owned();
    workflow.metadata.summary = Some("Contract pack workflow fixture.".to_owned());

    WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: pack_id.to_owned(),
            title: "Contract Pack".to_owned(),
            summary: Some("Contract pack fixture.".to_owned()),
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: vec![workflow],
        review_routines: vec![ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: routine_id.to_owned(),
                title: "Contract Duplicate Title Routine".to_owned(),
                summary: Some("Contract duplicate-title routine fixture.".to_owned()),
            },
            source: ReviewRoutineSource::Audit {
                audit: CorpusAuditKind::DuplicateTitles,
                limit: 20,
            },
            inputs: Vec::new(),
            save_review: ReviewRoutineSaveReviewPolicy {
                enabled: true,
                review_id: Some(review_id.to_owned()),
                title: Some("Contract Duplicate Title Review".to_owned()),
                summary: Some("Saved by contract routine.".to_owned()),
                overwrite: false,
            },
            compare: Some(ReviewRoutineComparePolicy {
                target: ReviewRoutineCompareTarget::LatestCompatibleReview,
                report_profile_id: Some("profile/contract/diff".to_owned()),
            }),
            report_profile_ids: vec!["profile/contract/detail".to_owned()],
        }],
        report_profiles: vec![
            ReportProfileSpec {
                metadata: ReportProfileMetadata {
                    profile_id: "profile/contract/detail".to_owned(),
                    title: "Contract Detail".to_owned(),
                    summary: Some("Routine and review report lines.".to_owned()),
                },
                subjects: vec![ReportProfileSubject::Routine],
                mode: ReportProfileMode::Detail,
                status_filters: None,
                diff_buckets: None,
                jsonl_line_kinds: Some(vec![
                    ReportJsonlLineKind::Routine,
                    ReportJsonlLineKind::Review,
                    ReportJsonlLineKind::Finding,
                ]),
            },
            ReportProfileSpec {
                metadata: ReportProfileMetadata {
                    profile_id: "profile/contract/diff".to_owned(),
                    title: "Contract Diff".to_owned(),
                    summary: Some("Routine diff report lines.".to_owned()),
                },
                subjects: vec![ReportProfileSubject::Diff],
                mode: ReportProfileMode::Detail,
                status_filters: Some(vec![ReviewFindingStatus::Open]),
                diff_buckets: Some(vec![ReviewRunDiffBucket::Unchanged]),
                jsonl_line_kinds: Some(vec![
                    ReportJsonlLineKind::Diff,
                    ReportJsonlLineKind::Unchanged,
                ]),
            },
        ],
        entrypoint_routine_ids: vec![routine_id.to_owned()],
    }
}

fn write_pack_manifest(pack: &WorkbenchPackManifest, path: &Path) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(pack)?)?;
    Ok(())
}

#[test]
fn headless_commands_expose_stable_json_shapes() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let status = json_command("status", &root, &db, &[])?;
    assert!(status.status.success(), "{status:?}");
    let status_json: Value = serde_json::from_slice(&status.stdout)?;
    assert_exact_object_keys(
        &status_json,
        &[
            "version",
            "root",
            "db",
            "files_indexed",
            "nodes_indexed",
            "links_indexed",
        ],
    );

    let resolve = json_command("resolve-node", &root, &db, &["--id", "left-id"])?;
    assert!(resolve.status.success(), "{resolve:?}");
    let resolve_json: Value = serde_json::from_slice(&resolve.stdout)?;
    assert_exact_object_keys(
        &resolve_json,
        &[
            "node_key",
            "explicit_id",
            "file_path",
            "title",
            "outline_path",
            "aliases",
            "tags",
            "refs",
            "todo_keyword",
            "scheduled_for",
            "deadline_for",
            "closed_at",
            "level",
            "line",
            "kind",
            "file_mtime_ns",
            "backlink_count",
            "forward_link_count",
        ],
    );

    let explore = json_command(
        "explore",
        &root,
        &db,
        &["--key", &anonymous_anchor_key, "--lens", "time"],
    )?;
    assert!(explore.status.success(), "{explore:?}");
    let explore_json: Value = serde_json::from_slice(&explore.stdout)?;
    assert_exact_object_keys(&explore_json, &["lens", "sections"]);

    let compare = json_command(
        "compare",
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--group",
            "tension",
        ],
    )?;
    assert!(compare.status.success(), "{compare:?}");
    let compare_json: Value = serde_json::from_slice(&compare.stdout)?;
    assert_exact_object_keys(&compare_json, &["left_note", "right_note", "sections"]);

    let list = artifact_json_command("list", &root, &db, &[])?;
    assert!(list.status.success(), "{list:?}");
    let list_json: Value = serde_json::from_slice(&list.stdout)?;
    assert_exact_object_keys(&list_json, &["artifacts"]);
    let first_summary = &list_json["artifacts"][0];
    assert_exact_object_keys(first_summary, &["artifact_id", "title", "summary", "kind"]);

    let show = artifact_json_command("show", &root, &db, &["artifact/structure"])?;
    assert!(show.status.success(), "{show:?}");
    let show_json: Value = serde_json::from_slice(&show.stdout)?;
    assert_exact_object_keys(&show_json, &["artifact"]);
    assert_exact_object_keys(
        &show_json["artifact"],
        &[
            "artifact_id",
            "title",
            "summary",
            "kind",
            "root_node_key",
            "current_node_key",
            "lens",
            "limit",
            "unique",
            "frozen_context",
        ],
    );

    let run = artifact_json_command("run", &root, &db, &["artifact/comparison"])?;
    assert!(run.status.success(), "{run:?}");
    let run_json: Value = serde_json::from_slice(&run.stdout)?;
    assert_exact_object_keys(&run_json, &["artifact"]);
    assert_exact_object_keys(
        &run_json["artifact"],
        &[
            "artifact_id",
            "title",
            "summary",
            "kind",
            "artifact",
            "root_note",
            "result",
        ],
    );

    let export = artifact_json_command("export", &root, &db, &["artifact/structure"])?;
    assert!(export.status.success(), "{export:?}");
    let export_json: Value = serde_json::from_slice(&export.stdout)?;
    assert_exact_object_keys(
        &export_json,
        &[
            "artifact_id",
            "title",
            "summary",
            "kind",
            "root_node_key",
            "current_node_key",
            "lens",
            "limit",
            "unique",
            "frozen_context",
        ],
    );

    let import_payload = serde_json::to_string(&SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "artifact/imported".to_owned(),
            title: "Imported Artifact".to_owned(),
            summary: Some("Imported via CLI".to_owned()),
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "file:comparison.org".to_owned(),
                current_node_key: "file:comparison.org".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 5,
                unique: false,
                frozen_context: false,
            }),
        },
    })?;
    let import_file = Path::new(&db).with_extension("import.json");
    fs::write(&import_file, import_payload)?;
    let import = artifact_json_command(
        "import",
        &root,
        &db,
        &[import_file.to_str().expect("utf-8 path")],
    )?;
    assert!(import.status.success(), "{import:?}");
    let import_json: Value = serde_json::from_slice(&import.stdout)?;
    assert_exact_object_keys(&import_json, &["artifact"]);
    assert_exact_object_keys(
        &import_json["artifact"],
        &["artifact_id", "title", "summary", "kind"],
    );

    let delete = artifact_json_command("delete", &root, &db, &["artifact/imported"])?;
    assert!(delete.status.success(), "{delete:?}");
    let delete_json: Value = serde_json::from_slice(&delete.stdout)?;
    assert_exact_object_keys(&delete_json, &["artifact_id"]);

    let explore_save = json_command(
        "explore",
        &root,
        &db,
        &[
            "--key",
            &anonymous_anchor_key,
            "--lens",
            "refs",
            "--save",
            "--artifact-id",
            "artifact/saved-explore",
            "--artifact-title",
            "Saved Explore",
        ],
    )?;
    assert!(explore_save.status.success(), "{explore_save:?}");
    let explore_save_json: Value = serde_json::from_slice(&explore_save.stdout)?;
    assert_exact_object_keys(&explore_save_json, &["result", "artifact"]);
    assert_exact_object_keys(
        &explore_save_json["artifact"],
        &["artifact_id", "title", "summary", "kind"],
    );

    let compare_save = json_command(
        "compare",
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--save",
            "--artifact-id",
            "artifact/saved-compare",
            "--artifact-title",
            "Saved Compare",
        ],
    )?;
    assert!(compare_save.status.success(), "{compare_save:?}");
    let compare_save_json: Value = serde_json::from_slice(&compare_save.stdout)?;
    assert_exact_object_keys(&compare_save_json, &["result", "artifact"]);
    assert_saved_artifact_summary_keys(&compare_save_json["artifact"]);

    let review_list = review_json_command("list", &root, &db, &[])?;
    assert!(review_list.status.success(), "{review_list:?}");
    let review_list_json: Value = serde_json::from_slice(&review_list.stdout)?;
    assert_exact_object_keys(&review_list_json, &["reviews"]);
    assert_review_summary_keys(&review_list_json["reviews"][0]);

    let review_show = review_json_command("show", &root, &db, &["review/workflow/base"])?;
    assert!(review_show.status.success(), "{review_show:?}");
    let review_show_json: Value = serde_json::from_slice(&review_show.stdout)?;
    assert_exact_object_keys(&review_show_json, &["review"]);
    assert_workflow_review_keys(&review_show_json["review"]);
    assert_exact_object_keys(
        &review_show_json["review"]["workflow"],
        &["workflow_id", "title", "summary", "step_count"],
    );
    assert_review_finding_keys(&review_show_json["review"]["findings"][0], "step");

    let review_diff = review_json_command(
        "diff",
        &root,
        &db,
        &["review/workflow/base", "review/workflow/target"],
    )?;
    assert!(review_diff.status.success(), "{review_diff:?}");
    let review_diff_json: Value = serde_json::from_slice(&review_diff.stdout)?;
    assert_exact_object_keys(&review_diff_json, &["diff"]);
    assert_exact_object_keys(
        &review_diff_json["diff"],
        &[
            "base_review",
            "target_review",
            "added",
            "removed",
            "unchanged",
            "content_changed",
            "status_changed",
        ],
    );
    assert_exact_object_keys(
        &review_diff_json["diff"]["status_changed"][0],
        &["finding_id", "from_status", "to_status", "base", "target"],
    );
    assert_review_summary_keys(&review_diff_json["diff"]["base_review"]);
    assert_review_summary_keys(&review_diff_json["diff"]["target_review"]);

    let review_mark = review_json_command(
        "mark",
        &root,
        &db,
        &[
            "review/workflow/base",
            "workflow-step/resolve-focus",
            "dismissed",
        ],
    )?;
    assert!(review_mark.status.success(), "{review_mark:?}");
    let review_mark_json: Value = serde_json::from_slice(&review_mark.stdout)?;
    assert_exact_object_keys(&review_mark_json, &["transition"]);
    assert_exact_object_keys(
        &review_mark_json["transition"],
        &["review_id", "finding_id", "from_status", "to_status"],
    );

    let review_delete = review_json_command("delete", &root, &db, &["review/workflow/target"])?;
    assert!(review_delete.status.success(), "{review_delete:?}");
    let review_delete_json: Value = serde_json::from_slice(&review_delete.stdout)?;
    assert_exact_object_keys(&review_delete_json, &["review_id"]);

    Ok(())
}

#[test]
fn everyday_cli_commands_expose_stable_json_shapes() -> Result<()> {
    let (workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    fs::write(
        Path::new(&root).join("everyday.org"),
        r#":PROPERTIES:
:ID: everyday-id
:END:
#+title: Everyday
#+filetags: :base:

Links to [[id:left-id][Left]].
* TODO Planned
SCHEDULED: <2026-05-13 Wed>
Body mentions durable phrase.
"#,
    )?;
    let files = scan_root(Path::new(&root))?;
    let mut database = Database::open(Path::new(&db))?;
    database.sync_index(&files)?;

    let sync_root = json_command_path(&["sync", "root"], &root, &db, &[])?;
    assert!(sync_root.status.success(), "{sync_root:?}");
    let sync_root_json: Value = serde_json::from_slice(&sync_root.stdout)?;
    assert_exact_object_keys(
        &sync_root_json,
        &["files_indexed", "nodes_indexed", "links_indexed"],
    );

    let sync_file = json_command_path(&["sync", "file"], &root, &db, &["everyday.org"])?;
    assert!(sync_file.status.success(), "{sync_file:?}");
    let sync_file_json: Value = serde_json::from_slice(&sync_file.stdout)?;
    assert_exact_object_keys(&sync_file_json, &["file_path"]);

    let file_list = json_command_path(&["file", "list"], &root, &db, &[])?;
    assert!(file_list.status.success(), "{file_list:?}");
    let file_list_json: Value = serde_json::from_slice(&file_list.stdout)?;
    assert_exact_object_keys(&file_list_json, &["files"]);

    let file_search = json_command_path(&["file", "search"], &root, &db, &["Everyday"])?;
    assert!(file_search.status.success(), "{file_search:?}");
    let file_search_json: Value = serde_json::from_slice(&file_search.stdout)?;
    assert_exact_object_keys(&file_search_json, &["files"]);
    assert_file_record_keys(&file_search_json["files"][0]);

    let node_show = json_command_path(&["node", "show"], &root, &db, &["--id", "everyday-id"])?;
    assert!(node_show.status.success(), "{node_show:?}");
    let node_show_json: Value = serde_json::from_slice(&node_show.stdout)?;
    assert_node_record_keys(&node_show_json);

    let node_search = json_command_path(&["node", "search"], &root, &db, &["Everyday"])?;
    assert!(node_search.status.success(), "{node_search:?}");
    let node_search_json: Value = serde_json::from_slice(&node_search.stdout)?;
    assert_exact_object_keys(&node_search_json, &["nodes"]);
    assert_node_record_keys(&node_search_json["nodes"][0]);

    let node_random = json_command_path(&["node", "random"], &root, &db, &[])?;
    assert!(node_random.status.success(), "{node_random:?}");
    let node_random_json: Value = serde_json::from_slice(&node_random.stdout)?;
    assert_exact_object_keys(&node_random_json, &["node"]);
    assert_node_record_keys(&node_random_json["node"]);

    let node_backlinks =
        json_command_path(&["node", "backlinks"], &root, &db, &["--id", "left-id"])?;
    assert!(node_backlinks.status.success(), "{node_backlinks:?}");
    let backlinks_json: Value = serde_json::from_slice(&node_backlinks.stdout)?;
    assert_exact_object_keys(&backlinks_json, &["backlinks"]);
    assert_exact_object_keys(
        &backlinks_json["backlinks"][0],
        &[
            "source_note",
            "source_anchor",
            "row",
            "col",
            "preview",
            "explanation",
        ],
    );
    assert_node_record_keys(&backlinks_json["backlinks"][0]["source_note"]);

    let node_forward = json_command_path(
        &["node", "forward-links"],
        &root,
        &db,
        &["--id", "everyday-id"],
    )?;
    assert!(node_forward.status.success(), "{node_forward:?}");
    let forward_json: Value = serde_json::from_slice(&node_forward.stdout)?;
    assert_exact_object_keys(&forward_json, &["forward_links"]);
    assert_exact_object_keys(
        &forward_json["forward_links"][0],
        &["destination_note", "row", "col", "preview", "explanation"],
    );
    assert_node_record_keys(&forward_json["forward_links"][0]["destination_note"]);

    let node_at_point = json_command_path(
        &["node", "at-point"],
        &root,
        &db,
        &["--file", "everyday.org", "--line", "10"],
    )?;
    assert!(node_at_point.status.success(), "{node_at_point:?}");
    let at_point_json: Value = serde_json::from_slice(&node_at_point.stdout)?;
    assert_anchor_record_keys(&at_point_json);

    let node_ensure_id = json_command_path(
        &["node", "ensure-id"],
        &root,
        &db,
        &["--key", &anonymous_anchor_key],
    )?;
    assert!(node_ensure_id.status.success(), "{node_ensure_id:?}");
    let ensure_id_json: Value = serde_json::from_slice(&node_ensure_id.stdout)?;
    assert_anchor_record_keys(&ensure_id_json);

    let metadata_show = json_command_path(
        &["node", "metadata", "show"],
        &root,
        &db,
        &["--id", "everyday-id"],
    )?;
    assert!(metadata_show.status.success(), "{metadata_show:?}");
    let metadata_show_json: Value = serde_json::from_slice(&metadata_show.stdout)?;
    assert_node_record_keys(&metadata_show_json);

    let alias_add = json_command_path(
        &["node", "alias", "add"],
        &root,
        &db,
        &["--id", "everyday-id", "Daily Alias"],
    )?;
    assert!(alias_add.status.success(), "{alias_add:?}");
    let alias_json: Value = serde_json::from_slice(&alias_add.stdout)?;
    assert_node_record_keys(&alias_json);

    let ref_set = json_command_path(
        &["node", "ref", "set"],
        &root,
        &db,
        &["--id", "everyday-id", "cite:everyday2026"],
    )?;
    assert!(ref_set.status.success(), "{ref_set:?}");
    let ref_set_json: Value = serde_json::from_slice(&ref_set.stdout)?;
    assert_node_record_keys(&ref_set_json);

    let tag_set = json_command_path(
        &["node", "tag", "set"],
        &root,
        &db,
        &["--id", "everyday-id", "scriptable"],
    )?;
    assert!(tag_set.status.success(), "{tag_set:?}");
    let tag_set_json: Value = serde_json::from_slice(&tag_set.stdout)?;
    assert_node_record_keys(&tag_set_json);

    let ref_search = json_command_path(&["ref", "search"], &root, &db, &["everyday"])?;
    assert!(ref_search.status.success(), "{ref_search:?}");
    let ref_search_json: Value = serde_json::from_slice(&ref_search.stdout)?;
    assert_exact_object_keys(&ref_search_json, &["refs"]);
    assert_exact_object_keys(&ref_search_json["refs"][0], &["reference", "node"]);
    assert_node_record_keys(&ref_search_json["refs"][0]["node"]);

    let ref_show = json_command_path(&["ref", "show"], &root, &db, &["cite:everyday2026"])?;
    assert!(ref_show.status.success(), "{ref_show:?}");
    let ref_show_json: Value = serde_json::from_slice(&ref_show.stdout)?;
    assert_node_record_keys(&ref_show_json);

    let tag_search = json_command_path(&["tag", "search"], &root, &db, &["scriptable"])?;
    assert!(tag_search.status.success(), "{tag_search:?}");
    let tag_search_json: Value = serde_json::from_slice(&tag_search.stdout)?;
    assert_exact_object_keys(&tag_search_json, &["tags"]);

    let occurrence_search =
        json_command_path(&["search", "occurrences"], &root, &db, &["durable phrase"])?;
    assert!(occurrence_search.status.success(), "{occurrence_search:?}");
    let occurrence_json: Value = serde_json::from_slice(&occurrence_search.stdout)?;
    assert_exact_object_keys(&occurrence_json, &["occurrences"]);
    assert_occurrence_record_keys(&occurrence_json["occurrences"][0]);
    assert_anchor_record_keys(&occurrence_json["occurrences"][0]["owning_anchor"]);

    let agenda_today = json_command_path(&["agenda", "today"], &root, &db, &[])?;
    assert!(agenda_today.status.success(), "{agenda_today:?}");
    let agenda_today_json: Value = serde_json::from_slice(&agenda_today.stdout)?;
    assert_exact_object_keys(&agenda_today_json, &["nodes"]);

    let agenda_date = json_command_path(&["agenda", "date"], &root, &db, &["2026-05-13"])?;
    assert!(agenda_date.status.success(), "{agenda_date:?}");
    let agenda_json: Value = serde_json::from_slice(&agenda_date.stdout)?;
    assert_exact_object_keys(&agenda_json, &["nodes"]);
    assert_anchor_record_keys(&agenda_json["nodes"][0]);

    let agenda_range = json_command_path(
        &["agenda", "range"],
        &root,
        &db,
        &["2026-05-12", "2026-05-14"],
    )?;
    assert!(agenda_range.status.success(), "{agenda_range:?}");
    let agenda_range_json: Value = serde_json::from_slice(&agenda_range.stdout)?;
    assert_exact_object_keys(&agenda_range_json, &["nodes"]);

    let graph_stdout = json_command_path(&["graph", "dot"], &root, &db, &[])?;
    assert!(graph_stdout.status.success(), "{graph_stdout:?}");
    let graph_json: Value = serde_json::from_slice(&graph_stdout.stdout)?;
    assert_exact_object_keys(&graph_json, &["dot"]);

    let graph_path = workspace.path().join("graph.dot");
    let graph_file = json_command_path(
        &["graph", "dot"],
        &root,
        &db,
        &["--output", graph_path.to_str().expect("utf-8 path")],
    )?;
    assert!(graph_file.status.success(), "{graph_file:?}");
    let graph_file_json: Value = serde_json::from_slice(&graph_file.stdout)?;
    assert_exact_object_keys(&graph_file_json, &["output_path", "format"]);

    let note_create = json_command_path(
        &["note", "create"],
        &root,
        &db,
        &[
            "--title",
            "Contract Note",
            "--file",
            "contract-note.org",
            "--ref",
            "cite:contractnote2026",
        ],
    )?;
    assert!(note_create.status.success(), "{note_create:?}");
    let note_create_json: Value = serde_json::from_slice(&note_create.stdout)?;
    assert_node_record_keys(&note_create_json);

    let note_ensure = json_command_path(
        &["note", "ensure-file"],
        &root,
        &db,
        &["--file", "ensured.org", "--title", "Ensured"],
    )?;
    assert!(note_ensure.status.success(), "{note_ensure:?}");
    let note_ensure_json: Value = serde_json::from_slice(&note_ensure.stdout)?;
    assert_node_record_keys(&note_ensure_json);

    let note_append = json_command_path(
        &["note", "append-heading"],
        &root,
        &db,
        &[
            "--file",
            "ensured.org",
            "--title",
            "Ensured",
            "--heading",
            "Child",
        ],
    )?;
    assert!(note_append.status.success(), "{note_append:?}");
    let note_append_json: Value = serde_json::from_slice(&note_append.stdout)?;
    assert_anchor_record_keys(&note_append_json);

    let note_append_to_node = json_command_path(
        &["note", "append-to-node"],
        &root,
        &db,
        &["--key", "file:ensured.org", "--heading", "Grandchild"],
    )?;
    assert!(
        note_append_to_node.status.success(),
        "{note_append_to_node:?}"
    );
    let note_append_to_node_json: Value = serde_json::from_slice(&note_append_to_node.stdout)?;
    assert_anchor_record_keys(&note_append_to_node_json);

    let note_append_outline = json_command_path(
        &["note", "append-outline"],
        &root,
        &db,
        &[
            "--file",
            "outline-contract.org",
            "--head",
            "#+title: Outline Contract\n",
            "--outline",
            "Inbox",
            "Review",
            "--heading",
            "Finding",
        ],
    )?;
    assert!(
        note_append_outline.status.success(),
        "{note_append_outline:?}"
    );
    let note_append_outline_json: Value = serde_json::from_slice(&note_append_outline.stdout)?;
    assert_anchor_record_keys(&note_append_outline_json);

    let capture_node = json_command_path(
        &["capture", "node"],
        &root,
        &db,
        &[
            "--title",
            "Captured Contract",
            "--file",
            "captured-contract.org",
        ],
    )?;
    assert!(capture_node.status.success(), "{capture_node:?}");
    let capture_node_json: Value = serde_json::from_slice(&capture_node.stdout)?;
    assert_node_record_keys(&capture_node_json);

    let capture_template = json_command_path(
        &["capture", "template"],
        &root,
        &db,
        &[
            "--file",
            "captured-template.org",
            "--title",
            "Captured Template",
            "--type",
            "plain",
            "--content",
            "Captured template body",
        ],
    )?;
    assert!(capture_template.status.success(), "{capture_template:?}");
    let capture_template_json: Value = serde_json::from_slice(&capture_template.stdout)?;
    assert_anchor_record_keys(&capture_template_json);

    let capture_preview = json_command_path(
        &["capture", "preview"],
        &root,
        &db,
        &[
            "--file",
            "preview-contract.org",
            "--title",
            "Preview Contract",
            "--type",
            "plain",
            "--content",
            "Preview body",
        ],
    )?;
    assert!(capture_preview.status.success(), "{capture_preview:?}");
    let capture_preview_json: Value = serde_json::from_slice(&capture_preview.stdout)?;
    assert_exact_object_keys(
        &capture_preview_json,
        &["file_path", "content", "preview_node"],
    );
    assert_preview_node_keys(&capture_preview_json["preview_node"]);

    let daily_ensure =
        json_command_path(&["daily", "ensure"], &root, &db, &["--date", "2026-05-13"])?;
    assert!(daily_ensure.status.success(), "{daily_ensure:?}");
    let daily_ensure_json: Value = serde_json::from_slice(&daily_ensure.stdout)?;
    assert_node_record_keys(&daily_ensure_json);

    let daily_show = json_command_path(&["daily", "show"], &root, &db, &["--date", "2026-05-13"])?;
    assert!(daily_show.status.success(), "{daily_show:?}");
    let daily_show_json: Value = serde_json::from_slice(&daily_show.stdout)?;
    assert_node_record_keys(&daily_show_json);

    let daily_append = json_command_path(
        &["daily", "append"],
        &root,
        &db,
        &["--date", "2026-05-13", "--heading", "Daily Contract"],
    )?;
    assert!(daily_append.status.success(), "{daily_append:?}");
    let daily_append_json: Value = serde_json::from_slice(&daily_append.stdout)?;
    assert_anchor_record_keys(&daily_append_json);

    Ok(())
}

#[test]
fn workflow_and_audit_commands_expose_stable_json_shapes() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let workflow_list = workflow_json_command("list", &root, &db, &[])?;
    assert!(workflow_list.status.success(), "{workflow_list:?}");
    let workflow_list_json: Value = serde_json::from_slice(&workflow_list.stdout)?;
    assert_exact_object_keys(&workflow_list_json, &["workflows", "issues"]);
    let first_workflow = &workflow_list_json["workflows"][0];
    assert_exact_object_keys(
        first_workflow,
        &["workflow_id", "title", "summary", "step_count"],
    );

    let workflow_show =
        workflow_json_command("show", &root, &db, &[BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID])?;
    assert!(workflow_show.status.success(), "{workflow_show:?}");
    let workflow_show_json: Value = serde_json::from_slice(&workflow_show.stdout)?;
    assert_exact_object_keys(&workflow_show_json, &["workflow"]);
    assert_exact_object_keys(
        &workflow_show_json["workflow"],
        &[
            "workflow_id",
            "title",
            "summary",
            "compatibility",
            "inputs",
            "steps",
        ],
    );
    assert_exact_object_keys(
        &workflow_show_json["workflow"]["compatibility"],
        &["version"],
    );
    assert_exact_object_keys(
        &workflow_show_json["workflow"]["inputs"][0],
        &["input_id", "title", "summary", "kind"],
    );
    assert_exact_object_keys(
        &workflow_show_json["workflow"]["steps"][0],
        &["step_id", "kind", "target"],
    );
    assert_exact_object_keys(
        &workflow_show_json["workflow"]["steps"][1],
        &["step_id", "kind", "focus", "lens", "limit", "unique"],
    );

    let workflow_run = workflow_json_command(
        "run",
        &root,
        &db,
        &[
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anonymous_anchor_key}"),
        ],
    )?;
    assert!(workflow_run.status.success(), "{workflow_run:?}");
    let workflow_run_json: Value = serde_json::from_slice(&workflow_run.stdout)?;
    assert_exact_object_keys(&workflow_run_json, &["result"]);
    assert_exact_object_keys(&workflow_run_json["result"], &["workflow", "steps"]);
    assert_exact_object_keys(
        &workflow_run_json["result"]["workflow"],
        &["workflow_id", "title", "summary", "step_count"],
    );
    assert_exact_object_keys(
        &workflow_run_json["result"]["steps"][0],
        &["step_id", "kind", "node"],
    );
    assert_exact_object_keys(
        &workflow_run_json["result"]["steps"][1],
        &["step_id", "kind", "focus_node_key", "result"],
    );

    fs::write(
        Path::new(&root).join("duplicate-a.org"),
        r#":PROPERTIES:
:ID: dup-a-id
:END:
#+title: Shared Title
"#,
    )?;
    fs::write(
        Path::new(&root).join("duplicate-b.org"),
        r#":PROPERTIES:
:ID: dup-b-id
:END:
#+title: shared title
"#,
    )?;
    let files = scan_root(Path::new(&root))?;
    let mut database = Database::open(Path::new(&db))?;
    database.sync_index(&files)?;

    let audit = audit_json_command("duplicate-titles", &root, &db, &[])?;
    assert!(audit.status.success(), "{audit:?}");
    let audit_json: Value = serde_json::from_slice(&audit.stdout)?;
    assert_exact_object_keys(&audit_json, &["audit", "entries"]);
    assert_exact_object_keys(&audit_json["entries"][0], &["kind", "record"]);
    assert_exact_object_keys(&audit_json["entries"][0]["record"], &["title", "notes"]);

    Ok(())
}

#[test]
fn pack_commands_expose_stable_json_shapes_and_round_trip_contracts() -> Result<()> {
    let (workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;
    let pack = contract_workbench_pack(
        "pack/contract",
        "workflow/pack/contract",
        "routine/pack/contract",
        "review/routine/contract/current",
    );
    let pack_path = workspace.path().join("contract-pack.json");
    write_pack_manifest(&pack, &pack_path)?;
    let pack_bytes = serde_json::to_vec_pretty(&pack)?;

    let validated = pack_validate_json_command(pack_path.to_str().expect("utf-8 path"))?;
    assert!(validated.status.success(), "{validated:?}");
    let validated_json: Value = serde_json::from_slice(&validated.stdout)?;
    assert_exact_object_keys(&validated_json, &["pack", "valid", "issues"]);
    assert_eq!(validated_json["valid"], true);
    assert_pack_summary_keys(&validated_json["pack"]);
    assert!(
        validated_json["issues"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );

    let validated_stdin = pack_validate_json_command_with_stdin(&pack_bytes)?;
    assert!(validated_stdin.status.success(), "{validated_stdin:?}");
    let validated_stdin_json: Value = serde_json::from_slice(&validated_stdin.stdout)?;
    assert_exact_object_keys(&validated_stdin_json, &["pack", "valid", "issues"]);
    assert_eq!(validated_stdin_json["pack"], validated_json["pack"]);

    let imported = pack_json_command(
        "import",
        &root,
        &db,
        &[pack_path.to_str().expect("utf-8 path")],
    )?;
    assert!(imported.status.success(), "{imported:?}");
    let imported_json: Value = serde_json::from_slice(&imported.stdout)?;
    assert_exact_object_keys(&imported_json, &["pack"]);
    assert_pack_summary_keys(&imported_json["pack"]);

    let listed = pack_json_command("list", &root, &db, &[])?;
    assert!(listed.status.success(), "{listed:?}");
    let listed_json: Value = serde_json::from_slice(&listed.stdout)?;
    assert_exact_object_keys(&listed_json, &["packs", "issues"]);
    assert_pack_summary_keys(&listed_json["packs"][0]);
    assert!(listed_json["issues"].as_array().is_some_and(Vec::is_empty));

    let shown = pack_json_command("show", &root, &db, &["pack/contract"])?;
    assert!(shown.status.success(), "{shown:?}");
    let shown_json: Value = serde_json::from_slice(&shown.stdout)?;
    assert_exact_object_keys(&shown_json, &["pack"]);
    assert_pack_manifest_keys(&shown_json["pack"]);
    assert_exact_object_keys(
        &shown_json["pack"]["workflows"][0],
        &[
            "workflow_id",
            "title",
            "summary",
            "compatibility",
            "inputs",
            "steps",
        ],
    );
    assert_review_routine_spec_keys(&shown_json["pack"]["review_routines"][0]);
    assert_exact_object_keys(
        &shown_json["pack"]["review_routines"][0]["source"],
        &["kind", "audit", "limit"],
    );
    assert_report_profile_keys(&shown_json["pack"]["report_profiles"][0]);

    let exported_stdout = pack_json_command("export", &root, &db, &["pack/contract"])?;
    assert!(exported_stdout.status.success(), "{exported_stdout:?}");
    let exported_stdout_json: Value = serde_json::from_slice(&exported_stdout.stdout)?;
    assert_pack_manifest_keys(&exported_stdout_json);
    assert_eq!(exported_stdout_json, shown_json["pack"]);

    let export_path = workspace.path().join("exported-pack.json");
    let exported_file = pack_json_command(
        "export",
        &root,
        &db,
        &[
            "pack/contract",
            "--output",
            export_path.to_str().expect("utf-8 path"),
        ],
    )?;
    assert!(exported_file.status.success(), "{exported_file:?}");
    let exported_file_json: Value = serde_json::from_slice(&exported_file.stdout)?;
    assert_exact_object_keys(&exported_file_json, &["pack", "output_path"]);
    assert_pack_summary_keys(&exported_file_json["pack"]);
    let written_pack_json: Value = serde_json::from_slice(&fs::read(&export_path)?)?;
    assert_eq!(written_pack_json, shown_json["pack"]);

    let deleted = pack_json_command("delete", &root, &db, &["pack/contract"])?;
    assert!(deleted.status.success(), "{deleted:?}");
    let deleted_json: Value = serde_json::from_slice(&deleted.stdout)?;
    assert_exact_object_keys(&deleted_json, &["pack_id"]);

    let imported_stdin =
        pack_json_command_with_stdin("import", &root, &db, &["-"], &exported_stdout.stdout)?;
    assert!(imported_stdin.status.success(), "{imported_stdin:?}");
    let imported_stdin_json: Value = serde_json::from_slice(&imported_stdin.stdout)?;
    assert_exact_object_keys(&imported_stdin_json, &["pack"]);
    assert_pack_summary_keys(&imported_stdin_json["pack"]);

    let reopened_show = pack_json_command("show", &root, &db, &["pack/contract"])?;
    assert!(reopened_show.status.success(), "{reopened_show:?}");
    let reopened_show_json: Value = serde_json::from_slice(&reopened_show.stdout)?;
    assert_eq!(reopened_show_json["pack"], shown_json["pack"]);

    Ok(())
}

#[test]
fn routine_commands_expose_stable_json_shapes_and_review_round_trips() -> Result<()> {
    let (workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;
    seed_duplicate_title_audit_fixture(&root, &db)?;

    let base = audit_json_command(
        "duplicate-titles",
        &root,
        &db,
        &[
            "--limit",
            "20",
            "--save-review",
            "--review-id",
            "review/routine/contracts/base",
            "--review-title",
            "Base Routine Contract Review",
        ],
    )?;
    assert!(base.status.success(), "{base:?}");

    let pack = contract_workbench_pack(
        "pack/routine-contract",
        "workflow/pack/routine-contract",
        "routine/pack/routine-contract",
        "review/routine/contracts/current",
    );
    let pack_path = workspace.path().join("routine-contract-pack.json");
    write_pack_manifest(&pack, &pack_path)?;
    let imported = pack_json_command(
        "import",
        &root,
        &db,
        &[pack_path.to_str().expect("utf-8 path")],
    )?;
    assert!(imported.status.success(), "{imported:?}");

    let listed = routine_json_command("list", &root, &db, &[])?;
    assert!(listed.status.success(), "{listed:?}");
    let listed_json: Value = serde_json::from_slice(&listed.stdout)?;
    assert_exact_object_keys(&listed_json, &["routines", "issues"]);
    let routines = listed_json["routines"]
        .as_array()
        .expect("routine list should be an array");
    let contract_routine = routines
        .iter()
        .find(|routine| routine["routine_id"] == "routine/pack/routine-contract")
        .expect("imported routine should be listed");
    assert_review_routine_summary_keys(contract_routine);
    assert!(listed_json["issues"].as_array().is_some_and(Vec::is_empty));

    let shown = routine_json_command("show", &root, &db, &["routine/pack/routine-contract"])?;
    assert!(shown.status.success(), "{shown:?}");
    let shown_json: Value = serde_json::from_slice(&shown.stdout)?;
    assert_exact_object_keys(&shown_json, &["routine"]);
    assert_review_routine_spec_keys(&shown_json["routine"]);
    assert_exact_object_keys(
        &shown_json["routine"]["source"],
        &["kind", "audit", "limit"],
    );

    let built_in = routine_json_command(
        "show",
        &root,
        &db,
        &[BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID],
    )?;
    assert!(built_in.status.success(), "{built_in:?}");
    let built_in_json: Value = serde_json::from_slice(&built_in.stdout)?;
    assert_review_routine_spec_keys(&built_in_json["routine"]);
    assert_exact_object_keys(
        &built_in_json["routine"]["source"],
        &["kind", "workflow_id"],
    );
    assert_exact_object_keys(
        &built_in_json["routine"]["inputs"][0],
        &["input_id", "title", "summary", "kind"],
    );

    let run = routine_json_command("run", &root, &db, &["routine/pack/routine-contract"])?;
    assert!(run.status.success(), "{run:?}");
    let run_json: Value = serde_json::from_slice(&run.stdout)?;
    assert_exact_object_keys(&run_json, &["result"]);
    assert_exact_object_keys(
        &run_json["result"],
        &["routine", "source", "saved_review", "compare", "reports"],
    );
    assert_review_routine_summary_keys(&run_json["result"]["routine"]);
    assert_exact_object_keys(&run_json["result"]["source"], &["kind", "result"]);
    assert_exact_object_keys(
        &run_json["result"]["source"]["result"],
        &["audit", "entries"],
    );
    assert_review_summary_keys(&run_json["result"]["saved_review"]);
    assert_exact_object_keys(
        &run_json["result"]["compare"],
        &["target", "base_review", "diff", "report"],
    );
    assert_review_summary_keys(&run_json["result"]["compare"]["base_review"]);
    assert_exact_object_keys(
        &run_json["result"]["compare"]["diff"],
        &[
            "base_review",
            "target_review",
            "added",
            "removed",
            "unchanged",
            "content_changed",
            "status_changed",
        ],
    );
    assert_applied_report_profile_keys(&run_json["result"]["compare"]["report"]);
    assert_applied_report_profile_keys(&run_json["result"]["reports"][0]);
    assert_exact_object_keys(
        &run_json["result"]["reports"][0]["lines"][0],
        &["kind", "routine"],
    );

    let shown_review =
        review_json_command("show", &root, &db, &["review/routine/contracts/current"])?;
    assert!(shown_review.status.success(), "{shown_review:?}");
    let shown_review_json: Value = serde_json::from_slice(&shown_review.stdout)?;
    assert_audit_review_keys(&shown_review_json["review"]);
    assert_review_finding_keys(&shown_review_json["review"]["findings"][0], "entry");

    let diff = review_json_command(
        "diff",
        &root,
        &db,
        &[
            "review/routine/contracts/base",
            "review/routine/contracts/current",
        ],
    )?;
    assert!(diff.status.success(), "{diff:?}");
    let diff_json: Value = serde_json::from_slice(&diff.stdout)?;
    assert_exact_object_keys(&diff_json, &["diff"]);
    assert_eq!(
        diff_json["diff"]["unchanged"].as_array().map(Vec::len),
        Some(1)
    );

    let duplicate = contract_workbench_pack(
        "pack/routine-contract-duplicate",
        "workflow/pack/routine-contract-duplicate",
        "routine/pack/routine-contract",
        "review/routine/contracts/duplicate",
    );
    let duplicate_path = workspace
        .path()
        .join("routine-contract-duplicate-pack.json");
    write_pack_manifest(&duplicate, &duplicate_path)?;
    let imported_duplicate = pack_json_command(
        "import",
        &root,
        &db,
        &[duplicate_path.to_str().expect("utf-8 path")],
    )?;
    assert!(
        imported_duplicate.status.success(),
        "{imported_duplicate:?}"
    );

    let listed_with_issue = routine_json_command("list", &root, &db, &[])?;
    assert!(listed_with_issue.status.success(), "{listed_with_issue:?}");
    let listed_with_issue_json: Value = serde_json::from_slice(&listed_with_issue.stdout)?;
    let routine_issue = listed_with_issue_json["issues"]
        .as_array()
        .expect("issues should be an array")
        .iter()
        .find(|issue| issue["kind"] == "duplicate-review-routine-id")
        .expect("duplicate routine issue should be reported");
    assert_workflow_catalog_issue_keys(routine_issue, &["pack_id", "routine_id"]);

    Ok(())
}

#[test]
fn workflow_discovery_and_report_outputs_expose_stable_json_shapes() -> Result<()> {
    let (workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;
    let workflow_dir = workspace.path().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    let valid = discovered_workflow(
        "workflow/test/discovered-unresolved",
        "Discovered Unresolved Sweep",
        "Discovered workflow fixture.",
    );
    fs::write(
        workflow_dir.join("valid.json"),
        serde_json::to_vec_pretty(&valid)?,
    )?;

    let mut invalid = discovered_workflow(
        "workflow/test/invalid-workflow",
        "Invalid Workflow",
        "Invalid workflow fixture.",
    );
    invalid.steps.clear();
    fs::write(
        workflow_dir.join("invalid.json"),
        serde_json::to_vec_pretty(&invalid)?,
    )?;

    let listed =
        workflow_json_command_with_dirs("list", &root, &db, &[workflow_dir.as_path()], &[])?;
    assert!(listed.status.success(), "{listed:?}");
    let listed_json: Value = serde_json::from_slice(&listed.stdout)?;
    assert_exact_object_keys(&listed_json, &["workflows", "issues"]);
    assert_exact_object_keys(
        &listed_json["issues"][0],
        &["path", "kind", "workflow_id", "message"],
    );

    let workflow_report_path = workspace.path().join("workflow-report.jsonl");
    let workflow_report = run_slipbox(&[
        "workflow".to_owned(),
        "run".to_owned(),
        "--root".to_owned(),
        root.clone(),
        "--db".to_owned(),
        db.clone(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--jsonl".to_owned(),
        "--output".to_owned(),
        workflow_report_path
            .to_str()
            .expect("utf-8 path")
            .to_owned(),
        BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID.to_owned(),
        "--input".to_owned(),
        format!("focus=key:{anonymous_anchor_key}"),
    ])?;
    assert!(workflow_report.status.success(), "{workflow_report:?}");
    let workflow_report_json: Value = serde_json::from_slice(&workflow_report.stdout)?;
    assert_exact_object_keys(
        &workflow_report_json,
        &["workflow", "format", "output_path", "step_count"],
    );
    let workflow_lines = parse_jsonl_values(&fs::read(&workflow_report_path)?);
    assert_exact_object_keys(&workflow_lines[0], &["kind", "workflow"]);
    assert_exact_object_keys(&workflow_lines[1], &["kind", "step"]);

    let audit_report_path = workspace.path().join("audit-report.json");
    let audit_report = audit_json_command(
        "duplicate-titles",
        &root,
        &db,
        &["--output", audit_report_path.to_str().expect("utf-8 path")],
    )?;
    assert!(audit_report.status.success(), "{audit_report:?}");
    let audit_report_json: Value = serde_json::from_slice(&audit_report.stdout)?;
    assert_exact_object_keys(
        &audit_report_json,
        &["audit", "format", "output_path", "entry_count"],
    );
    let written_audit: Value = serde_json::from_slice(&fs::read(&audit_report_path)?)?;
    assert_exact_object_keys(&written_audit, &["audit", "entries"]);

    fs::write(
        Path::new(&root).join("duplicate-a.org"),
        r#":PROPERTIES:
:ID: dup-a-id
:END:
#+title: Shared Title
"#,
    )?;
    fs::write(
        Path::new(&root).join("duplicate-b.org"),
        r#":PROPERTIES:
:ID: dup-b-id
:END:
#+title: shared title
"#,
    )?;
    let files = scan_root(Path::new(&root))?;
    let mut database = Database::open(Path::new(&db))?;
    database.sync_index(&files)?;

    let audit_jsonl = run_slipbox(&[
        "audit".to_owned(),
        "duplicate-titles".to_owned(),
        "--root".to_owned(),
        root.clone(),
        "--db".to_owned(),
        db.clone(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--jsonl".to_owned(),
    ])?;
    assert!(audit_jsonl.status.success(), "{audit_jsonl:?}");
    let audit_lines = parse_jsonl_values(&audit_jsonl.stdout);
    assert_exact_object_keys(&audit_lines[0], &["kind", "audit"]);
    assert_exact_object_keys(&audit_lines[1], &["kind", "entry"]);

    Ok(())
}

#[test]
fn save_review_commands_expose_stable_json_shapes() -> Result<()> {
    let (workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;
    seed_duplicate_title_audit_fixture(&root, &db)?;

    let audit_save = audit_json_command(
        "duplicate-titles",
        &root,
        &db,
        &[
            "--save-review",
            "--review-id",
            "review/audit/contracts/duplicates",
            "--review-title",
            "Duplicate Contract Review",
            "--review-summary",
            "Contract-level duplicate-title review.",
        ],
    )?;
    assert!(audit_save.status.success(), "{audit_save:?}");
    let audit_save_json: Value = serde_json::from_slice(&audit_save.stdout)?;
    assert_exact_object_keys(&audit_save_json, &["result", "review"]);
    assert_exact_object_keys(&audit_save_json["result"], &["audit", "entries"]);
    assert_review_summary_keys(&audit_save_json["review"]);
    assert_eq!(
        audit_save_json["review"]["review_id"],
        "review/audit/contracts/duplicates"
    );

    let audit_show =
        review_json_command("show", &root, &db, &["review/audit/contracts/duplicates"])?;
    assert!(audit_show.status.success(), "{audit_show:?}");
    let audit_show_json: Value = serde_json::from_slice(&audit_show.stdout)?;
    assert_exact_object_keys(&audit_show_json, &["review"]);
    assert_audit_review_keys(&audit_show_json["review"]);
    assert_review_finding_keys(&audit_show_json["review"]["findings"][0], "entry");
    assert_exact_object_keys(
        &audit_show_json["review"]["findings"][0]["entry"],
        &["kind", "record"],
    );

    let workflow_save = workflow_json_command(
        "run",
        &root,
        &db,
        &[
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anonymous_anchor_key}"),
            "--save-review",
            "--review-id",
            "review/workflow/contracts/unresolved",
            "--review-title",
            "Unresolved Contract Review",
        ],
    )?;
    assert!(workflow_save.status.success(), "{workflow_save:?}");
    let workflow_save_json: Value = serde_json::from_slice(&workflow_save.stdout)?;
    assert_exact_object_keys(&workflow_save_json, &["result", "review"]);
    assert_exact_object_keys(&workflow_save_json["result"], &["workflow", "steps"]);
    assert_review_summary_keys(&workflow_save_json["review"]);
    assert_eq!(
        workflow_save_json["review"]["review_id"],
        "review/workflow/contracts/unresolved"
    );

    let workflow_report_path = workspace.path().join("workflow-save-review.jsonl");
    let workflow_report = run_slipbox(&[
        "workflow".to_owned(),
        "run".to_owned(),
        "--root".to_owned(),
        root.clone(),
        "--db".to_owned(),
        db.clone(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--jsonl".to_owned(),
        "--output".to_owned(),
        workflow_report_path
            .to_str()
            .expect("utf-8 path")
            .to_owned(),
        BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID.to_owned(),
        "--input".to_owned(),
        format!("focus=key:{anonymous_anchor_key}"),
        "--save-review".to_owned(),
        "--review-id".to_owned(),
        "review/workflow/contracts/report".to_owned(),
    ])?;
    assert!(workflow_report.status.success(), "{workflow_report:?}");
    let workflow_report_json: Value = serde_json::from_slice(&workflow_report.stdout)?;
    assert_exact_object_keys(
        &workflow_report_json,
        &["workflow", "format", "output_path", "step_count", "review"],
    );
    assert_review_summary_keys(&workflow_report_json["review"]);
    let workflow_lines = parse_jsonl_values(&fs::read(&workflow_report_path)?);
    assert_exact_object_keys(&workflow_lines[0], &["kind", "workflow"]);
    assert_exact_object_keys(&workflow_lines[1], &["kind", "step"]);

    Ok(())
}

#[test]
fn review_save_diff_mark_round_trip_through_binary_boundary() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;
    seed_duplicate_title_audit_fixture(&root, &db)?;

    let base = audit_json_command(
        "duplicate-titles",
        &root,
        &db,
        &[
            "--save-review",
            "--review-id",
            "review/audit/contracts/base",
            "--review-title",
            "Base Duplicate Review",
        ],
    )?;
    assert!(base.status.success(), "{base:?}");
    let base_json: Value = serde_json::from_slice(&base.stdout)?;
    assert_eq!(base_json["review"]["finding_count"], 1);

    let target = audit_json_command(
        "duplicate-titles",
        &root,
        &db,
        &[
            "--save-review",
            "--review-id",
            "review/audit/contracts/target",
            "--review-title",
            "Target Duplicate Review",
        ],
    )?;
    assert!(target.status.success(), "{target:?}");

    let shown = review_json_command("show", &root, &db, &["review/audit/contracts/target"])?;
    assert!(shown.status.success(), "{shown:?}");
    let shown_json: Value = serde_json::from_slice(&shown.stdout)?;
    assert_audit_review_keys(&shown_json["review"]);
    let finding_id = shown_json["review"]["findings"][0]["finding_id"]
        .as_str()
        .expect("finding id should be a string")
        .to_owned();

    let diff = review_json_command(
        "diff",
        &root,
        &db,
        &[
            "review/audit/contracts/base",
            "review/audit/contracts/target",
        ],
    )?;
    assert!(diff.status.success(), "{diff:?}");
    let diff_json: Value = serde_json::from_slice(&diff.stdout)?;
    assert_exact_object_keys(&diff_json, &["diff"]);
    assert_eq!(
        diff_json["diff"]["unchanged"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(diff_json["diff"]["added"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        diff_json["diff"]["status_changed"].as_array().map(Vec::len),
        Some(0)
    );

    let mark = review_json_command(
        "mark",
        &root,
        &db,
        &["review/audit/contracts/target", &finding_id, "reviewed"],
    )?;
    assert!(mark.status.success(), "{mark:?}");
    let mark_json: Value = serde_json::from_slice(&mark.stdout)?;
    assert_exact_object_keys(&mark_json, &["transition"]);
    assert_eq!(mark_json["transition"]["from_status"], "open");
    assert_eq!(mark_json["transition"]["to_status"], "reviewed");

    let reopened = review_json_command("show", &root, &db, &["review/audit/contracts/target"])?;
    assert!(reopened.status.success(), "{reopened:?}");
    let reopened_json: Value = serde_json::from_slice(&reopened.stdout)?;
    assert_eq!(reopened_json["review"]["findings"][0]["status"], "reviewed");
    assert_eq!(
        reopened_json["review"]["findings"][0]["finding_id"],
        finding_id
    );

    Ok(())
}

#[test]
fn workflow_show_json_round_trips_into_local_spec_inspection() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;

    let shown = workflow_json_command("show", &root, &db, &[BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID])?;
    assert!(shown.status.success(), "{shown:?}");
    let shown_json: Value = serde_json::from_slice(&shown.stdout)?;
    assert_exact_object_keys(&shown_json, &["workflow"]);

    let workflow_bytes = serde_json::to_vec_pretty(&shown_json["workflow"])?;
    let local = workflow_json_command_with_stdin("show", &["--spec", "-"], &workflow_bytes)?;
    assert!(local.status.success(), "{local:?}");
    let local_json: Value = serde_json::from_slice(&local.stdout)?;
    assert_eq!(local_json, shown_json);

    Ok(())
}

#[test]
fn headless_commands_report_structured_daemon_failures() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;
    let import_payload = serde_json::to_string(&SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "artifact/importable".to_owned(),
            title: "Importable".to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "file:comparison.org".to_owned(),
                current_node_key: "file:comparison.org".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 5,
                unique: false,
                frozen_context: false,
            }),
        },
    })?;
    let import_file = Path::new(&db).with_extension("daemon-failure-import.json");
    fs::write(&import_file, import_payload)?;

    let command_sets = vec![
        with_bad_server_program(vec!["status".to_owned()], &root, &db, 1),
        with_bad_server_program(
            vec![
                "resolve-node".to_owned(),
                "--id".to_owned(),
                "left-id".to_owned(),
            ],
            &root,
            &db,
            1,
        ),
        with_bad_server_program(
            vec![
                "explore".to_owned(),
                "--id".to_owned(),
                "left-id".to_owned(),
                "--lens".to_owned(),
                "structure".to_owned(),
            ],
            &root,
            &db,
            1,
        ),
        with_bad_server_program(
            vec![
                "compare".to_owned(),
                "--left-id".to_owned(),
                "left-id".to_owned(),
                "--right-id".to_owned(),
                "right-id".to_owned(),
            ],
            &root,
            &db,
            1,
        ),
        with_bad_server_program(
            vec![
                "explore".to_owned(),
                "--key".to_owned(),
                anonymous_anchor_key.clone(),
                "--lens".to_owned(),
                "tasks".to_owned(),
                "--save".to_owned(),
                "--artifact-id".to_owned(),
                "artifact/daemon-save-explore".to_owned(),
                "--artifact-title".to_owned(),
                "Daemon Save Explore".to_owned(),
            ],
            &root,
            &db,
            1,
        ),
        with_bad_server_program(
            vec![
                "compare".to_owned(),
                "--left-id".to_owned(),
                "left-id".to_owned(),
                "--right-id".to_owned(),
                "right-id".to_owned(),
                "--save".to_owned(),
                "--artifact-id".to_owned(),
                "artifact/daemon-save-compare".to_owned(),
                "--artifact-title".to_owned(),
                "Daemon Save Compare".to_owned(),
            ],
            &root,
            &db,
            1,
        ),
        with_bad_server_program(vec!["sync".to_owned(), "root".to_owned()], &root, &db, 2),
        with_bad_server_program(
            vec![
                "sync".to_owned(),
                "file".to_owned(),
                "comparison.org".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(vec!["file".to_owned(), "list".to_owned()], &root, &db, 2),
        with_bad_server_program(
            vec![
                "node".to_owned(),
                "ensure-id".to_owned(),
                "--id".to_owned(),
                "left-id".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "node".to_owned(),
                "alias".to_owned(),
                "add".to_owned(),
                "--id".to_owned(),
                "left-id".to_owned(),
                "Alias".to_owned(),
            ],
            &root,
            &db,
            3,
        ),
        with_bad_server_program(
            vec![
                "note".to_owned(),
                "create".to_owned(),
                "--title".to_owned(),
                "Bad Daemon Note".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "capture".to_owned(),
                "node".to_owned(),
                "--title".to_owned(),
                "Bad Daemon Capture".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "daily".to_owned(),
                "ensure".to_owned(),
                "--date".to_owned(),
                "2026-05-13".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec!["artifact".to_owned(), "list".to_owned()],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "artifact".to_owned(),
                "show".to_owned(),
                "artifact/structure".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "artifact".to_owned(),
                "run".to_owned(),
                "artifact/structure".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "artifact".to_owned(),
                "export".to_owned(),
                "artifact/structure".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "artifact".to_owned(),
                "delete".to_owned(),
                "artifact/structure".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "artifact".to_owned(),
                "import".to_owned(),
                import_file.to_str().expect("utf-8 path").to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(vec!["review".to_owned(), "list".to_owned()], &root, &db, 2),
        with_bad_server_program(
            vec![
                "review".to_owned(),
                "show".to_owned(),
                "review/workflow/base".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "review".to_owned(),
                "diff".to_owned(),
                "review/workflow/base".to_owned(),
                "review/workflow/target".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "review".to_owned(),
                "mark".to_owned(),
                "review/workflow/base".to_owned(),
                "workflow-step/resolve-focus".to_owned(),
                "dismissed".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
        with_bad_server_program(
            vec![
                "review".to_owned(),
                "delete".to_owned(),
                "review/workflow/base".to_owned(),
            ],
            &root,
            &db,
            2,
        ),
    ];

    for command in command_sets {
        let output = run_slipbox(&command)?;
        assert_error_failure(&output, "failed to start slipbox daemon");
    }

    Ok(())
}

#[test]
fn everyday_cli_commands_report_structured_json_failures() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;
    seed_duplicate_title_audit_fixture(&root, &db)?;
    fs::write(
        Path::new(&root).join("duplicate-c.org"),
        r#":PROPERTIES:
:ID: dup-c-id
:END:
#+title: Shared Title
"#,
    )?;
    let files = scan_root(Path::new(&root))?;
    let mut database = Database::open(Path::new(&db))?;
    database.sync_index(&files)?;

    let duplicate_show =
        json_command_path(&["node", "show"], &root, &db, &["--title", "Shared Title"])?;
    assert_error_failure(&duplicate_show, "multiple nodes match Shared Title");

    let missing_target = json_command_path(
        &["note", "append-to-node"],
        &root,
        &db,
        &["--id", "missing-id", "--heading", "Missing"],
    )?;
    assert_error_failure(&missing_target, "unknown node id: missing-id");

    let invalid_daily_date =
        json_command_path(&["daily", "ensure"], &root, &db, &["--date", "05/13/2026"])?;
    assert_error_failure(&invalid_daily_date, "invalid daily date \"05/13/2026\"");

    let invalid_daily_path = json_command_path(
        &["daily", "ensure"],
        &root,
        &db,
        &["--file-format", "../%Y-%m-%d.org"],
    )?;
    assert_error_failure(
        &invalid_daily_path,
        "daily file path must stay within --root",
    );

    let malformed_capture = json_command_path(
        &["capture", "template"],
        &root,
        &db,
        &[
            "--node-key",
            "file:comparison.org",
            "--file",
            "ignored.org",
            "--type",
            "plain",
            "--content",
            "ignored",
        ],
    )?;
    assert_error_failure(&malformed_capture, "--node-key cannot be combined");

    let metadata_missing_target = json_command_path(
        &["node", "tag", "add"],
        &root,
        &db,
        &["--id", "missing-id", "ghost"],
    )?;
    assert_error_failure(&metadata_missing_target, "unknown node id: missing-id");

    let invalid_agenda_range = json_command_path(
        &["agenda", "range"],
        &root,
        &db,
        &["2026-05-14", "2026-05-13"],
    )?;
    assert_error_failure(
        &invalid_agenda_range,
        "agenda range end 2026-05-13 is before start 2026-05-14",
    );

    let invalid_graph_option =
        json_command_path(&["graph", "dot"], &root, &db, &["--hide-link-type", "ref"])?;
    assert_error_failure(&invalid_graph_option, "unsupported graph link type filter");

    let daemon_failure =
        json_command_path_with_bad_server(&["node", "search"], &root, &db, &["Left"])?;
    assert_error_failure(&daemon_failure, "failed to start slipbox daemon");

    Ok(())
}

#[test]
fn workflow_and_audit_commands_report_structured_json_failures() -> Result<()> {
    let (workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;

    let malformed_spec =
        workflow_json_command_with_stdin("show", &["--spec", "-"], br#"{"workflow_id":"broken""#)?;
    assert_error_failure(
        &malformed_spec,
        "failed to parse workflow spec JSON from stdin",
    );

    let invalid_spec = workflow_json_command_with_stdin(
        "show",
        &["--spec", "-"],
        br#"{"workflow_id":"workflow/invalid","title":"Invalid","inputs":[],"steps":[]}"#,
    )?;
    assert_error_failure(
        &invalid_spec,
        "invalid workflow spec: workflows must contain at least one step",
    );

    let future_spec = workflow_json_command_with_stdin(
        "show",
        &["--spec", "-"],
        br#"{"workflow_id":"workflow/future","title":"Future","compatibility":{"version":2},"inputs":[],"steps":[{"step_id":"future-step","kind":"future-step","future_field":true}]}"#,
    )?;
    assert_error_failure(
        &future_spec,
        "invalid workflow spec: unsupported workflow spec compatibility version 2; supported version is 1",
    );

    let unknown_show = workflow_json_command("show", &root, &db, &["workflow/builtin/missing"])?;
    assert_error_failure(&unknown_show, "unknown workflow: workflow/builtin/missing");

    let unknown_run = workflow_json_command("run", &root, &db, &["workflow/builtin/missing"])?;
    assert_error_failure(&unknown_run, "unknown workflow: workflow/builtin/missing");

    let audit_failure_path = workspace.path().join("missing").join("audit.jsonl");
    let audit_failure = run_slipbox(&[
        "audit".to_owned(),
        "duplicate-titles".to_owned(),
        "--root".to_owned(),
        root.clone(),
        "--db".to_owned(),
        db.clone(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--jsonl".to_owned(),
        "--output".to_owned(),
        audit_failure_path.to_str().expect("utf-8 path").to_owned(),
    ])?;
    assert_error_failure(&audit_failure, "failed to write report to");

    Ok(())
}

#[test]
fn pack_and_routine_commands_report_structured_json_failures() -> Result<()> {
    let (workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;

    let malformed_import =
        pack_json_command_with_stdin("import", &root, &db, &["-"], br#"{"pack_id":"broken""#)?;
    assert_error_failure(
        &malformed_import,
        "failed to parse workbench pack JSON from stdin",
    );

    let future_import = pack_json_command_with_stdin(
        "import",
        &root,
        &db,
        &["-"],
        br#"{"pack_id":"pack/future","title":"Future Pack","compatibility":{"version":2},"workflows":[{"kind":"future-workflow-shape"}]}"#,
    )?;
    assert_error_failure(
        &future_import,
        "invalid workbench pack: unsupported workbench pack compatibility version 2; supported version is 1",
    );

    let future_validate = pack_validate_json_command_with_stdin(
        br#"{"pack_id":"pack/future","title":"Future Pack","compatibility":{"version":2},"workflows":[{"kind":"future-workflow-shape"}]}"#,
    )?;
    assert!(future_validate.status.success(), "{future_validate:?}");
    let future_validate_json: Value = serde_json::from_slice(&future_validate.stdout)?;
    assert_exact_object_keys(&future_validate_json, &["pack", "valid", "issues"]);
    assert_eq!(future_validate_json["valid"], false);
    assert_workbench_pack_issue_keys(&future_validate_json["issues"][0]);
    assert_eq!(
        future_validate_json["issues"][0]["kind"],
        "unsupported-version"
    );

    let pack = contract_workbench_pack(
        "pack/error-contract",
        "workflow/pack/error-contract",
        "routine/pack/error-contract",
        "review/routine/error-contract/current",
    );
    let pack_path = workspace.path().join("error-contract-pack.json");
    write_pack_manifest(&pack, &pack_path)?;
    let imported = pack_json_command(
        "import",
        &root,
        &db,
        &[pack_path.to_str().expect("utf-8 path")],
    )?;
    assert!(imported.status.success(), "{imported:?}");
    let duplicate_import = pack_json_command(
        "import",
        &root,
        &db,
        &[pack_path.to_str().expect("utf-8 path")],
    )?;
    assert_error_failure(
        &duplicate_import,
        "workbench pack already exists: pack/error-contract",
    );

    let mut missing_reference = contract_workbench_pack(
        "pack/missing-reference",
        "workflow/pack/missing-reference",
        "routine/pack/missing-reference",
        "review/routine/missing-reference/current",
    );
    missing_reference.review_routines[0].report_profile_ids = vec!["profile/missing".to_owned()];
    let missing_reference_path = workspace.path().join("missing-reference-pack.json");
    write_pack_manifest(&missing_reference, &missing_reference_path)?;
    let missing_reference_import = pack_json_command(
        "import",
        &root,
        &db,
        &[missing_reference_path.to_str().expect("utf-8 path")],
    )?;
    assert_error_failure(
        &missing_reference_import,
        "references missing profile_id profile/missing",
    );

    let mut unsupported_profile = contract_workbench_pack(
        "pack/unsupported-profile",
        "workflow/pack/unsupported-profile",
        "routine/pack/unsupported-profile",
        "review/routine/unsupported-profile/current",
    );
    unsupported_profile.report_profiles[0].jsonl_line_kinds =
        Some(vec![ReportJsonlLineKind::Unsupported(
            "future-line".to_owned(),
        )]);
    let unsupported_profile_path = workspace.path().join("unsupported-profile-pack.json");
    write_pack_manifest(&unsupported_profile, &unsupported_profile_path)?;
    let unsupported_profile_import = pack_json_command(
        "import",
        &root,
        &db,
        &[unsupported_profile_path.to_str().expect("utf-8 path")],
    )?;
    assert_error_failure(&unsupported_profile_import, "unsupported: future-line");

    let missing_input = routine_json_command(
        "run",
        &root,
        &db,
        &[BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID],
    )?;
    assert_error_failure(&missing_input, "workflow input focus must be assigned");

    let missing_routine = routine_json_command("show", &root, &db, &["routine/missing"])?;
    assert_error_failure(&missing_routine, "unknown review routine: routine/missing");

    seed_duplicate_title_audit_fixture(&root, &db)?;
    let first = routine_json_command("run", &root, &db, &["routine/pack/error-contract"])?;
    assert!(first.status.success(), "{first:?}");
    let conflict = routine_json_command("run", &root, &db, &["routine/pack/error-contract"])?;
    assert_error_failure(
        &conflict,
        "review run already exists: review/routine/error-contract/current",
    );

    Ok(())
}

#[test]
fn artifact_id_commands_reject_invalid_ids_consistently() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;

    for subcommand in ["show", "run", "export", "delete"] {
        let output = artifact_json_command(subcommand, &root, &db, &[" artifact/structure "])?;
        assert_error_failure(
            &output,
            "artifact_id must not have leading or trailing whitespace",
        );
    }

    Ok(())
}

#[test]
fn live_save_commands_reject_save_flags_without_save_mode() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let explore = json_command(
        "explore",
        &root,
        &db,
        &[
            "--key",
            &anonymous_anchor_key,
            "--lens",
            "tasks",
            "--artifact-id",
            "artifact/stray",
        ],
    )?;
    assert_error_failure(&explore, "--artifact-id require --save");

    let compare = json_command(
        "compare",
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--overwrite",
        ],
    )?;
    assert_error_failure(&compare, "--overwrite require --save");

    Ok(())
}

#[test]
fn live_save_commands_report_structured_json_failures() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let initial = json_command(
        "explore",
        &root,
        &db,
        &[
            "--key",
            &anonymous_anchor_key,
            "--lens",
            "refs",
            "--save",
            "--artifact-id",
            "artifact/conflict",
            "--artifact-title",
            "Conflict Seed",
        ],
    )?;
    assert!(initial.status.success(), "{initial:?}");

    let explore_conflict = json_command(
        "explore",
        &root,
        &db,
        &[
            "--key",
            &anonymous_anchor_key,
            "--lens",
            "tasks",
            "--save",
            "--artifact-id",
            "artifact/conflict",
            "--artifact-title",
            "Conflict Explore",
        ],
    )?;
    assert_error_failure(
        &explore_conflict,
        "exploration artifact already exists: artifact/conflict",
    );

    let compare_conflict = json_command(
        "compare",
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--save",
            "--artifact-id",
            "artifact/conflict",
            "--artifact-title",
            "Conflict Compare",
        ],
    )?;
    assert_error_failure(
        &compare_conflict,
        "exploration artifact already exists: artifact/conflict",
    );

    let explore_missing_metadata = json_command(
        "explore",
        &root,
        &db,
        &[
            "--key",
            &anonymous_anchor_key,
            "--lens",
            "time",
            "--save",
            "--artifact-id",
            "artifact/missing-title",
        ],
    )?;
    assert_error_failure(
        &explore_missing_metadata,
        "--save requires --artifact-title",
    );

    let compare_missing_metadata = json_command(
        "compare",
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--save",
            "--artifact-title",
            "Missing Id",
        ],
    )?;
    assert_error_failure(&compare_missing_metadata, "--save requires --artifact-id");

    Ok(())
}

#[test]
fn review_commands_and_save_review_flows_report_structured_json_failures() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let show_missing = review_json_command("show", &root, &db, &["review/missing"])?;
    assert_error_failure(&show_missing, "unknown review run: review/missing");

    let diff_missing = review_json_command(
        "diff",
        &root,
        &db,
        &["review/workflow/base", "review/missing"],
    )?;
    assert_error_failure(&diff_missing, "unknown review run: review/missing");

    let mark_invalid = review_json_command(
        "mark",
        &root,
        &db,
        &[
            "review/workflow/base",
            "workflow-step/resolve-focus",
            "done",
        ],
    )?;
    assert_error_failure(&mark_invalid, "invalid review finding status `done`");

    let delete_invalid = review_json_command("delete", &root, &db, &[" review/workflow/base "])?;
    assert_error_failure(
        &delete_invalid,
        "review_id must not have leading or trailing whitespace",
    );

    seed_duplicate_title_audit_fixture(&root, &db)?;
    let initial = audit_json_command(
        "duplicate-titles",
        &root,
        &db,
        &[
            "--save-review",
            "--review-id",
            "review/audit/contracts/conflict",
        ],
    )?;
    assert!(initial.status.success(), "{initial:?}");

    let conflict = audit_json_command(
        "duplicate-titles",
        &root,
        &db,
        &[
            "--save-review",
            "--review-id",
            "review/audit/contracts/conflict",
        ],
    )?;
    assert_error_failure(
        &conflict,
        "review run already exists: review/audit/contracts/conflict",
    );

    let audit_stray = audit_json_command(
        "duplicate-titles",
        &root,
        &db,
        &["--review-id", "review/audit/contracts/stray"],
    )?;
    assert_error_failure(&audit_stray, "--review-id require --save-review");

    let workflow_stray = workflow_json_command(
        "run",
        &root,
        &db,
        &[
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anonymous_anchor_key}"),
            "--review-title",
            "Stray Workflow Review",
        ],
    )?;
    assert_error_failure(&workflow_stray, "--review-title require --save-review");

    Ok(())
}

#[test]
fn exported_artifact_json_round_trips_into_import_and_show() -> Result<()> {
    let (_source_workspace, source_root, source_db, _anonymous_anchor_key) =
        build_indexed_fixture()?;
    let export =
        artifact_json_command("export", &source_root, &source_db, &["artifact/structure"])?;
    assert!(export.status.success(), "{export:?}");
    let exported_json: Value = serde_json::from_slice(&export.stdout)?;
    assert_exact_object_keys(
        &exported_json,
        &[
            "artifact_id",
            "title",
            "summary",
            "kind",
            "root_node_key",
            "current_node_key",
            "lens",
            "limit",
            "unique",
            "frozen_context",
        ],
    );

    let (_target_workspace, target_root, target_db, _target_anchor_key) = build_indexed_fixture()?;
    let delete =
        artifact_json_command("delete", &target_root, &target_db, &["artifact/structure"])?;
    assert!(delete.status.success(), "{delete:?}");

    let import = artifact_json_command_with_stdin(
        "import",
        &target_root,
        &target_db,
        &["-"],
        &export.stdout,
    )?;
    assert!(import.status.success(), "{import:?}");
    let import_json: Value = serde_json::from_slice(&import.stdout)?;
    assert_exact_object_keys(&import_json, &["artifact"]);
    assert_saved_artifact_summary_keys(&import_json["artifact"]);

    let show = artifact_json_command("show", &target_root, &target_db, &["artifact/structure"])?;
    assert!(show.status.success(), "{show:?}");
    let show_json: Value = serde_json::from_slice(&show.stdout)?;
    assert_exact_object_keys(&show_json, &["artifact"]);
    assert_eq!(show_json["artifact"], exported_json);

    Ok(())
}

#[test]
fn saved_and_executed_comparison_json_contracts_stay_distinct() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;

    let compare_save = json_command(
        "compare",
        &root,
        &db,
        &[
            "--left-id",
            "left-id",
            "--right-id",
            "right-id",
            "--group",
            "tension",
            "--save",
            "--artifact-id",
            "artifact/contract-compare",
            "--artifact-title",
            "Contract Compare",
        ],
    )?;
    assert!(compare_save.status.success(), "{compare_save:?}");
    let compare_save_json: Value = serde_json::from_slice(&compare_save.stdout)?;
    assert_exact_object_keys(&compare_save_json, &["result", "artifact"]);
    assert_exact_object_keys(
        &compare_save_json["result"],
        &["left_note", "right_note", "sections"],
    );
    assert_saved_artifact_summary_keys(&compare_save_json["artifact"]);
    assert_eq!(compare_save_json["artifact"]["kind"], "comparison");

    let run = artifact_json_command("run", &root, &db, &["artifact/contract-compare"])?;
    assert!(run.status.success(), "{run:?}");
    let run_json: Value = serde_json::from_slice(&run.stdout)?;
    assert_exact_object_keys(&run_json, &["artifact"]);
    assert_exact_object_keys(
        &run_json["artifact"],
        &[
            "artifact_id",
            "title",
            "summary",
            "kind",
            "artifact",
            "root_note",
            "result",
        ],
    );
    assert_eq!(run_json["artifact"]["kind"], "comparison");
    assert_exact_object_keys(
        &run_json["artifact"]["artifact"],
        &[
            "root_node_key",
            "left_node_key",
            "right_node_key",
            "active_lens",
            "structure_unique",
            "comparison_group",
            "limit",
            "frozen_context",
        ],
    );
    assert_exact_object_keys(
        &run_json["artifact"]["result"],
        &["left_note", "right_note", "sections"],
    );

    Ok(())
}
