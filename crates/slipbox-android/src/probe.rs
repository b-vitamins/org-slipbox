//! Fixture probe: one owned corpus, one engine lifetime, one durable database.
//!
//! The probe qualifies a packaged native library. Its corpus and databases live
//! in a directory it claims and attempts to remove, so it never reads a real
//! slipbox.

use std::fmt::Debug;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde_json::json;
use slipbox_core::{
    GlossaryDueParams, GlossaryDueResult, GlossaryTermParams, GlossaryTermResult, IndexStats,
    IndexedFilesResult, ListGlossaryTermsParams, ListGlossaryTermsResult, NodeFromIdParams,
    NodeRecord, ReadNodeSourceParams, ReadNodeSourceResult, SearchNodesParams, SearchNodesResult,
};
use slipbox_engine::service::SlipboxService;
use slipbox_engine::{DiscoveryPolicy, PlatformPolicy};
use slipbox_rpc::{
    JsonRpcError, METHOD_GLOSSARY_DUE, METHOD_GLOSSARY_TERM, METHOD_INDEX, METHOD_INDEXED_FILES,
    METHOD_LIST_GLOSSARY_TERMS, METHOD_NODE_FROM_ID, METHOD_READ_NODE_SOURCE, METHOD_SEARCH_NODES,
};

const ALPHA: &str = "\
#+title: Alpha

* First heading
:PROPERTIES:
:ID: alpha-first
:END:
See [[id:beta-target][Beta]].
";

const BETA: &str = "\
#+title: Beta

* Target heading
:PROPERTIES:
:ID: beta-target
:END:
Target body.
";

const TERM: &str = "\
#+title: Riemann integral
#+glossary: t
:PROPERTIES:
:ID:              riemann-integral
:GLOSSARY_STATUS: confirmed
:SR_DUE:          2026-08-01
:SR_EASE:         2.50
:SR_INTERVAL:     6
:SR_REPS:         3
:SR_LAST:         2026-07-26
:END:

A definite integral.
";

const CORPUS: &[(&str, &str)] = &[
    ("alpha.org", ALPHA),
    ("beta.org", BETA),
    ("riemann.org", TERM),
];

const ENVELOPE_NAME: &str = "sealed.org.gpg";

const REFUSED: i64 = -32600;

const AFTER_SCHEDULE: &str = "2026-09-13";
const BEFORE_SCHEDULE: &str = "2026-07-01";

static RUNS: AtomicU64 = AtomicU64::new(0);

const CLAIM_ATTEMPTS: u32 = 64;

#[derive(Debug, Serialize)]
pub struct ProbeCheck {
    pub name: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
}

/// The stage that raised an error, with its whole context chain in `detail`.
#[derive(Debug, Serialize)]
pub struct ProbeFailure {
    pub stage: String,
    pub detail: String,
}

/// `passed` requires every check to pass and no reported failure.
#[derive(Debug, Serialize)]
pub struct ProbeReport {
    pub passed: bool,
    pub checks: Vec<ProbeCheck>,
    pub failure: Option<ProbeFailure>,
}

impl ProbeReport {
    fn new(checks: Vec<ProbeCheck>, failure: Option<ProbeFailure>) -> Self {
        Self {
            passed: failure.is_none() && checks.iter().all(|check| check.passed),
            checks,
            failure,
        }
    }

    /// Report a stage that could not start, in the shape callers decode. It
    /// carries no check: nothing was owned, so nothing was exercised or removed.
    pub fn aborted(stage: &str, detail: String) -> Self {
        Self::new(
            Vec::new(),
            Some(ProbeFailure {
                stage: stage.to_owned(),
                detail,
            }),
        )
    }

    pub fn failed_checks(&self) -> impl Iterator<Item = &ProbeCheck> {
        self.checks.iter().filter(|check| !check.passed)
    }
}

