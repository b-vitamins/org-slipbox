use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};

use anyhow::{Context, Result};
use serde_json::json;
use slipbox_core::{GenerationBinding, GenerationId, NotesFolder, SourceId};
use slipbox_git::{
    DeltaOutcome, DeltaRequest, SnapshotOutcome, SnapshotRequest, derive_delta, materialize,
};
use slipbox_store::Database;
use slipbox_sync::{
    ACTIVE_GENERATION_FILE, GenerationLease, GenerationStore, InFlightGeneration,
    PublicationDisposition, PublicationError, PublicationStage, PublishGenerationRequest,
    StageIndexRequest, StagedIndexOutcome, stage_index,
};
use tempfile::TempDir;

const SOURCE: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn activation_is_atomic_and_existing_reader_leases_remain_coherent() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write_corpus(
        "Alpha one",
        "body-one",
        "Target one",
        "target-one",
        b"asset-one",
    )?;
    let first_revision = fixture.commit_all("first generation")?;
    let (snapshot, index) = fixture.candidates(&first_revision, None, None, "first")?;
    let first_request = fixture.publish_request("g1", None, snapshot, index)?;
    let first = fixture
        .store
        .publish(&first_request, &AtomicBool::new(false), |_| {})?;
    assert_eq!(first.disposition, PublicationDisposition::Published);
    let old_lease = fixture.store.lease()?.context("first ready generation")?;
    assert_generation(
        &old_lease,
        "Alpha one",
        "body-one",
        "Target one",
        b"asset-one",
    )?;

    fixture.write_corpus(
        "Alpha two",
        "body-two",
        "Target two",
        "target-two",
        b"asset-two",
    )?;
    let second_revision = fixture.commit_all("second generation")?;
    let (snapshot, index) = fixture.candidates(
        &second_revision,
        Some(&first_revision),
        Some(old_lease.index_directory().to_owned()),
        "second",
    )?;
    let second_request = fixture.publish_request(
        "g2",
        Some(old_lease.binding().generation.clone()),
        snapshot,
        index,
    )?;

    let reached_activation = Arc::new(Barrier::new(2));
    let release_activation = Arc::new(Barrier::new(2));
    let publisher = fixture.store.clone();
    let reached_in_thread = Arc::clone(&reached_activation);
    let release_in_thread = Arc::clone(&release_activation);
    let publication = std::thread::spawn(move || {
        publisher.publish(&second_request, &AtomicBool::new(false), |stage| {
            if stage == PublicationStage::Activating {
                reached_in_thread.wait();
                release_in_thread.wait();
            }
        })
    });

    reached_activation.wait();
    let while_staging = fixture.store.lease()?.context("last good generation")?;
    assert_generation_concurrently(
        &while_staging,
        "Alpha one",
        "body-one",
        "Target one",
        b"asset-one",
    )?;
    assert_generation(
        &old_lease,
        "Alpha one",
        "body-one",
        "Target one",
        b"asset-one",
    )?;
    let freshness = fixture.store.freshness(
        Some(&second_revision),
        Some(InFlightGeneration::publishing(&second_revision)?),
    )?;
    assert_eq!(
        serde_json::to_value(&freshness)?,
        json!({
            "source": SOURCE,
            "fetched_revision": second_revision,
            "ready_revision": first_revision,
            "ready_generation": "g1",
            "in_flight": { "stage": "publishing", "revision": freshness.fetched_revision },
        })
    );

    release_activation.wait();
    let published = publication.join().expect("publication thread")?;
    assert_eq!(published.disposition, PublicationDisposition::Published);
    let new_lease = fixture.store.lease()?.context("second ready generation")?;
    assert_generation(
        &new_lease,
        "Alpha two",
        "body-two",
        "Target two",
        b"asset-two",
    )?;
    assert_generation(
        &old_lease,
        "Alpha one",
        "body-one",
        "Target one",
        b"asset-one",
    )?;
    let ready = fixture.store.freshness(Some(new_lease.revision()), None)?;
    assert_eq!(ready.ready_revision.as_deref(), Some(new_lease.revision()));
    assert_eq!(
        ready.ready_generation.as_ref().map(GenerationId::as_str),
        Some("g2")
    );
    assert!(ready.in_flight.is_none());
    Ok(())
}

