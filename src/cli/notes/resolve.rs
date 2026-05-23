use super::super::output::CliCommandError;
use super::super::render::notes::render_node_summary;
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTargetArgs, resolve_note_target, run_headless_command,
};
use anyhow::Result;
use clap::Args;
use slipbox_core::NodeRecord;
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Args)]
pub(crate) struct ResolveNodeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
}

pub(crate) fn run_resolve_node(args: &ResolveNodeArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
}

impl HeadlessCommand for ResolveNodeArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        resolve_note_target(client, &self.target.target())
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}
