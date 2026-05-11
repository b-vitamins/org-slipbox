use std::fs;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;
use slipbox_core::{
    BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID, BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID,
    CorpusAuditKind, ListReviewRoutinesResult, ReportJsonlLineKind, ReportProfileMetadata,
    ReportProfileMode, ReportProfileSpec, ReportProfileSubject, ReviewFindingStatus,
    ReviewRoutineComparePolicy, ReviewRoutineCompareTarget, ReviewRoutineMetadata,
    ReviewRoutineResult, ReviewRoutineSaveReviewPolicy, ReviewRoutineSource, ReviewRoutineSpec,
    ReviewRunDiffBucket, ReviewRunResult, RunReviewRoutineResult, WorkbenchPackCompatibility,
    WorkbenchPackManifest, WorkbenchPackMetadata, WorkbenchPackResult, WorkflowCatalogIssueKind,
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

fn build_indexed_fixture() -> Result<(tempfile::TempDir, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("focus.org"),
        r#":PROPERTIES:
:ID: focus-id
:ROAM_REFS: cite:focus2024
:END:
#+title: Focus

Links to [[id:peer-id][Peer]].
"#,
    )?;
    fs::write(
        root.join("duplicate-a.org"),
        r#":PROPERTIES:
:ID: dup-a-id
:END:
#+title: Shared Title
"#,
    )?;
    fs::write(
        root.join("duplicate-b.org"),
        r#":PROPERTIES:
:ID: dup-b-id
:END:
#+title: shared title
"#,
    )?;
    fs::write(
        root.join("peer.org"),
        r#":PROPERTIES:
:ID: peer-id
:ROAM_REFS: cite:focus2024
:END:
#+title: Peer

Links back to [[id:focus-id][Focus]].
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

fn routine_command(root: &str, db: &str, args: &[&str]) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.arg("routine");
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
    command.arg("audit");
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

fn pack_import_command(root: &str, db: &str, pack: &WorkbenchPackManifest) -> Result<()> {
    let directory = tempdir()?;
    let path = directory.path().join("pack.json");
    fs::write(&path, serde_json::to_vec_pretty(pack)?)?;
    let output = Command::new(slipbox_binary())
        .args([
            "pack",
            "import",
            "--json",
            path.to_str().context("utf-8 path")?,
            "--root",
            root,
            "--db",
            db,
            "--server-program",
            slipbox_binary(),
        ])
        .output()?;
    assert!(output.status.success(), "{output:?}");
    let parsed: WorkbenchPackResult = serde_json::from_slice(
        &Command::new(slipbox_binary())
            .args([
                "pack",
                "show",
                "--json",
                &pack.metadata.pack_id,
                "--root",
                root,
                "--db",
                db,
                "--server-program",
                slipbox_binary(),
            ])
            .output()?
            .stdout,
    )?;
    assert_eq!(parsed.pack.metadata.pack_id, pack.metadata.pack_id);
    Ok(())
}

fn sample_routine_pack(pack_id: &str, routine_id: &str, review_id: &str) -> WorkbenchPackManifest {
    WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: pack_id.to_owned(),
            title: "Routine Pack".to_owned(),
            summary: Some("Imported routine fixture".to_owned()),
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: Vec::new(),
        review_routines: vec![ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: routine_id.to_owned(),
                title: "Imported Duplicate Title Review".to_owned(),
                summary: Some("Review duplicate title pressure".to_owned()),
            },
            source: ReviewRoutineSource::Audit {
                audit: CorpusAuditKind::DuplicateTitles,
                limit: 20,
            },
            inputs: Vec::new(),
            save_review: ReviewRoutineSaveReviewPolicy {
                enabled: true,
                review_id: Some(review_id.to_owned()),
                title: Some("Imported Duplicate Title Review".to_owned()),
                summary: Some("Saved by imported routine".to_owned()),
                overwrite: false,
            },
            compare: Some(ReviewRoutineComparePolicy {
                target: ReviewRoutineCompareTarget::LatestCompatibleReview,
                report_profile_id: Some("profile/routine/diff".to_owned()),
            }),
            report_profile_ids: vec!["profile/routine/detail".to_owned()],
        }],
        report_profiles: vec![
            ReportProfileSpec {
                metadata: ReportProfileMetadata {
                    profile_id: "profile/routine/detail".to_owned(),
                    title: "Routine Detail".to_owned(),
                    summary: None,
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
                    profile_id: "profile/routine/diff".to_owned(),
                    title: "Routine Diff".to_owned(),
                    summary: None,
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

#[test]
fn routine_list_and_show_cover_built_ins_and_imported_routines() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;
    let pack = sample_routine_pack(
        "pack/routine-cli",
        "routine/pack/duplicate-title-review",
        "review/routine/imported-current",
    );
    pack_import_command(&root, &db, &pack)?;

    let listed = routine_command(&root, &db, &["list", "--json"])?;

    assert!(listed.status.success(), "{listed:?}");
    let parsed: ListReviewRoutinesResult = serde_json::from_slice(&listed.stdout)?;
    let routine_ids = parsed
        .routines
        .iter()
        .map(|routine| routine.metadata.routine_id.as_str())
        .collect::<Vec<_>>();
    assert!(routine_ids.contains(&BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID));
    assert!(routine_ids.contains(&BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID));
    assert!(routine_ids.contains(&"routine/pack/duplicate-title-review"));
    assert!(parsed.issues.is_empty());
    assert!(listed.stderr.is_empty());

    let human = routine_command(&root, &db, &["list"])?;
    assert!(human.status.success(), "{human:?}");
    let stdout = String::from_utf8(human.stdout)?;
    assert!(stdout.contains("Context Sweep Review [routine/builtin/context-sweep-review]"));
    assert!(
        stdout.contains("Imported Duplicate Title Review [routine/pack/duplicate-title-review]")
    );

    let shown = routine_command(
        &root,
        &db,
        &["show", "--json", "routine/pack/duplicate-title-review"],
    )?;
    assert!(shown.status.success(), "{shown:?}");
    let parsed: ReviewRoutineResult = serde_json::from_slice(&shown.stdout)?;
    assert_eq!(
        parsed.routine.metadata.routine_id,
        "routine/pack/duplicate-title-review"
    );
    assert!(matches!(
        parsed.routine.source,
        ReviewRoutineSource::Audit {
            audit: CorpusAuditKind::DuplicateTitles,
            ..
        }
    ));

    Ok(())
}

#[test]
fn routine_run_executes_imported_routines_with_save_compare_and_profiles() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;
    let base = audit_command(
        &root,
        &db,
        &[
            "duplicate-titles",
            "--json",
            "--limit",
            "20",
            "--save-review",
            "--review-id",
            "review/routine/imported-base",
        ],
    )?;
    assert!(base.status.success(), "{base:?}");

    let pack = sample_routine_pack(
        "pack/routine-cli",
        "routine/pack/duplicate-title-review",
        "review/routine/imported-current",
    );
    pack_import_command(&root, &db, &pack)?;

    let run = routine_command(
        &root,
        &db,
        &["run", "--json", "routine/pack/duplicate-title-review"],
    )?;

    assert!(run.status.success(), "{run:?}");
    let parsed: RunReviewRoutineResult = serde_json::from_slice(&run.stdout)?;
    assert_eq!(
        parsed.result.routine.metadata.routine_id,
        "routine/pack/duplicate-title-review"
    );
    assert_eq!(
        parsed
            .result
            .saved_review
            .as_ref()
            .expect("routine should save review")
            .metadata
            .review_id,
        "review/routine/imported-current"
    );
    assert_eq!(
        parsed
            .result
            .compare
            .as_ref()
            .expect("routine should compare")
            .base_review
            .as_ref()
            .expect("compatible base review should be selected")
            .metadata
            .review_id,
        "review/routine/imported-base"
    );
    assert!(
        parsed
            .result
            .reports
            .iter()
            .any(|report| report.profile.metadata.profile_id == "profile/routine/detail")
    );
    assert!(run.stderr.is_empty());

    let shown = Command::new(slipbox_binary())
        .args([
            "review",
            "show",
            "--json",
            "review/routine/imported-current",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            slipbox_binary(),
        ])
        .output()?;
    assert!(shown.status.success(), "{shown:?}");
    let review: ReviewRunResult = serde_json::from_slice(&shown.stdout)?;
    assert_eq!(
        review.review.metadata.review_id,
        "review/routine/imported-current"
    );

    Ok(())
}

#[test]
fn routine_run_prints_human_built_in_workflow_routine_output() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let run = routine_command(
        &root,
        &db,
        &[
            "run",
            BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID,
            "--input",
            "focus=id:focus-id",
        ],
    )?;

    assert!(run.status.success(), "{run:?}");
    let stdout = String::from_utf8(run.stdout)?;
    assert!(
        stdout.contains("routine: Context Sweep Review [routine/builtin/context-sweep-review]")
    );
    assert!(stdout.contains("workflow: Context Sweep [workflow/builtin/context-sweep]"));
    assert!(stdout.contains("saved review:"));
    assert!(run.stderr.is_empty());

    Ok(())
}

#[test]
fn routine_commands_report_structured_json_errors() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;

    let missing_input = routine_command(
        &root,
        &db,
        &["run", "--json", BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID],
    )?;
    assert_eq!(missing_input.status.code(), Some(1));
    let parsed: ErrorPayload = serde_json::from_slice(&missing_input.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("workflow input focus must be assigned")
    );

    let missing_show = routine_command(&root, &db, &["show", "--json", "routine/missing"])?;
    assert_eq!(missing_show.status.code(), Some(1));
    let parsed: ErrorPayload = serde_json::from_slice(&missing_show.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("unknown review routine: routine/missing")
    );

    let pack = sample_routine_pack(
        "pack/routine-cli",
        "routine/pack/duplicate-title-review",
        "review/routine/conflict",
    );
    pack_import_command(&root, &db, &pack)?;
    let first = routine_command(
        &root,
        &db,
        &["run", "--json", "routine/pack/duplicate-title-review"],
    )?;
    assert!(first.status.success(), "{first:?}");
    let conflict = routine_command(
        &root,
        &db,
        &["run", "--json", "routine/pack/duplicate-title-review"],
    )?;
    assert_eq!(conflict.status.code(), Some(1));
    let parsed: ErrorPayload = serde_json::from_slice(&conflict.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("review run already exists: review/routine/conflict")
    );

    Ok(())
}

#[test]
fn routine_list_reports_imported_routine_catalog_issues() -> Result<()> {
    let (_workspace, root, db) = build_indexed_fixture()?;
    let first = sample_routine_pack("pack/routine-a", "routine/pack/shared", "review/routine/a");
    let second = sample_routine_pack("pack/routine-b", "routine/pack/shared", "review/routine/b");
    pack_import_command(&root, &db, &first)?;
    pack_import_command(&root, &db, &second)?;

    let listed = routine_command(&root, &db, &["list", "--json"])?;

    assert!(listed.status.success(), "{listed:?}");
    let parsed: ListReviewRoutinesResult = serde_json::from_slice(&listed.stdout)?;
    assert!(parsed.issues.iter().any(|issue| {
        issue.kind == WorkflowCatalogIssueKind::DuplicateReviewRoutineId
            && issue.pack_id.as_deref() == Some("pack/routine-b")
            && issue.routine_id.as_deref() == Some("routine/pack/shared")
    }));

    Ok(())
}
