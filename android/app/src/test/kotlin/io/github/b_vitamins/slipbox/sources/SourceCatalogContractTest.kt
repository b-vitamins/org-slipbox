/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sources

import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SourceCatalogContractTest {

    @Test
    fun commitCarriesIdentityAndNoCredentialValue() {
        val request =
            SourceCatalogRequest.Commit(
                catalog = "/private/source-catalog.json",
                expectedRevision = 7,
                source = source(),
                store = "/private/store",
                generation = "generation-8",
            )

        val encoded = SourceCatalogWire.encode(request).toString(Charsets.UTF_8)

        assertTrue(encoded.contains("\"operation\":\"commit\""))
        assertTrue(encoded.contains("\"expected_revision\":7"))
        assertTrue(encoded.contains("\"credential\":null"))
        assertTrue(encoded.length < MAX_SOURCE_CATALOG_REQUEST_BYTES)
    }

    @Test
    fun strictReadyResponseDecodesAllVerifiedPathsAndCounts() {
        val response =
            """{"version":1,"outcome":"ready","revision":3,"ready":{"source":{"id":"0123456789abcdef0123456789abcdef","display_name":"Notes","provider":"generic_https","visibility":"public","provider_repository_id":null,"account":null,"remote":"https://example.com/notes.git","branch":"main","notes_folder":"","credential":null},"binding":{"source":"0123456789abcdef0123456789abcdef","generation":"generation-8"},"revision":"0123456789012345678901234567890123456789","content_root":"/private/source","database":"/private/index/slipbox.db","stats":{"files_indexed":4,"nodes_indexed":9,"links_indexed":5}}}"""
                .toByteArray()

        val decoded = SourceCatalogWire.decode(response) as SourceCatalogResponse.Ready

        assertEquals(3, decoded.revision)
        assertEquals("generation-8", decoded.ready.binding.generation)
        assertEquals(9, decoded.ready.stats.nodesIndexed)
    }

    @Test(expected = SourceCatalogContractException::class)
    fun unknownResponseFieldsAreRefused() {
        SourceCatalogWire.decode(
            """{"version":1,"outcome":"loaded","revision":0,"active_source":null,"extra":true}"""
                .toByteArray(),
        )
    }

    private fun source() =
        RefreshSource(
            id = "0123456789abcdef0123456789abcdef",
            displayName = "Notes",
            provider = RefreshProvider.GENERIC_HTTPS,
            visibility = RefreshVisibility.PUBLIC,
            remote = "https://example.com/notes.git",
            branch = "main",
            notesFolder = "",
        )
}
