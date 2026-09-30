use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result};
use serde_json::Value;
use slipbox_core::{NotesFolder, SourceId};
use slipbox_git::{
    DeltaOutcome, DeltaRequest, SnapshotOutcome, SnapshotRequest, derive_delta, materialize,
};
use slipbox_store::Database;
use slipbox_sync::{
    STAGED_INDEX_MANIFEST_FILE, StageIndexError, StageIndexRequest, StagedIndexDisposition,
    inspect_staged_index, stage_index,
};
use tempfile::TempDir;

const SOURCE: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn incremental_candidate_matches_a_clean_rebuild_across_reader_semantics() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write(
        "notes/alpha.org",
        r#"#+title: Alpha
:PROPERTIES:
:ID: alpha-id
:END:

Alpha points to [[id:beta-id][Beta]].
"#,
    )?;
    fixture.write(
        "notes/beta.org",
        r#"#+title: Beta
#+glossary: t
:PROPERTIES:
:ID: beta-id
:ROAM_ALIASES: "Second"
:END:

Beta definition with searchable basalt.
"#,
    )?;
    fixture.write(
        "notes/delete.org",
        r#"#+title: Deleted term
#+glossary: t
:PROPERTIES:
:ID: delete-id
:END:

Obsolete vocabulary.
"#,
    )?;
    fixture.write(
        "notes/rename.org",
        r#"#+title: Moving note
:PROPERTIES:
:ID: moving-id
:END:

Moving points to [[id:beta-id][Beta]].
"#,
    )?;
    fixture.write(
        "notes/stable.org",
        r#"#+title: Stable
:PROPERTIES:
:ID: stable-id
:END:

Stable points to [[id:alpha-id][Alpha]].
"#,
    )?;
    fixture.write(
        "notes/trail.org",
        "#+title: Reading trail\n\nStable prose.\n",
    )?;
    fixture.write_bytes("assets/old.png", b"image bytes")?;
    let previous = fixture.commit_all("initial corpus")?;
    let previous_snapshot = fixture.snapshot(&previous, "snapshot-previous")?;
    let previous_delta = fixture.initial_delta(&previous)?;
    let base = fixture.stage(previous_snapshot, previous_delta, None, "index-base")?;

    fixture.write(
        "notes/alpha.org",
        r#"#+title: Alpha revised
:PROPERTIES:
:ID: alpha-id
:END:

Alpha now points to [[id:moving-id][Moved]].
"#,
    )?;
    fixture.write(
        "notes/beta.org",
        r#"#+title: Beta revised
#+glossary: t
:PROPERTIES:
:ID: beta-id
:ROAM_ALIASES: "Second concept"
:END:

Beta definition now contains searchable granite.
"#,
    )?;
    fixture.remove("notes/delete.org")?;
    fixture.rename("notes/rename.org", "notes/moved.org")?;
    fixture.write(
        "notes/moved.org",
        r#"#+title: Moved note
:PROPERTIES:
:ID: moving-id
:END:

Moved points to [[id:alpha-id][Alpha revised]].
"#,
    )?;
    fixture.write(
        "notes/gamma.org",
        r#"#+title: Gamma
#+glossary: t
:PROPERTIES:
:ID: gamma-id
:END:

Gamma points to [[id:alpha-id][Alpha revised]].
"#,
    )?;
    fixture.rename("assets/old.png", "assets/new.png")?;
    fixture.rename("notes/trail.org", "notes/trail-moved.org")?;
    let revision = fixture.commit_all("mixed repository delta")?;
    let incremental_snapshot = fixture.snapshot(&revision, "snapshot-incremental")?;
    let full_snapshot = fixture.snapshot(&revision, "snapshot-full")?;
    let delta = fixture.delta(&previous, &revision)?;

    let incremental = fixture.stage(
        incremental_snapshot,
        delta,
        Some(base.database.parent().context("base directory")?.to_owned()),
        "index-incremental",
    )?;
    let full = fixture.stage(
        full_snapshot,
        fixture.initial_delta(&revision)?,
        None,
        "index-full",
    )?;

    assert_eq!(incremental.parsed_org_files, 5);
    assert_eq!(incremental.removed_file_paths, 3);
    assert_databases_equivalent(
        &incremental.database,
        &full.database,
        &["alpha-id", "beta-id", "moving-id", "gamma-id", "stable-id"],
    )?;

    let database = Database::open(&incremental.database)?;
    assert_eq!(incremental.reading_file_renames, 1);
    assert!(database.node_from_id("delete-id")?.is_none());
    assert!(database.file_record("notes/delete.org")?.is_none());
    assert!(database.file_record("notes/rename.org")?.is_none());
    assert_eq!(
        database
            .node_from_id("moving-id")?
            .context("renamed note")?
            .file_path,
        "notes/moved.org"
    );
    assert_eq!(
        database
            .note_by_key("file:notes/trail.org")?
            .context("reading alias for renamed file note")?
            .node_key,
        "file:notes/trail-moved.org"
    );
    assert!(
        database
            .search_nodes("obsolete vocabulary", 20, None)?
            .is_empty()
    );
    assert_eq!(
        database.search_occurrence_document_paths("granite", 20, 0)?,
        vec!["notes/beta.org"]
    );
    drop(database);

    fixture.rename("notes/trail-moved.org", "notes/trail-final.org")?;
    let chained_revision = fixture.commit_all("move the trail again")?;
    let chained = fixture.stage(
        fixture.snapshot(&chained_revision, "snapshot-chained")?,
        fixture.delta(&revision, &chained_revision)?,
        Some(
            incremental
                .database
                .parent()
                .context("incremental directory")?
                .to_owned(),
        ),
        "index-chained",
    )?;
    let database = Database::open(&chained.database)?;
    for previous in ["trail.org", "trail-moved.org"] {
        assert_eq!(
            database
                .note_by_key(&format!("file:notes/{previous}"))?
                .context("chained reading alias")?
                .node_key,
            "file:notes/trail-final.org"
        );
    }
    drop(database);

    fixture.write(
        "notes/trail.org",
        "#+title: Reused path\n\nDifferent note.\n",
    )?;
    let reused_revision = fixture.commit_all("reuse the original path")?;
    let reused = fixture.stage(
        fixture.snapshot(&reused_revision, "snapshot-reused")?,
        fixture.delta(&chained_revision, &reused_revision)?,
        Some(
            chained
                .database
                .parent()
                .context("chained directory")?
                .to_owned(),
        ),
        "index-reused",
    )?;
    let database = Database::open(&reused.database)?;
    assert_eq!(
        database
            .note_by_key("file:notes/trail.org")?
            .context("current key wins over a reading alias")?
            .title,
        "Reused path"
    );
    assert_eq!(
        database
            .note_by_key("file:notes/trail-moved.org")?
            .context("unreused alias retains the moved note")?
            .node_key,
        "file:notes/trail-final.org"
    );
    Ok(())
}

