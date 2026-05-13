use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use slipbox_core::{
    AgendaParams, AnchorRecord, AppendHeadingAtOutlinePathParams, AppendHeadingParams,
    AppendHeadingToNodeParams, AuditRemediationApplyAction, AuditRemediationConfidence,
    AuditRemediationPreviewPayload, BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID,
    BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID, BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID,
    BacklinksParams, CaptureContentType, CaptureNodeParams, CaptureTemplateParams,
    CaptureTemplatePreviewParams, CompareNotesParams, CorpusAuditEntry, CorpusAuditKind,
    DanglingLinkAuditRecord, EnsureFileNodeParams, EnsureNodeIdParams,
    ExecuteExplorationArtifactResult, ExplorationArtifactIdParams, ExplorationArtifactMetadata,
    ExplorationArtifactPayload, ExplorationLens, ExploreParams, ExtractSubtreeParams,
    FileDiagnosticsParams, ForwardLinksParams, GraphParams, ImportWorkbenchPackParams,
    IndexFileParams, NodeFromIdParams, NodeFromRefParams, NodeFromTitleOrAliasParams, NodeKind,
    RefileRegionParams, RefileSubtreeParams, ReflinksParams, ReportProfileMetadata,
    ReportProfileMode, ReportProfileSpec, ReportProfileSubject, ReviewFinding,
    ReviewFindingPayload, ReviewFindingRemediationApplyParams,
    ReviewFindingRemediationPreviewParams, ReviewFindingStatus, ReviewRoutineIdParams, ReviewRun,
    ReviewRunDiffParams, ReviewRunIdParams, ReviewRunMetadata, ReviewRunPayload, RewriteFileParams,
    RunReviewRoutineParams, RunWorkflowParams, SaveCorpusAuditReviewParams,
    SaveExplorationArtifactParams, SaveReviewRunParams, SaveWorkflowReviewParams,
    SavedExplorationArtifact, SavedLensViewArtifact, SearchFilesParams, SearchNodesParams,
    SearchOccurrencesParams, SearchRefsParams, SearchTagsParams, StructuralWriteIndexRefreshStatus,
    StructuralWriteOperationKind, StructuralWriteResult, UnlinkedReferencesParams,
    UpdateNodeMetadataParams, ValidateWorkbenchPackParams, WorkbenchPackCompatibility,
    WorkbenchPackIdParams, WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackMetadata,
    WorkflowIdParams, WorkflowInputAssignment, WorkflowResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError, DaemonServeConfig};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::{TempDir, tempdir};

fn daemon_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

fn build_indexed_fixture() -> Result<(TempDir, PathBuf, PathBuf, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        r#":PROPERTIES:
:ID: alpha-id
:ROAM_REFS: cite:shared2024 cite:alpha2024
:END:
#+title: Alpha
#+filetags: :alpha:shared:

See [[id:beta-id][Beta]].
"#,
    )?;
    fs::write(
        root.join("beta.org"),
        r#":PROPERTIES:
:ID: beta-id
:ROAM_REFS: cite:shared2024 cite:beta2024
:END:
#+title: Beta

* TODO Follow Up
:PROPERTIES:
:ID: beta-task-id
:END:
SCHEDULED: <2026-05-03 Sun>

* TODO Anonymous Follow Up
SCHEDULED: <2026-05-04 Mon>
"#,
    )?;
    fs::write(
        root.join("weak.org"),
        r#":PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:shared2024
:END:
#+title: Weak

Weakly integrated peer with shared references and no direct links.
Alpha appears here as unlinked text.
"#,
    )?;
    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;
    let anonymous_anchor_key = database
        .anchor_at_point("beta.org", 13)?
        .context("anonymous heading anchor should exist")?
        .node_key;

    Ok((workspace, root, db, anonymous_anchor_key))
}

fn sample_review_run() -> ReviewRun {
    ReviewRun {
        metadata: ReviewRunMetadata {
            review_id: "review/audit/dangling-links".to_owned(),
            title: "Dangling Link Review".to_owned(),
            summary: Some("Review dangling links".to_owned()),
        },
        payload: ReviewRunPayload::Audit {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 200,
        },
        findings: vec![ReviewFinding {
            finding_id: "audit/dangling-links/source/missing-id".to_owned(),
            status: ReviewFindingStatus::Open,
            payload: ReviewFindingPayload::Audit {
                entry: Box::new(CorpusAuditEntry::DanglingLink {
                    record: Box::new(DanglingLinkAuditRecord {
                        source: AnchorRecord {
                            node_key: "file:source.org".to_owned(),
                            explicit_id: Some("source-id".to_owned()),
                            file_path: "source.org".to_owned(),
                            title: "Source".to_owned(),
                            outline_path: "Source".to_owned(),
                            aliases: Vec::new(),
                            tags: Vec::new(),
                            refs: Vec::new(),
                            todo_keyword: None,
                            scheduled_for: None,
                            deadline_for: None,
                            closed_at: None,
                            level: 0,
                            line: 1,
                            kind: NodeKind::File,
                            file_mtime_ns: 0,
                            backlink_count: 0,
                            forward_link_count: 0,
                        },
                        missing_explicit_id: "missing-id".to_owned(),
                        line: 6,
                        column: 11,
                        preview: "Points to [[id:missing-id][Missing]].".to_owned(),
                    }),
                }),
            },
        }],
    }
}

