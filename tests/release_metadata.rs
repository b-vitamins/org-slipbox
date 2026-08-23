#![cfg(unix)]

use std::fs;
use std::process::{Command, Output};

use anyhow::Result;
use tempfile::tempdir;

fn check_metadata(readme: &str, heading: &str, release: Option<&str>) -> Result<Output> {
    let workspace = tempdir()?;
    let client = workspace.path().join("crates/slipbox-web/client");
    fs::create_dir_all(&client)?;
    fs::write(
        workspace.path().join("Makefile"),
        include_str!("../Makefile"),
    )?;
    fs::write(
        workspace.path().join("Cargo.toml"),
        "[workspace.package]\nversion = \"0.18.0\"\n",
    )?;
    fs::write(workspace.path().join("README.md"), readme)?;
    fs::write(workspace.path().join("CHANGELOG.md"), heading)?;
    fs::write(
        workspace.path().join("org-slipbox.el"),
        ";; Copyright (C) 2026 Ayan Das\n;; Version: 0.18.0\n",
    )?;
    fs::write(
        client.join("package.json"),
        "{\n  \"version\": \"0.18.0\"\n}\n",
    )?;
    fs::write(
        client.join("package-lock.json"),
        "{\n  \"version\": \"0.18.0\",\n  \"packages\": {\n    \"\": {\n      \"version\": \"0.18.0\"\n    },\n    \"node_modules/example\": {}\n  }\n}\n",
    )?;
    let mut command = Command::new("make");
    command
        .current_dir(workspace.path())
        .arg("check-release-metadata");
    command.arg(format!("RELEASE_VERSION={}", release.unwrap_or_default()));
    Ok(command.output()?)
}

#[test]
fn candidate_metadata_passes_without_claiming_a_shipped_release() -> Result<()> {
    let output = check_metadata(
        "The latest shipped release is `0.17.0`.\nThe current release candidate is `0.18.0`.\n",
        "## [0.18.0] - Unreleased\n",
        None,
    )?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

#[test]
fn metadata_rejects_a_version_claimed_as_both_candidate_and_shipped() -> Result<()> {
    let output = check_metadata(
        "The latest shipped release is `0.18.0`.\nThe current release candidate is `0.18.0`.\n",
        "## [0.18.0] - Unreleased\n",
        None,
    )?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("both a candidate and shipped"));
    Ok(())
}

#[test]
fn release_gate_rejects_candidate_metadata() -> Result<()> {
    let output = check_metadata(
        "The current release candidate is `0.18.0`.\n",
        "## [0.18.0] - Unreleased\n",
        Some("0.18.0"),
    )?;
    assert!(!output.status.success());
    let errors = String::from_utf8_lossy(&output.stdout);
    assert!(errors.contains("latest shipped release"));
    assert!(errors.contains("dated release section"));
    Ok(())
}

#[test]
fn release_gate_accepts_matching_shipped_metadata() -> Result<()> {
    let output = check_metadata(
        "The latest shipped release is `0.18.0`.\n",
        "## [0.18.0] - 2026-09-12\n",
        Some("0.18.0"),
    )?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

#[test]
fn release_gate_rejects_a_version_other_than_the_package_version() -> Result<()> {
    let output = check_metadata(
        "The latest shipped release is `0.18.0`.\n",
        "## [0.18.0] - 2026-09-12\n",
        Some("0.19.0"),
    )?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("does not match 0.18.0"));
    Ok(())
}

#[test]
fn candidate_gate_rejects_a_premature_release_date() -> Result<()> {
    let output = check_metadata(
        "The current release candidate is `0.18.0`.\n",
        "## [0.18.0] - 2026-09-12\n",
        None,
    )?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("must be marked Unreleased"));
    Ok(())
}

fn extract_release_notes(changelog: &str) -> Result<Output> {
    let workspace = tempdir()?;
    let changelog_path = workspace.path().join("CHANGELOG.md");
    fs::write(&changelog_path, changelog)?;
    let workflow = include_str!("../.github/workflows/release.yml");
    let (_, extraction) = workflow
        .split_once("awk -v version=\"$version\" '")
        .unwrap();
    let (program, _) = extraction.split_once("' CHANGELOG.md").unwrap();
    Ok(Command::new("awk")
        .args(["-v", "version=0.18.0", program])
        .arg(changelog_path)
        .output()?)
}

#[test]
fn release_notes_extract_only_the_matching_changelog_section() -> Result<()> {
    let output = extract_release_notes(
        "## [Unreleased]\n\n## [0.18.0] - Unreleased\n\n### Fixed\n- A fix.\n\n## [0.17.0] - 2026-07-30\n- An older fix.\n",
    )?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "\n### Fixed\n- A fix.\n\n"
    );
    Ok(())
}

#[test]
fn release_notes_reject_missing_empty_and_whitespace_only_sections() -> Result<()> {
    for changelog in [
        "## [0.17.0] - 2026-07-30\n- An older fix.\n",
        "## [0.18.0] - Unreleased\n## [0.17.0] - 2026-07-30\n- An older fix.\n",
        "## [0.18.0] - Unreleased\n\n \t\n## [0.17.0] - 2026-07-30\n- An older fix.\n",
    ] {
        assert!(!extract_release_notes(changelog)?.status.success());
    }
    Ok(())
}
