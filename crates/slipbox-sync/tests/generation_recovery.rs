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
use slipbox_git::{
    DeltaOutcome, DeltaRequest, SnapshotOutcome, SnapshotRequest, derive_delta, inspect_snapshot,
    materialize,
};
use slipbox_store::Database;
use slipbox_sync::{
    GENERATION_INDEX_DIRECTORY, GENERATIONS_DIRECTORY, GenerationLease, GenerationStore,
    PublicationError, PublicationStage, PublishGenerationRequest, RecoveryAction,
    RecoveryDisposition, RecoveryNoticeKind, RecoveryPolicy, STAGED_INDEX_MANIFEST_FILE,
    StageIndexRequest, StagedIndexOutcome, inspect_staged_index, stage_index,
};
use tempfile::TempDir;

const SOURCE: &str = "0123456789abcdef0123456789abcdef";
const CRASH_ROOT: &str = "SLIPBOX_RECOVERY_CRASH_ROOT";
const CRASH_STAGE: &str = "SLIPBOX_RECOVERY_CRASH_STAGE";
const CRASH_GENERATION: &str = "SLIPBOX_RECOVERY_CRASH_GENERATION";
const CRASH_EXPECTED: &str = "SLIPBOX_RECOVERY_CRASH_EXPECTED";
const CRASH_EXIT: i32 = 86;

#[test]
fn crash_worker() -> Result<()> {
    let Ok(root) = std::env::var(CRASH_ROOT) else {
        return Ok(());
    };
    let root = PathBuf::from(root);
    let stage = parse_stage(&std::env::var(CRASH_STAGE)?)?;
    let generation = std::env::var(CRASH_GENERATION)?;
    let expected = std::env::var(CRASH_EXPECTED)?;
    let source = SourceId::parse(SOURCE)?;
    let store = GenerationStore::open(source.clone(), root.clone())?;
    let snapshot = inspect_snapshot(&root.join("candidates/crash-snapshot"))?;
    let index = inspect_staged_index(&root.join("candidates/crash-index"))?;
    let request = PublishGenerationRequest::new(
        fixture_record(source.clone())?,
        GenerationBinding::new(source, GenerationId::parse(&generation)?),
        (!expected.is_empty())
            .then(|| GenerationId::parse(&expected))
            .transpose()?,
        snapshot,
        index,
    )?;
    let result = store.publish(&request, &AtomicBool::new(false), |current| {
        if current == stage {
            std::process::exit(CRASH_EXIT);
        }
    });
    anyhow::bail!("publication did not exit at {stage:?}: {result:?}")
}

#[test]
fn every_publication_crash_point_recovers_without_exposing_partial_content() -> Result<()> {
    for stage in [
        PublicationStage::Preparing,
        PublicationStage::SourceSealed,
        PublicationStage::IndexSealed,
        PublicationStage::ManifestSealed,
        PublicationStage::Activating,
        PublicationStage::Retained,
        PublicationStage::Complete,
    ] {
        let fixture = Fixture::new()?;
        fixture.write_corpus("Alpha one", "body-one", "target-one", b"asset-one")?;
        let first_revision = fixture.commit_all("first generation")?;
        let (snapshot, index) = fixture.candidates(&first_revision, None, None, "first")?;
        fixture.publish("g1", None, snapshot, index)?;
        let first = fixture.store.lease()?.context("first generation")?;

        fixture.write_corpus("Alpha two", "body-two", "target-two", b"asset-two")?;
        let second_revision = fixture.commit_all("second generation")?;
        let (snapshot, index) = fixture.candidates(
            &second_revision,
            Some(&first_revision),
            Some(first.index_directory().to_owned()),
            "second",
        )?;
        fs::rename(
            snapshot
                .content_root
                .parent()
                .context("snapshot directory")?,
            fixture.store.candidates_root().join("crash-snapshot"),
        )?;
        fs::rename(
            index.database.parent().context("index directory")?,
            fixture.store.candidates_root().join("crash-index"),
        )?;
        fixture.run_crash_worker(stage, "g2", "g1")?;

        let restarted = GenerationStore::open(fixture.source.clone(), fixture.root.path().into())?;
        let recovered = restarted.recover(RecoveryPolicy::default())?;
        let ready = recovered.ready.context("a readable generation")?;
        if matches!(
            stage,
            PublicationStage::ManifestSealed
                | PublicationStage::Activating
                | PublicationStage::Retained
                | PublicationStage::Complete
        ) {
            assert_eq!(ready.binding().generation.as_str(), "g2", "{stage:?}");
            assert_generation(&ready, "Alpha two", "body-two", b"asset-two")?;
        } else {
            assert_eq!(ready.binding().generation.as_str(), "g1", "{stage:?}");
            assert_generation(&ready, "Alpha one", "body-one", b"asset-one")?;
            assert!(
                recovered.notices.iter().any(|notice| {
                    notice.kind == RecoveryNoticeKind::InterruptedPublication
                        && notice.action == RecoveryAction::RetryImport
                }),
                "{stage:?}: {:?}",
                recovered.notices
            );
        }
        assert!(fs::read_dir(restarted.candidates_root())?.next().is_none());
    }
    Ok(())
}

