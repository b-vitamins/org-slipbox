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
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal const val EXPLORATION_QUERY_LIMIT = 50

internal sealed interface ReaderExplorationPhase {
    data object AwaitingLens : ReaderExplorationPhase
    data class Loading(val lens: ExplorationLens) : ReaderExplorationPhase
    data class NoSubstrate(val lens: ExplorationLens) : ReaderExplorationPhase
    data class Empty(val lens: ExplorationLens) : ReaderExplorationPhase
    data class Ready(val result: ExploreResult) : ReaderExplorationPhase
    data class Failed(val lens: ExplorationLens) : ReaderExplorationPhase
}

internal fun ReaderExplorationPhase.lens(): ExplorationLens? =
    when (this) {
        ReaderExplorationPhase.AwaitingLens -> null
        is ReaderExplorationPhase.Loading -> lens
        is ReaderExplorationPhase.NoSubstrate -> lens
        is ReaderExplorationPhase.Empty -> lens
        is ReaderExplorationPhase.Ready -> result.lens
        is ReaderExplorationPhase.Failed -> lens
    }

internal interface ReaderExplorationSource : AutoCloseable {
    fun explore(nodeKey: String, lens: ExplorationLens, limit: Int): ExploreResult
}

internal fun interface ReaderExplorationSourceFactory {
    fun open(ready: ReadySource): ReaderExplorationSource
}

private object NativeReaderExplorationSourceFactory : ReaderExplorationSourceFactory {
    override fun open(ready: ReadySource): ReaderExplorationSource {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : ReaderExplorationSource {
                override fun explore(
                    nodeKey: String,
                    lens: ExplorationLens,
                    limit: Int,
                ): ExploreResult =
                    (session.answer(ReadOperation.Explore(nodeKey, lens, limit)).await()
                            as EngineAnswer.Explore)
                        .result

                override fun close() = host.close()
            }
        } catch (failure: Throwable) {
            host.close()
            throw failure
        }
    }
}

