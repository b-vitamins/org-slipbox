/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import io.github.b_vitamins.slipbox.engine.BridgeEvidenceRecord
import io.github.b_vitamins.slipbox.engine.ContentSegment
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.ExplorationEntry
import io.github.b_vitamins.slipbox.engine.ExplorationExplanation
import io.github.b_vitamins.slipbox.engine.ExplorationLens
import io.github.b_vitamins.slipbox.engine.ExplorationSectionKind
import io.github.b_vitamins.slipbox.engine.ExploreResult
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.engine.UnlinkedReferenceRecord
import io.github.b_vitamins.slipbox.engine.UnlinkedReferencesResult
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal const val RELATED_QUERY_LIMIT = 50
internal const val RELATED_DISPLAY_LIMIT = 8
internal const val MENTIONS_QUERY_LIMIT = 200
internal const val MENTIONS_DISPLAY_LIMIT = 8
private const val MENTION_CONTEXT_BEFORE = 24
private const val RELATION_PREVIEW_CHARACTERS = 120

internal sealed interface RelatedDiscoveryPhase {
    data object Idle : RelatedDiscoveryPhase
    data object Loading : RelatedDiscoveryPhase
    data object Empty : RelatedDiscoveryPhase
    data class Ready(val result: ExploreResult, val showAll: Boolean = false) : RelatedDiscoveryPhase
    data object Failed : RelatedDiscoveryPhase
}

internal sealed interface MentionDiscoveryPhase {
    data object Idle : MentionDiscoveryPhase
    data object Loading : MentionDiscoveryPhase
    data object Empty : MentionDiscoveryPhase
    data class Ready(
        val result: UnlinkedReferencesResult,
        val showAll: Boolean = false,
    ) : MentionDiscoveryPhase
    data object Failed : MentionDiscoveryPhase
}

internal data class RelatedNote(
    val note: NodeRecord,
    val explanation: ExplorationExplanation.BridgeCandidate,
)

internal data class RelatedGroup(
    val connector: BridgeEvidenceRecord,
    val notes: List<RelatedNote>,
)

internal data class RelatedDiscovery(
    val groups: List<RelatedGroup>,
    val rankedCount: Int,
) {
    val noteCount: Int = groups.sumOf { it.notes.size }
}

internal data class MentionOccurrence(
    val record: UnlinkedReferenceRecord,
    val excerpt: List<ContentSegment>,
)

internal data class MentionGroup(
    val note: NodeRecord,
    val occurrences: List<MentionOccurrence>,
)

/** Project bridge candidates without repeating notes already named by directed relations. */
internal fun relatedDiscovery(
    result: ExploreResult,
    listed: Set<String>,
): RelatedDiscovery {
    val groups = linkedMapOf<String, Pair<BridgeEvidenceRecord, MutableList<RelatedNote>>>()
    val placed = mutableSetOf<String>()
    var ranked = 0
    result.sections
        .asSequence()
        .filter { it.kind == ExplorationSectionKind.BRIDGE_CANDIDATES }
        .flatMap { it.entries.asSequence() }
        .forEach { entry ->
            val record = (entry as? ExplorationEntry.Anchor)?.record ?: return@forEach
            val explanation = record.explanation as? ExplorationExplanation.BridgeCandidate
                ?: return@forEach
            val connector = explanation.viaNotes.firstOrNull() ?: return@forEach
            ranked += 1
            if (record.anchor.nodeKey in listed || !placed.add(record.anchor.nodeKey)) return@forEach
            val group = groups.getOrPut(connector.nodeKey) { connector to mutableListOf() }
            group.second += RelatedNote(record.anchor, explanation)
        }
    return RelatedDiscovery(
        groups = groups.values.map { (connector, notes) -> RelatedGroup(connector, notes) },
        rankedCount = ranked,
    )
}

/** Keep the strongest ranked notes while preserving their connector groups and source order. */
internal fun RelatedDiscovery.bounded(limit: Int): List<RelatedGroup> {
    if (limit >= noteCount) return groups
    val kept =
        groups
            .flatMap { it.notes }
            .sortedByDescending { note -> note.explanation.viaNotes.distinctBy { it.nodeKey }.size }
            .take(limit.coerceAtLeast(0))
            .mapTo(mutableSetOf()) { it.note.nodeKey }
    return groups.mapNotNull { group ->
        group.copy(notes = group.notes.filter { it.note.nodeKey in kept }).takeIf { it.notes.isNotEmpty() }
    }
}

