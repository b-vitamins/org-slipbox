mod capture;
mod document;
mod metadata;
mod path;
mod rewrite;

use std::path::PathBuf;

pub use capture::{
    append_heading, append_heading_at_outline_path, append_heading_to_node, capture_file_note,
    capture_file_note_at, capture_file_note_at_with_head_and_refs, capture_file_note_at_with_refs,
    capture_file_note_with_refs, capture_template, ensure_file_note,
};
pub use metadata::{ensure_node_id, update_node_metadata};
pub use rewrite::{demote_entire_file, extract_subtree, promote_entire_file, refile_subtree};

pub struct CaptureOutcome {
    pub absolute_path: PathBuf,
    pub node_key: String,
}

pub struct MetadataUpdate {
    pub aliases: Option<Vec<String>>,
    pub refs: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

pub struct RewriteOutcome {
    pub changed_paths: Vec<PathBuf>,
    pub removed_paths: Vec<PathBuf>,
    pub explicit_id: String,
}
