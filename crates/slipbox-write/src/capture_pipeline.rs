use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use slipbox_core::{CaptureContentType, CaptureTemplateParams, NodeKind, NodeRecord};
use uuid::Uuid;

use crate::capture::{capture_entry, capture_list_item, capture_plain, capture_table_line};
use crate::document::OrgDocument;
use crate::path::{
    default_capture_file_title, next_available_path, normalize_relative_org_path,
    normalized_head_source, slugify,
};
use crate::{CaptureOutcome, CapturePreviewOutcome};

pub(crate) enum CaptureTargetSelection {
    File {
        relative_path: String,
        node_key: String,
    },
    Heading {
        relative_path: String,
        line_number: usize,
        level: usize,
        node_key: String,
    },
}

struct PreparedCapture {
    absolute_path: PathBuf,
    relative_path: String,
    document: OrgDocument,
    node_key: String,
}

pub fn capture_template(
    root: &Path,
    target_node: Option<&NodeRecord>,
    params: &CaptureTemplateParams,
) -> Result<CaptureOutcome> {
    let prepared = prepare_capture_template(root, target_node, params, None, false)?;

    if let Some(parent) = prepared.absolute_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    fs::write(&prepared.absolute_path, prepared.document.render())
        .with_context(|| format!("failed to write {}", prepared.absolute_path.display()))?;

    Ok(CaptureOutcome {
        absolute_path: prepared.absolute_path,
        node_key: prepared.node_key,
    })
}

pub fn preview_capture_template(
    root: &Path,
    target_node: Option<&NodeRecord>,
    params: &CaptureTemplateParams,
    source_override: Option<&str>,
    ensure_node_id: bool,
) -> Result<CapturePreviewOutcome> {
    let prepared =
        prepare_capture_template(root, target_node, params, source_override, ensure_node_id)?;

    Ok(CapturePreviewOutcome {
        absolute_path: prepared.absolute_path,
        relative_path: prepared.relative_path,
        node_key: prepared.node_key,
        content: prepared.document.render(),
    })
}

fn prepare_capture_template(
    root: &Path,
    target_node: Option<&NodeRecord>,
    params: &CaptureTemplateParams,
    source_override: Option<&str>,
    ensure_node_id: bool,
) -> Result<PreparedCapture> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let refs = params.normalized_refs();
    let relative_path = resolve_template_relative_path(root, target_node, params)?;
    let absolute_path = root.join(&relative_path);
    let existed_on_disk = absolute_path.exists();
    let source = match source_override {
        Some(source) => source.to_owned(),
        None if existed_on_disk => fs::read_to_string(&absolute_path)
            .with_context(|| format!("failed to read {}", absolute_path.display()))?,
        None => normalized_head_source(params.head.as_deref()),
    };
    let mut document = OrgDocument::from_source(&source);

    if !existed_on_disk && source_override.is_none() {
        if params.head.is_none() {
            document.set_file_keyword(
                "title",
                Some(default_capture_file_title(&relative_path, &params.title)),
            );
        }
        document.ensure_file_identity_with_refs(&refs)?;
    }

    let target = resolve_capture_target(&mut document, &relative_path, target_node, params)?;
    let node_key = match params.capture_type {
        CaptureContentType::Entry => capture_entry(
            &mut document,
            &target,
            &params.content,
            &params.title,
            params.prepend,
            params.normalized_empty_lines_before(),
            params.normalized_empty_lines_after(),
        )?,
        CaptureContentType::Plain => capture_plain(
            &mut document,
            &target,
            &params.content,
            params.prepend,
            params.normalized_empty_lines_before(),
            params.normalized_empty_lines_after(),
        )?,
        CaptureContentType::Item | CaptureContentType::Checkitem => capture_list_item(
            &mut document,
            &target,
            &params.content,
            params.capture_type,
            params.prepend,
            params.normalized_empty_lines_before(),
            params.normalized_empty_lines_after(),
        )?,
        CaptureContentType::TableLine => capture_table_line(
            &mut document,
            &target,
            &params.content,
            params.prepend,
            params.normalized_table_line_pos().as_deref(),
        )?,
    };

    if ensure_node_id {
        ensure_capture_node_identity(&mut document, &node_key)?;
    }

    Ok(PreparedCapture {
        absolute_path,
        relative_path,
        document,
        node_key,
    })
}

fn resolve_template_relative_path(
    root: &Path,
    target_node: Option<&NodeRecord>,
    params: &CaptureTemplateParams,
) -> Result<String> {
    if let Some(target) = target_node {
        return Ok(target.file_path.clone());
    }

    if let Some(file_path) = params.file_path.as_deref() {
        return normalize_relative_org_path(file_path);
    }

    let title = params.title.trim();
    let slug = if title.is_empty() {
        "note".to_owned()
    } else {
        slugify(title)
    };
    Ok(next_available_path(root, &slug))
}

fn resolve_capture_target(
    document: &mut OrgDocument,
    relative_path: &str,
    target_node: Option<&NodeRecord>,
    params: &CaptureTemplateParams,
) -> Result<CaptureTargetSelection> {
    if let Some(target) = target_node {
        return Ok(match target.kind {
            NodeKind::File => CaptureTargetSelection::File {
                relative_path: relative_path.to_owned(),
                node_key: target.node_key.clone(),
            },
            NodeKind::Heading => CaptureTargetSelection::Heading {
                relative_path: relative_path.to_owned(),
                line_number: target.line as usize,
                level: target.level as usize,
                node_key: target.node_key.clone(),
            },
        });
    }

    let outline_path = params.normalized_outline_path();
    if let Some((line_number, level)) = document.ensure_outline_path(&outline_path)? {
        Ok(CaptureTargetSelection::Heading {
            relative_path: relative_path.to_owned(),
            line_number,
            level,
            node_key: format!("heading:{}:{line_number}", relative_path.replace('\\', "/")),
        })
    } else {
        Ok(CaptureTargetSelection::File {
            relative_path: relative_path.to_owned(),
            node_key: format!("file:{}", relative_path.replace('\\', "/")),
        })
    }
}

pub(crate) fn capture_target_node_key(target: &CaptureTargetSelection) -> String {
    match target {
        CaptureTargetSelection::File { node_key, .. }
        | CaptureTargetSelection::Heading { node_key, .. } => node_key.clone(),
    }
}

fn ensure_capture_node_identity(document: &mut OrgDocument, node_key: &str) -> Result<()> {
    if node_key.starts_with("file:") {
        document.ensure_file_identity()?;
        return Ok(());
    }

    let Some(line_number) = capture_heading_line_number(node_key) else {
        bail!("unsupported capture node key: {node_key}");
    };
    let explicit_id = document
        .heading_property_value(line_number, "ID")?
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    document.set_heading_property(line_number, "ID", Some(explicit_id))?;
    Ok(())
}

fn capture_heading_line_number(node_key: &str) -> Option<usize> {
    let (_, line_number) = node_key.strip_prefix("heading:")?.rsplit_once(':')?;
    line_number.parse::<usize>().ok()
}
