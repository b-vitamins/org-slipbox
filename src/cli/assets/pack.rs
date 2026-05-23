use super::super::output::{CliCommandError, OutputMode, write_json_export, write_output};
use super::super::render::assets::{
    render_workbench_pack_list, render_workbench_pack_manifest, render_workbench_pack_validation,
};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, run_daemon_operation, run_headless_command,
};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;
use slipbox_core::{
    DeleteWorkbenchPackResult, ImportWorkbenchPackParams, ListWorkbenchPacksResult,
    ValidateWorkbenchPackResult, WorkbenchPackCompatibilityEnvelope, WorkbenchPackIdParams,
    WorkbenchPackIssue, WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackResult,
    WorkbenchPackSummary,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Debug, Serialize)]
struct PackExportFileResult {
    pack: WorkbenchPackSummary,
    output_path: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackArgs {
    #[command(subcommand)]
    pub(crate) command: PackCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum PackCommand {
    /// List imported workbench packs and catalog issues.
    List(PackListArgs),
    /// Show an imported workbench pack manifest.
    Show(PackShowArgs),
    /// Validate a local workbench pack JSON file/stdin without daemon state.
    Validate(PackValidateArgs),
    /// Import a workbench pack manifest through the daemon.
    Import(PackImportArgs),
    /// Export an imported workbench pack manifest as stable JSON.
    Export(PackExportArgs),
    /// Delete an imported workbench pack by durable identifier.
    Delete(PackDeleteArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable workbench pack identifier.
    #[arg(value_name = "PACK_ID")]
    pub(crate) pack_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackShowArgs {
    #[command(flatten)]
    pub(crate) pack: PackIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackValidateArgs {
    /// Read workbench pack JSON from this path, or `-` for stdin. Does not start the daemon.
    #[arg(default_value = "-", value_name = "PATH")]
    pub(crate) input: String,
    /// Emit stable JSON to stdout and structured JSON errors to stderr.
    #[arg(long)]
    pub(crate) json: bool,
}

impl PackValidateArgs {
    #[must_use]
    fn output_mode(&self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackImportArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Read imported workbench pack JSON from this path, or `-` for stdin.
    #[arg(default_value = "-", value_name = "PATH")]
    pub(crate) input: String,
    /// Replace an existing pack with the same durable identifier.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackExportArgs {
    #[command(flatten)]
    pub(crate) pack: PackIdArgs,
    /// Write exported JSON to this path instead of stdout. Use `-` for stdout.
    #[arg(long, value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackDeleteArgs {
    #[command(flatten)]
    pub(crate) pack: PackIdArgs,
}

pub(crate) fn run_pack(args: &PackArgs) -> Result<(), CliCommandError> {
    match &args.command {
        PackCommand::List(command) => run_headless_command(command),
        PackCommand::Show(command) => run_headless_command(command),
        PackCommand::Validate(command) => run_pack_validate(command),
        PackCommand::Import(command) => run_pack_import(command),
        PackCommand::Export(command) => run_pack_export(command),
        PackCommand::Delete(command) => run_headless_command(command),
    }
}

fn run_pack_validate(command: &PackValidateArgs) -> Result<(), CliCommandError> {
    let output_mode = command.output_mode();
    let validation = validate_local_workbench_pack(&command.input)
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &validation, |value| {
        render_workbench_pack_validation(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn run_pack_import(command: &PackImportArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let pack = read_workbench_pack_manifest(&command.input)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let imported = run_daemon_operation(&command.headless, output_mode, |client| {
        client.import_workbench_pack(&ImportWorkbenchPackParams {
            pack,
            overwrite: command.overwrite,
        })
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &imported, |value| {
        format!(
            "imported pack: {} (workflows: {}, routines: {}, profiles: {})\n",
            value.pack.metadata.pack_id,
            value.pack.workflow_count,
            value.pack.review_routine_count,
            value.pack.report_profile_count
        )
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn run_pack_export(command: &PackExportArgs) -> Result<(), CliCommandError> {
    let output_mode = command.pack.headless.output_mode();
    let pack = run_daemon_operation(&command.pack.headless, output_mode, |client| {
        client.export_workbench_pack(&WorkbenchPackIdParams {
            pack_id: command.pack.pack_id.clone(),
        })
    })?;

    if write_json_export(
        output_mode,
        command.output.as_deref(),
        &pack,
        "failed to serialize workbench pack",
        |path| format!("failed to write exported workbench pack {}", path.display()),
    )? {
        let output_path = command
            .output
            .as_deref()
            .expect("json export helper only returns true for a concrete output path");
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let result = PackExportFileResult {
            pack: WorkbenchPackSummary::from(&pack),
            output_path: output_path.display().to_string(),
        };
        write_output(&mut writer, output_mode, &result, |value| {
            format!(
                "exported pack: {} -> {}\n",
                value.pack.metadata.pack_id, value.output_path
            )
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
        return Ok(());
    }
    Ok(())
}

fn read_pack_json_input(input: &str) -> Result<Vec<u8>> {
    if input == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read workbench pack JSON from stdin")?;
        return Ok(bytes);
    }

    fs::read(input).with_context(|| format!("failed to read workbench pack JSON from {input}"))
}

fn pack_json_parse_context(input: &str) -> String {
    if input == "-" {
        "failed to parse workbench pack JSON from stdin".to_owned()
    } else {
        format!("failed to parse workbench pack JSON from {input}")
    }
}

fn validate_local_workbench_pack(input: &str) -> Result<ValidateWorkbenchPackResult> {
    let bytes = read_pack_json_input(input)?;
    let compatibility: WorkbenchPackCompatibilityEnvelope =
        serde_json::from_slice(&bytes).with_context(|| pack_json_parse_context(input))?;
    if let Some(message) = compatibility.compatibility.validation_error() {
        return Ok(ValidateWorkbenchPackResult {
            pack: None,
            valid: false,
            issues: vec![WorkbenchPackIssue {
                kind: WorkbenchPackIssueKind::UnsupportedVersion,
                asset_id: compatibility.pack_id,
                message,
            }],
        });
    }

    let pack: WorkbenchPackManifest =
        serde_json::from_slice(&bytes).with_context(|| pack_json_parse_context(input))?;
    let issues = pack.validation_issues();
    Ok(ValidateWorkbenchPackResult {
        pack: issues.is_empty().then(|| WorkbenchPackSummary::from(&pack)),
        valid: issues.is_empty(),
        issues,
    })
}

fn read_workbench_pack_manifest(input: &str) -> Result<WorkbenchPackManifest> {
    let bytes = read_pack_json_input(input)?;
    let compatibility: WorkbenchPackCompatibilityEnvelope =
        serde_json::from_slice(&bytes).with_context(|| pack_json_parse_context(input))?;
    if let Some(message) = compatibility.compatibility.validation_error() {
        anyhow::bail!("invalid workbench pack: {message}");
    }
    serde_json::from_slice(&bytes).with_context(|| pack_json_parse_context(input))
}

impl HeadlessCommand for PackListArgs {
    type Output = ListWorkbenchPacksResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_workbench_packs()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_workbench_pack_list(output)
    }
}

impl HeadlessCommand for PackShowArgs {
    type Output = WorkbenchPackResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.pack.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.workbench_pack(&WorkbenchPackIdParams {
            pack_id: self.pack.pack_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_workbench_pack_manifest(&output.pack)
    }
}

impl HeadlessCommand for PackDeleteArgs {
    type Output = DeleteWorkbenchPackResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.pack.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.delete_workbench_pack(&WorkbenchPackIdParams {
            pack_id: self.pack.pack_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!("deleted pack: {}\n", output.pack_id)
    }
}
