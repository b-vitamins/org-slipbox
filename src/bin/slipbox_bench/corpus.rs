use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use slipbox_core::{
    ExplorationLens, WorkflowExploreFocus, WorkflowInputKind, WorkflowInputSpec, WorkflowMetadata,
    WorkflowResolveTarget as WorkflowSpecResolveTarget, WorkflowSpec, WorkflowSpecCompatibility,
    WorkflowStepPayload, WorkflowStepSpec,
};
use slipbox_store::Database;

use crate::slipbox_bench::constants::{
    EXPLORATION_FOCUS_INDEX, EXPLORATION_FOCUS_REF, EXPLORATION_SHARED_REF, HOT_NODE_ID,
    WORKFLOW_BENCHMARK_ID, WORKFLOW_DISCOVERY_DIR,
};
use crate::slipbox_bench::fixtures::{CorpusFixture, PointQuery};
use crate::slipbox_bench::profile::CorpusConfig;
use crate::slipbox_bench::report::write_json;

pub(crate) fn generate_corpus(workspace: &Path, config: &CorpusConfig) -> Result<CorpusFixture> {
    let total_headings = config
        .files
        .checked_mul(config.headings_per_file)
        .context("benchmark corpus heading count overflowed")?;
    if total_headings < 8 {
        bail!(
            "benchmark corpus requires at least 8 headings to reserve workflow, audit, and exploration fixtures"
        );
    }
    let duplicate_title_upper_index = total_headings - 8;
    let duplicate_title_lower_index = total_headings - 7;
    let dangling_one_index = total_headings - 6;
    let dangling_two_index = total_headings - 5;
    let orphan_index = total_headings - 4;
    let audit_weak_index = total_headings - 3;
    let exploration_unresolved_index = total_headings - 2;
    let exploration_weak_index = total_headings - 1;
    let exploration_node_id = format!("node-{EXPLORATION_FOCUS_INDEX:06}");

    let root = workspace.join("corpus");
    if root.exists() {
        fs::remove_dir_all(&root)
            .with_context(|| format!("failed to clear corpus directory {}", root.display()))?;
    }
    fs::create_dir_all(&root)
        .with_context(|| format!("failed to create corpus directory {}", root.display()))?;

    let notes_dir = root.join("notes");
    fs::create_dir_all(&notes_dir)
        .with_context(|| format!("failed to create notes directory {}", notes_dir.display()))?;
    let workflow_dir = root.join(WORKFLOW_DISCOVERY_DIR);
    fs::create_dir_all(&workflow_dir).with_context(|| {
        format!(
            "failed to create workflow directory {}",
            workflow_dir.display()
        )
    })?;

    let mut search_queries = BTreeSet::new();
    let mut file_queries = BTreeSet::new();
    let mut point_queries = Vec::new();
    let mut workflow_focus_point = None;
    let mut mutable_file = PathBuf::new();
    let mut mutable_relative_path = String::new();
    let mut mutable_template = String::new();
    let mut forward_node_id = None;
    let mut expected_links = 0_usize;

    for file_index in 0..config.files {
        let relative_path = format!("notes/file-{file_index:04}.org");
        let absolute_path = root.join(&relative_path);
        let bucket_tag = format!("bucket{}", file_index % 8);
        let file_title = format!("Bench File {file_index:04}");
        if file_queries.len() < config.query_count {
            file_queries.insert(relative_path.clone());
            file_queries.insert(file_title.clone());
        }
        let mut lines = vec![
            format!("#+title: {file_title}"),
            format!("#+filetags: :bench:{bucket_tag}:"),
            String::from(":PROPERTIES:"),
            format!(":ID: file-{file_index:04}"),
            String::from(":END:"),
            String::new(),
        ];

        for heading_index in 0..config.headings_per_file {
            let global_index = file_index * config.headings_per_file + heading_index;
            let heading_line = (lines.len() + 1) as u32;
            if point_queries.len() < config.query_count {
                point_queries.push(PointQuery {
                    file_path: relative_path.clone(),
                    line: heading_line,
                });
            }

            let is_exploration_focus = global_index == EXPLORATION_FOCUS_INDEX;
            let is_duplicate_title_upper = global_index == duplicate_title_upper_index;
            let is_duplicate_title_lower = global_index == duplicate_title_lower_index;
            let is_dangling_one = global_index == dangling_one_index;
            let is_dangling_two = global_index == dangling_two_index;
            let is_orphan = global_index == orphan_index;
            let is_audit_weak = global_index == audit_weak_index;
            let is_exploration_unresolved = global_index == exploration_unresolved_index;
            let is_exploration_weak = global_index == exploration_weak_index;
            let is_special_fixture = is_exploration_focus
                || is_duplicate_title_upper
                || is_duplicate_title_lower
                || is_dangling_one
                || is_dangling_two
                || is_orphan
                || is_audit_weak
                || is_exploration_unresolved
                || is_exploration_weak;

            let title = if is_exploration_focus {
                "Exploration Focus".to_owned()
            } else if is_duplicate_title_upper {
                "Shared Audit Title".to_owned()
            } else if is_duplicate_title_lower {
                "shared audit title".to_owned()
            } else if is_dangling_one {
                "Dangling Audit One".to_owned()
            } else if is_dangling_two {
                "Dangling Audit Two".to_owned()
            } else if is_orphan {
                "Orphan Audit".to_owned()
            } else if is_audit_weak {
                "Weak Audit".to_owned()
            } else if is_exploration_unresolved {
                "Exploration Unresolved".to_owned()
            } else if is_exploration_weak {
                "Exploration Weak".to_owned()
            } else {
                format!("Bench Topic {global_index:06}")
            };
            let alias = if is_exploration_focus {
                "Exploration Focus Alias".to_owned()
            } else if is_duplicate_title_upper {
                "Shared Audit Title Alias".to_owned()
            } else if is_duplicate_title_lower {
                "shared audit title alias".to_owned()
            } else if is_dangling_one {
                "Dangling Audit One Alias".to_owned()
            } else if is_dangling_two {
                "Dangling Audit Two Alias".to_owned()
            } else if is_orphan {
                "Orphan Audit Alias".to_owned()
            } else if is_audit_weak {
                "Weak Audit Alias".to_owned()
            } else if is_exploration_unresolved {
                "Exploration Unresolved Alias".to_owned()
            } else if is_exploration_weak {
                "Exploration Weak Alias".to_owned()
            } else {
                format!("Alias {global_index:06}")
            };
            let tag = format!("tag{}", global_index % 17);
            let todo = if is_exploration_focus
                || is_audit_weak
                || is_exploration_unresolved
                || (!is_special_fixture && global_index % 4 == 0)
            {
                "TODO "
            } else {
                ""
            };
            let day = (global_index % 28) + 1;

            lines.push(format!("* {todo}{title} :{tag}:{bucket_tag}:"));
            lines.push(String::from(":PROPERTIES:"));
            lines.push(format!(":ID: node-{global_index:06}"));
            lines.push(format!(":ROAM_ALIASES: \"{alias}\""));
            if is_exploration_focus {
                lines.push(format!(
                    ":ROAM_REFS: {EXPLORATION_SHARED_REF} {EXPLORATION_FOCUS_REF}"
                ));
            } else if is_audit_weak || is_exploration_unresolved || is_exploration_weak {
                lines.push(format!(":ROAM_REFS: {EXPLORATION_SHARED_REF}"));
            } else if !is_special_fixture && global_index % config.ref_stride == 0 {
                lines.push(format!(":ROAM_REFS: @cite{global_index:06}"));
            }
            lines.push(String::from(":END:"));
            if is_exploration_focus || is_audit_weak || is_exploration_unresolved {
                lines.push(String::from("SCHEDULED: <2026-03-05 Thu>"));
            } else if !is_special_fixture && global_index % config.scheduled_stride == 0 {
                lines.push(format!("SCHEDULED: <2026-03-{day:02} Tue>"));
            }
            if is_exploration_focus || is_exploration_weak {
                lines.push(String::from("DEADLINE: <2026-03-09 Mon>"));
            } else if !is_special_fixture && global_index % config.deadline_stride == 0 {
                lines.push(format!("DEADLINE: <2026-03-{day:02} Tue>"));
            }
            lines.push(format!("Bench body for {title}."));
            if is_duplicate_title_upper {
                lines.push(format!(
                    "Links to [[id:node-{:06}][matching duplicate]].",
                    duplicate_title_lower_index
                ));
                expected_links += 1;
            } else if is_duplicate_title_lower {
                lines.push(format!(
                    "Links to [[id:node-{:06}][matching duplicate]].",
                    duplicate_title_upper_index
                ));
                expected_links += 1;
            } else if is_dangling_one {
                lines.push(String::from(
                    "Broken [[id:missing-bench-audit-one][missing]].",
                ));
                expected_links += 1;
            } else if is_dangling_two {
                lines.push(String::from(
                    "Broken [[id:missing-bench-audit-two][missing]].",
                ));
                expected_links += 1;
            } else if !is_special_fixture && global_index > 0 {
                lines.push(format!("Prev [[id:node-{:06}][prev]].", global_index - 1));
                expected_links += 1;
                if forward_node_id.is_none() {
                    forward_node_id = Some(format!("node-{global_index:06}"));
                }
            }
            if !is_special_fixture
                && global_index != 0
                && global_index % config.hot_link_stride == 0
            {
                lines.push(format!("Hub [[id:{HOT_NODE_ID}][hub]]."));
                expected_links += 1;
            }
            if !is_special_fixture && global_index % config.hot_link_stride == 0 {
                lines.push(String::from("Reference cite:cite000000."));
                lines.push(String::from("Mention Bench Topic 000000."));
                lines.push(String::from("[[id:node-000000][Bench Topic 000000]]."));
                expected_links += 1;
            }
            lines.push(String::new());

            if is_exploration_focus {
                let workflow_anchor_line = (lines.len() + 1) as u32;
                lines.push(String::from("** TODO Workflow Focus Anchor"));
                lines.push(String::from(":PROPERTIES:"));
                lines.push(format!(
                    ":ROAM_REFS: {EXPLORATION_SHARED_REF} {EXPLORATION_FOCUS_REF}"
                ));
                lines.push(String::from(":END:"));
                lines.push(String::from("SCHEDULED: <2026-03-05 Thu>"));
                lines.push(String::from("DEADLINE: <2026-03-09 Mon>"));
                lines.push(String::from(
                    "Anchor-only focus for workflow benchmark paths.",
                ));
                lines.push(String::new());
                workflow_focus_point = Some(PointQuery {
                    file_path: relative_path.clone(),
                    line: workflow_anchor_line,
                });
            }

            if search_queries.len() < config.query_count {
                search_queries.insert(title);
                search_queries.insert(alias);
                search_queries.insert(tag);
                search_queries.insert(bucket_tag.clone());
            }
        }

        if file_index == 0 {
            lines.push(String::from("Mutable __BENCH_MUTABLE__"));
            lines.push(String::new());
            mutable_file = absolute_path.clone();
            mutable_relative_path = relative_path.clone();
        }

        let rendered = lines.join("\n") + "\n";
        fs::write(&absolute_path, &rendered)
            .with_context(|| format!("failed to write corpus file {}", absolute_path.display()))?;
        if file_index == 0 {
            mutable_template = rendered;
        }
    }

    for workflow_index in 0..config.workflow_specs {
        let path = workflow_dir.join(format!("workflow-{workflow_index:04}.json"));
        let workflow = if workflow_index == 0 {
            benchmark_workflow_spec()
        } else {
            catalog_workflow_spec(workflow_index)
        };
        write_json(&path, &workflow)?;
    }

    Ok(CorpusFixture {
        root,
        workflow_dirs: vec![PathBuf::from(WORKFLOW_DISCOVERY_DIR)],
        mutable_file,
        mutable_relative_path,
        mutable_template,
        hot_node_id: HOT_NODE_ID.to_owned(),
        exploration_node_id,
        workflow_focus_point: workflow_focus_point
            .context("benchmark corpus did not produce a workflow focus anchor")?,
        workflow_specs: config.workflow_specs,
        forward_node_id: forward_node_id
            .context("benchmark corpus did not produce a forward-link source node")?,
        search_queries: search_queries
            .into_iter()
            .take(config.query_count)
            .collect(),
        file_queries: file_queries.into_iter().take(config.query_count).collect(),
        point_queries,
        expected_files: config.files,
        expected_nodes: config.files * (config.headings_per_file + 1) + 1,
        expected_links,
    })
}

