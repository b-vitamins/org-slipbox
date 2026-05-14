use std::env;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use slipbox_core::{
    AgendaParams, AppendHeadingParams, CaptureNodeParams, GraphParams, IndexFileParams,
    NodeFromIdParams, NodeRecord, SearchNodesParams, SearchNodesSort, SearchOccurrencesParams,
    UpdateNodeMetadataParams,
};
use slipbox_index::{DiscoveryPolicy, scan_path_with_policy, scan_root_with_policy};
use slipbox_store::Database;

use crate::occurrences_query::query_occurrences;
use crate::reflinks_query::query_reflinks;
use crate::slipbox_bench::WorkbenchBench;
use crate::slipbox_bench::constants::{AGENDA_END, AGENDA_START};
use crate::slipbox_bench::corpus::assert_expected_counts;
use crate::slipbox_bench::fixtures::CorpusFixture;
use crate::slipbox_bench::profile::BenchmarkProfile;
use crate::slipbox_bench::report::{
    TimingReport, elapsed_ms, measure_iterations, remove_sqlite_artifacts,
};
use crate::unlinked_references_query::query_unlinked_references;

pub(crate) fn benchmark_full_index(
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
    policy: &DiscoveryPolicy,
) -> Result<TimingReport> {
    let mut samples = Vec::with_capacity(profile.iterations.full_index);
    let bench_root = fixture.root.parent().unwrap_or(fixture.root.as_path());
    for iteration in 0..profile.iterations.full_index {
        let db_path = bench_root.join(format!("full-index-{iteration}.sqlite3"));
        remove_sqlite_artifacts(&db_path)?;
        let start = Instant::now();
        let files = scan_root_with_policy(&fixture.root, policy)
            .context("failed to scan benchmark corpus")?;
        let mut database = Database::open(&db_path)
            .with_context(|| format!("failed to open benchmark database {}", db_path.display()))?;
        database
            .sync_index(&files)
            .context("failed to index benchmark corpus")?;
        assert_expected_counts(&database, fixture)?;
        samples.push(elapsed_ms(start));
    }
    Ok(TimingReport::from_samples(samples))
}

pub(crate) fn prepare_database(
    fixture: &CorpusFixture,
    policy: &DiscoveryPolicy,
) -> Result<Database> {
    let db_path = baseline_db_path(fixture);
    remove_sqlite_artifacts(&db_path)?;
    let files =
        scan_root_with_policy(&fixture.root, policy).context("failed to scan benchmark corpus")?;
    let mut database = Database::open(&db_path)
        .with_context(|| format!("failed to open baseline database {}", db_path.display()))?;
    database
        .sync_index(&files)
        .context("failed to index baseline benchmark corpus")?;
    assert_expected_counts(&database, fixture)?;
    Ok(database)
}

pub(crate) fn baseline_db_path(fixture: &CorpusFixture) -> PathBuf {
    fixture
        .root
        .parent()
        .unwrap_or(fixture.root.as_path())
        .join("baseline.sqlite3")
}

pub(crate) fn benchmark_search_nodes(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.search_nodes, |iteration| {
        let query = &fixture.search_queries[iteration % fixture.search_queries.len()];
        let nodes = database
            .search_nodes(query, profile.iterations.search_limit, None)
            .with_context(|| format!("failed to search nodes for query {query}"))?;
        if nodes.is_empty() {
            bail!("benchmark search query {query} returned no nodes");
        }
        black_box(nodes.len());
        Ok(())
    })
}

pub(crate) fn benchmark_search_nodes_sorted(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    const SORTS: [SearchNodesSort; 5] = [
        SearchNodesSort::Title,
        SearchNodesSort::File,
        SearchNodesSort::FileMtime,
        SearchNodesSort::BacklinkCount,
        SearchNodesSort::ForwardLinkCount,
    ];

    measure_iterations(profile.iterations.search_nodes_sorted, |iteration| {
        let query = &fixture.search_queries[iteration % fixture.search_queries.len()];
        let sort = SORTS[iteration % SORTS.len()].clone();
        let nodes = database
            .search_nodes(query, profile.iterations.search_limit, Some(sort.clone()))
            .with_context(|| {
                format!("failed to search nodes for query {query} with sort {sort:?}")
            })?;
        if nodes.is_empty() {
            bail!("benchmark sorted search query {query} with sort {sort:?} returned no nodes");
        }
        black_box(nodes.len());
        Ok(())
    })
}