#[test]
fn a_first_import_is_ready_only_after_a_complete_generation_can_be_verified() -> Result<()> {
    for stage in [
        PublicationStage::Preparing,
        PublicationStage::SourceSealed,
        PublicationStage::IndexSealed,
        PublicationStage::ManifestSealed,
        PublicationStage::Activating,
        PublicationStage::Complete,
    ] {
        let fixture = Fixture::new()?;
        fixture.write_corpus("Alpha one", "body-one", "target-one", b"asset-one")?;
        let revision = fixture.commit_all("first generation")?;
        let (snapshot, index) = fixture.candidates(&revision, None, None, "first")?;
        fs::rename(
            snapshot
                .content_root
                .parent()
                .context("snapshot directory")?,
            fixture.store.candidates_root().join("crash-snapshot"),
        )?;
        fs::rename(
            index.database.parent().context("index directory")?,
            fixture.store.candidates_root().join("crash-index"),
        )?;
        fixture.run_crash_worker(stage, "g1", "")?;

        let restarted = GenerationStore::open(fixture.source.clone(), fixture.root.path().into())?;
        let recovered = restarted.recover(RecoveryPolicy::default())?;
        if matches!(
            stage,
            PublicationStage::ManifestSealed
                | PublicationStage::Activating
                | PublicationStage::Complete
        ) {
            let ready = recovered.ready.context("sealed first generation")?;
            assert_generation(&ready, "Alpha one", "body-one", b"asset-one")?;
            assert_eq!(
                recovered.disposition,
                if stage == PublicationStage::Complete {
                    RecoveryDisposition::ReadyUnchanged
                } else {
                    RecoveryDisposition::PublicationCompleted
                }
            );
        } else {
            assert!(recovered.ready.is_none(), "{stage:?} became readable");
            assert_eq!(
                recovered.disposition,
                RecoveryDisposition::NoReadyGeneration
            );
        }
    }
    Ok(())
}

