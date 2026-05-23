use std::io;

use anyhow::Result;
use serde::Serialize;
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

use super::super::output::{CliCommandError, OutputMode, write_output};
use super::scope::HeadlessArgs;

pub(crate) trait HeadlessCommand {
    type Output: Serialize;

    fn headless_args(&self) -> &HeadlessArgs;
    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError>;
    fn render_human(&self, output: &Self::Output) -> String;
}

pub(crate) fn run_daemon_operation<T>(
    headless: &HeadlessArgs,
    output_mode: OutputMode,
    operation: impl FnOnce(&mut DaemonClient) -> Result<T, DaemonClientError>,
) -> Result<T, CliCommandError> {
    run_daemon_task(headless, output_mode, |client| {
        operation(client).map_err(Into::into)
    })
}

pub(crate) fn run_daemon_task<T>(
    headless: &HeadlessArgs,
    output_mode: OutputMode,
    operation: impl FnOnce(&mut DaemonClient) -> Result<T>,
) -> Result<T, CliCommandError> {
    let mut client = headless.connect_with_output_mode(output_mode)?;
    let output =
        operation(&mut client).map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    Ok(output)
}

pub(crate) fn run_headless_command<C>(command: &C) -> Result<(), CliCommandError>
where
    C: HeadlessCommand,
{
    let output_mode = command.headless_args().output_mode();
    let output = run_daemon_operation(command.headless_args(), output_mode, |client| {
        command.execute(client)
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        command.render_human(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}
