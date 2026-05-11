mod dispatch;
mod handlers;
mod rpc;
mod state;
mod workflows;

use std::io::{self, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use slipbox_core::{
    CorpusAuditParams, CorpusAuditResult, ImportWorkbenchPackParams, ImportWorkbenchPackResult,
    ListReviewRoutinesResult, ListReviewRunsResult, ListWorkbenchPacksResult, ListWorkflowsResult,
    MarkReviewFindingParams, MarkReviewFindingResult, ReviewFindingRemediationPreviewParams,
    ReviewFindingRemediationPreviewResult, ReviewRunDiffParams, ReviewRunDiffResult,
    ReviewRunIdParams, ReviewRunResult, RunReviewRoutineParams, RunReviewRoutineResult,
    RunWorkflowParams, RunWorkflowResult, SaveCorpusAuditReviewParams, SaveCorpusAuditReviewResult,
    SaveWorkflowReviewParams, SaveWorkflowReviewResult, ValidateWorkbenchPackParams,
    ValidateWorkbenchPackResult,
};
use slipbox_index::DiscoveryPolicy;
use slipbox_rpc::{JsonRpcErrorObject, JsonRpcResponse, read_framed_message, write_framed_message};

use self::dispatch::handle_request;
use self::state::ServerState;

#[allow(dead_code)]
pub(crate) struct WorkbenchBench {
    state: ServerState,
}

#[allow(dead_code)]
impl WorkbenchBench {
    pub(crate) fn new(
        root: PathBuf,
        db: PathBuf,
        workflow_dirs: Vec<PathBuf>,
        discovery: DiscoveryPolicy,
    ) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
        Ok(Self {
            state: ServerState::new(root, db, workflow_dirs, discovery)?,
        })
    }

    pub(crate) fn list_workflows(&mut self) -> Result<ListWorkflowsResult> {
        let value = handlers::query::list_workflows(&mut self.state, serde_json::json!({}))
            .context("workflow discovery benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode workflow discovery benchmark result")
    }

    pub(crate) fn run_workflow(&mut self, params: &RunWorkflowParams) -> Result<RunWorkflowResult> {
        let value = handlers::query::run_workflow(&mut self.state, serde_json::to_value(params)?)
            .context("workflow benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode workflow benchmark result")
    }

    pub(crate) fn corpus_audit(&mut self, params: &CorpusAuditParams) -> Result<CorpusAuditResult> {
        let value = handlers::query::corpus_audit(&mut self.state, serde_json::to_value(params)?)
            .context("corpus audit benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode corpus audit benchmark result")
    }

    pub(crate) fn list_review_runs(&mut self) -> Result<ListReviewRunsResult> {
        let value = handlers::query::list_review_runs(&mut self.state, serde_json::json!({}))
            .context("review list benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review list benchmark result")
    }

    pub(crate) fn review_run(&mut self, params: &ReviewRunIdParams) -> Result<ReviewRunResult> {
        let value = handlers::query::review_run(&mut self.state, serde_json::to_value(params)?)
            .context("review show benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review show benchmark result")
    }

    pub(crate) fn diff_review_runs(
        &mut self,
        params: &ReviewRunDiffParams,
    ) -> Result<ReviewRunDiffResult> {
        let value =
            handlers::query::diff_review_runs(&mut self.state, serde_json::to_value(params)?)
                .context("review diff benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review diff benchmark result")
    }

    pub(crate) fn review_finding_remediation_preview(
        &mut self,
        params: &ReviewFindingRemediationPreviewParams,
    ) -> Result<ReviewFindingRemediationPreviewResult> {
        let value = handlers::query::review_finding_remediation_preview(
            &mut self.state,
            serde_json::to_value(params)?,
        )
        .context("review remediation preview benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode review remediation preview benchmark result")
    }

    pub(crate) fn mark_review_finding(
        &mut self,
        params: &MarkReviewFindingParams,
    ) -> Result<MarkReviewFindingResult> {
        let value =
            handlers::query::mark_review_finding(&mut self.state, serde_json::to_value(params)?)
                .context("review mark benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review mark benchmark result")
    }

    pub(crate) fn save_corpus_audit_review(
        &mut self,
        params: &SaveCorpusAuditReviewParams,
    ) -> Result<SaveCorpusAuditReviewResult> {
        let value = handlers::query::save_corpus_audit_review(
            &mut self.state,
            serde_json::to_value(params)?,
        )
        .context("audit save-review benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode audit save-review benchmark result")
    }

    pub(crate) fn save_workflow_review(
        &mut self,
        params: &SaveWorkflowReviewParams,
    ) -> Result<SaveWorkflowReviewResult> {
        let value =
            handlers::query::save_workflow_review(&mut self.state, serde_json::to_value(params)?)
                .context("workflow save-review benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode workflow save-review benchmark result")
    }

    pub(crate) fn list_review_routines(&mut self) -> Result<ListReviewRoutinesResult> {
        let value = handlers::query::list_review_routines(&mut self.state, serde_json::json!({}))
            .context("review routine catalog benchmark request failed")?;
        serde_json::from_value(value)
            .context("failed to decode review routine catalog benchmark result")
    }

    pub(crate) fn run_review_routine(
        &mut self,
        params: &RunReviewRoutineParams,
    ) -> Result<RunReviewRoutineResult> {
        let value =
            handlers::query::run_review_routine(&mut self.state, serde_json::to_value(params)?)
                .context("review routine benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode review routine benchmark result")
    }

    pub(crate) fn import_workbench_pack(
        &mut self,
        params: &ImportWorkbenchPackParams,
    ) -> Result<ImportWorkbenchPackResult> {
        let value =
            handlers::query::import_workbench_pack(&mut self.state, serde_json::to_value(params)?)
                .context("workbench pack import benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode workbench pack import result")
    }

    pub(crate) fn validate_workbench_pack(
        &mut self,
        params: &ValidateWorkbenchPackParams,
    ) -> Result<ValidateWorkbenchPackResult> {
        let value = handlers::query::validate_workbench_pack(
            &mut self.state,
            serde_json::to_value(params)?,
        )
        .context("workbench pack validation benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode workbench pack validation result")
    }

    pub(crate) fn list_workbench_packs(&mut self) -> Result<ListWorkbenchPacksResult> {
        let value = handlers::query::list_workbench_packs(&mut self.state, serde_json::json!({}))
            .context("workbench pack catalog benchmark request failed")?;
        serde_json::from_value(value).context("failed to decode workbench pack catalog result")
    }
}

pub(crate) fn serve(
    root: PathBuf,
    db: PathBuf,
    workflow_dirs: Vec<PathBuf>,
    discovery: DiscoveryPolicy,
) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let mut state = ServerState::new(root, db, workflow_dirs, discovery)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    loop {
        match read_framed_message(&mut reader) {
            Ok(Some(request)) => {
                let response = handle_request(&mut state, request);
                write_framed_message(&mut writer, &response)?;
            }
            Ok(None) => break,
            Err(error) => {
                let response = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    JsonRpcErrorObject::parse_error(error.to_string()),
                );
                write_framed_message(&mut writer, &response)?;
            }
        }
    }

    Ok(())
}
