use std::collections::VecDeque;
use std::path::Path;

use crate::rpc::{JsonRpcTransport, RpcClient};
use crate::{DaemonClient, DaemonClientError, DaemonServeConfig};
use serde_json::Value;
use serde_json::json;
use slipbox_core::*;
use slipbox_rpc::*;

#[derive(Default)]
struct MockTransport {
    requests: Vec<JsonRpcRequest>,
    responses: VecDeque<Result<JsonRpcResponse, DaemonClientError>>,
    shutdowns: usize,
}

impl MockTransport {
    fn with_response(response: JsonRpcResponse) -> Self {
        Self {
            requests: Vec::new(),
            responses: VecDeque::from([Ok(response)]),
            shutdowns: 0,
        }
    }
}

fn error_transport(response_count: u64) -> MockTransport {
    MockTransport {
        requests: Vec::new(),
        responses: (1..=response_count)
            .map(|id| {
                Ok(JsonRpcResponse::error(
                    json!(id),
                    JsonRpcErrorObject::invalid_request("contract stop".to_owned()),
                ))
            })
            .collect(),
        shutdowns: 0,
    }
}

fn expect_rpc_error<T>(result: Result<T, DaemonClientError>) {
    match result {
        Err(DaemonClientError::Rpc(error)) => {
            assert_eq!(error.code, -32600);
            assert_eq!(error.message, "contract stop");
        }
        Err(other) => panic!("expected queued RPC error, got {other:?}"),
        Ok(_) => panic!("method should surface queued RPC error"),
    }
}

fn assert_request(request: &JsonRpcRequest, method: &str, params: Value) {
    assert_eq!(request.method, method);
    assert_eq!(request.params, params);
}

fn sample_saved_artifact() -> SavedExplorationArtifact {
    SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "artifact/rpc-contract".to_owned(),
            title: "RPC Contract".to_owned(),
            summary: None,
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: "file:alpha.org".to_owned(),
                current_node_key: "file:alpha.org".to_owned(),
                lens: ExplorationLens::Structure,
                limit: 10,
                unique: false,
                frozen_context: false,
            }),
        },
    }
}

fn sample_review_run() -> ReviewRun {
    ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/rpc-contract".to_owned(),
            title: "RPC Contract Review".to_owned(),
            summary: None,
        },
        payload: ReviewRunPayload::Workflow {
            workflow: WorkflowSummary {
                metadata: WorkflowMetadata {
                    workflow_id: "workflow/rpc-contract".to_owned(),
                    title: "RPC Contract Workflow".to_owned(),
                    summary: None,
                },
                step_count: 1,
            },
            inputs: Vec::new(),
            step_ids: vec!["resolve".to_owned()],
        },
        findings: Vec::new(),
    }
}

fn sample_workbench_pack() -> WorkbenchPackManifest {
    WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: "pack/rpc-contract".to_owned(),
            title: "RPC Contract Pack".to_owned(),
            summary: None,
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: Vec::new(),
        review_routines: Vec::new(),
        report_profiles: Vec::new(),
        entrypoint_routine_ids: Vec::new(),
    }
}

fn structural_report_response(id: u64, operation: &str) -> JsonRpcResponse {
    JsonRpcResponse::success(
        json!(id),
        json!({
            "operation": operation,
            "changed_files": ["notes.org"],
            "removed_files": [],
            "index_refresh": "refreshed",
            "result": null
        }),
    )
}

fn remediation_apply_response(id: u64) -> JsonRpcResponse {
    JsonRpcResponse::success(
        json!(id),
        json!({
            "application": {
                "review_id": "review/audit/dangling-links",
                "finding_id": "audit/dangling-links/source/missing-id",
                "preview_identity": {
                    "kind": "dangling-link",
                    "source_node_key": "file:source.org",
                    "missing_explicit_id": "missing-id",
                    "file_path": "source.org",
                    "line": 6,
                    "column": 11,
                    "preview": "Points to [[id:missing-id][Missing]]."
                },
                "action": {
                    "kind": "unlink-dangling-link",
                    "source_node_key": "file:source.org",
                    "missing_explicit_id": "missing-id",
                    "file_path": "source.org",
                    "line": 6,
                    "column": 11,
                    "preview": "Points to [[id:missing-id][Missing]].",
                    "replacement_text": "Missing"
                },
                "changed_files": ["source.org"],
                "removed_files": [],
                "index_refresh": "refreshed"
            }
        }),
    )
}

fn slipbox_link_rewrite_preview_response(id: u64) -> JsonRpcResponse {
    JsonRpcResponse::success(
        json!(id),
        json!({
            "preview": {
                "file_path": "source.org",
                "rewrites": [{
                    "line": 3,
                    "column": 5,
                    "preview": "See [[slipbox:Target][Target Label]].",
                    "link_text": "[[slipbox:Target][Target Label]]",
                    "title_or_alias": "Target",
                    "description": "Target Label",
                    "target": {
                        "node_key": "file:target.org",
                        "explicit_id": null,
                        "file_path": "target.org",
                        "title": "Target",
                        "outline_path": "Target",
                        "aliases": [],
                        "tags": [],
                        "refs": [],
                        "todo_keyword": null,
                        "scheduled_for": null,
                        "deadline_for": null,
                        "closed_at": null,
                        "glossary": false,
                        "glossary_status": null,
                        "sr_due": null,
                        "sr_ease": null,
                        "sr_interval": null,
                        "sr_reps": null,
                        "sr_last": null,
                        "level": 1,
                        "line": 1,
                        "kind": "file",
                        "file_mtime_ns": 0,
                        "backlink_count": 0,
                        "forward_link_count": 0
                    },
                    "target_explicit_id": null,
                    "replacement": null
                }]
            }
        }),
    )
}

