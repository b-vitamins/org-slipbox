use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use slipbox_core::{NodeKind, NodeRecord};
use uuid::Uuid;

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

pub fn capture_file_note(root: &Path, title: &str) -> Result<CaptureOutcome> {
    capture_file_note_with_refs(root, title, &[])
}

pub fn capture_file_note_with_refs(
    root: &Path,
    title: &str,
    refs: &[String],
) -> Result<CaptureOutcome> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let title = normalized_title(title)?;
    let slug = slugify(title);
    let relative_path = next_available_path(root, &slug);
    create_file_note(root, &relative_path, title, refs)
}

pub fn capture_file_note_at(root: &Path, file_path: &str, title: &str) -> Result<CaptureOutcome> {
    capture_file_note_at_with_refs(root, file_path, title, &[])
}

pub fn capture_file_note_at_with_refs(
    root: &Path,
    file_path: &str,
    title: &str,
    refs: &[String],
) -> Result<CaptureOutcome> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let title = normalized_title(title)?;
    let relative_path = next_available_relative_path(root, file_path)?;
    create_file_note(root, &relative_path, title, refs)
}

pub fn ensure_file_note(root: &Path, file_path: &str, title: &str) -> Result<CaptureOutcome> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let title = normalized_title(title)?;
    let relative_path = normalize_relative_org_path(file_path)?;
    let absolute_path = root.join(&relative_path);
    if !absolute_path.exists() {
        write_file_note(&absolute_path, title, &[])?;
    }

    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("file:{}", relative_path.replace('\\', "/")),
    })
}

pub fn append_heading(
    root: &Path,
    file_path: &str,
    title: &str,
    heading: &str,
    level: usize,
) -> Result<CaptureOutcome> {
    let heading = heading.trim();
    if heading.is_empty() {
        bail!("capture heading must not be empty");
    }

    let file_note = ensure_file_note(root, file_path, title)?;
    let source = fs::read_to_string(&file_note.absolute_path)
        .with_context(|| format!("failed to read {}", file_note.absolute_path.display()))?;
    let (updated, line_number) = append_heading_to_source(&source, heading, level.max(1));
    fs::write(&file_note.absolute_path, updated)
        .with_context(|| format!("failed to write {}", file_note.absolute_path.display()))?;

    Ok(CaptureOutcome {
        absolute_path: file_note.absolute_path,
        node_key: format!(
            "heading:{}:{line_number}",
            file_note.node_key.trim_start_matches("file:")
        ),
    })
}

pub fn append_heading_to_node(
    root: &Path,
    node: &NodeRecord,
    heading: &str,
) -> Result<CaptureOutcome> {
    let heading = heading.trim();
    if heading.is_empty() {
        bail!("capture heading must not be empty");
    }

    let absolute_path = root.join(&node.file_path);
    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let (updated, line_number) = match node.kind {
        NodeKind::File => append_heading_to_source(&source, heading, 1),
        NodeKind::Heading => {
            append_heading_under_node(&source, node.line as usize, node.level as usize, heading)?
        }
    };
    fs::write(&absolute_path, updated)
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;

    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("heading:{}:{line_number}", node.file_path),
    })
}

pub fn ensure_node_id(root: &Path, node: &NodeRecord) -> Result<PathBuf> {
    if node.explicit_id.is_some() {
        return Ok(root.join(&node.file_path));
    }

    let absolute_path = root.join(&node.file_path);
    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let explicit_id = Uuid::new_v4().to_string();
    let updated = match node.kind {
        NodeKind::File => insert_file_id(&source, &explicit_id),
        NodeKind::Heading => insert_heading_id(&source, node.line as usize, &explicit_id)?,
    };
    fs::write(&absolute_path, updated)
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;
    Ok(absolute_path)
}

