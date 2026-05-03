use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use serde::Deserialize;
use slipbox_core::{
    BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID, BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
    BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID, ExplorationLens, ListWorkflowsResult, RunWorkflowResult,
    WorkflowResult, built_in_workflow,
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
    let mut command = Command::new(slipbox_binary());
    command.args(["workflow"]);
    command.args(args);
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

#[test]
fn workflow_list_command_lists_built_ins_as_summaries() -> Result<()> {
    let (_workspace, root, db, _anchor_key) = build_indexed_fixture()?;

    let output = workflow_command(&root, &db, &["list", "--json"])?;

    assert!(output.status.success(), "{output:?}");
    let parsed: ListWorkflowsResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.workflows.len(), 3);
    let workflow_ids: Vec<&str> = parsed
        .workflows
        .iter()
        .map(|workflow| workflow.metadata.workflow_id.as_str())
        .collect();
    assert!(workflow_ids.contains(&BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID));
    assert!(workflow_ids.contains(&BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID));
    assert!(workflow_ids.contains(&BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID));
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
    assert!(stdout.contains("Context Sweep [workflow/builtin/context-sweep]"));
    assert!(stdout.contains("steps: 4"));
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
