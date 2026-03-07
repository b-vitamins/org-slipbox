mod dispatch;
mod framing;
mod handlers;
mod rpc;
mod state;

use std::io::{self, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use slipbox_rpc::{JsonRpcErrorObject, JsonRpcResponse};

use self::dispatch::handle_request;
use self::framing::{read_request, write_response};
use self::state::ServerState;

pub(crate) fn serve(root: PathBuf, db: PathBuf) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let mut state = ServerState::new(root, db)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    loop {
        match read_request(&mut reader) {
            Ok(Some(request)) => {
                let response = handle_request(&mut state, request);
                write_response(&mut writer, &response)?;
            }
            Ok(None) => break,
            Err(error) => {
                let response = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    JsonRpcErrorObject::parse_error(error.to_string()),
                );
                write_response(&mut writer, &response)?;
            }
        }
    }

    Ok(())
}
