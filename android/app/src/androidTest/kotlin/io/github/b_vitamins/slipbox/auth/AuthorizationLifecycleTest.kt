/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AuthorizationLifecycleTest {

    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    private val clock = ParkingClock()

    private val delivery = PostedDelivery()

    private val owners = mutableListOf<GithubAuthorizationOwner>()

    @After
    fun releaseTheOwners() {
        clock.release()
        owners.forEach { it.close() }
    }

    @Test
    fun aDestroyedLifecycleWithdrawsTheAttemptAndDeliversNothing() {
        val transport = PendingTransport()
        val owner = ownerOf(transport)
        val listener = ProbeListener()
        val attempt = verifying(owner, listener)

        instrumentation.runOnMainSync {
            val screen = ProbeLifecycle()
            screen.registry.addObserver(owner)
            screen.registry.currentState = Lifecycle.State.CREATED
            screen.registry.currentState = Lifecycle.State.DESTROYED
        }

        assertFalse("a destroyed screen left its attempt running", attempt.isLive)
        assertNull(owner.running())
        delivery.drain()
        assertEquals(
            "a withdrawn attempt reported an outcome",
            emptyList<AuthorizationOutcome>(),
            listener.settled,
        )
        assertEquals(
            "a withdrawn attempt kept polling",
            0,
            transport.exchangesOf(DeviceEndpoint.ACCESS_TOKEN),
        )
    }

    @Test
    fun comingBackFromTheBrowserIsAResumeAndSettlesNothing() {
        val transport = PendingTransport()
        val owner = ownerOf(transport)
        val listener = ProbeListener()
        val attempt = verifying(owner, listener)

        instrumentation.runOnMainSync {
            val screen = ProbeLifecycle()
            screen.registry.addObserver(owner)
            screen.registry.currentState = Lifecycle.State.RESUMED
            screen.registry.currentState = Lifecycle.State.STARTED
            screen.registry.currentState = Lifecycle.State.RESUMED
        }

        assertTrue("a resume withdrew the attempt it should have left alone", attempt.isLive)
        assertSame(attempt, owner.running())
        delivery.drain()
        assertEquals(
            "a return from the browser settled the attempt",
            emptyList<AuthorizationOutcome>(),
            listener.settled,
        )
        assertEquals("the code was shown more than once", 1, listener.waiting.size)
    }


    private fun verifying(
        owner: GithubAuthorizationOwner,
        listener: ProbeListener,
    ): AuthorizationAttempt {
        val attempt = owner.authorize(listener)
        assertTrue("no verification was ever delivered", delivery.awaitPosts(1))
        delivery.drain()
        assertEquals(PLACEHOLDER_CODE, listener.waiting.single().userCode)
        assertTrue("the attempt never reached its polling", clock.awaitPolling())
        return attempt
    }


    private fun ownerOf(transport: AuthorizationTransport): GithubAuthorizationOwner =
        GithubAuthorizationOwner(
            requireNotNull(GithubApp.of(PROBE_CLIENT_ID, INSTALLATION_PAGE)),
            transport,
            clock,
            delivery,
        ).also { owners.add(it) }


    private class ProbeLifecycle : LifecycleOwner {

        val registry = LifecycleRegistry(this)

        override val lifecycle: Lifecycle
            get() = registry
    }
}