#[test]
fn corruption_restores_the_retained_generation_and_schema_failures_request_a_rebuild() -> Result<()>
{
    let fixture = Fixture::new()?;
    fixture.write_corpus("Alpha one", "body-one", "target-one", b"asset-one")?;
    let first_revision = fixture.commit_all("first generation")?;
    let (snapshot, index) = fixture.candidates(&first_revision, None, None, "first")?;
    fixture.publish("g1", None, snapshot, index)?;
    let first = fixture.store.lease()?.context("first generation")?;

    fixture.write_corpus("Alpha two", "body-two", "target-two", b"asset-two")?;
    let second_revision = fixture.commit_all("second generation")?;
    let (snapshot, index) = fixture.candidates(
        &second_revision,
        Some(&first_revision),
        Some(first.index_directory().to_owned()),
        "second",
    )?;
    fixture.publish("g2", Some(GenerationId::parse("g1")?), snapshot, index)?;
    drop(first);
    fs::write(
        fixture
            .root
            .path()
            .join("generations/g2/index/index.sqlite3"),
        b"corrupt database",
    )?;

    let restarted = GenerationStore::open(fixture.source.clone(), fixture.root.path().into())?;
    assert_eq!(
        restarted.lease().unwrap_err(),
        PublicationError::StoreUnavailable
    );
    let recovered = restarted.recover(RecoveryPolicy::default())?;
    assert_eq!(recovered.disposition, RecoveryDisposition::RetainedRestored);
    let ready = recovered.ready.context("retained generation")?;
    assert_eq!(ready.binding().generation.as_str(), "g1");
    assert_generation(&ready, "Alpha one", "body-one", b"asset-one")?;
    assert!(recovered.notices.iter().any(|notice| {
        notice.kind == RecoveryNoticeKind::ActiveGenerationCorrupt
            && notice.action == RecoveryAction::RefetchSource
    }));

    let schema_fixture = Fixture::new()?;
    schema_fixture.write_corpus("Alpha one", "body-one", "target-one", b"asset-one")?;
    let first_revision = schema_fixture.commit_all("first generation")?;
    let (snapshot, index) = schema_fixture.candidates(&first_revision, None, None, "first")?;
    schema_fixture.publish("g1", None, snapshot, index)?;
    let first = schema_fixture.store.lease()?.context("first generation")?;
    schema_fixture.write_corpus("Alpha two", "body-two", "target-two", b"asset-two")?;
    let second_revision = schema_fixture.commit_all("second generation")?;
    let (snapshot, index) = schema_fixture.candidates(
        &second_revision,
        Some(&first_revision),
        Some(first.index_directory().to_owned()),
        "second",
    )?;
    let request =
        schema_fixture.request("g2", Some(GenerationId::parse("g1")?), snapshot, index)?;
    let cancelled = AtomicBool::new(false);
    assert_eq!(
        schema_fixture
            .store
            .publish(&request, &cancelled, |stage| {
                if stage == PublicationStage::Activating {
                    cancelled.store(true, Ordering::Relaxed);
                }
            })
            .unwrap_err(),
        PublicationError::Cancelled
    );
    let manifest_path = schema_fixture
        .root
        .path()
        .join(GENERATIONS_DIRECTORY)
        .join("g2")
        .join(GENERATION_INDEX_DIRECTORY)
        .join(STAGED_INDEX_MANIFEST_FILE);
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    manifest["index_schema"] = serde_json::json!(0);
    fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;

    let recovered = schema_fixture.store.recover(RecoveryPolicy::default())?;
    assert_eq!(
        recovered
            .ready
            .as_ref()
            .context("last good generation")?
            .binding()
            .generation
            .as_str(),
        "g1"
    );
    assert!(recovered.notices.iter().any(|notice| {
        notice.kind == RecoveryNoticeKind::IndexRebuildRequired
            && notice.action == RecoveryAction::RebuildIndex
            && notice.generation.as_ref().map(GenerationId::as_str) == Some("g2")
    }));
    assert!(
        !schema_fixture
            .root
            .path()
            .join(GENERATIONS_DIRECTORY)
            .join("g2")
            .exists()
    );
    Ok(())
}

#[test]
fn pointer_corruption_repairs_or_preserves_the_last_good_generations() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write_corpus("Alpha one", "body-one", "target-one", b"asset-one")?;
    let first_revision = fixture.commit_all("first generation")?;
    let (snapshot, index) = fixture.candidates(&first_revision, None, None, "first")?;
    fixture.publish("g1", None, snapshot, index)?;
    let first = fixture.store.lease()?.context("first generation")?;

    fixture.write_corpus("Alpha two", "body-two", "target-two", b"asset-two")?;
    let second_revision = fixture.commit_all("second generation")?;
    let (snapshot, index) = fixture.candidates(
        &second_revision,
        Some(&first_revision),
        Some(first.index_directory().to_owned()),
        "second",
    )?;
    fixture.publish("g2", Some(GenerationId::parse("g1")?), snapshot, index)?;
    drop(first);

    let retained = fixture.root.path().join("retained-generation.json");
    fs::write(&retained, b"not json")?;
    let repaired = fixture.store.recover(RecoveryPolicy::default())?;
    assert_eq!(
        repaired
            .ready
            .as_ref()
            .context("active generation")?
            .binding()
            .generation
            .as_str(),
        "g2"
    );
    let retained_manifest: serde_json::Value = serde_json::from_slice(&fs::read(&retained)?)?;
    assert_eq!(retained_manifest["binding"]["generation"], "g1");
    assert!(fixture.root.path().join("generations/g1").exists());

    fs::write(
        fixture.root.path().join("active-generation.json"),
        b"not json",
    )?;
    let restored = fixture.store.recover(RecoveryPolicy::default())?;
    assert_eq!(restored.disposition, RecoveryDisposition::RetainedRestored);
    assert_eq!(
        restored
            .ready
            .as_ref()
            .context("retained generation")?
            .binding()
            .generation
            .as_str(),
        "g1"
    );
    assert!(fixture.root.path().join("generations/g2").exists());
    assert!(restored.notices.iter().any(|notice| {
        notice.kind == RecoveryNoticeKind::ActiveGenerationCorrupt
            && notice.action == RecoveryAction::RefetchSource
    }));
    Ok(())
}

