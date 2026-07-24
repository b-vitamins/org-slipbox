use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use serde::Serialize;
use slipbox_core::{BacklinksParams, ForwardLinksParams, NodeFromKeyParams, NodeRecord};

use crate::ReadingBridge;
use crate::http::response::ApiError;

/// Largest hop radius a neighborhood request may ask for.
pub(crate) const MAX_HOPS: u32 = 3;
/// Default hop radius when the caller does not specify one.
pub(crate) const DEFAULT_HOPS: u32 = 1;
/// Largest per-node, per-direction fan-out the walk will follow.
pub(crate) const MAX_FANOUT: usize = 200;
/// Default per-node, per-direction fan-out.
pub(crate) const DEFAULT_FANOUT: usize = 32;
/// Hard ceiling on distinct nodes in one neighborhood. Reaching it stops the
/// walk and marks the result truncated.
const MAX_NODES: usize = 256;

/// A hop-bounded local neighborhood around one origin note.
#[derive(Debug, Serialize)]
pub(crate) struct Neighborhood {
    origin: String,
    hops: u32,
    fanout: usize,
    /// Every distinct note reached, including the origin at distance zero.
    nodes: Vec<NeighborhoodNode>,
    /// Directed links between reached notes, deduplicated.
    edges: Vec<NeighborhoodEdge>,
    /// Set by either bound: the node ceiling, or a note whose links filled
    /// `fanout`.
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct NeighborhoodNode {
    node: NodeRecord,
    distance: u32,
}

#[derive(Debug, Serialize)]
struct NeighborhoodEdge {
    source: String,
    target: String,
    kind: EdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
enum EdgeKind {
    Forward,
}

/// Walk the local neighborhood around `origin_key` breadth-first.
///
/// Each reached note contributes its forward links as `origin -> destination`
/// edges and its backlinks as `source -> origin` edges, both bounded by
/// `fanout`. The walk stops at `hops` or the node ceiling, whichever comes
/// first, and either bound sets `truncated`.
pub(crate) fn walk(
    bridge: &ReadingBridge,
    origin_key: &str,
    hops: u32,
    fanout: usize,
) -> Result<Neighborhood, ApiError> {
    let origin = bridge
        .node_from_key(&NodeFromKeyParams {
            node_key: origin_key.to_owned(),
        })?
        .ok_or_else(|| ApiError::not_found(format!("no note found for key `{origin_key}`")))?;

    let mut walk = Walk::from_origin(origin.clone());

    while let Some((key, depth)) = walk.frontier.pop_front() {
        if depth >= hops {
            continue;
        }
        let next_depth = depth + 1;

        let forward = bridge.forward_links(&ForwardLinksParams {
            node_key: key.clone(),
            limit: fanout,
            unique: true,
        })?;
        // A full page means the fan-out bound, not the note, decided where the
        // list ended.
        walk.truncated |= forward.forward_links.len() >= fanout;
        for record in forward.forward_links {
            let source = key.clone();
            if !walk.connect(
                source,
                record.destination_note,
                next_depth,
                Endpoint::Target,
            ) {
                break;
            }
        }

        let backward = bridge.backlinks(&BacklinksParams {
            node_key: key.clone(),
            limit: fanout,
            unique: true,
        })?;
        walk.truncated |= backward.backlinks.len() >= fanout;
        for record in backward.backlinks {
            let target = key.clone();
            if !walk.connect(target, record.source_note, next_depth, Endpoint::Source) {
                break;
            }
        }

        // The node ceiling ends the whole walk; a filled fan-out only marks the
        // result and lets the remaining frontier expand.
        if walk.nodes.len() >= MAX_NODES {
            walk.truncated = true;
            break;
        }
    }

    Ok(Neighborhood {
        origin: origin.node_key,
        hops,
        fanout,
        nodes: walk.nodes,
        edges: walk.edges,
        truncated: walk.truncated,
    })
}

/// Which end of an edge the newly reached note sits at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endpoint {
    Source,
    Target,
}

/// The neighborhood under construction. Nodes and edges are accumulated only
/// through [`Walk::connect`], which holds the invariant that every edge endpoint
/// is a note present in `nodes`.
struct Walk {
    /// Doubles as the visited set: a key is present once first reached, at its
    /// shortest distance, and is never revisited.
    distance: HashMap<String, u32>,
    nodes: Vec<NeighborhoodNode>,
    edges: Vec<NeighborhoodEdge>,
    seen_edges: HashSet<(String, String)>,
    frontier: VecDeque<(String, u32)>,
    truncated: bool,
}

impl Walk {
    /// A walk holding only its origin, at distance zero and queued to expand.
    fn from_origin(origin: NodeRecord) -> Self {
        let key = origin.node_key.clone();
        Self {
            distance: HashMap::from([(key.clone(), 0)]),
            nodes: vec![NeighborhoodNode {
                node: origin,
                distance: 0,
            }],
            edges: Vec::new(),
            seen_edges: HashSet::new(),
            frontier: VecDeque::from([(key, 0)]),
            truncated: false,
        }
    }

