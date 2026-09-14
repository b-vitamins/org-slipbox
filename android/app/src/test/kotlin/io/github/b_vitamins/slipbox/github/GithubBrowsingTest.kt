/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class GithubBrowsingTest {

    private val installations = GithubRead { it.installations() }

    private val seen = CopyOnWriteArrayList<GithubOutcome<GithubListing<GithubInstallation>>>()

    @Test
    fun anOutcomeReachesTheOwnerThroughItsDelivery() {
        val transport = listing()
        val delivery = QueuedDelivery()
        val browsing = browsing(transport, delivery)

        val traversal = browsing.browse(installations) { seen.add(it) }

        assertTrue(delivery.awaitPosts(1))
        assertTrue("nothing arrives before the delivery runs", seen.isEmpty())
        assertEquals(1, delivery.drain())
        assertEquals(1, seen.single().read().entries.size)
        assertFalse(traversal.isLive)
        assertNull("a settled traversal is no longer the running one", browsing.running())
        browsing.close()
    }

    @Test
    fun cancellingStopsBeforeTheNextPageAndReportsNothing() {
        val second = "${installationsUrl()}&page=2"
        val transport =
            RecordedApiTransport()
                .answers(
                    installationsUrl(),
                    answered(installationsBody(installationEntry()), link = nextLink(second)),
                )
        val delivery = QueuedDelivery()
        val browsing = browsing(transport, delivery)
        val finished = CountDownLatch(1)
        transport.beforeFetch = { browsing.running()?.cancel() }

        val read =
            GithubRead { provider ->
                val outcome = provider.installations()
                finished.countDown()
                outcome
            }

        val traversal = browsing.browse(read) { seen.add(it) }

        assertTrue(finished.await(WAIT_MILLIS, TimeUnit.MILLISECONDS))
        assertFalse(traversal.isLive)
        assertEquals("the next page was never requested", listOf(installationsUrl()), transport.urls)
        delivery.drain()
        assertTrue("a cancelled traversal reports nothing", seen.isEmpty())
        browsing.close()
    }

    @Test
    fun aReplacementBrowseWithdrawsTheRunningTraversal() {
        val transport = listing()
        val browsing = browsing(transport, DirectDelivery)
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val first = browsing.browse(blocking(entered, release)) { seen.add(it) }
        assertTrue(entered.await(WAIT_MILLIS, TimeUnit.MILLISECONDS))
        val settled = CountDownLatch(1)

        browsing.browse(installations) { settled.countDown() }

        assertTrue(settled.await(WAIT_MILLIS, TimeUnit.MILLISECONDS))
        assertFalse("the replaced traversal stops", first.isLive)
        assertTrue("only the replacement reports", seen.isEmpty())
        release.countDown()
        browsing.close()
    }

    @Test
    fun aClosedOwnerStopsItsTraversalAndStartsNoOther() {
        val transport = listing()
        val browsing = browsing(transport, DirectDelivery)
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val first = browsing.browse(blocking(entered, release)) { seen.add(it) }
        assertTrue(entered.await(WAIT_MILLIS, TimeUnit.MILLISECONDS))

        browsing.close()
        val later = browsing.browse(installations) { seen.add(it) }

        assertFalse(first.isLive)
        assertFalse(later.isLive)
        assertTrue("a closed owner issues no request", transport.urls.isEmpty())
        release.countDown()
    }

    @Test
    fun anUnexpectedFaultIsReportedAsARefusalNotAsACrash() {
        val browsing = browsing(RecordedApiTransport(), DirectDelivery)
        val settled = CountDownLatch(1)

        browsing.browse(installations) {
            seen.add(it)
            settled.countDown()
        }

        assertTrue(settled.await(WAIT_MILLIS, TimeUnit.MILLISECONDS))
        assertEquals(
            GithubRefusal.TransportFailed(GithubStage.Traversal, "java.lang.AssertionError"),
            seen.single().refusal(),
        )
        browsing.close()
    }

    private fun listing(): RecordedApiTransport =
        RecordedApiTransport()
            .answers(installationsUrl(), answered(installationsBody(installationEntry())))

    private fun browsing(
        transport: GithubApiTransport,
        delivery: GithubDelivery,
    ): GithubBrowsing = GithubBrowsing(testAuthorization(), testApp(), transport, delivery)

    private fun blocking(
        entered: CountDownLatch,
        release: CountDownLatch,
    ): GithubRead<GithubListing<GithubInstallation>> =
        GithubRead { provider ->
            entered.countDown()
            release.await(WAIT_MILLIS, TimeUnit.MILLISECONDS)
            provider.installations()
        }
}
