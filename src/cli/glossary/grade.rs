use super::super::render::glossary::render_grade_result;
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTargetArgs, resolve_note_target,
};
use clap::Args;
use slipbox_core::{GradeTermParams, GradeTermResult};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Args)]
pub(crate) struct GlossaryGradeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// SM-2 quality score, clamped to `0..=5` during scheduling.
    #[arg(long)]
    pub(crate) quality: i64,
    /// ISO `YYYY-MM-DD` reference date. Uses the daemon's local date when omitted.
    #[arg(long, value_name = "DATE")]
    pub(crate) today: Option<String>,
}

impl HeadlessCommand for GlossaryGradeArgs {
    type Output = GradeTermResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let node = resolve_note_target(client, &self.target.target())?;
        client.grade_term(&GradeTermParams {
            node_key: node.node_key,
            quality: self.quality,
            today: self.today.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_grade_result(output)
    }
}
