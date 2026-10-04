use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use slipbox_core::{
    GenerationBinding, GenerationId, GitBranch, NotesFolder, RemoteUrl, SourceConfiguration,
    SourceDisplayName, SourceId, SourceProvider, SourceRecord, SourceVisibility,
};
use slipbox_git::{DeltaRequest, SnapshotRequest, derive_delta, materialize};
use slipbox_store::{Database, DirectedRelationPosition, GlossaryPosition, NotePosition};
use slipbox_sync::{
    GenerationLease, GenerationStore, PublicationError, PublicationStage, PublishGenerationRequest,
    StageIndexError, StageIndexRequest, StagedIndexOutcome, stage_index,
};
use tempfile::TempDir;

const SOURCE: &str = "0123456789abcdef0123456789abcdef";
const PAGE_SIZE: usize = 7;

#[test]
fn substantial_git_corpora_remain_coherent_through_updates_and_rewritten_history() -> Result<()> {
    for size in [32, 256] {
        exercise_corpus(size)?;
    }
    Ok(())
}

fn exercise_corpus(size: usize) -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write_corpus(size, "initial-marker", b"asset-one")?;
    let first_revision = fixture.commit_all("initial corpus")?;
    let first = fixture
        .publish("g1", None, None, None, &first_revision, "first")
        .context("publish the initial generation")?;
    assert_generation(
        &first,
        size + 1,
        glossary_count(size),
        "initial-marker",
        b"asset-one",
    )
    .context("read the initial generation")?;

    fixture.remove("notes/note-0001.org")?;
    fixture.rename("notes/note-0002.org", "notes/moved-0002.org")?;
    fixture.write_note("notes/moved-0002.org", 2, "moved-marker", false)?;
    fixture.write_note("notes/note-0003.org", 3, "revision-two-marker", true)?;
    fixture.write_bytes("assets/figure.txt", b"asset-two")?;
    let second_revision = fixture.commit_all("incremental update")?;
    let second = fixture
        .publish(
            "g2",
            Some(GenerationId::parse("g1")?),
            Some(&first_revision),
            Some(first.index_directory().to_owned()),
            &second_revision,
            "second",
        )
        .context("publish the incremental generation")?;
    assert_generation(
        &second,
        size,
        glossary_count(size),
        "revision-two-marker",
        b"asset-two",
    )
    .context("read the incremental generation")?;
    let second_database = Database::open(second.database())?;
    assert!(second_database.node_from_id("note-0001")?.is_none());
    assert_eq!(
        second_database
            .node_from_id("note-0002")?
            .context("renamed note")?
            .file_path,
        "notes/moved-0002.org"
    );
    assert_eq!(content(&first, "assets/figure.txt")?, b"asset-one");

    fixture.reset(&first_revision)?;
    fixture.write_note("notes/note-0003.org", 3, "rewritten-history-marker", true)?;
    fixture.write_bytes("assets/figure.txt", b"asset-rewritten")?;
    let rewritten_revision = fixture.commit_all("rewritten branch")?;
    let rewritten = fixture
        .publish(
            "g3",
            Some(GenerationId::parse("g2")?),
            Some(&second_revision),
            Some(second.index_directory().to_owned()),
            &rewritten_revision,
            "rewritten",
        )
        .context("publish the rewritten generation")?;
    assert_generation(
        &rewritten,
        size + 1,
        glossary_count(size),
        "rewritten-history-marker",
        b"asset-rewritten",
    )
    .context("read the rewritten generation")?;
    let rewritten_database = Database::open(rewritten.database())?;
    assert!(rewritten_database.node_from_id("note-0001")?.is_some());
    assert_eq!(
        rewritten_database
            .node_from_id("note-0002")?
            .context("restored note")?
            .file_path,
        "notes/note-0002.org"
    );

    fixture.write_note("notes/note-0004.org", 4, "interrupted-marker", false)?;
    let interrupted_revision = fixture.commit_all("interrupted update")?;
    let (snapshot, delta) = fixture.snapshot_and_delta(
        Some(&rewritten_revision),
        &interrupted_revision,
        "cancelled-index",
    )?;
    let cancelled_index = fixture
        .store
        .candidates_root()
        .join("index-cancelled-index");
    let request = StageIndexRequest::new(
        snapshot,
        delta,
        Some(rewritten.index_directory().to_owned()),
        cancelled_index.clone(),
    )?;
    assert_eq!(
        stage_index(&request, &AtomicBool::new(true), |_| {}),
        Err(StageIndexError::Cancelled)
    );
    assert!(!cancelled_index.exists());
    assert_active(&fixture.store, "g3", "rewritten-history-marker")?;

    let (snapshot, index) = fixture.candidates(
        Some(&rewritten_revision),
        Some(rewritten.index_directory().to_owned()),
        &interrupted_revision,
        "cancelled-publication",
    )?;
    let request =
        fixture.publish_request("g4", Some(GenerationId::parse("g3")?), snapshot, index)?;
    let cancelled = AtomicBool::new(false);
    let result = fixture.store.publish(&request, &cancelled, |stage| {
        if stage == PublicationStage::SourceSealed {
            cancelled.store(true, Ordering::Relaxed);
        }
    });
    assert_eq!(result.unwrap_err(), PublicationError::Cancelled);
    assert_active(&fixture.store, "g3", "rewritten-history-marker")?;
    Ok(())
}

