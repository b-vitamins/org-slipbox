use super::super::output::CliCommandError;
use super::super::render::notes::{render_anchor_summary, render_node_summary};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTargetArgs, resolve_note_target, run_headless_command,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{
    AnchorRecord, AppendHeadingAtOutlinePathParams, AppendHeadingParams, AppendHeadingToNodeParams,
    CaptureNodeParams, EnsureFileNodeParams, NodeRecord,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteArgs {
    #[command(subcommand)]
    pub(crate) command: NoteCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum NoteCommand {
    /// Create a file note.
    Create(NoteCreateArgs),
    /// Ensure a specific file note exists.
    EnsureFile(NoteEnsureFileArgs),
    /// Append a heading to a file note.
    AppendHeading(NoteAppendHeadingArgs),
    /// Append a child heading below an exact note target.
    AppendToNode(NoteAppendToNodeArgs),
    /// Append a heading under an outline path, creating missing outline headings.
    AppendOutline(NoteAppendOutlineArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteCreateArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Note title.
    #[arg(long, value_name = "TITLE")]
    pub(crate) title: String,
    /// File path to create, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: Option<PathBuf>,
    /// Optional leading Org content to preserve before generated metadata.
    #[arg(long, value_name = "ORG")]
    pub(crate) head: Option<String>,
    /// Reference to attach to the created note.
    #[arg(long = "ref", num_args = 1.., value_name = "REF")]
    pub(crate) refs: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteEnsureFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to ensure, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// File note title.
    #[arg(long, value_name = "TITLE")]
    pub(crate) title: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteAppendHeadingArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to append within, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// File note title used when the file must be created.
    #[arg(long, value_name = "TITLE")]
    pub(crate) title: String,
    /// Heading title to append.
    #[arg(long, value_name = "HEADING")]
    pub(crate) heading: String,
    /// Org heading level.
    #[arg(long, default_value_t = 1)]
    pub(crate) level: u32,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteAppendToNodeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// Child heading title to append.
    #[arg(long, value_name = "HEADING")]
    pub(crate) heading: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteAppendOutlineArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to append within, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// Heading title to append under the outline path.
    #[arg(long, value_name = "HEADING")]
    pub(crate) heading: String,
    /// One or more outline path segments.
    #[arg(long = "outline", required = true, num_args = 1.., value_name = "HEADING")]
    pub(crate) outline_path: Vec<String>,
    /// Optional leading Org content used when the file must be created.
    #[arg(long, value_name = "ORG")]
    pub(crate) head: Option<String>,
}

pub(crate) fn run_note(args: &NoteArgs) -> Result<(), CliCommandError> {
    match &args.command {
        NoteCommand::Create(command) => run_headless_command(command),
        NoteCommand::EnsureFile(command) => run_headless_command(command),
        NoteCommand::AppendHeading(command) => run_headless_command(command),
        NoteCommand::AppendToNode(command) => run_headless_command(command),
        NoteCommand::AppendOutline(command) => run_headless_command(command),
    }
}

impl HeadlessCommand for NoteCreateArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.capture_node(&CaptureNodeParams {
            title: self.title.clone(),
            file_path: self.file.as_ref().map(|path| path.display().to_string()),
            head: self.head.clone(),
            refs: self.refs.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for NoteEnsureFileArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.ensure_file_node(&EnsureFileNodeParams {
            file_path: self.file.display().to_string(),
            title: self.title.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for NoteAppendHeadingArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.append_heading(&AppendHeadingParams {
            file_path: self.file.display().to_string(),
            title: self.title.clone(),
            heading: self.heading.clone(),
            level: self.level,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
    }
}

impl HeadlessCommand for NoteAppendToNodeArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let node = resolve_note_target(client, &self.target.target())?;
        client.append_heading_to_node(&AppendHeadingToNodeParams {
            node_key: node.node_key,
            heading: self.heading.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
    }
}

impl HeadlessCommand for NoteAppendOutlineArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.append_heading_at_outline_path(&AppendHeadingAtOutlinePathParams {
            file_path: self.file.display().to_string(),
            heading: self.heading.clone(),
            outline_path: self.outline_path.clone(),
            head: self.head.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
    }
}
