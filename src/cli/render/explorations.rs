use slipbox_core::{
    AnchorRecord, ComparisonConnectorDirection, ExecutedExplorationArtifact,
    ExecutedExplorationArtifactPayload, ExplorationArtifactKind, ExplorationArtifactSummary,
    ExplorationEntry, ExplorationExplanation, ExplorationLens, ExplorationSectionKind,
    ExploreResult, ListExplorationArtifactsResult, NodeRecord, NoteComparisonEntry,
    NoteComparisonExplanation, NoteComparisonGroup, NoteComparisonResult,
    NoteComparisonSectionKind, PlanningField, PlanningRelationRecord, SavedComparisonArtifact,
    SavedExplorationArtifact, SavedLensViewArtifact, SavedTrailArtifact, SavedTrailStep,
    TrailReplayResult, TrailReplayStepResult,
};

pub(crate) fn render_saved_artifact_summary(artifact: &ExplorationArtifactSummary) -> String {
    format!(
        "saved artifact: {} [{}]\n",
        artifact.metadata.artifact_id,
        render_artifact_kind(artifact.kind)
    )
}

pub(crate) fn render_explore_result(result: &ExploreResult) -> String {
    let mut output = String::new();
    output.push_str(&format!("lens: {}\n", render_exploration_lens(result.lens)));
    for section in &result.sections {
        output.push('\n');
        output.push_str(&format!(
            "[{}]\n",
            render_exploration_section_kind(section.kind)
        ));
        if section.entries.is_empty() {
            output.push_str("(none)\n");
            continue;
        }
        for entry in &section.entries {
            render_exploration_entry(&mut output, entry);
        }
    }
    output
}

pub(crate) fn render_exploration_lens(lens: ExplorationLens) -> &'static str {
    match lens {
        ExplorationLens::Structure => "structure",
        ExplorationLens::Refs => "refs",
        ExplorationLens::Time => "time",
        ExplorationLens::Tasks => "tasks",
        ExplorationLens::Bridges => "bridges",
        ExplorationLens::Dormant => "dormant",
        ExplorationLens::Unresolved => "unresolved",
    }
}

pub(crate) fn render_exploration_section_kind(kind: ExplorationSectionKind) -> &'static str {
    match kind {
        ExplorationSectionKind::Backlinks => "backlinks",
        ExplorationSectionKind::ForwardLinks => "forward links",
        ExplorationSectionKind::Reflinks => "reflinks",
        ExplorationSectionKind::UnlinkedReferences => "unlinked references",
        ExplorationSectionKind::TimeNeighbors => "time neighbors",
        ExplorationSectionKind::TaskNeighbors => "task neighbors",
        ExplorationSectionKind::BridgeCandidates => "bridge candidates",
        ExplorationSectionKind::DormantNotes => "dormant notes",
        ExplorationSectionKind::UnresolvedTasks => "unresolved tasks",
        ExplorationSectionKind::WeaklyIntegratedNotes => "weakly integrated notes",
    }
}

pub(crate) fn render_exploration_entry(output: &mut String, entry: &ExplorationEntry) {
    match entry {
        ExplorationEntry::Backlink { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_node_identity(&record.source_note),
                record.row,
                record.col
            ));
            if let Some(anchor) = &record.source_anchor {
                output.push_str(&format!("  anchor: {}\n", render_anchor_identity(anchor)));
            }
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::ForwardLink { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_node_identity(&record.destination_note),
                record.row,
                record.col
            ));
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::Reflink { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_anchor_identity(&record.source_anchor),
                record.row,
                record.col
            ));
            output.push_str(&format!(
                "  matched reference: {}\n",
                record.matched_reference
            ));
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::UnlinkedReference { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_anchor_identity(&record.source_anchor),
                record.row,
                record.col
            ));
            output.push_str(&format!("  matched text: {}\n", record.matched_text));
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::Anchor { record } => {
            output.push_str(&format!("- {}\n", render_anchor_identity(&record.anchor)));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
    }
}

