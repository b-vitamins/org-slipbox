/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import android.content.Intent
import android.net.Uri
import androidx.core.net.toUri
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BrowserHandoffTest {

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun aPageIsHandedOverAsAViewOfItsOwnTask() {
        val intent = requireNotNull(handoff().intentFor(VERIFICATION_PAGE))

        assertEquals(Intent.ACTION_VIEW, intent.action)
        assertEquals(VERIFICATION_PAGE.toUri(), intent.data)
        assertTrue(
            "a page handed to a browser must not join this task",
            intent.flags and Intent.FLAG_ACTIVITY_NEW_TASK != 0,
        )
        assertTrue(
            "only a browsable page is a page for a browser",
            intent.categories.orEmpty().contains(Intent.CATEGORY_BROWSABLE),
        )
    }

    @Test
    fun anAddressThatIsNotAnHttpsPageIsNeverHandedOver() {
        val handoff = handoff()

        assertNull("a plain http page was handed over", handoff.intentFor("http://github.com/x"))
        assertNull("a relative address was handed over", handoff.intentFor("github.com/x"))
        assertNull("an address with no host was handed over", handoff.intentFor("https://"))
        assertNull("an application scheme was handed over", handoff.intentFor("slipbox://grant"))
    }

    @Test
    fun aBrowserThatTookThePageIsReportedAsHavingTakenIt() {
        val browser = HandoffContext(context)

        assertTrue(SystemBrowserHandoff(browser).open(VERIFICATION_PAGE))

        assertEquals(1, browser.offered.size)
        assertEquals(VERIFICATION_PAGE.toUri(), browser.offered.single().data)
    }

    @Test
    fun aDeviceWithNoBrowserIsReportedRatherThanRaised() {
        val browser = HandoffContext(context, accepts = false)

        assertFalse(SystemBrowserHandoff(browser).open(VERIFICATION_PAGE))

        assertEquals("the page was never offered", 1, browser.offered.size)
    }

    @Test
    fun anAddressNoBrowserMayTakeIsRefusedBeforeAnyBrowserSeesIt() {
        val browser = HandoffContext(context)

        assertFalse(SystemBrowserHandoff(browser).open("http://github.com/login/device"))

        assertEquals(
            "an address that is not an https page was offered anyway",
            emptyList<Uri>(),
            browser.offered.map { it.data },
        )
    }


    private fun handoff(): SystemBrowserHandoff = SystemBrowserHandoff(HandoffContext(context))
}
