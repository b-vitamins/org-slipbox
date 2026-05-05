use std::fs;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{
    AnchorRecord, CorpusAuditEntry, CorpusAuditKind, DanglingLinkAuditRecord,
    DeleteReviewRunResult, ListReviewRunsResult, NodeKind, NodeRecord, ReviewFinding,
    ReviewFindingPayload, ReviewFindingStatus, ReviewRun, ReviewRunKind, ReviewRunMetadata,
    ReviewRunPayload, ReviewRunResult, WorkflowMetadata, WorkflowStepReport,
    WorkflowStepReportPayload, WorkflowSummary,
};
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

fn build_review_fixture() -> Result<(tempfile::TempDir, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let database = Database::open(&db)?;
    database.save_review_run(&audit_review_run())?;
    database.save_review_run(&workflow_review_run())?;

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
    ))
}

fn audit_review_run() -> ReviewRun {
    ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/dangling".to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: Some("Review dangling links".to_owned()),
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 20,
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

fn workflow_review_run() -> ReviewRun {
    ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/workflow/context".to_owned(),
            title: "Workflow Review".to_owned(),
            summary: Some("Review workflow execution".to_owned()),
        },
        payload: ReviewRunPayload::Workflow {
            workflow: WorkflowSummary {
                metadata: WorkflowMetadata {
                    workflow_id: "workflow/test/context".to_owned(),
                    title: "Context Workflow".to_owned(),
                    summary: Some("A test workflow".to_owned()),
                },
                step_count: 1,
            },
            inputs: Vec::new(),
            step_ids: vec!["resolve-focus".to_owned()],
        },
        findings: vec![ReviewFinding {
            finding_id: "workflow-step/resolve-focus".to_owned(),
            status: ReviewFindingStatus::Reviewed,
            payload: ReviewFindingPayload::WorkflowStep {
                step: Box::new(WorkflowStepReport {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepReportPayload::Resolve {
                        node: Box::new(sample_node("heading:focus.org:3", "Focus Node")),
                    },
                }),
            },
        }],
    }
}

fn sample_node(node_key: &str, title: &str) -> NodeRecord {
    NodeRecord {
        node_key: node_key.to_owned(),
        explicit_id: Some("focus-id".to_owned()),
        file_path: "focus.org".to_owned(),
        title: title.to_owned(),
        outline_path: title.to_owned(),
        aliases: Vec::new(),
        tags: vec!["review".to_owned()],
        refs: Vec::new(),
        todo_keyword: Some("TODO".to_owned()),
        scheduled_for: None,
        deadline_for: None,
        closed_at: None,
        level: 1,
        line: 3,
        kind: NodeKind::Heading,
        file_mtime_ns: 0,
        backlink_count: 2,
        forward_link_count: 1,
    }
}

fn review_list_command(root: &str, db: &str, json: bool) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "review",
        "list",
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
    ]);
    if json {
        command.arg("--json");
    }
    Ok(command.output()?)
}

fn review_id_command(
    root: &str,
    db: &str,
    subcommand: &str,
    review_id: &str,
    json: bool,
) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "review",
        subcommand,
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
    ]);
    if json {
        command.arg("--json");
    }
    command.arg(review_id);
    Ok(command.output()?)
}

