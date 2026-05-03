use std::fs;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{
    BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID, CorpusAuditKind, CorpusAuditReportLine,
    CorpusAuditResult, WorkflowReportLine, WorkflowSummary,
};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum ReportFormat {
    Human,
    Json,
    Jsonl,
}

#[derive(Debug, Deserialize)]
struct WorkflowReportOutputResult {
    workflow: WorkflowSummary,
    format: ReportFormat,
    output_path: String,
    step_count: usize,
}

#[derive(Debug, Deserialize)]
struct AuditReportOutputResult {
    audit: CorpusAuditKind,
    format: ReportFormat,
    output_path: String,
    entry_count: usize,
}

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

fn build_workflow_fixture() -> Result<(tempfile::TempDir, String, String, String)> {
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
        .expect("anonymous heading anchor should exist")
        .node_key;

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
        anonymous_anchor_key,
    ))
}

fn build_audit_fixture() -> Result<(tempfile::TempDir, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("duplicate-a.org"),
        r#":PROPERTIES:
:ID: dup-a-id
:END:
#+title: Shared Title

Links to [[id:dup-b-id][Other duplicate]].
"#,
    )?;
    fs::write(
        root.join("duplicate-b.org"),
        r#":PROPERTIES:
:ID: dup-b-id
:END:
#+title: shared title

Links to [[id:dup-a-id][Other duplicate]].
"#,
    )?;
    fs::write(
        root.join("dangling-source.org"),
        r#":PROPERTIES:
:ID: dangling-source-id
:END:
#+title: Dangling Source

Points to [[id:missing-id][Missing]].
"#,
    )?;
    fs::write(
        root.join("orphan.org"),
        r#":PROPERTIES:
:ID: orphan-id
:END:
#+title: Orphan

Just an orphan note.
"#,
    )?;
    fs::write(
        root.join("weak.org"),
        r#":PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:weak2024
:END:
#+title: Weak

Has refs but no structural links.
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
    ))
}

fn workflow_command(root: &str, db: &str, args: &[&str]) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args(["workflow", "run"]);
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

fn audit_command(root: &str, db: &str, args: &[&str]) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args(["audit"]);
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

fn parse_jsonl_lines<T>(bytes: &[u8]) -> Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).map_err(anyhow::Error::from))
        .collect()
}

