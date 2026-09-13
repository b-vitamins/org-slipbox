//! The fixture probe's contract, exercised through the crate's public seam.

use std::fs;
use std::thread;

use serde_json::Value;
use slipbox_android::{ProbeReport, run_fixture_probe};
use tempfile::tempdir;

const EXPECTED_CHECKS: usize = 35;

#[test]
fn the_probe_indexes_queries_and_reopens_one_durable_database() {
    let parent = tempdir().expect("a temporary parent directory");
    let report = run_fixture_probe(parent.path());

    assert!(report.passed, "{}", describe(&report));
    assert_eq!(report.failed_checks().count(), 0, "{}", describe(&report));
    assert_eq!(
        report.checks.len(),
        EXPECTED_CHECKS,
        "{}",
        describe(&report)
    );

    assert_eq!(actual(&report, "index.files"), "3");
    assert_eq!(actual(&report, "index.nodes"), "5");
    assert_eq!(actual(&report, "index.links"), "1");

    // The reopened service answers from the durable file; a service opened on a
    // second database over the same root answers nothing.
    assert_eq!(
        actual(&report, "reopened.notes.files"),
        r#"["alpha.org", "beta.org", "riemann.org"]"#
    );
    assert_eq!(
        actual(&report, "reopened.notes.search"),
        r#"["Target heading"]"#
    );
    assert_eq!(
        actual(&report, "reopened.glossary.terms"),
        r#"["Riemann integral"]"#
    );
    assert_eq!(
        actual(&report, "reopened.glossary.due_before_schedule"),
        "[]"
    );
    assert_eq!(actual(&report, "control.notes.files"), "[]");
    assert_eq!(actual(&report, "control.glossary.terms"), "[]");

    // A mobile service authorizes no decryptor, whatever the workspace build
    // compiles for the desktop.
    assert_eq!(actual(&report, "authority.refusal_code"), "-32600");
    assert_eq!(actual(&report, "authority.names_capability"), "true");
    assert_eq!(actual(&report, "authority.names_path"), "true");

    assert_eq!(actual(&report, "close.database_outlives_service"), "true");
    assert_eq!(actual(&report, "cleanup.workspace_removed"), "Ok(())");
    assert_eq!(entries(parent.path()), Vec::<String>::new());
}

#[test]
fn the_report_encodes_the_json_contract_the_jni_seam_returns() {
    let parent = tempdir().expect("a temporary parent directory");
    let report = run_fixture_probe(parent.path());
    let encoded: Value = serde_json::to_value(&report).expect("a report encodes as JSON");

    assert_eq!(encoded["passed"], Value::Bool(true));
    assert!(encoded["failure"].is_null(), "{}", encoded["failure"]);
    let checks = encoded["checks"]
        .as_array()
        .expect("a report encodes its checks as an array");
    assert_eq!(checks.len(), EXPECTED_CHECKS);
    for key in ["name", "expected", "actual", "passed"] {
        assert!(
            checks[0].get(key).is_some(),
            "an encoded check carries {key}: {}",
            checks[0]
        );
    }
}

#[test]
fn concurrent_probes_share_a_parent_without_sharing_a_workspace() {
    let parent = tempdir().expect("a temporary parent directory");
    let reports: Vec<ProbeReport> = thread::scope(|scope| {
        let handles: Vec<_> = (0..3)
            .map(|_| scope.spawn(|| run_fixture_probe(parent.path())))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a probe thread finishes"))
            .collect()
    });

    for report in &reports {
        assert!(report.passed, "{}", describe(report));
    }
    assert_eq!(entries(parent.path()), Vec::<String>::new());
}

#[test]
fn a_parent_that_is_not_a_directory_is_reported_as_a_prepare_failure() {
    let parent = tempdir().expect("a temporary parent directory");
    let occupied = parent.path().join("occupied");
    fs::write(&occupied, "not a directory\n").expect("the occupied path is written");

    let report = run_fixture_probe(&occupied);

    assert!(!report.passed);
    // A claim that owns nothing records no removal, successful or otherwise.
    assert!(report.checks.is_empty(), "{}", describe(&report));
    let failure = report.failure.expect("a prepare failure is reported");
    assert_eq!(failure.stage, "prepare");
    assert!(
        failure
            .detail
            .contains("failed to claim the probe directory"),
        "{}",
        failure.detail
    );
    assert!(
        failure.detail.contains("occupied"),
        "the failure names the path it could not use: {}",
        failure.detail
    );
}

#[cfg(unix)]
#[test]
fn an_unwritable_parent_is_reported_without_leaving_state() {
    use std::os::unix::fs::PermissionsExt;

    let outer = tempdir().expect("a temporary parent directory");
    let parent = outer.path().join("read-only");
    fs::create_dir(&parent).expect("the read-only parent is created");
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o500))
        .expect("the parent is made read-only");

    let report = run_fixture_probe(&parent);

    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
        .expect("the parent is made writable again");

    assert!(!report.passed);
    assert!(report.checks.is_empty(), "{}", describe(&report));
    let failure = report.failure.expect("a prepare failure is reported");
    assert_eq!(failure.stage, "prepare");
    assert!(
        failure.detail.contains("Permission denied"),
        "{}",
        failure.detail
    );
    assert_eq!(entries(&parent), Vec::<String>::new());
}

fn actual<'a>(report: &'a ProbeReport, name: &str) -> &'a str {
    report
        .checks
        .iter()
        .find(|check| check.name == name)
        .unwrap_or_else(|| panic!("{name} is recorded"))
        .actual
        .as_str()
}

fn entries(directory: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .expect("the directory is readable")
        .map(|entry| {
            entry
                .expect("a directory entry is readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn describe(report: &ProbeReport) -> String {
    let failed: Vec<String> = report
        .failed_checks()
        .map(|check| {
            format!(
                "{}: expected {}, actual {}",
                check.name, check.expected, check.actual
            )
        })
        .collect();
    format!("failure {:?}, failed checks {failed:?}", report.failure)
}
