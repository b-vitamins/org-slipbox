use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use serde::de::DeserializeOwned;
use slipbox_core::{
    BacklinksParams, BacklinksResult, PingInfo, SearchNodesParams, SearchNodesResult,
};
use slipbox_rpc::{
    JsonRpcError, JsonRpcErrorObject, JsonRpcRequest, JsonRpcResponse, METHOD_BACKLINKS,
    METHOD_INDEX, METHOD_PING, METHOD_SEARCH_NODES,
};
use slipbox_store::Database;

#[derive(Debug, Parser)]
#[command(author, version, about = "Org slipbox tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the JSON-RPC daemon over stdio.
    Serve {
        /// Root directory containing Org files.
        #[arg(long)]
        root: PathBuf,
        /// SQLite database path.
        #[arg(long)]
        db: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { root, db } => serve(root, db),
    }
}

fn serve(root: PathBuf, db: PathBuf) -> Result<()> {
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

struct ServerState {
    root: PathBuf,
    db_path: PathBuf,
    database: Database,
}

impl ServerState {
    fn new(root: PathBuf, db_path: PathBuf) -> Result<Self> {
        let database = Database::open(&db_path)?;
        Ok(Self {
            root,
            db_path,
            database,
        })
    }
}

fn read_request(reader: &mut impl BufRead) -> Result<Option<JsonRpcRequest>> {
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .context("failed to read framing header")?;
        if bytes == 0 {
            return Ok(None);
        }

        if line == "\r\n" {
            break;
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        let (name, value) = trimmed
            .split_once(':')
            .with_context(|| format!("invalid header line: {trimmed}"))?;
        if name.eq_ignore_ascii_case("content-length") {
            let parsed = value
                .trim()
                .parse::<usize>()
                .with_context(|| format!("invalid content length: {}", value.trim()))?;
            content_length = Some(parsed);
        }
    }

    let length = content_length.context("missing Content-Length header")?;
    let mut body = vec![0_u8; length];
    reader
        .read_exact(&mut body)
        .context("failed to read framed body")?;

    let request = serde_json::from_slice(&body).context("invalid JSON-RPC request body")?;
    Ok(Some(request))
}

fn write_response(writer: &mut impl Write, response: &JsonRpcResponse) -> Result<()> {
    let body = serde_json::to_vec(response).context("failed to serialize JSON-RPC response")?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())
        .context("failed to write framing header")?;
    writer
        .write_all(&body)
        .context("failed to write JSON-RPC response body")?;
    writer
        .flush()
        .context("failed to flush JSON-RPC response")?;
    Ok(())
}

fn handle_request(state: &mut ServerState, request: JsonRpcRequest) -> JsonRpcResponse {
    let JsonRpcRequest { id, method, .. } = request;
    let id = id.unwrap_or(serde_json::Value::Null);

    let response = dispatch_request(state, &method, request.params);

    match response {
        Ok(result) => JsonRpcResponse::success(id, result),
        Err(error) => JsonRpcResponse::error(id, error.into_inner()),
    }
}

fn dispatch_request(
    state: &mut ServerState,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    match method {
        METHOD_PING => to_value(PingInfo {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            root: state.root.display().to_string(),
            db: state.db_path.display().to_string(),
        }),
        METHOD_INDEX => {
            let files = slipbox_index::scan_root(&state.root)
                .map_err(|error| internal_error(error.context("failed to scan Org files")))?;
            let stats = state
                .database
                .sync_index(&files)
                .map_err(|error| internal_error(error.context("failed to update SQLite index")))?;
            to_value(stats)
        }
        METHOD_SEARCH_NODES => {
            let params: SearchNodesParams = parse_params(params)?;
            let nodes = state
                .database
                .search_nodes(&params.query, params.normalized_limit())
                .map_err(|error| internal_error(error.context("failed to query nodes")))?;
            to_value(SearchNodesResult { nodes })
        }
        METHOD_BACKLINKS => {
            let params: BacklinksParams = parse_params(params)?;
            let backlinks = state
                .database
                .backlinks(&params.node_key, params.normalized_limit())
                .map_err(|error| internal_error(error.context("failed to query backlinks")))?;
            to_value(BacklinksResult { backlinks })
        }
        _ => Err(JsonRpcError::new(JsonRpcErrorObject::method_not_found(
            format!("unsupported method: {method}"),
        ))),
    }
}

fn parse_params<T>(params: serde_json::Value) -> Result<T, JsonRpcError>
where
    T: DeserializeOwned,
{
    let value = if params.is_null() {
        serde_json::json!({})
    } else {
        params
    };

    serde_json::from_value(value).map_err(|error| {
        JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
            "invalid request parameters: {error}"
        )))
    })
}

fn to_value<T>(value: T) -> Result<serde_json::Value, JsonRpcError>
where
    T: serde::Serialize,
{
    serde_json::to_value(value)
        .map_err(|error| internal_error(anyhow!("failed to serialize JSON-RPC result: {error}")))
}

fn internal_error(error: anyhow::Error) -> JsonRpcError {
    JsonRpcError::new(JsonRpcErrorObject::internal_error(error.to_string()))
}