/// Index, query, close and reopen a synthetic corpus under `parent`.
///
/// `parent` must be a writable application-private directory. The probe claims
/// one fresh child of it, never an existing path. Every ordinary failure after
/// that claim keeps its own stage and detail and records the removal outcome as
/// `cleanup.workspace_removed`; a removal that fails names the path that
/// survived and is never retried. A claim that fails owns nothing, so it reports
/// only the `prepare` failure and records no removal. A panic reports no removal
/// either: the unwind attempts one without observing its outcome.
pub fn run_fixture_probe(parent: &Path) -> ProbeReport {
    probe(parent, |workspace, checks| {
        stage("prepare", workspace.prepare())?;
        exercise(workspace, checks)
    })
}

/// The claim, removal and report assembly every run shares; `run` is the only
/// stage a private fault control replaces.
fn probe(
    parent: &Path,
    run: impl FnOnce(&mut Workspace, &mut Vec<ProbeCheck>) -> Result<(), ProbeFailure>,
) -> ProbeReport {
    let mut workspace = match Workspace::claim(parent) {
        Ok(workspace) => workspace,
        Err(error) => return ProbeReport::aborted("prepare", format!("{error:#}")),
    };

    let mut checks = Vec::new();
    let failure = run(&mut workspace, &mut checks).err();
    record(
        &mut checks,
        "cleanup.workspace_removed",
        Ok(()),
        workspace.remove(),
    );

    ProbeReport::new(checks, failure)
}

fn candidate(parent: &Path, run: u64) -> PathBuf {
    parent.join(format!("slipbox-fixture-probe-{}-{run}", process::id()))
}

struct Workspace {
    directory: PathBuf,
    owned: bool,
}

impl Workspace {
    /// Claim a fresh child of `parent`, leaving every occupied candidate as it is.
    ///
    /// `create_dir` neither follows nor replaces an existing directory, file or
    /// symlink, so one atomic call decides ownership and a collision costs only
    /// another candidate.
    fn claim(parent: &Path) -> Result<Self> {
        for _ in 0..CLAIM_ATTEMPTS {
            let directory = candidate(parent, RUNS.fetch_add(1, Ordering::Relaxed));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    return Ok(Self {
                        directory,
                        owned: true,
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to claim the probe directory {}",
                            directory.display()
                        )
                    });
                }
            }
        }
        Err(anyhow!(
            "{CLAIM_ATTEMPTS} probe directories under {} are occupied",
            parent.display()
        ))
    }

    fn prepare(&mut self) -> Result<()> {
        let root = self.root();
        fs::create_dir(&root)
            .with_context(|| format!("failed to create the probe root {}", root.display()))?;
        for (name, contents) in CORPUS {
            let path = root.join(name);
            fs::write(&path, contents)
                .with_context(|| format!("failed to write the fixture {}", path.display()))?;
        }
        Ok(())
    }

    /// Remove the claimed child and report what the removal itself returned.
    ///
    /// Ownership ends on this first attempt, so a reported failure is never
    /// retried and rewritten. Absence counts as success only when the removal
    /// says the child is already gone: a denied or otherwise failed removal is an
    /// error naming the path, because metadata under an inaccessible parent
    /// cannot tell an absent child from an unreadable one.
    fn remove(&mut self) -> Result<(), String> {
        if !self.owned {
            return Ok(());
        }
        self.owned = false;
        match fs::remove_dir_all(&self.directory) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "failed to remove {}: {error}",
                self.directory.display()
            )),
        }
    }

    fn root(&self) -> PathBuf {
        self.directory.join("notes")
    }

    fn database(&self) -> PathBuf {
        self.directory.join("slipbox.sqlite")
    }

    fn control_database(&self) -> PathBuf {
        self.directory.join("control.sqlite")
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        // Best effort for an unwind, which reports nothing: an ordinary failure
        // has already attempted and recorded its own removal by here.
        let _ = self.remove();
    }
}

fn stage<T>(name: &str, result: Result<T>) -> Result<T, ProbeFailure> {
    result.map_err(|error| ProbeFailure {
        stage: name.to_owned(),
        detail: format!("{error:#}"),
    })
}

