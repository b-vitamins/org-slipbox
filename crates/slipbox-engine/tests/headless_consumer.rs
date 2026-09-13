//! Headless service contracts exercised through the public API.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde_json::{Value, json};
use slipbox_core::{
    IndexFileParams, IndexFileResult, IndexStats, IndexedFilesResult, NodeFromIdParams, NodeRecord,
    ReadFileSourceParams, ReadNodeSourceParams, ReadNodeSourceResult, SearchNodesParams,
    SearchNodesResult,
};
use slipbox_engine::service::{
    FreshnessBehavior, OperationFamily, OperationMutation, SlipboxService,
    operation_descriptor_by_method,
};
use slipbox_engine::{DiscoveryPolicy, handle_request};
use slipbox_rpc::{
    JsonRpcErrorKind, JsonRpcRequest, METHOD_CAPTURE_NODE, METHOD_INDEX, METHOD_INDEX_FILE,
    METHOD_INDEXED_FILES, METHOD_NODE_FROM_ID, METHOD_PING, METHOD_READ_FILE_SOURCE,
    METHOD_READ_NODE_SOURCE, METHOD_SEARCH_NODES,
};
use tempfile::{TempDir, tempdir};

const ALPHA: &str = "#+title: Alpha\n\n* First heading\n:PROPERTIES:\n:ID: alpha-first\n:END:\nSee [[id:beta-target][Beta]].\n";
const BETA: &str =
    "#+title: Beta\n\n* Target heading\n:PROPERTIES:\n:ID: beta-target\n:END:\nTarget body.\n";

struct Workspace {
    _workspace: TempDir,
    root: PathBuf,
    service: SlipboxService,
}

impl Workspace {
    fn open(files: &[(&str, &str)]) -> Result<Self> {
        let workspace = tempdir()?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root)?;
        for (name, contents) in files {
            fs::write(root.join(name), contents)?;
        }
        let service = SlipboxService::new(
            root.clone(),
            workspace.path().join("slipbox.sqlite"),
            Vec::new(),
            DiscoveryPolicy::default(),
        )?;
        Ok(Self {
            _workspace: workspace,
            root,
            service,
        })
    }

    fn write(&self, name: &str, contents: &str) -> Result<()> {
        fs::write(self.root.join(name), contents)?;
        Ok(())
    }

    fn indexed_files(&mut self) -> Result<Vec<String>> {
        let indexed: IndexedFilesResult = self.service.invoke(METHOD_INDEXED_FILES, json!({}))?;
        let mut files = indexed.files;
        files.sort();
        Ok(files)
    }

    fn titles(&mut self, query: &str) -> Result<Vec<String>> {
        let found: SearchNodesResult = self.service.invoke(
            METHOD_SEARCH_NODES,
            SearchNodesParams {
                query: query.to_owned(),
                limit: 20,
                sort: None,
            },
        )?;
        Ok(found.nodes.into_iter().map(|node| node.title).collect())
    }
}

#[test]
fn a_headless_consumer_indexes_searches_and_reads_source() -> Result<()> {
    let mut workspace = Workspace::open(&[("alpha.org", ALPHA), ("beta.org", BETA)])?;

    let stats: IndexStats = workspace.service.invoke(METHOD_INDEX, json!({}))?;
    assert_eq!(stats.files_indexed, 2);
    assert_eq!(stats.nodes_indexed, 4);
    assert_eq!(stats.links_indexed, 1);

    assert_eq!(workspace.indexed_files()?, vec!["alpha.org", "beta.org"]);
    assert_eq!(workspace.titles("target")?, vec!["Target heading"]);

    let node: Option<NodeRecord> = workspace.service.invoke(
        METHOD_NODE_FROM_ID,
        NodeFromIdParams {
            id: "beta-target".to_owned(),
        },
    )?;
    let node = node.expect("beta-target resolves to an indexed node");
    assert_eq!(node.title, "Target heading");
    assert_eq!(node.file_path, "beta.org");

    let read: ReadNodeSourceResult = workspace.service.invoke(
        METHOD_READ_NODE_SOURCE,
        ReadNodeSourceParams {
            node_key: node.node_key.clone(),
            context_before: None,
            context_after: None,
            max_lines: None,
        },
    )?;
    assert_eq!(read.anchor.node_key, node.node_key);
    assert!(
        read.source.content.contains("Target body."),
        "source slice carries the node body: {}",
        read.source.content
    );

    Ok(())
}