#[test]
fn review_list_command_lists_review_runs_as_summaries() -> Result<()> {
    let (_workspace, root, db) = build_review_fixture()?;

    let output = review_list_command(&root, &db, true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: ListReviewRunsResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.reviews.len(), 2);
    assert_eq!(
        parsed.reviews[0].metadata.review_id,
        "review/audit/dangling"
    );
    assert_eq!(parsed.reviews[0].kind, ReviewRunKind::Audit);
    assert_eq!(parsed.reviews[0].finding_count, 1);
    assert_eq!(parsed.reviews[0].status_counts.open, 1);
    assert_eq!(
        parsed.reviews[1].metadata.review_id,
        "review/workflow/context"
    );
    assert_eq!(parsed.reviews[1].kind, ReviewRunKind::Workflow);
    assert_eq!(parsed.reviews[1].finding_count, 1);
    assert_eq!(parsed.reviews[1].status_counts.reviewed, 1);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn review_list_command_prints_human_summaries() -> Result<()> {
    let (_workspace, root, db) = build_review_fixture()?;

    let output = review_list_command(&root, &db, false)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("- Dangling Link Review [audit]"));
    assert!(stdout.contains("review id: review/audit/dangling"));
    assert!(stdout.contains("findings: 1"));
    assert!(stdout.contains("open/reviewed/dismissed/accepted: 1/0/0/0"));
    assert!(stdout.contains("- Workflow Review [workflow]"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn review_show_command_returns_review_run_json() -> Result<()> {
    let (_workspace, root, db) = build_review_fixture()?;

    let output = review_id_command(&root, &db, "show", "review/audit/dangling", true)?;

    assert!(output.status.success(), "{output:?}");
    let parsed: ReviewRunResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.review.metadata.review_id, "review/audit/dangling");
    match parsed.review.payload {
        ReviewRunPayload::Audit { audit, limit } => {
            assert_eq!(audit, CorpusAuditKind::DanglingLinks);
            assert_eq!(limit, 20);
        }
        other => panic!("unexpected review payload: {other:?}"),
    }
    assert_eq!(parsed.review.findings.len(), 1);
    assert_eq!(parsed.review.findings[0].status, ReviewFindingStatus::Open);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn review_show_command_prints_human_review_details() -> Result<()> {
    let (_workspace, root, db) = build_review_fixture()?;

    let output = review_id_command(&root, &db, "show", "review/audit/dangling", false)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("review id: review/audit/dangling"));
    assert!(stdout.contains("kind: audit"));
    assert!(stdout.contains("audit: dangling-links"));
    assert!(stdout.contains("[findings]"));
    assert!(
        stdout.contains(
            "dangling link: Source [file:source.org] source.org:1 -> missing id missing-id"
        )
    );
    assert!(stdout.contains("location: source.org:12:7"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn review_show_command_prints_workflow_finding_payloads() -> Result<()> {
    let (_workspace, root, db) = build_review_fixture()?;

    let output = review_id_command(&root, &db, "show", "review/workflow/context", false)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("review id: review/workflow/context"));
    assert!(stdout.contains("kind: workflow"));
    assert!(stdout.contains("workflow: Context Workflow [workflow/test/context]"));
    assert!(stdout.contains("- workflow-step/resolve-focus [workflow-step]"));
    assert!(stdout.contains("status: reviewed"));
    assert!(stdout.contains("[step resolve-focus]"));
    assert!(stdout.contains("kind: resolve"));
    assert!(stdout.contains("node key: heading:focus.org:3"));
    assert!(stdout.contains("title: Focus Node"));
    assert!(stdout.contains("tags: review"));
    assert!(stdout.contains("todo: TODO"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn review_delete_command_acknowledges_and_removes_reviews() -> Result<()> {
    let (_workspace, root, db) = build_review_fixture()?;

    let delete = review_id_command(&root, &db, "delete", "review/audit/dangling", true)?;

    assert!(delete.status.success(), "{delete:?}");
    let parsed: DeleteReviewRunResult = serde_json::from_slice(&delete.stdout)?;
    assert_eq!(parsed.review_id, "review/audit/dangling");

    let listed = review_list_command(&root, &db, true)?;
    assert!(listed.status.success(), "{listed:?}");
    let parsed_list: ListReviewRunsResult = serde_json::from_slice(&listed.stdout)?;
    assert_eq!(parsed_list.reviews.len(), 1);
    assert_eq!(
        parsed_list.reviews[0].metadata.review_id,
        "review/workflow/context"
    );

    Ok(())
}

#[test]
fn review_delete_command_prints_human_acknowledgement() -> Result<()> {
    let (_workspace, root, db) = build_review_fixture()?;

    let output = review_id_command(&root, &db, "delete", "review/workflow/context", false)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert_eq!(stdout, "deleted review: review/workflow/context\n");
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn review_show_command_reports_missing_reviews() -> Result<()> {
    let (_workspace, root, db) = build_review_fixture()?;

    let output = review_id_command(&root, &db, "show", "review/missing", true)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("unknown review run: review/missing")
    );

    Ok(())
}

#[test]
fn review_delete_command_reports_invalid_review_ids() -> Result<()> {
    let (_workspace, root, db) = build_review_fixture()?;

    let output = review_id_command(&root, &db, "delete", " review/audit/dangling ", true)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("review_id must not have leading or trailing whitespace")
    );

    Ok(())
}
