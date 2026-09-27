use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use slipbox_core::{NotesFolder, SourceId};
use tempfile::TempDir;

use super::*;

const SOURCE: &str = "0123456789abcdef0123456789abcdef";
const OTHER_SOURCE: &str = "fedcba9876543210fedcba9876543210";

#[test]
fn materializes_notes_and_repository_assets_as_a_private_owned_snapshot() {
    let fixture = Fixture::new();
    fixture.write("notes/note.org", b"* Note\n");
    fixture.write("notes/caption.txt", b"caption\n");
    fixture.write("assets/pixel.png", b"synthetic png bytes");
    fixture.write("shared-notes/linked.org", b"* Linked through a tree\n");
    fixture.write("README.md", b"not source material\n");
    fixture.write("script.sh", b"#!/bin/sh\nexit 99\n");
    #[cfg(unix)]
    std::os::unix::fs::symlink("../assets/pixel.png", fixture.seed.join("notes/linked.png"))
        .expect("safe fixture symlink");
    #[cfg(unix)]
    std::os::unix::fs::symlink("../shared-notes", fixture.seed.join("notes/shared"))
        .expect("safe fixture directory symlink");
    fixture.commit_all("snapshot");
    run_git_in(
        &fixture.seed,
        ["update-index", "--chmod=+x", "notes/caption.txt"],
    );
    run_git_in(&fixture.seed, ["commit", "-m", "executable source mode"]);
    run_git_in(&fixture.seed, ["push", "origin", "main"]);

    let destination = fixture.root.path().join("snapshots/revision-1");
    let request = fixture.request(&destination, "notes");
    let cancelled = AtomicBool::new(false);
    let mut stages = Vec::new();
    let outcome = materialize(&request, &cancelled, |progress| {
        stages.push(progress.stage);
    })
    .expect("materialized snapshot");

    assert_eq!(outcome.disposition, SnapshotDisposition::Created);
    assert_eq!(outcome.source.as_str(), SOURCE);
    assert_eq!(outcome.revision, fixture.revision());
    #[cfg(unix)]
    assert_eq!(outcome.org_files, 2);
    #[cfg(not(unix))]
    assert_eq!(outcome.org_files, 1);
    #[cfg(unix)]
    assert_eq!(outcome.assets, 3);
    #[cfg(not(unix))]
    assert_eq!(outcome.assets, 2);
    #[cfg(unix)]
    assert_eq!(outcome.files, 5);
    #[cfg(not(unix))]
    assert_eq!(outcome.files, 3);
    assert!(outcome.diagnostics.is_empty());
    assert_eq!(
        fs::read_to_string(destination.join("content/notes/note.org")).expect("note"),
        "* Note\n"
    );
    assert_eq!(
        fs::read(destination.join("content/notes/linked.png")).expect("linked asset"),
        b"synthetic png bytes"
    );
    assert!(destination.join("content/assets/pixel.png").is_file());
    #[cfg(unix)]
    assert_eq!(
        fs::read_to_string(destination.join("content/notes/shared/linked.org"))
            .expect("Org through a confined directory symlink"),
        "* Linked through a tree\n"
    );
    assert!(!destination.join("content/README.md").exists());
    assert!(!destination.join("content/script.sh").exists());
    assert!(
        !fs::symlink_metadata(destination.join("content/notes/linked.png"))
            .expect("linked asset metadata")
            .file_type()
            .is_symlink()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(destination.join("content/notes/caption.txt"))
                .expect("caption metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    assert_eq!(stages.first(), Some(&SnapshotProgressStage::Preparing));
    assert_eq!(stages.get(1), Some(&SnapshotProgressStage::Inspecting));
    assert_eq!(stages.last(), Some(&SnapshotProgressStage::Complete));
    assert_eq!(
        stages
            .iter()
            .filter(|stage| **stage == SnapshotProgressStage::Writing)
            .count(),
        outcome.files as usize
    );

    let existing = materialize(&request, &cancelled, |_| {}).expect("idempotent retry");
    assert_eq!(existing.disposition, SnapshotDisposition::Existing);
    let foreign = SnapshotRequest::new(
        SourceId::parse(OTHER_SOURCE).expect("other source"),
        fixture.remote.clone(),
        &fixture.revision(),
        NotesFolder::parse("notes").expect("notes folder"),
        destination,
    )
    .expect("foreign request");
    assert_eq!(
        materialize(&foreign, &cancelled, |_| {}),
        Err(SnapshotError::OwnershipMismatch)
    );
}

#[test]
fn reports_unsupported_inputs_without_running_repository_programs() {
    let fixture = Fixture::new();
    fixture.write("notes/ok.org", b"* O\n");
    fixture.write("notes/invalid.org", &[0xff, 0xfe]);
    fixture.write("notes/large.org", b"12345");
    fixture.write("notes/secret.org.gpg", b"encrypted envelope");
    fixture.write(
        "assets/lfs.png",
        b"version https://git-lfs.github.com/spec/v1\noid sha256:0123456789abcdef\nsize 9\n",
    );
    fixture.write("assets/ok.webp", b"webp");
    fixture.write(".gitattributes", b"*.png filter=evil -text\n");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc/passwd", fixture.seed.join("notes/escape.png"))
            .expect("escaping fixture symlink");
        std::os::unix::fs::symlink(
            "../assets/missing.png",
            fixture.seed.join("notes/missing.png"),
        )
        .expect("missing fixture symlink");
        std::os::unix::fs::symlink("cycle", fixture.seed.join("notes/cycle"))
            .expect("cyclic fixture symlink");
    }
    fixture.commit_all("unsupported inputs");
    let base = fixture.revision();
    let cache = format!("160000,{base},notes/vendor");
    run_git_in(
        &fixture.seed,
        ["update-index", "--add", "--cacheinfo", cache.as_str()],
    );
    run_git_in(&fixture.seed, ["commit", "-m", "submodule entry"]);
    run_git_in(&fixture.seed, ["push", "origin", "main"]);
    let marker = fixture.root.path().join("filter-ran");
    let command = format!("touch {}", marker.display());
    run_git([
        "--git-dir",
        text(&fixture.remote),
        "config",
        "filter.evil.smudge",
        command.as_str(),
    ]);

    let destination = fixture.root.path().join("snapshot");
    let limits = SnapshotLimits::new(100, 4, 1024, 4096).expect("test limits");
    let request = fixture.request(&destination, "notes").with_limits(limits);
    let outcome = materialize(&request, &AtomicBool::new(false), |_| {})
        .expect("snapshot with explicit diagnostics");
    let diagnostics = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.path.as_str(), diagnostic.reason))
        .collect::<BTreeSet<_>>();

    for expected in [
        (".gitattributes", SnapshotDiagnosticReason::ExternalFilter),
        ("assets/lfs.png", SnapshotDiagnosticReason::LfsPointer),
        (
            "notes/invalid.org",
            SnapshotDiagnosticReason::UnsupportedEncoding,
        ),
        ("notes/large.org", SnapshotDiagnosticReason::OversizedInput),
        (
            "notes/secret.org.gpg",
            SnapshotDiagnosticReason::EncryptedOrg,
        ),
        ("notes/vendor", SnapshotDiagnosticReason::Submodule),
    ] {
        assert!(diagnostics.contains(&expected), "missing {expected:?}");
    }
    #[cfg(unix)]
    for expected in [
        ("notes/escape.png", SnapshotDiagnosticReason::SymlinkEscapes),
        (
            "notes/missing.png",
            SnapshotDiagnosticReason::SymlinkUnavailable,
        ),
        ("notes/cycle", SnapshotDiagnosticReason::SymlinkCycle),
    ] {
        assert!(diagnostics.contains(&expected), "missing {expected:?}");
    }
    assert_eq!(
        fs::read_to_string(destination.join("content/notes/ok.org")).expect("eligible Org"),
        "* O\n"
    );
    assert!(destination.join("content/assets/ok.webp").is_file());
    assert!(!destination.join("content/assets/lfs.png").exists());
    assert!(!destination.join("content/notes/invalid.org").exists());
    assert!(!marker.exists(), "a repository filter was executed");
}

