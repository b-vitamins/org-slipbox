/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PublicSourceInputTest {

    @Test
    fun canonicalPublicInputMintsIdentityAndKeepsNoCredential() {
        val source = requireNotNull(publicSource("HTTPS://Example.COM/owner/notes.git", "main", "."))

        assertTrue(source.id.matches(Regex("[0-9a-f]{32}")))
        assertEquals("notes", source.displayName)
        assertEquals(RefreshProvider.GENERIC_HTTPS, source.provider)
        assertEquals(RefreshVisibility.PUBLIC, source.visibility)
        assertEquals("https://example.com/owner/notes.git", source.remote)
        assertEquals("", source.notesFolder)
        assertNull(source.credential)
    }

    @Test
    fun credentialedUnsafeAndNonHttpsInputsAreRefusedBeforeImport() {
        for (url in listOf(
            "http://example.com/notes.git",
            "https://token@example.com/notes.git",
            "https://example.com/../notes.git",
            "https://example.com/notes%20here.git",
        )) {
            assertNull(url, publicSource(url, "main", ""))
        }
        assertNull(publicSource("https://example.com/notes.git", "HEAD", ""))
        assertNull(publicSource("https://example.com/notes.git", "main", "../notes"))
    }
}