pub(crate) fn render_node_identity(node: &NodeRecord) -> String {
    format!(
        "{} [{}] {}:{}",
        node.title, node.node_key, node.file_path, node.line
    )
}

pub(crate) fn render_anchor_identity(anchor: &AnchorRecord) -> String {
    format!(
        "{} [{}] {}:{}",
        anchor.title, anchor.node_key, anchor.file_path, anchor.line
    )
}

pub(crate) fn render_exploration_explanation(explanation: &ExplorationExplanation) -> String {
    match explanation {
        ExplorationExplanation::Backlink => "backlink".to_owned(),
        ExplorationExplanation::ForwardLink => "forward link".to_owned(),
        ExplorationExplanation::SharedReference { reference } => {
            format!("shared reference {reference}")
        }
        ExplorationExplanation::UnlinkedReference { matched_text } => {
            format!("unlinked reference text match {matched_text}")
        }
        ExplorationExplanation::TimeNeighbor { relations } => {
            format!(
                "planning relations {}",
                render_planning_relations(relations)
            )
        }
        ExplorationExplanation::TaskNeighbor {
            shared_todo_keyword,
            planning_relations,
        } => {
            let mut parts = Vec::new();
            if let Some(keyword) = shared_todo_keyword {
                parts.push(format!("shared todo {keyword}"));
            }
            if !planning_relations.is_empty() {
                parts.push(format!(
                    "planning relations {}",
                    render_planning_relations(planning_relations)
                ));
            }
            parts.join("; ")
        }
        ExplorationExplanation::BridgeCandidate {
            references,
            via_notes,
        } => format!(
            "shared references {}; via {}",
            references.join(", "),
            via_notes
                .iter()
                .map(|note| format!("{} [{}]", note.title, note.node_key))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ExplorationExplanation::DormantSharedReference {
            references,
            modified_at_ns,
        } => format!(
            "shared references {}; modified_at_ns {}",
            references.join(", "),
            modified_at_ns
        ),
        ExplorationExplanation::UnresolvedSharedReference {
            references,
            todo_keyword,
        } => format!(
            "shared references {}; todo {}",
            references.join(", "),
            todo_keyword
        ),
        ExplorationExplanation::WeaklyIntegratedSharedReference {
            references,
            structural_link_count,
        } => format!(
            "shared references {}; structural link count {}",
            references.join(", "),
            structural_link_count
        ),
    }
}

pub(crate) fn render_planning_relations(relations: &[PlanningRelationRecord]) -> String {
    relations
        .iter()
        .map(|relation| {
            format!(
                "{}->{} {}",
                render_planning_field(relation.source_field),
                render_planning_field(relation.candidate_field),
                relation.date
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn render_planning_field(field: PlanningField) -> &'static str {
    match field {
        PlanningField::Scheduled => "scheduled",
        PlanningField::Deadline => "deadline",
    }
}

pub(crate) fn render_compare_result(
    result: &NoteComparisonResult,
    group: NoteComparisonGroup,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("group: {}\n", render_comparison_group(group)));
    output.push_str(&format!(
        "left: {}\n",
        render_node_identity(&result.left_note)
    ));
    output.push_str(&format!(
        "right: {}\n",
        render_node_identity(&result.right_note)
    ));
    for section in &result.sections {
        output.push('\n');
        output.push_str(&format!(
            "[{}]\n",
            render_comparison_section_kind(section.kind)
        ));
        if section.entries.is_empty() {
            output.push_str("(none)\n");
            continue;
        }
        for entry in &section.entries {
            render_comparison_entry(&mut output, entry);
        }
    }
    output
}

pub(crate) fn render_artifact_list(result: &ListExplorationArtifactsResult) -> String {
    let mut output = String::new();
    if result.artifacts.is_empty() {
        output.push_str("(none)\n");
        return output;
    }

    for artifact in &result.artifacts {
        output.push_str(&format!(
            "- {} [{}]\n",
            artifact.metadata.title,
            render_artifact_kind(artifact.kind)
        ));
        output.push_str(&format!(
            "  artifact id: {}\n",
            artifact.metadata.artifact_id
        ));
        if let Some(summary) = &artifact.metadata.summary {
            output.push_str(&format!("  summary: {summary}\n"));
        }
    }
    output
}

pub(crate) fn render_saved_exploration_artifact(artifact: &SavedExplorationArtifact) -> String {
    let mut output = String::new();
    render_artifact_metadata(&mut output, &artifact.metadata, artifact.kind());
    match &artifact.payload {
        slipbox_core::ExplorationArtifactPayload::LensView { artifact } => {
            render_saved_lens_view_artifact(&mut output, artifact);
        }
        slipbox_core::ExplorationArtifactPayload::Comparison { artifact } => {
            render_saved_comparison_artifact(&mut output, artifact);
        }
        slipbox_core::ExplorationArtifactPayload::Trail { artifact } => {
            render_saved_trail_artifact(&mut output, artifact);
        }
    }
    output
}

pub(crate) fn render_executed_exploration_artifact(
    artifact: &ExecutedExplorationArtifact,
) -> String {
    let mut output = String::new();
    render_artifact_metadata(&mut output, &artifact.metadata, artifact.kind());
    match &artifact.payload {
        ExecutedExplorationArtifactPayload::LensView {
            artifact,
            root_note,
            current_note,
            result,
        } => {
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            output.push_str(&format!(
                "current: {}\n",
                render_node_identity(current_note)
            ));
            render_saved_lens_view_state(&mut output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_explore_result(result));
        }
        ExecutedExplorationArtifactPayload::Comparison {
            artifact,
            root_note,
            result,
        } => {
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            render_saved_comparison_state(&mut output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_compare_result(result, NoteComparisonGroup::All));
        }
        ExecutedExplorationArtifactPayload::Trail { artifact, replay } => {
            render_saved_trail_state(&mut output, artifact);
            output.push('\n');
            output.push_str("[replay]\n");
            output.push_str(&render_trail_replay_result(replay));
        }
    }
    output
}

pub(crate) fn render_artifact_metadata(
    output: &mut String,
    metadata: &slipbox_core::ExplorationArtifactMetadata,
    kind: ExplorationArtifactKind,
) {
    output.push_str(&format!("artifact id: {}\n", metadata.artifact_id));
    output.push_str(&format!("title: {}\n", metadata.title));
    output.push_str(&format!("kind: {}\n", render_artifact_kind(kind)));
    if let Some(summary) = &metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
}

pub(crate) fn render_saved_lens_view_artifact(
    output: &mut String,
    artifact: &SavedLensViewArtifact,
) {
    render_saved_lens_view_state(output, artifact, "");
}

pub(crate) fn render_saved_lens_view_state(
    output: &mut String,
    artifact: &SavedLensViewArtifact,
    label_prefix: &str,
) {
    output.push_str(&format!(
        "{}root node key: {}\n",
        label_prefix, artifact.root_node_key
    ));
    output.push_str(&format!(
        "{}current node key: {}\n",
        label_prefix, artifact.current_node_key
    ));
    output.push_str(&format!(
        "{}lens: {}\n",
        label_prefix,
        render_exploration_lens(artifact.lens)
    ));
    output.push_str(&format!("{}limit: {}\n", label_prefix, artifact.limit));
    output.push_str(&format!("{}unique: {}\n", label_prefix, artifact.unique));
    output.push_str(&format!(
        "{}frozen context: {}\n",
        label_prefix, artifact.frozen_context
    ));
}

pub(crate) fn render_saved_comparison_artifact(
    output: &mut String,
    artifact: &SavedComparisonArtifact,
) {
    render_saved_comparison_state(output, artifact, "");
}

pub(crate) fn render_saved_comparison_state(
    output: &mut String,
    artifact: &SavedComparisonArtifact,
    label_prefix: &str,
) {
    output.push_str(&format!(
        "{}root node key: {}\n",
        label_prefix, artifact.root_node_key
    ));
    output.push_str(&format!(
        "{}left node key: {}\n",
        label_prefix, artifact.left_node_key
    ));
    output.push_str(&format!(
        "{}right node key: {}\n",
        label_prefix, artifact.right_node_key
    ));
    output.push_str(&format!(
        "{}active lens: {}\n",
        label_prefix,
        render_exploration_lens(artifact.active_lens)
    ));
    output.push_str(&format!(
        "{}comparison group: {}\n",
        label_prefix,
        render_comparison_group(artifact.comparison_group)
    ));
    output.push_str(&format!("{}limit: {}\n", label_prefix, artifact.limit));
    output.push_str(&format!(
        "{}structure unique: {}\n",
        label_prefix, artifact.structure_unique
    ));
    output.push_str(&format!(
        "{}frozen context: {}\n",
        label_prefix, artifact.frozen_context
    ));
}

pub(crate) fn render_saved_trail_artifact(output: &mut String, artifact: &SavedTrailArtifact) {
    render_saved_trail_state(output, artifact);
    for (index, step) in artifact.steps.iter().enumerate() {
        output.push('\n');
        output.push_str(&format!("[step {index}]\n"));
        render_saved_trail_step(output, step);
    }
    if let Some(step) = &artifact.detached_step {
        output.push('\n');
        output.push_str("[detached step]\n");
        render_saved_trail_step(output, step);
    }
}

pub(crate) fn render_saved_trail_state(output: &mut String, artifact: &SavedTrailArtifact) {
    output.push_str(&format!("steps: {}\n", artifact.steps.len()));
    output.push_str(&format!("cursor: {}\n", artifact.cursor));
    output.push_str(&format!(
        "detached step: {}\n",
        if artifact.detached_step.is_some() {
            "present"
        } else {
            "none"
        }
    ));
}

pub(crate) fn render_saved_trail_step(output: &mut String, step: &SavedTrailStep) {
    match step {
        SavedTrailStep::LensView { artifact } => {
            output.push_str("kind: lens-view\n");
            render_saved_lens_view_state(output, artifact, "");
        }
        SavedTrailStep::Comparison { artifact } => {
            output.push_str("kind: comparison\n");
            render_saved_comparison_state(output, artifact, "");
        }
    }
}

pub(crate) fn render_trail_replay_result(replay: &TrailReplayResult) -> String {
    let mut output = String::new();
    output.push_str(&format!("steps: {}\n", replay.steps.len()));
    output.push_str(&format!("cursor: {}\n", replay.cursor));
    output.push_str(&format!(
        "detached step: {}\n",
        if replay.detached_step.is_some() {
            "present"
        } else {
            "none"
        }
    ));
    for (index, step) in replay.steps.iter().enumerate() {
        output.push('\n');
        output.push_str(&format!("[step {index}]\n"));
        render_trail_replay_step(&mut output, step);
    }
    if let Some(step) = &replay.detached_step {
        output.push('\n');
        output.push_str("[detached step]\n");
        render_trail_replay_step(&mut output, step);
    }
    output
}

pub(crate) fn render_trail_replay_step(output: &mut String, step: &TrailReplayStepResult) {
    match step {
        TrailReplayStepResult::LensView {
            artifact,
            root_note,
            current_note,
            result,
        } => {
            output.push_str("kind: lens-view\n");
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            output.push_str(&format!(
                "current: {}\n",
                render_node_identity(current_note)
            ));
            render_saved_lens_view_state(output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_explore_result(result));
        }
        TrailReplayStepResult::Comparison {
            artifact,
            root_note,
            result,
        } => {
            output.push_str("kind: comparison\n");
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            render_saved_comparison_state(output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_compare_result(result, NoteComparisonGroup::All));
        }
    }
}

pub(crate) fn render_artifact_kind(kind: ExplorationArtifactKind) -> &'static str {
    match kind {
        ExplorationArtifactKind::LensView => "lens-view",
        ExplorationArtifactKind::Comparison => "comparison",
        ExplorationArtifactKind::Trail => "trail",
    }
}

pub(crate) fn render_comparison_group(group: NoteComparisonGroup) -> &'static str {
    match group {
        NoteComparisonGroup::All => "all",
        NoteComparisonGroup::Overlap => "overlap",
        NoteComparisonGroup::Divergence => "divergence",
        NoteComparisonGroup::Tension => "tension",
    }
}

pub(crate) fn render_comparison_section_kind(kind: NoteComparisonSectionKind) -> &'static str {
    match kind {
        NoteComparisonSectionKind::SharedRefs => "shared refs",
        NoteComparisonSectionKind::SharedPlanningDates => "shared planning dates",
        NoteComparisonSectionKind::LeftOnlyRefs => "left-only refs",
        NoteComparisonSectionKind::RightOnlyRefs => "right-only refs",
        NoteComparisonSectionKind::SharedBacklinks => "shared backlinks",
        NoteComparisonSectionKind::SharedForwardLinks => "shared forward links",
        NoteComparisonSectionKind::ContrastingTaskStates => "contrasting task states",
        NoteComparisonSectionKind::PlanningTensions => "planning tensions",
        NoteComparisonSectionKind::IndirectConnectors => "indirect connectors",
    }
}