#[test]
fn cancellation_and_budget_failures_leave_last_good_content_untouched() {
    let fixture = Fixture::new();
    fixture.write("notes/note.org", b"* N\n");
    fixture.write("assets/pixel.png", b"asset");
    fixture.commit_all("budget fixture");

    let published = fixture.root.path().join("published");
    let published_request = fixture.request(&published, "notes");
    materialize(&published_request, &AtomicBool::new(false), |_| {}).expect("last good snapshot");
    let before = fs::read(published.join("content/notes/note.org")).expect("last good note");

    let candidate = fixture.root.path().join("over-budget");
    let limits = SnapshotLimits::new(100, 1024, 1024, 5).expect("small total budget");
    let request = fixture.request(&candidate, "notes").with_limits(limits);
    assert_eq!(
        materialize(&request, &AtomicBool::new(false), |_| {}),
        Err(SnapshotError::InputExhausted)
    );
    assert!(!candidate.exists());
    assert_eq!(
        fs::read(published.join("content/notes/note.org")).expect("preserved note"),
        before
    );

    let entry_limited = fixture.root.path().join("entry-limited");
    let limits = SnapshotLimits::new(1, 1024, 1024, 4096).expect("entry limit");
    let request = fixture.request(&entry_limited, "notes").with_limits(limits);
    assert_eq!(
        materialize(&request, &AtomicBool::new(false), |_| {}),
        Err(SnapshotError::InputExhausted)
    );
    assert!(!entry_limited.exists());

    let cancelled_destination = fixture.root.path().join("cancelled");
    let cancelled_request = fixture.request(&cancelled_destination, "notes");
    let cancelled = AtomicBool::new(false);
    assert_eq!(
        materialize(&cancelled_request, &cancelled, |progress| {
            if progress.stage == SnapshotProgressStage::Inspecting {
                cancelled.store(true, Ordering::Relaxed);
            }
        }),
        Err(SnapshotError::Cancelled)
    );
    assert!(!cancelled_destination.exists());
    assert_eq!(
        fs::read(published.join("content/notes/note.org")).expect("preserved note"),
        before
    );
}