pub fn update_node_metadata(
    root: &Path,
    node: &NodeRecord,
    update: &MetadataUpdate,
) -> Result<PathBuf> {
    let absolute_path = root.join(&node.file_path);
    let source = fs::read_to_string(&absolute_path)
        .with_context(|| format!("failed to read {}", absolute_path.display()))?;
    let mut document = OrgDocument::from_source(&source);

    match node.kind {
        NodeKind::File => {
            if let Some(aliases) = &update.aliases {
                document.set_file_property("ROAM_ALIASES", property_value(aliases));
            }
            if let Some(refs) = &update.refs {
                document.set_file_property("ROAM_REFS", property_value(refs));
            }
            if let Some(tags) = &update.tags {
                document.set_file_keyword("filetags", keyword_value(tags));
            }
        }
        NodeKind::Heading => {
            if let Some(aliases) = &update.aliases {
                document.set_heading_property(
                    node.line as usize,
                    "ROAM_ALIASES",
                    property_value(aliases),
                )?;
            }
            if let Some(refs) = &update.refs {
                document.set_heading_property(
                    node.line as usize,
                    "ROAM_REFS",
                    property_value(refs),
                )?;
            }
            if let Some(tags) = &update.tags {
                document.set_heading_tags(node.line as usize, tags)?;
            }
        }
    }

    fs::write(&absolute_path, document.render())
        .with_context(|| format!("failed to write {}", absolute_path.display()))?;
    Ok(absolute_path)
}

