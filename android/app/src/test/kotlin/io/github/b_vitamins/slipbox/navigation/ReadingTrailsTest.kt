/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import io.github.b_vitamins.slipbox.sync.PackagedSourceRefreshStorage
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class ReadingTrailsTest {

    @get:Rule val temporary = TemporaryFolder()

    @Test
    fun aTrailReturnsOnTheCurrentGenerationWithStableIdentityAndPosition() {
        val store = ReadingTrailStore(temporary.root)
        val routes =
            listOf(
                reader("file:first.org", "first-id", ReadingAnchor("a:0", 0.25f, 0.5f)),
                reader("file:second.org", null, ReadingAnchor("b:0", 0.75f, 0.125f)),
            )

        store.save(ALPHA, routes)
        val restored = store.load(bind(ALPHA, "generation-9")).map { it as SlipboxRoute.Reader }

        assertEquals(routes.map { it.note.nodeKey }, restored.map { it.note.nodeKey })
        assertEquals(routes.map { it.note.explicitId }, restored.map { it.note.explicitId })
        assertEquals(routes.map { it.anchor }, restored.map { it.anchor })
        assertTrue(restored.all { it.note.binding == bind(ALPHA, "generation-9") })
    }

    @Test
    fun aRebuildDoesNotOwnTheTrailAndAnotherSourceCannotClaimIt() {
        val store = ReadingTrailStore(temporary.root)
        val route = reader("file:note.org", "note-id", ReadingAnchor(progress = 0.4f))
        store.save(ALPHA, listOf(route))
        val trail =
            requireNotNull(PackagedSourceRefreshStorage.readingTrailFile(temporary.root, ALPHA))
        val configuration = File(trail.parentFile?.parentFile, "configuration-rebuilt")
        configuration.mkdirs()
        File(configuration, "index.sqlite3").writeText("derived")
        configuration.deleteRecursively()

        assertEquals(1, store.load(bind(ALPHA, "rebuilt")).size)
        assertTrue(store.load(bind(BETA, "rebuilt")).isEmpty())

        trail.writeText(trail.readText().replace(ALPHA, BETA))
        assertTrue(store.load(bind(ALPHA, "rebuilt")).isEmpty())
    }

    @Test
    fun aTrailIsBoundedAndAnEmptyHistoryClearsIt() {
        val store = ReadingTrailStore(temporary.root)
        val routes = (0 until 80).map { reader("file:$it.org", null, ReadingAnchor.Start) }

        store.save(ALPHA, routes)
        val restored = store.load(bind(ALPHA, "later")).map { it as SlipboxRoute.Reader }
        assertEquals(64, restored.size)
        assertEquals("file:16.org", restored.first().note.nodeKey)

        store.save(ALPHA, emptyList())
        assertTrue(store.load(bind(ALPHA, "later")).isEmpty())
    }

    private fun reader(
        key: String,
        id: String?,
        anchor: ReadingAnchor,
    ): SlipboxRoute.Reader =
        SlipboxRoute.Reader(
            BoundNote(bind(ALPHA), key, explicitId = id, filePath = key.removePrefix("file:")),
            anchor,
        )
}
