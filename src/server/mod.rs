mod dispatch;
pub(crate) mod handlers;
pub mod operations;
mod rpc;
pub(crate) mod state;
mod workflows;

use std::io::{self, BufReader};
use std::path::PathBuf;

use anyhow::Result;
use slipbox_index::DiscoveryPolicy;
use slipbox_rpc::{JsonRpcErrorObject, JsonRpcResponse, read_framed_message, write_framed_message};

use self::dispatch::handle_request;
use crate::service::SlipboxService;

pub fn serve(
    root: PathBuf,
    db: PathBuf,
    workflow_dirs: Vec<PathBuf>,
    discovery: DiscoveryPolicy,
    read_only: bool,
) -> Result<()> {
    let mut service = SlipboxService::new(root, db, workflow_dirs, discovery)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    loop {
        match read_framed_message(&mut reader) {
            Ok(Some(request)) => {
                let response = handle_request(&mut service, request, read_only);
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