pub fn refile_subtree(
    root: &Path,
    source: &NodeRecord,
    target: &NodeRecord,
) -> Result<RewriteOutcome> {
    if source.node_key == target.node_key {
        bail!("target is the same as current node");
    }

    let source_path = root.join(&source.file_path);
    let target_path = root.join(&target.file_path);
    if source.kind == NodeKind::File && source_path == target_path {
        bail!("target is inside the current subtree");
    }

    let source_source = fs::read_to_string(&source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    let target_source = if source_path == target_path {
        None
    } else {
        Some(
            fs::read_to_string(&target_path)
                .with_context(|| format!("failed to read {}", target_path.display()))?,
        )
    };

    let mut source_document = OrgDocument::from_source(&source_source);
    let (mut subtree_lines, explicit_id) = source_document.subtree_lines(source)?;
    let target_root_level = match target.kind {
        NodeKind::File => 1,
        NodeKind::Heading => target.level as usize + 1,
    };
    shift_subtree_levels(&mut subtree_lines, target_root_level)?;

    if source_path == target_path {
        if source.kind != NodeKind::Heading {
            bail!("target is inside the current subtree");
        }

        let (source_start, source_end) = source_document.subtree_range(source.line as usize)?;
        if target.kind == NodeKind::Heading {
            let target_index = source_document.heading_index(target.line as usize)?;
            if (source_start..source_end).contains(&target_index) {
                bail!("target is inside the current subtree");
            }
        }

        let insert_index = source_document.insertion_index(target)?;
        source_document.remove_range(source_start, source_end);
        let adjusted_insert = if insert_index > source_start {
            insert_index - (source_end - source_start)
        } else {
            insert_index
        };
        source_document.insert_subtree(adjusted_insert, target.kind, subtree_lines);

        fs::write(&source_path, source_document.render())
            .with_context(|| format!("failed to write {}", source_path.display()))?;

        return Ok(RewriteOutcome {
            changed_paths: vec![source_path],
            removed_paths: Vec::new(),
            explicit_id,
        });
    }

    let mut target_document = OrgDocument::from_source(target_source.as_deref().unwrap_or(""));
    target_document.insert_subtree(
        target_document.insertion_index(target)?,
        target.kind,
        subtree_lines,
    );

    let mut changed_paths = vec![target_path.clone()];
    let mut removed_paths = Vec::new();

    if source.kind == NodeKind::File {
        fs::remove_file(&source_path)
            .with_context(|| format!("failed to remove {}", source_path.display()))?;
        removed_paths.push(source_path);
    } else {
        let (source_start, source_end) = source_document.subtree_range(source.line as usize)?;
        source_document.remove_range(source_start, source_end);
        if source_document.has_meaningful_content() {
            fs::write(&source_path, source_document.render())
                .with_context(|| format!("failed to write {}", source_path.display()))?;
            changed_paths.push(source_path);
        } else {
            fs::remove_file(&source_path)
                .with_context(|| format!("failed to remove {}", source_path.display()))?;
            removed_paths.push(source_path);
        }
    }

    fs::write(&target_path, target_document.render())
        .with_context(|| format!("failed to write {}", target_path.display()))?;

    Ok(RewriteOutcome {
        changed_paths,
        removed_paths,
        explicit_id,
    })
}

pub fn extract_subtree(
    root: &Path,
    source: &NodeRecord,
    file_path: &str,
) -> Result<RewriteOutcome> {
    if source.kind != NodeKind::Heading {
        bail!("only heading nodes can be extracted");
    }

    let relative_path = normalize_relative_org_path(file_path)?;
    let source_path = root.join(&source.file_path);
    let target_path = root.join(&relative_path);
    if source_path == target_path {
        bail!("target file must differ from the source file");
    }
    if target_path.exists() {
        bail!("{} exists. Aborting", target_path.display());
    }

    let source_source = fs::read_to_string(&source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    let mut source_document = OrgDocument::from_source(&source_source);
    let (subtree_lines, explicit_id) = source_document.subtree_lines(source)?;
    let target_document = OrgDocument::from_extracted_subtree(&subtree_lines, &explicit_id)?;
    let (source_start, source_end) = source_document.subtree_range(source.line as usize)?;
    source_document.remove_range(source_start, source_end);

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    fs::write(&source_path, source_document.render())
        .with_context(|| format!("failed to write {}", source_path.display()))?;
    fs::write(&target_path, target_document.render())
        .with_context(|| format!("failed to write {}", target_path.display()))?;

    Ok(RewriteOutcome {
        changed_paths: vec![source_path, target_path],
        removed_paths: Vec::new(),
        explicit_id,
    })
}

fn insert_file_id(source: &str, explicit_id: &str) -> String {
    let mut document = OrgDocument::from_source(source);
    document.set_file_property("ID", Some(explicit_id.to_owned()));
    document.render()
}

fn insert_heading_id(source: &str, line_number: usize, explicit_id: &str) -> Result<String> {
    let mut document = OrgDocument::from_source(source);
    document.set_heading_property(line_number, "ID", Some(explicit_id.to_owned()))?;
    Ok(document.render())
}

fn create_file_note(
    root: &Path,
    file_path: &str,
    title: &str,
    refs: &[String],
) -> Result<CaptureOutcome> {
    let relative_path = normalize_relative_org_path(file_path)?;
    let absolute_path = root.join(&relative_path);
    write_file_note(&absolute_path, title, refs)?;

    Ok(CaptureOutcome {
        absolute_path,
        node_key: format!("file:{}", relative_path.replace('\\', "/")),
    })
}

fn write_file_note(path: &Path, title: &str, refs: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let explicit_id = Uuid::new_v4().to_string();
    let mut content = format!("#+title: {title}\n:PROPERTIES:\n:ID: {explicit_id}\n");
    if !refs.is_empty() {
        content.push_str(&format!(":ROAM_REFS: {}\n", format_property_values(refs)));
    }
    content.push_str(":END:\n\n");
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

fn append_heading_to_source(source: &str, heading: &str, level: usize) -> (String, usize) {
    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    if !lines.is_empty() && lines.last().is_some_and(|line| !line.trim().is_empty()) {
        lines.push(String::new());
    }

    let line_number = lines.len() + 1;
    lines.push(format!("{} {}", "*".repeat(level), heading));

    (render_lines(&lines, source.ends_with('\n')), line_number)
}

fn append_heading_under_node(
    source: &str,
    line_number: usize,
    level: usize,
    heading: &str,
) -> Result<(String, usize)> {
    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    if line_number == 0 || line_number > lines.len() {
        bail!("heading line {line_number} is out of range");
    }

    let heading_index = line_number - 1;
    let mut insert_index = lines.len();
    for (index, line) in lines.iter().enumerate().skip(heading_index + 1) {
        if heading_level(line).is_some_and(|candidate| candidate <= level) {
            insert_index = index;
            break;
        }
    }

    if insert_index > 0 && !lines[insert_index - 1].trim().is_empty() {
        lines.insert(insert_index, String::new());
        insert_index += 1;
    }

    let line_number = insert_index + 1;
    lines.insert(
        insert_index,
        format!("{} {}", "*".repeat(level + 1), heading),
    );

    Ok((render_lines(&lines, source.ends_with('\n')), line_number))
}

fn normalized_title(title: &str) -> Result<&str> {
    let title = title.trim();
    if title.is_empty() {
        bail!("capture title must not be empty");
    }
    Ok(title)
}

fn normalize_relative_org_path(file_path: &str) -> Result<String> {
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

fn next_available_path(root: &Path, slug: &str) -> String {
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

fn next_available_relative_path(root: &Path, file_path: &str) -> Result<String> {
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

fn slugify(title: &str) -> String {
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

fn property_value(values: &[String]) -> Option<String> {
    if values.is_empty() {
        None
    } else {
        Some(format_property_values(values))
    }
}

fn keyword_value(values: &[String]) -> Option<String> {
    if values.is_empty() {
        None
    } else {
        Some(format_colon_tags(values))
    }
}

fn format_property_values(values: &[String]) -> String {
    values
        .iter()
        .map(|value| {
            if value
                .chars()
                .any(|character| character.is_whitespace() || character == '"')
            {
                format!("{value:?}")
            } else {
                value.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_colon_tags(values: &[String]) -> String {
    format!(":{}:", values.join(":"))
}

fn render_lines(lines: &[String], had_trailing_newline: bool) -> String {
    let mut rendered = lines.join("\n");
    if (had_trailing_newline || !rendered.ends_with('\n')) && !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered
}

fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let stars = trimmed
        .chars()
        .take_while(|character| *character == '*')
        .count();
    if stars == 0
        || !trimmed
            .chars()
            .nth(stars)
            .is_some_and(|character| character.is_whitespace())
    {
        return None;
    }
    Some(stars)
}

fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    if line.len() < keyword.len() {
        return None;
    }

    let prefix = &line[..keyword.len()];
    if prefix.eq_ignore_ascii_case(keyword) {
        Some(&line[keyword.len()..])
    } else {
        None
    }
}

fn split_heading_tags(input: &str) -> (&str, Vec<String>) {
    let Some(position) = input.rfind(" :") else {
        return (input.trim(), Vec::new());
    };
    let title = &input[..position];
    let suffix = &input[position + 1..];
    let tags = parse_colon_tags(suffix);
    if tags.is_empty() {
        (input.trim(), Vec::new())
    } else {
        (title.trim_end(), dedup_strings(tags))
    }
}

fn split_todo_keyword(input: &str) -> (Option<String>, &str) {
    let Some((first, rest)) = input.split_once(' ') else {
        return (None, input);
    };

    if looks_like_todo_keyword(first) {
        (Some(first.to_owned()), rest.trim_start())
    } else {
        (None, input)
    }
}

fn looks_like_todo_keyword(token: &str) -> bool {
    matches!(
        token,
        "TODO" | "DONE" | "NEXT" | "WAITING" | "STARTED" | "CANCELLED" | "HOLD"
    )
}

fn parse_colon_tags(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with(':') || !trimmed.ends_with(':') {
        return Vec::new();
    }

    trimmed
        .split(':')
        .filter(|part| !part.is_empty())
        .filter(|part| !part.chars().any(char::is_whitespace))
        .map(ToOwned::to_owned)
        .collect()
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !value.is_empty() && !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

fn shift_subtree_levels(lines: &mut [String], desired_root_level: usize) -> Result<()> {
    let current_root_level = lines
        .first()
        .and_then(|line| heading_level(line))
        .context("subtree must begin with a heading")?;
    let delta = desired_root_level as isize - current_root_level as isize;

    for line in lines {
        if let Some(level) = heading_level(line) {
            let trimmed = line.trim_start();
            let new_level = (level as isize + delta).max(1) as usize;
            *line = format!("{}{}", "*".repeat(new_level), &trimmed[level..]);
        }
    }

    Ok(())
}

fn promote_subtree_lines(lines: &mut [String]) {
    for line in lines {
        if let Some(level) = heading_level(line) {
            let trimmed = line.trim_start();
            let new_level = level.saturating_sub(1).max(1);
            *line = format!("{}{}", "*".repeat(new_level), &trimmed[level..]);
        }
    }
}

fn demote_all_headings(lines: &mut [String]) {
    for line in lines {
        if let Some(level) = heading_level(line) {
            let trimmed = line.trim_start();
            *line = format!("{}{}", "*".repeat(level + 1), &trimmed[level..]);
        }
    }
}

fn format_heading_line(level: usize, title: &str, tags: &[String]) -> String {
    if tags.is_empty() {
        format!("{} {}", "*".repeat(level), title)
    } else {
        format!(
            "{} {} {}",
            "*".repeat(level),
            title,
            format_colon_tags(tags)
        )
    }
}

struct OrgDocument {
    lines: Vec<String>,
    had_trailing_newline: bool,
}

impl OrgDocument {
    fn from_source(source: &str) -> Self {
        Self {
            lines: source.lines().map(ToOwned::to_owned).collect(),
            had_trailing_newline: source.ends_with('\n'),
        }
    }

    fn from_extracted_subtree(subtree_lines: &[String], explicit_id: &str) -> Result<Self> {
        let root_line = subtree_lines
            .first()
            .context("subtree must begin with a heading")?;
        let level = heading_level(root_line).context("subtree must begin with a heading")?;
        let heading_text = root_line.trim_start()[level + 1..].trim();
        let (title_text, tags) = split_heading_tags(heading_text);
        let (_, title) = split_todo_keyword(title_text);
        let title = title.trim();
        if title.is_empty() {
            bail!("subtree heading must include a title");
        }

        let mut lines = vec![format!("#+title: {title}")];
        if !tags.is_empty() {
            lines.push(format!("#+filetags: {}", format_colon_tags(&tags)));
        }
        let mut remainder = subtree_lines[1..].to_vec();
        promote_subtree_lines(&mut remainder);
        lines.extend(remainder);

        let mut document = Self {
            lines,
            had_trailing_newline: true,
        };
        document.set_file_property("ID", Some(explicit_id.to_owned()));
        Ok(document)
    }

    fn render(&self) -> String {
        if self.lines.is_empty() {
            String::new()
        } else {
            render_lines(&self.lines, self.had_trailing_newline)
        }
    }

    fn has_meaningful_content(&self) -> bool {
        self.lines.iter().any(|line| !line.trim().is_empty())
    }

    fn heading_index(&self, line_number: usize) -> Result<usize> {
        if line_number == 0 || line_number > self.lines.len() {
            bail!("heading line {line_number} is out of range");
        }
        let index = line_number - 1;
        if heading_level(&self.lines[index]).is_none() {
            bail!("line {line_number} is not a heading");
        }
        Ok(index)
    }

    fn subtree_range(&self, line_number: usize) -> Result<(usize, usize)> {
        let start = self.heading_index(line_number)?;
        let level = heading_level(&self.lines[start]).context("heading line is invalid")?;
        let mut end = self.lines.len();
        for (index, line) in self.lines.iter().enumerate().skip(start + 1) {
            if heading_level(line).is_some_and(|candidate| candidate <= level) {
                end = index;
                break;
            }
        }
        Ok((start, end))
    }

    fn insertion_index(&self, target: &NodeRecord) -> Result<usize> {
        match target.kind {
            NodeKind::File => Ok(self.lines.len()),
            NodeKind::Heading => {
                let start = self.heading_index(target.line as usize)?;
                let level = target.level as usize;
                let mut insert_index = self.lines.len();
                for (index, line) in self.lines.iter().enumerate().skip(start + 1) {
                    if heading_level(line).is_some_and(|candidate| candidate <= level) {
                        insert_index = index;
                        break;
                    }
                }
                Ok(insert_index)
            }
        }
    }

    fn remove_range(&mut self, start: usize, end: usize) {
        self.lines.drain(start..end);
    }

    fn insert_subtree(
        &mut self,
        index: usize,
        target_kind: NodeKind,
        mut subtree_lines: Vec<String>,
    ) {
        if matches!(target_kind, NodeKind::File)
            && index > 0
            && self
                .lines
                .get(index - 1)
                .is_some_and(|line| !line.trim().is_empty())
        {
            subtree_lines.insert(0, String::new());
        }

        self.lines.splice(index..index, subtree_lines);
    }

    fn subtree_lines(&self, node: &NodeRecord) -> Result<(Vec<String>, String)> {
        match node.kind {
            NodeKind::Heading => {
                let (start, end) = self.subtree_range(node.line as usize)?;
                let mut lines = self.lines[start..end].to_vec();
                let explicit_id = node
                    .explicit_id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                ensure_heading_property_value(&mut lines, 0, "ID", Some(explicit_id.clone()))?;
                Ok((lines, explicit_id))
            }
            NodeKind::File => {
                let mut lines = self.lines.clone();
                remove_file_keyword(&mut lines, "title");
                remove_file_keyword(&mut lines, "filetags");
                demote_all_headings(&mut lines);
                lines.insert(0, format_heading_line(1, &node.title, &node.tags));

                let explicit_id = node
                    .explicit_id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                ensure_heading_property_value(&mut lines, 0, "ID", Some(explicit_id.clone()))?;
                Ok((lines, explicit_id))
            }
        }
    }

    fn set_file_property(&mut self, property: &str, value: Option<String>) {
        set_file_property_value(&mut self.lines, property, value);
    }

    fn set_heading_property(
        &mut self,
        line_number: usize,
        property: &str,
        value: Option<String>,
    ) -> Result<()> {
        let heading_index = self.heading_index(line_number)?;
        ensure_heading_property_value(&mut self.lines, heading_index, property, value)
    }

    fn set_file_keyword(&mut self, keyword: &str, value: Option<String>) {
        set_file_keyword_value(&mut self.lines, keyword, value);
    }

    fn set_heading_tags(&mut self, line_number: usize, tags: &[String]) -> Result<()> {
        let index = self.heading_index(line_number)?;
        let level = heading_level(&self.lines[index]).context("heading line is invalid")?;
        let trimmed = self.lines[index].trim_start();
        let heading_text = trimmed[level + 1..].trim();
        let (title, _) = split_heading_tags(heading_text);
        self.lines[index] = format_heading_line(level, title, tags);
        Ok(())
    }
}

fn remove_file_keyword(lines: &mut Vec<String>, keyword: &str) {
    let limit = file_keyword_limit(lines);
    if let Some(index) = (0..limit)
        .find(|index| strip_keyword(lines[*index].trim_start(), &format!("#+{keyword}:")).is_some())
    {
        lines.remove(index);
    }
}

fn file_keyword_limit(lines: &[String]) -> usize {
    lines
        .iter()
        .position(|line| heading_level(line).is_some())
        .unwrap_or(lines.len())
}

fn file_property_insert_index(lines: &[String]) -> usize {
    let mut index = 0;
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }

    while index < lines.len() && lines[index].trim_start().starts_with("#+") {
        index += 1;
    }

    index
}

fn file_property_drawer_bounds(lines: &[String]) -> Option<(usize, usize)> {
    let mut index = file_property_insert_index(lines);
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }

    if lines
        .get(index)
        .is_some_and(|line| line.trim().eq_ignore_ascii_case(":PROPERTIES:"))
    {
        for (end, line) in lines.iter().enumerate().skip(index + 1) {
            if line.trim().eq_ignore_ascii_case(":END:") {
                return Some((index, end + 1));
            }
        }
    }

    None
}

fn set_file_property_value(lines: &mut Vec<String>, property: &str, value: Option<String>) {
    if let Some((start, end)) = file_property_drawer_bounds(lines) {
        let property_line = format!(":{property}:");
        if let Some(index) = (start + 1..end - 1)
            .find(|index| strip_keyword(lines[*index].trim(), &property_line).is_some())
        {
            if let Some(value) = value {
                lines[index] = format!(":{property}: {value}");
            } else {
                lines.remove(index);
                if start + 1 == end - 2 {
                    lines.drain(start..start + 2);
                }
            }
            return;
        }

        if let Some(value) = value {
            lines.insert(end - 1, format!(":{property}: {value}"));
        }
        return;
    }

    if let Some(value) = value {
        let insert_index = file_property_insert_index(lines);
        let mut drawer = vec![
            String::from(":PROPERTIES:"),
            format!(":{property}: {value}"),
            String::from(":END:"),
        ];
        if insert_index == lines.len()
            || lines
                .get(insert_index)
                .is_some_and(|line| !line.trim().is_empty())
        {
            drawer.push(String::new());
        }
        lines.splice(insert_index..insert_index, drawer);
    }
}

fn set_file_keyword_value(lines: &mut Vec<String>, keyword: &str, value: Option<String>) {
    let needle = format!("#+{keyword}:");
    let limit = file_keyword_limit(lines);
    if let Some(index) =
        (0..limit).find(|index| strip_keyword(lines[*index].trim_start(), &needle).is_some())
    {
        if let Some(value) = value {
            lines[index] = format!("#+{}: {value}", keyword.to_ascii_lowercase());
        } else {
            lines.remove(index);
        }
        return;
    }

    if let Some(value) = value {
        let insert_index = file_property_insert_index(lines);
        lines.insert(
            insert_index,
            format!("#+{}: {value}", keyword.to_ascii_lowercase()),
        );
    }
}

fn heading_property_drawer_bounds(
    lines: &[String],
    heading_index: usize,
) -> Option<(usize, usize)> {
    let drawer_start = heading_index + 1;
    if lines
        .get(drawer_start)
        .is_some_and(|line| line.trim().eq_ignore_ascii_case(":PROPERTIES:"))
    {
        for (end, line) in lines.iter().enumerate().skip(drawer_start + 1) {
            if line.trim().eq_ignore_ascii_case(":END:") {
                return Some((drawer_start, end + 1));
            }
        }
    }
    None
}

fn ensure_heading_property_value(
    lines: &mut Vec<String>,
    heading_index: usize,
    property: &str,
    value: Option<String>,
) -> Result<()> {
    if heading_index >= lines.len() || heading_level(&lines[heading_index]).is_none() {
        bail!("heading line {} is out of range", heading_index + 1);
    }

    if let Some((start, end)) = heading_property_drawer_bounds(lines, heading_index) {
        let property_line = format!(":{property}:");
        if let Some(index) = (start + 1..end - 1)
            .find(|index| strip_keyword(lines[*index].trim(), &property_line).is_some())
        {
            if let Some(value) = value {
                lines[index] = format!(":{property}: {value}");
            } else {
                lines.remove(index);
                if start + 1 == end - 2 {
                    lines.drain(start..start + 2);
                }
            }
            return Ok(());
        }

        if let Some(value) = value {
            lines.insert(end - 1, format!(":{property}: {value}"));
        }
        return Ok(());
    }

    if let Some(value) = value {
        lines.splice(
            heading_index + 1..heading_index + 1,
            [
                String::from(":PROPERTIES:"),
                format!(":{property}: {value}"),
                String::from(":END:"),
            ],
        );
    }

    Ok(())
}
