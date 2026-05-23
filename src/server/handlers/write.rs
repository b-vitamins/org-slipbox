mod capture;
mod link_rewrite;
mod structural;

pub(crate) use capture::{
    append_heading, append_heading_at_outline_path, append_heading_to_node, capture_node,
    capture_template, capture_template_preview, ensure_file_node, ensure_node_id,
    update_node_metadata,
};
pub(crate) use link_rewrite::{slipbox_link_rewrite_apply, slipbox_link_rewrite_preview};
pub(crate) use structural::{
    demote_entire_file, extract_subtree, promote_entire_file, refile_region, refile_subtree,
};
