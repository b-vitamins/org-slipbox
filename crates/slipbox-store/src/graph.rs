use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Context, Result, bail};
use rusqlite::params;
use slipbox_core::{GraphParams, GraphTitleShortening, NodeRecord};

use crate::Database;
use crate::nodes::{anchor_select_columns, note_where, row_to_note};
#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphEdge {
    source_node_key: String,
    destination_node_key: String,
}

impl Database {
    pub fn graph_dot(&self, params: &GraphParams) -> Result<String> {
        let hidden_link_types = params.normalized_hidden_link_types();
        if let Some(unsupported) = hidden_link_types
            .iter()
            .find(|link_type| link_type.as_str() != "id")
        {
            bail!("unsupported graph link type filter: {unsupported}");
        }

        let nodes = self.graph_nodes()?;
        let root_node_key = params.root_node_key.as_deref();
        if let Some(root_node_key) = root_node_key {
            if !nodes.iter().any(|node| node.node_key == root_node_key) {
                bail!("unknown graph root node: {root_node_key}");
            }
        }

        let hide_id_links = hidden_link_types.iter().any(|link_type| link_type == "id");
        let edges = if hide_id_links {
            Vec::new()
        } else {
            self.graph_edges()?
        };

        let (selected_nodes, selected_edges) = select_graph_scope(
            &nodes,
            &edges,
            root_node_key,
            params.max_distance,
            params.include_orphans,
        );

        Ok(format_graph_dot(&selected_nodes, &selected_edges, params))
    }

    fn graph_nodes(&self) -> Result<Vec<NodeRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE {}
              ORDER BY n.file_path, n.line",
            anchor_select_columns("n"),
            note_where("n"),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map([], row_to_note)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read graph nodes")
    }

    fn graph_edges(&self) -> Result<Vec<GraphEdge>> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT l.source_node_key,
                             dest.node_key
               FROM links AS l
               JOIN nodes AS dest ON dest.explicit_id = l.destination_explicit_id
              ORDER BY l.source_node_key, dest.node_key",
        )?;
        let rows = statement.query_map(params![], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let raw_edges = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read graph edges")?;

        let notes_by_file = self
            .indexed_files()?
            .into_iter()
            .map(|file_path| {
                let owners = self.note_owners_by_anchor_key(&file_path)?;
                Ok((file_path, owners))
            })
            .collect::<Result<HashMap<_, _>>>()?;
        let notes_by_key = self
            .graph_nodes()?
            .into_iter()
            .map(|node| (node.node_key.clone(), node))
            .collect::<HashMap<_, _>>();

        let mut edges = Vec::new();
        let mut seen = HashSet::new();
        for (source_anchor_key, destination_key) in raw_edges {
            let file_path = anchor_file_path(&source_anchor_key);
            let Some(source_note) = notes_by_file
                .get(file_path)
                .and_then(|owners| owners.get(&source_anchor_key))
            else {
                continue;
            };
            let Some(destination_note) = notes_by_key.get(&destination_key) else {
                continue;
            };
            let edge = GraphEdge {
                source_node_key: source_note.node_key.clone(),
                destination_node_key: destination_note.node_key.clone(),
            };
            if seen.insert((
                edge.source_node_key.clone(),
                edge.destination_node_key.clone(),
            )) {
                edges.push(edge);
            }
        }
        edges.sort_by(|left, right| {
            left.source_node_key
                .cmp(&right.source_node_key)
                .then_with(|| left.destination_node_key.cmp(&right.destination_node_key))
        });
        Ok(edges)
    }
}

fn anchor_file_path(anchor_key: &str) -> &str {
    anchor_key
        .strip_prefix("file:")
        .or_else(|| {
            anchor_key
                .strip_prefix("heading:")
                .and_then(|rest| rest.rsplit_once(':').map(|(file_path, _)| file_path))
        })
        .unwrap_or(anchor_key)
}