fn sample_workbench_pack() -> WorkbenchPackManifest {
    WorkbenchPackManifest {
        metadata: WorkbenchPackMetadata {
            pack_id: "pack/daemon-client".to_owned(),
            title: "Daemon Client Pack".to_owned(),
            summary: Some("Pack round trip over stdio".to_owned()),
        },
        compatibility: WorkbenchPackCompatibility::default(),
        workflows: Vec::new(),
        review_routines: Vec::new(),
        report_profiles: vec![ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: "profile/daemon-client/detail".to_owned(),
                title: "Daemon Client Detail".to_owned(),
                summary: None,
            },
            subjects: vec![ReportProfileSubject::Audit],
            mode: ReportProfileMode::Detail,
            status_filters: None,
            diff_buckets: None,
            jsonl_line_kinds: None,
        }],
        entrypoint_routine_ids: Vec::new(),
    }
}

#[test]
fn daemon_client_exposes_everyday_read_operations() -> Result<()> {
    let (_workspace, root, db, _anonymous_anchor_key) = build_indexed_fixture()?;
    let mut client = DaemonClient::spawn(daemon_binary(), &DaemonServeConfig::new(&root, &db))?;

    let stats = client.index()?;
    assert_eq!(stats.files_indexed, 3);
    assert_eq!(stats.nodes_indexed, 5);

    let indexed_file = client.index_file(&IndexFileParams {
        file_path: root.join("alpha.org").display().to_string(),
    })?;
    assert_eq!(indexed_file.file_path, "alpha.org");

    let files = client.indexed_files()?;
    assert_eq!(
        files.files,
        vec![
            "alpha.org".to_owned(),
            "beta.org".to_owned(),
            "weak.org".to_owned()
        ]
    );

    let file_search = client.search_files(&SearchFilesParams {
        query: "Alpha".to_owned(),
        limit: 10,
    })?;
    assert_eq!(file_search.files.len(), 1);
    assert_eq!(file_search.files[0].file_path, "alpha.org");

    let occurrence_search = client.search_occurrences(&SearchOccurrencesParams {
        query: "unlinked text".to_owned(),
        limit: 10,
    })?;
    assert_eq!(occurrence_search.occurrences.len(), 1);
    assert_eq!(occurrence_search.occurrences[0].file_path, "weak.org");

    let tag_search = client.search_tags(&SearchTagsParams {
        query: "sha".to_owned(),
        limit: 10,
    })?;
    assert_eq!(tag_search.tags, vec!["shared".to_owned()]);

    let random = client.random_node()?;
    assert!(random.node.is_some());

    let alpha = client
        .node_from_id(&NodeFromIdParams {
            id: "alpha-id".to_owned(),
        })?
        .context("Alpha should resolve by ID")?;
    let beta = client
        .node_from_id(&NodeFromIdParams {
            id: "beta-id".to_owned(),
        })?
        .context("Beta should resolve by ID")?;

    let beta_anchor = client
        .anchor_at_point(&slipbox_core::NodeAtPointParams {
            file_path: root.join("beta.org").display().to_string(),
            line: 13,
        })?
        .context("anonymous heading anchor should resolve at point")?;
    assert_eq!(beta_anchor.title, "Anonymous Follow Up");

    let backlinks = client.backlinks(&BacklinksParams {
        node_key: beta.node_key.clone(),
        limit: 10,
        unique: false,
    })?;
    assert_eq!(backlinks.backlinks.len(), 1);
    assert_eq!(backlinks.backlinks[0].source_note.title, "Alpha");

    let forward_links = client.forward_links(&ForwardLinksParams {
        node_key: alpha.node_key.clone(),
        limit: 10,
        unique: false,
    })?;
    assert_eq!(forward_links.forward_links.len(), 1);
    assert_eq!(
        forward_links.forward_links[0].destination_note.title,
        "Beta"
    );

    let reflinks = client.reflinks(&ReflinksParams {
        node_key: alpha.node_key.clone(),
        limit: 10,
    })?;
    assert!(
        reflinks
            .reflinks
            .iter()
            .any(|record| record.source_anchor.title == "Weak")
    );

    let unlinked = client.unlinked_references(&UnlinkedReferencesParams {
        node_key: alpha.node_key.clone(),
        limit: 10,
    })?;
    assert!(
        unlinked
            .unlinked_references
            .iter()
            .any(|record| record.source_anchor.title == "Weak")
    );

    let agenda = client.agenda(&AgendaParams {
        start: "2026-05-03T00:00:00".to_owned(),
        end: "2026-05-03T23:59:59".to_owned(),
        limit: 10,
    })?;
    assert_eq!(agenda.nodes.len(), 1);
    assert_eq!(agenda.nodes[0].title, "Follow Up");

    let refs = client.search_refs(&SearchRefsParams {
        query: "beta2024".to_owned(),
        limit: 10,
    })?;
    assert_eq!(refs.refs.len(), 1);
    assert_eq!(refs.refs[0].node.title, "Beta");

    let graph = client.graph_dot(&GraphParams {
        root_node_key: None,
        max_distance: None,
        include_orphans: true,
        hidden_link_types: Vec::new(),
        max_title_length: 100,
        shorten_titles: None,
        node_url_prefix: None,
    })?;
    assert!(
        graph
            .dot
            .contains("\"file:alpha.org\" -> \"file:beta.org\";")
    );

    client.shutdown()?;
    Ok(())
}

