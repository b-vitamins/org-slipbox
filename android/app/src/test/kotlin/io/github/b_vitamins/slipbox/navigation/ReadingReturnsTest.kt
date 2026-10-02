/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sync.PackagedSourceRefreshStorage
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class ReadingReturnsTest {

    @get:Rule val temporary = TemporaryFolder()

    @Test
    fun bookmarksAndRecentsSurviveARebuildAndRestartOnTheCurrentGeneration() {
        val store = ReadingReturnsStore(temporary.root)
        val original =
            ReadingReturnsSnapshot(
                bookmarks = listOf(entry("file:book.org", "book-id", "Book")),
                recents =
                    listOf(
                        entry(
                            "file:recent.org",
                            null,
                            "Recent",
                            ReadingAnchor("block:2", 0.72f, 0.25f),
                        ),
                    ),
            )
        store.save(ALPHA, original)
        val record =
            requireNotNull(PackagedSourceRefreshStorage.readingReturnsFile(temporary.root, ALPHA))
        val replaceable = File(record.parentFile?.parentFile, "configuration-rebuilt")
        assertFalse(record.readText().contains("Book"))
        assertFalse(record.readText().contains("Recent"))
        replaceable.mkdirs()
        File(replaceable, "index.sqlite3").writeText("derived")
        replaceable.deleteRecursively()

        val restored = store.load(bind(ALPHA, "generation-9"))

        assertEquals(listOf("book-id"), restored.bookmarks.map { it.note.explicitId })
        assertEquals(ReadingAnchor("block:2", 0.72f, 0.25f), restored.recents.single().anchor)
        assertTrue(
            (restored.bookmarks + restored.recents).all {
                it.note.binding == bind(ALPHA, "generation-9")
            },
        )
    }

    @Test
    fun equalKeysRemainSourceScopedAndAClaimedPayloadIsRefused() {
        val store = ReadingReturnsStore(temporary.root)
        store.save(ALPHA, ReadingReturnsSnapshot(recents = listOf(entry("file:same.org", null, "Alpha"))))
        store.save(
            BETA,
            ReadingReturnsSnapshot(
                recents = listOf(entry("file:same.org", null, "Beta", source = BETA)),
            ),
        )

        assertEquals(ALPHA, store.load(bind(ALPHA, "later")).recents.single().note.binding.source)
        assertEquals(BETA, store.load(bind(BETA, "later")).recents.single().note.binding.source)

        val alpha =
            requireNotNull(PackagedSourceRefreshStorage.readingReturnsFile(temporary.root, ALPHA))
        alpha.writeText(alpha.readText().replace(ALPHA, BETA))
        assertEquals(ReadingReturnsSnapshot(), store.load(bind(ALPHA, "later")))
    }

    @Test
    fun storedListsKeepNewestOrderingWithinBoundsAndClearWithoutTouchingSourceFiles() {
        val store = ReadingReturnsStore(temporary.root)
        val repository = temporary.newFolder("repository")
        val org = File(repository, "note.org").apply { writeText("* Note\n") }
        store.save(
            ALPHA,
            ReadingReturnsSnapshot(
                bookmarks = (0 until 140).map { entry("file:b$it.org", null, "B$it") },
                recents = (0 until 80).map { entry("file:r$it.org", null, "R$it") },
            ),
        )

        val restored = store.load(bind(ALPHA, "later"))
        assertEquals(128, restored.bookmarks.size)
        assertEquals("file:b0.org", restored.bookmarks.first().note.nodeKey)
        assertEquals(64, restored.recents.size)
        assertEquals("file:r0.org", restored.recents.first().note.nodeKey)

        store.save(ALPHA, ReadingReturnsSnapshot())
        assertEquals(ReadingReturnsSnapshot(), store.load(bind(ALPHA, "later")))
        assertTrue(org.isFile)
        assertEquals("* Note\n", org.readText())
    }

    @Test
    fun reconciliationUsesStableIdsMarksDeletionAndLearnsRenamedKeys() {
        val moved = node("file:moved.org", "stable-id", "Moved")
        val alias = node("file:renamed.org", null, "Renamed")
        val restored =
            ReadingReturnsSnapshot(
                bookmarks = listOf(entry("file:old.org", "stable-id", "Old title")),
                recents =
                    listOf(
                        entry("file:reused.org", "deleted-id", "Deleted"),
                        entry("file:before.org", null, "Before"),
                    ),
            )
        val state =
            ReadingReturnsState(
                ready = ready(),
                restored = restored,
                resolverFactory =
                    ReadingReturnResolverFactory {
                        resolver { held ->
                            when (held.explicitId) {
                                "stable-id" -> moved
                                "deleted-id" -> null
                                else -> alias.takeIf { held.nodeKey == "file:before.org" }
                            }
                        }
                    },
                delivery = ImportDelivery { it() },
            )

        await {
            state.snapshot.bookmarks.single().availability ==
                ReadingReturnAvailability.Available &&
                state.snapshot.recents.all {
                    it.availability != ReadingReturnAvailability.Checking
                }
        }

        assertEquals("file:moved.org", state.snapshot.bookmarks.single().note.nodeKey)
        assertEquals("Moved", state.snapshot.bookmarks.single().title)
        assertEquals(ReadingReturnAvailability.Missing, state.snapshot.recents.first().availability)
        assertEquals("file:reused.org", state.snapshot.recents.first().note.nodeKey)
        assertEquals("file:renamed.org", state.snapshot.recents.last().note.nodeKey)
        state.close()
    }

    @Test
    fun recentAndBookmarkMutationsAreOrderedExplicitAndSourceLocal() {
        val saves = mutableListOf<Pair<String, ReadingReturnsSnapshot>>()
        val state =
            ReadingReturnsState(
                ready = ready(),
                restored = ReadingReturnsSnapshot(),
                sink = ReadingReturnsSink { source, snapshot -> saves += source to snapshot },
                delivery = ImportDelivery { it() },
            )
        val first = node("file:first.org", "first-id", "First")
        val second = node("file:second.org", null, "Second")

        state.recordRecent(first, ReadingAnchor(progress = 0.2f))
        state.recordRecent(second, ReadingAnchor(progress = 0.4f))
        state.recordRecent(first, ReadingAnchor(progress = 0.8f))
        assertEquals(listOf("First", "Second"), state.snapshot.recents.map { it.title })
        assertEquals(0.8f, state.continueReading?.anchor?.progress)

        state.toggleBookmark(first)
        assertTrue(state.isBookmarked(BoundNote(ready().binding, first.nodeKey, first.explicitId)))
        state.rememberReadingPlace(
            BoundNote(ready().binding, first.nodeKey, first.explicitId),
            ReadingAnchor(progress = 0.9f),
        )
        assertEquals(0.9f, state.snapshot.bookmarks.single().anchor.progress)
        state.removeRecent(state.snapshot.recents.last())
        assertEquals(listOf("First"), state.snapshot.recents.map { it.title })
        state.clearBookmarks()
        assertTrue(state.snapshot.bookmarks.isEmpty())
        assertFalse(saves.isEmpty())
        assertTrue(saves.all { it.first == ALPHA })

        state.clearAll()
        assertEquals(ReadingReturnsSnapshot(), saves.last().second)
        state.close()
    }

    @Test
    fun aLateStartupReconciliationDoesNotDowngradeAReadingReturnAddedMeanwhile() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val old = node("file:old.org", null, "Old")
        val current = node("file:current.org", "current-id", "Current")
        val state =
            ReadingReturnsState(
                ready = ready(),
                restored = ReadingReturnsSnapshot(recents = listOf(entry(old.nodeKey, null, old.title))),
                resolverFactory =
                    ReadingReturnResolverFactory {
                        resolver {
                            entered.countDown()
                            release.await(5, TimeUnit.SECONDS)
                            old
                        }
                    },
                delivery = ImportDelivery { it() },
            )
        assertTrue(entered.await(5, TimeUnit.SECONDS))

        state.recordRecent(current, ReadingAnchor(progress = 0.5f))
        release.countDown()
        await {
            state.snapshot.recents.any {
                it.title == "Old" && it.availability == ReadingReturnAvailability.Available
            }
        }

        assertEquals(
            ReadingReturnAvailability.Available,
            state.snapshot.recents.first { it.title == "Current" }.availability,
        )
        state.close()
    }

    private fun entry(
        key: String,
        id: String?,
        title: String,
        anchor: ReadingAnchor = ReadingAnchor.Start,
        source: String = ALPHA,
    ): ReadingReturn =
        ReadingReturn(
            note = BoundNote(bind(source, "current"), key, id, key.removePrefix("file:")),
            title = title,
            anchor = anchor,
        )

    private fun node(key: String, id: String?, title: String): NodeRecord =
        NodeRecord(
            nodeKey = key,
            explicitId = id,
            filePath = key.removePrefix("file:"),
            title = title,
            outlinePath = "",
            aliases = emptyList(),
            tags = emptyList(),
            refs = emptyList(),
            todoKeyword = null,
            scheduledFor = null,
            deadlineFor = null,
            closedAt = null,
            glossary = false,
            glossaryStatus = null,
            srDue = null,
            srEase = null,
            srInterval = null,
            srReps = null,
            srLast = null,
            level = 0,
            line = 1,
            kind = NodeKind.FILE,
            fileMtimeNs = 0,
            backlinkCount = 0,
            forwardLinkCount = 0,
        )

    private fun ready(): ReadySource =
        ReadySource(
            source =
                RefreshSource(
                    id = ALPHA,
                    displayName = "owner/notes",
                    provider = RefreshProvider.GENERIC_HTTPS,
                    visibility = RefreshVisibility.PUBLIC,
                    remote = "https://example.com/notes.git",
                    branch = "main",
                    notesFolder = "",
                ),
            binding = bind(ALPHA, "current"),
            revision = "0123456789abcdef",
            contentRoot = "/private/source",
            database = "/private/index.sqlite",
            stats = ReadySourceStats(2, 2, 0),
        )

    private fun resolver(resolve: (BoundNote) -> NodeRecord?): ReadingReturnResolver =
        object : ReadingReturnResolver {
            override fun resolve(note: BoundNote): NodeRecord? = resolve(note)

            override fun close() = Unit
        }

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
    }
}