#[test]
fn workflow_run_command_emits_stable_jsonl_reports_to_stdout() -> Result<()> {
    let (_workspace, root, db, anchor_key) = build_workflow_fixture()?;

    let output = workflow_command(
        &root,
        &db,
        &[
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--jsonl",
        ],
    )?;

    assert!(output.status.success(), "{output:?}");
    let lines: Vec<WorkflowReportLine> = parse_jsonl_lines(&output.stdout)?;
    assert_eq!(lines.len(), 5);
    match &lines[0] {
        WorkflowReportLine::Workflow { workflow } => {
            assert_eq!(
                workflow.metadata.workflow_id,
                BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
            );
            assert_eq!(workflow.step_count, 4);
        }
        other => panic!("expected workflow header line, got {other:?}"),
    }
    assert!(matches!(lines[1], WorkflowReportLine::Step { .. }));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn workflow_run_command_writes_jsonl_reports_to_file_and_returns_json_ack() -> Result<()> {
    let (workspace, root, db, anchor_key) = build_workflow_fixture()?;
    let output_path = workspace.path().join("workflow-report.jsonl");

    let output = workflow_command(
        &root,
        &db,
        &[
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--jsonl",
            "--output",
            output_path
                .to_str()
                .expect("output path should be valid utf-8"),
        ],
    )?;

    assert!(output.status.success(), "{output:?}");
    let ack: WorkflowReportOutputResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        ack.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
    );
    assert_eq!(ack.format, ReportFormat::Jsonl);
    assert_eq!(ack.output_path, output_path.display().to_string());
    assert_eq!(ack.step_count, 4);

    let written = fs::read(&output_path)?;
    let lines: Vec<WorkflowReportLine> = parse_jsonl_lines(&written)?;
    assert_eq!(lines.len(), 5);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn audit_commands_emit_stable_jsonl_reports_to_stdout() -> Result<()> {
    let (_workspace, root, db) = build_audit_fixture()?;

    let output = audit_command(&root, &db, &["duplicate-titles", "--jsonl"])?;

    assert!(output.status.success(), "{output:?}");
    let lines: Vec<CorpusAuditReportLine> = parse_jsonl_lines(&output.stdout)?;
    assert_eq!(lines.len(), 2);
    match &lines[0] {
        CorpusAuditReportLine::Audit { audit } => {
            assert_eq!(*audit, CorpusAuditKind::DuplicateTitles);
        }
        other => panic!("expected audit header line, got {other:?}"),
    }
    assert!(matches!(lines[1], CorpusAuditReportLine::Entry { .. }));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn audit_commands_write_json_reports_to_file_and_return_json_ack() -> Result<()> {
    let (workspace, root, db) = build_audit_fixture()?;
    let output_path = workspace.path().join("audit-report.json");

    let output = audit_command(
        &root,
        &db,
        &[
            "dangling-links",
            "--json",
            "--output",
            output_path
                .to_str()
                .expect("output path should be valid utf-8"),
        ],
    )?;

    assert!(output.status.success(), "{output:?}");
    let ack: AuditReportOutputResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(ack.audit, CorpusAuditKind::DanglingLinks);
    assert_eq!(ack.format, ReportFormat::Json);
    assert_eq!(ack.output_path, output_path.display().to_string());
    assert_eq!(ack.entry_count, 1);

    let written: CorpusAuditResult = serde_json::from_slice(&fs::read(&output_path)?)?;
    assert_eq!(written.audit, CorpusAuditKind::DanglingLinks);
    assert_eq!(written.entries.len(), 1);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn workflow_and_audit_reports_reject_json_and_jsonl_together() -> Result<()> {
    let (_workflow_workspace, workflow_root, workflow_db, anchor_key) = build_workflow_fixture()?;
    let workflow_output = workflow_command(
        &workflow_root,
        &workflow_db,
        &[
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            &format!("focus=key:{anchor_key}"),
            "--json",
            "--jsonl",
        ],
    )?;
    assert!(!workflow_output.status.success(), "{workflow_output:?}");
    let workflow_error: ErrorPayload = serde_json::from_slice(&workflow_output.stderr)?;
    assert!(
        workflow_error
            .error
            .message
            .contains("--json and --jsonl are mutually exclusive")
    );

    let (_audit_workspace, audit_root, audit_db) = build_audit_fixture()?;
    let audit_output = audit_command(
        &audit_root,
        &audit_db,
        &["duplicate-titles", "--json", "--jsonl"],
    )?;
    assert!(!audit_output.status.success(), "{audit_output:?}");
    let audit_error: ErrorPayload = serde_json::from_slice(&audit_output.stderr)?;
    assert!(
        audit_error
            .error
            .message
            .contains("--json and --jsonl are mutually exclusive")
    );

    Ok(())
}

#[test]
fn workflow_jsonl_failures_keep_structured_stderr() -> Result<()> {
    let (_workspace, root, db, _anchor_key) = build_workflow_fixture()?;

    let output = workflow_command(
        &root,
        &db,
        &[
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
            "--input",
            "focus=badkind:value",
            "--jsonl",
        ],
    )?;

    assert!(!output.status.success(), "{output:?}");
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("invalid workflow input assignment focus=badkind:value")
    );
    assert!(output.stdout.is_empty());

    Ok(())
}

#[test]
fn audit_jsonl_failures_keep_structured_stderr() -> Result<()> {
    let (workspace, root, db) = build_audit_fixture()?;
    let output_path = workspace.path().join("missing").join("report.jsonl");

    let output = audit_command(
        &root,
        &db,
        &[
            "duplicate-titles",
            "--jsonl",
            "--output",
            output_path
                .to_str()
                .expect("output path should be valid utf-8"),
        ],
    )?;

    assert!(!output.status.success(), "{output:?}");
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(error.error.message.contains("failed to write report to"));
    assert!(output.stdout.is_empty());

    Ok(())
}