#[test]
fn revisions_notes_roots_destinations_and_tree_paths_are_closed() {
    let fixture = Fixture::new();
    fixture.write("notes/note.org", b"* Note\n");
    fixture.commit_all("validation fixture");
    let destination = fixture.root.path().join("snapshot");

    assert_eq!(
        SnapshotRequest::new(
            source(),
            fixture.remote.clone(),
            "HEAD",
            NotesFolder::root(),
            destination.clone(),
        )
        .expect_err("symbolic revisions are refused"),
        SnapshotError::RevisionRefused
    );
    assert_eq!(
        SnapshotRequest::new(
            source(),
            fixture.remote.clone(),
            &fixture.revision(),
            NotesFolder::root(),
            fixture.remote.join("snapshot"),
        )
        .expect_err("a snapshot cannot overlap its object store"),
        SnapshotError::DestinationRefused
    );

    let foreign_root = tempfile::tempdir().expect("foreign source root");
    assert_eq!(
        SnapshotRequest::new(
            source(),
            fixture.remote.clone(),
            &fixture.revision(),
            NotesFolder::root(),
            foreign_root.path().join("snapshot"),
        )
        .expect_err("a snapshot cannot cross its source storage boundary"),
        SnapshotError::DestinationRefused
    );
    assert!(!foreign_root.path().join("snapshot").exists());

    let unavailable = SnapshotRequest::new(
        source(),
        fixture.remote.clone(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        NotesFolder::root(),
        destination.clone(),
    )
    .expect("well-formed absent revision");
    assert_eq!(
        materialize(&unavailable, &AtomicBool::new(false), |_| {}),
        Err(SnapshotError::RevisionUnavailable)
    );
    assert!(!destination.exists());

    let missing_notes = SnapshotRequest::new(
        source(),
        fixture.remote.clone(),
        &fixture.revision(),
        NotesFolder::parse("missing").expect("missing folder spelling"),
        destination.clone(),
    )
    .expect("missing notes request");
    assert_eq!(
        materialize(&missing_notes, &AtomicBool::new(false), |_| {}),
        Err(SnapshotError::NotesFolderUnavailable)
    );
    assert!(!destination.exists());

    #[cfg(unix)]
    {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let hostile = Fixture::new();
        let name = OsString::from_vec(vec![b'n', 0xff, b'.', b'o', b'r', b'g']);
        fs::write(hostile.seed.join(name), b"* hostile\n").expect("non-UTF-8 fixture");
        hostile.commit_all("hostile path");
        let hostile_destination = hostile.root.path().join("snapshot");
        let request = hostile.request(&hostile_destination, "");
        assert_eq!(
            materialize(&request, &AtomicBool::new(false), |_| {}),
            Err(SnapshotError::UnsafePath)
        );
        assert!(!hostile_destination.exists());
    }
}

#[test]
fn a_diagnostic_report_never_truncates_an_incomplete_source() {
    let fixture = Fixture::new();
    for index in 0..=MAX_SNAPSHOT_DIAGNOSTICS {
        fixture.write(
            &format!("notes/secret-{index:03}.org.gpg"),
            b"encrypted envelope",
        );
    }
    fixture.commit_all("diagnostic bound");
    let destination = fixture.root.path().join("snapshot");

    assert_eq!(
        materialize(
            &fixture.request(&destination, "notes"),
            &AtomicBool::new(false),
            |_| {},
        ),
        Err(SnapshotError::DiagnosticsExhausted)
    );
    assert!(!destination.exists());
}

#[test]
fn a_maximum_diagnostic_manifest_remains_an_idempotent_snapshot() {
    let fixture = Fixture::new();
    let component = "a".repeat(249);
    for index in 0..MAX_SNAPSHOT_DIAGNOSTICS {
        fixture.write(
            &format!("{component}/{component}/{component}/{component}/secret-{index:03}.org.gpg"),
            b"encrypted envelope",
        );
    }
    fixture.commit_all("maximum diagnostic manifest");
    let destination = fixture.root.path().join("snapshot");
    let request = fixture.request(&destination, "");

    let created = materialize(&request, &AtomicBool::new(false), |_| {})
        .expect("maximum diagnostic snapshot");
    assert_eq!(created.disposition, SnapshotDisposition::Created);
    assert_eq!(created.diagnostics.len(), MAX_SNAPSHOT_DIAGNOSTICS);
    let manifest_bytes = fs::metadata(destination.join(SNAPSHOT_MANIFEST_FILE))
        .expect("manifest metadata")
        .len();
    assert!(manifest_bytes > 64 * 1024);
    assert!(manifest_bytes <= MAX_SNAPSHOT_MANIFEST_BYTES);

    let existing =
        materialize(&request, &AtomicBool::new(false), |_| {}).expect("maximum diagnostic retry");
    assert_eq!(existing.disposition, SnapshotDisposition::Existing);
    assert_eq!(existing.diagnostics, created.diagnostics);
}

#[test]
fn an_existing_snapshot_with_unbounded_manifest_counts_is_refused_unchanged() {
    let fixture = Fixture::new();
    fixture.write("notes/note.org", b"* Note\n");
    fixture.commit_all("manifest bounds");
    let destination = fixture.root.path().join("snapshot");
    let request = fixture.request(&destination, "notes");
    materialize(&request, &AtomicBool::new(false), |_| {}).expect("initial snapshot");
    let manifest_path = destination.join(SNAPSHOT_MANIFEST_FILE);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest bytes"))
            .expect("manifest document");
    manifest["entries"] = serde_json::json!(MAX_SNAPSHOT_ENTRIES + 1);
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("tampered manifest document"),
    )
    .expect("tampered manifest");

    assert_eq!(
        materialize(&request, &AtomicBool::new(false), |_| {}),
        Err(SnapshotError::DestinationOccupied)
    );
    assert_eq!(
        fs::read_to_string(destination.join("content/notes/note.org"))
            .expect("preserved snapshot note"),
        "* Note\n"
    );
}

