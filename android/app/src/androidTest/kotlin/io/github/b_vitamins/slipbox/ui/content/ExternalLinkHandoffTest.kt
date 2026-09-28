/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.content.Intent
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ExternalLinkHandoffTest {

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val handoff = SystemExternalLinkHandoff(context)

    @Test
    fun httpAndHttpsPagesCrossAsCredentialFreeBrowsableIntents() {
        for (url in listOf("http://example.org/page", "https://example.org/page?q=one")) {
            val intent = requireNotNull(handoff.intentFor(url))
            assertEquals(Intent.ACTION_VIEW, intent.action)
            assertEquals(url, intent.dataString)
            assertTrue(intent.categories.orEmpty().contains(Intent.CATEGORY_BROWSABLE))
            assertTrue(intent.flags and Intent.FLAG_ACTIVITY_NEW_TASK != 0)
            assertTrue(intent.extras?.isEmpty == true || intent.extras == null)
        }
    }

    @Test
    fun schemesCredentialsAndMalformedAddressesAreRefused() {
        for (url in
            listOf(
                "javascript:alert(1)",
                "file:///data/local/tmp/note.org",
                "mailto:reader@example.org",
                "https://" + "user:secret@" + "example.org/page",
                "https:example.org/page",
                "https:\\example.org/page",
                "https://",
                "relative/page",
            )
        ) {
            assertNull("$url crossed", handoff.intentFor(url))
        }
    }
}
