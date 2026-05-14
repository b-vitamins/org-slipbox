use std::collections::BTreeSet;
use std::hint::black_box;

use anyhow::{Result, bail};
use slipbox_core::{
    CorpusAuditEntry, CorpusAuditKind, ExtractSubtreeParams, RefileRegionParams,
    RefileSubtreeParams, ReviewFindingPayload, ReviewFindingRemediationApplyParams,
    ReviewFindingRemediationPreviewParams, ReviewRunIdParams, RewriteFileParams,
    SaveCorpusAuditReviewParams, SlipboxLinkRewriteApplyParams, SlipboxLinkRewritePreviewParams,
    StructuralWriteOperationKind,
};

use crate::slipbox_bench::WorkbenchBench;
use crate::slipbox_bench::assertions::{
    assert_saved_audit_review_fixture, assert_slipbox_link_rewrite_preview_fixture,
    assert_structural_write_fixture, indexed_node_from_id,
    unlink_dangling_link_action_from_preview, write_indexed_bench_file,
};
use crate::slipbox_bench::constants::REMEDIATION_APPLY_REVIEW_ID;
use crate::slipbox_bench::fixtures::{
    CorpusFixture, RemediationApplyBenchmarkFixture, SlipboxLinkRewriteBenchmarkFixture,
    StructuralBenchmarkFixture,
};
use crate::slipbox_bench::profile::BenchmarkProfile;
use crate::slipbox_bench::report::{TimingReport, measure_iterations};