#[test]
fn daemon_client_diagnostics_reject_root_escape_paths() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(workspace.path().join("outside.org"), "#+title: Outside\n")?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut client = DaemonClient::spawn(daemon_binary(), &DaemonServeConfig::new(&root, &db))?;

    let error = client
        .diagnose_file(&FileDiagnosticsParams {
            file_path: "../outside.org".to_owned(),
        })
        .expect_err("diagnoseFile must reject paths outside the slipbox root");

    match error {
        DaemonClientError::Rpc(error) => {
            assert_eq!(error.code, -32600);
            assert!(
                error.message.contains("must stay within the slipbox root"),
                "{}",
                error.message
            );
        }
        other => panic!("expected JSON-RPC invalid_request, got {other:?}"),
    }

    client.shutdown()?;
    Ok(())
}

#[test]
fn daemon_client_exposes_everyday_write_operations_with_read_your_writes() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut client = DaemonClient::spawn(daemon_binary(), &DaemonServeConfig::new(&root, &db))?;

    let initial = client.index()?;
    assert_eq!(initial.files_indexed, 0);
    assert_eq!(initial.nodes_indexed, 0);

    let captured = client.capture_node(&CaptureNodeParams {
        title: "Captured Client Note".to_owned(),
        file_path: Some("captured.org".to_owned()),
        head: None,
        refs: vec!["cite:captured2026".to_owned()],
    })?;
    assert_eq!(captured.title, "Captured Client Note");
    assert!(captured.explicit_id.is_some());

    let captured_by_ref = client
        .node_from_ref(&NodeFromRefParams {
            reference: "cite:captured2026".to_owned(),
        })?
        .context("captured note should be immediately ref-resolvable")?;
    assert_eq!(captured_by_ref.node_key, captured.node_key);

    let updated = client.update_node_metadata(&UpdateNodeMetadataParams {
        node_key: captured.node_key.clone(),
        aliases: Some(vec!["Captured Alias".to_owned()]),
        refs: Some(vec![
            "cite:captured2026".to_owned(),
            "cite:updated2026".to_owned(),
        ]),
        tags: Some(vec!["client".to_owned(), "updated".to_owned()]),
    })?;
    assert_eq!(updated.aliases, vec!["Captured Alias".to_owned()]);
    assert_eq!(
        updated.refs,
        vec!["@captured2026".to_owned(), "@updated2026".to_owned()]
    );
    assert_eq!(
        updated.tags,
        vec!["client".to_owned(), "updated".to_owned()]
    );

    let alias_resolved = client
        .node_from_title_or_alias(&NodeFromTitleOrAliasParams {
            title_or_alias: "Captured Alias".to_owned(),
            nocase: false,
        })?
        .context("updated alias should resolve immediately")?;
    assert_eq!(alias_resolved.node_key, captured.node_key);

    let ensured = client.ensure_file_node(&EnsureFileNodeParams {
        file_path: "daily/2026-05-12.org".to_owned(),
        title: "2026-05-12".to_owned(),
    })?;
    assert_eq!(ensured.file_path, "daily/2026-05-12.org");
    assert!(ensured.explicit_id.is_some());

    let appended = client.append_heading(&AppendHeadingParams {
        file_path: "daily/2026-05-12.org".to_owned(),
        title: "2026-05-12".to_owned(),
        heading: "Standup".to_owned(),
        level: 2,
    })?;
    assert_eq!(appended.title, "Standup");
    assert_eq!(appended.kind, NodeKind::Heading);

    let appended_at_point = client
        .anchor_at_point(&slipbox_core::NodeAtPointParams {
            file_path: root.join("daily/2026-05-12.org").display().to_string(),
            line: appended.line,
        })?
        .context("appended heading should be immediately point-resolvable")?;
    assert_eq!(appended_at_point.node_key, appended.node_key);

    let identified_heading = client.ensure_node_id(&EnsureNodeIdParams {
        node_key: appended.node_key.clone(),
    })?;
    assert_eq!(identified_heading.node_key, appended.node_key);
    assert!(identified_heading.explicit_id.is_some());

    let child = client.append_heading_to_node(&AppendHeadingToNodeParams {
        node_key: captured.node_key.clone(),
        heading: "Captured Child".to_owned(),
    })?;
    assert_eq!(child.title, "Captured Child");
    assert_eq!(child.file_path, "captured.org");

    let outline = client.append_heading_at_outline_path(&AppendHeadingAtOutlinePathParams {
        file_path: "projects/review.org".to_owned(),
        heading: "Nested Finding".to_owned(),
        outline_path: vec!["Inbox".to_owned(), "Reviews".to_owned()],
        head: Some("#+title: Review".to_owned()),
    })?;
    assert_eq!(outline.title, "Nested Finding");
    assert_eq!(outline.outline_path, "Inbox / Reviews / Nested Finding");

    let preview = client.capture_template_preview(&CaptureTemplatePreviewParams {
        capture: CaptureTemplateParams {
            title: "Preview".to_owned(),
            file_path: Some("preview.org".to_owned()),
            node_key: None,
            head: None,
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Plain,
            content: "Preview body".to_owned(),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        },
        source_override: None,
        ensure_node_id: false,
    })?;
    assert_eq!(preview.file_path, "preview.org");
    assert!(!root.join("preview.org").exists());

    let templated = client.capture_template(&CaptureTemplateParams {
        title: "Template".to_owned(),
        file_path: Some("template.org".to_owned()),
        node_key: None,
        head: None,
        outline_path: Vec::new(),
        capture_type: CaptureContentType::Plain,
        content: "Template body".to_owned(),
        refs: vec!["cite:template2026".to_owned()],
        prepend: false,
        empty_lines_before: 0,
        empty_lines_after: 0,
        table_line_pos: None,
    })?;
    assert_eq!(templated.title, "Template");
    assert_eq!(templated.file_path, "template.org");

    let occurrences = client.search_occurrences(&SearchOccurrencesParams {
        query: "Template body".to_owned(),
        limit: 10,
    })?;
    assert_eq!(occurrences.occurrences.len(), 1);
    assert_eq!(occurrences.occurrences[0].file_path, "template.org");

    client.shutdown()?;
    Ok(())
}