#[test]
fn a_file_delta_neither_parses_nor_prunes_unchanged_files() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write(
        "notes/changed.org",
        "#+title: Before\n:PROPERTIES:\n:ID: changed-id\n:END:\n",
    )?;
    fixture.write(
        "notes/unchanged.org",
        "#+title: Unchanged\n:PROPERTIES:\n:ID: unchanged-id\n:END:\n",
    )?;
    let previous = fixture.commit_all("two notes")?;
    let base = fixture.stage(
        fixture.snapshot(&previous, "snapshot-base")?,
        fixture.initial_delta(&previous)?,
        None,
        "index-base",
    )?;

    fixture.write(
        "notes/changed.org",
        "#+title: After\n:PROPERTIES:\n:ID: changed-id\n:END:\n",
    )?;
    let revision = fixture.commit_all("change one note")?;
    let snapshot = fixture.snapshot(&revision, "snapshot-current")?;
    fs::remove_file(snapshot.content_root.join("notes/unchanged.org"))?;
    let staged = fixture.stage(
        snapshot,
        fixture.delta(&previous, &revision)?,
        Some(base.database.parent().context("base directory")?.to_owned()),
        "index-current",
    )?;

    assert_eq!(staged.parsed_org_files, 1);
    assert_eq!(staged.removed_file_paths, 0);
    let database = Database::open(&staged.database)?;
    assert_eq!(
        database
            .node_from_id("changed-id")?
            .context("changed note")?
            .title,
        "After"
    );
    assert_eq!(
        database
            .node_from_id("unchanged-id")?
            .context("unchanged note")?
            .title,
        "Unchanged"
    );
    Ok(())
}