fn select_graph_scope(
    nodes: &[NodeRecord],
    edges: &[GraphEdge],
    root_node_key: Option<&str>,
    max_distance: Option<u32>,
    include_orphans: bool,
) -> (Vec<NodeRecord>, Vec<GraphEdge>) {
    if let Some(root_node_key) = root_node_key {
        let visited = neighborhood_node_keys(edges, root_node_key, max_distance);
        let selected_nodes = nodes
            .iter()
            .filter(|node| visited.contains(node.node_key.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let selected_edges = edges
            .iter()
            .filter(|edge| {
                visited.contains(edge.source_node_key.as_str())
                    && visited.contains(edge.destination_node_key.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        return (selected_nodes, selected_edges);
    }

    if include_orphans {
        return (nodes.to_vec(), edges.to_vec());
    }

    let mut connected = HashSet::new();
    for edge in edges {
        connected.insert(edge.source_node_key.as_str());
        connected.insert(edge.destination_node_key.as_str());
    }
    let selected_nodes = nodes
        .iter()
        .filter(|node| connected.contains(node.node_key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    (selected_nodes, edges.to_vec())
}

fn neighborhood_node_keys(
    edges: &[GraphEdge],
    root_node_key: &str,
    max_distance: Option<u32>,
) -> HashSet<String> {
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        adjacency
            .entry(edge.source_node_key.as_str())
            .or_default()
            .push(edge.destination_node_key.as_str());
        adjacency
            .entry(edge.destination_node_key.as_str())
            .or_default()
            .push(edge.source_node_key.as_str());
    }

    let mut visited = HashSet::from([root_node_key.to_owned()]);
    let mut queue = VecDeque::from([(root_node_key.to_owned(), 0_u32)]);

    while let Some((node_key, distance)) = queue.pop_front() {
        if max_distance.is_some_and(|limit| distance >= limit) {
            continue;
        }

        if let Some(neighbors) = adjacency.get(node_key.as_str()) {
            for neighbor in neighbors {
                if visited.insert((*neighbor).to_owned()) {
                    queue.push_back(((*neighbor).to_owned(), distance + 1));
                }
            }
        }
    }

    visited
}

fn format_graph_dot(nodes: &[NodeRecord], edges: &[GraphEdge], params: &GraphParams) -> String {
    let mut dot = String::from("digraph \"org-slipbox\" {\n");
    dot.push_str("  graph [overlap=false];\n");
    dot.push_str("  node [shape=box, style=\"rounded\"];\n");

    for node in nodes {
        dot.push_str(&format_graph_node(node, params));
    }

    for edge in edges {
        dot.push_str(&format!(
            "  \"{}\" -> \"{}\";\n",
            dot_escape(&edge.source_node_key),
            dot_escape(&edge.destination_node_key)
        ));
    }

    dot.push_str("}\n");
    dot
}

fn format_graph_node(node: &NodeRecord, params: &GraphParams) -> String {
    let label = format_graph_label(node, params);
    let tooltip = graph_node_tooltip(node);
    let mut attributes = vec![
        format!("label=\"{}\"", dot_escape(&label)),
        format!("tooltip=\"{}\"", dot_escape(&tooltip)),
    ];
    if let Some(url) = graph_node_url(node, params) {
        attributes.push(format!("URL=\"{}\"", dot_escape(&url)));
    }
    format!(
        "  \"{}\" [{}];\n",
        dot_escape(&node.node_key),
        attributes.join(", ")
    )
}

fn format_graph_label(node: &NodeRecord, params: &GraphParams) -> String {
    let title = node.title.trim();
    if title.is_empty() {
        return node.node_key.clone();
    }

    let max_length = params.normalized_max_title_length();
    if title.chars().count() <= max_length {
        return title.to_owned();
    }

    match params.shorten_titles {
        Some(GraphTitleShortening::Wrap) => wrap_title(title, max_length),
        _ => truncate_title(title, max_length),
    }
}

fn graph_node_url(node: &NodeRecord, params: &GraphParams) -> Option<String> {
    let prefix = params
        .node_url_prefix
        .as_deref()
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())?;
    let target = node
        .explicit_id
        .as_deref()
        .map(str::trim)
        .filter(|explicit_id| !explicit_id.is_empty())?;
    Some(format!("{prefix}{}", urlencoding::encode(target)))
}

fn graph_node_tooltip(node: &NodeRecord) -> String {
    match node.kind.as_str() {
        "file" => node.file_path.clone(),
        _ => format!("{}:{} {}", node.file_path, node.line, node.title),
    }
}

fn truncate_title(title: &str, max_length: usize) -> String {
    let ellipsis = "...";
    let visible = max_length.saturating_sub(ellipsis.chars().count()).max(1);
    let mut truncated = title.chars().take(visible).collect::<String>();
    truncated.push_str(ellipsis);
    truncated
}

fn wrap_title(title: &str, max_length: usize) -> String {
    let mut wrapped = String::new();
    let mut line_length = 0_usize;
    for word in title.split_whitespace() {
        let word_length = word.chars().count();
        let separator = usize::from(line_length > 0);
        if line_length + separator + word_length > max_length && line_length > 0 {
            wrapped.push('\n');
            wrapped.push_str(word);
            line_length = word_length;
        } else {
            if line_length > 0 {
                wrapped.push(' ');
                line_length += 1;
            }
            wrapped.push_str(word);
            line_length += word_length;
        }
    }
    wrapped
}

fn dot_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}
