use super::super::render::glossary::render_mark_result;
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTargetArgs, resolve_note_target,
};
use clap::{Args, ValueEnum};
use slipbox_core::{GlossaryStatus, MarkGlossaryTermParams, MarkGlossaryTermResult};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum GlossaryStatusArg {
    Stub,
    Confirmed,
}

impl From<GlossaryStatusArg> for GlossaryStatus {
    fn from(value: GlossaryStatusArg) -> Self {
        match value {
            GlossaryStatusArg::Stub => Self::Stub,
            GlossaryStatusArg::Confirmed => Self::Confirmed,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct GlossaryMarkArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// Confirmation status to record. Leaves the existing status unchanged when omitted.
    #[arg(long, value_enum)]
    pub(crate) status: Option<GlossaryStatusArg>,
}

impl HeadlessCommand for GlossaryMarkArgs {
    type Output = MarkGlossaryTermResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let node = resolve_note_target(client, &self.target.target())?;
        client.mark_glossary_term(&MarkGlossaryTermParams {
            node_key: node.node_key,
            status: self.status.map(GlossaryStatus::from),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_mark_result(output)
    }
}