#[test]
fn an_asset_only_delta_advances_the_candidate_without_index_work() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write(
        "notes/alpha.org",
        "#+title: Alpha\n:PROPERTIES:\n:ID: alpha-id\n:END:\n",
    )?;
    fixture.write_bytes("assets/figure.png", b"first image")?;
    let previous = fixture.commit_all("initial note and asset")?;
    let base = fixture.stage(
        fixture.snapshot(&previous, "snapshot-base")?,
        fixture.initial_delta(&previous)?,
        None,
        "index-base",
    )?;

    fixture.write_bytes("assets/figure.png", b"second image")?;
    let revision = fixture.commit_all("change only the asset")?;
    let staged = fixture.stage(
        fixture.snapshot(&revision, "snapshot-current")?,
        fixture.delta(&previous, &revision)?,
        Some(base.database.parent().context("base directory")?.to_owned()),
        "index-current",
    )?;

    assert_eq!(staged.revision, revision);
    assert_eq!(staged.parsed_org_files, 0);
    assert_eq!(staged.removed_file_paths, 0);
    assert_databases_equivalent(&staged.database, &base.database, &["alpha-id"])?;
    Ok(())
}

#[test]
fn staging_failure_and_cancellation_leave_the_completed_base_untouched() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write(
        "notes/alpha.org",
        "#+title: Alpha\n:PROPERTIES:\n:ID: alpha-id\n:END:\n",
    )?;
    let previous = fixture.commit_all("initial")?;
    let base = fixture.stage(
        fixture.snapshot(&previous, "snapshot-base")?,
        fixture.initial_delta(&previous)?,
        None,
        "index-base",
    )?;
    let base_directory = base.database.parent().context("base directory")?;
    let database_before = fs::read(&base.database)?;
    let manifest_before = fs::read(base_directory.join(STAGED_INDEX_MANIFEST_FILE))?;

    fixture.write(
        "notes/alpha.org",
        "#+title: Broken\n:PROPERTIES:\n:ID: alpha-id\n:END:\n",
    )?;
    let revision = fixture.commit_all("change alpha")?;
    let snapshot = fixture.snapshot(&revision, "snapshot-current")?;
    fs::write(snapshot.content_root.join("notes/alpha.org"), [0xff, 0xfe])?;
    let destination = fixture.root.path().join("index-failed");
    let request = StageIndexRequest::new(
        snapshot,
        fixture.delta(&previous, &revision)?,
        Some(base_directory.to_owned()),
        destination.clone(),
    )?;

    assert_eq!(
        stage_index(&request, &AtomicBool::new(false), |_| {}),
        Err(StageIndexError::InputInvalid)
    );
    assert!(!destination.exists());
    assert_eq!(fs::read(&base.database)?, database_before);
    assert_eq!(
        fs::read(base_directory.join(STAGED_INDEX_MANIFEST_FILE))?,
        manifest_before
    );

    let cancelled_destination = fixture.root.path().join("index-cancelled");
    let request = StageIndexRequest::new(
        fixture.snapshot(&revision, "snapshot-cancelled")?,
        fixture.delta(&previous, &revision)?,
        Some(base_directory.to_owned()),
        cancelled_destination.clone(),
    )?;
    assert_eq!(
        stage_index(&request, &AtomicBool::new(true), |_| {}),
        Err(StageIndexError::Cancelled)
    );
    assert!(!cancelled_destination.exists());
    assert_eq!(fs::read(&base.database)?, database_before);
    Ok(())
}

