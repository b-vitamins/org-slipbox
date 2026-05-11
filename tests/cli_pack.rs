use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use serde::Deserialize;
use slipbox_core::{
    BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID, DeleteWorkbenchPackResult, ImportWorkbenchPackResult,
    ListWorkbenchPacksResult, ValidateWorkbenchPackResult, WorkbenchPackCompatibility,
    WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackMetadata, WorkbenchPackResult,
    WorkflowCatalogIssueKind, built_in_workflow,
};
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

fn build_empty_fixture() -> Result<(tempfile::TempDir, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
    ))
}

fn sample_pack(pack_id: &str, title: &str, workflow_id: &str) -> WorkbenchPackManifest {
    let mut workflow =
        built_in_workflow(BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID).expect("built-in should exist");
    workflow.metadata.workflow_id = workflow_id.to_owned();
    workflow.metadata.title = format!("{title} Workflow");
    workflow.metadata.summary = Some("Pack workflow fixture".to_owned());
    WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: pack_id.to_owned(),
            title: title.to_owned(),
            summary: Some("Pack fixture".to_owned()),
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: vec![workflow],
        review_routines: Vec::new(),
        report_profiles: Vec::new(),
        entrypoint_routine_ids: Vec::new(),
    }
}

fn write_pack_file(pack: &WorkbenchPackManifest) -> Result<(tempfile::TempDir, String)> {
    let directory = tempdir()?;
    let path = directory.path().join("pack.json");
    fs::write(&path, serde_json::to_vec_pretty(pack)?)?;
    Ok((directory, path.display().to_string()))
}

fn pack_command(root: &str, db: &str, args: &[&str]) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.arg("pack");
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

fn pack_import_command(
    root: &str,
    db: &str,
    input: &str,
    extra_args: &[&str],
    json: bool,
) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "pack",
        "import",
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
    command.args(extra_args);
    command.arg(input);
    Ok(command.output()?)
}