#[test]
fn cleanup_defers_live_readers_then_bounds_storage_and_reports_pressure() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write_corpus("Alpha one", "body-one", "target-one", b"asset-one")?;
    let first_revision = fixture.commit_all("first generation")?;
    let (snapshot, index) = fixture.candidates(&first_revision, None, None, "first")?;
    fixture.publish("g1", None, snapshot, index)?;
    let held = fixture.store.lease()?.context("held first generation")?;
    let mut held_asset = held.open_content("assets/figure.txt")?;

    fixture.write_corpus("Alpha two", "body-two", "target-two", b"asset-two")?;
    let second_revision = fixture.commit_all("second generation")?;
    let (snapshot, index) = fixture.candidates(
        &second_revision,
        Some(&first_revision),
        Some(held.index_directory().to_owned()),
        "second",
    )?;
    fixture.publish("g2", Some(GenerationId::parse("g1")?), snapshot, index)?;
    let second = fixture.store.lease()?.context("second generation")?;

    fixture.write_corpus("Alpha three", "body-three", "target-three", b"asset-three")?;
    let third_revision = fixture.commit_all("third generation")?;
    let (snapshot, index) = fixture.candidates(
        &third_revision,
        Some(&second_revision),
        Some(second.index_directory().to_owned()),
        "third",
    )?;
    fixture.publish("g3", Some(GenerationId::parse("g2")?), snapshot, index)?;
    drop(second);

    let cleaner = GenerationStore::open(fixture.source.clone(), fixture.root.path().into())?;
    let pressured = cleaner.recover(RecoveryPolicy::new(u64::MAX))?;
    assert_eq!(pressured.deferred_leased_generations, 1);
    assert!(fixture.root.path().join("generations/g1").exists());
    assert!(pressured.notices.iter().any(|notice| {
        notice.kind == RecoveryNoticeKind::CleanupDeferred
            && notice.action == RecoveryAction::ReleaseReaders
            && notice.generation.as_ref().map(GenerationId::as_str) == Some("g1")
    }));
    assert!(pressured.notices.iter().any(|notice| {
        notice.kind == RecoveryNoticeKind::StoragePressure
            && notice.action == RecoveryAction::FreeStorage
            && notice.available_bytes.is_some()
            && notice.required_bytes == Some(u64::MAX)
    }));
    let pressure = pressured
        .notices
        .iter()
        .find(|notice| notice.kind == RecoveryNoticeKind::StoragePressure)
        .context("storage pressure notice")?;
    assert_eq!(
        serde_json::to_value(pressure)?,
        serde_json::json!({
            "kind": "storage-pressure",
            "action": "free-storage",
            "generation": "g3",
            "available_bytes": pressure.available_bytes,
            "required_bytes": u64::MAX,
        })
    );
    assert_generation(&held, "Alpha one", "body-one", b"asset-one")?;

    drop(held);
    let asset_held = cleaner.recover(RecoveryPolicy::default())?;
    assert_eq!(asset_held.deferred_leased_generations, 1);
    assert!(fixture.root.path().join("generations/g1").exists());
    let mut asset = Vec::new();
    held_asset.read_to_end(&mut asset)?;
    assert_eq!(asset, b"asset-one");
    drop(held_asset);

    let cleaned = cleaner.recover(RecoveryPolicy::default())?;
    assert_eq!(cleaned.reclaimed_generations, 1);
    assert!(!fixture.root.path().join("generations/g1").exists());
    assert_eq!(
        fs::read_dir(fixture.root.path().join(GENERATIONS_DIRECTORY))?.count(),
        2
    );
    assert_generation(
        cleaned.ready.as_ref().context("third generation")?,
        "Alpha three",
        "body-three",
        b"asset-three",
    )?;
    Ok(())
}

