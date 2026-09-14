/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import android.os.Looper
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.INSTALLATION_PAGE
import io.github.b_vitamins.slipbox.auth.PROBE_CLIENT_ID
import io.github.b_vitamins.slipbox.auth.syntheticAuthorization
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class GithubBrowsingDeviceTest {

    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    private val owners = mutableListOf<GithubBrowsing>()

    private val delivered = CopyOnWriteArrayList<GithubOutcome<GithubListing<GithubInstallation>>>()

    private val installations = GithubRead { it.installations() }

    @After
    fun releaseTheOwners() {
        owners.forEach { it.close() }
    }

    @Test
    fun anOutcomeArrivesOnTheMainThread() {
        val browsing = browsing(GatedTransport())
        val settled = CountDownLatch(1)
        val onMainThread = AtomicBoolean(false)

        browsing.browse(installations) { outcome ->
            onMainThread.set(Looper.myLooper() == Looper.getMainLooper())
            delivered.add(outcome)
            settled.countDown()
        }

        assertTrue(settled.await(WAIT_MILLIS, TimeUnit.MILLISECONDS))
        assertTrue("the outcome arrived off the main thread", onMainThread.get())
        assertEquals(1, outcomeEntries())
    }

    @Test
    fun aDestroyedScreenWithdrawsTheTraversalAndDeliversNothing() {
        val gate = CountDownLatch(1)
        val transport = GatedTransport(gate)
        val browsing = browsing(transport)

        val traversal = browsing.browse(installations) { delivered.add(it) }
        assertTrue("the traversal never reached its request", transport.awaitEntry())
        instrumentation.runOnMainSync {
            val screen = ProbeLifecycle()
            screen.registry.addObserver(browsing)
            screen.registry.currentState = Lifecycle.State.CREATED
            screen.registry.currentState = Lifecycle.State.DESTROYED
        }
        gate.countDown()
        instrumentation.waitForIdleSync()

        assertFalse("a destroyed screen left its traversal running", traversal.isLive)
        assertNull(browsing.running())
        assertTrue("a withdrawn traversal reported an outcome", delivered.isEmpty())
        assertEquals("a withdrawn traversal kept reading", 1, transport.fetches.get())
    }

    private fun outcomeEntries(): Int =
        when (val outcome = delivered.single()) {
            is GithubOutcome.Read -> outcome.value.entries.size
            is GithubOutcome.Refused -> throw AssertionError("refused: ${outcome.refusal}")
        }

    private fun browsing(transport: GithubApiTransport): GithubBrowsing =
        GithubBrowsing(
            syntheticAuthorization(),
            requireNotNull(GithubApp.of(PROBE_CLIENT_ID, INSTALLATION_PAGE)),
            transport,
            GithubDelivery.MainThread,
        ).also { owners.add(it) }

    private class ProbeLifecycle : LifecycleOwner {

        val registry = LifecycleRegistry(this)

        override val lifecycle: Lifecycle
            get() = registry
    }

    private class GatedTransport(private val gate: CountDownLatch? = null) : GithubApiTransport {

        val fetches = AtomicInteger()

        private val entered = CountDownLatch(1)

        fun awaitEntry(): Boolean = entered.await(WAIT_MILLIS, TimeUnit.MILLISECONDS)

        override fun fetch(request: GithubApiRequest): GithubApiReply {
            fetches.incrementAndGet()
            entered.countDown()
            gate?.await(WAIT_MILLIS, TimeUnit.MILLISECONDS)
            return GithubApiReply.Answered(200, INSTALLATIONS, GithubApiHeaders.None)
        }
    }

    private companion object {

        const val WAIT_MILLIS = 10_000L

        const val INSTALLATIONS =
            """{"installations":[{"id":77000002,""" +
                """"account":{"id":42000001,"login":"synthetic-account"},""" +
                """"repository_selection":"selected"}]}"""
    }
}
