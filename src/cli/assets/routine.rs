use super::super::output::CliCommandError;
use super::super::render::assets::{
    render_review_routine_execution_result, render_review_routine_list, render_review_routine_spec,
};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, parse_workflow_input_assignments, run_headless_command,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{
    ListReviewRoutinesResult, ReviewRoutineIdParams, ReviewRoutineResult, RunReviewRoutineParams,
    RunReviewRoutineResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

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
    #[arg(value_name = "ROUTINE_ID")]
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
    #[arg(value_name = "ROUTINE_ID")]
    pub(crate) routine_id: String,
    /// Routine input assignment as `input-id=kind:value` where kind is `id`, `title`, `ref`, or `key`.
    #[arg(long = "input", value_name = "INPUT=KIND:VALUE")]
    pub(crate) inputs: Vec<String>,
}

pub(crate) fn run_routine(args: &RoutineArgs) -> Result<(), CliCommandError> {
    match &args.command {
        RoutineCommand::List(command) => run_headless_command(command),
        RoutineCommand::Show(command) => run_headless_command(command),
        RoutineCommand::Run(command) => run_headless_command(command),
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