fn exercise(workspace: &Workspace, checks: &mut Vec<ProbeCheck>) -> Result<(), ProbeFailure> {
    let mut service = stage("open", open(workspace, workspace.database()))?;
    stage("index", index(&mut service, checks))?;
    stage("query", answer_queries(&mut service, checks, "indexed"))?;
    stage(
        "authority",
        refuse_encrypted_source(workspace, &mut service, checks),
    )?;

    drop(service);
    record(
        checks,
        "close.database_outlives_service",
        true,
        workspace.database().is_file(),
    );

    let mut reopened = stage("reopen", open(workspace, workspace.database()))?;
    stage("reopen", answer_queries(&mut reopened, checks, "reopened"))?;
    drop(reopened);

    let mut control = stage("control", open(workspace, workspace.control_database()))?;
    stage("control", answer_nothing(&mut control, checks))?;
    Ok(())
}

fn open(workspace: &Workspace, database: PathBuf) -> Result<SlipboxService> {
    SlipboxService::with_platform(
        workspace.root(),
        database,
        Vec::new(),
        DiscoveryPolicy::default(),
        // Android executes no subprocess, whatever decryptors the workspace
        // features compile elsewhere.
        PlatformPolicy::headless(),
    )
}

fn index(service: &mut SlipboxService, checks: &mut Vec<ProbeCheck>) -> Result<()> {
    let stats: IndexStats = service.invoke(METHOD_INDEX, json!({}))?;
    record(checks, "index.files", 3, stats.files_indexed);
    record(checks, "index.nodes", 5, stats.nodes_indexed);
    record(checks, "index.links", 1, stats.links_indexed);
    Ok(())
}

fn answer_queries(
    service: &mut SlipboxService,
    checks: &mut Vec<ProbeCheck>,
    phase: &str,
) -> Result<()> {
    record(
        checks,
        &format!("{phase}.notes.files"),
        owned(&["alpha.org", "beta.org", "riemann.org"]),
        indexed_files(service)?,
    );
    record(
        checks,
        &format!("{phase}.notes.search"),
        owned(&["Target heading"]),
        search_titles(service, "target")?,
    );

    let note = node_from_id(service, "beta-target")?;
    record(
        checks,
        &format!("{phase}.notes.title_of_match"),
        "Target heading".to_owned(),
        note.title.clone(),
    );
    record(
        checks,
        &format!("{phase}.notes.file_of_match"),
        "beta.org".to_owned(),
        note.file_path.clone(),
    );

    let read: ReadNodeSourceResult = service.invoke(
        METHOD_READ_NODE_SOURCE,
        ReadNodeSourceParams {
            node_key: note.node_key.clone(),
            context_before: None,
            context_after: None,
            max_lines: None,
        },
    )?;
    record(
        checks,
        &format!("{phase}.source.anchor"),
        note.node_key.clone(),
        read.anchor.node_key.clone(),
    );
    record(
        checks,
        &format!("{phase}.source.carries_body"),
        true,
        read.source.content.contains("Target body."),
    );

    record(
        checks,
        &format!("{phase}.glossary.terms"),
        owned(&["Riemann integral"]),
        glossary_titles(service)?,
    );

    let term = node_from_id(service, "riemann-integral")?;
    let resolved: GlossaryTermResult = service.invoke(
        METHOD_GLOSSARY_TERM,
        GlossaryTermParams {
            node_key: term.node_key.clone(),
        },
    )?;
    let resolved = resolved.term.ok_or_else(|| {
        anyhow!(
            "the marked fixture term {} resolves to no term",
            term.node_key
        )
    })?;
    record(
        checks,
        &format!("{phase}.glossary.status"),
        Some("confirmed".to_owned()),
        resolved.glossary_status.clone(),
    );
    record(
        checks,
        &format!("{phase}.glossary.due_date"),
        Some("2026-08-01".to_owned()),
        resolved.sr_due.clone(),
    );
    record(
        checks,
        &format!("{phase}.glossary.due_after_schedule"),
        owned(&["Riemann integral"]),
        due_titles(service, AFTER_SCHEDULE)?,
    );
    record(
        checks,
        &format!("{phase}.glossary.due_before_schedule"),
        Vec::new(),
        due_titles(service, BEFORE_SCHEDULE)?,
    );
    Ok(())
}