/** Group every retained occurrence by its owning note without losing matched context. */
internal fun mentionGroups(
    result: UnlinkedReferencesResult,
    listed: Set<String>,
): List<MentionGroup> {
    val groups = linkedMapOf<String, Pair<NodeRecord, MutableList<MentionOccurrence>>>()
    val placed = mutableSetOf<String>()
    result.unlinkedReferences.forEach { record ->
        val note = record.sourceNote
        if (note.nodeKey in listed) return@forEach
        val identity = "${note.nodeKey}\u0000${record.row}\u0000${record.col}\u0000${record.matchedText}"
        if (!placed.add(identity)) return@forEach
        val group = groups.getOrPut(note.nodeKey) { note to mutableListOf() }
        group.second += MentionOccurrence(record, mentionExcerpt(record))
    }
    return groups.values.map { (note, occurrences) -> MentionGroup(note, occurrences) }
}

/** Highlight the scanner's exact one-based Unicode character column, with a safe fallback. */
internal fun mentionExcerpt(record: UnlinkedReferenceRecord): List<ContentSegment> {
    val line = record.preview.codePointStrings()
    val match = record.matchedText.codePointStrings()
    val stated = record.col.toInt() - 1
    val at =
        when {
            line.sitsAt(match, stated) -> stated
            else -> (0..line.size).firstOrNull { line.sitsAt(match, it) }
        }
    if (at == null) {
        val to = minOf(line.size, RELATION_PREVIEW_CHARACTERS)
        return listOf(ContentSegment(line.take(to).joinToString("") + ellipsis(to < line.size), false))
    }
    val from = maxOf(0, at - MENTION_CONTEXT_BEFORE)
    val to = maxOf(minOf(line.size, from + RELATION_PREVIEW_CHARACTERS), at + match.size)
    return listOf(
            ContentSegment(ellipsis(from > 0) + line.subList(from, at).joinToString(""), false),
            ContentSegment(match.joinToString(""), true),
            ContentSegment(line.subList(at + match.size, to).joinToString("") + ellipsis(to < line.size), false),
        )
        .filter { it.text.isNotEmpty() }
}

private fun String.codePointStrings(): List<String> = buildList {
    var offset = 0
    while (offset < length) {
        val point = codePointAt(offset)
        add(String(Character.toChars(point)))
        offset += Character.charCount(point)
    }
}

private fun List<String>.sitsAt(run: List<String>, at: Int): Boolean =
    at >= 0 && at + run.size <= size && run.indices.all { this[at + it] == run[it] }

private fun ellipsis(needed: Boolean): String = if (needed) "…" else ""

internal interface ReaderDiscoverySource : AutoCloseable {
    fun related(nodeKey: String, limit: Int): ExploreResult
    fun mentions(nodeKey: String, limit: Int): UnlinkedReferencesResult
}

internal fun interface ReaderDiscoverySourceFactory {
    fun open(ready: ReadySource): ReaderDiscoverySource
}

private object NativeReaderDiscoverySourceFactory : ReaderDiscoverySourceFactory {
    override fun open(ready: ReadySource): ReaderDiscoverySource {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : ReaderDiscoverySource {
                override fun related(nodeKey: String, limit: Int): ExploreResult =
                    (session
                            .answer(ReadOperation.Explore(nodeKey, ExplorationLens.BRIDGES, limit))
                            .await() as EngineAnswer.Explore)
                        .result

                override fun mentions(nodeKey: String, limit: Int): UnlinkedReferencesResult =
                    (session.answer(ReadOperation.UnlinkedReferences(nodeKey, limit)).await()
                            as EngineAnswer.UnlinkedReferences)
                        .result

                override fun close() = host.close()
            }
        } catch (failure: Throwable) {
            host.close()
            throw failure
        }
    }
}

