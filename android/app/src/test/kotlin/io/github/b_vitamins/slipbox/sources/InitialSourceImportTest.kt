/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sources

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.sync.CancellableSourceRefreshExecutor
import io.github.b_vitamins.slipbox.sync.RefreshDisposition
import io.github.b_vitamins.slipbox.sync.RefreshFailure
import io.github.b_vitamins.slipbox.sync.RefreshFailureReason
import io.github.b_vitamins.slipbox.sync.RefreshProgress
import io.github.b_vitamins.slipbox.sync.RefreshProgressUnit
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshRetry
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshState
import io.github.b_vitamins.slipbox.sync.RefreshStatus
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import io.github.b_vitamins.slipbox.sync.SourceRefresh
import io.github.b_vitamins.slipbox.sync.SourceRefreshOutcome
import io.github.b_vitamins.slipbox.sync.SourceRefreshPaths
import io.github.b_vitamins.slipbox.sync.SourceRefreshRuntime
import io.github.b_vitamins.slipbox.sync.SourceRefreshStatusExecutor
import io.github.b_vitamins.slipbox.sync.SourceRefreshStatusOutcome
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class InitialSourceImportTest {

    @Test
    fun readyGenerationIsCommittedBeforeReadyIsDelivered() {
        val executor = FakeExecutor()
        val activations = AtomicInteger()
        val source = source()
        val ready = ready(source)
        val activation = SourceActivation { revision, activated, generation ->
            assertEquals(2, revision)
            assertEquals(source, activated)
            assertEquals(GENERATION, generation)
            activations.incrementAndGet()
            SourceCatalogResult.Active(3, ready)
        }
        val owner = owner(executor, activation)
        val settled = CountDownLatch(1)
        var result: InitialSourceImportEvent? = null

        assertTrue(owner.begin(source, 2) { event ->
            if (event is InitialSourceImportEvent.Ready) {
                result = event
                settled.countDown()
            }
        })

        assertTrue(settled.await(2, TimeUnit.SECONDS))
        assertTrue(result is InitialSourceImportEvent.Ready)
        assertEquals(1, activations.get())
        owner.close()
    }

    @Test
    fun aRunningImportRefusesDuplicatesAndCancellationSettlesOnce() {
        val executor = BlockingExecutor()
        val owner = owner(executor, SourceActivation { _, _, _ -> error("must not activate") })
        val source = source()
        val settled = CountDownLatch(1)
        val cancellations = AtomicInteger()

        assertTrue(owner.begin(source, 0) { event ->
            if (event is InitialSourceImportEvent.Cancelled) {
                cancellations.incrementAndGet()
                settled.countDown()
            }
        })
        assertTrue(executor.started.await(2, TimeUnit.SECONDS))
        assertFalse(owner.begin(source, 0) {})
        owner.cancel()
        executor.release.countDown()

        assertTrue(settled.await(2, TimeUnit.SECONDS))
        assertEquals(1, executor.cancels.get())
        assertEquals(1, cancellations.get())
        owner.close()
    }

    @Test
    fun aFailedImportCanBeRetriedWithoutDuplicateActivation() {
        val executor = RetryExecutor()
        val source = source()
        val activations = AtomicInteger()
        val owner =
            owner(executor) { _, _, _ ->
                activations.incrementAndGet()
                SourceCatalogResult.Active(1, ready(source))
            }
        val failed = CountDownLatch(1)

        assertTrue(owner.begin(source, 0) { event ->
            if (event is InitialSourceImportEvent.Failed) failed.countDown()
        })
        assertTrue(failed.await(2, TimeUnit.SECONDS))

        val ready = CountDownLatch(1)
        assertTrue(owner.begin(source, 0) { event ->
            if (event is InitialSourceImportEvent.Ready) ready.countDown()
        })

        assertTrue(ready.await(2, TimeUnit.SECONDS))
        assertEquals(2, executor.attempts.get())
        assertEquals(1, activations.get())
        owner.close()
    }

    private fun owner(executor: CancellableSourceRefreshExecutor, activation: SourceActivation) =
        InitialSourceImportOwner(
            SourceRefreshRuntime(executor, { SourceRefreshPaths(File("repo"), File("store")) }),
            activation,
            ImportDelivery { it() },
        )

    private fun source() =
        RefreshSource(
            id = SOURCE,
            displayName = "Notes",
            provider = RefreshProvider.GENERIC_HTTPS,
            visibility = RefreshVisibility.PUBLIC,
            remote = "https://example.com/notes.git",
            branch = "main",
            notesFolder = "",
        )

    private fun ready(source: RefreshSource) =
        ReadySource(
            source = source,
            binding = GenerationBinding(SOURCE, GENERATION),
            revision = REVISION,
            contentRoot = "/private/source",
            database = "/private/index/slipbox.db",
            stats = ReadySourceStats(4, 9, 5),
        )

    private open class FakeExecutor : CancellableSourceRefreshExecutor, SourceRefreshStatusExecutor {

        override fun refresh(request: SourceRefresh): SourceRefreshOutcome =
            SourceRefreshOutcome.Answered(RefreshDisposition.STARTED, status(request.source))

        override fun cancel(operation: Long): Boolean = true

        override fun status(source: String): SourceRefreshStatusOutcome = SourceRefreshStatusOutcome.Idle

        protected fun status(source: RefreshSource) =
            RefreshStatus(
                source = source,
                operation = 1,
                state = RefreshState.READY,
                fetchedRevision = REVISION,
                readyRevision = REVISION,
                readyGeneration = GENERATION,
                progress = RefreshProgress(1, 1, RefreshProgressUnit.STEPS),
                failure = null,
            )
    }

    private class BlockingExecutor : FakeExecutor() {

        val started = CountDownLatch(1)
        val release = CountDownLatch(1)
        val cancels = AtomicInteger()

        override fun refresh(request: SourceRefresh): SourceRefreshOutcome {
            started.countDown()
            release.await(2, TimeUnit.SECONDS)
            return SourceRefreshOutcome.Answered(
                RefreshDisposition.STARTED,
                status(request.source).copy(state = RefreshState.CANCELLED, readyGeneration = null),
            )
        }

        override fun cancel(operation: Long): Boolean {
            cancels.incrementAndGet()
            return true
        }
    }

    private class RetryExecutor : FakeExecutor() {

        val attempts = AtomicInteger()

        override fun refresh(request: SourceRefresh): SourceRefreshOutcome {
            if (attempts.incrementAndGet() > 1) return super.refresh(request)
            return SourceRefreshOutcome.Answered(
                RefreshDisposition.STARTED,
                status(request.source).copy(
                    state = RefreshState.FAILED,
                    readyGeneration = null,
                    failure =
                        RefreshFailure(
                            RefreshFailureReason.TRANSPORT_FAILED,
                            RefreshRetry.Backoff(30),
                        ),
                ),
            )
        }
    }

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val GENERATION = "generation-8"
        const val REVISION = "0123456789012345678901234567890123456789"
    }
}