pub(crate) fn render_comparison_entry(output: &mut String, entry: &NoteComparisonEntry) {
    match entry {
        NoteComparisonEntry::Reference { record } => {
            output.push_str(&format!("- {}\n", record.reference));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
        NoteComparisonEntry::Node { record } => {
            output.push_str(&format!("- {}\n", render_node_identity(&record.node)));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
        NoteComparisonEntry::PlanningRelation { record } => {
            output.push_str(&format!(
                "- {} {} <> {} {}\n",
                record.date,
                render_planning_field(record.left_field),
                render_planning_field(record.right_field),
                record.date
            ));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
        NoteComparisonEntry::TaskState { record } => {
            output.push_str(&format!(
                "- {} <> {}\n",
                record.left_todo_keyword, record.right_todo_keyword
            ));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
    }
}

pub(crate) fn render_note_comparison_explanation(
    explanation: &NoteComparisonExplanation,
) -> String {
    match explanation {
        NoteComparisonExplanation::SharedReference => "shared reference".to_owned(),
        NoteComparisonExplanation::SharedPlanningDate => "shared planning date".to_owned(),
        NoteComparisonExplanation::LeftOnlyReference => "left-only reference".to_owned(),
        NoteComparisonExplanation::RightOnlyReference => "right-only reference".to_owned(),
        NoteComparisonExplanation::SharedBacklink => "shared backlink".to_owned(),
        NoteComparisonExplanation::SharedForwardLink => "shared forward link".to_owned(),
        NoteComparisonExplanation::ContrastingTaskState => "contrasting task state".to_owned(),
        NoteComparisonExplanation::PlanningTension => "planning tension".to_owned(),
        NoteComparisonExplanation::IndirectConnector { direction } => {
            format!(
                "indirect connector {}",
                render_connector_direction(*direction)
            )
        }
    }
}

pub(crate) fn render_connector_direction(direction: ComparisonConnectorDirection) -> &'static str {
    match direction {
        ComparisonConnectorDirection::LeftToRight => "left-to-right",
        ComparisonConnectorDirection::RightToLeft => "right-to-left",
        ComparisonConnectorDirection::Bidirectional => "bidirectional",
    }
}