fn pack_export_command(
    root: &str,
    db: &str,
    pack_id: &str,
    extra_args: &[&str],
    json: bool,
) -> Result<std::process::Output> {
    let mut command = Command::new(slipbox_binary());
    command.args([
        "pack",
        "export",
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
    command.arg(pack_id);
    command.args(extra_args);
    Ok(command.output()?)
}

#[test]
fn pack_validate_command_reads_file_and_stdin_without_daemon_scope() -> Result<()> {
    let pack = sample_pack("pack/validate", "Validate Pack", "workflow/pack/validate");
    let (_pack_dir, pack_path) = write_pack_file(&pack)?;

    let output = Command::new(slipbox_binary())
        .args(["pack", "validate", "--json", &pack_path])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    let parsed: ValidateWorkbenchPackResult = serde_json::from_slice(&output.stdout)?;
    assert!(parsed.valid);
    assert_eq!(
        parsed
            .pack
            .as_ref()
            .expect("valid pack should include summary")
            .metadata
            .pack_id,
        "pack/validate"
    );
    assert!(output.stderr.is_empty());

    let mut child = Command::new(slipbox_binary())
        .args(["pack", "validate", "--json", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .context("stdin should be piped")?
        .write_all(&serde_json::to_vec(&pack)?)?;
    drop(child.stdin.take());
    let stdin_output = child.wait_with_output()?;

    assert!(stdin_output.status.success(), "{stdin_output:?}");
    let parsed: ValidateWorkbenchPackResult = serde_json::from_slice(&stdin_output.stdout)?;
    assert!(parsed.valid);
    assert!(stdin_output.stderr.is_empty());

    Ok(())
}

#[test]
fn pack_validate_command_reports_future_versions_before_typed_parse() -> Result<()> {
    let future = serde_json::json!({
        "pack_id": "pack/future",
        "title": "Future Pack",
        "compatibility": { "version": 2 },
        "workflows": [{ "kind": "future-workflow-shape" }]
    });
    let directory = tempdir()?;
    let path = directory.path().join("future.json");
    fs::write(&path, serde_json::to_vec_pretty(&future)?)?;

    let output = Command::new(slipbox_binary())
        .args([
            "pack",
            "validate",
            "--json",
            path.to_str().context("utf-8 path")?,
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    let parsed: ValidateWorkbenchPackResult = serde_json::from_slice(&output.stdout)?;
    assert!(!parsed.valid);
    assert_eq!(
        parsed.issues[0].kind,
        WorkbenchPackIssueKind::UnsupportedVersion
    );
    assert_eq!(parsed.issues[0].asset_id.as_deref(), Some("pack/future"));
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn pack_import_show_export_delete_round_trips_through_daemon() -> Result<()> {
    let (_workspace, root, db) = build_empty_fixture()?;
    let pack = sample_pack(
        "pack/round-trip",
        "Round Trip Pack",
        "workflow/pack/round-trip",
    );
    let (_pack_dir, pack_path) = write_pack_file(&pack)?;

    let imported = pack_import_command(&root, &db, &pack_path, &[], true)?;

    assert!(imported.status.success(), "{imported:?}");
    let parsed: ImportWorkbenchPackResult = serde_json::from_slice(&imported.stdout)?;
    assert_eq!(parsed.pack.metadata.pack_id, "pack/round-trip");
    assert_eq!(parsed.pack.workflow_count, 1);
    assert!(imported.stderr.is_empty());

    let shown = pack_command(&root, &db, &["show", "--json", "pack/round-trip"])?;
    assert!(shown.status.success(), "{shown:?}");
    let parsed: WorkbenchPackResult = serde_json::from_slice(&shown.stdout)?;
    assert_eq!(parsed.pack, pack);
    assert!(shown.stderr.is_empty());

    let exported = pack_export_command(&root, &db, "pack/round-trip", &[], true)?;
    assert!(exported.status.success(), "{exported:?}");
    let parsed_export: WorkbenchPackManifest = serde_json::from_slice(&exported.stdout)?;
    assert_eq!(parsed_export, pack);
    assert!(exported.stderr.is_empty());

    let export_dir = tempdir()?;
    let export_path = export_dir.path().join("pack.json");
    let export_file = pack_export_command(
        &root,
        &db,
        "pack/round-trip",
        &["--output", export_path.to_str().context("utf-8 path")?],
        false,
    )?;
    assert!(export_file.status.success(), "{export_file:?}");
    let stdout = String::from_utf8(export_file.stdout)?;
    assert!(stdout.contains("exported pack: pack/round-trip ->"));
    let parsed_file: WorkbenchPackManifest = serde_json::from_slice(&fs::read(export_path)?)?;
    assert_eq!(parsed_file, pack);

    let deleted = pack_command(&root, &db, &["delete", "--json", "pack/round-trip"])?;
    assert!(deleted.status.success(), "{deleted:?}");
    let parsed: DeleteWorkbenchPackResult = serde_json::from_slice(&deleted.stdout)?;
    assert_eq!(parsed.pack_id, "pack/round-trip");
    assert!(deleted.stderr.is_empty());

    let listed = pack_command(&root, &db, &["list", "--json"])?;
    assert!(listed.status.success(), "{listed:?}");
    let parsed: ListWorkbenchPacksResult = serde_json::from_slice(&listed.stdout)?;
    assert!(parsed.packs.is_empty());
    assert!(parsed.issues.is_empty());

    Ok(())
}

#[test]
fn pack_import_command_enforces_overwrite_policy() -> Result<()> {
    let (_workspace, root, db) = build_empty_fixture()?;
    let mut pack = sample_pack("pack/overwrite", "Original Pack", "workflow/pack/overwrite");
    let (_original_dir, original_path) = write_pack_file(&pack)?;
    let first = pack_import_command(&root, &db, &original_path, &[], true)?;
    assert!(first.status.success(), "{first:?}");

    pack.metadata.title = "Updated Pack".to_owned();
    let (_updated_dir, updated_path) = write_pack_file(&pack)?;
    let conflict = pack_import_command(&root, &db, &updated_path, &[], true)?;

    assert_eq!(conflict.status.code(), Some(1));
    assert!(conflict.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&conflict.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("workbench pack already exists: pack/overwrite")
    );

    let overwrite = pack_import_command(&root, &db, &updated_path, &["--overwrite"], false)?;
    assert!(overwrite.status.success(), "{overwrite:?}");
    let stdout = String::from_utf8(overwrite.stdout)?;
    assert!(stdout.contains("imported pack: pack/overwrite"));

    let shown = pack_command(&root, &db, &["show", "--json", "pack/overwrite"])?;
    let parsed: WorkbenchPackResult = serde_json::from_slice(&shown.stdout)?;
    assert_eq!(parsed.pack.metadata.title, "Updated Pack");

    Ok(())
}

#[test]
fn pack_list_command_reports_imported_packs_and_catalog_issues() -> Result<()> {
    let (_workspace, root, db) = build_empty_fixture()?;
    let first = sample_pack("pack/a", "Pack A", "workflow/pack/shared");
    let second = sample_pack("pack/b", "Pack B", "workflow/pack/shared");
    let (_first_dir, first_path) = write_pack_file(&first)?;
    let (_second_dir, second_path) = write_pack_file(&second)?;

    assert!(
        pack_import_command(&root, &db, &first_path, &[], true)?
            .status
            .success()
    );
    assert!(
        pack_import_command(&root, &db, &second_path, &[], true)?
            .status
            .success()
    );

    let listed = pack_command(&root, &db, &["list", "--json"])?;

    assert!(listed.status.success(), "{listed:?}");
    let parsed: ListWorkbenchPacksResult = serde_json::from_slice(&listed.stdout)?;
    assert_eq!(parsed.packs.len(), 2);
    assert!(
        parsed.issues.iter().any(|issue| {
            issue.kind == WorkflowCatalogIssueKind::DuplicateWorkflowId
                && issue.pack_id.as_deref() == Some("pack/b")
                && issue.workflow_id.as_deref() == Some("workflow/pack/shared")
        }),
        "{:?}",
        parsed.issues
    );

    let human = pack_command(&root, &db, &["list"])?;
    assert!(human.status.success(), "{human:?}");
    let stdout = String::from_utf8(human.stdout)?;
    assert!(stdout.contains("Pack A [pack/a]"));
    assert!(stdout.contains("Pack B [pack/b]"));
    assert!(stdout.contains("[issues]"));
    assert!(stdout.contains("duplicate-workflow-id"));

    Ok(())
}

#[test]
fn pack_import_command_reports_malformed_json() -> Result<()> {
    let (_workspace, root, db) = build_empty_fixture()?;
    let input_dir = tempdir()?;
    let input_path = input_dir.path().join("broken.json");
    fs::write(&input_path, b"{ not valid json")?;

    let output = pack_import_command(
        &root,
        &db,
        input_path.to_str().context("utf-8 path")?,
        &[],
        true,
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("failed to parse workbench pack JSON")
    );

    Ok(())
}

#[test]
fn pack_show_delete_and_export_report_missing_packs() -> Result<()> {
    let (_workspace, root, db) = build_empty_fixture()?;

    for args in [
        ["show", "--json", "pack/missing"].as_slice(),
        ["delete", "--json", "pack/missing"].as_slice(),
        ["export", "--json", "pack/missing"].as_slice(),
    ] {
        let output = pack_command(&root, &db, args)?;
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty());
        let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
        assert!(
            parsed
                .error
                .message
                .contains("unknown workbench pack: pack/missing")
        );
    }

    Ok(())
}
