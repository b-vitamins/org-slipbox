mod dispatch;
mod handlers;
mod rpc;
mod state;
mod workflows;

use std::io::{self, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use slipbox_core::{
    CorpusAuditParams, CorpusAuditResult, ListWorkflowsResult, RunWorkflowParams, RunWorkflowResult,
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