#[test]
fn indexing_one_file_refreshes_it_without_pruning_the_root() -> Result<()> {
    let mut workspace = Workspace::open(&[("alpha.org", ALPHA), ("beta.org", BETA)])?;
    workspace
        .service
        .invoke::<_, IndexStats>(METHOD_INDEX, json!({}))?;

    workspace.write(
        "alpha.org",
        &format!("{ALPHA}\n* Second heading\n:PROPERTIES:\n:ID: alpha-second\n:END:\n"),
    )?;
    workspace.write(
        "gamma.org",
        "#+title: Gamma\n\n* Third heading\n:PROPERTIES:\n:ID: gamma-third\n:END:\n",
    )?;

    let updated: IndexFileResult = workspace.service.invoke(
        METHOD_INDEX_FILE,
        IndexFileParams {
            file_path: "alpha.org".to_owned(),
        },
    )?;
    assert_eq!(updated.file_path, "alpha.org");

    assert_eq!(workspace.titles("second")?, vec!["Second heading"]);
    assert_eq!(workspace.indexed_files()?, vec!["alpha.org", "beta.org"]);
    assert_eq!(workspace.titles("target")?, vec!["Target heading"]);

    fs::remove_file(workspace.root.join("beta.org"))?;
    let removed: IndexFileResult = workspace.service.invoke(
        METHOD_INDEX_FILE,
        IndexFileParams {
            file_path: "beta.org".to_owned(),
        },
    )?;
    assert_eq!(removed.file_path, "beta.org");
    assert_eq!(workspace.indexed_files()?, vec!["alpha.org"]);
    assert_eq!(workspace.titles("target")?, Vec::<String>::new());
    assert_eq!(workspace.titles("first")?, vec!["First heading"]);

    Ok(())
}

#[test]
fn an_unknown_method_is_refused_without_a_handler_lookup_fallback() -> Result<()> {
    let mut workspace = Workspace::open(&[("alpha.org", ALPHA)])?;

    let error = workspace
        .service
        .invoke_value("slipbox/methodThatDoesNotExist", Value::Null)
        .expect_err("an unclassified method has no handler")
        .into_inner();
    assert_eq!(error.code, -32601);
    assert_eq!(
        error.data.expect("refusal carries a kind").kind,
        JsonRpcErrorKind::MethodNotFound
    );
    assert!(
        error.message.contains("slipbox/methodThatDoesNotExist"),
        "refusal names the method: {}",
        error.message
    );

    Ok(())
}

#[test]
fn a_read_only_session_is_fail_closed_over_the_dispatch_entry_point() -> Result<()> {
    let mut workspace = Workspace::open(&[("alpha.org", ALPHA)])?;

    let admitted = handle_request(
        &mut workspace.service,
        JsonRpcRequest::new(json!(1), METHOD_PING, Value::Null),
        true,
    );
    assert!(
        admitted.error.is_none(),
        "a read-only session admits a read-only method: {admitted:?}"
    );
    assert_eq!(admitted.id, json!(1));

    for method in [METHOD_CAPTURE_NODE, "slipbox/methodThatDoesNotExist"] {
        let refused = handle_request(
            &mut workspace.service,
            JsonRpcRequest::new(json!(2), method, Value::Null),
            true,
        );
        let error = refused
            .error
            .expect("a read-only session refuses anything not classified read-only");
        assert_eq!(error.code, -32600);
        assert!(
            error.message.contains("read-only slipbox session"),
            "refusal states the session policy: {}",
            error.message
        );
    }

    Ok(())
}

