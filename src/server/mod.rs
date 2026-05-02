mod dispatch;
mod handlers;
mod rpc;
mod state;

use std::io::{self, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use slipbox_index::DiscoveryPolicy;
use slipbox_rpc::{JsonRpcErrorObject, JsonRpcResponse, read_framed_message, write_framed_message};

use self::dispatch::handle_request;
use self::state::ServerState;

pub(crate) fn serve(root: PathBuf, db: PathBuf, discovery: DiscoveryPolicy) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let mut state = ServerState::new(root, db, discovery)?;
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