fn assert_generation(
    generation: &GenerationLease,
    expected_notes: usize,
    expected_glossary: usize,
    marker: &str,
    asset: &[u8],
) -> Result<()> {
    let database = Database::open(generation.database())?;

    let mut notes = Vec::new();
    let mut note_position = None;
    let mut crossed_note_page = false;
    loop {
        let page = database.list_notes(PAGE_SIZE, note_position.as_ref())?;
        assert_eq!(page.total, expected_notes);
        notes.extend(page.notes);
        crossed_note_page |= page.has_more;
        let Some(token) = page.next_position else {
            break;
        };
        note_position = Some(NotePosition::parse(&token).context("note cursor")?);
    }
    assert!(crossed_note_page);
    assert_eq!(notes.len(), expected_notes);
    assert_eq!(
        notes
            .iter()
            .map(|note| &note.node_key)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        expected_notes
    );

    let mut glossary = Vec::new();
    let mut glossary_position = None;
    let mut crossed_glossary_page = false;
    loop {
        let page = database.list_glossary_terms(PAGE_SIZE, glossary_position.as_ref())?;
        assert_eq!(page.total, expected_glossary);
        glossary.extend(page.terms);
        crossed_glossary_page |= page.has_more;
        let Some(token) = page.next_position else {
            break;
        };
        glossary_position = Some(GlossaryPosition::parse_term(&token).context("glossary cursor")?);
    }
    assert!(crossed_glossary_page);
    assert_eq!(glossary.len(), expected_glossary);

    let hub = database.node_from_id("hub-id")?.context("hub note")?;
    let mut relations = Vec::new();
    let mut relation_position = None;
    let mut crossed_relation_page = false;
    loop {
        let page = database.directed_relations(&hub, PAGE_SIZE, relation_position.as_ref())?;
        assert_eq!(page.total as usize, expected_notes - 1);
        relations.extend(page.relations);
        crossed_relation_page |= page.has_more;
        let Some(token) = page.next_position else {
            break;
        };
        relation_position =
            Some(DirectedRelationPosition::parse(&token).context("relation cursor")?);
    }
    assert!(crossed_relation_page);
    assert_eq!(relations.len(), expected_notes - 1);
    assert!(!database.search_corpus(marker, 10)?.is_empty());
    assert!(
        !database
            .search_occurrence_document_paths(marker, 10, 0)?
            .is_empty()
    );
    assert_eq!(content(generation, "assets/figure.txt")?, asset);
    Ok(())
}

fn assert_active(store: &GenerationStore, expected: &str, marker: &str) -> Result<()> {
    let active = store.lease()?.context("last good generation")?;
    assert_eq!(active.binding().generation.as_str(), expected);
    assert!(
        !Database::open(active.database())?
            .search_corpus(marker, 10)?
            .is_empty()
    );
    Ok(())
}

