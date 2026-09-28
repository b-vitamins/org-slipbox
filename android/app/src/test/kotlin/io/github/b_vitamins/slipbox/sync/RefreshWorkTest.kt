/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sync

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RefreshWorkTest {

    @Test
    fun completedInitialImportInstallsOnlyTheQuietPeriodicLane() {
        val queue = RecordingRefreshWorkQueue()
        val source = source()

        val outcome = SourceRefreshScheduler(queue, RecordingActiveRefreshes()).imported(source)

        assertEquals(RefreshScheduleOutcome.Accepted, outcome)
        assertEquals(
            listOf(WorkCall(WorkKind.PERIODIC, RefreshTrigger.PERIODIC, source, false)),
            queue.calls,
        )
    }

    @Test
    fun configurationInstallsBothLanesWithoutCancellingAnInitialSource() {
        val queue = RecordingRefreshWorkQueue()
        val active = RecordingActiveRefreshes()
        val source = source()

        val outcome = SourceRefreshScheduler(queue, active).configure(null, source)

        assertEquals(RefreshScheduleOutcome.Accepted, outcome)
        assertEquals(
            listOf(
                WorkCall(WorkKind.PERIODIC, RefreshTrigger.PERIODIC, source, false),
                WorkCall(WorkKind.ONE_SHOT, RefreshTrigger.STARTUP, source, false),
            ),
            queue.calls,
        )
        assertTrue(active.cancelled.isEmpty())
    }

    @Test
    fun relabellingUpdatesWorkWithoutCancellingTheImport() {
        val queue = RecordingRefreshWorkQueue()
        val active = RecordingActiveRefreshes()
        val original = source()
        val relabelled = original.copy(displayName = "Renamed notes")

        SourceRefreshScheduler(queue, active).configure(original, relabelled)

        assertTrue(queue.calls.all { !it.replace })
        assertTrue(active.cancelled.isEmpty())
    }

    @Test
    fun changedImportConfigurationCancelsAndReplacesBothLanes() {
        val queue = RecordingRefreshWorkQueue()
        val active = RecordingActiveRefreshes()
        val original = source()
        val moved = original.copy(branch = "next")

        SourceRefreshScheduler(queue, active).configure(original, moved)

        assertEquals(listOf(original.id), active.cancelled)
        assertEquals(2L, queue.calls.size.toLong())
        assertTrue(queue.calls.all { it.replace })
        assertEquals(setOf(RefreshTrigger.PERIODIC, RefreshTrigger.STARTUP), queue.calls.map { it.trigger }.toSet())
    }

    @Test
    fun changedSourceIdentityCancelsTheOldSourceBeforeInstallingTheNewOne() {
        val queue = RecordingRefreshWorkQueue()
        val active = RecordingActiveRefreshes()
        val original = source()
        val replacement = source(1)

        SourceRefreshScheduler(queue, active).configure(original, replacement)

        assertEquals(listOf(original.id), active.cancelled)
        assertEquals(listOf(original.id), queue.removed)
        assertTrue(queue.calls.all { it.source == replacement && it.replace })
    }

    @Test
    fun startupChecksOnlyTheBoundedPrefixAndRegistersRecurringWork() {
        val queue = RecordingRefreshWorkQueue()
        val sources = (0 until SourceRefreshScheduler.MAX_STARTUP_REFRESHES + 3).map(::source)

        val outcome =
            SourceRefreshScheduler(queue, RecordingActiveRefreshes()).startup(sources)

        assertEquals(SourceRefreshScheduler.MAX_STARTUP_REFRESHES.toLong(), outcome.accepted.toLong())
        assertEquals(0L, outcome.refused.toLong())
        assertEquals(3L, outcome.deferred.toLong())
        assertEquals(
            (SourceRefreshScheduler.MAX_STARTUP_REFRESHES * 2 + 3).toLong(),
            queue.calls.size.toLong(),
        )
        assertEquals(
            sources.size.toLong(),
            queue.calls.count { it.kind == WorkKind.PERIODIC }.toLong(),
        )
    }

    @Test
    fun manualRefreshUsesTheSharedOneShotLane() {
        val queue = RecordingRefreshWorkQueue()
        val source = source()

        val outcome = SourceRefreshScheduler(queue, RecordingActiveRefreshes()).manual(source)

        assertEquals(RefreshScheduleOutcome.Accepted, outcome)
        assertEquals(
            listOf(WorkCall(WorkKind.ONE_SHOT, RefreshTrigger.MANUAL, source, false)),
            queue.calls,
        )
    }

    @Test
    fun removalCancelsNativeAndQueuedWorkForExactlyOneSource() {
        val queue = RecordingRefreshWorkQueue()
        val active = RecordingActiveRefreshes()
        val source = source()

        val outcome = SourceRefreshScheduler(queue, active).remove(source.id)

        assertEquals(RefreshScheduleOutcome.Accepted, outcome)
        assertEquals(listOf(source.id), active.cancelled)
        assertEquals(listOf(source.id), queue.removed)
    }

    @Test
    fun invalidAndOversizedSourcesNeverReachTheQueue() {
        val queue = RecordingRefreshWorkQueue()
        val scheduler = SourceRefreshScheduler(queue, RecordingActiveRefreshes())

        assertEquals(
            RefreshScheduleOutcome.Refused(RefreshScheduleRefusal.INVALID_SOURCE),
            scheduler.manual(source().copy(id = "not-a-source")),
        )
        assertEquals(
            RefreshScheduleOutcome.Refused(RefreshScheduleRefusal.INPUT_OVERSIZED),
            scheduler.manual(source().copy(remote = "https://example.com/" + "x".repeat(9_000))),
        )
        assertTrue(queue.calls.isEmpty())
    }

    @Test
    fun workInputRoundTripsStrictlyAndStaysCredentialFree() {
        val source = source()
        val input = RefreshWorkInput(RefreshTrigger.PERIODIC, source)
        val encoded = requireNotNull(RefreshWorkWire.encode(input))

        val decoded = requireNotNull(RefreshWorkWire.decode(requireNotNull(RefreshWorkWire.data(input))))

        assertEquals(input, decoded)
        assertTrue(encoded.contains(source.credential.orEmpty()))
        assertFalse(encoded.contains("github_pat_"))
        assertEquals(null, RefreshWorkWire.decode(androidx.work.Data.EMPTY))
    }

    private companion object {

        fun source(seed: Int = 0): RefreshSource {
            val digit = "0123456789abcdef"[seed % 16]
            return RefreshSource(
                id = digit.toString().repeat(32),
                displayName = "Notes $seed",
                provider = RefreshProvider.GITHUB,
                visibility = RefreshVisibility.PRIVATE,
                providerRepositoryId = "R_fixture_$seed",
                account = "U_fixture_$seed",
                remote = "https://github.com/example/notes-$seed.git",
                branch = "main",
                notesFolder = "notes",
                credential = "slipbox.source.credential-$seed",
            )
        }
    }
}

private enum class WorkKind {
    ONE_SHOT,
    PERIODIC,
}

private data class WorkCall(
    val kind: WorkKind,
    val trigger: RefreshTrigger,
    val source: RefreshSource,
    val replace: Boolean,
)

private class RecordingRefreshWorkQueue : RefreshWorkQueue {

    val calls = mutableListOf<WorkCall>()

    val removed = mutableListOf<String>()

    override fun oneShot(input: RefreshWorkInput, replace: Boolean) {
        calls.add(WorkCall(WorkKind.ONE_SHOT, input.trigger, input.source, replace))
    }

    override fun periodic(input: RefreshWorkInput, replace: Boolean) {
        calls.add(WorkCall(WorkKind.PERIODIC, input.trigger, input.source, replace))
    }

    override fun cancel(source: String) {
        removed.add(source)
    }
}

private class RecordingActiveRefreshes : ActiveSourceRefreshes {

    val cancelled = mutableListOf<String>()

    override fun cancel(source: String) {
        cancelled.add(source)
    }
}