pub(crate) fn benchmark_backlinks(
    database: &mut Database,
    profile: &BenchmarkProfile,
    hot_node: &NodeRecord,
) -> Result<TimingReport> {
    let sample = database
        .backlinks(
            &hot_node.node_key,
            profile.iterations.backlinks_limit,
            false,
        )
        .context("failed to fetch backlink sample")?;
    if sample.is_empty() {
        bail!("benchmark hot node produced no backlinks");
    }
    measure_iterations(profile.iterations.backlinks, |_| {
        let backlinks = database
            .backlinks(
                &hot_node.node_key,
                profile.iterations.backlinks_limit,
                false,
            )
            .context("failed to query backlinks")?;
        black_box(backlinks.len());
        Ok(())
    })
}

pub(crate) fn benchmark_search_files(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.search_files, |iteration| {
        let query = &fixture.file_queries[iteration % fixture.file_queries.len()];
        let files = database
            .search_files(query, profile.iterations.search_limit)
            .with_context(|| format!("failed to search files for query {query}"))?;
        if files.is_empty() {
            bail!("benchmark file search query {query} returned no files");
        }
        black_box(files.len());
        Ok(())
    })
}

pub(crate) fn benchmark_search_occurrences(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    let trace_occurrences = env::var_os("SLIPBOX_BENCH_TRACE_OCCURRENCES").is_some();
    measure_iterations(profile.iterations.search_occurrences, |iteration| {
        let query = &fixture.search_queries[iteration % fixture.search_queries.len()];
        let start = Instant::now();
        let occurrences = query_occurrences(database, query, profile.iterations.search_limit)
            .with_context(|| format!("failed to search occurrences for query {query}"))?;
        if occurrences.is_empty() {
            bail!("benchmark occurrence query {query} returned no hits");
        }
        if trace_occurrences {
            eprintln!(
                "searchOccurrences query={query:?} elapsed_ms={:.2} hits={}",
                elapsed_ms(start),
                occurrences.len()
            );
        }
        black_box(occurrences.len());
        Ok(())
    })
}

pub(crate) fn benchmark_forward_links(
    database: &mut Database,
    profile: &BenchmarkProfile,
    source_node: &NodeRecord,
) -> Result<TimingReport> {
    let sample = database
        .forward_links(
            &source_node.node_key,
            profile.iterations.backlinks_limit,
            false,
        )
        .context("failed to fetch forward-link sample")?;
    if sample.is_empty() {
        bail!("benchmark source node produced no forward links");
    }
    measure_iterations(profile.iterations.forward_links, |_| {
        let forward_links = database
            .forward_links(
                &source_node.node_key,
                profile.iterations.backlinks_limit,
                false,
            )
            .context("failed to query forward links")?;
        black_box(forward_links.len());
        Ok(())
    })
}

pub(crate) fn benchmark_reflinks(
    database: &mut Database,
    profile: &BenchmarkProfile,
    root: &Path,
    source_node: &NodeRecord,
) -> Result<TimingReport> {
    let source_anchor = source_node.clone().into();
    let sample = query_reflinks(
        database,
        root,
        &source_anchor,
        profile.iterations.reflinks_limit,
    )
    .context("failed to fetch reflink sample")?;
    if sample.is_empty() {
        bail!("benchmark source node produced no reflinks");
    }
    measure_iterations(profile.iterations.reflinks, |_| {
        let reflinks = query_reflinks(
            database,
            root,
            &source_anchor,
            profile.iterations.reflinks_limit,
        )
        .context("failed to query reflinks")?;
        black_box(reflinks.len());
        Ok(())
    })
}

pub(crate) fn benchmark_unlinked_references(
    database: &mut Database,
    profile: &BenchmarkProfile,
    root: &Path,
    node: &NodeRecord,
) -> Result<TimingReport> {
    let node_anchor = node.clone().into();
    let sample = query_unlinked_references(
        database,
        root,
        &node_anchor,
        profile.iterations.unlinked_references_limit,
    )
    .context("failed to query unlinked references")?;
    if sample.is_empty() {
        bail!("benchmark hot node produced no unlinked references");
    }

    measure_iterations(profile.iterations.unlinked_references, |_| {
        let unlinked_references = query_unlinked_references(
            database,
            root,
            &node_anchor,
            profile.iterations.unlinked_references_limit,
        )
        .context("failed to query unlinked references")?;
        black_box(unlinked_references.len());
        Ok(())
    })
}

