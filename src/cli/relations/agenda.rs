use super::super::output::CliCommandError;
use super::super::render::relations::render_agenda_result;
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, invalid_request_error, run_headless_command,
};
use anyhow::Result;
use chrono::{Local, NaiveDate};
use clap::{Args, Subcommand};
use slipbox_core::{AgendaParams, AgendaResult};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Args)]
pub(crate) struct AgendaArgs {
    #[command(subcommand)]
    pub(crate) command: AgendaCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum AgendaCommand {
    /// Query entries for the current local date.
    Today(AgendaTodayArgs),
    /// Query entries for one ISO date, YYYY-MM-DD.
    Date(AgendaDateArgs),
    /// Query entries for an inclusive ISO date range, YYYY-MM-DD YYYY-MM-DD.
    Range(AgendaRangeArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct AgendaTodayArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Maximum agenda entries to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct AgendaDateArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// ISO date to query, YYYY-MM-DD.
    #[arg(value_name = "DATE")]
    pub(crate) date: String,
    /// Maximum agenda entries to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct AgendaRangeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Inclusive ISO start date, YYYY-MM-DD.
    #[arg(value_name = "START")]
    pub(crate) start: String,
    /// Inclusive ISO end date, YYYY-MM-DD.
    #[arg(value_name = "END")]
    pub(crate) end: String,
    /// Maximum agenda entries to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
}

pub(crate) fn run_agenda(args: &AgendaArgs) -> Result<(), CliCommandError> {
    match &args.command {
        AgendaCommand::Today(command) => run_headless_command(command),
        AgendaCommand::Date(command) => run_headless_command(command),
        AgendaCommand::Range(command) => run_headless_command(command),
    }
}

fn parse_agenda_date(value: &str) -> Result<NaiveDate, DaemonClientError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        invalid_request_error(format!(
            "invalid agenda date {value:?}: expected ISO date YYYY-MM-DD"
        ))
    })
}

fn today_local_date() -> NaiveDate {
    Local::now().date_naive()
}

fn agenda_params(
    start: NaiveDate,
    end: NaiveDate,
    limit: usize,
) -> Result<AgendaParams, DaemonClientError> {
    if end < start {
        return Err(invalid_request_error(format!(
            "agenda range end {} is before start {}",
            end.format("%Y-%m-%d"),
            start.format("%Y-%m-%d")
        )));
    }
    Ok(AgendaParams {
        start: format!("{}T00:00:00", start.format("%Y-%m-%d")),
        end: format!("{}T23:59:59", end.format("%Y-%m-%d")),
        limit,
    })
}

impl HeadlessCommand for AgendaTodayArgs {
    type Output = AgendaResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let today = today_local_date();
        client.agenda(&agenda_params(today, today, self.limit)?)
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_agenda_result(output)
    }
}

impl HeadlessCommand for AgendaDateArgs {
    type Output = AgendaResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let date = parse_agenda_date(&self.date)?;
        client.agenda(&agenda_params(date, date, self.limit)?)
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_agenda_result(output)
    }
}

impl HeadlessCommand for AgendaRangeArgs {
    type Output = AgendaResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let start = parse_agenda_date(&self.start)?;
        let end = parse_agenda_date(&self.end)?;
        client.agenda(&agenda_params(start, end, self.limit)?)
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_agenda_result(output)
    }
}
