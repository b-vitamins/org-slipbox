use super::super::output::{CliCommandError, write_output};
use super::super::render::reviews::{
    render_mark_review_finding_result, render_review_diff, render_review_list, render_review_run,
};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, parse_review_finding_status, run_daemon_operation,
    run_headless_command,
};
use super::remediation::{ReviewRemediationArgs, run_review_remediation};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{
    DeleteReviewRunResult, ListReviewRunsResult, MarkReviewFindingParams, ReviewRunDiffParams,
    ReviewRunDiffResult, ReviewRunIdParams, ReviewRunResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::io;

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewArgs {
    #[command(subcommand)]
    pub(crate) command: ReviewCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReviewCommand {
    /// List durable operational review runs.
    List(ReviewListArgs),
    /// Show a durable review run.
    Show(ReviewShowArgs),
    /// Compare two compatible durable review runs.
    Diff(ReviewDiffArgs),
    /// Preview and apply safe remediation actions for review findings.
    Remediation(ReviewRemediationArgs),
    /// Update one durable review finding status.
    Mark(ReviewMarkArgs),
    /// Delete a durable review run by identifier.
    Delete(ReviewDeleteArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review run identifier.
    #[arg(value_name = "REVIEW_ID")]
    pub(crate) review_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewShowArgs {
    #[command(flatten)]
    pub(crate) review: ReviewIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewDiffArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Baseline durable review run identifier.
    #[arg(value_name = "BASE_REVIEW_ID")]
    pub(crate) base_review_id: String,
    /// Target durable review run identifier.
    #[arg(value_name = "TARGET_REVIEW_ID")]
    pub(crate) target_review_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewMarkArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review run identifier.
    #[arg(value_name = "REVIEW_ID")]
    pub(crate) review_id: String,
    /// Typed durable finding identifier within the review run.
    #[arg(value_name = "FINDING_ID")]
    pub(crate) finding_id: String,
    /// New finding status: open, reviewed, dismissed, or accepted.
    #[arg(value_name = "STATUS")]
    pub(crate) status: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewDeleteArgs {
    #[command(flatten)]
    pub(crate) review: ReviewIdArgs,
}

pub(crate) fn run_review(args: &ReviewArgs) -> Result<(), CliCommandError> {
    match &args.command {
        ReviewCommand::List(command) => run_headless_command(command),
        ReviewCommand::Show(command) => run_headless_command(command),
        ReviewCommand::Diff(command) => run_headless_command(command),
        ReviewCommand::Remediation(command) => run_review_remediation(command),
        ReviewCommand::Mark(command) => run_review_mark(command),
        ReviewCommand::Delete(command) => run_headless_command(command),
    }
}

impl HeadlessCommand for ReviewListArgs {
    type Output = ListReviewRunsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_review_runs()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_list(output)
    }
}

impl HeadlessCommand for ReviewShowArgs {
    type Output = ReviewRunResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.review.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.review_run(&ReviewRunIdParams {
            review_id: self.review.review_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_run(&output.review)
    }
}

impl HeadlessCommand for ReviewDiffArgs {
    type Output = ReviewRunDiffResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.diff_review_runs(&ReviewRunDiffParams {
            base_review_id: self.base_review_id.clone(),
            target_review_id: self.target_review_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_diff(&output.diff)
    }
}

impl HeadlessCommand for ReviewDeleteArgs {
    type Output = DeleteReviewRunResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.review.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.delete_review_run(&ReviewRunIdParams {
            review_id: self.review.review_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!("deleted review: {}\n", output.review_id)
    }
}

fn run_review_mark(command: &ReviewMarkArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let status = parse_review_finding_status(&command.status)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let output = run_daemon_operation(&command.headless, output_mode, |client| {
        client.mark_review_finding(&MarkReviewFindingParams {
            review_id: command.review_id.clone(),
            finding_id: command.finding_id.clone(),
            status,
        })
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        render_mark_review_finding_result(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}
