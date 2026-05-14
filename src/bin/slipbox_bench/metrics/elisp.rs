use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use slipbox_core::{
    BacklinkRecord, CompareNotesParams, ExplorationEntry, ExplorationLens, ExplorationSection,
    ExplorationSectionKind, ExploreResult, ForwardLinkRecord, NodeRecord, NoteComparisonResult,
};
use slipbox_store::Database;

use crate::slipbox_bench::constants::DEDICATED_COMPARE_CANDIDATE_LIMIT;
use crate::slipbox_bench::fixtures::{
    BufferFixture, DedicatedBufferFixture, DedicatedExplorationBufferFixture,
};
use crate::slipbox_bench::profile::BenchmarkProfile;
use crate::slipbox_bench::report::{ElispTimingReport, TimingReport, write_json};

pub(crate) fn benchmark_persistent_buffer(
    repo_root: &Path,
    profile: &BenchmarkProfile,
    node: &NodeRecord,
    backlinks: &[BacklinkRecord],
    forward_links: &[ForwardLinkRecord],
) -> Result<TimingReport> {
    let fixture = BufferFixture {
        node,
        backlinks,
        forward_links,
    };
    let fixture_file = repo_root
        .join("target")
        .join("bench")
        .join("persistent-buffer-fixture.json");
    if let Some(parent) = fixture_file.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create persistent buffer fixture directory {}",
                parent.display()
            )
        })?;
    }
    write_json(&fixture_file, &fixture)?;

    let emacs = std::env::var("EMACS").unwrap_or_else(|_| "emacs".to_owned());
    let eval = format!(
        "(princ (org-slipbox-buffer-bench-run-file {:?} {} {}))",
        fixture_file.to_string_lossy(),
        profile.iterations.persistent_buffer_samples,
        profile.iterations.persistent_buffer_iterations
    );
    let output = Command::new(&emacs)
        .current_dir(repo_root)
        .arg("-Q")
        .arg("--batch")
        .arg("-L")
        .arg(".")
        .arg("-l")
        .arg("org-slipbox.el")
        .arg("-l")
        .arg("benches/org-slipbox-buffer-bench.el")
        .arg("--eval")
        .arg(eval)
        .output()
        .with_context(|| format!("failed to execute {emacs} for persistent buffer benchmark"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("{emacs} exited with {}", output.status)
        } else {
            stderr
        };
        bail!("persistent buffer benchmark failed: {message}");
    }

    let report: ElispTimingReport = serde_json::from_slice(&output.stdout)
        .context("failed to parse persistent buffer report")?;
    if report.samples_ms.is_empty() {
        bail!("persistent buffer benchmark produced no samples");
    }
    Ok(TimingReport::from_samples(report.samples_ms))
}

pub(crate) fn select_dedicated_compare_fixture(
    database: &Database,
    node: &NodeRecord,
    backlinks: &[BacklinkRecord],
    forward_links: &[ForwardLinkRecord],
    limit: usize,
) -> Result<(NodeRecord, NoteComparisonResult)> {
    let params = |left: &NodeRecord, right: &NodeRecord| CompareNotesParams {
        left_node_key: left.node_key.clone(),
        right_node_key: right.node_key.clone(),
        limit,
    };

    let mut seen = BTreeSet::new();
    let candidates = forward_links
        .iter()
        .map(|record| record.destination_note.clone())
        .chain(backlinks.iter().map(|record| record.source_note.clone()))
        .filter(|candidate| {
            candidate.node_key != node.node_key && seen.insert(candidate.node_key.clone())
        })
        .take(DEDICATED_COMPARE_CANDIDATE_LIMIT)
        .collect::<Vec<_>>();

    let mut best = None;
    for candidate in candidates {
        let comparison = database.compare_notes(node, &candidate, &params(node, &candidate))?;
        let score = comparison
            .sections
            .iter()
            .map(|section| section.entries.len())
            .sum::<usize>();
        if best
            .as_ref()
            .is_none_or(|(_, _, best_score)| score > *best_score)
        {
            best = Some((candidate, comparison, score));
        }
    }

    if let Some((candidate, comparison, _score)) = best {
        Ok((candidate, comparison))
    } else {
        let comparison = database.compare_notes(node, node, &params(node, node))?;
        Ok((node.clone(), comparison))
    }
}

