use super::super::output::{CliCommandError, write_output};
use super::super::render::relations::{
    render_slipbox_link_rewrite_application, render_slipbox_link_rewrite_preview,
};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, normalize_edit_file_path, run_daemon_operation,
    run_headless_command,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{
    SlipboxLinkRewriteApplyParams, SlipboxLinkRewritePreviewParams, SlipboxLinkRewritePreviewResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkArgs {
    #[command(subcommand)]
    pub(crate) command: LinkCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum LinkCommand {
    /// Replace exact `slipbox:` Org links with stable `id:` links.
    RewriteSlipbox(LinkRewriteSlipboxArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkRewriteSlipboxArgs {
    #[command(subcommand)]
    pub(crate) command: LinkRewriteSlipboxCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum LinkRewriteSlipboxCommand {
    /// Preview supported `slipbox:` link rewrites in one file.
    Preview(LinkRewriteSlipboxPreviewArgs),
    /// Apply supported `slipbox:` link rewrites in one file after confirmation.
    #[command(
        long_about = "Apply supported `slipbox:` link rewrites in one file. Requires --confirm-replace-slipbox-links and returns changed-file refresh status."
    )]
    Apply(LinkRewriteSlipboxApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkRewriteSlipboxFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to inspect or rewrite, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkRewriteSlipboxPreviewArgs {
    #[command(flatten)]
    pub(crate) target: LinkRewriteSlipboxFileArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkRewriteSlipboxApplyArgs {
    #[command(flatten)]
    pub(crate) target: LinkRewriteSlipboxFileArgs,
    /// Confirm replacing supported `slipbox:` links in the selected file.
    #[arg(long)]
    pub(crate) confirm_replace_slipbox_links: bool,
}

pub(crate) fn run_link(args: &LinkArgs) -> Result<(), CliCommandError> {
    match &args.command {
        LinkCommand::RewriteSlipbox(command) => run_link_rewrite_slipbox(command),
    }
}

impl HeadlessCommand for LinkRewriteSlipboxPreviewArgs {
    type Output = SlipboxLinkRewritePreviewResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.target.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let file_path =
            normalize_edit_file_path(&self.target.headless.scope.root, &self.target.file)?;
        client.slipbox_link_rewrite_preview(&SlipboxLinkRewritePreviewParams { file_path })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_slipbox_link_rewrite_preview(&output.preview)
    }
}

fn run_link_rewrite_slipbox(command: &LinkRewriteSlipboxArgs) -> Result<(), CliCommandError> {
    match &command.command {
        LinkRewriteSlipboxCommand::Preview(command) => run_headless_command(command),
        LinkRewriteSlipboxCommand::Apply(command) => run_link_rewrite_slipbox_apply(command),
    }
}

fn run_link_rewrite_slipbox_apply(
    command: &LinkRewriteSlipboxApplyArgs,
) -> Result<(), CliCommandError> {
    let output_mode = command.target.headless.output_mode();
    if !command.confirm_replace_slipbox_links {
        return Err(CliCommandError::new(
            output_mode,
            anyhow::anyhow!("link rewrite apply requires --confirm-replace-slipbox-links"),
        ));
    }

    let file_path =
        normalize_edit_file_path(&command.target.headless.scope.root, &command.target.file)
            .map_err(|error| CliCommandError::new(output_mode, error))?;
    let output = run_daemon_operation(&command.target.headless, output_mode, |client| {
        let preview = client
            .slipbox_link_rewrite_preview(&SlipboxLinkRewritePreviewParams { file_path })?
            .preview;
        client.slipbox_link_rewrite_apply(&SlipboxLinkRewriteApplyParams {
            expected_preview: preview,
        })
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        render_slipbox_link_rewrite_application(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}
