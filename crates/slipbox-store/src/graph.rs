use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Context, Result, bail};
use rusqlite::params;
use slipbox_core::{GraphParams, GraphTitleShortening, NodeRecord};

use crate::Database;
use crate::nodes::{node_select_columns, row_to_node};

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
              ORDER BY n.file_path, n.line",
            node_select_columns("n")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map([], row_to_node)?;
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
            Ok(GraphEdge {
                source_node_key: row.get(0)?,
                destination_node_key: row.get(1)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read graph edges")
    }
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
    let max_title_length = params.normalized_max_title_length();
    let title = shorten_title(&node.title, params.shorten_titles, max_title_length);
    let tooltip = if node.kind.as_str() == "file" {
        node.file_path.clone()
    } else {
        format!("{}:{} {}", node.file_path, node.line, node.title)
    };
    let mut attributes = vec![
        format!("label=\"{}\"", dot_escape(&title)),
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

fn graph_node_url(node: &NodeRecord, params: &GraphParams) -> Option<String> {
    let prefix = params
        .node_url_prefix
        .as_deref()
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())?;
    let explicit_id = node
        .explicit_id
        .as_deref()
        .map(str::trim)
        .filter(|explicit_id| !explicit_id.is_empty())?;
    Some(format!(
        "{prefix}{}",
        percent_encode_query_value(explicit_id)
    ))
}

fn shorten_title(
    title: &str,
    mode: Option<GraphTitleShortening>,
    max_title_length: usize,
) -> String {
    match mode {
        Some(GraphTitleShortening::Truncate) => truncate_title(title, max_title_length),
        Some(GraphTitleShortening::Wrap) => wrap_title(title, max_title_length),
        None => title.to_owned(),
    }
}

fn truncate_title(title: &str, max_title_length: usize) -> String {
    let count = title.chars().count();
    if count <= max_title_length {
        return title.to_owned();
    }

    let shortened = title
        .chars()
        .take(max_title_length.saturating_sub(3))
        .collect::<String>();
    format!("{shortened}...")
}

fn wrap_title(title: &str, max_title_length: usize) -> String {
    if title.chars().count() <= max_title_length {
        return title.to_owned();
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for word in title.split_whitespace() {
        let word_length = word.chars().count();
        let current_length = current.chars().count();
        let separator = usize::from(!current.is_empty());

        if current_length + separator + word_length <= max_title_length {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            continue;
        }

        if !current.is_empty() {
            lines.push(current);
            current = String::new();
        }

        if word_length <= max_title_length {
            current.push_str(word);
            continue;
        }

        let mut chunk = String::new();
        for character in word.chars() {
            chunk.push(character);
            if chunk.chars().count() >= max_title_length {
                lines.push(chunk);
                chunk = String::new();
            }
        }
        if !chunk.is_empty() {
            current = chunk;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines.join("\n")
}

fn dot_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => {}
            _ => escaped.push(character),
        }
    }
    escaped
}

fn percent_encode_query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
