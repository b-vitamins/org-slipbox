use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use slipbox_core::{NotesFolder, SourceId};
use tempfile::TempDir;

use super::*;

const SOURCE: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn derives_added_modified_deleted_and_renamed_notes_and_assets() {
    let fixture = Fixture::new();
    fixture.write(
        "notes/stay.org",
        b"* Stable title\n\nOriginal body words.\n",
    );
    fixture.write(
        "notes/move.org",
        b"* Moving note\n\nA durable body with enough unchanged words for identity.\n",
    );
    fixture.write("notes/delete.org", b"* Delete this note\n");
    fixture.write("assets/icon.png", b"unchanged-image");
    fixture.write("assets/old.png", b"renamed-image");
    let previous = fixture.commit_all("initial tree");

    fixture.write("notes/stay.org", b"* Stable title\n\nEdited body words.\n");
    fs::rename(
        fixture.seed.join("notes/move.org"),
        fixture.seed.join("notes/moved.org"),
    )
    .expect("rename note");
    fixture.write(
        "notes/moved.org",
        b"* Moving note revised\n\nA durable body with enough unchanged words for identity.\n",
    );
    fs::remove_file(fixture.seed.join("notes/delete.org")).expect("delete note");
    fs::rename(
        fixture.seed.join("assets/old.png"),
        fixture.seed.join("assets/new.png"),
    )
    .expect("rename asset");
    fixture.write("notes/add.org", b"* Added note\n");
    fixture.write("assets/caption.txt", b"new caption\n");
    let revision = fixture.commit_all("mixed changes");

    let outcome = fixture.delta(&previous, &revision).expect("complete delta");

    assert_eq!(outcome.disposition, DeltaDisposition::Changed);
    assert_eq!(outcome.history, DeltaHistory::FastForward);
    assert_eq!(outcome.rename_detection, RenameDetection::Complete);
    assert_eq!(
        outcome.added,
        vec![
            DeltaFile {
                path: "assets/caption.txt".into(),
                kind: DeltaFileKind::Asset,
            },
            DeltaFile {
                path: "notes/add.org".into(),
                kind: DeltaFileKind::Org,
            },
        ]
    );
    assert_eq!(
        outcome.modified,
        vec![DeltaFile {
            path: "notes/stay.org".into(),
            kind: DeltaFileKind::Org,
        }]
    );
    assert_eq!(
        outcome.deleted,
        vec![DeltaFile {
            path: "notes/delete.org".into(),
            kind: DeltaFileKind::Org,
        }]
    );
    assert_eq!(
        outcome.renamed,
        vec![
            RenameHint {
                from: "assets/old.png".into(),
                to: "assets/new.png".into(),
                kind: DeltaFileKind::Asset,
                content_changed: false,
            },
            RenameHint {
                from: "notes/move.org".into(),
                to: "notes/moved.org".into(),
                kind: DeltaFileKind::Org,
                content_changed: true,
            },
        ]
    );
}

#[test]
fn unchanged_revision_is_a_no_op_before_inventory_work() {
    let fixture = Fixture::new();
    fixture.write("notes/one.org", b"* One\n");
    fixture.write("notes/two.org", b"* Two\n");
    let revision = fixture.commit_all("two notes");
    let snapshot_limits = SnapshotLimits::new(1, 1024, 1024, 4096).expect("small entry limit");
    let limits = DeltaLimits::new(10, 10, 10)
        .expect("delta limits")
        .with_snapshot_limits(snapshot_limits);
    let request = fixture.request(&revision, &revision).with_limits(limits);

    let outcome = derive_delta(&request, &AtomicBool::new(false)).expect("unchanged no-op");

    assert_eq!(outcome.disposition, DeltaDisposition::Unchanged);
    assert_eq!(outcome.history, DeltaHistory::SameRevision);
    assert_eq!(outcome.change_count(), 0);
}

