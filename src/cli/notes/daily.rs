use super::super::output::CliCommandError;
use super::super::render::notes::{render_anchor_summary, render_node_summary};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, invalid_request_error, normalize_daily_file_path,
    require_resolved_node, run_headless_command,
};
use anyhow::Result;
use chrono::{Local, NaiveDate};
use clap::{Args, Subcommand};
use slipbox_core::{
    AnchorRecord, AppendHeadingParams, CaptureContentType, CaptureTemplateParams,
    EnsureFileNodeParams, NodeFromKeyParams, NodeRecord,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyArgs {
    #[command(subcommand)]
    pub(crate) command: DailyCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum DailyCommand {
    /// Ensure a daily note exists.
    Ensure(DailyEnsureArgs),
    /// Show an already indexed daily note without creating it.
    Show(DailyShowArgs),
    /// Append a heading to a daily note.
    Append(DailyAppendArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyTargetArgs {
    /// ISO date to use, YYYY-MM-DD. Defaults to today's local date.
    #[arg(long, value_name = "DATE")]
    pub(crate) date: Option<String>,
    /// Daily note directory inside --root.
    #[arg(long, default_value = "daily", value_name = "DIR")]
    pub(crate) directory: String,
    /// strftime-compatible daily note filename format.
    #[arg(
        long = "file-format",
        default_value = "%Y-%m-%d.org",
        value_name = "FORMAT"
    )]
    pub(crate) file_format: String,
    /// strftime-compatible daily note title format.
    #[arg(
        long = "title-format",
        default_value = "%Y-%m-%d",
        value_name = "FORMAT"
    )]
    pub(crate) title_format: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyEnsureArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: DailyTargetArgs,
    /// Optional strftime-compatible file head used when creating the daily note.
    #[arg(long, value_name = "ORG")]
    pub(crate) head: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyShowArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: DailyTargetArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyAppendArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: DailyTargetArgs,
    /// Heading title to append.
    #[arg(long, value_name = "HEADING")]
    pub(crate) heading: String,
    /// Org heading level.
    #[arg(long, default_value_t = 1)]
    pub(crate) level: u32,
    /// Optional strftime-compatible file head used when creating the daily note.
    #[arg(long, value_name = "ORG")]
    pub(crate) head: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DailyTarget {
    pub(crate) date: NaiveDate,
    pub(crate) file_path: String,
    pub(crate) title: String,
}

impl DailyTarget {
    pub(crate) fn node_key(&self) -> String {
        format!("file:{}", self.file_path.replace('\\', "/"))
    }
}

impl DailyTargetArgs {
    fn target(&self) -> Result<DailyTarget, DaemonClientError> {
        if Path::new(&self.directory).is_absolute() {
            return Err(invalid_request_error(
                "daily --directory must be relative to --root",
            ));
        }
        let date = match &self.date {
            Some(date) => parse_daily_date(date)?,
            None => today_local_date(),
        };
        let filename = date.format(&self.file_format).to_string();
        let file_path = if self.directory.trim().is_empty() {
            filename
        } else {
            PathBuf::from(&self.directory)
                .join(filename)
                .display()
                .to_string()
        };
        Ok(DailyTarget {
            date,
            file_path: normalize_daily_file_path(&file_path)?,
            title: date.format(&self.title_format).to_string(),
        })
    }
}

pub(crate) fn run_daily(args: &DailyArgs) -> Result<(), CliCommandError> {
    match &args.command {
        DailyCommand::Ensure(command) => run_headless_command(command),
        DailyCommand::Show(command) => run_headless_command(command),
        DailyCommand::Append(command) => run_headless_command(command),
    }
}

fn parse_daily_date(value: &str) -> Result<NaiveDate, DaemonClientError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        invalid_request_error(format!(
            "invalid daily date {value:?}: expected ISO date YYYY-MM-DD"
        ))
    })
}

fn today_local_date() -> NaiveDate {
    Local::now().date_naive()
}

fn ensure_daily_node(
    client: &mut DaemonClient,
    target: &DailyTarget,
    head: Option<&str>,
) -> Result<NodeRecord, DaemonClientError> {
    if let Some(head) = head {
        if let Some(existing) = client.node_from_key(&NodeFromKeyParams {
            node_key: target.node_key(),
        })? {
            return Ok(existing);
        }

        client.capture_template(&CaptureTemplateParams {
            title: target.title.clone(),
            file_path: Some(target.file_path.clone()),
            node_key: None,
            head: Some(target.date.format(head).to_string()),
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Plain,
            content: String::new(),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        })?;
        return require_resolved_node(
            client.node_from_key(&NodeFromKeyParams {
                node_key: target.node_key(),
            })?,
            format!(
                "unknown daily note for {} after ensure: {}",
                target.date.format("%Y-%m-%d"),
                target.file_path
            ),
        );
    }

    client.ensure_file_node(&EnsureFileNodeParams {
        file_path: target.file_path.clone(),
        title: target.title.clone(),
    })
}

impl HeadlessCommand for DailyEnsureArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let target = self.target.target()?;
        ensure_daily_node(client, &target, self.head.as_deref())
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for DailyShowArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let target = self.target.target()?;
        require_resolved_node(
            client.node_from_key(&NodeFromKeyParams {
                node_key: target.node_key(),
            })?,
            format!(
                "unknown daily note for {}: {}",
                target.date.format("%Y-%m-%d"),
                target.file_path
            ),
        )
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for DailyAppendArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let target = self.target.target()?;
        if self.head.is_some() {
            ensure_daily_node(client, &target, self.head.as_deref())?;
        }
        client.append_heading(&AppendHeadingParams {
            file_path: target.file_path,
            title: target.title,
            heading: self.heading.clone(),
            level: self.level,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
    }
}