fn content(generation: &GenerationLease, relative: &str) -> Result<Vec<u8>> {
    let mut file = generation.open_content(relative)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn glossary_count(size: usize) -> usize {
    1 + size.div_ceil(3)
}

struct Fixture {
    _root: TempDir,
    repository: PathBuf,
    source: SourceId,
    record: SourceRecord,
    notes: NotesFolder,
    store: GenerationStore,
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
        let source = SourceId::parse(SOURCE)?;
        let notes = NotesFolder::parse("notes")?;
        let record = SourceRecord::new(SourceConfiguration {
            id: source.clone(),
            display_name: SourceDisplayName::parse("Journey fixture")?,
            provider: SourceProvider::GenericHttps,
            visibility: SourceVisibility::Public,
            provider_repository_id: None,
            account: None,
            remote: RemoteUrl::parse("https://example.invalid/journey.git")?,
            branch: GitBranch::parse("main")?,
            notes_folder: notes.clone(),
            credential: None,
        })?;
        let store = GenerationStore::initialize(source.clone(), root.path().to_owned())?;
        Ok(Self {
            _root: root,
            repository,
            source,
            record,
            notes,
            store,
        })
    }

    fn write_corpus(&self, size: usize, marker: &str, asset: &[u8]) -> Result<()> {
        self.write(
            "notes/hub.org",
            "#+title: Hub\n#+glossary: t\n:PROPERTIES:\n:ID: hub-id\n:END:\nCentral concept.\n",
        )?;
        for index in 0..size {
            self.write_note(
                &format!("notes/note-{index:04}.org"),
                index,
                marker,
                index % 3 == 0,
            )?;
        }
        self.write_bytes("assets/figure.txt", asset)
    }

    fn write_note(&self, relative: &str, index: usize, marker: &str, glossary: bool) -> Result<()> {
        let glossary = if glossary { "#+glossary: t\n" } else { "" };
        self.write(
            relative,
            &format!(
                "#+title: Note {index:04}\n{glossary}:PROPERTIES:\n:ID: note-{index:04}\n:END:\n{marker} token-{index:04}. [[id:hub-id][Hub]].\n"
            ),
        )
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
        fs::rename(self.repository.join(from), self.repository.join(to))?;
        Ok(())
    }

    fn reset(&self, revision: &str) -> Result<()> {
        run_git_in(&self.repository, ["reset", "--hard", revision])
    }

    fn commit_all(&self, message: &str) -> Result<String> {
        run_git_in(&self.repository, ["add", "-A"])?;
        run_git_in(&self.repository, ["commit", "-m", message])?;
        Ok(git_stdout_in(&self.repository, ["rev-parse", "HEAD"])?
            .trim()
            .to_owned())
    }

    fn publish(
        &self,
        generation: &str,
        expected: Option<GenerationId>,
        previous: Option<&str>,
        base: Option<PathBuf>,
        revision: &str,
        name: &str,
    ) -> Result<GenerationLease> {
        let (snapshot, index) = self.candidates(previous, base, revision, name)?;
        let request = self.publish_request(generation, expected, snapshot, index)?;
        Ok(self
            .store
            .publish(&request, &AtomicBool::new(false), |_| {})?
            .generation)
    }

    fn candidates(
        &self,
        previous: Option<&str>,
        base: Option<PathBuf>,
        revision: &str,
        name: &str,
    ) -> Result<(slipbox_git::SnapshotOutcome, StagedIndexOutcome)> {
        let (snapshot, delta) = self.snapshot_and_delta(previous, revision, name)?;
        let request = StageIndexRequest::new(
            snapshot.clone(),
            delta,
            base,
            self.store.candidates_root().join(format!("index-{name}")),
        )?;
        let index = stage_index(&request, &AtomicBool::new(false), |_| {})?;
        Ok((snapshot, index))
    }

    fn snapshot_and_delta(
        &self,
        previous: Option<&str>,
        revision: &str,
        name: &str,
    ) -> Result<(slipbox_git::SnapshotOutcome, slipbox_git::DeltaOutcome)> {
        let snapshot = materialize(
            &SnapshotRequest::new(
                self.source.clone(),
                self.repository.clone(),
                revision,
                self.notes.clone(),
                self.store
                    .candidates_root()
                    .join(format!("snapshot-{name}")),
            )?,
            &AtomicBool::new(false),
            |_| {},
        )?;
        let request = match previous {
            Some(previous) => DeltaRequest::between(
                self.source.clone(),
                self.repository.clone(),
                previous,
                revision,
                self.notes.clone(),
            )?,
            None => DeltaRequest::initial(
                self.source.clone(),
                self.repository.clone(),
                revision,
                self.notes.clone(),
            )?,
        };
        Ok((snapshot, derive_delta(&request, &AtomicBool::new(false))?))
    }

    fn publish_request(
        &self,
        generation: &str,
        expected: Option<GenerationId>,
        snapshot: slipbox_git::SnapshotOutcome,
        index: StagedIndexOutcome,
    ) -> Result<PublishGenerationRequest> {
        Ok(PublishGenerationRequest::new(
            self.record.clone(),
            GenerationBinding::new(self.source.clone(), GenerationId::parse(generation)?),
            expected,
            snapshot,
            index,
        )?)
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
