use super::super::output::{CliCommandError, write_output};
use super::super::render::notes::{render_anchor_summary, render_capture_preview};
use super::super::runtime::{HeadlessArgs, run_daemon_operation};
use anyhow::{Context, Result};
use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use slipbox_core::{CaptureContentType, CaptureTemplateParams, CaptureTemplatePreviewParams};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
pub(crate) struct CaptureArgs {
    #[command(subcommand)]
    pub(crate) command: CaptureCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum CaptureCommand {
    /// Write content through the daemon-owned capture-template engine.
    Template(CaptureTemplateCommandArgs),
    /// Preview capture-template output without writing files.
    Preview(CapturePreviewCommandArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum CaptureTypeArg {
    Plain,
    Entry,
    Item,
    Checkitem,
    TableLine,
}

impl From<CaptureTypeArg> for CaptureContentType {
    fn from(value: CaptureTypeArg) -> Self {
        match value {
            CaptureTypeArg::Plain => Self::Plain,
            CaptureTypeArg::Entry => Self::Entry,
            CaptureTypeArg::Item => Self::Item,
            CaptureTypeArg::Checkitem => Self::Checkitem,
            CaptureTypeArg::TableLine => Self::TableLine,
        }
    }
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("content-source")
        .args(["content", "content_file", "content_stdin"])
        .multiple(false)
))]
pub(crate) struct CaptureTemplateFields {
    /// Optional title used by file creation and entry captures.
    #[arg(long, default_value = "", value_name = "TITLE")]
    pub(crate) title: String,
    /// File target, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: Option<PathBuf>,
    /// Exact target node key.
    #[arg(long = "node-key", value_name = "KEY")]
    pub(crate) node_key: Option<String>,
    /// Optional leading Org content used when the file must be created.
    #[arg(long, value_name = "ORG")]
    pub(crate) head: Option<String>,
    /// One or more outline path segments.
    #[arg(long = "outline", num_args = 1.., value_name = "HEADING")]
    pub(crate) outline_path: Vec<String>,
    /// Capture content type.
    #[arg(long = "type", value_enum, default_value_t = CaptureTypeArg::Plain)]
    pub(crate) capture_type: CaptureTypeArg,
    /// Capture content supplied directly on the command line.
    #[arg(long, value_name = "TEXT")]
    pub(crate) content: Option<String>,
    /// Read capture content from a file.
    #[arg(long = "content-file", value_name = "PATH")]
    pub(crate) content_file: Option<PathBuf>,
    /// Read capture content from stdin.
    #[arg(long = "content-stdin")]
    pub(crate) content_stdin: bool,
    /// Reference to attach when creating a new target file.
    #[arg(long = "ref", num_args = 1.., value_name = "REF")]
    pub(crate) refs: Vec<String>,
    /// Insert before existing target body content when supported.
    #[arg(long)]
    pub(crate) prepend: bool,
    /// Empty lines before inserted capture content.
    #[arg(long, default_value_t = 0)]
    pub(crate) empty_lines_before: u32,
    /// Empty lines after inserted capture content.
    #[arg(long, default_value_t = 0)]
    pub(crate) empty_lines_after: u32,
    /// Org table insertion position used for table-line captures.
    #[arg(long = "table-line-pos", value_name = "POSITION")]
    pub(crate) table_line_pos: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct CaptureTemplateCommandArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) template: CaptureTemplateFields,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("preview-source")
        .args(["source_override", "source_file"])
        .multiple(false)
))]
pub(crate) struct CapturePreviewCommandArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) template: CaptureTemplateFields,
    /// Source text to preview against instead of reading the target file.
    #[arg(long = "source", value_name = "TEXT")]
    pub(crate) source_override: Option<String>,
    /// Read source text to preview against from a file.
    #[arg(long = "source-file", value_name = "PATH")]
    pub(crate) source_file: Option<PathBuf>,
    /// Ensure the previewed target node has an explicit ID in rendered output.
    #[arg(long)]
    pub(crate) ensure_node_id: bool,
}

impl CaptureTemplateFields {
    fn params(&self) -> Result<CaptureTemplateParams> {
        self.validate_target_mode()?;
        Ok(CaptureTemplateParams {
            title: self.title.clone(),
            file_path: self.file.as_ref().map(|path| path.display().to_string()),
            node_key: self.node_key.clone(),
            head: self.head.clone(),
            outline_path: self.outline_path.clone(),
            capture_type: self.capture_type.into(),
            content: self.content()?,
            refs: self.refs.clone(),
            prepend: self.prepend,
            empty_lines_before: self.empty_lines_before,
            empty_lines_after: self.empty_lines_after,
            table_line_pos: self.table_line_pos.clone(),
        })
    }

    fn content(&self) -> Result<String> {
        if let Some(content) = &self.content {
            return Ok(content.clone());
        }
        if let Some(path) = &self.content_file {
            return fs::read_to_string(path).with_context(|| {
                format!("failed to read capture content from {}", path.display())
            });
        }
        if self.content_stdin {
            let mut content = String::new();
            io::stdin()
                .read_to_string(&mut content)
                .context("failed to read capture content from stdin")?;
            return Ok(content);
        }
        Ok(String::new())
    }

    fn validate_target_mode(&self) -> Result<()> {
        if self.node_key.is_some()
            && (self.file.is_some()
                || self.head.is_some()
                || !self.outline_path.is_empty()
                || !self.refs.is_empty())
        {
            anyhow::bail!("--node-key cannot be combined with --file, --head, --outline, or --ref");
        }
        Ok(())
    }
}

impl CapturePreviewCommandArgs {
    fn source_override(&self) -> Result<Option<String>> {
        if let Some(source) = &self.source_override {
            return Ok(Some(source.clone()));
        }
        if let Some(path) = &self.source_file {
            return fs::read_to_string(path)
                .with_context(|| {
                    format!(
                        "failed to read capture preview source from {}",
                        path.display()
                    )
                })
                .map(Some);
        }
        Ok(None)
    }
}

pub(crate) fn run_capture(args: &CaptureArgs) -> Result<(), CliCommandError> {
    match &args.command {
        CaptureCommand::Template(command) => run_capture_template(command),
        CaptureCommand::Preview(command) => run_capture_preview(command),
    }
}

fn run_capture_template(command: &CaptureTemplateCommandArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let params = command
        .template
        .params()
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let captured = run_daemon_operation(&command.headless, output_mode, |client| {
        client.capture_template(&params)
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &captured, |value| {
        render_anchor_summary(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn run_capture_preview(command: &CapturePreviewCommandArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let capture = command
        .template
        .params()
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let source_override = command
        .source_override()
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let preview = run_daemon_operation(&command.headless, output_mode, |client| {
        client.capture_template_preview(&CaptureTemplatePreviewParams {
            capture,
            source_override,
            ensure_node_id: command.ensure_node_id,
        })
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &preview, |value| {
        render_capture_preview(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}
