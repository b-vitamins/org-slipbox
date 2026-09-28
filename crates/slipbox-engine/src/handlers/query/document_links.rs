use std::path::{Component, Path, PathBuf};

use slipbox_core::{AnchorRecord, DocumentLinkResolution, ResolveDocumentLinkParams};
use slipbox_rpc::JsonRpcError;
use url::Url;

use crate::rpc::{internal_error, parse_params, to_value};
use crate::state::ServerState;

pub(crate) fn resolve_document_link(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ResolveDocumentLinkParams = parse_params(params)?;
    let source = state.known_anchor(&params.source_node_key, "document link source")?;
    let resolution = resolve(state, &source, &params.target)?;
    to_value(resolution)
}

fn resolve(
    state: &ServerState,
    source: &AnchorRecord,
    target: &str,
) -> Result<DocumentLinkResolution, JsonRpcError> {
    if target.is_empty() || target.chars().any(char::is_control) {
        return Ok(DocumentLinkResolution::Unsupported);
    }

    if external_scheme(target) {
        return Ok(resolve_external(target));
    }

    if let Some(id) = target.strip_prefix("id:") {
        if id.is_empty() || id.trim() != id {
            return Ok(DocumentLinkResolution::Unsupported);
        }
        let node = state
            .database
            .node_from_id(id)
            .map_err(|error| internal_error(error.context("failed to resolve document ID")))?;
        return Ok(node.map_or(DocumentLinkResolution::Missing, |node| {
            DocumentLinkResolution::Note {
                node_key: node.node_key,
            }
        }));
    }

    if target.starts_with("file:") || target.starts_with("heading:") {
        let exact = state
            .database
            .anchor_by_key(target)
            .map_err(|error| internal_error(error.context("failed to resolve document key")))?;
        if let Some(anchor) = exact {
            return Ok(note(anchor));
        }
        if target.starts_with("heading:") {
            return Ok(DocumentLinkResolution::Missing);
        }
    }

    if target_scheme(target).is_some() && !target.starts_with("file:") {
        return Ok(DocumentLinkResolution::Unsupported);
    }

    if target.starts_with('*') || target.starts_with('#') {
        return resolve_selector(state, &source.file_path, target);
    }

    let relative = target.strip_prefix("file:").unwrap_or(target);
    let (path, selector) = relative
        .split_once("::")
        .map_or((relative, None), |(path, selector)| (path, Some(selector)));
    let Some(decoded_path) = decode_internal(path) else {
        return Ok(DocumentLinkResolution::Unsupported);
    };
    if !decoded_path.to_ascii_lowercase().ends_with(".org") {
        return Ok(DocumentLinkResolution::Unsupported);
    }
    let Some(file_path) = relative_file_path(&source.file_path, &decoded_path) else {
        return Ok(DocumentLinkResolution::Unsupported);
    };
    let file = state
        .database
        .anchor_by_key(&format!("file:{file_path}"))
        .map_err(|error| internal_error(error.context("failed to resolve document file")))?;
    let Some(file) = file else {
        return Ok(DocumentLinkResolution::Missing);
    };
    match selector {
        None | Some("") => Ok(note(file)),
        Some(selector) => resolve_selector(state, &file_path, selector),
    }
}

fn resolve_external(target: &str) -> DocumentLinkResolution {
    let authority = target.split_once(':').map(|(_, remainder)| remainder);
    if target.trim() != target
        || target.chars().any(char::is_whitespace)
        || target.contains('\\')
        || !authority.is_some_and(|remainder| remainder.starts_with("//"))
    {
        return DocumentLinkResolution::Unsupported;
    }
    let Ok(url) = Url::parse(target) else {
        return DocumentLinkResolution::Unsupported;
    };
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return DocumentLinkResolution::Unsupported;
    }
    DocumentLinkResolution::External { url: url.into() }
}

