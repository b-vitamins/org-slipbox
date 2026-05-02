use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{ArgGroup, Args};
use serde::{Deserialize, Serialize};
use slipbox_core::{
    NodeFromIdParams, NodeFromKeyParams, NodeFromRefParams, NodeFromTitleOrAliasParams, NodeRecord,
    StatusInfo,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError, DaemonServeConfig};
use slipbox_index::DiscoveryPolicy;
use slipbox_rpc::JsonRpcErrorObject;

#[derive(Debug, Clone, Args)]
pub(crate) struct ScopeArgs {
    /// Root directory containing Org files.
    #[arg(long)]
    pub(crate) root: PathBuf,
    /// SQLite database path.
    #[arg(long)]
    pub(crate) db: PathBuf,
    /// File extensions eligible for discovery and indexing.
    #[arg(long = "file-extension")]
    pub(crate) file_extensions: Vec<String>,
    /// Relative-path regular expressions to exclude from discovery.
    #[arg(long = "exclude-regexp")]
    pub(crate) exclude_regexps: Vec<String>,
}

impl ScopeArgs {
    pub(crate) fn discovery_policy(&self) -> Result<DiscoveryPolicy> {
        if self.file_extensions.is_empty() && self.exclude_regexps.is_empty() {
            Ok(DiscoveryPolicy::default())
        } else {
            DiscoveryPolicy::new(self.file_extensions.clone(), self.exclude_regexps.clone())
        }
    }