fn parse_stage(value: &str) -> Result<PublicationStage> {
    match value {
        "preparing" => Ok(PublicationStage::Preparing),
        "source-sealed" => Ok(PublicationStage::SourceSealed),
        "index-sealed" => Ok(PublicationStage::IndexSealed),
        "manifest-sealed" => Ok(PublicationStage::ManifestSealed),
        "activating" => Ok(PublicationStage::Activating),
        "retained" => Ok(PublicationStage::Retained),
        "complete" => Ok(PublicationStage::Complete),
        _ => anyhow::bail!("unknown crash stage"),
    }
}

fn stage_name(stage: PublicationStage) -> &'static str {
    match stage {
        PublicationStage::Preparing => "preparing",
        PublicationStage::SourceSealed => "source-sealed",
        PublicationStage::IndexSealed => "index-sealed",
        PublicationStage::ManifestSealed => "manifest-sealed",
        PublicationStage::Activating => "activating",
        PublicationStage::Retained => "retained",
        PublicationStage::Complete => "complete",
    }
}

fn assert_generation(lease: &GenerationLease, title: &str, body: &str, asset: &[u8]) -> Result<()> {
    let database = Database::open(lease.database())?;
    assert_eq!(
        database
            .node_from_id("alpha-id")?
            .context("alpha note")?
            .title,
        title
    );
    let mut document = String::new();
    lease
        .open_content("notes/alpha.org")?
        .read_to_string(&mut document)?;
    assert!(document.contains(body), "document was {document:?}");
    let mut actual_asset = Vec::new();
    lease
        .open_content("assets/figure.txt")?
        .read_to_end(&mut actual_asset)?;
    assert_eq!(actual_asset, asset);
    Ok(())
}

struct Fixture {
    root: TempDir,
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
        let record = fixture_record(source.clone())?;
        let store = GenerationStore::initialize(source.clone(), root.path().to_owned())?;
        Ok(Self {
            root,
            repository,
            source,
            record,
            notes,
            store,
        })
    }

    fn write_corpus(&self, title: &str, body: &str, target: &str, asset: &[u8]) -> Result<()> {
        self.write(
            "notes/alpha.org",
            &format!(
                "#+title: {title}\n:PROPERTIES:\n:ID: alpha-id\n:END:\n{body}\n[[id:{target}]]\n"
            ),
        )?;
        self.write(
            "notes/target.org",
            &format!("#+title: Target\n:PROPERTIES:\n:ID: {target}\n:END:\n"),
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
        let delta = match previous {
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
        let delta: DeltaOutcome = derive_delta(&delta, &AtomicBool::new(false))?;
        let index = stage_index(
            &StageIndexRequest::new(
                snapshot.clone(),
                delta,
                base,
                self.store.candidates_root().join(format!("index-{name}")),
            )?,
            &AtomicBool::new(false),
            |_| {},
        )?;
        Ok((snapshot, index))
    }

    fn request(
        &self,
        generation: &str,
        expected: Option<GenerationId>,
        snapshot: SnapshotOutcome,
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

    fn publish(
        &self,
        generation: &str,
        expected: Option<GenerationId>,
        snapshot: SnapshotOutcome,
        index: StagedIndexOutcome,
    ) -> Result<()> {
        self.store.publish(
            &self.request(generation, expected, snapshot, index)?,
            &AtomicBool::new(false),
            |_| {},
        )?;
        Ok(())
    }

    fn run_crash_worker(
        &self,
        stage: PublicationStage,
        generation: &str,
        expected: &str,
    ) -> Result<()> {
        let output = Command::new(std::env::current_exe()?)
            .args(["--exact", "crash_worker", "--nocapture"])
            .env(CRASH_ROOT, self.root.path())
            .env(CRASH_STAGE, stage_name(stage))
            .env(CRASH_GENERATION, generation)
            .env(CRASH_EXPECTED, expected)
            .output()?;
        anyhow::ensure!(
            output.status.code() == Some(CRASH_EXIT),
            "crash worker failed at {stage:?}: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}

fn fixture_record(source: SourceId) -> Result<SourceRecord> {
    Ok(SourceRecord::new(SourceConfiguration {
        id: source,
        display_name: SourceDisplayName::parse("Fixture")?,
        provider: SourceProvider::GenericHttps,
        visibility: SourceVisibility::Public,
        provider_repository_id: None,
        account: None,
        remote: RemoteUrl::parse("https://example.invalid/fixture.git")?,
        branch: GitBranch::parse("main")?,
        notes_folder: NotesFolder::parse("notes")?,
        credential: None,
    })?)
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
