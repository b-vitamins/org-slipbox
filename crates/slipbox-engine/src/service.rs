//! Service-facing contracts shared by protocol adapters.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;
use slipbox_index::{DiscoveryPolicy, PlatformPolicy};
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

use crate::operations::operation_by_method;
pub use crate::operations::{
    FreshnessBehavior, OperationDescriptor, OperationFamily, OperationMutation,
    operation_descriptor_by_method, operation_descriptors,
};
use crate::state::ServerState;

pub struct SlipboxService {
    state: ServerState,
}

impl SlipboxService {
    /// Open a headless service, regardless of the build's enabled features.
    pub fn new(
        root: PathBuf,
        db: PathBuf,
        workflow_dirs: Vec<PathBuf>,
        discovery: DiscoveryPolicy,
    ) -> Result<Self> {
        Self::with_platform(
            root,
            db,
            workflow_dirs,
            discovery,
            PlatformPolicy::headless(),
        )
    }

    /// Open a service under an explicit platform authority.
    pub fn with_platform(
        root: PathBuf,
        db: PathBuf,
        workflow_dirs: Vec<PathBuf>,
        discovery: DiscoveryPolicy,
        platform: PlatformPolicy,
    ) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
        Ok(Self {
            state: ServerState::with_platform(root, db, workflow_dirs, discovery, platform)?,
        })
    }

    pub fn invoke_value(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let Some(operation) = operation_by_method(method) else {
            return Err(JsonRpcError::new(JsonRpcErrorObject::method_not_found(
                format!("unsupported method: {method}"),
            )));
        };

        operation.invoke(&mut self.state, params)
    }

    pub fn invoke<P, R>(&mut self, method: &str, params: P) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let params = serde_json::to_value(params).context("failed to encode service request")?;
        let value = self
            .invoke_value(method, params)
            .with_context(|| format!("service operation {method} failed"))?;
        serde_json::from_value(value)
            .with_context(|| format!("failed to decode service operation {method} result"))
    }
}