#[test]
fn schema_mismatch_requires_an_explicit_rebuild_and_completed_stages_are_idempotent() -> Result<()>
{
    let fixture = Fixture::new()?;
    fixture.write("notes/alpha.org", "#+title: Alpha\n")?;
    let previous = fixture.commit_all("initial")?;
    let snapshot = fixture.snapshot(&previous, "snapshot-base")?;
    let delta = fixture.initial_delta(&previous)?;
    let destination = fixture.root.path().join("index-base");
    let request = StageIndexRequest::new(snapshot, delta, None, destination.clone())?;
    let created = stage_index(&request, &AtomicBool::new(false), |_| {})?;
    let existing = stage_index(&request, &AtomicBool::new(false), |_| {})?;
    assert_eq!(created.disposition, StagedIndexDisposition::Created);
    assert_eq!(existing.disposition, StagedIndexDisposition::Existing);
    assert_eq!(created.database, existing.database);
    assert_eq!(inspect_staged_index(&destination)?.stats, created.stats);

    let manifest_path = destination.join(STAGED_INDEX_MANIFEST_FILE);
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    manifest["index_schema"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;

    fixture.write("notes/alpha.org", "#+title: Alpha revised\n")?;
    let revision = fixture.commit_all("update")?;
    let next_destination = fixture.root.path().join("index-next");
    let request = StageIndexRequest::new(
        fixture.snapshot(&revision, "snapshot-next")?,
        fixture.delta(&previous, &revision)?,
        Some(destination),
        next_destination.clone(),
    )?;
    assert_eq!(
        stage_index(&request, &AtomicBool::new(false), |_| {}),
        Err(StageIndexError::RebuildRequired)
    );
    assert!(!next_destination.exists());
    Ok(())
}

fn assert_databases_equivalent(incremental: &Path, full: &Path, node_ids: &[&str]) -> Result<()> {
    let incremental = Database::open(incremental)?;
    let full = Database::open(full)?;
    assert_eq!(incremental.stats()?, full.stats()?);
    assert_eq!(incremental.indexed_files()?, full.indexed_files()?);
    for query in ["", "revised", "granite", "Gamma", "obsolete"] {
        assert_eq!(
            incremental.search_nodes(query, 200, None)?,
            full.search_nodes(query, 200, None)?,
            "node query {query:?} differed"
        );
    }
    for query in ["granite", "points to", "obsolete vocabulary"] {
        assert_eq!(
            incremental.search_occurrence_document_paths(query, 200, 0)?,
            full.search_occurrence_document_paths(query, 200, 0)?,
            "occurrence query {query:?} differed"
        );
    }
    let incremental_glossary = incremental.list_glossary_terms(200, None)?;
    let full_glossary = full.list_glossary_terms(200, None)?;
    assert_eq!(incremental_glossary.terms, full_glossary.terms);
    assert_eq!(incremental_glossary.total, full_glossary.total);
    assert_eq!(incremental_glossary.has_more, full_glossary.has_more);
    assert_eq!(
        incremental_glossary.next_position,
        full_glossary.next_position
    );
    for id in node_ids {
        let incremental_note = incremental
            .node_from_id(id)?
            .with_context(|| format!("incremental node {id}"))?;
        let full_note = full
            .node_from_id(id)?
            .with_context(|| format!("full node {id}"))?;
        assert_eq!(incremental_note, full_note);
        assert_eq!(
            incremental.backlink_note_count(&incremental_note.node_key)?,
            full.backlink_note_count(&full_note.node_key)?
        );
        assert_eq!(
            incremental.forward_link_note_count(&incremental_note.node_key)?,
            full.forward_link_note_count(&full_note.node_key)?
        );
        assert_eq!(
            incremental.backlinks(&incremental_note.node_key, 200, false)?,
            full.backlinks(&full_note.node_key, 200, false)?
        );
        assert_eq!(
            incremental.forward_links(&incremental_note.node_key, 200, false)?,
            full.forward_links(&full_note.node_key, 200, false)?
        );
    }
    Ok(())
}

struct Fixture {
    root: TempDir,
    repository: PathBuf,
    source: SourceId,
    notes: NotesFolder,
}

impl Fixture {
    fn new() -> Result<Self> {
        let root = tempfile::tempdir()?;
        let repository = root.path().join("repository");
        run_git(["init", text(&repository)])?;
        run_git_in(&repository, ["config", "user.name", "Slipbox Fixture"])?;
        run_git_in(
            &repository,
            ["config", "user.email", "fixture@example.invalid"],
        )?;
        run_git_in(&repository, ["config", "gc.auto", "0"])?;
        run_git_in(&repository, ["config", "maintenance.auto", "false"])?;
        run_git_in(&repository, ["branch", "-M", "main"])?;
        Ok(Self {
            root,
            repository,
            source: SourceId::parse(SOURCE).expect("fixture source"),
            notes: NotesFolder::parse("notes").expect("fixture notes folder"),
        })
    }

    fn write(&self, relative: &str, contents: &str) -> Result<()> {
        self.write_bytes(relative, contents.as_bytes())
    }

    fn write_bytes(&self, relative: &str, contents: &[u8]) -> Result<()> {
        let path = self.repository.join(relative);
        fs::create_dir_all(path.parent().context("fixture parent")?)?;
        fs::write(path, contents)?;
        Ok(())
    }

    fn remove(&self, relative: &str) -> Result<()> {
        fs::remove_file(self.repository.join(relative))?;
        Ok(())
    }

    fn rename(&self, from: &str, to: &str) -> Result<()> {
        let destination = self.repository.join(to);
        fs::create_dir_all(destination.parent().context("fixture parent")?)?;
        fs::rename(self.repository.join(from), destination)?;
        Ok(())
    }

    fn commit_all(&self, message: &str) -> Result<String> {
        run_git_in(&self.repository, ["add", "-A"])?;
        run_git_in(&self.repository, ["commit", "-m", message])?;
        Ok(git_stdout_in(&self.repository, ["rev-parse", "HEAD"])?
            .trim()
            .to_owned())
    }

    fn snapshot(&self, revision: &str, name: &str) -> Result<SnapshotOutcome> {
        let request = SnapshotRequest::new(
            self.source.clone(),
            self.repository.clone(),
            revision,
            self.notes.clone(),
            self.root.path().join(name),
        )?;
        Ok(materialize(&request, &AtomicBool::new(false), |_| {})?)
    }

    fn initial_delta(&self, revision: &str) -> Result<DeltaOutcome> {
        let request = DeltaRequest::initial(
            self.source.clone(),
            self.repository.clone(),
            revision,
            self.notes.clone(),
        )?;
        Ok(derive_delta(&request, &AtomicBool::new(false))?)
    }

    fn delta(&self, previous: &str, revision: &str) -> Result<DeltaOutcome> {
        let request = DeltaRequest::between(
            self.source.clone(),
            self.repository.clone(),
            previous,
            revision,
            self.notes.clone(),
        )?;
        Ok(derive_delta(&request, &AtomicBool::new(false))?)
    }

    fn stage(
        &self,
        snapshot: SnapshotOutcome,
        delta: DeltaOutcome,
        base: Option<PathBuf>,
        name: &str,
    ) -> Result<slipbox_sync::StagedIndexOutcome> {
        let request = StageIndexRequest::new(snapshot, delta, base, self.root.path().join(name))?;
        Ok(stage_index(&request, &AtomicBool::new(false), |_| {})?)
    }
}

fn run_git<const N: usize>(arguments: [&str; N]) -> Result<()> {
    let output = Command::new("git").args(arguments).output()?;
    anyhow::ensure!(
        output.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn run_git_in<const N: usize>(directory: &Path, arguments: [&str; N]) -> Result<()> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn git_stdout_in<const N: usize>(directory: &Path, arguments: [&str; N]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

fn text(path: &Path) -> &str {
    path.to_str().expect("fixture path is UTF-8")
}
