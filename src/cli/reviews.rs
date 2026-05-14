use super::runtime::{
    AuditReportOutputResult, CliCommandError, HeadlessArgs, HeadlessCommand, OutputMode,
    ReportFormat, ReportOutputArgs, SaveReviewArgs, SavedAuditReportOutputResult,
    parse_review_finding_status, render_report_bytes, run_headless_command, write_output,
    write_report_destination,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{
    AuditRemediationApplyAction, AuditRemediationPreviewPayload, CorpusAuditKind,
    CorpusAuditParams, CorpusAuditResult, DeleteReviewRunResult, ListReviewRunsResult,
    MarkReviewFindingParams, ReviewFindingRemediationApplyParams, ReviewFindingRemediationPreview,
    ReviewFindingRemediationPreviewParams, ReviewFindingRemediationPreviewResult,
    ReviewRunDiffParams, ReviewRunDiffResult, ReviewRunIdParams, ReviewRunResult,
    SaveCorpusAuditReviewParams, SaveCorpusAuditReviewResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::io::{self};

use super::render::*;

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
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
    #[command(flatten)]
    pub(crate) report: ReportOutputArgs,
    #[command(flatten)]
    pub(crate) save_review: SaveReviewArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewArgs {
    #[command(subcommand)]
    pub(crate) command: ReviewCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReviewCommand {
    /// List durable operational review runs.
    List(ReviewListArgs),
    /// Show a durable review run.
    Show(ReviewShowArgs),
    /// Compare two compatible durable review runs.
    Diff(ReviewDiffArgs),
    /// Preview and apply safe remediation actions for review findings.
    Remediation(ReviewRemediationArgs),
    /// Update one durable review finding status.
    Mark(ReviewMarkArgs),
    /// Delete a durable review run by identifier.
    Delete(ReviewDeleteArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review run identifier.
    pub(crate) review_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewShowArgs {
    #[command(flatten)]
    pub(crate) review: ReviewIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewDiffArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Baseline durable review run identifier.
    pub(crate) base_review_id: String,
    /// Target durable review run identifier.
    pub(crate) target_review_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewRemediationArgs {
    #[command(subcommand)]
    pub(crate) command: ReviewRemediationCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReviewRemediationCommand {
    /// Inspect the daemon-owned remediation preview for one finding.
    Preview(ReviewRemediationPreviewArgs),
    /// Apply one supported remediation action after explicit confirmation.
    Apply(ReviewRemediationApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewFindingIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review run identifier.
    pub(crate) review_id: String,
    /// Typed durable finding identifier within the review run.
    pub(crate) finding_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewRemediationPreviewArgs {
    #[command(flatten)]
    pub(crate) finding: ReviewFindingIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewRemediationApplyArgs {
    #[command(flatten)]
    pub(crate) finding: ReviewFindingIdArgs,
    /// Confirm applying the supported unlink-dangling-link remediation.
    #[arg(long)]
    pub(crate) confirm_unlink_dangling_link: bool,
    /// Replacement text for the removed id link. Defaults to the current link label.
    #[arg(long)]
    pub(crate) replacement_text: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewMarkArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review run identifier.
    pub(crate) review_id: String,
    /// Typed durable finding identifier within the review run.
    pub(crate) finding_id: String,
    /// New finding status: open, reviewed, dismissed, or accepted.
    pub(crate) status: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewDeleteArgs {
    #[command(flatten)]
    pub(crate) review: ReviewIdArgs,
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

pub(crate) fn run_review(args: &ReviewArgs) -> Result<(), CliCommandError> {
    match &args.command {
        ReviewCommand::List(command) => run_headless_command(command),
        ReviewCommand::Show(command) => run_headless_command(command),
        ReviewCommand::Diff(command) => run_headless_command(command),
        ReviewCommand::Remediation(command) => run_review_remediation(command),
        ReviewCommand::Mark(command) => run_review_mark(command),
        ReviewCommand::Delete(command) => run_headless_command(command),
    }
}

impl HeadlessCommand for ReviewListArgs {
    type Output = ListReviewRunsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_review_runs()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_list(output)
    }
}

impl HeadlessCommand for ReviewShowArgs {
    type Output = ReviewRunResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.review.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.review_run(&ReviewRunIdParams {
            review_id: self.review.review_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_run(&output.review)
    }
}

impl HeadlessCommand for ReviewDiffArgs {
    type Output = ReviewRunDiffResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.diff_review_runs(&ReviewRunDiffParams {
            base_review_id: self.base_review_id.clone(),
            target_review_id: self.target_review_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_diff(&output.diff)
    }
}

impl HeadlessCommand for ReviewRemediationPreviewArgs {
    type Output = ReviewFindingRemediationPreviewResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.finding.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.review_finding_remediation_preview(&ReviewFindingRemediationPreviewParams {
            review_id: self.finding.review_id.clone(),
            finding_id: self.finding.finding_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_remediation_preview(&output.preview)
    }
}

impl HeadlessCommand for ReviewDeleteArgs {
    type Output = DeleteReviewRunResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.review.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.delete_review_run(&ReviewRunIdParams {
            review_id: self.review.review_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!("deleted review: {}\n", output.review_id)
    }
}

fn run_review_remediation(command: &ReviewRemediationArgs) -> Result<(), CliCommandError> {
    match &command.command {
        ReviewRemediationCommand::Preview(command) => run_headless_command(command),
        ReviewRemediationCommand::Apply(command) => run_review_remediation_apply(command),
    }
}

fn run_review_remediation_apply(
    command: &ReviewRemediationApplyArgs,
) -> Result<(), CliCommandError> {
    let output_mode = command.finding.headless.output_mode();
    if !command.confirm_unlink_dangling_link {
        return Err(CliCommandError::new(
            output_mode,
            anyhow::anyhow!("review remediation apply requires --confirm-unlink-dangling-link"),
        ));
    }

    let mut client = command.finding.headless.connect()?;
    let preview = client
        .review_finding_remediation_preview(&ReviewFindingRemediationPreviewParams {
            review_id: command.finding.review_id.clone(),
            finding_id: command.finding.finding_id.clone(),
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?
        .preview;
    let action = remediation_action_from_preview(&preview, command.replacement_text.as_deref())
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let output = client
        .review_finding_remediation_apply(&ReviewFindingRemediationApplyParams {
            review_id: command.finding.review_id.clone(),
            finding_id: command.finding.finding_id.clone(),
            expected_preview: preview.preview_identity,
            action,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        render_review_remediation_application(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn remediation_action_from_preview(
    preview: &ReviewFindingRemediationPreview,
    replacement_text: Option<&str>,
) -> Result<AuditRemediationApplyAction> {
    match &preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview,
            ..
        } => {
            let replacement_text = match replacement_text {
                Some(value) => value.to_owned(),
                None => {
                    replacement_text_from_dangling_preview(preview, *column, missing_explicit_id)?
                }
            };
            Ok(AuditRemediationApplyAction::UnlinkDanglingLink {
                source_node_key: source.node_key.clone(),
                missing_explicit_id: missing_explicit_id.clone(),
                file_path: file_path.clone(),
                line: *line,
                column: *column,
                preview: preview.clone(),
                replacement_text,
            })
        }
        AuditRemediationPreviewPayload::DuplicateTitle { .. } => {
            anyhow::bail!(
                "review remediation apply currently supports only unlink-dangling-link findings"
            )
        }
    }
}

fn replacement_text_from_dangling_preview(
    preview: &str,
    column: u32,
    missing_explicit_id: &str,
) -> Result<String> {
    let link = org_link_at_preview_column(preview, column)
        .or_else(|| first_org_id_link_for_target(preview, missing_explicit_id))
        .ok_or_else(|| {
            anyhow::anyhow!("failed to derive unlink-dangling-link replacement text from preview")
        })?;
    let (target, label) = org_id_link_target_and_label(link).ok_or_else(|| {
        anyhow::anyhow!("failed to derive unlink-dangling-link replacement text from preview")
    })?;
    if target != missing_explicit_id {
        anyhow::bail!(
            "preview link target {target} does not match missing id {missing_explicit_id}"
        );
    }
    Ok(label.unwrap_or(target).to_owned())
}

fn org_link_at_preview_column(preview: &str, column: u32) -> Option<&str> {
    let link_start = byte_index_for_column(preview, column)?;
    let suffix = preview.get(link_start..)?;
    if !suffix.starts_with("[[") {
        return None;
    }
    let link_end = suffix.find("]]")? + 2;
    suffix.get(..link_end)
}

fn first_org_id_link_for_target<'a>(preview: &'a str, target: &str) -> Option<&'a str> {
    let mut search_start = 0_usize;
    while let Some(relative_start) = preview.get(search_start..)?.find("[[") {
        let start = search_start + relative_start;
        let suffix = preview.get(start..)?;
        let link_end = suffix.find("]]")? + 2;
        let link = suffix.get(..link_end)?;
        if org_id_link_target_and_label(link).is_some_and(|(candidate, _)| candidate == target) {
            return Some(link);
        }
        search_start = start + link_end;
    }
    None
}

fn byte_index_for_column(line: &str, column: u32) -> Option<usize> {
    if column == 0 {
        return None;
    }
    if column == 1 {
        return Some(0);
    }
    line.char_indices()
        .nth(column as usize - 1)
        .map(|(index, _)| index)
}

fn org_id_link_target_and_label(link: &str) -> Option<(&str, Option<&str>)> {
    let inner = link.strip_prefix("[[")?.strip_suffix("]]")?;
    let (target, label) = inner
        .split_once("][")
        .map_or((inner, None), |(target, label)| (target, Some(label)));
    target
        .trim()
        .strip_prefix("id:")
        .map(|id| (id.trim(), label))
}

fn run_review_mark(command: &ReviewMarkArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let status = parse_review_finding_status(&command.status)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let mut client = command.headless.connect()?;
    let output = client
        .mark_review_finding(&MarkReviewFindingParams {
            review_id: command.review_id.clone(),
            finding_id: command.finding_id.clone(),
            status,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        render_mark_review_finding_result(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
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
    let mut client = args.headless.connect_with_output_mode(error_output_mode)?;

    if let Some(save_review) = save_review {
        let saved = client
            .save_corpus_audit_review(&SaveCorpusAuditReviewParams {
                audit: kind,
                limit: args.limit,
                review_id: save_review.review_id,
                title: save_review.title,
                summary: save_review.summary,
                overwrite: save_review.overwrite,
            })
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        client
            .shutdown()
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        return write_saved_audit_command_output(args, report_format, saved, error_output_mode);
    }

    let result = client
        .corpus_audit(&CorpusAuditParams {
            audit: kind,
            limit: args.limit,
        })
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;

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
