use super::super::render::glossary::render_glossary_due_result;
use super::super::runtime::{HeadlessArgs, HeadlessCommand};
use clap::Args;
use slipbox_core::{GlossaryDueParams, GlossaryDueResult};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Args)]
pub(crate) struct GlossaryDueArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// ISO `YYYY-MM-DD` reference date. Uses the daemon's local date when omitted.
    #[arg(long, value_name = "DATE")]
    pub(crate) today: Option<String>,
    /// Match due-term headwords and synonyms.
    #[arg(long, value_name = "QUERY")]
    pub(crate) query: Option<String>,
    /// Maximum due terms to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

impl HeadlessCommand for GlossaryDueArgs {
    type Output = GlossaryDueResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.glossary_due(&GlossaryDueParams {
            today: self.today.clone(),
            query: self.query.clone(),
            limit: self.limit,
            after: None,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_glossary_due_result(output)
    }
}
