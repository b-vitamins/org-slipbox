/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

@file:OptIn(ExperimentalSerializationApi::class)

package io.github.b_vitamins.slipbox.engine

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonClassDiscriminator
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

// Kotlin mirrors of canonical engine results, paired with their operation kinds.

/** An indexed file or heading. The engine's anchor record has the same shape. */
@Serializable
data class NodeRecord(
    @SerialName("node_key") val nodeKey: String,
    @SerialName("explicit_id") val explicitId: String?,
    @SerialName("file_path") val filePath: String,
    val title: String,
    @SerialName("outline_path") val outlinePath: String,
    val aliases: List<String>,
    val tags: List<String>,
    val refs: List<String>,
    @SerialName("todo_keyword") val todoKeyword: String?,
    @SerialName("scheduled_for") val scheduledFor: String?,
    @SerialName("deadline_for") val deadlineFor: String?,
    @SerialName("closed_at") val closedAt: String?,
    val glossary: Boolean,
    @SerialName("glossary_status") val glossaryStatus: String?,
    @SerialName("sr_due") val srDue: String?,
    @SerialName("sr_ease") val srEase: String?,
    @SerialName("sr_interval") val srInterval: String?,
    @SerialName("sr_reps") val srReps: String?,
    @SerialName("sr_last") val srLast: String?,
    val level: Long,
    val line: Long,
    val kind: NodeKind,
    @SerialName("file_mtime_ns") val fileMtimeNs: Long,
    @SerialName("backlink_count") val backlinkCount: Long,
    @SerialName("forward_link_count") val forwardLinkCount: Long,
)

@Serializable
enum class NodeKind {
    @SerialName("file")
    FILE,

    @SerialName("heading")
    HEADING,
}

@Serializable
data class StatusInfo(
    val version: String,
    val root: String,
    val db: String,
    @SerialName("files_indexed") val filesIndexed: Long,
    @SerialName("nodes_indexed") val nodesIndexed: Long,
    @SerialName("notes_indexed") val notesIndexed: Long,
    @SerialName("links_indexed") val linksIndexed: Long,
)

@Serializable
data class IndexedFilesResult(
    val files: List<String>,
)

@Serializable
data class SearchNodesResult(
    val nodes: List<NodeRecord>,
)

@Serializable
data class ContentSegment(
    val text: String,
    val matched: Boolean,
)

/** The segments concatenate to the excerpt, matched runs marked. */
@Serializable
data class ContentSnippet(
    val segments: List<ContentSegment>,
)

@Serializable
data class NodeContentHit(
    val node: NodeRecord,
    val snippet: ContentSnippet,
)

@Serializable
data class SearchNodeContentResult(
    val hits: List<NodeContentHit>,
)

@Serializable
data class SourceSlice(
    @SerialName("file_path") val filePath: String,
    @SerialName("start_line") val startLine: Long,
    @SerialName("line_count") val lineCount: Long,
    @SerialName("total_lines") val totalLines: Long,
    val content: String,
    @SerialName("truncated_before") val truncatedBefore: Boolean,
    @SerialName("truncated_after") val truncatedAfter: Boolean,
)

@Serializable
data class ReadNodeSourceResult(
    val anchor: NodeRecord,
    val source: SourceSlice,
    @SerialName("node_start_line") val nodeStartLine: Long,
    @SerialName("node_line_count") val nodeLineCount: Long,
)

@Serializable
data class ListGlossaryTermsResult(
    val terms: List<NodeRecord>,
    val total: Long,
    @SerialName("has_more") val hasMore: Boolean,
    @SerialName("next_position") val nextPosition: String?,
)

@Serializable
data class SearchGlossaryResult(
    val terms: List<NodeRecord>,
    val total: Long,
    @SerialName("has_more") val hasMore: Boolean,
)

/** One glossary lookup; the canonical lookup admits absence, so a term may be null. */
@Serializable
data class GlossaryTermResult(
    val term: NodeRecord?,
)

@Serializable
data class BacklinksResult(
    val backlinks: List<BacklinkRecord>,
)

@Serializable
data class BacklinkRecord(
    @SerialName("source_note") val sourceNote: NodeRecord,
    @SerialName("source_anchor") val sourceAnchor: NodeRecord?,
    val row: Long,
    val col: Long,
    val preview: String,
    val explanation: ExplorationExplanation,
)

@Serializable
data class ForwardLinksResult(
    @SerialName("forward_links") val forwardLinks: List<ForwardLinkRecord>,
)

