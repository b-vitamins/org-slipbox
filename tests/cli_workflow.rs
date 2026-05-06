use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use serde::Deserialize;
use slipbox_core::{
    BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID, BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
    BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID, BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
    BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID, ExplorationLens, ListWorkflowsResult,
    ReviewRunPayload, ReviewRunResult, RunWorkflowResult, SaveWorkflowReviewResult,
    WorkflowCatalogIssueKind, WorkflowResult, WorkflowSpec, built_in_workflow,
};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[derive(Debug, Deserialize)]
struct ErrorPayload {
    error: ErrorMessage,
}

#[derive(Debug, Deserialize)]
struct ErrorMessage {
    message: String,
}

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

fn build_indexed_fixture() -> Result<(tempfile::TempDir, String, String, String)> {
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
    fs::write(
        root.join("weak.org"),
        r#":PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:shared2024
:END:
#+title: Weak

Weakly integrated peer with shared references and no direct links.
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

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
        anonymous_anchor_key,
    ))
}

fn workflow_command(root: &str, db: &str, args: &[&str]) -> Result<std::process::Output> {
    workflow_command_with_dirs(root, db, &[], args)
}

fn workflow_command_with_dirs(
    root: &str,
    db: &str,
    workflow_dirs: &[&Path],
    args: &[&str],
) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args(["workflow"]);
    command.args(args);
    for workflow_dir in workflow_dirs {
        command.args([
            "--workflow-dir",
            workflow_dir
                .to_str()
                .context("workflow dir path should be valid utf-8")?,
        ]);
    }
    command.args([
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
    ]);
    Ok(command.output()?)
}

fn discovered_workflow(workflow_id: &str, title: &str, summary: &str) -> WorkflowSpec {
    let mut workflow = built_in_workflow(BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID)
        .expect("built-in workflow should exist");
    workflow.metadata.workflow_id = workflow_id.to_owned();
    workflow.metadata.title = title.to_owned();
    workflow.metadata.summary = Some(summary.to_owned());
    workflow
}

fn workflow_show_stdin(json: bool, payload: &[u8]) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args(["workflow", "show", "--spec", "-"]);
    if json {
        command.arg("--json");
    }
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;
    child
        .stdin
        .as_mut()
        .context("stdin pipe should exist")?
        .write_all(payload)?;
    Ok(child.wait_with_output()?)
}

fn review_show_command(root: &str, db: &str, review_id: &str) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "review",
        "show",
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
        review_id,
        "--json",
    ]);
    Ok(command.output()?)
}

