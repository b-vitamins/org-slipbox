//! Service-facing contracts shared by protocol adapters.

use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;
use serde::de::DeserializeOwned;
use slipbox_engine::service::SlipboxService as EngineService;
use slipbox_engine::{DiscoveryPolicy, PlatformPolicy};
use slipbox_rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

pub use slipbox_engine::service::{
    FreshnessBehavior, OperationDescriptor, OperationFamily, OperationMutation,
    operation_descriptor_by_method, operation_descriptors,
};

/// A desktop embedding of the headless engine service.
pub struct SlipboxService {
    engine: EngineService,
}

impl SlipboxService {
    /// Open a service with desktop authority for encrypted sources.
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
            PlatformPolicy::desktop(),
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
        Ok(Self {
            engine: EngineService::with_platform(root, db, workflow_dirs, discovery, platform)?,
        })
    }

    pub fn invoke_value(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        self.engine.invoke_value(method, params)
    }

    pub fn invoke<P, R>(&mut self, method: &str, params: P) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        self.engine.invoke(method, params)
    }

    /// Dispatch one decoded JSON-RPC request, refusing a mutating method when
    /// `read_only`.
    pub fn dispatch(&mut self, request: JsonRpcRequest, read_only: bool) -> JsonRpcResponse {
        slipbox_engine::handle_request(&mut self.engine, request, read_only)
    }
}