@Serializable
data class ForwardLinkRecord(
    @SerialName("destination_note") val destinationNote: NodeRecord,
    val row: Long,
    val col: Long,
    val preview: String,
    val explanation: ExplorationExplanation,
)

@Serializable
data class ReflinkRecord(
    @SerialName("source_anchor") val sourceAnchor: NodeRecord,
    val row: Long,
    val col: Long,
    val preview: String,
    @SerialName("matched_reference") val matchedReference: String,
    val explanation: ExplorationExplanation,
)

@Serializable
data class UnlinkedReferenceRecord(
    @SerialName("source_note") val sourceNote: NodeRecord,
    @SerialName("source_anchor") val sourceAnchor: NodeRecord,
    val row: Long,
    val col: Long,
    val preview: String,
    @SerialName("matched_text") val matchedText: String,
    val explanation: ExplorationExplanation,
)

@Serializable
data class BridgeEvidenceRecord(
    @SerialName("node_key") val nodeKey: String,
    @SerialName("explicit_id") val explicitId: String?,
    val title: String,
)

@Serializable
data class PlanningRelationRecord(
    @SerialName("source_field") val sourceField: PlanningField,
    @SerialName("candidate_field") val candidateField: PlanningField,
    val date: String,
)

@Serializable
enum class PlanningField {
    @SerialName("scheduled")
    SCHEDULED,

    @SerialName("deadline")
    DEADLINE,
}

/** Why the engine relates one record to the note being explored. */
@Serializable
@JsonClassDiscriminator(OPERATION_DISCRIMINATOR)
sealed class ExplorationExplanation {

    @Serializable
    @SerialName("backlink")
    data object Backlink : ExplorationExplanation()

    @Serializable
    @SerialName("forward-link")
    data object ForwardLink : ExplorationExplanation()

    @Serializable
    @SerialName("shared-reference")
    data class SharedReference(
        val reference: String,
    ) : ExplorationExplanation()

    @Serializable
    @SerialName("unlinked-reference")
    data class UnlinkedReference(
        @SerialName("matched_text") val matchedText: String,
    ) : ExplorationExplanation()

    @Serializable
    @SerialName("time-neighbor")
    data class TimeNeighbor(
        val relations: List<PlanningRelationRecord>,
    ) : ExplorationExplanation()

    @Serializable
    @SerialName("task-neighbor")
    data class TaskNeighbor(
        @SerialName("shared_todo_keyword") val sharedTodoKeyword: String?,
        @SerialName("planning_relations") val planningRelations: List<PlanningRelationRecord>,
    ) : ExplorationExplanation()

    @Serializable
    @SerialName("bridge-candidate")
    data class BridgeCandidate(
        val references: List<String>,
        @SerialName("via_notes") val viaNotes: List<BridgeEvidenceRecord>,
    ) : ExplorationExplanation()

    @Serializable
    @SerialName("dormant-shared-reference")
    data class DormantSharedReference(
        val references: List<String>,
        @SerialName("modified_at_ns") val modifiedAtNs: Long,
        @SerialName("via_notes") val viaNotes: List<BridgeEvidenceRecord>,
    ) : ExplorationExplanation()

    @Serializable
    @SerialName("unresolved-shared-reference")
    data class UnresolvedSharedReference(
        val references: List<String>,
        @SerialName("todo_keyword") val todoKeyword: String,
    ) : ExplorationExplanation()

    @Serializable
    @SerialName("weakly-integrated-shared-reference")
    data class WeaklyIntegratedSharedReference(
        val references: List<String>,
        @SerialName("structural_link_count") val structuralLinkCount: Long,
        @SerialName("via_notes") val viaNotes: List<BridgeEvidenceRecord>,
    ) : ExplorationExplanation()
}

@Serializable
data class AnchorExplorationRecord(
    val anchor: NodeRecord,
    val explanation: ExplorationExplanation,
)

@Serializable
enum class ExplorationSectionKind {
    @SerialName("backlinks")
    BACKLINKS,

    @SerialName("forward-links")
    FORWARD_LINKS,

    @SerialName("reflinks")
    REFLINKS,

    @SerialName("unlinked-references")
    UNLINKED_REFERENCES,

    @SerialName("time-neighbors")
    TIME_NEIGHBORS,

    @SerialName("task-neighbors")
    TASK_NEIGHBORS,

