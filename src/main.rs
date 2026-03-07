use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use serde::de::DeserializeOwned;
use slipbox_core::{
    AgendaParams, AgendaResult, AppendHeadingParams, AppendHeadingToNodeParams, BacklinksParams,
    BacklinksResult, CaptureNodeParams, EnsureFileNodeParams, EnsureNodeIdParams,
    ExtractSubtreeParams, IndexFileParams, NodeAtPointParams, NodeFromIdParams, NodeFromRefParams,
    NodeFromTitleOrAliasParams, NodeRecord, PingInfo, RandomNodeResult, RefileSubtreeParams,
    SearchNodesParams, SearchNodesResult, SearchRefsParams, SearchRefsResult, SearchTagsParams,
    SearchTagsResult, UpdateNodeMetadataParams,
};
use slipbox_rpc::{
    JsonRpcError, JsonRpcErrorObject, JsonRpcRequest, JsonRpcResponse, METHOD_AGENDA,
    METHOD_APPEND_HEADING, METHOD_APPEND_HEADING_TO_NODE, METHOD_BACKLINKS, METHOD_CAPTURE_NODE,
    METHOD_ENSURE_FILE_NODE, METHOD_ENSURE_NODE_ID, METHOD_EXTRACT_SUBTREE, METHOD_INDEX,
    METHOD_INDEX_FILE, METHOD_NODE_AT_POINT, METHOD_NODE_FROM_ID, METHOD_NODE_FROM_REF,
    METHOD_NODE_FROM_TITLE_OR_ALIAS, METHOD_PING, METHOD_RANDOM_NODE, METHOD_REFILE_SUBTREE,
    METHOD_SEARCH_NODES, METHOD_SEARCH_REFS, METHOD_SEARCH_TAGS, METHOD_UPDATE_NODE_METADATA,
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
        METHOD_RANDOM_NODE => {
            let node = state
                .database
                .random_node()
                .map_err(|error| internal_error(error.context("failed to query random node")))?;
            to_value(RandomNodeResult { node })
        }
        METHOD_SEARCH_TAGS => {
            let params: SearchTagsParams = parse_params(params)?;
            let tags = state
                .database
                .search_tags(&params.query, params.normalized_limit())
                .map_err(|error| internal_error(error.context("failed to query tags")))?;
            to_value(SearchTagsResult { tags })
        }
        METHOD_NODE_FROM_ID => {
            let params: NodeFromIdParams = parse_params(params)?;
            let node = state
                .database
                .node_from_id(&params.id)
                .map_err(|error| internal_error(error.context("failed to resolve node ID")))?;
            to_value(node)
        }
        METHOD_NODE_FROM_TITLE_OR_ALIAS => {
            let params: NodeFromTitleOrAliasParams = parse_params(params)?;
            let matches = state
                .database
                .node_from_title_or_alias(&params.title_or_alias, params.nocase)
                .map_err(|error| {
                    internal_error(error.context("failed to resolve node title or alias"))
                })?;
            if matches.len() > 1 {
                return Err(JsonRpcError::new(JsonRpcErrorObject::invalid_request(
                    format!("multiple nodes match {}", params.title_or_alias),
                )));
            }
            to_value(matches.into_iter().next())
        }
        METHOD_NODE_AT_POINT => {
            let params: NodeAtPointParams = parse_params(params)?;
            let (relative_path, _) = resolve_index_path(&state.root, &params.file_path)
                .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
            let node = state
                .database
                .node_at_point(&relative_path, params.normalized_line())
                .map_err(|error| {
                    internal_error(error.context("failed to resolve node at point"))
                })?;
            to_value(node)
        }
        METHOD_BACKLINKS => {
            let params: BacklinksParams = parse_params(params)?;
            let backlinks = state
                .database
                .backlinks(&params.node_key, params.normalized_limit())
                .map_err(|error| internal_error(error.context("failed to query backlinks")))?;
            to_value(BacklinksResult { backlinks })
        }
        METHOD_SEARCH_REFS => {
            let params: SearchRefsParams = parse_params(params)?;
            let refs = state
                .database
                .search_refs(&params.query, params.normalized_limit())
                .map_err(|error| internal_error(error.context("failed to query refs")))?;
            to_value(SearchRefsResult { refs })
        }
        METHOD_NODE_FROM_REF => {
            let params: NodeFromRefParams = parse_params(params)?;
            let node = state
                .database
                .node_from_ref(&params.reference)
                .map_err(|error| internal_error(error.context("failed to resolve ref")))?;
            to_value(node)
        }
        METHOD_AGENDA => {
            let params: AgendaParams = parse_params(params)?;
            let nodes = state
                .database
                .agenda_nodes(&params.start, &params.end, params.normalized_limit())
                .map_err(|error| internal_error(error.context("failed to query agenda")))?;
            to_value(AgendaResult { nodes })
        }
        METHOD_CAPTURE_NODE => {
            let params: CaptureNodeParams = parse_params(params)?;
            let refs = params.normalized_refs();
            let captured = match params.file_path.as_deref() {
                Some(file_path) => slipbox_write::capture_file_note_at_with_refs(
                    &state.root,
                    file_path,
                    &params.title,
                    &refs,
                ),
                None => {
                    slipbox_write::capture_file_note_with_refs(&state.root, &params.title, &refs)
                }
            }
            .map_err(|error| internal_error(error.context("failed to capture node")))?;
            sync_one_path(state, &captured.absolute_path)?;
            let node = read_required_node(state, &captured.node_key, "captured node")?;
            to_value(node)
        }
        METHOD_ENSURE_FILE_NODE => {
            let params: EnsureFileNodeParams = parse_params(params)?;
            let ensured =
                slipbox_write::ensure_file_note(&state.root, &params.file_path, &params.title)
                    .map_err(|error| internal_error(error.context("failed to ensure file node")))?;
            sync_one_path(state, &ensured.absolute_path)?;
            let node = read_required_node(state, &ensured.node_key, "ensured file node")?;
            to_value(node)
        }
        METHOD_APPEND_HEADING => {
            let params: AppendHeadingParams = parse_params(params)?;
            let captured = slipbox_write::append_heading(
                &state.root,
                &params.file_path,
                &params.title,
                &params.heading,
                params.normalized_level(),
            )
            .map_err(|error| internal_error(error.context("failed to append heading")))?;
            sync_one_path(state, &captured.absolute_path)?;
            let node = read_required_node(state, &captured.node_key, "captured heading")?;
            to_value(node)
        }
        METHOD_APPEND_HEADING_TO_NODE => {
            let params: AppendHeadingToNodeParams = parse_params(params)?;
            let target = state
                .database
                .node_by_key(&params.node_key)
                .map_err(|error| internal_error(error.context("failed to fetch target node")))?
                .ok_or_else(|| {
                    JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
                        "unknown node: {}",
                        params.node_key
                    )))
                })?;
            let captured =
                slipbox_write::append_heading_to_node(&state.root, &target, &params.heading)
                    .map_err(|error| {
                        internal_error(error.context("failed to append heading to node"))
                    })?;
            sync_one_path(state, &captured.absolute_path)?;
            let node = read_required_node(state, &captured.node_key, "captured heading")?;
            to_value(node)
        }
        METHOD_ENSURE_NODE_ID => {
            let params: EnsureNodeIdParams = parse_params(params)?;
            let node = state
                .database
                .node_by_key(&params.node_key)
                .map_err(|error| internal_error(error.context("failed to fetch node")))?
                .ok_or_else(|| {
                    JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
                        "unknown node: {}",
                        params.node_key
                    )))
                })?;

            if node.explicit_id.is_none() {
                let updated_path = slipbox_write::ensure_node_id(&state.root, &node)
                    .map_err(|error| internal_error(error.context("failed to assign node ID")))?;
                sync_one_path(state, &updated_path)?;
            }

            let refreshed = state
                .database
                .node_by_key(&params.node_key)
                .map_err(|error| internal_error(error.context("failed to read refreshed node")))?
                .ok_or_else(|| {
                    internal_error(anyhow!(
                        "node {} disappeared after ID update",
                        params.node_key
                    ))
                })?;
            to_value(refreshed)
        }
        METHOD_UPDATE_NODE_METADATA => {
            let params: UpdateNodeMetadataParams = parse_params(params)?;
            let node = read_known_node(state, &params.node_key, "node")?;
            let updated_path = slipbox_write::update_node_metadata(
                &state.root,
                &node,
                &slipbox_write::MetadataUpdate {
                    aliases: params.normalized_aliases(),
                    refs: params.normalized_refs(),
                    tags: params.normalized_tags(),
                },
            )
            .map_err(|error| internal_error(error.context("failed to update node metadata")))?;
            sync_one_path(state, &updated_path)?;
            let refreshed = read_required_node(state, &params.node_key, "updated node")?;
            to_value(refreshed)
        }
        METHOD_REFILE_SUBTREE => {
            let params: RefileSubtreeParams = parse_params(params)?;
            let source = read_known_node(state, &params.source_node_key, "source node")?;
            let target = read_known_node(state, &params.target_node_key, "target node")?;
            let outcome = slipbox_write::refile_subtree(&state.root, &source, &target)
                .map_err(|error| internal_error(error.context("failed to refile subtree")))?;
            sync_changed_paths(state, &outcome.changed_paths)?;
            remove_deleted_paths(state, &outcome.removed_paths)?;
            let moved = read_required_node_by_id(state, &outcome.explicit_id, "refiled node")?;
            to_value(moved)
        }
        METHOD_EXTRACT_SUBTREE => {
            let params: ExtractSubtreeParams = parse_params(params)?;
            let source = read_known_node(state, &params.source_node_key, "source node")?;
            let (relative_path, _) = resolve_index_path(&state.root, &params.file_path)
                .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
            let outcome = slipbox_write::extract_subtree(&state.root, &source, &relative_path)
                .map_err(|error| internal_error(error.context("failed to extract subtree")))?;
            sync_changed_paths(state, &outcome.changed_paths)?;
            let extracted =
                read_required_node_by_id(state, &outcome.explicit_id, "extracted node")?;
            to_value(extracted)
        }
        METHOD_INDEX_FILE => {
            let params: IndexFileParams = parse_params(params)?;
            let (relative_path, absolute_path) = resolve_index_path(&state.root, &params.file_path)
                .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
            if absolute_path.exists() {
                sync_one_path(state, &absolute_path)?;
            } else {
                state
                    .database
                    .remove_file_index(&relative_path)
                    .map_err(|error| {
                        internal_error(error.context("failed to remove file from SQLite index"))
                    })?;
            }
            to_value(serde_json::json!({ "file_path": relative_path }))
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

fn sync_one_path(state: &mut ServerState, path: &std::path::Path) -> Result<(), JsonRpcError> {
    let indexed_file = slipbox_index::scan_path(&state.root, path)
        .map_err(|error| internal_error(error.context("failed to scan updated file")))?;
    state
        .database
        .sync_file_index(&indexed_file)
        .map_err(|error| {
            internal_error(error.context("failed to sync updated file into SQLite"))
        })?;
    Ok(())
}

fn sync_changed_paths(state: &mut ServerState, paths: &[PathBuf]) -> Result<(), JsonRpcError> {
    for path in paths {
        sync_one_path(state, path)?;
    }
    Ok(())
}

fn remove_deleted_paths(state: &mut ServerState, paths: &[PathBuf]) -> Result<(), JsonRpcError> {
    for path in paths {
        let relative_path = path
            .strip_prefix(&state.root)
            .map_err(|_| {
                internal_error(anyhow!(
                    "{} is not under {}",
                    path.display(),
                    state.root.display()
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        state
            .database
            .remove_file_index(&relative_path)
            .map_err(|error| {
                internal_error(error.context("failed to remove file from SQLite index"))
            })?;
    }
    Ok(())
}

fn read_required_node(
    state: &mut ServerState,
    node_key: &str,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    state
        .database
        .node_by_key(node_key)
        .map_err(|error| internal_error(error.context(format!("failed to fetch {description}"))))?
        .ok_or_else(|| {
            internal_error(anyhow!(
                "{description} {node_key} was not found after indexing"
            ))
        })
}

fn read_required_node_by_id(
    state: &mut ServerState,
    explicit_id: &str,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    state
        .database
        .node_from_id(explicit_id)
        .map_err(|error| internal_error(error.context(format!("failed to fetch {description}"))))?
        .ok_or_else(|| {
            internal_error(anyhow!(
                "{description} {explicit_id} was not found after indexing"
            ))
        })
}

fn read_known_node(
    state: &mut ServerState,
    node_key: &str,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    state
        .database
        .node_by_key(node_key)
        .map_err(|error| internal_error(error.context(format!("failed to fetch {description}"))))?
        .ok_or_else(|| {
            JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
                "unknown {description}: {node_key}"
            )))
        })
}

fn resolve_index_path(root: &std::path::Path, file_path: &str) -> Result<(String, PathBuf)> {
    let candidate = PathBuf::from(file_path);
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        root.join(candidate)
    };
    let relative = absolute
        .strip_prefix(root)
        .map_err(|_| anyhow!("{} is not under {}", absolute.display(), root.display()))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok((relative, absolute))
}