#[test]
fn workflow_list_command_lists_built_ins_as_summaries() -> Result<()> {
    let (_workspace, root, db, _anchor_key) = build_indexed_fixture()?;

    let output = workflow_command(&root, &db, &["list", "--json"])?;

    assert!(output.status.success(), "{output:?}");
    let parsed: ListWorkflowsResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.workflows.len(), 5);
    let workflow_ids: Vec<&str> = parsed
        .workflows
        .iter()
        .map(|workflow| workflow.metadata.workflow_id.as_str())
        .collect();
    assert!(workflow_ids.contains(&BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID));
    assert!(workflow_ids.contains(&BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID));
    assert!(workflow_ids.contains(&BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID));
    assert!(workflow_ids.contains(&BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID));
    assert!(workflow_ids.contains(&BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn workflow_list_command_prints_human_summaries() -> Result<()> {
    let (_workspace, root, db, _anchor_key) = build_indexed_fixture()?;

    let output = workflow_command(&root, &db, &["list"])?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("Comparison Tension Review [workflow/builtin/comparison-tension-review]")
    );
    assert!(stdout.contains("Periodic Review [workflow/builtin/periodic-review]"));
    assert!(stdout.contains("Weak Integration Review [workflow/builtin/weak-integration-review]"));
    assert!(stdout.contains("Context Sweep [workflow/builtin/context-sweep]"));
    assert!(stdout.contains("steps: 6"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn workflow_show_command_returns_built_in_specs_as_json() -> Result<()> {
    let (_workspace, root, db, _anchor_key) = build_indexed_fixture()?;

    let output = workflow_command(
        &root,
        &db,
        &["show", BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID, "--json"],
    )?;

    assert!(output.status.success(), "{output:?}");
    let parsed: WorkflowResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
    );
    assert_eq!(parsed.workflow.inputs.len(), 1);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn workflow_show_command_prints_human_built_in_specs() -> Result<()> {
    let (_workspace, root, db, _anchor_key) = build_indexed_fixture()?;

    let output = workflow_command(&root, &db, &["show", BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID])?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("workflow id: workflow/builtin/context-sweep"));
    assert!(stdout.contains("compatibility: workflow-spec/v1"));
    assert!(stdout.contains("[inputs]"));
    assert!(stdout.contains("- focus [focus-target]"));
    assert!(stdout.contains("- explore-refs [explore]"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn workflow_show_command_reads_spec_json_from_file_and_stdin() -> Result<()> {
    let (_workspace, _root, _db, _anchor_key) = build_indexed_fixture()?;
    let workflow = built_in_workflow(BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID)
        .context("built-in workflow should exist")?;
    let spec_workspace = tempdir()?;
    let spec_path = spec_workspace.path().join("workflow.json");
    fs::write(&spec_path, serde_json::to_vec_pretty(&workflow)?)?;

    let file_output = Command::new(slipbox_binary())
        .args([
            "workflow",
            "show",
            "--spec",
            spec_path
                .to_str()
                .context("spec path should be valid utf-8")?,
            "--json",
        ])
        .output()?;
    assert!(file_output.status.success(), "{file_output:?}");
    let parsed_from_file: WorkflowResult = serde_json::from_slice(&file_output.stdout)?;
    assert_eq!(parsed_from_file.workflow, workflow);

    let stdin_output = workflow_show_stdin(true, &serde_json::to_vec_pretty(&workflow)?)?;
    assert!(stdin_output.status.success(), "{stdin_output:?}");
    let parsed_from_stdin: WorkflowResult = serde_json::from_slice(&stdin_output.stdout)?;
    assert_eq!(parsed_from_stdin.workflow, workflow);
    assert!(stdin_output.stderr.is_empty());

    Ok(())
}

#[test]
fn workflow_run_command_executes_built_ins_as_json() -> Result<()> {
    let (_workspace, root, db, anchor_key) = build_indexed_fixture()?;

    let output = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--json",
        ],
    )?;

    assert!(output.status.success(), "{output:?}");
    let parsed: RunWorkflowResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed.result.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
    );
    assert_eq!(parsed.result.steps.len(), 4);
    match &parsed.result.steps[2].payload {
        slipbox_core::WorkflowStepReportPayload::Explore {
            focus_node_key,
            result,
        } => {
            assert_eq!(focus_node_key, &anchor_key);
            assert_eq!(result.lens, ExplorationLens::Tasks);
        }
        other => panic!("expected tasks explore report, got {:?}", other.kind()),
    }
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn workflow_run_command_prints_human_execution_reports() -> Result<()> {
    let (_workspace, root, db, anchor_key) = build_indexed_fixture()?;

    let output = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
        ],
    )?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("workflow: Unresolved Sweep [workflow/builtin/unresolved-sweep]"));
    assert!(stdout.contains("[step explore-tasks]"));
    assert!(stdout.contains(&format!("focus node key: {anchor_key}")));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn workflow_run_command_executes_operational_review_built_ins() -> Result<()> {
    let (_workspace, root, db, anchor_key) = build_indexed_fixture()?;

    let periodic = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--json",
        ],
    )?;

    assert!(periodic.status.success(), "{periodic:?}");
    let periodic: RunWorkflowResult = serde_json::from_slice(&periodic.stdout)?;
    assert_eq!(
        periodic.result.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID
    );
    assert_eq!(periodic.result.steps.len(), 6);
    match &periodic.result.steps[4].payload {
        slipbox_core::WorkflowStepReportPayload::Explore {
            focus_node_key,
            result,
        } => {
            assert_eq!(focus_node_key, &anchor_key);
            assert_eq!(result.lens, ExplorationLens::Refs);
        }
        other => panic!("expected refs review step, got {:?}", other.kind()),
    }

    let weak = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--json",
        ],
    )?;

    assert!(weak.status.success(), "{weak:?}");
    let weak: RunWorkflowResult = serde_json::from_slice(&weak.stdout)?;
    assert_eq!(
        weak.result.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID
    );
    assert_eq!(weak.result.steps.len(), 4);
    match &weak.result.steps[1].payload {
        slipbox_core::WorkflowStepReportPayload::Explore { result, .. } => {
            assert_eq!(result.lens, ExplorationLens::Unresolved);
            assert!(result.sections.iter().any(|section| {
                section.kind == slipbox_core::ExplorationSectionKind::WeaklyIntegratedNotes
            }));
        }
        other => panic!(
            "expected weak integration review step, got {:?}",
            other.kind()
        ),
    }

    Ok(())
}