    @SerialName("bridge-candidates")
    BRIDGE_CANDIDATES,

    @SerialName("dormant-notes")
    DORMANT_NOTES,

    @SerialName("unresolved-tasks")
    UNRESOLVED_TASKS,

    @SerialName("weakly-integrated-notes")
    WEAKLY_INTEGRATED_NOTES,
}

/**
 * Exploration records are flattened beside their kind, requiring a custom serializer.
 */
@Serializable(with = ExplorationEntrySerializer::class)
sealed class ExplorationEntry {

    data class Backlink(val record: BacklinkRecord) : ExplorationEntry()

    data class ForwardLink(val record: ForwardLinkRecord) : ExplorationEntry()

    data class Reflink(val record: ReflinkRecord) : ExplorationEntry()

    data class UnlinkedReference(val record: UnlinkedReferenceRecord) : ExplorationEntry()

    data class Anchor(val record: AnchorExplorationRecord) : ExplorationEntry()
}

@Serializable
data class ExplorationSection(
    val kind: ExplorationSectionKind,
    val entries: List<ExplorationEntry>,
)

@Serializable
data class ExploreResult(
    val lens: ExplorationLens,
    val sections: List<ExplorationSection>,
)

@Serializable
data class IndexStats(
    @SerialName("files_indexed") val filesIndexed: Long,
    @SerialName("nodes_indexed") val nodesIndexed: Long,
    @SerialName("links_indexed") val linksIndexed: Long,
)

@Serializable
data class IndexFileResult(
    @SerialName("file_path") val filePath: String,
)

/** The answer of an operation a reading session admits. */
sealed interface ReadAnswer

/** The answer of an operation a maintenance session admits. */
sealed interface MaintenanceAnswer

/**
 * A canonical engine result paired with its operation kind.
 */
@Serializable
@JsonClassDiscriminator(OPERATION_DISCRIMINATOR)
sealed class EngineAnswer {

    @Serializable
    @SerialName("status")
    data class Status(val result: StatusInfo) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("indexedFiles")
    data class IndexedFiles(val result: IndexedFilesResult) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("searchNodes")
    data class SearchNodes(val result: SearchNodesResult) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("searchNodeContent")
    data class SearchNodeContent(val result: SearchNodeContentResult) : EngineAnswer(), ReadAnswer

    /** The canonical lookup admits absence, so an absent node is a null result. */
    @Serializable
    @SerialName("nodeFromId")
    data class NodeFromId(val result: NodeRecord?) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("nodeFromKey")
    data class NodeFromKey(val result: NodeRecord?) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("readNodeSource")
    data class ReadNodeSource(val result: ReadNodeSourceResult) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("listGlossaryTerms")
    data class ListGlossaryTerms(val result: ListGlossaryTermsResult) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("searchGlossary")
    data class SearchGlossary(val result: SearchGlossaryResult) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("glossaryTerm")
    data class GlossaryTerm(val result: GlossaryTermResult) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("backlinks")
    data class Backlinks(val result: BacklinksResult) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("forwardLinks")
    data class ForwardLinks(val result: ForwardLinksResult) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("explore")
    data class Explore(val result: ExploreResult) : EngineAnswer(), ReadAnswer

    @Serializable
    @SerialName("index")
    data class Index(val result: IndexStats) : EngineAnswer(), MaintenanceAnswer

    @Serializable
    @SerialName("indexFile")
    data class IndexFile(val result: IndexFileResult) : EngineAnswer(), MaintenanceAnswer
}

/** [answer] as this operation's own answer, or null if it answers another one. */
internal fun ReadOperation.answerIn(answer: EngineAnswer): ReadAnswer? =
    when (this) {
        is ReadOperation.Status -> answer as? EngineAnswer.Status
        is ReadOperation.IndexedFiles -> answer as? EngineAnswer.IndexedFiles
        is ReadOperation.SearchNodes -> answer as? EngineAnswer.SearchNodes
        is ReadOperation.SearchNodeContent -> answer as? EngineAnswer.SearchNodeContent
        is ReadOperation.NodeFromId -> answer as? EngineAnswer.NodeFromId
        is ReadOperation.NodeFromKey -> answer as? EngineAnswer.NodeFromKey
        is ReadOperation.ReadNodeSource -> answer as? EngineAnswer.ReadNodeSource
        is ReadOperation.ListGlossaryTerms -> answer as? EngineAnswer.ListGlossaryTerms
        is ReadOperation.SearchGlossary -> answer as? EngineAnswer.SearchGlossary
        is ReadOperation.GlossaryTerm -> answer as? EngineAnswer.GlossaryTerm
        is ReadOperation.Backlinks -> answer as? EngineAnswer.Backlinks
        is ReadOperation.ForwardLinks -> answer as? EngineAnswer.ForwardLinks
        is ReadOperation.Explore -> answer as? EngineAnswer.Explore
    }