#[test]
fn cancellation_and_rename_limits_are_explicit() {
    let fixture = Fixture::new();
    fixture.write("notes/old-a.org", b"* Apples\nred green fruit\n");
    fixture.write("notes/old-b.org", b"* Oceans\nblue deep water\n");
    let previous = fixture.commit_all("old notes");
    fs::remove_file(fixture.seed.join("notes/old-a.org")).expect("delete old A");
    fs::remove_file(fixture.seed.join("notes/old-b.org")).expect("delete old B");
    fixture.write("notes/new-a.org", b"* Engines\nsteel piston fuel\n");
    fixture.write("notes/new-b.org", b"* Forests\ngreen leaf canopy\n");
    let revision = fixture.commit_all("replacement notes");
    let limits = DeltaLimits::new(10, 10, 1).expect("one rename comparison");
    let request = fixture.request(&previous, &revision).with_limits(limits);

    let outcome = derive_delta(&request, &AtomicBool::new(false)).expect("bounded exact delta");

    assert_eq!(outcome.rename_detection, RenameDetection::Limited);
    assert_eq!(outcome.added.len(), 2);
    assert_eq!(outcome.deleted.len(), 2);
    assert!(outcome.renamed.is_empty());

    let cancelled = AtomicBool::new(false);
    cancelled.store(true, Ordering::Relaxed);
    assert_eq!(
        derive_delta(&request, &cancelled),
        Err(DeltaError::Cancelled)
    );
}

#[test]
fn handles_initial_import_rewind_and_rewritten_history_without_a_merge_base() {
    let fixture = Fixture::new();
    fixture.write("notes/main.org", b"* Main history\n");
    let first = fixture.commit_all("main root");
    fixture.write("notes/later.org", b"* Later\n");
    let second = fixture.commit_all("main child");

    let initial = derive_delta(
        &DeltaRequest::initial(source(), fixture.seed.clone(), &second, notes_folder())
            .expect("initial request"),
        &AtomicBool::new(false),
    )
    .expect("initial delta");
    assert_eq!(initial.history, DeltaHistory::Initial);
    assert_eq!(initial.added.len(), 2);

    let rewind = fixture.delta(&second, &first).expect("rewind delta");
    assert_eq!(rewind.history, DeltaHistory::Rewind);
    assert_eq!(
        rewind.deleted,
        vec![DeltaFile {
            path: "notes/later.org".into(),
            kind: DeltaFileKind::Org,
        }]
    );

    run_git_in(&fixture.seed, ["checkout", "--orphan", "rewritten"]);
    run_git_in(&fixture.seed, ["read-tree", "--empty"]);
    fs::remove_file(fixture.seed.join("notes/main.org")).expect("remove main note");
    fs::remove_file(fixture.seed.join("notes/later.org")).expect("remove later note");
    fixture.write("notes/rewrite.org", b"* Rewritten branch\n");
    let rewritten = fixture.commit_all("rewritten root");

    let divergence = fixture.delta(&second, &rewritten).expect("divergent delta");
    assert_eq!(divergence.history, DeltaHistory::Diverged);
    assert_eq!(
        divergence.added,
        vec![DeltaFile {
            path: "notes/rewrite.org".into(),
            kind: DeltaFileKind::Org,
        }]
    );
    assert_eq!(divergence.deleted.len(), 2);
}

#[test]
fn missing_ancestry_is_explicit_without_preventing_an_exact_tree_delta() {
    let fixture = Fixture::new();
    fixture.write("notes/old.org", b"* Old\n");
    let previous = fixture.commit_all("available root");
    fixture.write("notes/new.org", b"* New\n");
    run_git_in(&fixture.seed, ["add", "-A"]);
    let tree = git_stdout_in(&fixture.seed, ["write-tree"]);
    let missing_parent = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let commit = format!(
        "tree {}\nparent {missing_parent}\nauthor Slipbox Fixture <fixture@example.invalid> 1 +0000\ncommitter Slipbox Fixture <fixture@example.invalid> 1 +0000\n\nshallow replacement\n",
        tree.trim()
    );
    let revision = git_stdin_stdout_in(
        &fixture.seed,
        ["hash-object", "-t", "commit", "-w", "--stdin"],
        commit.as_bytes(),
    );

    let outcome = fixture
        .delta(&previous, revision.trim())
        .expect("tree delta with incomplete ancestry");

    assert_eq!(outcome.history, DeltaHistory::Incomplete);
    assert_eq!(
        outcome.added,
        vec![DeltaFile {
            path: "notes/new.org".into(),
            kind: DeltaFileKind::Org,
        }]
    );
    assert!(outcome.deleted.is_empty());
}