/// A service on a second, never-indexed database over the same root answers
/// nothing, so the reopened answers came from the durable file, not a fresh scan.
fn answer_nothing(service: &mut SlipboxService, checks: &mut Vec<ProbeCheck>) -> Result<()> {
    record(
        checks,
        "control.notes.files",
        Vec::new(),
        indexed_files(service)?,
    );
    record(
        checks,
        "control.notes.search",
        Vec::new(),
        search_titles(service, "target")?,
    );
    record(
        checks,
        "control.glossary.terms",
        Vec::new(),
        glossary_titles(service)?,
    );
    Ok(())
}

/// An envelope needs a decryptor this platform never authorizes: the scan
/// refuses it by capability and path, and recovers once it is gone.
fn refuse_encrypted_source(
    workspace: &Workspace,
    service: &mut SlipboxService,
    checks: &mut Vec<ProbeCheck>,
) -> Result<()> {
    let envelope = workspace.root().join(ENVELOPE_NAME);
    fs::write(&envelope, "sealed bytes\n")
        .with_context(|| format!("failed to write {}", envelope.display()))?;

    let refusal = service
        .invoke_value(METHOD_INDEX, json!({}))
        .err()
        .map(JsonRpcError::into_inner);
    record(checks, "authority.refused", true, refusal.is_some());
    let (code, message) = refusal.map_or((0, String::new()), |error| (error.code, error.message));
    record(checks, "authority.refusal_code", REFUSED, code);
    record(
        checks,
        "authority.names_capability",
        true,
        message.contains("gpg"),
    );
    record(
        checks,
        "authority.names_path",
        true,
        message.contains(ENVELOPE_NAME),
    );

    fs::remove_file(&envelope)
        .with_context(|| format!("failed to remove {}", envelope.display()))?;
    let stats: IndexStats = service.invoke(METHOD_INDEX, json!({}))?;
    record(checks, "authority.recovered_files", 3, stats.files_indexed);
    Ok(())
}

fn indexed_files(service: &mut SlipboxService) -> Result<Vec<String>> {
    let indexed: IndexedFilesResult = service.invoke(METHOD_INDEXED_FILES, json!({}))?;
    let mut files = indexed.files;
    files.sort();
    Ok(files)
}

fn search_titles(service: &mut SlipboxService, query: &str) -> Result<Vec<String>> {
    let found: SearchNodesResult = service.invoke(
        METHOD_SEARCH_NODES,
        SearchNodesParams {
            query: query.to_owned(),
            limit: 20,
            sort: None,
        },
    )?;
    Ok(titles(&found.nodes))
}

fn glossary_titles(service: &mut SlipboxService) -> Result<Vec<String>> {
    let listed: ListGlossaryTermsResult = service.invoke(
        METHOD_LIST_GLOSSARY_TERMS,
        ListGlossaryTermsParams {
            limit: 20,
            after: None,
        },
    )?;
    Ok(titles(&listed.terms))
}

fn due_titles(service: &mut SlipboxService, today: &str) -> Result<Vec<String>> {
    let due: GlossaryDueResult = service.invoke(
        METHOD_GLOSSARY_DUE,
        GlossaryDueParams {
            today: Some(today.to_owned()),
            query: None,
            limit: 20,
            after: None,
        },
    )?;
    Ok(titles(&due.terms))
}

fn node_from_id(service: &mut SlipboxService, id: &str) -> Result<NodeRecord> {
    let node: Option<NodeRecord> =
        service.invoke(METHOD_NODE_FROM_ID, NodeFromIdParams { id: id.to_owned() })?;
    node.ok_or_else(|| anyhow!("the fixture identifier {id} resolves to no indexed node"))
}