/** [answer] as this operation's own answer, or null if it answers another one. */
internal fun MaintenanceOperation.answerIn(answer: EngineAnswer): MaintenanceAnswer? =
    when (this) {
        is MaintenanceOperation.Index -> answer as? EngineAnswer.Index
        is MaintenanceOperation.IndexFile -> answer as? EngineAnswer.IndexFile
    }

private const val BACKLINK_ENTRY = "backlink"
private const val FORWARD_LINK_ENTRY = "forward-link"
private const val REFLINK_ENTRY = "reflink"
private const val UNLINKED_REFERENCE_ENTRY = "unlinked-reference"
private const val ANCHOR_ENTRY = "anchor"

/** Reads and writes one [ExplorationEntry] as its discriminant and its record. */
internal object ExplorationEntrySerializer : KSerializer<ExplorationEntry> {

    override val descriptor: SerialDescriptor =
        buildClassSerialDescriptor("io.github.b_vitamins.slipbox.engine.ExplorationEntry")

    override fun deserialize(decoder: Decoder): ExplorationEntry {
        val input = decoder as? JsonDecoder ?: throw notJson()
        val entry = input.decodeJsonElement().jsonObject
        val named = entry[OPERATION_DISCRIMINATOR]?.jsonPrimitive?.contentOrNull
        val record = JsonObject(entry - OPERATION_DISCRIMINATOR)
        return when (named) {
            BACKLINK_ENTRY ->
                ExplorationEntry.Backlink(
                    input.json.decodeFromJsonElement(BacklinkRecord.serializer(), record),
                )

            FORWARD_LINK_ENTRY ->
                ExplorationEntry.ForwardLink(
                    input.json.decodeFromJsonElement(ForwardLinkRecord.serializer(), record),
                )

            REFLINK_ENTRY ->
                ExplorationEntry.Reflink(
                    input.json.decodeFromJsonElement(ReflinkRecord.serializer(), record),
                )

            UNLINKED_REFERENCE_ENTRY ->
                ExplorationEntry.UnlinkedReference(
                    input.json.decodeFromJsonElement(UnlinkedReferenceRecord.serializer(), record),
                )

            ANCHOR_ENTRY ->
                ExplorationEntry.Anchor(
                    input.json.decodeFromJsonElement(AnchorExplorationRecord.serializer(), record),
                )

            else -> throw SerializationException("an exploration entry names no admitted kind")
        }
    }

    override fun serialize(encoder: Encoder, value: ExplorationEntry) {
        val output = encoder as? JsonEncoder ?: throw notJson()
        val named: String
        val record: JsonObject
        when (value) {
            is ExplorationEntry.Backlink -> {
                named = BACKLINK_ENTRY
                record = written(output, BacklinkRecord.serializer(), value.record)
            }

            is ExplorationEntry.ForwardLink -> {
                named = FORWARD_LINK_ENTRY
                record = written(output, ForwardLinkRecord.serializer(), value.record)
            }

            is ExplorationEntry.Reflink -> {
                named = REFLINK_ENTRY
                record = written(output, ReflinkRecord.serializer(), value.record)
            }

            is ExplorationEntry.UnlinkedReference -> {
                named = UNLINKED_REFERENCE_ENTRY
                record = written(output, UnlinkedReferenceRecord.serializer(), value.record)
            }

            is ExplorationEntry.Anchor -> {
                named = ANCHOR_ENTRY
                record = written(output, AnchorExplorationRecord.serializer(), value.record)
            }
        }
        val discriminant = mapOf(OPERATION_DISCRIMINATOR to JsonPrimitive(named))
        output.encodeJsonElement(JsonObject(discriminant + record))
    }

    private fun <T> written(
        output: JsonEncoder,
        serializer: KSerializer<T>,
        record: T,
    ): JsonObject = output.json.encodeToJsonElement(serializer, record).jsonObject

    private fun notJson(): SerializationException =
        SerializationException("an exploration entry is only read from and written to JSON")
}