fn slipbox_link_rewrite_apply_response(id: u64) -> JsonRpcResponse {
    JsonRpcResponse::success(
        json!(id),
        json!({
            "application": {
                "file_path": "source.org",
                "rewrites": [{
                    "line": 3,
                    "column": 5,
                    "title_or_alias": "Target",
                    "target_node_key": "file:target.org",
                    "target_explicit_id": "target-id",
                    "replacement": "[[id:target-id][Target Label]]"
                }],
                "changed_files": ["source.org", "target.org"],
                "removed_files": [],
                "index_refresh": "refreshed"
            }
        }),
    )
}

impl JsonRpcTransport for MockTransport {
    fn round_trip(
        &mut self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, DaemonClientError> {
        self.requests.push(request);
        self.responses
            .pop_front()
            .expect("mock response should be queued")
    }

    fn shutdown(&mut self) -> Result<(), DaemonClientError> {
        self.shutdowns += 1;
        Ok(())
    }
}

#[test]
fn ping_uses_canonical_method_and_parses_typed_result() {
    let response = JsonRpcResponse::success(
        json!(1),
        serde_json::to_value(PingInfo {
            version: "0.6.1".to_owned(),
            root: "/tmp/notes".to_owned(),
            db: "/tmp/slipbox.sqlite".to_owned(),
        })
        .expect("ping result should serialize"),
    );
    let mut client = RpcClient::new(MockTransport::with_response(response));

    let result = client.ping().expect("ping should succeed");

    assert_eq!(result.root, "/tmp/notes");
    assert_eq!(client.transport.requests.len(), 1);
    assert_eq!(client.transport.requests[0].method, METHOD_PING);
    assert_eq!(client.transport.requests[0].params, Value::Null);
}

#[test]
fn search_nodes_sends_structured_params() {
    let response = JsonRpcResponse::success(
        json!(1),
        serde_json::to_value(SearchNodesResult { nodes: Vec::new() })
            .expect("search result should serialize"),
    );
    let mut client = RpcClient::new(MockTransport::with_response(response));

    let result = client
        .search_nodes(&SearchNodesParams {
            query: "alpha".to_owned(),
            limit: 25,
            sort: Some(SearchNodesSort::Title),
        })
        .expect("search should succeed");

    assert!(result.nodes.is_empty());
    assert_eq!(client.transport.requests[0].method, METHOD_SEARCH_NODES);
    assert_eq!(
        client.transport.requests[0].params,
        json!({"query": "alpha", "limit": 25, "sort": "title"})
    );
}

#[test]
fn everyday_read_methods_send_canonical_rpc_contracts() {
    let mut client = RpcClient::new(error_transport(28));

    expect_rpc_error(client.status());
    expect_rpc_error(client.index());
    expect_rpc_error(client.index_file(&IndexFileParams {
        file_path: "alpha.org".to_owned(),
    }));
    expect_rpc_error(client.indexed_files());
    expect_rpc_error(client.diagnose_file(&FileDiagnosticsParams {
        file_path: "alpha.org".to_owned(),
    }));
    expect_rpc_error(client.diagnose_node(&NodeDiagnosticsParams {
        node_key: "file:alpha.org".to_owned(),
    }));
    expect_rpc_error(client.diagnose_index());
    expect_rpc_error(client.search_files(&SearchFilesParams {
        query: "alpha".to_owned(),
        limit: 7,
    }));
    expect_rpc_error(client.search_occurrences(&SearchOccurrencesParams {
        query: "needle".to_owned(),
        limit: 8,
    }));
    expect_rpc_error(client.graph_dot(&GraphParams {
        root_node_key: Some("file:alpha.org".to_owned()),
        max_distance: Some(2),
        include_orphans: true,
        hidden_link_types: vec!["ref".to_owned()],
        max_title_length: 42,
        shorten_titles: Some(slipbox_core::GraphTitleShortening::Truncate),
        node_url_prefix: Some("org-protocol://node=".to_owned()),
    }));
    expect_rpc_error(client.random_node());
    expect_rpc_error(client.search_tags(&SearchTagsParams {
        query: "project".to_owned(),
        limit: 9,
    }));
    expect_rpc_error(client.node_from_id(&NodeFromIdParams {
        id: "alpha-id".to_owned(),
    }));
    expect_rpc_error(client.node_from_key(&NodeFromKeyParams {
        node_key: "file:alpha.org".to_owned(),
    }));
    expect_rpc_error(
        client.node_from_title_or_alias(&NodeFromTitleOrAliasParams {
            title_or_alias: "Alpha".to_owned(),
            nocase: true,
        }),
    );
    expect_rpc_error(client.node_from_ref(&NodeFromRefParams {
        reference: "cite:alpha2026".to_owned(),
    }));
    expect_rpc_error(client.node_at_point(&NodeAtPointParams {
        file_path: "alpha.org".to_owned(),
        line: 3,
    }));
    expect_rpc_error(client.anchor_at_point(&NodeAtPointParams {
        file_path: "alpha.org".to_owned(),
        line: 4,
    }));
    expect_rpc_error(client.anchor_from_key(&AnchorFromKeyParams {
        node_key: "heading:alpha.org:4".to_owned(),
    }));
    expect_rpc_error(client.read_file_source(&ReadFileSourceParams {
        file_path: "alpha.org".to_owned(),
        start_line: Some(3),
        max_lines: Some(20),
    }));
    expect_rpc_error(client.read_node_source(&ReadNodeSourceParams {
        node_key: "heading:alpha.org:4".to_owned(),
        context_before: Some(1),
        context_after: Some(2),
        max_lines: Some(30),
    }));
    expect_rpc_error(client.note_context(&NoteContextParams {
        node_key: "file:alpha.org".to_owned(),
        source_context_before: Some(1),
        source_context_after: Some(2),
        source_max_lines: Some(30),
        relation_limit: Some(12),
    }));
    expect_rpc_error(client.backlinks(&BacklinksParams {
        node_key: "file:alpha.org".to_owned(),
        limit: 10,
        unique: true,
    }));
    expect_rpc_error(client.forward_links(&ForwardLinksParams {
        node_key: "file:alpha.org".to_owned(),
        limit: 11,
        unique: false,
    }));
    expect_rpc_error(client.reflinks(&ReflinksParams {
        node_key: "file:alpha.org".to_owned(),
        limit: 12,
    }));
    expect_rpc_error(client.unlinked_references(&UnlinkedReferencesParams {
        node_key: "file:alpha.org".to_owned(),
        limit: 13,
    }));
    expect_rpc_error(client.explore(&ExploreParams {
        node_key: "file:alpha.org".to_owned(),
        lens: ExplorationLens::Refs,
        limit: 14,
        unique: false,
    }));
    expect_rpc_error(client.agenda(&AgendaParams {
        start: "2026-05-13T00:00:00".to_owned(),
        end: "2026-05-13T23:59:59".to_owned(),
        limit: 15,
    }));

    assert_eq!(client.transport.requests.len(), 28);
    assert_request(&client.transport.requests[0], METHOD_STATUS, Value::Null);
    assert_request(&client.transport.requests[1], METHOD_INDEX, Value::Null);
    assert_request(
        &client.transport.requests[2],
        METHOD_INDEX_FILE,
        json!({"file_path": "alpha.org"}),
    );
    assert_request(
        &client.transport.requests[3],
        METHOD_INDEXED_FILES,
        Value::Null,
    );
    assert_request(
        &client.transport.requests[4],
        METHOD_DIAGNOSE_FILE,
        json!({"file_path": "alpha.org"}),
    );
    assert_request(
        &client.transport.requests[5],
        METHOD_DIAGNOSE_NODE,
        json!({"node_key": "file:alpha.org"}),
    );
    assert_request(
        &client.transport.requests[6],
        METHOD_DIAGNOSE_INDEX,
        Value::Null,
    );
    assert_request(
        &client.transport.requests[7],
        METHOD_SEARCH_FILES,
        json!({"query": "alpha", "limit": 7}),
    );
    assert_request(
        &client.transport.requests[8],
        METHOD_SEARCH_OCCURRENCES,
        json!({"query": "needle", "limit": 8}),
    );
    assert_request(
        &client.transport.requests[9],
        METHOD_GRAPH_DOT,
        json!({
            "root_node_key": "file:alpha.org",
            "max_distance": 2,
            "include_orphans": true,
            "hidden_link_types": ["ref"],
            "max_title_length": 42,
            "shorten_titles": "truncate",
            "node_url_prefix": "org-protocol://node="
        }),
    );
    assert_request(
        &client.transport.requests[10],
        METHOD_RANDOM_NODE,
        Value::Null,
    );
    assert_request(
        &client.transport.requests[11],
        METHOD_SEARCH_TAGS,
        json!({"query": "project", "limit": 9}),
    );
    assert_request(
        &client.transport.requests[12],
        METHOD_NODE_FROM_ID,
        json!({"id": "alpha-id"}),
    );
    assert_request(
        &client.transport.requests[13],
        METHOD_NODE_FROM_KEY,
        json!({"node_key": "file:alpha.org"}),
    );
    assert_request(
        &client.transport.requests[14],
        METHOD_NODE_FROM_TITLE_OR_ALIAS,
        json!({"title_or_alias": "Alpha", "nocase": true}),
    );
    assert_request(
        &client.transport.requests[15],
        METHOD_NODE_FROM_REF,
        json!({"reference": "cite:alpha2026"}),
    );
    assert_request(
        &client.transport.requests[16],
        METHOD_NODE_AT_POINT,
        json!({"file_path": "alpha.org", "line": 3}),
    );
    assert_request(
        &client.transport.requests[17],
        METHOD_ANCHOR_AT_POINT,
        json!({"file_path": "alpha.org", "line": 4}),
    );
    assert_request(
        &client.transport.requests[18],
        METHOD_ANCHOR_FROM_KEY,
        json!({"node_key": "heading:alpha.org:4"}),
    );
    assert_request(
        &client.transport.requests[19],
        METHOD_READ_FILE_SOURCE,
        json!({"file_path": "alpha.org", "start_line": 3, "max_lines": 20}),
    );
    assert_request(
        &client.transport.requests[20],
        METHOD_READ_NODE_SOURCE,
        json!({
            "node_key": "heading:alpha.org:4",
            "context_before": 1,
            "context_after": 2,
            "max_lines": 30
        }),
    );
    assert_request(
        &client.transport.requests[21],
        METHOD_NOTE_CONTEXT,
        json!({
            "node_key": "file:alpha.org",
            "source_context_before": 1,
            "source_context_after": 2,
            "source_max_lines": 30,
            "relation_limit": 12
        }),
    );
    assert_request(
        &client.transport.requests[22],
        METHOD_BACKLINKS,
        json!({"node_key": "file:alpha.org", "limit": 10, "unique": true}),
    );
    assert_request(
        &client.transport.requests[23],
        METHOD_FORWARD_LINKS,
        json!({"node_key": "file:alpha.org", "limit": 11, "unique": false}),
    );
    assert_request(
        &client.transport.requests[24],
        METHOD_REFLINKS,
        json!({"node_key": "file:alpha.org", "limit": 12}),
    );
    assert_request(
        &client.transport.requests[25],
        METHOD_UNLINKED_REFERENCES,
        json!({"node_key": "file:alpha.org", "limit": 13}),
    );
    assert_request(
        &client.transport.requests[26],
        METHOD_EXPLORE,
        json!({
            "node_key": "file:alpha.org",
            "lens": "refs",
            "limit": 14,
            "unique": false
        }),
    );
    assert_request(
        &client.transport.requests[27],
        METHOD_AGENDA,
        json!({
            "start": "2026-05-13T00:00:00",
            "end": "2026-05-13T23:59:59",
            "limit": 15
        }),
    );
}

#[test]
fn comparison_and_reference_methods_send_canonical_rpc_contracts() {
    let mut client = RpcClient::new(error_transport(2));

    expect_rpc_error(client.search_refs(&SearchRefsParams {
        query: "alpha".to_owned(),
        limit: 16,
    }));
    expect_rpc_error(client.compare_notes(&CompareNotesParams {
        left_node_key: "file:left.org".to_owned(),
        right_node_key: "file:right.org".to_owned(),
        limit: 17,
    }));

    assert_request(
        &client.transport.requests[0],
        METHOD_SEARCH_REFS,
        json!({"query": "alpha", "limit": 16}),
    );
    assert_request(
        &client.transport.requests[1],
        METHOD_COMPARE_NOTES,
        json!({
            "left_node_key": "file:left.org",
            "right_node_key": "file:right.org",
            "limit": 17
        }),
    );
}

#[test]
fn everyday_write_methods_send_canonical_rpc_contracts() {
    let mut client = RpcClient::new(error_transport(9));

    expect_rpc_error(client.capture_node(&CaptureNodeParams {
        title: "Captured".to_owned(),
        file_path: Some("captured.org".to_owned()),
        head: Some("#+title: Captured\n".to_owned()),
        refs: vec!["cite:captured2026".to_owned()],
    }));
    let capture_template = CaptureTemplateParams {
        title: "Template".to_owned(),
        file_path: Some("template.org".to_owned()),
        node_key: None,
        head: None,
        outline_path: vec!["Inbox".to_owned()],
        capture_type: CaptureContentType::Plain,
        content: "Body".to_owned(),
        refs: vec!["cite:template2026".to_owned()],
        prepend: true,
        empty_lines_before: 1,
        empty_lines_after: 2,
        table_line_pos: None,
    };
    expect_rpc_error(client.capture_template(&capture_template));
    expect_rpc_error(
        client.capture_template_preview(&CaptureTemplatePreviewParams {
            capture: capture_template,
            source_override: Some("Source".to_owned()),
            ensure_node_id: true,
        }),
    );
    expect_rpc_error(client.ensure_file_node(&EnsureFileNodeParams {
        file_path: "ensured.org".to_owned(),
        title: "Ensured".to_owned(),
    }));
    expect_rpc_error(client.append_heading(&AppendHeadingParams {
        file_path: "ensured.org".to_owned(),
        title: "Ensured".to_owned(),
        heading: "Child".to_owned(),
        level: 2,
    }));
    expect_rpc_error(client.append_heading_to_node(&AppendHeadingToNodeParams {
        node_key: "file:ensured.org".to_owned(),
        heading: "Grandchild".to_owned(),
    }));
    expect_rpc_error(
        client.append_heading_at_outline_path(&AppendHeadingAtOutlinePathParams {
            file_path: "outline.org".to_owned(),
            heading: "Finding".to_owned(),
            outline_path: vec!["Inbox".to_owned(), "Review".to_owned()],
            head: Some("#+title: Outline\n".to_owned()),
        }),
    );
    expect_rpc_error(client.ensure_node_id(&EnsureNodeIdParams {
        node_key: "heading:outline.org:1".to_owned(),
    }));
    expect_rpc_error(client.update_node_metadata(&UpdateNodeMetadataParams {
        node_key: "file:ensured.org".to_owned(),
        aliases: Some(vec!["Alias".to_owned()]),
        refs: Some(vec!["cite:ensured2026".to_owned()]),
        tags: Some(vec!["tag".to_owned()]),
    }));

    assert_request(
        &client.transport.requests[0],
        METHOD_CAPTURE_NODE,
        json!({
            "title": "Captured",
            "file_path": "captured.org",
            "head": "#+title: Captured\n",
            "refs": ["cite:captured2026"]
        }),
    );
    assert_request(
        &client.transport.requests[1],
        METHOD_CAPTURE_TEMPLATE,
        json!({
            "title": "Template",
            "file_path": "template.org",
            "node_key": null,
            "head": null,
            "outline_path": ["Inbox"],
            "capture_type": "plain",
            "content": "Body",
            "refs": ["cite:template2026"],
            "prepend": true,
            "empty_lines_before": 1,
            "empty_lines_after": 2,
            "table_line_pos": null
        }),
    );
    assert_request(
        &client.transport.requests[2],
        METHOD_CAPTURE_TEMPLATE_PREVIEW,
        json!({
            "title": "Template",
            "file_path": "template.org",
            "node_key": null,
            "head": null,
            "outline_path": ["Inbox"],
            "capture_type": "plain",
            "content": "Body",
            "refs": ["cite:template2026"],
            "prepend": true,
            "empty_lines_before": 1,
            "empty_lines_after": 2,
            "table_line_pos": null,
            "source_override": "Source",
            "ensure_node_id": true
        }),
    );
    assert_request(
        &client.transport.requests[3],
        METHOD_ENSURE_FILE_NODE,
        json!({"file_path": "ensured.org", "title": "Ensured"}),
    );
    assert_request(
        &client.transport.requests[4],
        METHOD_APPEND_HEADING,
        json!({
            "file_path": "ensured.org",
            "title": "Ensured",
            "heading": "Child",
            "level": 2
        }),
    );
    assert_request(
        &client.transport.requests[5],
        METHOD_APPEND_HEADING_TO_NODE,
        json!({"node_key": "file:ensured.org", "heading": "Grandchild"}),
    );
    assert_request(
        &client.transport.requests[6],
        METHOD_APPEND_HEADING_AT_OUTLINE_PATH,
        json!({
            "file_path": "outline.org",
            "heading": "Finding",
            "outline_path": ["Inbox", "Review"],
            "head": "#+title: Outline\n"
        }),
    );
    assert_request(
        &client.transport.requests[7],
        METHOD_ENSURE_NODE_ID,
        json!({"node_key": "heading:outline.org:1"}),
    );
    assert_request(
        &client.transport.requests[8],
        METHOD_UPDATE_NODE_METADATA,
        json!({
            "node_key": "file:ensured.org",
            "aliases": ["Alias"],
            "refs": ["cite:ensured2026"],
            "tags": ["tag"]
        }),
    );
}

#[test]
fn workflow_review_artifact_and_pack_methods_send_canonical_rpc_contracts() {
    let mut client = RpcClient::new(error_transport(27));
    let focus_input = WorkflowInputAssignment {
        input_id: "focus".to_owned(),
        target: WorkflowResolveTarget::NodeKey {
            node_key: "file:alpha.org".to_owned(),
        },
    };
    let saved_artifact = sample_saved_artifact();
    let review = sample_review_run();
    let pack = sample_workbench_pack();

    expect_rpc_error(client.list_workflows());
    expect_rpc_error(client.workflow(&WorkflowIdParams {
        workflow_id: "workflow/rpc-contract".to_owned(),
    }));
    expect_rpc_error(client.run_workflow(&RunWorkflowParams {
        workflow_id: "workflow/rpc-contract".to_owned(),
        inputs: vec![focus_input.clone()],
    }));
    expect_rpc_error(client.list_review_routines());
    expect_rpc_error(client.review_routine(&ReviewRoutineIdParams {
        routine_id: "routine/rpc-contract".to_owned(),
    }));
    expect_rpc_error(client.run_review_routine(&RunReviewRoutineParams {
        routine_id: "routine/rpc-contract".to_owned(),
        inputs: vec![focus_input.clone()],
    }));
    expect_rpc_error(client.corpus_audit(&CorpusAuditParams {
        audit: CorpusAuditKind::DanglingLinks,
        limit: 18,
    }));
    expect_rpc_error(
        client.save_exploration_artifact(&SaveExplorationArtifactParams {
            artifact: saved_artifact,
            overwrite: true,
        }),
    );
    expect_rpc_error(client.exploration_artifact(&ExplorationArtifactIdParams {
        artifact_id: "artifact/rpc-contract".to_owned(),
    }));
    expect_rpc_error(client.list_exploration_artifacts());
    expect_rpc_error(
        client.delete_exploration_artifact(&ExplorationArtifactIdParams {
            artifact_id: "artifact/rpc-contract".to_owned(),
        }),
    );
    expect_rpc_error(
        client.execute_exploration_artifact(&ExplorationArtifactIdParams {
            artifact_id: "artifact/rpc-contract".to_owned(),
        }),
    );
    expect_rpc_error(client.save_review_run(&SaveReviewRunParams {
        review,
        overwrite: true,
    }));
    expect_rpc_error(client.review_run(&ReviewRunIdParams {
        review_id: "review/rpc-contract".to_owned(),
    }));
    expect_rpc_error(client.diff_review_runs(&ReviewRunDiffParams {
        base_review_id: "review/base".to_owned(),
        target_review_id: "review/target".to_owned(),
    }));
    expect_rpc_error(client.review_finding_remediation_preview(
        &ReviewFindingRemediationPreviewParams {
            review_id: "review/rpc-contract".to_owned(),
            finding_id: "finding/rpc-contract".to_owned(),
        },
    ));
    expect_rpc_error(client.list_review_runs());
    expect_rpc_error(client.delete_review_run(&ReviewRunIdParams {
        review_id: "review/rpc-contract".to_owned(),
    }));
    expect_rpc_error(client.mark_review_finding(&MarkReviewFindingParams {
        review_id: "review/rpc-contract".to_owned(),
        finding_id: "finding/rpc-contract".to_owned(),
        status: ReviewFindingStatus::Reviewed,
    }));
    expect_rpc_error(
        client.save_corpus_audit_review(&SaveCorpusAuditReviewParams {
            audit: CorpusAuditKind::DuplicateTitles,
            limit: 19,
            review_id: Some("review/audit/rpc-contract".to_owned()),
            title: Some("Audit Contract".to_owned()),
            summary: None,
            overwrite: true,
        }),
    );
    expect_rpc_error(client.save_workflow_review(&SaveWorkflowReviewParams {
        workflow_id: "workflow/rpc-contract".to_owned(),
        inputs: vec![focus_input],
        review_id: Some("review/workflow/rpc-contract".to_owned()),
        title: Some("Workflow Contract".to_owned()),
        summary: Some("Summary".to_owned()),
        overwrite: false,
    }));
    expect_rpc_error(client.import_workbench_pack(&ImportWorkbenchPackParams {
        pack: pack.clone(),
        overwrite: true,
    }));
    expect_rpc_error(client.workbench_pack(&WorkbenchPackIdParams {
        pack_id: "pack/rpc-contract".to_owned(),
    }));
    expect_rpc_error(client.validate_workbench_pack(&ValidateWorkbenchPackParams { pack }));
    expect_rpc_error(client.export_workbench_pack(&WorkbenchPackIdParams {
        pack_id: "pack/rpc-contract".to_owned(),
    }));
    expect_rpc_error(client.list_workbench_packs());
    expect_rpc_error(client.delete_workbench_pack(&WorkbenchPackIdParams {
        pack_id: "pack/rpc-contract".to_owned(),
    }));

    assert_request(
        &client.transport.requests[0],
        METHOD_LIST_WORKFLOWS,
        json!({}),
    );
    assert_request(
        &client.transport.requests[1],
        METHOD_WORKFLOW,
        json!({"workflow_id": "workflow/rpc-contract"}),
    );
    assert_request(
        &client.transport.requests[2],
        METHOD_RUN_WORKFLOW,
        json!({
            "workflow_id": "workflow/rpc-contract",
            "inputs": [{
                "input_id": "focus",
                "kind": "node-key",
                "node_key": "file:alpha.org"
            }]
        }),
    );
    assert_request(
        &client.transport.requests[3],
        METHOD_LIST_REVIEW_ROUTINES,
        json!({}),
    );
    assert_request(
        &client.transport.requests[4],
        METHOD_REVIEW_ROUTINE,
        json!({"routine_id": "routine/rpc-contract"}),
    );
    assert_request(
        &client.transport.requests[5],
        METHOD_RUN_REVIEW_ROUTINE,
        json!({
            "routine_id": "routine/rpc-contract",
            "inputs": [{
                "input_id": "focus",
                "kind": "node-key",
                "node_key": "file:alpha.org"
            }]
        }),
    );
    assert_request(
        &client.transport.requests[6],
        METHOD_CORPUS_AUDIT,
        json!({"audit": "dangling-links", "limit": 18}),
    );
    assert_eq!(
        client.transport.requests[7].method,
        METHOD_SAVE_EXPLORATION_ARTIFACT
    );
    assert_eq!(client.transport.requests[7].params["overwrite"], true);
    assert_eq!(
        client.transport.requests[7].params["artifact"]["artifact_id"],
        "artifact/rpc-contract"
    );
    assert_request(
        &client.transport.requests[8],
        METHOD_EXPLORATION_ARTIFACT,
        json!({"artifact_id": "artifact/rpc-contract"}),
    );
    assert_request(
        &client.transport.requests[9],
        METHOD_LIST_EXPLORATION_ARTIFACTS,
        json!({}),
    );
    assert_request(
        &client.transport.requests[10],
        METHOD_DELETE_EXPLORATION_ARTIFACT,
        json!({"artifact_id": "artifact/rpc-contract"}),
    );
    assert_request(
        &client.transport.requests[11],
        METHOD_EXECUTE_EXPLORATION_ARTIFACT,
        json!({"artifact_id": "artifact/rpc-contract"}),
    );
    assert_eq!(client.transport.requests[12].method, METHOD_SAVE_REVIEW_RUN);
    assert_eq!(client.transport.requests[12].params["overwrite"], true);
    assert_eq!(
        client.transport.requests[12].params["review"]["review_id"],
        "review/rpc-contract"
    );
    assert_request(
        &client.transport.requests[13],
        METHOD_REVIEW_RUN,
        json!({"review_id": "review/rpc-contract"}),
    );
    assert_request(
        &client.transport.requests[14],
        METHOD_DIFF_REVIEW_RUNS,
        json!({"base_review_id": "review/base", "target_review_id": "review/target"}),
    );
    assert_request(
        &client.transport.requests[15],
        METHOD_REVIEW_FINDING_REMEDIATION_PREVIEW,
        json!({
            "review_id": "review/rpc-contract",
            "finding_id": "finding/rpc-contract"
        }),
    );
    assert_request(
        &client.transport.requests[16],
        METHOD_LIST_REVIEW_RUNS,
        json!({}),
    );
    assert_request(
        &client.transport.requests[17],
        METHOD_DELETE_REVIEW_RUN,
        json!({"review_id": "review/rpc-contract"}),
    );
    assert_request(
        &client.transport.requests[18],
        METHOD_MARK_REVIEW_FINDING,
        json!({
            "review_id": "review/rpc-contract",
            "finding_id": "finding/rpc-contract",
            "status": "reviewed"
        }),
    );
    assert_request(
        &client.transport.requests[19],
        METHOD_SAVE_CORPUS_AUDIT_REVIEW,
        json!({
            "audit": "duplicate-titles",
            "limit": 19,
            "review_id": "review/audit/rpc-contract",
            "title": "Audit Contract",
            "summary": null,
            "overwrite": true
        }),
    );
    assert_request(
        &client.transport.requests[20],
        METHOD_SAVE_WORKFLOW_REVIEW,
        json!({
            "workflow_id": "workflow/rpc-contract",
            "inputs": [{
                "input_id": "focus",
                "kind": "node-key",
                "node_key": "file:alpha.org"
            }],
            "review_id": "review/workflow/rpc-contract",
            "title": "Workflow Contract",
            "summary": "Summary",
            "overwrite": false
        }),
    );
    assert_eq!(
        client.transport.requests[21].method,
        METHOD_IMPORT_WORKBENCH_PACK
    );
    assert_eq!(client.transport.requests[21].params["overwrite"], true);
    assert_eq!(
        client.transport.requests[21].params["pack"]["pack_id"],
        "pack/rpc-contract"
    );
    assert_request(
        &client.transport.requests[22],
        METHOD_WORKBENCH_PACK,
        json!({"pack_id": "pack/rpc-contract"}),
    );
    assert_eq!(
        client.transport.requests[23].method,
        METHOD_VALIDATE_WORKBENCH_PACK
    );
    assert_eq!(
        client.transport.requests[23].params["pack"]["pack_id"],
        "pack/rpc-contract"
    );
    assert_request(
        &client.transport.requests[24],
        METHOD_EXPORT_WORKBENCH_PACK,
        json!({"pack_id": "pack/rpc-contract"}),
    );
    assert_request(
        &client.transport.requests[25],
        METHOD_LIST_WORKBENCH_PACKS,
        json!({}),
    );
    assert_request(
        &client.transport.requests[26],
        METHOD_DELETE_WORKBENCH_PACK,
        json!({"pack_id": "pack/rpc-contract"}),
    );
}

#[test]
fn structural_rewrite_methods_send_canonical_params() {
    let mut client = RpcClient::new(MockTransport {
        requests: Vec::new(),
        responses: VecDeque::from([
            Ok(structural_report_response(1, "refile-subtree")),
            Ok(structural_report_response(2, "refile-region")),
            Ok(structural_report_response(3, "extract-subtree")),
            Ok(structural_report_response(4, "promote-file")),
            Ok(structural_report_response(5, "demote-file")),
        ]),
        shutdowns: 0,
    });

    let refile_subtree = client
        .refile_subtree(&RefileSubtreeParams {
            source_node_key: "heading:source.org:4".to_owned(),
            target_node_key: "heading:target.org:2".to_owned(),
        })
        .expect("refile subtree should parse report");
    let refile_region = client
        .refile_region(&RefileRegionParams {
            file_path: "source.org".to_owned(),
            start: 4,
            end: 8,
            target_node_key: "heading:target.org:2".to_owned(),
        })
        .expect("refile region should parse report");
    let extract_subtree = client
        .extract_subtree(&ExtractSubtreeParams {
            source_node_key: "heading:source.org:10".to_owned(),
            file_path: "extracted.org".to_owned(),
        })
        .expect("extract subtree should parse report");
    let promote = client
        .promote_entire_file(&RewriteFileParams {
            file_path: "promote.org".to_owned(),
        })
        .expect("promote should parse report");
    let demote = client
        .demote_entire_file(&RewriteFileParams {
            file_path: "demote.org".to_owned(),
        })
        .expect("demote should parse report");

    assert_eq!(
        refile_subtree.operation,
        slipbox_core::StructuralWriteOperationKind::RefileSubtree
    );
    assert_eq!(
        refile_region.operation,
        slipbox_core::StructuralWriteOperationKind::RefileRegion
    );
    assert_eq!(
        extract_subtree.operation,
        slipbox_core::StructuralWriteOperationKind::ExtractSubtree
    );
    assert_eq!(
        promote.operation,
        slipbox_core::StructuralWriteOperationKind::PromoteFile
    );
    assert_eq!(
        demote.operation,
        slipbox_core::StructuralWriteOperationKind::DemoteFile
    );

    assert_eq!(client.transport.requests[0].method, METHOD_REFILE_SUBTREE);
    assert_eq!(
        client.transport.requests[0].params,
        json!({
            "source_node_key": "heading:source.org:4",
            "target_node_key": "heading:target.org:2"
        })
    );
    assert_eq!(client.transport.requests[1].method, METHOD_REFILE_REGION);
    assert_eq!(
        client.transport.requests[1].params,
        json!({
            "file_path": "source.org",
            "start": 4,
            "end": 8,
            "target_node_key": "heading:target.org:2"
        })
    );
    assert_eq!(client.transport.requests[2].method, METHOD_EXTRACT_SUBTREE);
    assert_eq!(
        client.transport.requests[2].params,
        json!({
            "source_node_key": "heading:source.org:10",
            "file_path": "extracted.org"
        })
    );
    assert_eq!(
        client.transport.requests[3].method,
        METHOD_PROMOTE_ENTIRE_FILE
    );
    assert_eq!(
        client.transport.requests[3].params,
        json!({"file_path": "promote.org"})
    );
    assert_eq!(
        client.transport.requests[4].method,
        METHOD_DEMOTE_ENTIRE_FILE
    );
    assert_eq!(
        client.transport.requests[4].params,
        json!({"file_path": "demote.org"})
    );
}

#[test]
fn remediation_apply_sends_canonical_params() {
    let mut client = RpcClient::new(MockTransport {
        requests: Vec::new(),
        responses: VecDeque::from([Ok(remediation_apply_response(1))]),
        shutdowns: 0,
    });

    let expected_preview = slipbox_core::AuditRemediationPreviewIdentity::DanglingLink {
        source_node_key: "file:source.org".to_owned(),
        missing_explicit_id: "missing-id".to_owned(),
        file_path: "source.org".to_owned(),
        line: 6,
        column: 11,
        preview: "Points to [[id:missing-id][Missing]].".to_owned(),
    };
    let action = slipbox_core::AuditRemediationApplyAction::UnlinkDanglingLink {
        source_node_key: "file:source.org".to_owned(),
        missing_explicit_id: "missing-id".to_owned(),
        file_path: "source.org".to_owned(),
        line: 6,
        column: 11,
        preview: "Points to [[id:missing-id][Missing]].".to_owned(),
        replacement_text: "Missing".to_owned(),
    };
    let result = client
        .review_finding_remediation_apply(&ReviewFindingRemediationApplyParams {
            review_id: "review/audit/dangling-links".to_owned(),
            finding_id: "audit/dangling-links/source/missing-id".to_owned(),
            expected_preview: expected_preview.clone(),
            action: action.clone(),
        })
        .expect("apply result should parse");

    assert_eq!(result.application.preview_identity, expected_preview);
    assert_eq!(result.application.action, action);
    assert_eq!(
        result.application.affected_files.changed_files,
        vec!["source.org".to_owned()]
    );
    assert_eq!(
        client.transport.requests[0].method,
        METHOD_REVIEW_FINDING_REMEDIATION_APPLY
    );
    assert_eq!(
        client.transport.requests[0].params,
        json!({
            "review_id": "review/audit/dangling-links",
            "finding_id": "audit/dangling-links/source/missing-id",
            "expected_preview": {
                "kind": "dangling-link",
                "source_node_key": "file:source.org",
                "missing_explicit_id": "missing-id",
                "file_path": "source.org",
                "line": 6,
                "column": 11,
                "preview": "Points to [[id:missing-id][Missing]]."
            },
            "action": {
                "kind": "unlink-dangling-link",
                "source_node_key": "file:source.org",
                "missing_explicit_id": "missing-id",
                "file_path": "source.org",
                "line": 6,
                "column": 11,
                "preview": "Points to [[id:missing-id][Missing]].",
                "replacement_text": "Missing"
            }
        })
    );
}

#[test]
fn slipbox_link_rewrite_methods_send_canonical_params() {
    let mut client = RpcClient::new(MockTransport {
        requests: Vec::new(),
        responses: VecDeque::from([
            Ok(slipbox_link_rewrite_preview_response(1)),
            Ok(slipbox_link_rewrite_apply_response(2)),
        ]),
        shutdowns: 0,
    });

    let preview_result = client
        .slipbox_link_rewrite_preview(&SlipboxLinkRewritePreviewParams {
            file_path: "source.org".to_owned(),
        })
        .expect("preview result should parse");
    let apply_result = client
        .slipbox_link_rewrite_apply(&SlipboxLinkRewriteApplyParams {
            expected_preview: preview_result.preview.clone(),
        })
        .expect("apply result should parse");

    assert_eq!(preview_result.preview.rewrites.len(), 1);
    assert_eq!(apply_result.application.rewrites.len(), 1);
    assert_eq!(
        client.transport.requests[0].method,
        METHOD_SLIPBOX_LINK_REWRITE_PREVIEW
    );
    assert_eq!(
        client.transport.requests[0].params,
        json!({"file_path": "source.org"})
    );
    assert_eq!(
        client.transport.requests[1].method,
        METHOD_SLIPBOX_LINK_REWRITE_APPLY
    );
    assert_eq!(
        client.transport.requests[1].params,
        json!({
            "expected_preview": {
                "file_path": "source.org",
                "rewrites": [{
                    "line": 3,
                    "column": 5,
                    "preview": "See [[slipbox:Target][Target Label]].",
                    "link_text": "[[slipbox:Target][Target Label]]",
                    "title_or_alias": "Target",
                    "description": "Target Label",
                    "target": {
                        "node_key": "file:target.org",
                        "explicit_id": null,
                        "file_path": "target.org",
                        "title": "Target",
                        "outline_path": "Target",
                        "aliases": [],
                        "tags": [],
                        "refs": [],
                        "todo_keyword": null,
                        "scheduled_for": null,
                        "deadline_for": null,
                        "closed_at": null,
                        "glossary": false,
                        "glossary_status": null,
                        "sr_due": null,
                        "sr_ease": null,
                        "sr_interval": null,
                        "sr_reps": null,
                        "sr_last": null,
                        "level": 1,
                        "line": 1,
                        "kind": "file",
                        "file_mtime_ns": 0,
                        "backlink_count": 0,
                        "forward_link_count": 0
                    },
                    "target_explicit_id": null,
                    "replacement": null
                }]
            }
        })
    );
}

#[test]
fn structural_rewrite_methods_map_rpc_errors() {
    let response = JsonRpcResponse::error(
        json!(1),
        JsonRpcErrorObject::invalid_request("unknown source node: missing".to_owned()),
    );
    let mut client = RpcClient::new(MockTransport::with_response(response));

    let error = client
        .refile_subtree(&RefileSubtreeParams {
            source_node_key: "missing".to_owned(),
            target_node_key: "heading:target.org:2".to_owned(),
        })
        .expect_err("rpc error should surface");

    match error {
        DaemonClientError::Rpc(error) => {
            assert_eq!(error.code, -32600);
            assert_eq!(error.message, "unknown source node: missing");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn maps_rpc_error_responses() {
    let response = JsonRpcResponse::error(
        json!(1),
        JsonRpcErrorObject::invalid_request("bad request".to_owned()),
    );
    let mut client = RpcClient::new(MockTransport::with_response(response));

    let error = client
        .explore(&ExploreParams {
            node_key: "file:alpha.org".to_owned(),
            lens: ExplorationLens::Structure,
            limit: 10,
            unique: false,
        })
        .expect_err("rpc error should surface");

    match error {
        DaemonClientError::Rpc(error) => {
            assert_eq!(error.code, -32600);
            assert_eq!(error.message, "bad request");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn rejects_response_id_mismatch() {
    let response = JsonRpcResponse::success(
        json!(99),
        serde_json::to_value(StatusInfo {
            version: "0.6.1".to_owned(),
            root: "/tmp/notes".to_owned(),
            db: "/tmp/slipbox.sqlite".to_owned(),
            files_indexed: 1,
            nodes_indexed: 1,
            links_indexed: 0,
        })
        .expect("status should serialize"),
    );
    let mut client = RpcClient::new(MockTransport::with_response(response));

    let error = client.status().expect_err("mismatched id should fail");

    match error {
        DaemonClientError::ResponseIdMismatch { expected, actual } => {
            assert_eq!(expected, "1");
            assert_eq!(actual, "99");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn rejects_malformed_results() {
    let response = JsonRpcResponse::success(json!(1), json!({"version": 7}));
    let mut client = RpcClient::new(MockTransport::with_response(response));

    let error = client.status().expect_err("malformed result should fail");

    match error {
        DaemonClientError::MalformedResult { method, .. } => {
            assert_eq!(method, METHOD_STATUS);
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn startup_failure_is_explicit() {
    let config = DaemonServeConfig::new("/tmp/notes", "/tmp/slipbox.sqlite");

    let result = DaemonClient::spawn(Path::new("/definitely/not/a/real/slipbox-binary"), &config);

    match result {
        Ok(_) => panic!("missing program should fail"),
        Err(DaemonClientError::StartDaemon { program, .. }) => {
            assert!(program.ends_with("slipbox-binary"));
        }
        Err(other) => panic!("unexpected error: {other}"),
    }
}
