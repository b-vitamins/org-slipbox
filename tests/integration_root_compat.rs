//! Public root-module compatibility and desktop authority.

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json::json;
use slipbox::server::operations;
use slipbox::service::{
    FreshnessBehavior, OperationFamily, OperationMutation, SlipboxService,
    operation_descriptor_by_method, operation_descriptors,
};
use slipbox_core::{
    IndexStats, ReadFileSourceParams, ReadFileSourceResult, SearchNodesParams, SearchNodesResult,
};
use slipbox_engine::service::SlipboxService as EngineService;
use slipbox_index::{DiscoveryPolicy, ExternalProgram, PlatformPolicy};
use slipbox_rpc::{METHOD_INDEX, METHOD_INDEX_FILE, METHOD_READ_FILE_SOURCE, METHOD_SEARCH_NODES};
use tempfile::tempdir;

const ALPHA: &str =
    "#+title: Alpha\n\n* First heading\n:PROPERTIES:\n:ID: alpha-first\n:END:\nPlain body.\n";

#[test]
fn the_root_root_path_module_still_resolves_paths() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(root.join("alpha.org"), ALPHA)?;

    let resolved = slipbox::root_path::resolve_root_path(&root, Path::new("alpha.org"))?;
    assert_eq!(resolved.relative_path, "alpha.org");
    assert_eq!(
        resolved.absolute_path,
        root.canonicalize()?.join("alpha.org")
    );

    let canonical_root = root.canonicalize()?;
    let from_canonical = slipbox::root_path::resolve_root_path_from_canonical_root(
        &canonical_root,
        &resolved.absolute_path,
    )?;
    assert_eq!(from_canonical, resolved);

    Ok(())
}

#[test]
fn the_root_operation_inventory_is_still_one_inventory() {
    let through_server: Vec<&str> = operations::operation_descriptors()
        .map(|descriptor| descriptor.method)
        .collect();
    let through_service: Vec<&str> = operation_descriptors()
        .map(|descriptor| descriptor.method)
        .collect();

    assert!(!through_server.is_empty());
    assert_eq!(
        through_server, through_service,
        "slipbox::server::operations and slipbox::service disagree on the inventory"
    );

    let descriptor =
        operation_descriptor_by_method(METHOD_INDEX_FILE).expect("indexFile is classified");
    assert_eq!(descriptor.family, OperationFamily::Files);
    assert_eq!(descriptor.mutation, OperationMutation::DerivedIndex);
    assert_eq!(descriptor.freshness, FreshnessBehavior::RefreshesOneFile);
}

#[test]
fn the_root_service_path_still_indexes_and_searches() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(root.join("alpha.org"), ALPHA)?;

    let mut service = SlipboxService::new(
        root,
        workspace.path().join("slipbox.sqlite"),
        Vec::new(),
        DiscoveryPolicy::default(),
    )?;

    let stats: IndexStats = service.invoke(METHOD_INDEX, json!({}))?;
    assert_eq!(stats.files_indexed, 1);

    let found: SearchNodesResult = service.invoke(
        METHOD_SEARCH_NODES,
        SearchNodesParams {
            query: "first".to_owned(),
            limit: 10,
            sort: None,
        },
    )?;
    assert_eq!(
        found
            .nodes
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        vec!["First heading"]
    );

    Ok(())
}

#[test]
fn the_desktop_context_still_authorizes_the_decryptors() -> Result<()> {
    let policy = PlatformPolicy::desktop();
    assert_eq!(
        policy.authorize_source(Path::new("/notes/secret.org.gpg"))?,
        Some(ExternalProgram::Gpg)
    );
    assert_eq!(
        policy.authorize_source(Path::new("/notes/secret.org.age"))?,
        Some(ExternalProgram::Age)
    );
    assert_eq!(
        policy.authorize_source(Path::new("/notes/plain.org"))?,
        None
    );

    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(root.join("alpha.org"), ALPHA)?;

    let mut service = SlipboxService::with_platform(
        root,
        workspace.path().join("slipbox.sqlite"),
        Vec::new(),
        DiscoveryPolicy::default(),
        policy,
    )?;
    let stats: IndexStats = service.invoke(METHOD_INDEX, json!({}))?;
    assert_eq!(stats.files_indexed, 1);

    Ok(())
}

#[test]
fn the_root_constructor_carries_desktop_authority_where_the_engine_refuses() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(root.join("alpha.org"), ALPHA)?;
    fs::write(root.join("secret.org.gpg"), "envelope fixture")?;
    let envelope = serde_json::to_value(ReadFileSourceParams {
        file_path: "secret.org.gpg".to_owned(),
        start_line: None,
        max_lines: None,
    })?;

    let mut desktop = SlipboxService::new(
        root.clone(),
        workspace.path().join("desktop.sqlite"),
        Vec::new(),
        DiscoveryPolicy::default(),
    )?;
    // This RPC checks authority but reads raw bytes, so no decryptor is executed.
    let admitted: ReadFileSourceResult =
        desktop.invoke(METHOD_READ_FILE_SOURCE, envelope.clone())?;
    assert_eq!(admitted.source.content, "envelope fixture");
    assert_eq!(admitted.source.file_path, "secret.org.gpg");

    let mut headless = EngineService::new(
        root,
        workspace.path().join("headless.sqlite"),
        Vec::new(),
        DiscoveryPolicy::default(),
    )?;
    let refused = headless
        .invoke_value(METHOD_READ_FILE_SOURCE, envelope)
        .expect_err("the engine default authorizes no decryptor")
        .into_inner();
    assert_eq!(refused.code, -32600);
    assert!(
        refused.message.contains("unsupported external program gpg")
            && refused.message.contains("secret.org.gpg"),
        "the refusal names the capability and the path: {}",
        refused.message
    );

    Ok(())
}

#[test]
fn both_constructors_index_a_plain_corpus_the_same_way() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(root.join("alpha.org"), ALPHA)?;

    let mut desktop = SlipboxService::new(
        root.clone(),
        workspace.path().join("desktop.sqlite"),
        Vec::new(),
        DiscoveryPolicy::default(),
    )?;
    let mut headless = EngineService::new(
        root,
        workspace.path().join("headless.sqlite"),
        Vec::new(),
        DiscoveryPolicy::default(),
    )?;

    let through_root: IndexStats = desktop.invoke(METHOD_INDEX, json!({}))?;
    let through_engine: IndexStats = headless.invoke(METHOD_INDEX, json!({}))?;
    assert_eq!(through_root.files_indexed, 1);
    assert_eq!(through_root.nodes_indexed, through_engine.nodes_indexed);
    assert_eq!(through_root.links_indexed, through_engine.links_indexed);

    Ok(())
}