fn titles(nodes: &[NodeRecord]) -> Vec<String> {
    nodes.iter().map(|node| node.title.clone()).collect()
}

fn owned(titles: &[&str]) -> Vec<String> {
    titles.iter().map(|title| (*title).to_owned()).collect()
}

fn record<T: Debug>(checks: &mut Vec<ProbeCheck>, name: &str, expected: T, actual: T) {
    let expected = format!("{expected:?}");
    let actual = format!("{actual:?}");
    checks.push(ProbeCheck {
        name: name.to_owned(),
        passed: expected == actual,
        expected,
        actual,
    });
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use tempfile::tempdir;

    use super::*;

    /// Held by every test that claims, so a predicted candidate stays the next one.
    static CLAIMS: Mutex<()> = Mutex::new(());

    fn claims() -> MutexGuard<'static, ()> {
        CLAIMS.lock().expect("the claim order is serialized")
    }

    fn next_candidate(parent: &Path) -> PathBuf {
        candidate(parent, RUNS.load(Ordering::Relaxed))
    }

    fn prepared(parent: &Path) -> Workspace {
        let guard = claims();
        let mut workspace = Workspace::claim(parent).expect("a candidate is claimed");
        drop(guard);
        workspace.prepare().expect("the workspace is prepared");
        workspace
    }

    #[cfg(unix)]
    fn set_mode(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .unwrap_or_else(|error| panic!("failed to chmod {}: {error}", path.display()));
    }

    fn entries(parent: &Path) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = fs::read_dir(parent)
            .expect("the parent is readable")
            .map(|entry| entry.expect("an entry is readable").path())
            .collect();
        found.sort();
        found
    }

    fn cleanup_check(report: &ProbeReport) -> &ProbeCheck {
        report
            .checks
            .iter()
            .find(|check| check.name == "cleanup.workspace_removed")
            .unwrap_or_else(|| panic!("a claimed run records its removal: {report:?}"))
    }

    fn occupy(parent: &Path) -> PathBuf {
        let occupied = next_candidate(parent);
        fs::create_dir(&occupied).expect("the candidate is occupied");
        fs::write(occupied.join("sentinel"), "unowned\n").expect("the sentinel is written");
        occupied
    }

    fn sentinel(directory: &Path) -> String {
        fs::read_to_string(directory.join("sentinel")).unwrap_or_default()
    }

    #[test]
    fn a_claim_skips_an_occupied_candidate_and_preserves_its_contents() {
        let parent = tempdir().expect("a temporary parent directory");
        let guard = claims();
        let occupied = occupy(parent.path());

        let workspace = Workspace::claim(parent.path()).expect("a fresh candidate is claimed");
        assert_ne!(workspace.directory, occupied);
        let claimed = workspace.directory.clone();
        drop(workspace);
        drop(guard);

        assert!(
            !claimed.exists(),
            "{} outlived its owner",
            claimed.display()
        );
        assert!(occupied.is_dir(), "{} was removed", occupied.display());
        assert_eq!(sentinel(&occupied), "unowned\n");
    }

    #[test]
    fn a_claim_skips_a_file_at_the_candidate_path() {
        let parent = tempdir().expect("a temporary parent directory");
        let guard = claims();
        let occupied = next_candidate(parent.path());
        fs::write(&occupied, "unowned\n").expect("the candidate is a file");

        let workspace = Workspace::claim(parent.path()).expect("a fresh candidate is claimed");
        assert_ne!(workspace.directory, occupied);
        drop(workspace);
        drop(guard);

        assert!(occupied.is_file(), "{} was replaced", occupied.display());
        assert_eq!(
            fs::read_to_string(&occupied).expect("the file is readable"),
            "unowned\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_claim_neither_follows_nor_replaces_a_symlinked_candidate() {
        let parent = tempdir().expect("a temporary parent directory");
        let target = parent.path().join("target");
        fs::create_dir(&target).expect("the symlink target is created");
        fs::write(target.join("sentinel"), "unowned\n").expect("the sentinel is written");

        let guard = claims();
        let link = next_candidate(parent.path());
        std::os::unix::fs::symlink(&target, &link).expect("the candidate is a symlink");

        let workspace = Workspace::claim(parent.path()).expect("a fresh candidate is claimed");
        assert_ne!(workspace.directory, link);
        drop(workspace);
        drop(guard);

        assert!(
            fs::symlink_metadata(&link)
                .expect("the symlink is readable")
                .file_type()
                .is_symlink()
        );
        assert_eq!(sentinel(&target), "unowned\n");
    }

    #[test]
    fn a_probe_succeeds_beside_the_occupied_candidate_it_leaves_alone() {
        let parent = tempdir().expect("a temporary parent directory");
        let guard = claims();
        let occupied = occupy(parent.path());
        let report = run_fixture_probe(parent.path());
        drop(guard);

        assert!(report.passed, "{report:?}");
        assert_eq!(cleanup_check(&report).actual, "Ok(())");
        assert_eq!(sentinel(&occupied), "unowned\n");
        assert_eq!(entries(parent.path()), vec![occupied]);
    }

    #[test]
    fn concurrent_claims_under_one_parent_do_not_collide() {
        let parent = tempdir().expect("a temporary parent directory");
        let guard = claims();
        let claimed: Vec<PathBuf> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        Workspace::claim(parent.path()).expect("a fresh candidate is claimed")
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("a claiming thread finishes"))
                .map(|workspace| workspace.directory.clone())
                .collect()
        });
        drop(guard);

        let distinct: std::collections::BTreeSet<&PathBuf> = claimed.iter().collect();
        assert_eq!(distinct.len(), claimed.len(), "{claimed:?}");
        for directory in &claimed {
            assert!(
                !directory.exists(),
                "{} outlived its owner",
                directory.display()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_preparation_failure_after_ownership_leaves_no_owned_residue() {
        let parent = tempdir().expect("a temporary parent directory");
        let guard = claims();
        let mut workspace = Workspace::claim(parent.path()).expect("a candidate is claimed");
        drop(guard);
        let claimed = workspace.directory.clone();
        set_mode(&claimed, 0o500);

        let error = workspace.prepare().expect_err("preparation fails");
        assert!(
            format!("{error:#}").contains("failed to create the probe root"),
            "{error:#}"
        );

        set_mode(&claimed, 0o700);
        drop(workspace);
        assert!(
            !claimed.exists(),
            "{} outlived its owner",
            claimed.display()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_preparation_failure_records_the_cleanup_that_succeeded() {
        let parent = tempdir().expect("a temporary parent directory");
        let guard = claims();
        let report = probe(parent.path(), |workspace, checks| {
            set_mode(&workspace.directory, 0o500);
            let prepared = stage("prepare", workspace.prepare());
            set_mode(&workspace.directory, 0o700);
            assert!(checks.is_empty(), "{checks:?}");
            prepared
        });
        drop(guard);

        let failure = report.failure.as_ref().expect("the failure is retained");
        assert_eq!(failure.stage, "prepare");
        assert!(
            failure.detail.contains("failed to create the probe root"),
            "{}",
            failure.detail
        );
        assert_eq!(cleanup_check(&report).actual, "Ok(())");
        assert!(cleanup_check(&report).passed, "{report:?}");
        assert!(!report.passed);
        assert_eq!(entries(parent.path()), Vec::<PathBuf>::new());
    }

    #[test]
    fn an_exercise_failure_records_the_cleanup_that_succeeded() {
        let parent = tempdir().expect("a temporary parent directory");
        let guard = claims();
        let report = probe(parent.path(), |workspace, checks| {
            stage("prepare", workspace.prepare())?;
            // A directory at the database path fails the open the corpus is ready
            // for, so the failure comes from the exercise rather than the claim.
            fs::create_dir(workspace.database()).expect("the database path is occupied");
            exercise(workspace, checks)
        });
        drop(guard);

        let failure = report.failure.as_ref().expect("the failure is retained");
        assert_eq!(failure.stage, "open", "{}", failure.detail);
        assert_eq!(cleanup_check(&report).actual, "Ok(())");
        assert!(!report.passed);
        assert_eq!(entries(parent.path()), Vec::<PathBuf>::new());
    }

    #[cfg(unix)]
    #[test]
    fn a_cleanup_failure_is_reported_rather_than_retried() {
        let outer = tempdir().expect("a temporary parent directory");
        let parent = outer.path().join("locked");
        fs::create_dir(&parent).expect("the parent is created");
        let mut workspace = prepared(&parent);
        let claimed = workspace.directory.clone();

        set_mode(&parent, 0o500);
        let error = workspace.remove().expect_err("the removal fails");
        set_mode(&parent, 0o700);

        assert!(error.contains("failed to remove"), "{error}");
        assert!(error.contains(&claimed.display().to_string()), "{error}");
        drop(workspace);
        assert!(
            claimed.is_dir(),
            "the guard retried a removal already reported"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_cleanup_failure_under_an_inaccessible_parent_is_not_absence() {
        let outer = tempdir().expect("a temporary parent directory");
        let parent = outer.path().join("sealed");
        fs::create_dir(&parent).expect("the parent is created");
        let mut workspace = prepared(&parent);
        let claimed = workspace.directory.clone();

        set_mode(&parent, 0o000);
        let error = workspace.remove().expect_err("the removal fails");
        set_mode(&parent, 0o700);

        assert!(error.contains("failed to remove"), "{error}");
        assert!(error.contains(&claimed.display().to_string()), "{error}");
        assert!(claimed.is_dir(), "{} was removed", claimed.display());
        drop(workspace);
        assert!(
            claimed.is_dir(),
            "the guard retried a removal already reported"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_primary_failure_and_a_failed_cleanup_are_both_retained() {
        let outer = tempdir().expect("a temporary parent directory");
        let parent = outer.path().join("sealed");
        fs::create_dir(&parent).expect("the parent is created");

        let mut claimed = PathBuf::new();
        let guard = claims();
        let report = probe(&parent, |workspace, _| {
            claimed = workspace.directory.clone();
            set_mode(&workspace.directory, 0o500);
            let prepared = stage("prepare", workspace.prepare());
            set_mode(&parent, 0o000);
            prepared
        });
        drop(guard);
        set_mode(&parent, 0o700);
        set_mode(&claimed, 0o700);

        let failure = report.failure.as_ref().expect("the failure is retained");
        assert_eq!(failure.stage, "prepare");
        assert!(
            failure.detail.contains("failed to create the probe root"),
            "{}",
            failure.detail
        );
        let cleanup = cleanup_check(&report);
        assert!(!cleanup.passed, "{cleanup:?}");
        assert!(cleanup.actual.contains("failed to remove"), "{cleanup:?}");
        assert!(!report.passed);
        assert!(claimed.is_dir(), "{} was removed", claimed.display());
    }

    #[cfg(unix)]
    #[test]
    fn an_already_absent_owned_child_is_removed_and_a_denied_one_is_not() {
        let outer = tempdir().expect("a temporary parent directory");
        let parent = outer.path().join("parent");
        fs::create_dir(&parent).expect("the parent is created");

        let mut absent = prepared(&parent);
        fs::remove_dir_all(&absent.directory).expect("the owned child is removed beforehand");
        assert_eq!(absent.remove(), Ok(()));

        let mut denied = prepared(&parent);
        let claimed = denied.directory.clone();
        set_mode(&parent, 0o500);
        let removal = denied.remove();
        set_mode(&parent, 0o700);

        assert!(removal.is_err(), "a denied removal reported absence");
        assert!(claimed.is_dir(), "{} was removed", claimed.display());
    }
}
