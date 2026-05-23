use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use slipbox_daemon_client::{DaemonClient, DaemonServeConfig};
use slipbox_index::DiscoveryPolicy;

use super::super::output::{CliCommandError, OutputMode};

#[derive(Debug, Clone, Args)]
pub(crate) struct ScopeArgs {
    /// Org source root. Commands only read or write files inside this tree.
    #[arg(long, value_name = "ROOT")]
    pub(crate) root: PathBuf,
    /// Derived SQLite index path. The daemon may rebuild it from Org files.
    #[arg(long, value_name = "DB")]
    pub(crate) db: PathBuf,
    /// Extra directory containing workflow spec JSON files. Repeat for multiple directories.
    #[arg(long = "workflow-dir", value_name = "DIR")]
    pub(crate) workflow_dirs: Vec<PathBuf>,
    /// File extension eligible for discovery and indexing. Defaults to `org`.
    #[arg(long = "file-extension", value_name = "EXT")]
    pub(crate) file_extensions: Vec<String>,
    /// Relative-path regular expression to exclude from discovery. Repeat as needed.
    #[arg(long = "exclude-regexp", value_name = "REGEXP")]
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
            workflow_dirs: self.workflow_dirs.clone(),
            file_extensions: self.file_extensions.clone(),
            exclude_regexps: self.exclude_regexps.clone(),
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct HeadlessArgs {
    #[command(flatten)]
    pub(crate) scope: ScopeArgs,
    /// Executable used to spawn `slipbox serve`. Defaults to the current binary.
    #[arg(long, value_name = "PATH")]
    pub(crate) server_program: Option<PathBuf>,
    /// Emit stable JSON to stdout and structured JSON errors to stderr.
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

    pub(crate) fn connect_with_output_mode(
        &self,
        output_mode: OutputMode,
    ) -> Result<DaemonClient, CliCommandError> {
        let program = self
            .server_program_path()
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        DaemonClient::spawn(program, &self.scope.daemon_config())
            .map_err(|error| CliCommandError::new(output_mode, error))
    }
}
