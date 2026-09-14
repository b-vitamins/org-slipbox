/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.auth

import io.github.b_vitamins.slipbox.auth.AuthorizationFault
import io.github.b_vitamins.slipbox.auth.AuthorizationOutcome
import io.github.b_vitamins.slipbox.auth.AuthorizationTransport
import io.github.b_vitamins.slipbox.auth.BrowserHandoff
import io.github.b_vitamins.slipbox.auth.DeviceGrant
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.GithubAuthorizationOwner
import io.github.b_vitamins.slipbox.auth.GithubEndpoint
import io.github.b_vitamins.slipbox.auth.INSTALLATION_URL
import io.github.b_vitamins.slipbox.auth.QueuedDelivery
import io.github.b_vitamins.slipbox.auth.RecordedTransport
import io.github.b_vitamins.slipbox.auth.SteppedClock
import io.github.b_vitamins.slipbox.auth.USER_CODE
import io.github.b_vitamins.slipbox.auth.VERIFICATION_URI
import io.github.b_vitamins.slipbox.auth.grantingTransport
import io.github.b_vitamins.slipbox.auth.pending
import io.github.b_vitamins.slipbox.auth.testApp
import java.util.concurrent.CountDownLatch
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class GithubAuthorizationStateTest {

    private val clock = SteppedClock()

    private val delivery = QueuedDelivery()

    private val browser = RecordingBrowser()

    private val states = mutableListOf<GithubAuthorizationState>()

    private val opened = mutableListOf<CountDownLatch>()

    @After
    fun releaseTheStates() {
        opened.forEach { it.countDown() }
        states.forEach { it.dispose() }
    }

    @Test
    fun anIdleSurfaceOffersNothingOfItsOwn() {
        val state = stateOver(transport = grantingTransport())

        assertSame(AuthorizationPhase.Idle, state.phase)
        assertFalse(state.browserRefused)
    }

    @Test
    fun aSurfaceSaysItIsAskingBeforeGitHubHasAnswered() {
        val state = stateOver(transport = grantingTransport())

        state.begin()

        assertSame(AuthorizationPhase.Requesting, state.phase)
    }

    @Test
    fun aBuildWithNoRegistrationSaysSoAndOffersNoRetry() {
        val state = stateOver(app = null, transport = RecordedTransport())

        state.begin()
        assertTrue(delivery.awaitPosts(1))
        delivery.drain()

        val settled = state.phase as AuthorizationPhase.Settled
        assertEquals(
            AuthorizationOutcome.Unavailable(AuthorizationFault.ConfigurationUnavailable),
            settled.outcome,
        )
        assertFalse(settled.outcome.isRetryable())
    }

    @Test
    fun aWaitingCodeIsWhatTheSurfaceShowsNext() {
        val gate = held()
        val state = stateOver(transport = grantingTransport(polls = listOf(pending())))

        state.begin()
        assertTrue(delivery.awaitPosts(1))
        delivery.drain()

        val waiting = codeOf(state)
        assertEquals(USER_CODE, waiting.userCode)
        assertEquals(VERIFICATION_URI, waiting.verificationUri)
        gate.countDown()
    }

    @Test
    fun comingBackFromTheBrowserSettlesNothing() {
        val gate = held()
        val state = verifying()

        state.openVerification()
        state.openVerification()
        delivery.drain()

        assertEquals(listOf(VERIFICATION_URI, VERIFICATION_URI), browser.opened)
        assertFalse("a browser return was taken for an answer", state.browserRefused)
        assertEquals("a browser return settled the surface", USER_CODE, codeOf(state).userCode)
        gate.countDown()
    }

    @Test
    fun aBrowserThatTookNothingIsSaidSoOnceAndNoLonger() {
        val gate = held()
        val state = verifying()

        browser.accepts = false
        state.openVerification()
        assertTrue(state.browserRefused)

        browser.accepts = true
        state.openVerification()

        assertFalse("a browser that opened the page still reads as refused", state.browserRefused)
        gate.countDown()
    }

    @Test
    fun nothingIsHandedOverWhileNoCodeIsWaiting() {
        val state = stateOver(transport = grantingTransport())

        state.openVerification()
        state.begin()
        state.openVerification()

        assertEquals("a page was opened with no code to enter", emptyList<String>(), browser.opened)
        assertFalse(state.browserRefused)
    }

    @Test
    fun installingIsOfferedOnlyWhereTheBuildNamesThePage() {
        val unconfigured = stateOver(transport = grantingTransport(), installationUrl = null)

        unconfigured.openInstallation()

        assertFalse(unconfigured.canInstall)
        assertEquals(emptyList<String>(), browser.opened)

        val configured = stateOver(transport = grantingTransport())

        configured.openInstallation()

        assertTrue(configured.canInstall)
        assertEquals(listOf(INSTALLATION_URL), browser.opened)
    }

    @Test
    fun cancellingLeavesTheSurfaceAsItWasFound() {
        val gate = held()
        val state = verifying()
        browser.accepts = false
        state.openVerification()

        state.cancel()

        assertSame(AuthorizationPhase.Idle, state.phase)
        assertFalse(state.browserRefused)
        assertNull(state.owner.running())
        delivery.drain()
        assertSame("a cancelled attempt settled the surface", AuthorizationPhase.Idle, state.phase)
        gate.countDown()
    }

    @Test
    fun aWithdrawnOutcomeIsNotAnOutcomeTheSurfaceShows() {
        val state = stateOver(transport = grantingTransport())
        state.begin()

        state.onSettled(AuthorizationOutcome.Withdrawn)

        assertSame(AuthorizationPhase.Idle, state.phase)
    }

    @Test
    fun disposingClosesTheOwnerForGood() {
        val gate = held()
        val transport = grantingTransport(polls = listOf(pending()))
        val state = stateOver(transport = transport)
        state.begin()
        assertTrue(delivery.awaitPosts(1))
        delivery.drain()

        state.dispose()

        assertSame(AuthorizationPhase.Idle, state.phase)
        assertNull(state.owner.running())
        state.begin()
        delivery.drain()
        assertNull("a disposed surface started another attempt", state.owner.running())
        assertEquals(
            "a disposed surface asked GitHub for another code",
            1,
            transport.exchangesOf(GithubEndpoint.DEVICE_CODE),
        )
        gate.countDown()
    }


    private fun verifying(): GithubAuthorizationState {
        val state = stateOver(transport = grantingTransport(polls = listOf(pending())))
        state.begin()
        assertTrue(delivery.awaitPosts(1))
        delivery.drain()
        assertEquals(USER_CODE, codeOf(state).userCode)
        return state
    }


    private fun codeOf(state: GithubAuthorizationState): DeviceGrant =
        when (val phase = state.phase) {
            is AuthorizationPhase.Verifying -> phase.grant
            else -> throw AssertionError("expected a waiting code, got $phase")
        }


    private fun stateOver(
        app: GithubApp? = testApp(),
        transport: AuthorizationTransport,
        installationUrl: String? = INSTALLATION_URL,
    ): GithubAuthorizationState =
        GithubAuthorizationState(
            GithubAuthorizationOwner(app, transport, clock, delivery),
            browser,
            installationUrl,
        ).also { states.add(it) }


    private fun held(): CountDownLatch =
        CountDownLatch(1).also {
            clock.pause = it
            opened.add(it)
        }


    private class RecordingBrowser(var accepts: Boolean = true) : BrowserHandoff {

        private val pages = mutableListOf<String>()

        val opened: List<String>
            get() = pages.toList()

        override fun open(url: String): Boolean {
            pages.add(url)
            return accepts
        }
    }
}