pub(crate) fn benchmark_workflow_spec() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: WORKFLOW_BENCHMARK_ID.to_owned(),
            title: "Benchmark Research Sweep".to_owned(),
            summary: Some(
                "Exercise discovery plus rich refs, unresolved, task, and time workflow paths."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus target".to_owned(),
            summary: Some("Note or anchor target to sweep".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowSpecResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-refs".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-unresolved".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Unresolved,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-tasks".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Tasks,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-time".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Time,
                    limit: 25,
                    unique: false,
                },
            },
        ],
    }
}

pub(crate) fn catalog_workflow_spec(workflow_index: usize) -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: format!("workflow/discovered/catalog-{workflow_index:04}"),
            title: format!("Catalog Workflow {workflow_index:04}"),
            summary: Some("Discovery-only catalog workflow for benchmark scale.".to_owned()),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus note".to_owned(),
            summary: Some("Exact note target".to_owned()),
            kind: WorkflowInputKind::NoteTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowSpecResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-structure".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Structure,
                    limit: 15,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-dormant".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Dormant,
                    limit: 15,
                    unique: false,
                },
            },
        ],
    }
}

pub(crate) fn assert_expected_counts(database: &Database, fixture: &CorpusFixture) -> Result<()> {
    let stats = database
        .stats()
        .context("failed to read benchmark index stats")?;
    if stats.files_indexed != fixture.expected_files as u64 {
        bail!(
            "expected {} indexed files, found {}",
            fixture.expected_files,
            stats.files_indexed
        );
    }
    if stats.nodes_indexed != fixture.expected_nodes as u64 {
        bail!(
            "expected {} indexed nodes, found {}",
            fixture.expected_nodes,
            stats.nodes_indexed
        );
    }
    if stats.links_indexed != fixture.expected_links as u64 {
        bail!(
            "expected {} indexed links, found {}",
            fixture.expected_links,
            stats.links_indexed
        );
    }
    Ok(())
}