    #[must_use]
    pub(crate) fn daemon_config(&self) -> DaemonServeConfig {
        DaemonServeConfig {
            root: self.root.clone(),
            db: self.db.clone(),
            file_extensions: self.file_extensions.clone(),
            exclude_regexps: self.exclude_regexps.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputMode {
    Human,
    Json,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct HeadlessArgs {
    #[command(flatten)]
    pub(crate) scope: ScopeArgs,
    /// Path to the slipbox executable used to spawn `slipbox serve`.
    #[arg(long)]
    pub(crate) server_program: Option<PathBuf>,
    /// Emit structured JSON to stdout and structured errors to stderr.
    #[arg(long)]
    pub(crate) json: bool,
}

impl HeadlessArgs {
    #[must_use]
    pub(crate) fn output_mode(&self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }

    pub(crate) fn server_program_path(&self) -> Result<PathBuf> {
        match &self.server_program {
            Some(path) => Ok(path.clone()),
            None => env::current_exe().context("failed to resolve current slipbox executable"),
        }
    }

    pub(crate) fn connect(&self) -> Result<DaemonClient, CliCommandError> {
        let program = self
            .server_program_path()
            .map_err(|error| CliCommandError::new(self.output_mode(), error))?;
        DaemonClient::spawn(program, &self.scope.daemon_config())
            .map_err(|error| CliCommandError::new(self.output_mode(), error))
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct StatusArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("target")
        .args(["id", "title", "reference", "key"])
        .required(true)
        .multiple(false)
))]
pub(crate) struct ResolveTargetArgs {
    /// Resolve an exact explicit Org ID.
    #[arg(long, group = "target")]
    pub(crate) id: Option<String>,
    /// Resolve an exact title or alias.
    #[arg(long, group = "target")]
    pub(crate) title: Option<String>,
    /// Resolve an exact reference.
    #[arg(long = "ref", group = "target")]
    pub(crate) reference: Option<String>,
    /// Resolve an exact node key.
    #[arg(long, group = "target")]
    pub(crate) key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolveTarget {
    Id(String),
    Title(String),
    Reference(String),
    Key(String),
}

impl ResolveTargetArgs {
    #[must_use]
    pub(crate) fn target(&self) -> ResolveTarget {
        if let Some(id) = &self.id {
            ResolveTarget::Id(id.clone())
        } else if let Some(title) = &self.title {
            ResolveTarget::Title(title.clone())
        } else if let Some(reference) = &self.reference {
            ResolveTarget::Reference(reference.clone())
        } else if let Some(node_key) = &self.key {
            ResolveTarget::Key(node_key.clone())
        } else {
            unreachable!("clap enforces exactly one target selector");
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ResolveNodeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
}

pub(crate) trait HeadlessCommand {
    type Output: Serialize;

    fn headless_args(&self) -> &HeadlessArgs;
    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError>;
    fn render_human(&self, output: &Self::Output) -> String;
}

pub(crate) fn run_status(args: &StatusArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
}

pub(crate) fn run_resolve_node(args: &ResolveNodeArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
}

pub(crate) fn report_error(error: &CliCommandError) -> ExitCode {
    let stderr = io::stderr();
    let mut writer = stderr.lock();
    let _ = error.write(&mut writer);
    ExitCode::from(1)
}

pub(crate) struct CliCommandError {
    output_mode: OutputMode,
    inner: anyhow::Error,
}

impl CliCommandError {
    pub(crate) fn new(output_mode: OutputMode, error: impl Into<anyhow::Error>) -> Self {
        Self {
            output_mode,
            inner: error.into(),
        }
    }

    fn write(&self, writer: &mut impl Write) -> Result<()> {
        match self.output_mode {
            OutputMode::Human => {
                writeln!(writer, "error: {}", self.inner)?;
            }
            OutputMode::Json => {
                let payload = ErrorPayload {
                    error: ErrorMessage {
                        message: self.inner.to_string(),
                    },
                };
                serde_json::to_writer(&mut *writer, &payload)?;
                writer.write_all(b"\n")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorPayload {
    error: ErrorMessage,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorMessage {
    message: String,
}

fn run_headless_command<C>(command: &C) -> Result<(), CliCommandError>
where
    C: HeadlessCommand,
{
    let output_mode = command.headless_args().output_mode();
    let mut client = command.headless_args().connect()?;
    let output = command
        .execute(&mut client)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        command.render_human(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn write_output<T>(
    writer: &mut impl Write,
    output_mode: OutputMode,
    value: &T,
    human_renderer: impl FnOnce(&T) -> String,
) -> Result<()>
where
    T: Serialize,
{
    match output_mode {
        OutputMode::Human => {
            writer.write_all(human_renderer(value).as_bytes())?;
        }
        OutputMode::Json => {
            serde_json::to_writer(&mut *writer, value)?;
            writer.write_all(b"\n")?;
        }
    }
    writer.flush()?;
    Ok(())
}

impl HeadlessCommand for StatusArgs {
    type Output = StatusInfo;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.status()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!(
            "version: {}\nroot: {}\ndb: {}\nfiles indexed: {}\nnodes indexed: {}\nlinks indexed: {}\n",
            output.version,
            output.root,
            output.db,
            output.files_indexed,
            output.nodes_indexed,
            output.links_indexed,
        )
    }
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

pub(crate) fn resolve_note_target(
    client: &mut DaemonClient,
    target: &ResolveTarget,
) -> Result<NodeRecord, DaemonClientError> {
    match target {
        ResolveTarget::Id(id) => require_resolved_node(
            client.node_from_id(&NodeFromIdParams { id: id.clone() })?,
            format!("unknown node id: {id}"),
        ),
        ResolveTarget::Title(title_or_alias) => require_resolved_node(
            client.node_from_title_or_alias(&NodeFromTitleOrAliasParams {
                title_or_alias: title_or_alias.clone(),
                nocase: false,
            })?,
            format!("unknown node title or alias: {title_or_alias}"),
        ),
        ResolveTarget::Reference(reference) => require_resolved_node(
            client.node_from_ref(&NodeFromRefParams {
                reference: reference.clone(),
            })?,
            format!("unknown node ref: {reference}"),
        ),
        ResolveTarget::Key(node_key) => require_resolved_node(
            client.node_from_key(&NodeFromKeyParams {
                node_key: node_key.clone(),
            })?,
            format!("unknown node key: {node_key}"),
        ),
    }
}

fn require_resolved_node(
    node: Option<NodeRecord>,
    error_message: String,
) -> Result<NodeRecord, DaemonClientError> {
    node.ok_or_else(|| DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(error_message)))
}

fn render_node_summary(node: &NodeRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("node key: {}\n", node.node_key));
    if let Some(explicit_id) = &node.explicit_id {
        output.push_str(&format!("id: {explicit_id}\n"));
    }
    output.push_str(&format!("title: {}\n", node.title));
    output.push_str(&format!("kind: {}\n", node.kind.as_str()));
    output.push_str(&format!("file: {}\n", node.file_path));
    output.push_str(&format!("line: {}\n", node.line));
    if !node.outline_path.is_empty() {
        output.push_str(&format!("outline path: {}\n", node.outline_path));
    }
    if !node.aliases.is_empty() {
        output.push_str(&format!("aliases: {}\n", node.aliases.join(", ")));
    }
    if !node.refs.is_empty() {
        output.push_str(&format!("refs: {}\n", node.refs.join(", ")));
    }
    if !node.tags.is_empty() {
        output.push_str(&format!("tags: {}\n", node.tags.join(", ")));
    }
    if let Some(todo_keyword) = &node.todo_keyword {
        output.push_str(&format!("todo: {todo_keyword}\n"));
    }
    if let Some(scheduled_for) = &node.scheduled_for {
        output.push_str(&format!("scheduled: {scheduled_for}\n"));
    }
    if let Some(deadline_for) = &node.deadline_for {
        output.push_str(&format!("deadline: {deadline_for}\n"));
    }
    if let Some(closed_at) = &node.closed_at {
        output.push_str(&format!("closed: {closed_at}\n"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{ErrorPayload, OutputMode, write_output};
    use slipbox_core::StatusInfo;

    #[test]
    fn writes_json_output_from_structured_results() {
        let mut output = Vec::new();
        let status = StatusInfo {
            version: "0.6.1".to_owned(),
            root: "/tmp/root".to_owned(),
            db: "/tmp/db.sqlite".to_owned(),
            files_indexed: 1,
            nodes_indexed: 2,
            links_indexed: 3,
        };

        write_output(&mut output, OutputMode::Json, &status, |_| String::new())
            .expect("json output should render");

        let parsed: StatusInfo =
            serde_json::from_slice(&output).expect("json output should deserialize");
        assert_eq!(parsed.files_indexed, 1);
    }

    #[test]
    fn writes_structured_json_errors() {
        let error = super::CliCommandError::new(OutputMode::Json, anyhow::anyhow!("broken"));
        let mut stderr = Vec::new();

        error.write(&mut stderr).expect("json error should render");

        let parsed: ErrorPayload =
            serde_json::from_slice(&stderr).expect("json error should deserialize");
        assert_eq!(parsed.error.message, "broken");
    }
}