fn resolve_selector(
    state: &ServerState,
    file_path: &str,
    selector: &str,
) -> Result<DocumentLinkResolution, JsonRpcError> {
    let Some(decoded) = decode_internal(selector) else {
        return Ok(DocumentLinkResolution::Unsupported);
    };
    if let Some(id) = decoded.strip_prefix('#') {
        if id.is_empty() {
            return Ok(DocumentLinkResolution::Unsupported);
        }
        let node = state.database.node_from_id(id).map_err(|error| {
            internal_error(error.context("failed to resolve document selector"))
        })?;
        return Ok(node.filter(|node| node.file_path == file_path).map_or(
            DocumentLinkResolution::Missing,
            |node| DocumentLinkResolution::Note {
                node_key: node.node_key,
            },
        ));
    }
    let heading = decoded.strip_prefix('*').unwrap_or(&decoded);
    if heading.is_empty() {
        return Ok(DocumentLinkResolution::Unsupported);
    }
    let anchors = state
        .database
        .anchors_in_file(file_path)
        .map_err(|error| internal_error(error.context("failed to resolve document heading")))?;
    Ok(anchors
        .into_iter()
        .find(|anchor| anchor.title == heading)
        .map_or(DocumentLinkResolution::Missing, note))
}

fn note(anchor: AnchorRecord) -> DocumentLinkResolution {
    DocumentLinkResolution::Note {
        node_key: anchor.node_key,
    }
}

fn external_scheme(target: &str) -> bool {
    target_scheme(target).is_some_and(|scheme| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

fn target_scheme(target: &str) -> Option<&str> {
    let (scheme, remainder) = target.split_once(':')?;
    if remainder.starts_with(':') {
        return None;
    }
    let mut characters = scheme.chars();
    characters.next()?.is_ascii_alphabetic().then_some(())?;
    characters
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.'))
        .then_some(scheme)
}

fn decode_internal(value: &str) -> Option<String> {
    if value.contains('\\') || !safe_escapes(value) {
        return None;
    }
    let decoded = urlencoding::decode(value).ok()?.into_owned();
    (!decoded.contains('\\') && !decoded.chars().any(char::is_control)).then_some(decoded)
}

fn safe_escapes(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        if index + 2 >= bytes.len() {
            return false;
        }
        let Some(high) = hexadecimal(bytes[index + 1]) else {
            return false;
        };
        let Some(low) = hexadecimal(bytes[index + 2]) else {
            return false;
        };
        if matches!(high * 16 + low, b'/' | b'\\' | 0) {
            return false;
        }
        index += 3;
    }
    true
}

fn hexadecimal(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn relative_file_path(source_file: &str, target: &str) -> Option<String> {
    let target = Path::new(target);
    if target.is_absolute() {
        return None;
    }
    let mut resolved = PathBuf::from(Path::new(source_file).parent().unwrap_or(Path::new("")));
    for component in target.components() {
        match component {
            Component::Normal(value) => resolved.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let path = resolved.to_str()?;
    (!path.is_empty()).then(|| path.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_decoder_accepts_unicode_and_rejects_ambiguous_bytes() {
        assert_eq!(
            decode_internal("caf%C3%A9.org"),
            Some("café.org".to_owned())
        );
        assert_eq!(decode_internal("notes%2Fother.org"), None);
        assert_eq!(
            decode_internal("%2e%2e/other.org"),
            Some("../other.org".to_owned())
        );
        assert_eq!(decode_internal("bad%2"), None);
    }

    #[test]
    fn relative_paths_never_traverse_or_become_absolute() {
        assert_eq!(
            relative_file_path("notes/current.org", "next.org"),
            Some("notes/next.org".to_owned()),
        );
        assert_eq!(relative_file_path("notes/current.org", "../next.org"), None);
        assert_eq!(relative_file_path("notes/current.org", "/next.org"), None);
    }

    #[test]
    fn external_pages_carry_no_credentials_or_application_scheme() {
        assert!(matches!(
            resolve_external("https://example.org/a?q=1#part"),
            DocumentLinkResolution::External { .. }
        ));
        assert_eq!(
            resolve_external("https://user:secret@example.org/a"),
            DocumentLinkResolution::Unsupported,
        );
        assert_eq!(
            resolve_external("javascript:alert(1)"),
            DocumentLinkResolution::Unsupported,
        );
        assert_eq!(
            resolve_external("https:example.org/a"),
            DocumentLinkResolution::Unsupported,
        );
        assert_eq!(
            resolve_external("https:\\example.org/a"),
            DocumentLinkResolution::Unsupported,
        );
    }
}