pub(crate) fn benchmark_dedicated_buffer(
    repo_root: &Path,
    profile: &BenchmarkProfile,
    node: &NodeRecord,
    compare_target: &NodeRecord,
    comparison_result: &NoteComparisonResult,
) -> Result<TimingReport> {
    let fixture = DedicatedBufferFixture {
        node,
        compare_target,
        comparison_result,
    };
    let fixture_file = repo_root
        .join("target")
        .join("bench")
        .join("dedicated-buffer-fixture.json");
    if let Some(parent) = fixture_file.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create dedicated buffer fixture directory {}",
                parent.display()
            )
        })?;
    }
    write_json(&fixture_file, &fixture)?;

    let emacs = std::env::var("EMACS").unwrap_or_else(|_| "emacs".to_owned());
    let eval = format!(
        "(princ (org-slipbox-buffer-bench-run-dedicated-file {:?} {} {}))",
        fixture_file.to_string_lossy(),
        profile.iterations.dedicated_buffer_samples,
        profile.iterations.dedicated_buffer_iterations
    );
    let output = Command::new(&emacs)
        .current_dir(repo_root)
        .arg("-Q")
        .arg("--batch")
        .arg("-L")
        .arg(".")
        .arg("-l")
        .arg("org-slipbox.el")
        .arg("-l")
        .arg("benches/org-slipbox-buffer-bench.el")
        .arg("--eval")
        .arg(eval)
        .output()
        .with_context(|| format!("failed to execute {emacs} for dedicated buffer benchmark"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("{emacs} exited with {}", output.status)
        } else {
            stderr
        };
        bail!("dedicated buffer benchmark failed: {message}");
    }

    let report: ElispTimingReport = serde_json::from_slice(&output.stdout)
        .context("failed to parse dedicated buffer report")?;
    if report.samples_ms.is_empty() {
        bail!("dedicated buffer benchmark produced no samples");
    }
    Ok(TimingReport::from_samples(report.samples_ms))
}

pub(crate) fn select_dedicated_exploration_fixture(
    database: &Database,
    node: &NodeRecord,
    limit: usize,
) -> Result<(ExplorationLens, ExploreResult)> {
    let unresolved_tasks = database.unresolved_tasks(node, limit)?;
    let weakly_integrated_notes = database.weakly_integrated_notes(node, limit)?;
    if unresolved_tasks.is_empty() || weakly_integrated_notes.is_empty() {
        bail!(
            "dedicated exploration benchmark requires non-empty unresolved and weakly integrated sections for node {}",
            node.node_key
        );
    }

    Ok((
        ExplorationLens::Unresolved,
        ExploreResult {
            lens: ExplorationLens::Unresolved,
            sections: vec![
                ExplorationSection {
                    kind: ExplorationSectionKind::UnresolvedTasks,
                    entries: unresolved_tasks
                        .into_iter()
                        .map(|record| ExplorationEntry::Anchor {
                            record: Box::new(record),
                        })
                        .collect(),
                },
                ExplorationSection {
                    kind: ExplorationSectionKind::WeaklyIntegratedNotes,
                    entries: weakly_integrated_notes
                        .into_iter()
                        .map(|record| ExplorationEntry::Anchor {
                            record: Box::new(record),
                        })
                        .collect(),
                },
            ],
        },
    ))
}

pub(crate) fn benchmark_dedicated_exploration_buffer(
    repo_root: &Path,
    profile: &BenchmarkProfile,
    node: &NodeRecord,
    lens: ExplorationLens,
    exploration_result: &ExploreResult,
) -> Result<TimingReport> {
    let fixture = DedicatedExplorationBufferFixture {
        node,
        lens,
        exploration_result,
    };
    let fixture_file = repo_root
        .join("target")
        .join("bench")
        .join("dedicated-exploration-buffer-fixture.json");
    if let Some(parent) = fixture_file.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create dedicated exploration fixture directory {}",
                parent.display()
            )
        })?;
    }
    write_json(&fixture_file, &fixture)?;

    let emacs = std::env::var("EMACS").unwrap_or_else(|_| "emacs".to_owned());
    let eval = format!(
        "(princ (org-slipbox-buffer-bench-run-exploration-file {:?} {} {}))",
        fixture_file.to_string_lossy(),
        profile.iterations.dedicated_exploration_buffer_samples,
        profile.iterations.dedicated_exploration_buffer_iterations
    );
    let output = Command::new(&emacs)
        .current_dir(repo_root)
        .arg("-Q")
        .arg("--batch")
        .arg("-L")
        .arg(".")
        .arg("-l")
        .arg("org-slipbox.el")
        .arg("-l")
        .arg("benches/org-slipbox-buffer-bench.el")
        .arg("--eval")
        .arg(eval)
        .output()
        .with_context(|| {
            format!("failed to execute {emacs} for dedicated exploration benchmark")
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("{emacs} exited with {}", output.status)
        } else {
            stderr
        };
        bail!("dedicated exploration benchmark failed: {message}");
    }

    let report: ElispTimingReport = serde_json::from_slice(&output.stdout)
        .context("failed to parse dedicated exploration buffer report")?;
    if report.samples_ms.is_empty() {
        bail!("dedicated exploration benchmark produced no samples");
    }
    Ok(TimingReport::from_samples(report.samples_ms))
}