#[test]
fn daemon_client_exposes_structural_rewrite_operations_over_stdio() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let main_source = r#":PROPERTIES:
:ID: main-id
:END:
#+title: Main

* Region Source
:PROPERTIES:
:ID: region-source-id
:END:
Region body.

* Region Target
:PROPERTIES:
:ID: region-target-id
:END:
Target body.

* Extract Source
:PROPERTIES:
:ID: extract-source-id
:END:
Extract body.
"#;
    fs::write(root.join("main.org"), main_source)?;
    fs::write(
        root.join("cross.org"),
        r#":PROPERTIES:
:ID: cross-id
:END:
#+title: Cross

* Cross Source
:PROPERTIES:
:ID: cross-source-id
:END:
Cross body.
"#,
    )?;
    fs::write(
        root.join("demote.org"),
        r#":PROPERTIES:
:ID: demote-file-id
:END:
#+title: Demote Me

Demote body.
"#,
    )?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut client = DaemonClient::spawn(daemon_binary(), &DaemonServeConfig::new(&root, &db))?;
    client.index()?;

    let region_target = client
        .node_from_id(&NodeFromIdParams {
            id: "region-target-id".to_owned(),
        })?
        .context("region target should resolve")?;
    let region_start = main_source
        .find("* Region Source")
        .context("region source heading should exist")?;
    let region_end = main_source
        .find("\n* Region Target")
        .context("region target heading should follow source")?;
    let region_report = client
        .refile_region(&RefileRegionParams {
            file_path: root.join("main.org").display().to_string(),
            start: main_source[..region_start].chars().count() as u32 + 1,
            end: main_source[..region_end].chars().count() as u32 + 1,
            target_node_key: region_target.node_key.clone(),
        })
        .context("same-file refile region should succeed")?;
    assert_eq!(
        region_report.operation,
        StructuralWriteOperationKind::RefileRegion
    );
    assert_eq!(
        region_report.index_refresh,
        StructuralWriteIndexRefreshStatus::Refreshed
    );
    assert_eq!(
        region_report.affected_files.changed_files,
        vec!["main.org".to_owned()]
    );
    assert!(region_report.affected_files.removed_files.is_empty());
    assert!(region_report.result.is_none());
    assert!(region_report.validation_error().is_none());

    let moved_region = client
        .node_from_id(&NodeFromIdParams {
            id: "region-source-id".to_owned(),
        })?
        .context("same-file region move should stay indexed")?;
    assert_eq!(moved_region.outline_path, "Region Target / Region Source");

    let region_target = client
        .node_from_id(&NodeFromIdParams {
            id: "region-target-id".to_owned(),
        })?
        .context("region target should still resolve after same-file rewrite")?;
    let cross_source = client
        .node_from_id(&NodeFromIdParams {
            id: "cross-source-id".to_owned(),
        })?
        .context("cross-file source should resolve")?;
    let cross_report = client
        .refile_subtree(&RefileSubtreeParams {
            source_node_key: cross_source.node_key,
            target_node_key: region_target.node_key,
        })
        .context("cross-file refile subtree should succeed")?;
    assert_eq!(
        cross_report.operation,
        StructuralWriteOperationKind::RefileSubtree
    );
    assert_eq!(
        cross_report.index_refresh,
        StructuralWriteIndexRefreshStatus::Refreshed
    );
    assert!(
        cross_report
            .affected_files
            .changed_files
            .contains(&"main.org".to_owned())
    );
    assert!(
        cross_report
            .affected_files
            .changed_files
            .contains(&"cross.org".to_owned())
    );
    let StructuralWriteResult::Node { node: cross_node } = cross_report
        .result
        .as_ref()
        .expect("cross-file refile should return moved node")
    else {
        panic!("cross-file refile should return node result");
    };
    assert_eq!(cross_node.title, "Cross Source");
    assert_eq!(cross_node.outline_path, "Region Target / Cross Source");
    assert!(cross_report.validation_error().is_none());

    let extract_source = client
        .node_from_id(&NodeFromIdParams {
            id: "extract-source-id".to_owned(),
        })?
        .context("extract source should resolve")?;
    let extract_report = client
        .extract_subtree(&ExtractSubtreeParams {
            source_node_key: extract_source.node_key,
            file_path: "extracted.org".to_owned(),
        })
        .context("extract subtree should succeed")?;
    assert_eq!(
        extract_report.operation,
        StructuralWriteOperationKind::ExtractSubtree
    );
    assert_eq!(
        extract_report.index_refresh,
        StructuralWriteIndexRefreshStatus::Refreshed
    );
    assert!(
        extract_report
            .affected_files
            .changed_files
            .contains(&"main.org".to_owned())
    );
    assert!(
        extract_report
            .affected_files
            .changed_files
            .contains(&"extracted.org".to_owned())
    );
    let StructuralWriteResult::Node {
        node: extracted_node,
    } = extract_report
        .result
        .as_ref()
        .expect("extract should return new file node")
    else {
        panic!("extract should return node result");
    };
    assert_eq!(extracted_node.kind, NodeKind::File);
    assert_eq!(extracted_node.file_path, "extracted.org");
    assert!(extract_report.validation_error().is_none());

    let demote_report = client
        .demote_entire_file(&RewriteFileParams {
            file_path: "demote.org".to_owned(),
        })
        .context("demote file should succeed")?;
    assert_eq!(
        demote_report.operation,
        StructuralWriteOperationKind::DemoteFile
    );
    assert_eq!(
        demote_report.index_refresh,
        StructuralWriteIndexRefreshStatus::Refreshed
    );
    let StructuralWriteResult::Anchor {
        anchor: demoted_anchor,
    } = demote_report
        .result
        .as_ref()
        .expect("demote should return root anchor")
    else {
        panic!("demote should return anchor result");
    };
    assert_eq!(demoted_anchor.kind, NodeKind::Heading);
    assert_eq!(demoted_anchor.title, "Demote Me");
    assert!(demote_report.validation_error().is_none());

    let promote_report = client
        .promote_entire_file(&RewriteFileParams {
            file_path: "demote.org".to_owned(),
        })
        .context("promote file should succeed")?;
    assert_eq!(
        promote_report.operation,
        StructuralWriteOperationKind::PromoteFile
    );
    assert_eq!(
        promote_report.index_refresh,
        StructuralWriteIndexRefreshStatus::Refreshed
    );
    let StructuralWriteResult::Node {
        node: promoted_node,
    } = promote_report
        .result
        .as_ref()
        .expect("promote should return file node")
    else {
        panic!("promote should return node result");
    };
    assert_eq!(promoted_node.kind, NodeKind::File);
    assert_eq!(promoted_node.file_path, "demote.org");
    assert!(promote_report.validation_error().is_none());

    client.shutdown()?;
    Ok(())
}