pub(crate) fn prepare_structural_benchmark_fixture(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<StructuralBenchmarkFixture> {
    let mut refile_subtree = Vec::with_capacity(profile.iterations.structural_refile_subtree);
    for iteration in 0..profile.iterations.structural_refile_subtree {
        let source_id = format!("bench-structural-refile-subtree-source-{iteration:04}");
        let target_id = format!("bench-structural-refile-subtree-target-{iteration:04}");
        let source_path = format!("bench-structural/refile-subtree/source-{iteration:04}.org");
        let target_path = format!("bench-structural/refile-subtree/target-{iteration:04}.org");
        write_indexed_bench_file(
            workbench,
            &fixture.root,
            &source_path,
            &format!(
                "#+title: Refile Subtree Source {iteration:04}\n\n* Move Subtree {iteration:04}\n:PROPERTIES:\n:ID: {source_id}\n:END:\nBody before refile.\n** Child {iteration:04}\nChild body.\n"
            ),
        )?;
        write_indexed_bench_file(
            workbench,
            &fixture.root,
            &target_path,
            &format!(
                "#+title: Refile Subtree Target {iteration:04}\n:PROPERTIES:\n:ID: {target_id}\n:END:\n\nTarget body.\n"
            ),
        )?;
        refile_subtree.push(RefileSubtreeParams {
            source_node_key: indexed_node_from_id(workbench, &source_id)?.node_key,
            target_node_key: indexed_node_from_id(workbench, &target_id)?.node_key,
        });
    }

    let mut refile_region = Vec::with_capacity(profile.iterations.structural_refile_region);
    for iteration in 0..profile.iterations.structural_refile_region {
        let target_id = format!("bench-structural-refile-region-target-{iteration:04}");
        let source_path = format!("bench-structural/refile-region/source-{iteration:04}.org");
        let target_path = format!("bench-structural/refile-region/target-{iteration:04}.org");
        let prefix = format!(
            "#+title: Refile Region Source {iteration:04}\n:PROPERTIES:\n:ID: bench-structural-refile-region-source-{iteration:04}\n:END:\n\n"
        );
        let selected = format!(
            "Region paragraph {iteration:04} line one.\nRegion paragraph {iteration:04} line two.\n"
        );
        let body = format!("{prefix}{selected}");
        let start = prefix.chars().count() as u32 + 1;
        let end = body.chars().count() as u32 + 1;
        write_indexed_bench_file(workbench, &fixture.root, &source_path, &body)?;
        write_indexed_bench_file(
            workbench,
            &fixture.root,
            &target_path,
            &format!(
                "#+title: Refile Region Target {iteration:04}\n:PROPERTIES:\n:ID: {target_id}\n:END:\n\nTarget body.\n"
            ),
        )?;
        refile_region.push(RefileRegionParams {
            file_path: source_path,
            start,
            end,
            target_node_key: indexed_node_from_id(workbench, &target_id)?.node_key,
        });
    }

    let mut extract_subtree = Vec::with_capacity(profile.iterations.structural_extract_subtree);
    for iteration in 0..profile.iterations.structural_extract_subtree {
        let source_id = format!("bench-structural-extract-source-{iteration:04}");
        let source_path = format!("bench-structural/extract/source-{iteration:04}.org");
        let target_path = format!("bench-structural/extract/extracted-{iteration:04}.org");
        write_indexed_bench_file(
            workbench,
            &fixture.root,
            &source_path,
            &format!(
                "#+title: Extract Source {iteration:04}\n\n* Extract Me {iteration:04} :bench:\n:PROPERTIES:\n:ID: {source_id}\n:END:\nExtract body.\n** Extract Child {iteration:04}\nChild body.\n"
            ),
        )?;
        extract_subtree.push(ExtractSubtreeParams {
            source_node_key: indexed_node_from_id(workbench, &source_id)?.node_key,
            file_path: target_path,
        });
    }

    let mut promote_file = Vec::with_capacity(profile.iterations.structural_promote_file);
    for iteration in 0..profile.iterations.structural_promote_file {
        let file_path = format!("bench-structural/promote/promote-{iteration:04}.org");
        write_indexed_bench_file(
            workbench,
            &fixture.root,
            &file_path,
            &format!(
                "* Promote Me {iteration:04} :bench:\n:PROPERTIES:\n:ID: bench-structural-promote-{iteration:04}\n:END:\nPromote body.\n\n** Promote Child {iteration:04}\nChild body.\n"
            ),
        )?;
        promote_file.push(RewriteFileParams { file_path });
    }

    let mut demote_file = Vec::with_capacity(profile.iterations.structural_demote_file);
    for iteration in 0..profile.iterations.structural_demote_file {
        let file_path = format!("bench-structural/demote/demote-{iteration:04}.org");
        write_indexed_bench_file(
            workbench,
            &fixture.root,
            &file_path,
            &format!(
                "#+title: Demote Me {iteration:04}\n#+filetags: :bench:\n:PROPERTIES:\n:ID: bench-structural-demote-{iteration:04}\n:END:\n\nDemote body.\n\n* Demote Child {iteration:04}\nChild body.\n"
            ),
        )?;
        demote_file.push(RewriteFileParams { file_path });
    }

    Ok(StructuralBenchmarkFixture {
        refile_subtree,
        refile_region,
        extract_subtree,
        promote_file,
        demote_file,
    })
}

pub(crate) fn benchmark_structural_refile_subtree(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &StructuralBenchmarkFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.structural_refile_subtree, |iteration| {
        let report = workbench.refile_subtree(&fixture.refile_subtree[iteration])?;
        assert_structural_write_fixture(&report, StructuralWriteOperationKind::RefileSubtree)?;
        black_box(report.affected_files.changed_files.len());
        Ok(())
    })
}

pub(crate) fn benchmark_structural_refile_region(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &StructuralBenchmarkFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.structural_refile_region, |iteration| {
        let report = workbench.refile_region(&fixture.refile_region[iteration])?;
        assert_structural_write_fixture(&report, StructuralWriteOperationKind::RefileRegion)?;
        black_box(report.affected_files.changed_files.len());
        Ok(())
    })
}

pub(crate) fn benchmark_structural_extract_subtree(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &StructuralBenchmarkFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.structural_extract_subtree, |iteration| {
        let report = workbench.extract_subtree(&fixture.extract_subtree[iteration])?;
        assert_structural_write_fixture(&report, StructuralWriteOperationKind::ExtractSubtree)?;
        black_box(report.affected_files.changed_files.len());
        Ok(())
    })
}

pub(crate) fn benchmark_structural_promote_file(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &StructuralBenchmarkFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.structural_promote_file, |iteration| {
        let report = workbench.promote_entire_file(&fixture.promote_file[iteration])?;
        assert_structural_write_fixture(&report, StructuralWriteOperationKind::PromoteFile)?;
        black_box(report.affected_files.changed_files.len());
        Ok(())
    })
}

