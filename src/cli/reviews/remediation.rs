use super::super::output::{CliCommandError, write_output};
use super::super::render::reviews::{
    render_review_remediation_application, render_review_remediation_preview,
};
use super::super::runtime::{HeadlessArgs, HeadlessCommand, run_daemon_task, run_headless_command};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{
    ReviewFindingRemediationApplyParams, ReviewFindingRemediationPreviewParams,
    ReviewFindingRemediationPreviewResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::io;

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewRemediationArgs {
    #[command(subcommand)]
    pub(crate) command: ReviewRemediationCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReviewRemediationCommand {
    /// Inspect the daemon-owned remediation preview for one finding.
    Preview(ReviewRemediationPreviewArgs),
    /// Apply one supported remediation action after explicit confirmation.
    #[command(
        long_about = "Apply one supported remediation action. The daemon revalidates the saved preview against current file contents before writing; use preview first when unsure."
    )]
    Apply(ReviewRemediationApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewFindingIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review run identifier.
    #[arg(value_name = "REVIEW_ID")]
    pub(crate) review_id: String,
    /// Typed durable finding identifier within the review run.
    #[arg(value_name = "FINDING_ID")]
    pub(crate) finding_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewRemediationPreviewArgs {
    #[command(flatten)]
    pub(crate) finding: ReviewFindingIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewRemediationApplyArgs {
    #[command(flatten)]
    pub(crate) finding: ReviewFindingIdArgs,
    /// Confirm applying the supported unlink-dangling-link remediation.
    #[arg(long)]
    pub(crate) confirm_unlink_dangling_link: bool,
    /// Replacement text for the removed id link. Defaults to the current link label.
    #[arg(long, value_name = "TEXT")]
    pub(crate) replacement_text: Option<String>,
}

pub(crate) fn run_review_remediation(
    command: &ReviewRemediationArgs,
) -> Result<(), CliCommandError> {
    match &command.command {
        ReviewRemediationCommand::Preview(command) => run_headless_command(command),
        ReviewRemediationCommand::Apply(command) => run_review_remediation_apply(command),
    }
}

impl HeadlessCommand for ReviewRemediationPreviewArgs {
    type Output = ReviewFindingRemediationPreviewResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.finding.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.review_finding_remediation_preview(&ReviewFindingRemediationPreviewParams {
            review_id: self.finding.review_id.clone(),
            finding_id: self.finding.finding_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_remediation_preview(&output.preview)
    }
}

fn run_review_remediation_apply(
    command: &ReviewRemediationApplyArgs,
) -> Result<(), CliCommandError> {
    let output_mode = command.finding.headless.output_mode();
    if !command.confirm_unlink_dangling_link {
        return Err(CliCommandError::new(
            output_mode,
            anyhow::anyhow!("review remediation apply requires --confirm-unlink-dangling-link"),
        ));
    }

    let output = run_daemon_task(&command.finding.headless, output_mode, |client| {
        let preview = client
            .review_finding_remediation_preview(&ReviewFindingRemediationPreviewParams {
                review_id: command.finding.review_id.clone(),
                finding_id: command.finding.finding_id.clone(),
            })?
            .preview;
        let action = preview
            .default_apply_action(command.replacement_text.as_deref())
            .map_err(anyhow::Error::msg)?;
        client
            .review_finding_remediation_apply(&ReviewFindingRemediationApplyParams {
                review_id: command.finding.review_id.clone(),
                finding_id: command.finding.finding_id.clone(),
                expected_preview: preview.preview_identity,
                action,
            })
            .map_err(Into::into)
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        render_review_remediation_application(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}