#[test]
fn failures_at_each_sealing_boundary_leave_the_last_good_generation_active() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write_corpus(
        "Alpha one",
        "body-one",
        "Target one",
        "target-one",
        b"asset-one",
    )?;
    let first_revision = fixture.commit_all("first generation")?;
    let (snapshot, index) = fixture.candidates(&first_revision, None, None, "first")?;
    fixture.store.publish(
        &fixture.publish_request("g1", None, snapshot, index)?,
        &AtomicBool::new(false),
        |_| {},
    )?;
    let active_before = fs::read(fixture.root.path().join(ACTIVE_GENERATION_FILE))?;

    fixture.write_corpus(
        "Alpha two",
        "body-two",
        "Target two",
        "target-two",
        b"asset-two",
    )?;
    let second_revision = fixture.commit_all("second generation")?;
    for (index, boundary) in [
        PublicationStage::SourceSealed,
        PublicationStage::IndexSealed,
        PublicationStage::ManifestSealed,
        PublicationStage::Activating,
        PublicationStage::Retained,
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("failed-{index}");
        let (snapshot, staged) = fixture.candidates(&second_revision, None, None, &name)?;
        let request =
            fixture.publish_request(&name, Some(GenerationId::parse("g1")?), snapshot, staged)?;
        let cancelled = AtomicBool::new(false);
        let result = fixture.store.publish(&request, &cancelled, |stage| {
            if stage == boundary {
                cancelled.store(true, Ordering::Relaxed);
            }
        });
        assert_eq!(result.unwrap_err(), PublicationError::Cancelled);
        assert_eq!(
            fs::read(fixture.root.path().join(ACTIVE_GENERATION_FILE))?,
            active_before,
            "active manifest changed at {boundary:?}"
        );
        let active = fixture.store.lease()?.context("last good generation")?;
        assert_eq!(active.binding().generation.as_str(), "g1");
        assert_generation(&active, "Alpha one", "body-one", "Target one", b"asset-one")?;
    }
    Ok(())
}

#[test]
fn activation_is_compare_and_swap_and_successful_retries_are_idempotent() -> Result<()> {
    let fixture = Fixture::new()?;
    assert!(fixture.store.lease()?.is_none());
    let empty = fixture
        .store
        .freshness(None, Some(InFlightGeneration::Fetching))?;
    assert!(empty.ready_revision.is_none());
    assert!(empty.ready_generation.is_none());

    fixture.write_corpus(
        "Alpha one",
        "body-one",
        "Target one",
        "target-one",
        b"asset-one",
    )?;
    let revision = fixture.commit_all("first generation")?;
    let (snapshot, index) = fixture.candidates(&revision, None, None, "candidate")?;
    assert_eq!(
        PublishGenerationRequest::new(
            GenerationBinding::new(fixture.source.clone(), GenerationId::parse("..")?),
            None,
            snapshot.clone(),
            index.clone(),
        )
        .unwrap_err(),
        PublicationError::RequestRefused
    );
    let stale = fixture.publish_request(
        "g1",
        Some(GenerationId::parse("generation-that-never-existed")?),
        snapshot.clone(),
        index.clone(),
    )?;
    assert_eq!(
        fixture
            .store
            .publish(&stale, &AtomicBool::new(false), |_| {})
            .unwrap_err(),
        PublicationError::ActiveGenerationChanged
    );
    assert!(snapshot.content_root.exists());
    assert!(index.database.exists());

    let request = fixture.publish_request("g1", None, snapshot, index)?;
    let created = fixture
        .store
        .publish(&request, &AtomicBool::new(false), |_| {})?;
    let retried = fixture
        .store
        .publish(&request, &AtomicBool::new(false), |_| {})?;
    assert_eq!(created.disposition, PublicationDisposition::Published);
    assert_eq!(retried.disposition, PublicationDisposition::AlreadyActive);
    assert_eq!(created.generation.binding(), retried.generation.binding());

    let other = SourceId::parse("fedcba9876543210fedcba9876543210")?;
    assert_eq!(
        GenerationStore::open(other, fixture.root.path().to_owned()).unwrap_err(),
        PublicationError::OwnershipMismatch
    );
    Ok(())
}

fn assert_generation(
    lease: &GenerationLease,
    title: &str,
    body_marker: &str,
    destination_title: &str,
    asset: &[u8],
) -> Result<()> {
    let database = Database::open(lease.database())?;
    let alpha = database.node_from_id("alpha-id")?.context("alpha note")?;
    assert_eq!(alpha.title, title);
    assert_eq!(
        database
            .forward_links(&alpha.node_key, 20, false)?
            .first()
            .context("alpha forward link")?
            .destination_note
            .title,
        destination_title
    );

    let mut document = String::new();
    lease
        .open_content("notes/alpha.org")?
        .read_to_string(&mut document)?;
    assert!(document.contains(body_marker), "document was {document:?}");
    let mut actual_asset = Vec::new();
    lease
        .open_content("assets/figure.txt")?
        .read_to_end(&mut actual_asset)?;
    assert_eq!(actual_asset, asset);
    Ok(())
}