#[test]
fn workflow_run_save_review_flags_persist_typed_reviews_and_enforce_policy() -> Result<()> {
    let (_workspace, root, db, anchor_key) = build_indexed_fixture()?;

    let output = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--save-review",
            "--review-id",
            "review/workflow/unresolved/custom",
            "--review-title",
            "Unresolved Workflow Review",
            "--review-summary",
            "Preserve unresolved workflow evidence.",
            "--json",
        ],
    )?;
    assert!(output.status.success(), "{output:?}");
    let saved: SaveWorkflowReviewResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        saved.result.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
    );
    assert_eq!(saved.result.steps.len(), 4);
    assert_eq!(
        saved.review.metadata.review_id,
        "review/workflow/unresolved/custom"
    );
    assert_eq!(saved.review.metadata.title, "Unresolved Workflow Review");
    assert_eq!(saved.review.finding_count, saved.result.steps.len());

    let shown = review_show_command(&root, &db, "review/workflow/unresolved/custom")?;
    assert!(shown.status.success(), "{shown:?}");
    let shown: ReviewRunResult = serde_json::from_slice(&shown.stdout)?;
    match &shown.review.payload {
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
        other => panic!("expected workflow review, got {:?}", other.kind()),
    }
    assert_eq!(shown.review.findings.len(), saved.result.steps.len());
    match &shown.review.findings[0].payload {
        slipbox_core::ReviewFindingPayload::WorkflowStep { step } => {
            assert_eq!(step.as_ref(), &saved.result.steps[0]);
        }
        other => panic!("expected workflow-step finding, got {:?}", other.kind()),
    }

    let conflict = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--save-review",
            "--review-id",
            "review/workflow/unresolved/custom",
            "--json",
        ],
    )?;
    assert_eq!(conflict.status.code(), Some(1));
    assert!(conflict.stdout.is_empty());
    let error: ErrorPayload = serde_json::from_slice(&conflict.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("review run already exists: review/workflow/unresolved/custom")
    );

    let overwritten = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--save-review",
            "--review-id",
            "review/workflow/unresolved/custom",
            "--review-title",
            "Replacement Workflow Review",
            "--overwrite",
            "--json",
        ],
    )?;
    assert!(overwritten.status.success(), "{overwritten:?}");
    let overwritten: SaveWorkflowReviewResult = serde_json::from_slice(&overwritten.stdout)?;
    assert_eq!(
        overwritten.review.metadata.title,
        "Replacement Workflow Review"
    );

    let invalid = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--save-review",
            "--review-id",
            " bad ",
            "--json",
        ],
    )?;
    assert_eq!(invalid.status.code(), Some(1));
    let error: ErrorPayload = serde_json::from_slice(&invalid.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("review_id must not have leading or trailing whitespace")
    );

    let stray = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--review-title",
            "Stray Review",
            "--json",
        ],
    )?;
    assert_eq!(stray.status.code(), Some(1));
    let error: ErrorPayload = serde_json::from_slice(&stray.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("--review-title require --save-review")
    );

    Ok(())
}

