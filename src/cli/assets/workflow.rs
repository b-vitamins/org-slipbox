use super::super::output::{
    CliCommandError, OutputMode, ReportFormat, ReportOutputArgs, render_report_bytes, write_output,
    write_report_destination,
};
use super::super::render::{
    assets::{render_workflow_execution_result, render_workflow_list, render_workflow_spec},
    reviews::render_saved_review_summary,
};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, SaveReviewArgs, ScopeArgs, parse_workflow_input_assignments,
    run_daemon_operation, run_headless_command,
};
use anyhow::{Context, Result};
use clap::{ArgGroup, Args, Subcommand};
use serde::Serialize;
use slipbox_core::{
    ListWorkflowsResult, ReviewRunSummary, RunWorkflowParams, RunWorkflowResult,
    SaveWorkflowReviewParams, SaveWorkflowReviewResult, WorkflowExecutionResult, WorkflowIdParams,
    WorkflowSpec, WorkflowSpecCompatibilityEnvelope, WorkflowSummary,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

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
    /// Run a workflow through daemon-owned execution.
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
    /// Org source root when showing a catalog workflow through the daemon.
    #[arg(long, value_name = "ROOT")]
    pub(crate) root: Option<PathBuf>,
    /// Derived SQLite index path when showing a catalog workflow through the daemon.
    #[arg(long, value_name = "DB")]
    pub(crate) db: Option<PathBuf>,
    /// Extra directory containing workflow spec JSON files.
    #[arg(long = "workflow-dir", value_name = "DIR")]
    pub(crate) workflow_dirs: Vec<PathBuf>,
    /// File extension eligible for discovery and indexing.
    #[arg(long = "file-extension", value_name = "EXT")]
    pub(crate) file_extensions: Vec<String>,
    /// Relative-path regular expression to exclude from discovery.
    #[arg(long = "exclude-regexp", value_name = "REGEXP")]
    pub(crate) exclude_regexps: Vec<String>,
    /// Executable used to spawn `slipbox serve`.
    #[arg(long, value_name = "PATH")]
    pub(crate) server_program: Option<PathBuf>,
    /// Emit stable JSON to stdout and structured JSON errors to stderr.
    #[arg(long)]
    pub(crate) json: bool,
    /// Built-in workflow identifier to inspect through the daemon.
    #[arg(group = "workflow-source", value_name = "WORKFLOW_ID")]
    pub(crate) workflow_id: Option<String>,
    /// Local workflow spec JSON path, or `-` for stdin. Does not start the daemon.
    #[arg(long, group = "workflow-source", value_name = "PATH")]
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
    /// Workflow identifier to run.
    #[arg(value_name = "WORKFLOW_ID")]
    pub(crate) workflow_id: String,
    /// Workflow input assignment as `input-id=kind:value` where kind is `id`, `title`, `ref`, or `key`.
    #[arg(long = "input", value_name = "INPUT=KIND:VALUE")]
    pub(crate) inputs: Vec<String>,
    #[command(flatten)]
    pub(crate) report: ReportOutputArgs,
    #[command(flatten)]
    pub(crate) save_review: SaveReviewArgs,
}

pub(crate) fn run_workflow(args: &WorkflowArgs) -> Result<(), CliCommandError> {
    match &args.command {
        WorkflowCommand::List(command) => run_headless_command(command),
        WorkflowCommand::Show(command) => run_workflow_show(command),
        WorkflowCommand::Run(command) => run_workflow_command(command),
    }
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
        let workflow = run_daemon_operation(&headless, output_mode, |client| {
            client.workflow(&WorkflowIdParams { workflow_id })
        })?;
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
    if let Some(save_review) = save_review {
        let saved = run_daemon_operation(&command.headless, error_output_mode, |client| {
            client.save_workflow_review(&SaveWorkflowReviewParams {
                workflow_id: command.workflow_id.clone(),
                inputs: parse_workflow_input_assignments(&command.inputs)?,
                review_id: save_review.review_id,
                title: save_review.title,
                summary: save_review.summary,
                overwrite: save_review.overwrite,
            })
        })?;
        return write_saved_workflow_command_output(
            command,
            report_format,
            saved,
            error_output_mode,
        );
    }

    let result = run_daemon_operation(&command.headless, error_output_mode, |client| {
        command.execute(client)
    })?;

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