fn assert_generation_concurrently(
    lease: &GenerationLease,
    title: &str,
    body_marker: &str,
    destination_title: &str,
    asset: &[u8],
) -> Result<()> {
    let query_lease = lease.clone();
    let document_lease = lease.clone();
    let asset_lease = lease.clone();
    let (query, document, actual_asset) = std::thread::scope(|scope| -> Result<_> {
        let query = scope.spawn(move || -> Result<(String, String)> {
            let database = Database::open(query_lease.database())?;
            let alpha = database.node_from_id("alpha-id")?.context("alpha note")?;
            let linked = database
                .forward_links(&alpha.node_key, 20, false)?
                .first()
                .context("alpha forward link")?
                .destination_note
                .title
                .clone();
            Ok((alpha.title, linked))
        });
        let document = scope.spawn(move || -> Result<String> {
            let mut document = String::new();
            document_lease
                .open_content("notes/alpha.org")?
                .read_to_string(&mut document)?;
            Ok(document)
        });
        let actual_asset = scope.spawn(move || -> Result<Vec<u8>> {
            let mut actual_asset = Vec::new();
            asset_lease
                .open_content("assets/figure.txt")?
                .read_to_end(&mut actual_asset)?;
            Ok(actual_asset)
        });
        Ok((
            query.join().expect("query reader")?,
            document.join().expect("document reader")?,
            actual_asset.join().expect("asset reader")?,
        ))
    })?;
    assert_eq!(query.0, title);
    assert_eq!(query.1, destination_title);
    assert!(document.contains(body_marker), "document was {document:?}");
    assert_eq!(actual_asset, asset);
    Ok(())
}

struct Fixture {
    root: TempDir,
    repository: PathBuf,
    source: SourceId,
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
        let store = GenerationStore::initialize(source.clone(), root.path().to_owned())?;
        Ok(Self {
            root,
            repository,
            source,
            notes,
            store,
        })
    }

    fn write_corpus(
        &self,
        title: &str,
        body_marker: &str,
        target_title: &str,
        target_id: &str,
        asset: &[u8],
    ) -> Result<()> {
        self.write(
            "notes/alpha.org",
            &format!(
                "#+title: {title}\n:PROPERTIES:\n:ID: alpha-id\n:END:\n{body_marker}\n[[id:{target_id}]]\n"
            ),
        )?;
        self.write(
            "notes/target.org",
            &format!("#+title: {target_title}\n:PROPERTIES:\n:ID: {target_id}\n:END:\n"),
        )?;
        self.write_bytes("assets/figure.txt", asset)
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

    fn commit_all(&self, message: &str) -> Result<String> {
        run_git_in(&self.repository, ["add", "-A"])?;
        run_git_in(&self.repository, ["commit", "-m", message])?;
        Ok(git_stdout_in(&self.repository, ["rev-parse", "HEAD"])?
            .trim()
            .to_owned())
    }

    fn candidates(
        &self,
        revision: &str,
        previous: Option<&str>,
        base: Option<PathBuf>,
        name: &str,
    ) -> Result<(SnapshotOutcome, StagedIndexOutcome)> {
        let snapshot_request = SnapshotRequest::new(
            self.source.clone(),
            self.repository.clone(),
            revision,
            self.notes.clone(),
            self.store
                .candidates_root()
                .join(format!("snapshot-{name}")),
        )?;
        let snapshot = materialize(&snapshot_request, &AtomicBool::new(false), |_| {})?;
        let delta_request = match previous {
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
        let delta: DeltaOutcome = derive_delta(&delta_request, &AtomicBool::new(false))?;
        let index_request = StageIndexRequest::new(
            snapshot.clone(),
            delta,
            base,
            self.store.candidates_root().join(format!("index-{name}")),
        )?;
        let index = stage_index(&index_request, &AtomicBool::new(false), |_| {})?;
        Ok((snapshot, index))
    }

    fn publish_request(
        &self,
        generation: &str,
        expected_active: Option<GenerationId>,
        snapshot: SnapshotOutcome,
        index: StagedIndexOutcome,
    ) -> Result<PublishGenerationRequest> {
        Ok(PublishGenerationRequest::new(
            GenerationBinding::new(self.source.clone(), GenerationId::parse(generation)?),
            expected_active,
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