#[test]
fn workflow_run_command_rejects_invalid_input_assignment_syntax() -> Result<()> {
    let (_workspace, root, db, _anchor_key) = build_indexed_fixture()?;

    let output = workflow_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
            "--input",
            "focus=badkind:value",
            "--json",
        ],
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("invalid workflow input assignment focus=badkind:value")
    );

    Ok(())
}

#[test]
fn workflow_discovery_lists_shows_and_runs_valid_specs_while_reporting_invalid_ones() -> Result<()>
{
    let (workspace, root, db, anchor_key) = build_indexed_fixture()?;
    let workflow_dir = workspace.path().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    let valid = discovered_workflow(
        "workflow/test/discovered-unresolved",
        "Discovered Unresolved Sweep",
        "Run unresolved and task-oriented exploration from a configured directory.",
    );
    fs::write(
        workflow_dir.join("valid.json"),
        serde_json::to_vec_pretty(&valid)?,
    )?;

    let mut invalid = discovered_workflow(
        "workflow/test/invalid-workflow",
        "Invalid Workflow",
        "Intentionally invalid workflow fixture.",
    );
    invalid.steps.clear();
    fs::write(
        workflow_dir.join("invalid.json"),
        serde_json::to_vec_pretty(&invalid)?,
    )?;
    fs::write(
        workflow_dir.join("future.json"),
        br#"{
  "workflow_id": "workflow/test/future-workflow",
  "title": "Future Workflow",
  "summary": "Unsupported future compatibility fixture.",
  "compatibility": {
    "version": 2
  },
  "inputs": [],
  "steps": [
    {
      "step_id": "future-step",
      "kind": "future-step",
      "future_field": true
    }
  ]
}"#,
    )?;

    let listed =
        workflow_command_with_dirs(&root, &db, &[workflow_dir.as_path()], &["list", "--json"])?;
    assert!(listed.status.success(), "{listed:?}");
    let listed: ListWorkflowsResult = serde_json::from_slice(&listed.stdout)?;
    assert!(
        listed
            .workflows
            .iter()
            .any(|workflow| workflow.metadata.workflow_id == valid.metadata.workflow_id)
    );
    assert_eq!(listed.issues.len(), 2);
    let invalid_issue = listed
        .issues
        .iter()
        .find(|issue| issue.workflow_id.as_deref() == Some("workflow/test/invalid-workflow"))
        .expect("invalid workflow issue should be reported");
    assert_eq!(invalid_issue.kind, WorkflowCatalogIssueKind::InvalidSpec);
    assert!(
        invalid_issue
            .message
            .contains("workflows must contain at least one step")
    );
    let future_issue = listed
        .issues
        .iter()
        .find(|issue| issue.workflow_id.as_deref() == Some("workflow/test/future-workflow"))
        .expect("future workflow issue should be reported");
    assert_eq!(
        future_issue.kind,
        WorkflowCatalogIssueKind::UnsupportedVersion
    );
    assert!(future_issue.message.contains("unsupported workflow spec"));

    let listed_human =
        workflow_command_with_dirs(&root, &db, &[workflow_dir.as_path()], &["list"])?;
    assert!(listed_human.status.success(), "{listed_human:?}");
    let listed_human = String::from_utf8(listed_human.stdout)?;
    assert!(
        listed_human.contains("Discovered Unresolved Sweep [workflow/test/discovered-unresolved]")
    );
    assert!(listed_human.contains("[issues]"));
    assert!(listed_human.contains("kind: invalid-spec"));
    assert!(listed_human.contains("workflow id: workflow/test/invalid-workflow"));
    assert!(listed_human.contains("kind: unsupported-version"));
    assert!(listed_human.contains("workflow id: workflow/test/future-workflow"));

    let shown = workflow_command_with_dirs(
        &root,
        &db,
        &[workflow_dir.as_path()],
        &["show", "workflow/test/discovered-unresolved", "--json"],
    )?;
    assert!(shown.status.success(), "{shown:?}");
    let shown: WorkflowResult = serde_json::from_slice(&shown.stdout)?;
    assert_eq!(shown.workflow, valid);

    let executed = workflow_command_with_dirs(
        &root,
        &db,
        &[workflow_dir.as_path()],
        &[
            "run",
            "workflow/test/discovered-unresolved",
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--json",
        ],
    )?;
    assert!(executed.status.success(), "{executed:?}");
    let executed: RunWorkflowResult = serde_json::from_slice(&executed.stdout)?;
    assert_eq!(
        executed.result.workflow.metadata.workflow_id,
        "workflow/test/discovered-unresolved"
    );
    match &executed.result.steps[2].payload {
        slipbox_core::WorkflowStepReportPayload::Explore {
            focus_node_key,
            result,
        } => {
            assert_eq!(focus_node_key, &anchor_key);
            assert_eq!(result.lens, ExplorationLens::Tasks);
        }
        other => panic!("expected tasks explore report, got {:?}", other.kind()),
    }

    Ok(())
}

