use serde::Deserialize;
use slipbox_core::{
    DeleteWorkbenchPackResult, ImportWorkbenchPackParams, ImportWorkbenchPackResult,
    ListWorkbenchPacksParams, ListWorkbenchPacksResult, ValidateWorkbenchPackParams,
    ValidateWorkbenchPackResult, WorkbenchPackCompatibilityEnvelope, WorkbenchPackIdParams,
    WorkbenchPackIssue, WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackResult,
    WorkbenchPackSummary,
};
use slipbox_rpc::JsonRpcError;

use super::super::common::{invalid_request, validate_pack_id_params};
use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;
use crate::server::workflows::discover_workflow_catalog;

fn save_workbench_pack_with_policy(
    state: &mut ServerState,
    pack: &WorkbenchPackManifest,
    overwrite: bool,
) -> Result<WorkbenchPackSummary, JsonRpcError> {
    if overwrite {
        state
            .database
            .save_workbench_pack(pack)
            .map_err(|error| internal_error(error.context("failed to save workbench pack")))?;
    } else if !state
        .database
        .save_workbench_pack_if_absent(pack)
        .map_err(|error| {
            internal_error(error.context("failed to save workbench pack without overwrite"))
        })?
    {
        return Err(invalid_request(format!(
            "workbench pack already exists: {}",
            pack.metadata.pack_id
        )));
    }

    Ok(WorkbenchPackSummary::from(pack))
}

fn known_workbench_pack(
    state: &ServerState,
    pack_id: &str,
) -> Result<WorkbenchPackManifest, JsonRpcError> {
    let pack = state
        .database
        .workbench_pack(pack_id)
        .map_err(|error| internal_error(error.context("failed to load workbench pack")))?;
    pack.ok_or_else(|| invalid_request(format!("unknown workbench pack: {pack_id}")))
}

#[derive(Debug, Deserialize)]
struct WorkbenchPackCompatibilityParams {
    pack: WorkbenchPackCompatibilityEnvelope,
}

fn workbench_pack_compatibility_issue(params: &serde_json::Value) -> Option<WorkbenchPackIssue> {
    let params = serde_json::from_value::<WorkbenchPackCompatibilityParams>(params.clone()).ok()?;
    params
        .pack
        .compatibility
        .validation_error()
        .map(|message| WorkbenchPackIssue {
            kind: WorkbenchPackIssueKind::UnsupportedVersion,
            asset_id: params.pack.pack_id,
            message,
        })
}

pub(crate) fn import_workbench_pack(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ImportWorkbenchPackParams = parse_params(params.clone()).map_err(|error| {
        workbench_pack_compatibility_issue(&params)
            .map(|issue| invalid_request(issue.message))
            .unwrap_or(error)
    })?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let pack = save_workbench_pack_with_policy(state, &params.pack, params.overwrite)?;
    to_value(ImportWorkbenchPackResult { pack })
}

pub(crate) fn workbench_pack(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: WorkbenchPackIdParams = parse_params(params)?;
    validate_pack_id_params(&params)?;
    to_value(WorkbenchPackResult {
        pack: known_workbench_pack(state, &params.pack_id)?,
    })
}

pub(crate) fn validate_workbench_pack(
    _state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ValidateWorkbenchPackParams = match parse_params(params.clone()) {
        Ok(params) => params,
        Err(error) => {
            if let Some(issue) = workbench_pack_compatibility_issue(&params) {
                return to_value(ValidateWorkbenchPackResult {
                    pack: None,
                    valid: false,
                    issues: vec![issue],
                });
            }
            return Err(error);
        }
    };
    let issues = params.pack.validation_issues();
    to_value(ValidateWorkbenchPackResult {
        pack: Some(params.pack.summary()),
        valid: issues.is_empty(),
        issues,
    })
}

pub(crate) fn export_workbench_pack(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: WorkbenchPackIdParams = parse_params(params)?;
    validate_pack_id_params(&params)?;
    to_value(known_workbench_pack(state, &params.pack_id)?)
}

pub(crate) fn list_workbench_packs(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let _params: ListWorkbenchPacksParams = parse_params(params)?;
    let packs = state
        .database
        .list_workbench_packs()
        .map_err(|error| internal_error(error.context("failed to list workbench packs")))?;
    let catalog = discover_workflow_catalog(&state.root, &state.workflow_dirs, &packs);
    to_value(ListWorkbenchPacksResult {
        packs: packs.iter().map(WorkbenchPackSummary::from).collect(),
        issues: catalog.issues().to_vec(),
    })
}

pub(crate) fn delete_workbench_pack(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: WorkbenchPackIdParams = parse_params(params)?;
    validate_pack_id_params(&params)?;
    if !state
        .database
        .delete_workbench_pack(&params.pack_id)
        .map_err(|error| internal_error(error.context("failed to delete workbench pack")))?
    {
        return Err(invalid_request(format!(
            "unknown workbench pack: {}",
            params.pack_id
        )));
    }
    to_value(DeleteWorkbenchPackResult {
        pack_id: params.pack_id,
    })
}