#[cfg(unix)]
#[test]
fn an_existing_snapshot_is_not_reached_through_a_symlinked_source_boundary() {
    let fixture = Fixture::new();
    fixture.write("notes/note.org", b"* Note\n");
    fixture.commit_all("storage boundary");
    let real_parent = fixture.root.path().join("real");
    fs::create_dir(&real_parent).expect("real snapshot parent");
    let destination = real_parent.join("snapshot");
    materialize(
        &fixture.request(&destination, "notes"),
        &AtomicBool::new(false),
        |_| {},
    )
    .expect("initial snapshot");
    let alias = fixture.root.path().join("alias");
    std::os::unix::fs::symlink(&real_parent, &alias).expect("snapshot parent alias");
    let aliased = fixture.request(&alias.join("snapshot"), "notes");

    assert_eq!(
        materialize(&aliased, &AtomicBool::new(false), |_| {}),
        Err(SnapshotError::StorageBoundary)
    );
    assert_eq!(
        fs::read_to_string(destination.join("content/notes/note.org"))
            .expect("preserved snapshot note"),
        "* Note\n"
    );
}

struct Fixture {
    root: TempDir,
    seed: PathBuf,
    remote: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("fixture root");
        let seed = root.path().join("seed");
        let remote = root.path().join("remote.git");
        run_git(["init", "--bare", text(&remote)]);
        run_git(["init", text(&seed)]);
        run_git_in(&seed, ["config", "user.name", "Slipbox Fixture"]);
        run_git_in(&seed, ["config", "user.email", "fixture@example.invalid"]);
        run_git_in(&seed, ["branch", "-M", "main"]);
        run_git_in(&seed, ["remote", "add", "origin", text(&remote)]);
        Self { root, seed, remote }
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.seed.join(relative);
        fs::create_dir_all(path.parent().expect("fixture file parent"))
            .expect("fixture directories");
        fs::write(path, bytes).expect("fixture file");
    }

    fn commit_all(&self, message: &str) {
        run_git_in(&self.seed, ["add", "-A"]);
        run_git_in(&self.seed, ["commit", "-m", message]);
        run_git_in(&self.seed, ["push", "origin", "main"]);
    }

    fn revision(&self) -> String {
        git_stdout_in(&self.seed, ["rev-parse", "HEAD"])
            .trim()
            .to_owned()
    }

    fn request(&self, destination: &Path, notes: &str) -> SnapshotRequest {
        SnapshotRequest::new(
            source(),
            self.remote.clone(),
            &self.revision(),
            NotesFolder::parse(notes).expect("notes folder"),
            destination.to_path_buf(),
        )
        .expect("snapshot request")
    }
}

fn source() -> SourceId {
    SourceId::parse(SOURCE).expect("fixture source")
}

fn run_git<const N: usize>(arguments: [&str; N]) {
    let output = Command::new("git")
        .args(arguments)
        .output()
        .expect("run fixture Git");
    assert!(
        output.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_git_in<const N: usize>(directory: &Path, arguments: [&str; N]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .expect("run fixture Git");
    assert!(
        output.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout_in<const N: usize>(directory: &Path, arguments: [&str; N]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .expect("run fixture Git");
    assert!(
        output.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("fixture Git UTF-8 output")
}

fn text(path: &Path) -> &str {
    path.to_str().expect("fixture path is UTF-8")
}