#[test]
fn the_engine_serves_the_shared_operation_classification() {
    for (method, family, mutation, freshness) in [
        (
            METHOD_INDEX,
            OperationFamily::System,
            OperationMutation::DerivedIndex,
            FreshnessBehavior::RefreshesRootIndex,
        ),
        (
            METHOD_INDEX_FILE,
            OperationFamily::Files,
            OperationMutation::DerivedIndex,
            FreshnessBehavior::RefreshesOneFile,
        ),
        (
            METHOD_SEARCH_NODES,
            OperationFamily::Notes,
            OperationMutation::ReadOnly,
            FreshnessBehavior::ReadsDerivedIndex,
        ),
        (
            METHOD_READ_NODE_SOURCE,
            OperationFamily::Notes,
            OperationMutation::ReadOnly,
            FreshnessBehavior::ReadsSourceFiles,
        ),
        (
            METHOD_PING,
            OperationFamily::System,
            OperationMutation::ReadOnly,
            FreshnessBehavior::ReportsState,
        ),
    ] {
        let descriptor = operation_descriptor_by_method(method)
            .unwrap_or_else(|| panic!("{method} is classified"));
        assert_eq!(descriptor.family, family, "{method} family");
        assert_eq!(descriptor.mutation, mutation, "{method} mutation");
        assert_eq!(descriptor.freshness, freshness, "{method} freshness");
    }
}

#[test]
fn a_headless_service_refuses_a_source_that_needs_an_external_program() -> Result<()> {
    let mut workspace = Workspace::open(&[
        ("alpha.org", ALPHA),
        ("secret.org.gpg", "not decodable here"),
    ])?;

    // Discovery eligibility does not grant decryptor authority.
    assert!(
        DiscoveryPolicy::default()
            .matches_path(&workspace.root, &workspace.root.join("secret.org.gpg"))
    );

    let error = workspace
        .service
        .invoke_value(METHOD_INDEX, json!({}))
        .expect_err("a headless policy authorizes no decryptor")
        .into_inner();
    assert_eq!(error.code, -32600);
    assert!(
        error.message.contains("gpg") && error.message.contains("secret.org.gpg"),
        "the scan states the missing capability and the path: {}",
        error.message
    );

    let error = workspace
        .service
        .invoke_value(
            METHOD_READ_FILE_SOURCE,
            serde_json::to_value(ReadFileSourceParams {
                file_path: "secret.org.gpg".to_owned(),
                start_line: None,
                max_lines: None,
            })?,
        )
        .expect_err("reading the envelope needs the same unauthorized program")
        .into_inner();
    assert_eq!(error.code, -32600);
    assert!(
        error.message.contains("gpg") && error.message.contains("secret.org.gpg"),
        "refusal names the capability and the path: {}",
        error.message
    );

    let error = workspace
        .service
        .invoke_value(
            METHOD_INDEX_FILE,
            serde_json::to_value(IndexFileParams {
                file_path: "secret.org.gpg".to_owned(),
            })?,
        )
        .expect_err("indexing one envelope needs the same unauthorized program")
        .into_inner();
    assert_eq!(error.code, -32600);
    assert!(
        error.message.contains("gpg") && error.message.contains("secret.org.gpg"),
        "the file update states the missing capability and the path: {}",
        error.message
    );

    // An existing index may contain an envelope even when this caller is headless.
    let indexed = slipbox_index::scan_source("secret.org.gpg", "#+title: Secret\n");
    let envelope_key = indexed.nodes[0].node_key.clone();
    slipbox_store::Database::open(&workspace._workspace.path().join("slipbox.sqlite"))?
        .sync_file_index(&indexed)?;
    let error = workspace
        .service
        .invoke_value(
            METHOD_READ_NODE_SOURCE,
            serde_json::to_value(ReadNodeSourceParams {
                node_key: envelope_key,
                context_before: None,
                context_after: None,
                max_lines: None,
            })?,
        )
        .expect_err("an indexed envelope does not confer decryptor authority")
        .into_inner();
    assert_eq!(error.code, -32600);
    assert!(error.message.contains("gpg") && error.message.contains("secret.org.gpg"));

    fs::remove_file(workspace.root.join("secret.org.gpg"))?;
    let stats: IndexStats = workspace.service.invoke(METHOD_INDEX, json!({}))?;
    assert_eq!(stats.files_indexed, 1);
    assert_eq!(workspace.indexed_files()?, vec!["alpha.org"]);

    Ok(())
}
