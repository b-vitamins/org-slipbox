use clap::Args;
use slipbox_core::{
    NodeFromIdParams, NodeFromKeyParams, NodeFromRefParams, NodeFromTitleOrAliasParams, NodeRecord,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

use super::error::require_resolved_node;

#[derive(Debug, Clone, Args)]
pub(crate) struct ResolveTargetArgs {
    /// Resolve by exact explicit Org ID.
    #[arg(long, group = "target", value_name = "ID")]
    pub(crate) id: Option<String>,
    /// Resolve by exact title or alias.
    #[arg(long, group = "target", value_name = "TITLE")]
    pub(crate) title: Option<String>,
    /// Resolve by exact reference, for example `cite:smith2026`.
    #[arg(long = "ref", group = "target", value_name = "REF")]
    pub(crate) reference: Option<String>,
    /// Resolve by exact node key, for example `file:note.org`.
    #[arg(long, group = "target", value_name = "KEY")]
    pub(crate) key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolveTarget {
    Id(String),
    Title(String),
    Reference(String),
    Key(String),
}

impl ResolveTargetArgs {
    #[must_use]
    pub(crate) fn target(&self) -> ResolveTarget {
        if let Some(id) = &self.id {
            ResolveTarget::Id(id.clone())
        } else if let Some(title) = &self.title {
            ResolveTarget::Title(title.clone())
        } else if let Some(reference) = &self.reference {
            ResolveTarget::Reference(reference.clone())
        } else if let Some(node_key) = &self.key {
            ResolveTarget::Key(node_key.clone())
        } else {
            unreachable!("clap enforces exactly one target selector");
        }
    }
}

pub(crate) fn resolve_note_target(
    client: &mut DaemonClient,
    target: &ResolveTarget,
) -> Result<NodeRecord, DaemonClientError> {
    match target {
        ResolveTarget::Id(id) => require_resolved_node(
            client.node_from_id(&NodeFromIdParams { id: id.clone() })?,
            format!("unknown node id: {id}"),
        ),
        ResolveTarget::Title(title_or_alias) => require_resolved_node(
            client.node_from_title_or_alias(&NodeFromTitleOrAliasParams {
                title_or_alias: title_or_alias.clone(),
                nocase: false,
            })?,
            format!("unknown node title or alias: {title_or_alias}"),
        ),
        ResolveTarget::Reference(reference) => require_resolved_node(
            client.node_from_ref(&NodeFromRefParams {
                reference: reference.clone(),
            })?,
            format!("unknown node ref: {reference}"),
        ),
        ResolveTarget::Key(node_key) => require_resolved_node(
            client.node_from_key(&NodeFromKeyParams {
                node_key: node_key.clone(),
            })?,
            format!("unknown node key: {node_key}"),
        ),
    }
}

pub(crate) fn resolve_anchor_or_note_target_key(
    client: &mut DaemonClient,
    target: &ResolveTarget,
) -> Result<String, DaemonClientError> {
    match target {
        ResolveTarget::Key(node_key) => Ok(node_key.clone()),
        _ => resolve_note_target(client, target).map(|node| node.node_key),
    }
}