/** Two independently lazy, independently failing discovery queries for one generation-bound note. */
@Stable
internal class ReaderDiscoveryState(
    private val note: BoundNote,
    private val ready: ReadySource,
    private val factory: ReaderDiscoverySourceFactory = NativeReaderDiscoverySourceFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
) : AutoCloseable {
    private val live = AtomicBoolean(true)
    private val relatedLoading = AtomicBoolean()
    private val mentionsLoading = AtomicBoolean()
    private val relatedRequest = AtomicLong()
    private val mentionsRequest = AtomicLong()
    private val relatedSource = AtomicReference<ReaderDiscoverySource?>()
    private val mentionsSource = AtomicReference<ReaderDiscoverySource?>()

    var related: RelatedDiscoveryPhase by mutableStateOf(RelatedDiscoveryPhase.Idle)
        private set
    var mentions: MentionDiscoveryPhase by mutableStateOf(MentionDiscoveryPhase.Idle)
        private set

    init {
        require(note.binding == ready.binding)
    }

    fun revealRelated() {
        if (related != RelatedDiscoveryPhase.Idle) return
        queryRelated()
    }

    fun refreshRelated() {
        if (related == RelatedDiscoveryPhase.Loading) return
        queryRelated()
    }

    fun showAllRelated() {
        val current = related as? RelatedDiscoveryPhase.Ready ?: return
        related = current.copy(showAll = true)
    }

    fun revealMentions() {
        if (mentions != MentionDiscoveryPhase.Idle) return
        queryMentions()
    }

    fun refreshMentions() {
        if (mentions == MentionDiscoveryPhase.Loading) return
        queryMentions()
    }

    fun showAllMentions() {
        val current = mentions as? MentionDiscoveryPhase.Ready ?: return
        mentions = current.copy(showAll = true)
    }

    private fun queryRelated() {
        if (!live.get() || !relatedLoading.compareAndSet(false, true)) return
        related = RelatedDiscoveryPhase.Loading
        val serial = relatedRequest.incrementAndGet()
        work("slipbox-related-notes") {
            val result =
                withSource(relatedSource) {
                    it.related(note.nodeKey, RELATED_QUERY_LIMIT).also { answer ->
                        require(answer.lens == ExplorationLens.BRIDGES)
                        val entries =
                            answer.sections
                                .filter { it.kind == ExplorationSectionKind.BRIDGE_CANDIDATES }
                                .flatMap { it.entries }
                        require(entries.size <= RELATED_QUERY_LIMIT)
                        require(
                            entries.all { entry ->
                                val record = (entry as? ExplorationEntry.Anchor)?.record
                                val explanation =
                                    record?.explanation as? ExplorationExplanation.BridgeCandidate
                                record?.anchor?.nodeKey?.isNotEmpty() == true &&
                                    explanation?.viaNotes?.isNotEmpty() == true &&
                                    explanation.viaNotes.all { it.nodeKey.isNotEmpty() }
                            },
                        )
                    }
                }
            delivery.post {
                if (!live.get() || relatedRequest.get() != serial) return@post
                relatedLoading.set(false)
                result.fold(
                    onSuccess = { answer ->
                        related =
                            if (relatedDiscovery(answer, emptySet()).rankedCount == 0) {
                                RelatedDiscoveryPhase.Empty
                            } else {
                                RelatedDiscoveryPhase.Ready(answer)
                            }
                    },
                    onFailure = { related = RelatedDiscoveryPhase.Failed },
                )
            }
        }
    }

    private fun queryMentions() {
        if (!live.get() || !mentionsLoading.compareAndSet(false, true)) return
        mentions = MentionDiscoveryPhase.Loading
        val serial = mentionsRequest.incrementAndGet()
        work("slipbox-unlinked-mentions") {
            val result =
                withSource(mentionsSource) {
                    it.mentions(note.nodeKey, MENTIONS_QUERY_LIMIT).also { answer ->
                        require(answer.unlinkedReferences.size <= MENTIONS_QUERY_LIMIT)
                        require(
                            answer.unlinkedReferences.all { record ->
                                val explanation =
                                    record.explanation as? ExplorationExplanation.UnlinkedReference
                                record.sourceNote.nodeKey.isNotEmpty() &&
                                    record.sourceAnchor.nodeKey.isNotEmpty() &&
                                    record.row > 0 && record.col > 0 && record.matchedText.isNotEmpty() &&
                                    explanation?.matchedText == record.matchedText
                            },
                        )
                    }
                }
            delivery.post {
                if (!live.get() || mentionsRequest.get() != serial) return@post
                mentionsLoading.set(false)
                result.fold(
                    onSuccess = { answer ->
                        mentions =
                            if (answer.unlinkedReferences.isEmpty()) {
                                MentionDiscoveryPhase.Empty
                            } else {
                                MentionDiscoveryPhase.Ready(answer)
                            }
                    },
                    onFailure = { mentions = MentionDiscoveryPhase.Failed },
                )
            }
        }
    }

    private fun work(name: String, block: () -> Unit) {
        Thread(block, name).apply { isDaemon = true }.start()
    }

    private inline fun <T> withSource(
        slot: AtomicReference<ReaderDiscoverySource?>,
        query: (ReaderDiscoverySource) -> T,
    ): Result<T> = runCatching {
        val opened = factory.open(ready)
        if (!live.get() || !slot.compareAndSet(null, opened)) {
            opened.close()
            error("the discovery request was closed while opening")
        }
        try {
            query(opened)
        } finally {
            slot.compareAndSet(opened, null)
            opened.close()
        }
    }

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        relatedRequest.incrementAndGet()
        mentionsRequest.incrementAndGet()
        relatedSource.getAndSet(null)?.close()
        mentionsSource.getAndSet(null)?.close()
    }
}

@Composable
internal fun rememberReaderDiscoveryState(note: BoundNote?, ready: ReadySource): ReaderDiscoveryState? {
    val state =
        note?.let { bound ->
            remember(bound, ready.binding, ready.contentRoot, ready.database) {
                ReaderDiscoveryState(bound, ready)
            }
        }
    DisposableEffect(state) { onDispose { state?.close() } }
    return state
}
