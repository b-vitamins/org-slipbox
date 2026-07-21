use super::super::render::glossary::{render_glossary_term_list, render_glossary_term_result};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTargetArgs, resolve_note_target,
};
use clap::Args;
use slipbox_core::{
    GlossaryTermParams, GlossaryTermResult, ListGlossaryTermsParams, ListGlossaryTermsResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Args)]
pub(crate) struct GlossaryListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Maximum terms to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct GlossaryShowArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
}

impl HeadlessCommand for GlossaryListArgs {
    type Output = ListGlossaryTermsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_glossary_terms(&ListGlossaryTermsParams { limit: self.limit })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_glossary_term_list(output)
    }
}

impl HeadlessCommand for GlossaryShowArgs {
    type Output = GlossaryTermResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let node = resolve_note_target(client, &self.target.target())?;
        client.glossary_term(&GlossaryTermParams {
            node_key: node.node_key,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_glossary_term_result(output)
    }
}