#[test]
fn daemon_client_queries_spawned_daemon_and_round_trips_artifacts() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;
    let canonical_root = root.canonicalize()?;
    let mut client = DaemonClient::spawn(daemon_binary(), &DaemonServeConfig::new(&root, &db))?;

    let ping = client.ping()?;
    assert_eq!(ping.root, canonical_root.display().to_string());
    assert_eq!(ping.db, db.display().to_string());

    let status = client.status()?;
    assert_eq!(status.files_indexed, 3);
    assert_eq!(status.nodes_indexed, 5);

    let alpha = client
        .search_nodes(&SearchNodesParams {
            query: "Alpha".to_owned(),
            limit: 10,
            sort: None,
        })?
        .nodes
        .into_iter()
        .find(|node| node.title == "Alpha")
        .context("Alpha note should resolve from search")?;
    let beta = client
        .node_from_id(&NodeFromIdParams {
            id: "beta-id".to_owned(),
        })?
        .context("Beta note should resolve by ID")?;

    let beta_from_title = client
        .node_from_title_or_alias(&NodeFromTitleOrAliasParams {
            title_or_alias: "Beta".to_owned(),
            nocase: false,
        })?
        .context("Beta note should resolve by title")?;
    assert_eq!(beta_from_title.node_key, beta.node_key);

    let beta_from_ref = client
        .node_from_ref(&NodeFromRefParams {
            reference: "cite:beta2024".to_owned(),
        })?
        .context("Beta note should resolve by unique ref")?;
    assert_eq!(beta_from_ref.node_key, beta.node_key);

    let beta_task = client
        .node_at_point(&slipbox_core::NodeAtPointParams {
            file_path: root.join("beta.org").display().to_string(),
            line: 8,
        })?
        .context("Follow Up heading should resolve at point")?;
    assert_eq!(beta_task.title, "Follow Up");

    let explore = client.explore(&ExploreParams {
        node_key: alpha.node_key.clone(),
        lens: ExplorationLens::Structure,
        limit: 10,
        unique: false,
    })?;
    let forward_section = explore
        .sections
        .iter()
        .find(|section| section.kind == slipbox_core::ExplorationSectionKind::ForwardLinks)
        .context("structure lens should include forward links")?;
    assert_eq!(forward_section.entries.len(), 1);

    let comparison = client.compare_notes(&CompareNotesParams {
        left_node_key: alpha.node_key.clone(),
        right_node_key: beta.node_key.clone(),
        limit: 10,
    })?;
    assert_eq!(comparison.left_note.title, "Alpha");
    assert_eq!(comparison.right_note.title, "Beta");
    assert!(
        comparison
            .sections
            .iter()
            .any(|section| { section.kind == slipbox_core::NoteComparisonSectionKind::SharedRefs })
    );

    let workflows = client.list_workflows()?;
    assert_eq!(workflows.workflows.len(), 5);

    let comparison_workflow: WorkflowResult = client.workflow(&WorkflowIdParams {
        workflow_id: BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID.to_owned(),
    })?;
    assert_eq!(
        comparison_workflow.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID
    );
    assert_eq!(comparison_workflow.workflow.inputs.len(), 2);

    let workflow_run = client.run_workflow(&RunWorkflowParams {
        workflow_id: BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID.to_owned(),
        inputs: vec![
            WorkflowInputAssignment {
                input_id: "left".to_owned(),
                target: slipbox_core::WorkflowResolveTarget::NodeKey {
                    node_key: alpha.node_key.clone(),
                },
            },
            WorkflowInputAssignment {
                input_id: "right".to_owned(),
                target: slipbox_core::WorkflowResolveTarget::NodeKey {
                    node_key: beta.node_key.clone(),
                },
            },
        ],
    })?;
    assert_eq!(workflow_run.result.steps.len(), 4);
    assert_eq!(workflow_run.result.steps[2].kind().label(), "compare");

    let unresolved_workflow_run = client.run_workflow(&RunWorkflowParams {
        workflow_id: slipbox_core::BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID.to_owned(),
        inputs: vec![WorkflowInputAssignment {
            input_id: "focus".to_owned(),
            target: slipbox_core::WorkflowResolveTarget::NodeKey {
                node_key: anonymous_anchor_key.clone(),
            },
        }],
    })?;
    match &unresolved_workflow_run.result.steps[2].payload {
        slipbox_core::WorkflowStepReportPayload::Explore {
            focus_node_key,
            result,
        } => {
            assert_eq!(focus_node_key, &anonymous_anchor_key);
            assert_eq!(result.lens, ExplorationLens::Tasks);
        }
        other => panic!("expected tasks explore step, got {:?}", other.kind()),
    }

    let saved = SavedExplorationArtifact {
        metadata: ExplorationArtifactMetadata {
            artifact_id: "alpha-structure".to_owned(),
            title: "Alpha structure".to_owned(),
            summary: Some("Forward structure for Alpha".to_owned()),
        },
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: alpha.node_key.clone(),
                current_node_key: alpha.node_key.clone(),
                lens: ExplorationLens::Structure,
                limit: 10,
                unique: false,
                frozen_context: false,
            }),
        },
    };

    let save = client.save_exploration_artifact(&SaveExplorationArtifactParams {
        artifact: saved.clone(),
        overwrite: true,
    })?;
    assert_eq!(save.artifact.metadata.artifact_id, "alpha-structure");

    let list = client.list_exploration_artifacts()?;
    assert_eq!(list.artifacts.len(), 1);
    assert_eq!(list.artifacts[0].metadata.artifact_id, "alpha-structure");

    let loaded = client.exploration_artifact(&ExplorationArtifactIdParams {
        artifact_id: "alpha-structure".to_owned(),
    })?;
    assert_eq!(loaded.artifact, saved);

    let executed = client.execute_exploration_artifact(&ExplorationArtifactIdParams {
        artifact_id: "alpha-structure".to_owned(),
    })?;
    match executed {
        ExecuteExplorationArtifactResult {
            artifact:
                slipbox_core::ExecutedExplorationArtifact {
                    payload:
                        slipbox_core::ExecutedExplorationArtifactPayload::LensView { result, .. },
                    ..
                },
        } => {
            assert_eq!(result.lens, ExplorationLens::Structure);
            assert!(result.sections.iter().any(|section| {
                section.kind == slipbox_core::ExplorationSectionKind::ForwardLinks
            }));
        }
        other => panic!("unexpected executed artifact payload: {other:?}"),
    }

    let deleted = client.delete_exploration_artifact(&ExplorationArtifactIdParams {
        artifact_id: "alpha-structure".to_owned(),
    })?;
    assert_eq!(deleted.artifact_id, "alpha-structure");
    assert!(client.list_exploration_artifacts()?.artifacts.is_empty());

    let pack = sample_workbench_pack();
    let pack_validation =
        client.validate_workbench_pack(&ValidateWorkbenchPackParams { pack: pack.clone() })?;
    assert!(pack_validation.valid);
    assert!(pack_validation.issues.is_empty());
    assert_eq!(pack_validation.pack, Some(pack.summary()));
    assert!(client.list_workbench_packs()?.packs.is_empty());

    let imported_pack = client.import_workbench_pack(&ImportWorkbenchPackParams {
        pack: pack.clone(),
        overwrite: false,
    })?;
    assert_eq!(imported_pack.pack, pack.summary());
    let pack_conflict = client
        .import_workbench_pack(&ImportWorkbenchPackParams {
            pack: pack.clone(),
            overwrite: false,
        })
        .expect_err("pack import should reject collisions by default");
    match pack_conflict {
        slipbox_daemon_client::DaemonClientError::Rpc(error) => {
            assert_eq!(
                error.message,
                "workbench pack already exists: pack/daemon-client"
            );
        }
        other => panic!("expected RPC conflict for duplicate pack import, got {other:?}"),
    }

    let mut updated_pack = pack.clone();
    updated_pack.metadata.title = "Daemon Client Pack Updated".to_owned();
    let overwritten_pack = client.import_workbench_pack(&ImportWorkbenchPackParams {
        pack: updated_pack.clone(),
        overwrite: true,
    })?;
    assert_eq!(overwritten_pack.pack, updated_pack.summary());

    let listed_packs = client.list_workbench_packs()?;
    assert_eq!(listed_packs.packs, vec![updated_pack.summary()]);
    let loaded_pack = client.workbench_pack(&WorkbenchPackIdParams {
        pack_id: "pack/daemon-client".to_owned(),
    })?;
    assert_eq!(loaded_pack.pack, updated_pack);
    let exported_pack = client.export_workbench_pack(&WorkbenchPackIdParams {
        pack_id: "pack/daemon-client".to_owned(),
    })?;
    assert_eq!(exported_pack, loaded_pack.pack);

    let routines = client.list_review_routines()?;
    assert!(routines.routines.iter().any(|routine| {
        routine.metadata.routine_id == BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID
    }));
    let routine = client.review_routine(&ReviewRoutineIdParams {
        routine_id: BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID.to_owned(),
    })?;
    assert_eq!(
        routine.routine.metadata.routine_id,
        BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID
    );
    let routine_run = client.run_review_routine(&RunReviewRoutineParams {
        routine_id: BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID.to_owned(),
        inputs: Vec::new(),
    })?;
    assert_eq!(
        routine_run.result.routine.metadata.routine_id,
        BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID
    );
    assert!(matches!(
        routine_run.result.source,
        slipbox_core::ReviewRoutineSourceExecutionResult::Audit { .. }
    ));
    let routine_review_id = routine_run
        .result
        .saved_review
        .as_ref()
        .expect("built-in routine should save a review")
        .metadata
        .review_id
        .clone();
    assert_eq!(
        routine_review_id,
        "review/routine/builtin/duplicate-title-review"
    );
    let deleted_routine_review = client.delete_review_run(&ReviewRunIdParams {
        review_id: routine_review_id,
    })?;
    assert_eq!(
        deleted_routine_review.review_id,
        "review/routine/builtin/duplicate-title-review"
    );

    let mut unsupported_pack = exported_pack.clone();
    unsupported_pack.compatibility = WorkbenchPackCompatibility { version: 2 };
    let unsupported_validation = client.validate_workbench_pack(&ValidateWorkbenchPackParams {
        pack: unsupported_pack,
    })?;
    assert!(!unsupported_validation.valid);
    assert_eq!(
        unsupported_validation.issues[0].kind,
        WorkbenchPackIssueKind::UnsupportedVersion
    );

    let deleted_pack = client.delete_workbench_pack(&WorkbenchPackIdParams {
        pack_id: "pack/daemon-client".to_owned(),
    })?;
    assert_eq!(deleted_pack.pack_id, "pack/daemon-client");
    assert!(client.list_workbench_packs()?.packs.is_empty());

    fs::write(
        root.join("source.org"),
        r#":PROPERTIES:
:ID: source-id
:END:
#+title: Source

Points to [[id:missing-id][Missing]].
"#,
    )?;

    let review = sample_review_run();
    let saved_review = client.save_review_run(&SaveReviewRunParams {
        review: review.clone(),
        overwrite: true,
    })?;
    assert_eq!(
        saved_review.review.metadata.review_id,
        "review/audit/dangling-links"
    );
    assert_eq!(saved_review.review.status_counts.open, 1);

    let listed_reviews = client.list_review_runs()?;
    assert_eq!(listed_reviews.reviews, vec![saved_review.review.clone()]);

    let loaded_review = client.review_run(&ReviewRunIdParams {
        review_id: "review/audit/dangling-links".to_owned(),
    })?;
    assert_eq!(loaded_review.review, review);

    let preview =
        client.review_finding_remediation_preview(&ReviewFindingRemediationPreviewParams {
            review_id: "review/audit/dangling-links".to_owned(),
            finding_id: "audit/dangling-links/source/missing-id".to_owned(),
        })?;
    assert_eq!(preview.preview.review_id, "review/audit/dangling-links");
    let action = match &preview.preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview,
            confidence,
            suggestion,
            ..
        } => {
            assert_eq!(missing_explicit_id, "missing-id");
            assert_eq!(*confidence, AuditRemediationConfidence::Medium);
            assert!(suggestion.contains("id:missing-id"));
            AuditRemediationApplyAction::UnlinkDanglingLink {
                source_node_key: source.node_key.clone(),
                missing_explicit_id: missing_explicit_id.clone(),
                file_path: file_path.clone(),
                line: *line,
                column: *column,
                preview: preview.clone(),
                replacement_text: "Missing".to_owned(),
            }
        }
        other => panic!("expected dangling-link remediation preview, got {other:?}"),
    };
    let applied =
        client.review_finding_remediation_apply(&ReviewFindingRemediationApplyParams {
            review_id: "review/audit/dangling-links".to_owned(),
            finding_id: "audit/dangling-links/source/missing-id".to_owned(),
            expected_preview: preview.preview.preview_identity,
            action,
        })?;
    assert_eq!(
        applied.application.affected_files.changed_files,
        vec!["source.org".to_owned()]
    );
    assert!(fs::read_to_string(root.join("source.org"))?.contains("Points to Missing."));

    let loaded_after_apply = client.review_run(&ReviewRunIdParams {
        review_id: "review/audit/dangling-links".to_owned(),
    })?;
    assert_eq!(loaded_after_apply.review, review);

    let mut target_review = review.clone();
    target_review.metadata.review_id = "review/audit/dangling-links/target".to_owned();
    target_review.findings[0].status = ReviewFindingStatus::Reviewed;
    let saved_target_review = client.save_review_run(&SaveReviewRunParams {
        review: target_review,
        overwrite: true,
    })?;
    assert_eq!(
        saved_target_review.review.metadata.review_id,
        "review/audit/dangling-links/target"
    );
    let diff = client.diff_review_runs(&ReviewRunDiffParams {
        base_review_id: "review/audit/dangling-links".to_owned(),
        target_review_id: "review/audit/dangling-links/target".to_owned(),
    })?;
    assert!(diff.diff.added.is_empty());
    assert!(diff.diff.removed.is_empty());
    assert!(diff.diff.unchanged.is_empty());
    assert_eq!(diff.diff.status_changed.len(), 1);
    assert_eq!(
        diff.diff.status_changed[0].finding_id,
        "audit/dangling-links/source/missing-id"
    );

    let marked = client.mark_review_finding(&slipbox_core::MarkReviewFindingParams {
        review_id: "review/audit/dangling-links".to_owned(),
        finding_id: "audit/dangling-links/source/missing-id".to_owned(),
        status: ReviewFindingStatus::Reviewed,
    })?;
    assert_eq!(marked.transition.from_status, ReviewFindingStatus::Open);
    assert_eq!(marked.transition.to_status, ReviewFindingStatus::Reviewed);

    let marked_review = client.review_run(&ReviewRunIdParams {
        review_id: "review/audit/dangling-links".to_owned(),
    })?;
    assert_eq!(
        marked_review.review.findings[0].status,
        ReviewFindingStatus::Reviewed
    );

    let deleted_review = client.delete_review_run(&ReviewRunIdParams {
        review_id: "review/audit/dangling-links".to_owned(),
    })?;
    assert_eq!(deleted_review.review_id, "review/audit/dangling-links");
    let deleted_target_review = client.delete_review_run(&ReviewRunIdParams {
        review_id: "review/audit/dangling-links/target".to_owned(),
    })?;
    assert_eq!(
        deleted_target_review.review_id,
        "review/audit/dangling-links/target"
    );
    assert!(client.list_review_runs()?.reviews.is_empty());

    let audit_review = client.save_corpus_audit_review(&SaveCorpusAuditReviewParams {
        audit: CorpusAuditKind::WeaklyIntegratedNotes,
        limit: 20,
        review_id: Some("review/audit/weakly-integrated-notes".to_owned()),
        title: Some("Weak Integration Review".to_owned()),
        summary: None,
        overwrite: true,
    })?;
    assert_eq!(
        audit_review.result.audit,
        CorpusAuditKind::WeaklyIntegratedNotes
    );
    assert_eq!(
        audit_review.review.metadata.review_id,
        "review/audit/weakly-integrated-notes"
    );

    let workflow_review = client.save_workflow_review(&SaveWorkflowReviewParams {
        workflow_id: BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID.to_owned(),
        inputs: vec![WorkflowInputAssignment {
            input_id: "focus".to_owned(),
            target: slipbox_core::WorkflowResolveTarget::NodeKey {
                node_key: anonymous_anchor_key,
            },
        }],
        review_id: Some("review/workflow/weak-integration".to_owned()),
        title: Some("Weak Integration Review".to_owned()),
        summary: None,
        overwrite: true,
    })?;
    assert_eq!(
        workflow_review.result.workflow.metadata.workflow_id,
        BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID
    );
    assert_eq!(
        workflow_review.review.finding_count,
        workflow_review.result.steps.len()
    );

    let loaded_workflow_review = client.review_run(&ReviewRunIdParams {
        review_id: "review/workflow/weak-integration".to_owned(),
    })?;
    match loaded_workflow_review.review.payload {
        ReviewRunPayload::Workflow {
            workflow,
            inputs,
            step_ids,
        } => {
            assert_eq!(
                workflow.metadata.workflow_id,
                BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID
            );
            assert_eq!(inputs.len(), 1);
            assert_eq!(step_ids.len(), workflow_review.result.steps.len());
            assert!(step_ids.contains(&"review-weak-integration".to_owned()));
        }
        other => panic!("expected workflow review payload, got {:?}", other.kind()),
    }

    client.shutdown()?;
    Ok(())
}

#[test]
fn daemon_client_can_attach_to_a_spawned_daemon_child() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(root.join("gamma.org"), "#+title: Gamma\n")?;
    let db = workspace.path().join("slipbox.sqlite");

    let child = Command::new(daemon_binary())
        .arg("serve")
        .arg("--root")
        .arg(&root)
        .arg("--db")
        .arg(&db)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut client = DaemonClient::from_child(child)?;

    let ping = client.ping()?;
    assert_eq!(ping.root, root.canonicalize()?.display().to_string());

    client.shutdown()?;
    Ok(())
}
