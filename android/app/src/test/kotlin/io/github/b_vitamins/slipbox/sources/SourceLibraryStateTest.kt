/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sources

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SourceLibraryStateTest {

    @Test
    fun startupSchedulesEveryConfiguredSourceAndExposesOnlyTheActiveGeneration() {
        val listing = listing()
        val started = CountDownLatch(1)
        var scheduled: List<RefreshSource> = emptyList()
        val state =
            SourceLibraryState(
                gateway = FakeCatalog(listing, SourceCatalogResult.Active(4, ready(alpha()))),
                startup = {
                    scheduled = it.toList()
                    started.countDown()
                },
                delivery = ImportDelivery { it() },
            )

        assertTrue(started.await(5, TimeUnit.SECONDS))
        await { state.phase is SourceLibraryPhase.Ready }
        assertEquals(listing.sources, scheduled)
        assertEquals("generation-a", state.readyGeneration(ALPHA))
        assertNull(state.readyGeneration(BETA))
        assertEquals(listing, state.catalog)
        state.close()
    }

    @Test
    fun selectingAReadySourceReplacesOnlyTheActiveCorpusIdentity() {
        val listing = listing()
        val catalog = FakeCatalog(listing, SourceCatalogResult.Active(4, ready(alpha())))
        catalog.selection = SourceCatalogResult.Active(5, ready(beta()))
        val state =
            SourceLibraryState(
                gateway = catalog,
                delivery = ImportDelivery { it() },
            )
        await { state.phase is SourceLibraryPhase.Ready }
        val selected = CountDownLatch(1)

        state.select(beta()) { selected.countDown() }

        assertTrue(selected.await(5, TimeUnit.SECONDS))
        assertEquals(BETA, state.catalog?.activeSource?.id)
        assertEquals("generation-b", state.readyGeneration(BETA))
        assertNull(state.readyGeneration(ALPHA))
        assertEquals(listOf(alpha(), beta()), state.catalog?.sources)
        state.close()
    }

    @Test
    fun unreadableActiveCacheDoesNotHideTheCatalogNeededForRecovery() {
        val listing = listing()
        val state =
            SourceLibraryState(
                gateway =
                    FakeCatalog(
                        listing,
                        SourceCatalogResult.Failed(
                            failure = SourceCatalogFailure.GENERATION_UNAVAILABLE,
                        ),
                    ),
                delivery = ImportDelivery { it() },
            )

        await { state.phase is SourceLibraryPhase.Failed }
        assertEquals(listing, state.catalog)
        state.close()
    }

    @Test
    fun readyConfigurationForAnInactiveSourceDoesNotSelectItLocally() {
        val listing = listing()
        val state =
            SourceLibraryState(
                gateway = FakeCatalog(listing, SourceCatalogResult.Active(4, ready(alpha()))),
                delivery = ImportDelivery { it() },
            )
        await { state.phase is SourceLibraryPhase.Ready }
        val configured = ready(beta().copy(branch = "next"))

        val becameActive = state.configured(configured, 5)

        assertEquals(false, becameActive)
        assertEquals(ALPHA, state.catalog?.activeSource?.id)
        assertEquals("next", state.catalog?.sources?.last()?.branch)
        assertEquals("generation-a", state.readyGeneration(ALPHA))
        assertNull(state.readyGeneration(BETA))
        state.close()
    }

    @Test
    fun aStaleCatalogLoadCannotReplaceANewerSourceSelection() {
        val firstEntered = CountDownLatch(1)
        val releaseFirst = CountDownLatch(1)
        val delivered = CountDownLatch(2)
        val calls = AtomicInteger()
        val first = listing()
        val second = SourceCatalogListing(5, listOf(alpha(), beta()), beta())
        val gateway =
            object : SourceLibraryCatalog {
                override fun list(): SourceCatalogListingResult {
                    if (calls.incrementAndGet() == 1) {
                        firstEntered.countDown()
                        assertTrue(releaseFirst.await(5, TimeUnit.SECONDS))
                        return SourceCatalogListingResult.Loaded(first)
                    }
                    return SourceCatalogListingResult.Loaded(second)
                }

                override fun load(listing: SourceCatalogListing): SourceCatalogResult =
                    SourceCatalogResult.Active(
                        listing.revision,
                        ready(requireNotNull(listing.activeSource)),
                    )

                override fun activate(
                    expectedRevision: Long,
                    source: RefreshSource,
                ): SourceCatalogResult = throw AssertionError("selection was not requested")
            }
        val state =
            SourceLibraryState(
                gateway = gateway,
                delivery =
                    ImportDelivery { action ->
                        action()
                        delivered.countDown()
                    },
            )
        assertTrue(firstEntered.await(5, TimeUnit.SECONDS))

        state.reload()
        await { (state.phase as? SourceLibraryPhase.Ready)?.source?.source?.id == BETA }
        releaseFirst.countDown()
        assertTrue(delivered.await(5, TimeUnit.SECONDS))

        assertEquals(BETA, state.catalog?.activeSource?.id)
        assertEquals("generation-b", state.readyGeneration(BETA))
        assertNull(state.readyGeneration(ALPHA))
        state.close()
    }

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
    }

    private fun listing(): SourceCatalogListing =
        SourceCatalogListing(4, listOf(alpha(), beta()), alpha())

    private fun ready(source: RefreshSource): ReadySource =
        ReadySource(
            source = source,
            binding =
                GenerationBinding(
                    source.id,
                    if (source.id == ALPHA) "generation-a" else "generation-b",
                ),
            revision = if (source.id == ALPHA) "revision-a" else "revision-b",
            contentRoot = "/private/${source.id}/source",
            database = "/private/${source.id}/index.sqlite",
            stats = ReadySourceStats(1, 2, 3),
        )

    private fun alpha(): RefreshSource =
        RefreshSource(
            id = ALPHA,
            displayName = "Alpha",
            provider = RefreshProvider.GENERIC_HTTPS,
            visibility = RefreshVisibility.PUBLIC,
            remote = "https://example.com/alpha.git",
            branch = "main",
            notesFolder = "",
        )

    private fun beta(): RefreshSource =
        alpha().copy(
            id = BETA,
            displayName = "Beta",
            remote = "https://example.com/beta.git",
        )

    private class FakeCatalog(
        private val listing: SourceCatalogListing,
        private val loaded: SourceCatalogResult,
    ) : SourceLibraryCatalog {

        var selection: SourceCatalogResult = loaded

        override fun list(): SourceCatalogListingResult =
            SourceCatalogListingResult.Loaded(listing)

        override fun load(listing: SourceCatalogListing): SourceCatalogResult = loaded

        override fun activate(
            expectedRevision: Long,
            source: RefreshSource,
        ): SourceCatalogResult = selection
    }

    private companion object {
        const val ALPHA = "0123456789abcdef0123456789abcdef"
        const val BETA = "fedcba9876543210fedcba9876543210"
    }
}