/** Owns one explicitly selected exploration lens for one source generation. */
@Stable
internal class ReaderExplorationState(
    private val note: BoundNote,
    private val anchor: NodeRecord,
    private val ready: ReadySource,
    private val factory: ReaderExplorationSourceFactory = NativeReaderExplorationSourceFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
) : AutoCloseable {
    private val live = AtomicBoolean(true)
    private val requests = AtomicLong()
    private val source = AtomicReference<ReaderExplorationSource?>()

    var phase: ReaderExplorationPhase by mutableStateOf(ReaderExplorationPhase.AwaitingLens)
        private set

    init {
        require(note.binding == ready.binding)
        require(note.nodeKey == anchor.nodeKey)
    }

    fun select(lens: ExplorationLens) {
        if (!live.get() || phase.lens() == lens) return
        query(lens)
    }

    fun refresh() {
        val lens = phase.lens() ?: return
        if (phase is ReaderExplorationPhase.Loading) return
        query(lens)
    }

    private fun query(lens: ExplorationLens) {
        if (!live.get()) return
        val serial = requests.incrementAndGet()
        source.getAndSet(null)?.close()
        phase = ReaderExplorationPhase.Loading(lens)
        Thread(
                {
                    val outcome =
                        withSource(serial) { opened ->
                            opened.explore(note.nodeKey, lens, EXPLORATION_QUERY_LIMIT).also {
                                validateExploration(lens, it)
                            }
                        }
                    delivery.post {
                        if (!live.get() || requests.get() != serial) return@post
                        outcome.fold(
                            onSuccess = { result ->
                                phase =
                                    when {
                                        result.sections.any { it.entries.isNotEmpty() } ->
                                            ReaderExplorationPhase.Ready(result)
                                        !anchor.hasExplorationSubstrate(lens) ->
                                            ReaderExplorationPhase.NoSubstrate(lens)
                                        else -> ReaderExplorationPhase.Empty(lens)
                                    }
                            },
                            onFailure = { phase = ReaderExplorationPhase.Failed(lens) },
                        )
                    }
                },
                "slipbox-exploration-${lens.name.lowercase()}",
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    private inline fun <T> withSource(
        serial: Long,
        query: (ReaderExplorationSource) -> T,
    ): Result<T> = runCatching {
        val opened = factory.open(ready)
        if (!live.get() || requests.get() != serial || !source.compareAndSet(null, opened)) {
            opened.close()
            error("the exploration request was superseded while opening")
        }
        try {
            query(opened)
        } finally {
            source.compareAndSet(opened, null)
            opened.close()
        }
    }

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        requests.incrementAndGet()
        source.getAndSet(null)?.close()
    }
}

internal fun validateExploration(requested: ExplorationLens, result: ExploreResult) {
    require(result.lens == requested)
    require(result.sections.map { it.kind } == requested.sectionKinds())
    require(result.sections.all { it.entries.size <= EXPLORATION_QUERY_LIMIT })
    require(
        result.sections.all { section ->
            section.entries.all { entry -> entry.belongsTo(section.kind) }
        },
    )
}

internal fun ExplorationLens.sectionKinds(): List<ExplorationSectionKind> =
    when (this) {
        ExplorationLens.STRUCTURE ->
            listOf(ExplorationSectionKind.BACKLINKS, ExplorationSectionKind.FORWARD_LINKS)
        ExplorationLens.REFS ->
            listOf(ExplorationSectionKind.REFLINKS, ExplorationSectionKind.UNLINKED_REFERENCES)
        ExplorationLens.TIME -> listOf(ExplorationSectionKind.TIME_NEIGHBORS)
        ExplorationLens.TASKS -> listOf(ExplorationSectionKind.TASK_NEIGHBORS)
        ExplorationLens.BRIDGES -> listOf(ExplorationSectionKind.BRIDGE_CANDIDATES)
        ExplorationLens.DORMANT -> listOf(ExplorationSectionKind.DORMANT_NOTES)
        ExplorationLens.UNRESOLVED ->
            listOf(
                ExplorationSectionKind.UNRESOLVED_TASKS,
                ExplorationSectionKind.WEAKLY_INTEGRATED_NOTES,
            )
    }

private fun ExplorationEntry.belongsTo(section: ExplorationSectionKind): Boolean =
    when (section) {
        ExplorationSectionKind.BACKLINKS ->
            this is ExplorationEntry.Backlink && record.explanation == ExplorationExplanation.Backlink
        ExplorationSectionKind.FORWARD_LINKS ->
            this is ExplorationEntry.ForwardLink &&
                record.explanation == ExplorationExplanation.ForwardLink
        ExplorationSectionKind.REFLINKS ->
            this is ExplorationEntry.Reflink &&
                record.explanation is ExplorationExplanation.SharedReference
        ExplorationSectionKind.UNLINKED_REFERENCES ->
            this is ExplorationEntry.UnlinkedReference &&
                record.explanation is ExplorationExplanation.UnlinkedReference
        ExplorationSectionKind.TIME_NEIGHBORS ->
            this is ExplorationEntry.Anchor &&
                record.explanation is ExplorationExplanation.TimeNeighbor
        ExplorationSectionKind.TASK_NEIGHBORS ->
            this is ExplorationEntry.Anchor &&
                record.explanation is ExplorationExplanation.TaskNeighbor
        ExplorationSectionKind.BRIDGE_CANDIDATES ->
            this is ExplorationEntry.Anchor &&
                record.explanation is ExplorationExplanation.BridgeCandidate
        ExplorationSectionKind.DORMANT_NOTES ->
            this is ExplorationEntry.Anchor &&
                record.explanation is ExplorationExplanation.DormantSharedReference
        ExplorationSectionKind.UNRESOLVED_TASKS ->
            this is ExplorationEntry.Anchor &&
                record.explanation is ExplorationExplanation.UnresolvedSharedReference
        ExplorationSectionKind.WEAKLY_INTEGRATED_NOTES ->
            this is ExplorationEntry.Anchor &&
                record.explanation is ExplorationExplanation.WeaklyIntegratedSharedReference
    }

private fun NodeRecord.hasExplorationSubstrate(lens: ExplorationLens): Boolean {
    val planning = scheduledFor != null || deadlineFor != null || closedAt != null
    val relations = refs.isNotEmpty() || backlinkCount > 0 || forwardLinkCount > 0
    return when (lens) {
        ExplorationLens.STRUCTURE,
        ExplorationLens.REFS,
        -> true
        ExplorationLens.TIME -> planning
        ExplorationLens.TASKS -> todoKeyword != null || planning
        ExplorationLens.BRIDGES,
        ExplorationLens.UNRESOLVED,
        -> relations
        ExplorationLens.DORMANT -> fileMtimeNs > 0
    }
}

@Composable
internal fun rememberReaderExplorationState(
    note: BoundNote?,
    anchor: NodeRecord?,
    ready: ReadySource,
): ReaderExplorationState? {
    val state =
        if (note == null || anchor == null) {
            null
        } else {
            remember(note, anchor, ready.binding, ready.contentRoot, ready.database) {
                ReaderExplorationState(note, anchor, ready)
            }
        }
    DisposableEffect(state) { onDispose { state?.close() } }
    return state
}