pub(crate) fn benchmark_node_at_point(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.node_at_point, |iteration| {
        let point = &fixture.point_queries[iteration % fixture.point_queries.len()];
        let node = database
            .node_at_point(&point.file_path, point.line)
            .with_context(|| {
                format!(
                    "failed to query node at point {}:{}",
                    point.file_path, point.line
                )
            })?;
        if node.is_none() {
            bail!(
                "benchmark node-at-point query {}:{} returned no node",
                point.file_path,
                point.line
            );
        }
        black_box(node);
        Ok(())
    })
}

pub(crate) fn benchmark_agenda(
    database: &mut Database,
    profile: &BenchmarkProfile,
) -> Result<TimingReport> {
    let sample = database
        .agenda_nodes(AGENDA_START, AGENDA_END, profile.iterations.agenda_limit)
        .context("failed to fetch agenda sample")?;
    if sample.is_empty() {
        bail!("benchmark agenda query returned no nodes");
    }
    measure_iterations(profile.iterations.agenda, |_| {
        let nodes = database
            .agenda_nodes(AGENDA_START, AGENDA_END, profile.iterations.agenda_limit)
            .context("failed to query agenda nodes")?;
        black_box(nodes.len());
        Ok(())
    })
}

pub(crate) fn benchmark_index_file(
    database: &mut Database,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
    policy: &DiscoveryPolicy,
) -> Result<TimingReport> {
    let mut samples = Vec::with_capacity(profile.iterations.index_file);
    for iteration in 0..profile.iterations.index_file {
        let source = fixture
            .mutable_template
            .replace("__BENCH_MUTABLE__", &format!("iteration-{iteration:04}"));
        fs::write(&fixture.mutable_file, source).with_context(|| {
            format!(
                "failed to write mutable benchmark file {}",
                fixture.mutable_file.display()
            )
        })?;
        let start = Instant::now();
        let indexed = scan_path_with_policy(&fixture.root, &fixture.mutable_file, policy)
            .with_context(|| {
                format!(
                    "failed to scan mutable benchmark file {}",
                    fixture.mutable_file.display()
                )
            })?;
        database
            .sync_file_index(&indexed)
            .context("failed to sync mutable benchmark file")?;
        samples.push(elapsed_ms(start));
    }

    fs::write(&fixture.mutable_file, &fixture.mutable_template).with_context(|| {
        format!(
            "failed to restore mutable benchmark file {}",
            fixture.mutable_file.display()
        )
    })?;
    let indexed = scan_path_with_policy(&fixture.root, &fixture.mutable_file, policy)
        .with_context(|| {
            format!(
                "failed to rescan mutable benchmark file {}",
                fixture.mutable_file.display()
            )
        })?;
    database
        .sync_file_index(&indexed)
        .context("failed to restore mutable benchmark file in index")?;

    let node = database
        .node_at_point(&fixture.mutable_relative_path, 7)
        .context("failed to verify mutable file after incremental index")?;
    if node.is_none() {
        bail!("mutable benchmark file no longer resolves a node after incremental sync");
    }

    Ok(TimingReport::from_samples(samples))
}

pub(crate) fn benchmark_everyday_file_sync(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    let mut samples = Vec::with_capacity(profile.iterations.everyday_file_sync);
    for iteration in 0..profile.iterations.everyday_file_sync {
        let source = fixture.mutable_template.replace(
            "__BENCH_MUTABLE__",
            &format!("everyday-sync-{iteration:04}"),
        );
        fs::write(&fixture.mutable_file, source).with_context(|| {
            format!(
                "failed to write everyday sync benchmark file {}",
                fixture.mutable_file.display()
            )
        })?;
        let params = IndexFileParams {
            file_path: fixture.mutable_relative_path.clone(),
        };
        let start = Instant::now();
        let result = workbench.index_file(&params)?;
        samples.push(elapsed_ms(start));
        if result.file_path != fixture.mutable_relative_path {
            bail!(
                "everyday file sync returned {}, expected {}",
                result.file_path,
                fixture.mutable_relative_path
            );
        }
    }
    fs::write(&fixture.mutable_file, &fixture.mutable_template).with_context(|| {
        format!(
            "failed to restore everyday sync benchmark file {}",
            fixture.mutable_file.display()
        )
    })?;
    workbench.index_file(&IndexFileParams {
        file_path: fixture.mutable_relative_path.clone(),
    })?;
    Ok(TimingReport::from_samples(samples))
}

