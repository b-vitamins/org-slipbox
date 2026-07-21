use super::super::render::glossary::render_glossary_search_result;
use super::super::runtime::{HeadlessArgs, HeadlessCommand};
use clap::Args;
use slipbox_core::{SearchGlossaryParams, SearchGlossaryResult};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Args)]
pub(crate) struct GlossarySearchArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Search text matched against indexed term titles, synonyms, refs, and definitions.
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    /// Maximum terms to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

impl HeadlessCommand for GlossarySearchArgs {
    type Output = SearchGlossaryResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.search_glossary(&SearchGlossaryParams {
            query: self.query.clone(),
            limit: self.limit,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_glossary_search_result(output)
    }
}
