use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputMode {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ReportFormat {
    Human,
    Json,
    Jsonl,
}

impl ReportFormat {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Jsonl => "jsonl",
        }
    }

    pub(crate) const fn error_output_mode(self) -> OutputMode {
        match self {
            Self::Human => OutputMode::Human,
            Self::Json | Self::Jsonl => OutputMode::Json,
        }
    }

    pub(crate) const fn ack_output_mode(self) -> OutputMode {
        match self {
            Self::Human => OutputMode::Human,
            Self::Json | Self::Jsonl => OutputMode::Json,
        }
    }
}

#[derive(Debug, Clone, Args, Default)]
pub(crate) struct ReportOutputArgs {
    /// Write the rendered report to this path instead of stdout. Use `-` for stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    /// Emit line-oriented JSON instead of a single structured result document.
    #[arg(long)]
    pub(crate) jsonl: bool,
}

impl ReportOutputArgs {
    pub(crate) fn format(&self, output_mode: OutputMode) -> Result<ReportFormat> {
        if self.jsonl && matches!(output_mode, OutputMode::Json) {
            anyhow::bail!("--json and --jsonl are mutually exclusive");
        }

        Ok(if self.jsonl {
            ReportFormat::Jsonl
        } else if matches!(output_mode, OutputMode::Json) {
            ReportFormat::Json
        } else {
            ReportFormat::Human
        })
    }

    pub(crate) fn output_path(&self) -> Option<&Path> {
        self.output
            .as_deref()
            .filter(|path| *path != Path::new("-"))
    }
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

    pub(crate) fn write(&self, writer: &mut impl Write) -> Result<()> {
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
pub(crate) struct ErrorPayload {
    pub(crate) error: ErrorMessage,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ErrorMessage {
    pub(crate) message: String,
}

pub(crate) fn write_output<T>(
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

pub(crate) fn render_report_bytes<T, L>(
    format: ReportFormat,
    value: &T,
    human_renderer: impl FnOnce(&T) -> String,
    jsonl_renderer: impl FnOnce(&T) -> Vec<L>,
) -> Result<Vec<u8>>
where
    T: Serialize,
    L: Serialize,
{
    let mut bytes = Vec::new();
    match format {
        ReportFormat::Human => bytes.extend_from_slice(human_renderer(value).as_bytes()),
        ReportFormat::Json => {
            serde_json::to_writer(&mut bytes, value)?;
            bytes.push(b'\n');
        }
        ReportFormat::Jsonl => {
            for line in jsonl_renderer(value) {
                serde_json::to_writer(&mut bytes, &line)?;
                bytes.push(b'\n');
            }
        }
    }
    Ok(bytes)
}

pub(crate) fn write_report_destination(bytes: &[u8], output_path: Option<&Path>) -> Result<()> {
    if let Some(path) = output_path {
        fs::write(path, bytes)
            .with_context(|| format!("failed to write report to {}", path.display()))?;
    } else {
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        writer.write_all(bytes)?;
        writer.flush()?;
    }
    Ok(())
}
