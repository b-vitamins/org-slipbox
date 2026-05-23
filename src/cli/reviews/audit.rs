use super::super::output::{
    CliCommandError, OutputMode, ReportFormat, ReportOutputArgs, render_report_bytes, write_output,
    write_report_destination,
};
use super::super::render::reviews::{
    render_corpus_audit_kind, render_corpus_audit_result, render_saved_review_summary,
};
use super::super::runtime::{HeadlessArgs, SaveReviewArgs, run_daemon_operation};
use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Serialize;
use slipbox_core::{
    CorpusAuditKind, CorpusAuditParams, CorpusAuditResult, ReviewRunSummary,
    SaveCorpusAuditReviewParams, SaveCorpusAuditReviewResult,
};
use std::io;

#[derive(Debug, Serialize)]
struct AuditReportOutputResult {
    audit: CorpusAuditKind,
    format: ReportFormat,
    output_path: String,
    entry_count: usize,
}

#[derive(Debug, Serialize)]
struct SavedAuditReportOutputResult {
    audit: CorpusAuditKind,
    format: ReportFormat,
    output_path: String,
    entry_count: usize,
    review: ReviewRunSummary,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct AuditArgs {
    #[command(subcommand)]
    pub(crate) command: AuditCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum AuditCommand {
    /// List links that point to missing explicit IDs.
    DanglingLinks(AuditRunArgs),
    /// Group note-title collisions that may need disambiguation.
    DuplicateTitles(AuditRunArgs),
    /// List notes with no refs, backlinks, or outgoing links.
    OrphanNotes(AuditRunArgs),
    /// List ref-backed notes with very weak structural integration.
    WeaklyIntegratedNotes(AuditRunArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct AuditRunArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Maximum audit entries to return.
    #[arg(long, default_value_t = 200, value_name = "N")]
    pub(crate) limit: usize,
    #[command(flatten)]
    pub(crate) report: ReportOutputArgs,
    #[command(flatten)]
    pub(crate) save_review: SaveReviewArgs,
}

pub(crate) fn run_audit(args: &AuditArgs) -> Result<(), CliCommandError> {
    match &args.command {
        AuditCommand::DanglingLinks(command) => {
            run_audit_command(CorpusAuditKind::DanglingLinks, command)
        }
        AuditCommand::DuplicateTitles(command) => {
            run_audit_command(CorpusAuditKind::DuplicateTitles, command)
        }
        AuditCommand::OrphanNotes(command) => {
            run_audit_command(CorpusAuditKind::OrphanNotes, command)
        }
        AuditCommand::WeaklyIntegratedNotes(command) => {
            run_audit_command(CorpusAuditKind::WeaklyIntegratedNotes, command)
        }
    }
}

fn run_audit_command(kind: CorpusAuditKind, args: &AuditRunArgs) -> Result<(), CliCommandError> {
    let output_mode = args.headless.output_mode();
    let report_format = args
        .report
        .format(output_mode)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let error_output_mode = report_format.error_output_mode();
    let save_review = args
        .save_review
        .request()
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    if let Some(save_review) = save_review {
        let saved = run_daemon_operation(&args.headless, error_output_mode, |client| {
            client.save_corpus_audit_review(&SaveCorpusAuditReviewParams {
                audit: kind,
                limit: args.limit,
                review_id: save_review.review_id,
                title: save_review.title,
                summary: save_review.summary,
                overwrite: save_review.overwrite,
            })
        })?;
        return write_saved_audit_command_output(args, report_format, saved, error_output_mode);
    }

    let result = run_daemon_operation(&args.headless, error_output_mode, |client| {
        client.corpus_audit(&CorpusAuditParams {
            audit: kind,
            limit: args.limit,
        })
    })?;

    let report_bytes = render_report_bytes(
        report_format,
        &result,
        render_corpus_audit_result,
        CorpusAuditResult::report_lines,
    )
    .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    write_report_destination(&report_bytes, args.report.output_path())
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;

    if let Some(output_path) = args.report.output_path() {
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let ack = AuditReportOutputResult {
            audit: result.audit,
            format: report_format,
            output_path: output_path.display().to_string(),
            entry_count: result.entries.len(),
        };
        write_output(
            &mut writer,
            report_format.ack_output_mode(),
            &ack,
            |value| {
                format!(
                    "wrote audit report: {} -> {} ({})\n",
                    render_corpus_audit_kind(value.audit),
                    value.output_path,
                    value.format.label(),
                )
            },
        )
        .map_err(|error| CliCommandError::new(report_format.ack_output_mode(), error))?;
    }
    Ok(())
}

fn write_saved_audit_command_output(
    args: &AuditRunArgs,
    report_format: ReportFormat,
    saved: SaveCorpusAuditReviewResult,
    error_output_mode: OutputMode,
) -> Result<(), CliCommandError> {
    if let Some(output_path) = args.report.output_path() {
        let report_bytes = render_report_bytes(
            report_format,
            &saved.result,
            render_corpus_audit_result,
            CorpusAuditResult::report_lines,
        )
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        write_report_destination(&report_bytes, Some(output_path))
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;

        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let ack = SavedAuditReportOutputResult {
            audit: saved.result.audit,
            format: report_format,
            output_path: output_path.display().to_string(),
            entry_count: saved.result.entries.len(),
            review: saved.review,
        };
        return write_output(
            &mut writer,
            report_format.ack_output_mode(),
            &ack,
            |value| {
                let mut output = format!(
                    "wrote audit report: {} -> {} ({})\n",
                    render_corpus_audit_kind(value.audit),
                    value.output_path,
                    value.format.label(),
                );
                output.push_str(&render_saved_review_summary(&value.review));
                output
            },
        )
        .map_err(|error| CliCommandError::new(report_format.ack_output_mode(), error));
    }

    match report_format {
        ReportFormat::Human => {
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            write_output(&mut writer, OutputMode::Human, &saved, |value| {
                let mut output = render_corpus_audit_result(&value.result);
                output.push('\n');
                output.push_str(&render_saved_review_summary(&value.review));
                output
            })
            .map_err(|error| CliCommandError::new(OutputMode::Human, error))
        }
        ReportFormat::Json => {
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            write_output(&mut writer, OutputMode::Json, &saved, |_| String::new())
                .map_err(|error| CliCommandError::new(OutputMode::Json, error))
        }
        ReportFormat::Jsonl => {
            let report_bytes = render_report_bytes(
                report_format,
                &saved.result,
                render_corpus_audit_result,
                CorpusAuditResult::report_lines,
            )
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
            write_report_destination(&report_bytes, None)
                .map_err(|error| CliCommandError::new(error_output_mode, error))
        }
    }
}
