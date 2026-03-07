use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

pub(crate) fn normalized_title(title: &str) -> Result<&str> {
    let title = title.trim();
    if title.is_empty() {
        bail!("capture title must not be empty");
    }
    Ok(title)
}

pub(crate) fn normalize_relative_org_path(file_path: &str) -> Result<String> {
    let candidate = Path::new(file_path);
    if candidate.is_absolute() {
        bail!("file path must be relative to the slipbox root");
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("file path must stay within the slipbox root")
            }
        }
    }

    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if normalized.is_empty() {
        bail!("file path must not be empty");
    }
    if !normalized.ends_with(".org") {
        bail!("file path must end with .org");
    }

    Ok(normalized)
}

pub(crate) fn next_available_path(root: &Path, slug: &str) -> String {
    for suffix in 0.. {
        let filename = if suffix == 0 {
            format!("{slug}.org")
        } else {
            format!("{slug}-{suffix}.org")
        };
        if !root.join(&filename).exists() {
            return filename;
        }
    }

    unreachable!("unbounded path generation must eventually find an unused file name")
}

pub(crate) fn next_available_relative_path(root: &Path, file_path: &str) -> Result<String> {
    let normalized = normalize_relative_org_path(file_path)?;
    let candidate = Path::new(&normalized);
    let stem = candidate
        .file_stem()
        .and_then(|stem| stem.to_str())
        .context("file path must include a valid file name")?;
    let extension = candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .context("file path must include a valid extension")?;
    let parent = candidate
        .parent()
        .filter(|path| !path.as_os_str().is_empty());

    for suffix in 0.. {
        let filename = if suffix == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{suffix}.{extension}")
        };
        let relative = parent
            .map(|path| path.join(&filename))
            .unwrap_or_else(|| PathBuf::from(&filename));
        let absolute = root.join(&relative);
        if !absolute.exists() {
            return Ok(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    unreachable!("unbounded path generation must eventually find an unused file name")
}

pub(crate) fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for character in title.chars() {
        let normalized = character.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        String::from("note")
    } else {
        trimmed.to_owned()
    }
}

pub(crate) fn default_capture_file_title(relative_path: &str, title: &str) -> String {
    let title = title.trim();
    if !title.is_empty() {
        return title.to_owned();
    }

    Path::new(relative_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace('-', " "))
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or_else(|| String::from("Note"))
}

pub(crate) fn normalized_head_source(head: Option<&str>) -> String {
    match head {
        Some(head) if !head.trim().is_empty() => {
            let mut source = head.to_owned();
            if !source.ends_with('\n') {
                source.push('\n');
            }
            source
        }
        _ => String::new(),
    }
}