    /// Admit `reached` at `depth`, then record the edge between it and `anchor`.
    ///
    /// Returns whether the walk may continue down this note's link list. A note
    /// the ceiling refuses contributes no edge, since that edge would name a
    /// note absent from `nodes`.
    fn connect(&mut self, anchor: String, reached: NodeRecord, depth: u32, end: Endpoint) -> bool {
        let reached_key = reached.node_key.clone();
        if !self.admit(reached, depth) {
            self.truncated = true;
            return false;
        }
        let (source, target) = match end {
            Endpoint::Source => (reached_key, anchor),
            Endpoint::Target => (anchor, reached_key),
        };
        self.record_edge(source, target);
        true
    }

    /// Add a newly reached note at `depth` and queue it for expansion, returning
    /// `false` when the node ceiling is reached. A note already seen is left at
    /// its shorter distance and not requeued.
    fn admit(&mut self, node: NodeRecord, depth: u32) -> bool {
        if self.distance.contains_key(&node.node_key) {
            return true;
        }
        if self.nodes.len() >= MAX_NODES {
            return false;
        }
        self.distance.insert(node.node_key.clone(), depth);
        self.frontier.push_back((node.node_key.clone(), depth));
        self.nodes.push(NeighborhoodNode {
            node,
            distance: depth,
        });
        true
    }

    /// Record a directed edge between two admitted notes, if it is new.
    fn record_edge(&mut self, source: String, target: String) {
        // A self-link contributes no structure to a neighborhood view.
        if source == target {
            return;
        }
        if !self.seen_edges.insert((source.clone(), target.clone())) {
            return;
        }
        self.edges.push(NeighborhoodEdge {
            source,
            target,
            kind: EdgeKind::Forward,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{Endpoint, MAX_NODES, Walk};

    fn note(key: &str) -> slipbox_core::NodeRecord {
        serde_json::from_value(serde_json::json!({
            "node_key": key,
            "explicit_id": null,
            "file_path": "notes/x.org",
            "title": key,
            "outline_path": key,
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
            "level": 0,
            "line": 1,
            "kind": "file",
            "file_mtime_ns": 0,
            "backlink_count": 0,
            "forward_link_count": 0,
        }))
        .expect("the fixture matches NodeRecord")
    }

    fn every_edge_endpoint_is_present(walk: &Walk) -> bool {
        let keys: std::collections::HashSet<&str> = walk
            .nodes
            .iter()
            .map(|entry| entry.node.node_key.as_str())
            .collect();
        walk.edges
            .iter()
            .all(|edge| keys.contains(edge.source.as_str()) && keys.contains(edge.target.as_str()))
    }

    #[test]
    fn an_edge_is_recorded_once() {
        let mut walk = Walk::from_origin(note("a"));
        assert!(walk.connect("a".to_owned(), note("b"), 1, Endpoint::Target));
        assert!(walk.connect("a".to_owned(), note("b"), 1, Endpoint::Target));
        assert_eq!(walk.edges.len(), 1);
    }

    #[test]
    fn a_self_link_records_no_edge() {
        let mut walk = Walk::from_origin(note("a"));
        assert!(walk.connect("a".to_owned(), note("a"), 1, Endpoint::Target));
        assert!(walk.edges.is_empty());
    }

    #[test]
    fn the_reverse_direction_is_its_own_edge() {
        let mut walk = Walk::from_origin(note("a"));
        assert!(walk.connect("a".to_owned(), note("b"), 1, Endpoint::Target));
        assert!(walk.connect("a".to_owned(), note("b"), 1, Endpoint::Source));
        assert_eq!(walk.edges.len(), 2);
        assert!(every_edge_endpoint_is_present(&walk));
    }

    #[test]
    fn a_note_already_reached_keeps_its_shorter_distance() {
        let mut walk = Walk::from_origin(note("origin"));
        assert!(walk.admit(note("a"), 1));
        assert!(walk.admit(note("a"), 2));
        assert_eq!(walk.distance.get("a"), Some(&1));
        assert_eq!(walk.nodes.len(), 2);
        assert_eq!(walk.frontier.len(), 2);
    }

    #[test]
    fn the_node_ceiling_refuses_further_notes() {
        let mut walk = Walk::from_origin(note("origin"));
        for index in 0..MAX_NODES - 1 {
            assert!(walk.admit(note(&format!("n{index}")), 1));
        }
        assert!(!walk.admit(note("overflow"), 1));
        assert_eq!(walk.nodes.len(), MAX_NODES);
    }

    #[test]
    fn the_edge_that_reaches_the_ceiling_is_not_published() {
        let mut walk = Walk::from_origin(note("hub"));
        for index in 0..MAX_NODES - 1 {
            assert!(walk.connect(
                "hub".to_owned(),
                note(&format!("n{index}")),
                1,
                Endpoint::Source,
            ));
        }
        assert_eq!(walk.nodes.len(), MAX_NODES);

        assert!(!walk.connect("hub".to_owned(), note("overflow"), 1, Endpoint::Source));
        assert!(walk.truncated);
        assert_eq!(walk.nodes.len(), MAX_NODES);
        assert_eq!(walk.edges.len(), MAX_NODES - 1);
        assert!(every_edge_endpoint_is_present(&walk));
    }
}