#[test]
fn missing_objects_fail_without_returning_a_partial_delta() {
    let fixture = Fixture::new();
    fixture.write("notes/ok.org", b"* Complete\n");
    let previous = fixture.commit_all("complete tree");
    let missing = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let notes_tree = git_stdin_stdout_in(
        &fixture.seed,
        ["mktree", "--missing"],
        format!("100644 blob {missing}\tmissing.org\n").as_bytes(),
    );
    let root_tree = git_stdin_stdout_in(
        &fixture.seed,
        ["mktree", "--missing"],
        format!("040000 tree {}\tnotes\n", notes_tree.trim()).as_bytes(),
    );
    let malformed = git_stdout_in(
        &fixture.seed,
        ["commit-tree", root_tree.trim(), "-m", "missing blob"],
    );
    let request = fixture.request(&previous, malformed.trim());

    assert_eq!(
        derive_delta(&request, &AtomicBool::new(false)),
        Err(DeltaError::ObjectInvalid)
    );
}

#[cfg(unix)]
#[test]
fn changes_to_symlink_targets_are_changes_to_every_materialized_path() {
    let fixture = Fixture::new();
    fixture.write("assets/target.txt", b"first\n");
    fs::create_dir_all(fixture.seed.join("notes")).expect("notes directory");
    std::os::unix::fs::symlink(
        "../assets/target.txt",
        fixture.seed.join("notes/linked.txt"),
    )
    .expect("confined symlink");
    let previous = fixture.commit_all("linked asset");
    fixture.write("assets/target.txt", b"second\n");
    let revision = fixture.commit_all("change target");

    let outcome = fixture
        .delta(&previous, &revision)
        .expect("symlink-aware delta");

    assert_eq!(
        outcome.modified,
        vec![
            DeltaFile {
                path: "assets/target.txt".into(),
                kind: DeltaFileKind::Asset,
            },
            DeltaFile {
                path: "notes/linked.txt".into(),
                kind: DeltaFileKind::Asset,
            },
        ]
    );
}

struct Fixture {
    _root: TempDir,
    seed: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("fixture root");
        let seed = root.path().join("seed");
        run_git(["init", text(&seed)]);
        run_git_in(&seed, ["config", "user.name", "Slipbox Fixture"]);
        run_git_in(&seed, ["config", "user.email", "fixture@example.invalid"]);
        run_git_in(&seed, ["branch", "-M", "main"]);
        Self { _root: root, seed }
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.seed.join(relative);
        fs::create_dir_all(path.parent().expect("fixture file parent"))
            .expect("fixture directories");
        fs::write(path, bytes).expect("fixture file");
    }

    fn commit_all(&self, message: &str) -> String {
        run_git_in(&self.seed, ["add", "-A"]);
        run_git_in(&self.seed, ["commit", "-m", message]);
        git_stdout_in(&self.seed, ["rev-parse", "HEAD"])
            .trim()
            .to_owned()
    }

    fn request(&self, previous: &str, revision: &str) -> DeltaRequest {
        DeltaRequest::between(
            source(),
            self.seed.clone(),
            previous,
            revision,
            notes_folder(),
        )
        .expect("delta request")
    }

    fn delta(&self, previous: &str, revision: &str) -> Result<DeltaOutcome, DeltaError> {
        derive_delta(&self.request(previous, revision), &AtomicBool::new(false))
    }
}

fn source() -> SourceId {
    SourceId::parse(SOURCE).expect("fixture source")
}

fn notes_folder() -> NotesFolder {
    NotesFolder::parse("notes").expect("notes folder")
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

fn git_stdin_stdout_in<const N: usize>(
    directory: &Path,
    arguments: [&str; N],
    input: &[u8],
) -> String {
    let mut child = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fixture Git");
    child
        .stdin
        .take()
        .expect("fixture Git stdin")
        .write_all(input)
        .expect("write fixture Git stdin");
    let output = child.wait_with_output().expect("wait for fixture Git");
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