pub(crate) fn benchmark_structural_demote_file(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &StructuralBenchmarkFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.structural_demote_file, |iteration| {
        let report = workbench.demote_entire_file(&fixture.demote_file[iteration])?;
        assert_structural_write_fixture(&report, StructuralWriteOperationKind::DemoteFile)?;
        black_box(report.affected_files.changed_files.len());
        Ok(())
    })
}

pub(crate) fn prepare_remediation_apply_benchmark_fixture(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<RemediationApplyBenchmarkFixture> {
    let mut expected_missing_ids = BTreeSet::new();
    for iteration in 0..profile.iterations.remediation_apply {
        let missing_id = format!("missing-bench-remediation-apply-{iteration:04}");
        let file_path = format!("bench-remediation/apply/source-{iteration:04}.org");
        write_indexed_bench_file(
            workbench,
            &fixture.root,
            &file_path,
            &format!(
                "#+title: Remediation Apply Source {iteration:04}\n:PROPERTIES:\n:ID: bench-remediation-apply-source-{iteration:04}\n:END:\n\nBroken [[id:{missing_id}][Missing Apply {iteration:04}]].\n"
            ),
        )?;
        expected_missing_ids.insert(missing_id);
    }

    let saved = workbench.save_corpus_audit_review(&SaveCorpusAuditReviewParams {
        audit: CorpusAuditKind::DanglingLinks,
        limit: profile
            .iterations
            .audit_limit
            .max(profile.iterations.remediation_apply + 10),
        review_id: Some(REMEDIATION_APPLY_REVIEW_ID.to_owned()),
        title: Some("Benchmark Remediation Apply Review".to_owned()),
        summary: Some("Per-iteration dangling-link remediation benchmark fixture.".to_owned()),
        overwrite: true,
    })?;
    assert_saved_audit_review_fixture(&saved, REMEDIATION_APPLY_REVIEW_ID)?;
    let review = workbench.review_run(&ReviewRunIdParams {
        review_id: REMEDIATION_APPLY_REVIEW_ID.to_owned(),
    })?;
    let mut apply_params = Vec::with_capacity(profile.iterations.remediation_apply);
    for finding in &review.review.findings {
        let ReviewFindingPayload::Audit { entry } = &finding.payload else {
            continue;
        };
        let CorpusAuditEntry::DanglingLink { record } = entry.as_ref() else {
            continue;
        };
        if !expected_missing_ids.remove(&record.missing_explicit_id) {
            continue;
        }
        let preview = workbench.review_finding_remediation_preview(
            &ReviewFindingRemediationPreviewParams {
                review_id: REMEDIATION_APPLY_REVIEW_ID.to_owned(),
                finding_id: finding.finding_id.clone(),
            },
        )?;
        let action = unlink_dangling_link_action_from_preview(
            &preview.preview,
            &record.missing_explicit_id,
        )?;
        apply_params.push(ReviewFindingRemediationApplyParams {
            review_id: REMEDIATION_APPLY_REVIEW_ID.to_owned(),
            finding_id: finding.finding_id.clone(),
            expected_preview: preview.preview.preview_identity,
            action,
        });
        if apply_params.len() == profile.iterations.remediation_apply {
            break;
        }
    }
    if apply_params.len() != profile.iterations.remediation_apply {
        bail!(
            "remediation apply benchmark prepared {} fixtures, expected {}",
            apply_params.len(),
            profile.iterations.remediation_apply
        );
    }
    Ok(RemediationApplyBenchmarkFixture { apply_params })
}

pub(crate) fn benchmark_remediation_apply(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &RemediationApplyBenchmarkFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.remediation_apply, |iteration| {
        let result =
            workbench.review_finding_remediation_apply(&fixture.apply_params[iteration])?;
        if let Some(error) = result.application.validation_error() {
            bail!("remediation apply benchmark returned invalid application: {error}");
        }
        if result.application.affected_files.changed_files.is_empty() {
            bail!("remediation apply benchmark returned no changed files");
        }
        black_box(result.application.affected_files.changed_files.len());
        Ok(())
    })
}