pub(crate) fn benchmark_everyday_node_show(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    node: &NodeRecord,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.everyday_node_show, |_| {
        let result = workbench
            .node_from_id(&NodeFromIdParams {
                id: node
                    .explicit_id
                    .clone()
                    .context("benchmark node has no ID")?,
            })?
            .context("everyday node show benchmark returned no node")?;
        if result.node_key != node.node_key {
            bail!(
                "everyday node show returned {}, expected {}",
                result.node_key,
                node.node_key
            );
        }
        black_box(result.title);
        Ok(())
    })
}

pub(crate) fn benchmark_everyday_node_search(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.everyday_node_search, |iteration| {
        let query = &fixture.search_queries[iteration % fixture.search_queries.len()];
        let result = workbench.search_nodes(&SearchNodesParams {
            query: query.clone(),
            limit: profile.iterations.search_limit,
            sort: None,
        })?;
        if result.nodes.is_empty() {
            bail!("everyday node search query {query} returned no nodes");
        }
        black_box(result.nodes.len());
        Ok(())
    })
}

pub(crate) fn benchmark_everyday_occurrence_search(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    fixture: &CorpusFixture,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.everyday_occurrence_search, |iteration| {
        let query = &fixture.search_queries[iteration % fixture.search_queries.len()];
        let result = workbench.search_occurrences(&SearchOccurrencesParams {
            query: query.clone(),
            limit: profile.iterations.search_limit,
        })?;
        if result.occurrences.is_empty() {
            bail!("everyday occurrence search query {query} returned no hits");
        }
        black_box(result.occurrences.len());
        Ok(())
    })
}

pub(crate) fn benchmark_everyday_agenda_range(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.everyday_agenda_range, |_| {
        let result = workbench.agenda(&AgendaParams {
            start: AGENDA_START.to_owned(),
            end: AGENDA_END.to_owned(),
            limit: profile.iterations.agenda_limit,
        })?;
        if result.nodes.is_empty() {
            bail!("everyday agenda range benchmark returned no nodes");
        }
        black_box(result.nodes.len());
        Ok(())
    })
}

pub(crate) fn benchmark_everyday_graph_dot(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    node: &NodeRecord,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.everyday_graph_dot, |_| {
        let result = workbench.graph_dot(&GraphParams {
            root_node_key: Some(node.node_key.clone()),
            max_distance: Some(2),
            include_orphans: false,
            hidden_link_types: Vec::new(),
            max_title_length: 60,
            shorten_titles: None,
            node_url_prefix: None,
        })?;
        if !result.dot.contains("digraph") {
            bail!("everyday graph DOT benchmark returned malformed DOT");
        }
        black_box(result.dot.len());
        Ok(())
    })
}

pub(crate) fn benchmark_everyday_capture_create(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.everyday_capture_create, |iteration| {
        let title = format!("Benchmark Capture {iteration:04}");
        let result = workbench.capture_node(&CaptureNodeParams {
            title: title.clone(),
            file_path: None,
            head: None,
            refs: vec![format!("bench-capture-{iteration:04}")],
        })?;
        if result.title != title {
            bail!(
                "everyday capture benchmark returned title {}, expected {}",
                result.title,
                title
            );
        }
        black_box(result.node_key);
        Ok(())
    })
}

pub(crate) fn benchmark_everyday_daily_append(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.everyday_daily_append, |iteration| {
        let heading = format!("Benchmark Daily Entry {iteration:04}");
        let result = workbench.append_heading(&AppendHeadingParams {
            file_path: format!("daily/bench-2026-03-{:02}.org", (iteration % 28) + 1),
            title: format!("Benchmark Daily {}", (iteration % 28) + 1),
            heading: heading.clone(),
            level: 1,
        })?;
        if result.title != heading {
            bail!(
                "everyday daily append returned title {}, expected {}",
                result.title,
                heading
            );
        }
        black_box(result.node_key);
        Ok(())
    })
}

pub(crate) fn benchmark_everyday_metadata_update(
    workbench: &mut WorkbenchBench,
    profile: &BenchmarkProfile,
    node: &NodeRecord,
) -> Result<TimingReport> {
    measure_iterations(profile.iterations.everyday_metadata_update, |iteration| {
        let alias = format!("Benchmark Alias {iteration:04}");
        let result = workbench.update_node_metadata(&UpdateNodeMetadataParams {
            node_key: node.node_key.clone(),
            aliases: Some(vec![alias.clone()]),
            refs: None,
            tags: None,
        })?;
        if !result.aliases.iter().any(|candidate| candidate == &alias) {
            bail!("everyday metadata update did not persist alias {alias}");
        }
        black_box(result.aliases.len());
        Ok(())
    })
}
