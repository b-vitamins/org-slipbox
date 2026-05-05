use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Result;
use serde_json::Value;
use slipbox_core::{
    BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID, BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
    ExplorationArtifactMetadata, ExplorationArtifactPayload, ExplorationLens, NodeKind, NodeRecord,
    ReviewFinding, ReviewFindingPayload, ReviewFindingStatus, ReviewRun, ReviewRunMetadata,
    ReviewRunPayload, SavedComparisonArtifact, SavedExplorationArtifact, SavedLensViewArtifact,
    WorkflowMetadata, WorkflowSpec, WorkflowStepReport, WorkflowStepReportPayload, WorkflowSummary,
    built_in_workflow,
};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

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

fn run_command(args: &[String]) -> Result<std::process::Output> {
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

fn run_command_with_stdin(args: &[String], stdin: &[u8]) -> Result<std::process::Output> {
    let mut child = Command::new(slipbox_binary())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .expect("child stdin should be piped")
        .write_all(stdin)?;
    Ok(child.wait_with_output()?)
}

fn base_args(root: &str, db: &str) -> Vec<String> {
    vec![
        "--root".to_owned(),
        root.to_owned(),
        "--db".to_owned(),
        db.to_owned(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
    ]
}

fn json_command(
    command: &str,
    root: &str,
    db: &str,
    extra: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec![command.to_owned()];
    args.extend(base_args(root, db));
    args.push("--json".to_owned());
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_command(&args)
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
    run_command(&args)
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
    run_command_with_stdin(&args, stdin)
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
    run_command(&args)
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
    run_command(&args)
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
    run_command_with_stdin(&args, stdin)
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
    run_command(&args)
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

fn sorted_keys(value: &Value) -> Vec<String> {
    let object = value.as_object().expect("expected JSON object");
    let mut keys: Vec<String> = object.keys().cloned().collect();
    keys.sort();
    keys
}

fn assert_exact_object_keys(value: &Value, expected: &[&str]) {
    let mut expected_keys: Vec<String> = expected.iter().map(|key| (*key).to_owned()).collect();
    expected_keys.sort();
    assert_eq!(sorted_keys(value), expected_keys);
}

fn assert_error_failure(output: &std::process::Output, needle: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let parsed: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be structured JSON");
    assert_exact_object_keys(&parsed, &["error"]);
    let message = parsed["error"]["message"]
        .as_str()
        .expect("error message should be a string");
    assert!(message.contains(needle), "{message}");
}

fn assert_saved_artifact_summary_keys(value: &Value) {
    assert_exact_object_keys(value, &["artifact_id", "title", "summary", "kind"]);
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
    assert_exact_object_keys(
        &review_list_json["reviews"][0],
        &[
            "review_id",
            "title",
            "summary",
            "kind",
            "finding_count",
            "status_counts",
        ],
    );

    let review_show = review_json_command("show", &root, &db, &["review/workflow/base"])?;
    assert!(review_show.status.success(), "{review_show:?}");
    let review_show_json: Value = serde_json::from_slice(&review_show.stdout)?;
    assert_exact_object_keys(&review_show_json, &["review"]);
    assert_exact_object_keys(
        &review_show_json["review"],
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
    assert_exact_object_keys(
        &review_show_json["review"]["findings"][0],
        &["finding_id", "status", "kind", "step"],
    );

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
        &["workflow_id", "title", "summary", "inputs", "steps"],
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
        &["path", "workflow_id", "message"],
    );

    let workflow_report_path = workspace.path().join("workflow-report.jsonl");
    let workflow_report = run_command(&[
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

    let audit_jsonl = run_command(&[
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
        let output = run_command(&command)?;
        assert_error_failure(&output, "failed to start slipbox daemon");
    }

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

    let unknown_show = workflow_json_command("show", &root, &db, &["workflow/builtin/missing"])?;
    assert_error_failure(&unknown_show, "unknown workflow: workflow/builtin/missing");

    let unknown_run = workflow_json_command("run", &root, &db, &["workflow/builtin/missing"])?;
    assert_error_failure(&unknown_run, "unknown workflow: workflow/builtin/missing");

    let audit_failure_path = workspace.path().join("missing").join("audit.jsonl");
    let audit_failure = run_command(&[
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