#[test]
fn workflow_discovery_uses_deterministic_collision_precedence() -> Result<()> {
    let (workspace, root, db, _anchor_key) = build_indexed_fixture()?;
    let first_dir = workspace.path().join("workflows-a");
    let second_dir = workspace.path().join("workflows-b");
    fs::create_dir_all(&first_dir)?;
    fs::create_dir_all(&second_dir)?;

    let earlier = discovered_workflow(
        "workflow/test/discovered-collision",
        "Earlier Winner",
        "The earlier configured workflow directory should win.",
    );
    let mut later = earlier.clone();
    later.metadata.title = "Later Loser".to_owned();
    later.metadata.summary = Some("This one should lose the collision.".to_owned());
    let mut shadow_builtin = discovered_workflow(
        BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
        "Shadow Built-in",
        "This should lose to the built-in workflow.",
    );
    shadow_builtin.metadata.summary = Some("Built-ins must win collisions.".to_owned());

    fs::write(
        first_dir.join("earlier.json"),
        serde_json::to_vec_pretty(&earlier)?,
    )?;
    fs::write(
        second_dir.join("later.json"),
        serde_json::to_vec_pretty(&later)?,
    )?;
    fs::write(
        second_dir.join("builtin-shadow.json"),
        serde_json::to_vec_pretty(&shadow_builtin)?,
    )?;

    let listed = workflow_command_with_dirs(
        &root,
        &db,
        &[first_dir.as_path(), second_dir.as_path()],
        &["list", "--json"],
    )?;
    assert!(listed.status.success(), "{listed:?}");
    let listed: ListWorkflowsResult = serde_json::from_slice(&listed.stdout)?;
    let discovered: Vec<_> = listed
        .workflows
        .iter()
        .filter(|workflow| workflow.metadata.workflow_id == "workflow/test/discovered-collision")
        .collect();
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].metadata.title, "Earlier Winner");
    assert!(
        listed
            .issues
            .iter()
            .any(|issue| issue.workflow_id.as_deref()
                == Some("workflow/test/discovered-collision")
                && issue.kind == WorkflowCatalogIssueKind::DuplicateWorkflowId
                && issue.message.contains("collides with discovered workflow"))
    );
    assert!(
        listed
            .issues
            .iter()
            .any(
                |issue| issue.workflow_id.as_deref() == Some(BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID)
                    && issue.kind == WorkflowCatalogIssueKind::DuplicateWorkflowId
                    && issue.message.contains("collides with built-in workflow")
            )
    );

    let shown = workflow_command_with_dirs(
        &root,
        &db,
        &[first_dir.as_path(), second_dir.as_path()],
        &["show", "workflow/test/discovered-collision", "--json"],
    )?;
    assert!(shown.status.success(), "{shown:?}");
    let shown: WorkflowResult = serde_json::from_slice(&shown.stdout)?;
    assert_eq!(shown.workflow.metadata.title, "Earlier Winner");

    Ok(())
}
