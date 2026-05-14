use super::output::{
    CliCommandError, OutputMode, ReportFormat, ReportOutputArgs, render_report_bytes, write_output,
    write_report_destination,
};
use super::render::{
    assets::{
        render_review_routine_execution_result, render_review_routine_list,
        render_review_routine_spec, render_workbench_pack_list, render_workbench_pack_manifest,
        render_workbench_pack_validation, render_workflow_execution_result, render_workflow_list,
        render_workflow_spec,
    },
    reviews::render_saved_review_summary,
};
use super::runtime::{
    HeadlessArgs, HeadlessCommand, SaveReviewArgs, ScopeArgs, parse_workflow_input_assignments,
    run_headless_command,
};
use anyhow::{Context, Result};
use clap::{ArgGroup, Args, Subcommand};
use serde::Serialize;
use slipbox_core::{
    DeleteWorkbenchPackResult, ImportWorkbenchPackParams, ListReviewRoutinesResult,
    ListWorkbenchPacksResult, ListWorkflowsResult, ReviewRoutineIdParams, ReviewRoutineResult,
    ReviewRunSummary, RunReviewRoutineParams, RunReviewRoutineResult, RunWorkflowParams,
    RunWorkflowResult, SaveWorkflowReviewParams, SaveWorkflowReviewResult,
    ValidateWorkbenchPackResult, WorkbenchPackCompatibilityEnvelope, WorkbenchPackIdParams,
    WorkbenchPackIssue, WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackResult,
    WorkbenchPackSummary, WorkflowExecutionResult, WorkflowIdParams, WorkflowSpec,
    WorkflowSpecCompatibilityEnvelope, WorkflowSummary,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct PackExportFileResult {
    pack: WorkbenchPackSummary,
    output_path: String,
}

#[derive(Debug, Serialize)]
struct WorkflowReportOutputResult {
    workflow: WorkflowSummary,
    format: ReportFormat,
    output_path: String,
    step_count: usize,
}

#[derive(Debug, Serialize)]
struct SavedWorkflowReportOutputResult {
    workflow: WorkflowSummary,
    format: ReportFormat,
    output_path: String,
    step_count: usize,
    review: ReviewRunSummary,
}

#[derive(Debug, Serialize)]
struct WorkflowShowFileResult {
    workflow: WorkflowSpec,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct WorkflowArgs {
    #[command(subcommand)]
    pub(crate) command: WorkflowCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum WorkflowCommand {
    /// List available named workflows.
    List(WorkflowListArgs),
    /// Show a built-in workflow or inspect a workflow spec JSON file/stdin.
    Show(WorkflowShowArgs),
    /// Run a built-in workflow over the canonical daemon boundary.
    Run(WorkflowRunArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct WorkflowListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("workflow-source")
        .args(["workflow_id", "spec"])
        .required(true)
        .multiple(false)
))]
pub(crate) struct WorkflowShowArgs {
    /// Root directory containing Org files when showing a built-in workflow through the daemon.
    #[arg(long)]
    pub(crate) root: Option<PathBuf>,
    /// SQLite database path when showing a built-in workflow through the daemon.
    #[arg(long)]
    pub(crate) db: Option<PathBuf>,
    /// Directories containing declarative workflow spec JSON files.
    #[arg(long = "workflow-dir")]
    pub(crate) workflow_dirs: Vec<PathBuf>,
    /// File extensions eligible for discovery and indexing.
    #[arg(long = "file-extension")]
    pub(crate) file_extensions: Vec<String>,
    /// Relative-path regular expressions to exclude from discovery.
    #[arg(long = "exclude-regexp")]
    pub(crate) exclude_regexps: Vec<String>,
    /// Path to the slipbox executable used to spawn `slipbox serve`.
    #[arg(long)]
    pub(crate) server_program: Option<PathBuf>,
    /// Emit structured JSON to stdout and structured errors to stderr.
    #[arg(long)]
    pub(crate) json: bool,
    /// Built-in workflow identifier to inspect through the daemon.
    #[arg(group = "workflow-source")]
    pub(crate) workflow_id: Option<String>,
    /// Read workflow spec JSON from this path, or `-` for stdin.
    #[arg(long, group = "workflow-source")]
    pub(crate) spec: Option<String>,
}

impl WorkflowShowArgs {
    #[must_use]
    fn output_mode(&self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }

    fn headless_args(&self) -> Result<HeadlessArgs> {
        let root = self
            .root
            .clone()
            .context("workflow show for built-in workflows requires --root and --db")?;
        let db = self
            .db
            .clone()
            .context("workflow show for built-in workflows requires --root and --db")?;
        Ok(HeadlessArgs {
            scope: ScopeArgs {
                root,
                db,
                workflow_dirs: self.workflow_dirs.clone(),
                file_extensions: self.file_extensions.clone(),
                exclude_regexps: self.exclude_regexps.clone(),
            },
            server_program: self.server_program.clone(),
            json: self.json,
        })
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct WorkflowRunArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Built-in workflow identifier to run.
    pub(crate) workflow_id: String,
    /// Workflow input assignment as `input-id=kind:value` where kind is `id`, `title`, `ref`, or `key`.
    #[arg(long = "input")]
    pub(crate) inputs: Vec<String>,
    #[command(flatten)]
    pub(crate) report: ReportOutputArgs,
    #[command(flatten)]
    pub(crate) save_review: SaveReviewArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineArgs {
    #[command(subcommand)]
    pub(crate) command: RoutineCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum RoutineCommand {
    /// List available review routines.
    List(RoutineListArgs),
    /// Show a review routine definition.
    Show(RoutineShowArgs),
    /// Run a review routine through daemon-owned semantics.
    Run(RoutineRunArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review routine identifier.
    pub(crate) routine_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineShowArgs {
    #[command(flatten)]
    pub(crate) routine: RoutineIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineRunArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Review routine identifier to run.
    pub(crate) routine_id: String,
    /// Routine input assignment as `input-id=kind:value` where kind is `id`, `title`, `ref`, or `key`.
    #[arg(long = "input")]
    pub(crate) inputs: Vec<String>,
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
    pub(crate) pack_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackShowArgs {
    #[command(flatten)]
    pub(crate) pack: PackIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackValidateArgs {
    /// Read workbench pack JSON from this path, or `-` for stdin.
    #[arg(default_value = "-")]
    pub(crate) input: String,
    /// Emit structured JSON to stdout and structured errors to stderr.
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
    #[arg(default_value = "-")]
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
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackDeleteArgs {
    #[command(flatten)]
    pub(crate) pack: PackIdArgs,
}

pub(crate) fn run_workflow(args: &WorkflowArgs) -> Result<(), CliCommandError> {
    match &args.command {
        WorkflowCommand::List(command) => run_headless_command(command),
        WorkflowCommand::Show(command) => run_workflow_show(command),
        WorkflowCommand::Run(command) => run_workflow_command(command),
    }
}

pub(crate) fn run_routine(args: &RoutineArgs) -> Result<(), CliCommandError> {
    match &args.command {
        RoutineCommand::List(command) => run_headless_command(command),
        RoutineCommand::Show(command) => run_headless_command(command),
        RoutineCommand::Run(command) => run_headless_command(command),
    }
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
    let mut client = command.headless.connect()?;
    let imported = client
        .import_workbench_pack(&ImportWorkbenchPackParams {
            pack,
            overwrite: command.overwrite,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

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
    let mut client = command.pack.headless.connect()?;
    let pack = client
        .export_workbench_pack(&WorkbenchPackIdParams {
            pack_id: command.pack.pack_id.clone(),
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    if let Some(output_path) = &command.output
        && output_path != Path::new("-")
    {
        let serialized = serde_json::to_vec_pretty(&pack)
            .context("failed to serialize workbench pack")
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        fs::write(output_path, serialized)
            .with_context(|| {
                format!(
                    "failed to write exported workbench pack {}",
                    output_path.display()
                )
            })
            .map_err(|error| CliCommandError::new(output_mode, error))?;

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

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    match output_mode {
        OutputMode::Human => {
            serde_json::to_writer_pretty(&mut writer, &pack)
                .context("failed to serialize workbench pack")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
            writer
                .write_all(b"\n")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
        }
        OutputMode::Json => {
            serde_json::to_writer(&mut writer, &pack)
                .context("failed to serialize workbench pack")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
            writer
                .write_all(b"\n")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
        }
    }
    writer
        .flush()
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    Ok(())
}

fn run_workflow_show(command: &WorkflowShowArgs) -> Result<(), CliCommandError> {
    let output_mode = command.output_mode();
    let workflow = if let Some(spec_input) = &command.spec {
        let bytes = read_workflow_json_input(spec_input)
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        let compatibility: WorkflowSpecCompatibilityEnvelope = serde_json::from_slice(&bytes)
            .with_context(|| {
                if spec_input == "-" {
                    "failed to parse workflow spec JSON from stdin".to_owned()
                } else {
                    format!("failed to parse workflow spec JSON from {spec_input}")
                }
            })
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        if let Some(message) = compatibility.compatibility.validation_error() {
            return Err(CliCommandError::new(
                output_mode,
                anyhow::anyhow!("invalid workflow spec: {message}"),
            ));
        }
        let workflow: WorkflowSpec = serde_json::from_slice(&bytes)
            .with_context(|| {
                if spec_input == "-" {
                    "failed to parse workflow spec JSON from stdin".to_owned()
                } else {
                    format!("failed to parse workflow spec JSON from {spec_input}")
                }
            })
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        if let Some(message) = workflow.validation_error() {
            return Err(CliCommandError::new(
                output_mode,
                anyhow::anyhow!("invalid workflow spec: {message}"),
            ));
        }
        WorkflowShowFileResult { workflow }
    } else {
        let workflow_id = command
            .workflow_id
            .clone()
            .expect("clap enforces workflow source selection");
        let headless = command
            .headless_args()
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        let mut client = headless.connect()?;
        let workflow = client
            .workflow(&WorkflowIdParams { workflow_id })
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        client
            .shutdown()
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        WorkflowShowFileResult {
            workflow: workflow.workflow,
        }
    };

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &workflow, |value| {
        render_workflow_spec(&value.workflow)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
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

fn read_workflow_json_input(input: &str) -> Result<Vec<u8>> {
    if input == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read workflow spec JSON from stdin")?;
        return Ok(bytes);
    }

    fs::read(input).with_context(|| format!("failed to read workflow spec JSON from {input}"))
}

impl HeadlessCommand for WorkflowListArgs {
    type Output = ListWorkflowsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_workflows()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_workflow_list(output)
    }
}

impl HeadlessCommand for WorkflowRunArgs {
    type Output = RunWorkflowResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.run_workflow(&RunWorkflowParams {
            workflow_id: self.workflow_id.clone(),
            inputs: parse_workflow_input_assignments(&self.inputs)?,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_workflow_execution_result(&output.result)
    }
}

impl HeadlessCommand for RoutineListArgs {
    type Output = ListReviewRoutinesResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_review_routines()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_routine_list(output)
    }
}

impl HeadlessCommand for RoutineShowArgs {
    type Output = ReviewRoutineResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.routine.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.review_routine(&ReviewRoutineIdParams {
            routine_id: self.routine.routine_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_routine_spec(&output.routine)
    }
}

impl HeadlessCommand for RoutineRunArgs {
    type Output = RunReviewRoutineResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.run_review_routine(&RunReviewRoutineParams {
            routine_id: self.routine_id.clone(),
            inputs: parse_workflow_input_assignments(&self.inputs)?,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_routine_execution_result(&output.result)
    }
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

fn run_workflow_command(command: &WorkflowRunArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let report_format = command
        .report
        .format(output_mode)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let error_output_mode = report_format.error_output_mode();
    let save_review = command
        .save_review
        .request()
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    let mut client = command
        .headless
        .connect_with_output_mode(error_output_mode)?;

    if let Some(save_review) = save_review {
        let saved = client
            .save_workflow_review(&SaveWorkflowReviewParams {
                workflow_id: command.workflow_id.clone(),
                inputs: parse_workflow_input_assignments(&command.inputs)
                    .map_err(|error| CliCommandError::new(error_output_mode, error))?,
                review_id: save_review.review_id,
                title: save_review.title,
                summary: save_review.summary,
                overwrite: save_review.overwrite,
            })
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        client
            .shutdown()
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        return write_saved_workflow_command_output(
            command,
            report_format,
            saved,
            error_output_mode,
        );
    }

    let result = command
        .execute(&mut client)
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;

    let report_bytes = render_report_bytes(
        report_format,
        &result,
        |value| command.render_human(value),
        |value| value.result.report_lines(),
    )
    .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    write_report_destination(&report_bytes, command.report.output_path())
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;

    if let Some(output_path) = command.report.output_path() {
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let ack = WorkflowReportOutputResult {
            workflow: result.result.workflow.clone(),
            format: report_format,
            output_path: output_path.display().to_string(),
            step_count: result.result.steps.len(),
        };
        write_output(
            &mut writer,
            report_format.ack_output_mode(),
            &ack,
            |value| {
                format!(
                    "wrote workflow report: {} -> {} ({})\n",
                    value.workflow.metadata.workflow_id,
                    value.output_path,
                    value.format.label(),
                )
            },
        )
        .map_err(|error| CliCommandError::new(report_format.ack_output_mode(), error))?;
    }
    Ok(())
}

fn write_saved_workflow_command_output(
    command: &WorkflowRunArgs,
    report_format: ReportFormat,
    saved: SaveWorkflowReviewResult,
    error_output_mode: OutputMode,
) -> Result<(), CliCommandError> {
    if let Some(output_path) = command.report.output_path() {
        let report_bytes = render_report_bytes(
            report_format,
            &saved.result,
            render_workflow_execution_result,
            WorkflowExecutionResult::report_lines,
        )
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        write_report_destination(&report_bytes, Some(output_path))
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;

        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let ack = SavedWorkflowReportOutputResult {
            workflow: saved.result.workflow.clone(),
            format: report_format,
            output_path: output_path.display().to_string(),
            step_count: saved.result.steps.len(),
            review: saved.review,
        };
        return write_output(
            &mut writer,
            report_format.ack_output_mode(),
            &ack,
            |value| {
                let mut output = format!(
                    "wrote workflow report: {} -> {} ({})\n",
                    value.workflow.metadata.workflow_id,
                    value.output_path,
                    value.format.label(),
                );
                output.push_str(&render_saved_review_summary(&value.review));
                output
            },
        )
        .map_err(|error| CliCommandError::new(report_format.ack_output_mode(), error));
    }

    match report_format {
        ReportFormat::Human => {
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            write_output(&mut writer, OutputMode::Human, &saved, |value| {
                let mut output = render_workflow_execution_result(&value.result);
                output.push('\n');
                output.push_str(&render_saved_review_summary(&value.review));
                output
            })
            .map_err(|error| CliCommandError::new(OutputMode::Human, error))
        }
        ReportFormat::Json => {
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            write_output(&mut writer, OutputMode::Json, &saved, |_| String::new())
                .map_err(|error| CliCommandError::new(OutputMode::Json, error))
        }
        ReportFormat::Jsonl => {
            let report_bytes = render_report_bytes(
                report_format,
                &saved.result,
                render_workflow_execution_result,
                WorkflowExecutionResult::report_lines,
            )
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
            write_report_destination(&report_bytes, None)
                .map_err(|error| CliCommandError::new(error_output_mode, error))
        }
    }
}
