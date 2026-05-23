use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use slipbox_daemon_client::DaemonClientError;

use super::error::invalid_request_error;

pub(crate) fn normalize_daily_file_path(file_path: &str) -> Result<String, DaemonClientError> {
    let candidate = Path::new(file_path);
    if candidate.is_absolute() {
        return Err(invalid_request_error(
            "daily file path must be relative to --root",
        ));
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(invalid_request_error(
                    "daily file path must stay within --root",
                ));
            }
        }
    }

    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if normalized.is_empty() {
        return Err(invalid_request_error("daily file path must not be empty"));
    }
    if !normalized.ends_with(".org") {
        return Err(invalid_request_error("daily file path must end with .org"));
    }
    Ok(normalized)
}

pub(crate) fn normalize_edit_file_path(
    root: &Path,
    file_path: &Path,
) -> Result<String, DaemonClientError> {
    let normalized = normalize_root_relative_path(root, file_path, "edit file path")?;
    if !normalized.ends_with(".org") {
        return Err(invalid_request_error("edit file path must end with .org"));
    }
    Ok(normalized)
}

pub(crate) fn normalize_diagnostic_file_path(
    root: &Path,
    file_path: &Path,
) -> Result<String, DaemonClientError> {
    normalize_root_relative_path(root, file_path, "diagnostic file path")
}

pub(crate) fn normalize_root_relative_path(
    root: &Path,
    file_path: &Path,
    description: &str,
) -> Result<String, DaemonClientError> {
    slipbox::root_path::resolve_root_path(root, file_path)
        .map(|resolved| resolved.relative_path)
        .map_err(|error| {
            let message = error.to_string();
            if message.contains("must stay within the slipbox root")
                || message.contains(" is not under ")
                || message.contains("must not contain parent-directory components")
            {
                invalid_request_error(format!("{description} must stay within --root"))
            } else {
                invalid_request_error(format!("{description} is invalid: {message}"))
            }
        })
}

pub(crate) fn validate_region_range(start: u32, end: u32) -> Result<(), DaemonClientError> {
    if start == 0 || end == 0 {
        return Err(invalid_request_error(
            "edit region positions must be positive 1-based character positions",
        ));
    }
    if start == end {
        return Err(invalid_request_error(
            "active region range must not be empty",
        ));
    }
    Ok(())
}