pub(crate) fn prepare_slipbox_link_rewrite_benchmark_fixture(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<SlipboxLinkRewriteBenchmarkFixture> {
    let preview_target_path = "bench-link-rewrite/preview-target.org";
    write_indexed_bench_file(
        workbench,
        &fixture.root,
        preview_target_path,
        "#+title: Benchmark Link Rewrite Preview Target\n:PROPERTIES:\n:ID: bench-link-rewrite-preview-target\n:END:\n",
    )?;
    let preview_source_path = "bench-link-rewrite/preview-source.org";
    write_indexed_bench_file(
        workbench,
        &fixture.root,
        preview_source_path,
        "#+title: Benchmark Link Rewrite Preview Source\n\nSee [[slipbox:Benchmark Link Rewrite Preview Target][Preview Target]].\n",
    )?;
    let preview_params = SlipboxLinkRewritePreviewParams {
        file_path: preview_source_path.to_owned(),
    };
    assert_slipbox_link_rewrite_preview_fixture(
        &workbench.slipbox_link_rewrite_preview(&preview_params)?,
    )?;

    let mut apply_params = Vec::with_capacity(profile.iterations.slipbox_link_rewrite_apply);
    for iteration in 0..profile.iterations.slipbox_link_rewrite_apply {
        let title = format!("Benchmark Link Rewrite Apply Target {iteration:04}");
        let target_path = format!("bench-link-rewrite/apply-target-{iteration:04}.org");
        let source_path = format!("bench-link-rewrite/apply-source-{iteration:04}.org");
        write_indexed_bench_file(
            workbench,
            &fixture.root,
            &target_path,
            &format!("#+title: {title}\n\nTarget without ID before rewrite apply.\n"),
        )?;
        write_indexed_bench_file(
            workbench,
            &fixture.root,
            &source_path,
            &format!(
                "#+title: Benchmark Link Rewrite Apply Source {iteration:04}\n\nSee [[slipbox:{title}][Apply Target {iteration:04}]].\n"
            ),
        )?;
        let expected_preview = workbench
            .slipbox_link_rewrite_preview(&SlipboxLinkRewritePreviewParams {
                file_path: source_path,
            })?
            .preview;
        if expected_preview.rewrites.is_empty() {
            bail!("slipbox link rewrite apply fixture produced no rewrites");
        }
        apply_params.push(SlipboxLinkRewriteApplyParams { expected_preview });
    }

    Ok(SlipboxLinkRewriteBenchmarkFixture {
        preview_params,
        apply_params,
    })
}

pub(crate) fn benchmark_slipbox_link_rewrite_preview(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &SlipboxLinkRewriteBenchmarkFixture,
) -> Result<TimingReport> {
    let sample = workbench.slipbox_link_rewrite_preview(&fixture.preview_params)?;
    assert_slipbox_link_rewrite_preview_fixture(&sample)?;
    measure_iterations(profile.iterations.slipbox_link_rewrite_preview, |_| {
        let result = workbench.slipbox_link_rewrite_preview(&fixture.preview_params)?;
        assert_slipbox_link_rewrite_preview_fixture(&result)?;
        black_box(result.preview.rewrites.len());
        Ok(())
    })
}

pub(crate) fn benchmark_slipbox_link_rewrite_apply(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &SlipboxLinkRewriteBenchmarkFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.slipbox_link_rewrite_apply, |iteration| {
        let result = workbench.slipbox_link_rewrite_apply(&fixture.apply_params[iteration])?;
        if let Some(error) = result.application.validation_error() {
            bail!("slipbox link rewrite apply benchmark returned invalid application: {error}");
        }
        if result.application.rewrites.is_empty() {
            bail!("slipbox link rewrite apply benchmark returned no rewrites");
        }
        if result.application.affected_files.changed_files.len() < 2 {
            bail!(
                "slipbox link rewrite apply benchmark expected source and target refreshes, got {:?}",
                result.application.affected_files.changed_files
            );
        }
        black_box(result.application.rewrites.len());
        Ok(())
    })
}
