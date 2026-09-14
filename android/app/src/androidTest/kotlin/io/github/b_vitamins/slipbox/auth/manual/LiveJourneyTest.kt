/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.manual

import android.os.Bundle
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.auth.BrowserHandoff
import io.github.b_vitamins.slipbox.auth.DeviceEndpoint
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.GithubAuthorizationOwner
import io.github.b_vitamins.slipbox.auth.INSTALLATION_PAGE
import io.github.b_vitamins.slipbox.auth.PROBE_ACCOUNT_ID
import io.github.b_vitamins.slipbox.auth.PROBE_CLIENT_ID
import io.github.b_vitamins.slipbox.auth.ParkingClock
import io.github.b_vitamins.slipbox.auth.PendingTransport
import io.github.b_vitamins.slipbox.auth.PostedDelivery
import io.github.b_vitamins.slipbox.auth.RepositorySelection
import io.github.b_vitamins.slipbox.auth.syntheticAuthorization
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPhase
import io.github.b_vitamins.slipbox.ui.auth.GithubAuthorizationState
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Synthetic input and outcome checks; the live journey is separate instrumentation. */
@RunWith(AndroidJUnit4::class)
class LiveJourneyTest {

    private val results =
        InstrumentationRegistry.getInstrumentation().targetContext.cacheDir.absolutePath

    private val owners = mutableListOf<GithubAuthorizationOwner>()

    @After
    fun releaseTheOwners() {
        owners.forEach { it.close() }
    }

    @Test
    fun aRunWithNoStageOfItsOwnIsRefusedByArgumentName() {
        val refusal = refusalOf(Bundle())

        assertTrue(refusal, refusal.contains(STAGE_ARGUMENT))
    }

    @Test
    fun aRunWithNowhereToWriteIsRefusedByArgumentName() {
        val refusal = refusalOf(arguments(stage = CANCEL_STAGE, results = null))

        assertTrue(refusal, refusal.contains(RESULTS_ARGUMENT))
    }

    @Test
    fun aRelativeResultPathIsRefusedRatherThanResolvedSomewhere() {
        val refusal = refusalOf(arguments(stage = CANCEL_STAGE, results = "live-auth"))

        assertTrue(refusal, refusal.contains(RESULTS_ARGUMENT))
    }

    @Test
    fun aConsentRunWithNoExpectedAccountIsRefusedByArgumentName() {
        val refusal = refusalOf(arguments(stage = GRANT_STAGE))

        assertTrue(refusal, refusal.contains(ACCOUNT_ARGUMENT))
    }

    @Test
    fun anExpectedAccountThatIsNotGitHubsNumberIsRefused() {
        val refusal = refusalOf(arguments(stage = GRANT_STAGE, account = "octocat"))

        assertTrue(refusal, refusal.contains(ACCOUNT_ARGUMENT))
    }

    @Test
    fun noRefusalRepeatsWhatItWasGiven() {
        val refusal = refusalOf(arguments(stage = GRANT_STAGE, account = "octocat"))

        assertFalse(refusal, refusal.contains("octocat"))
        assertFalse(refusal, refusal.contains(results))
    }

    @Test
    fun aWaitForConsentCannotOutliveTheCodeItIsWaitingFor() {
        val refusal =
            refusalOf(
                arguments(
                    stage = GRANT_STAGE,
                    account = PROBE_ACCOUNT_ID,
                    consentSeconds = (MAX_CONSENT_SECONDS + 1).toString(),
                ),
            )

        assertTrue(refusal, refusal.contains(CONSENT_ARGUMENT))
    }

    @Test
    fun aConsentRunTakesTheAccountItMustMatchAndAWaitOfItsOwn() {
        val given =
            givenOf(
                arguments(stage = GRANT_STAGE, account = PROBE_ACCOUNT_ID, consentSeconds = "120"),
            )

        assertEquals(LiveStage.Grant, given.stage)
        assertEquals(PROBE_ACCOUNT_ID, given.expectedAccountId)
        assertEquals(120_000L, given.consentMillis)
    }

    @Test
    fun aCancellationRunNeedsNoAccountAndWaitsForNobody() {
        val given = givenOf(arguments(stage = CANCEL_STAGE))

        assertEquals(LiveStage.Cancel, given.stage)
        assertNull(given.expectedAccountId)
    }

    @Test
    fun theGateRefusesAGrantThatNamesAnotherAccount() {
        val gate = LiveGate(syntheticAuthorization(accountId = "42000002"), PROBE_ACCOUNT_ID)

        assertFalse("another account's grant passed the identity gate", gate.identityMatch)
        assertTrue(gate.selectedInstallation)
    }

    @Test
    fun theGateRequiresAnInstallationOnSelectedRepositories() {
        val expected = PROBE_ACCOUNT_ID

        val none =
            LiveGate(
                syntheticAuthorization(installations = 0, selection = RepositorySelection.None),
                expected,
            )
        val all =
            LiveGate(
                syntheticAuthorization(selection = RepositorySelection.All),
                expected,
            )
        val selected = LiveGate(syntheticAuthorization(), expected)

        assertFalse("an account with no installation passed", none.selectedInstallation)
        assertFalse("an installation on every repository passed", all.selectedInstallation)
        assertTrue(selected.selectedInstallation)
        assertTrue(selected.identityMatch)
    }

    @Test
    fun aCancellationRunPassesOnItsOwnOutcomesAndAConsentRunDoesNot() {
        val cancel = LiveJourneyReport(LiveStage.Cancel).apply { withdrew() }
        val grant = LiveJourneyReport(LiveStage.Grant).apply { withdrew() }

        assertTrue(cancel.json(), cancel.passed)
        assertFalse("a consent run passed without a grant", grant.passed)
        grant.apply {
            authorized = true
            identityMatch = true
            selectedInstallation = true
            vaultHandoff = true
            vaultReadback = true
        }
        assertTrue(grant.json(), grant.passed)
    }

    @Test
    fun aRunThatFaultedIsNotAPassAndCarriesTheClassAlone() {
        val report =
            LiveJourneyReport(LiveStage.Cancel).apply {
                withdrew()
                refusal = "java.io.IOException"
            }

        assertFalse("a faulted run passed", report.passed)
        assertTrue(report.json(), report.json().contains("java.io.IOException"))
    }

    @Test
    fun noReportCarriesAnythingButItsStageAndItsOutcomes() {
        val report = LiveJourneyReport(LiveStage.Grant).apply { withdrew() }

        val json = report.json()
        assertFalse(json, json.contains(PROBE_ACCOUNT_ID))
        assertFalse(json, json.contains(results))
        assertEquals(
            "the report changed what it carries",
            REPORTED,
            Regex("\"([A-Za-z]+)\":").findAll(json).map { it.groupValues[1] }.toList(),
        )
    }

    @Test
    fun aWithdrawnCodeRequestGoesBackToIdleAndSettlesNothingAfterwards() {
        val transport = PendingTransport()
        val clock = ParkingClock()
        val delivery = PostedDelivery()
        val state = stateOf(transport, clock, delivery)
        state.begin()
        assertTrue("no verification was ever delivered", delivery.awaitPosts(1))
        delivery.drain()
        assertTrue("the code request never reached its screen", verifying(state))
        assertTrue("the attempt never reached its polling", clock.awaitPolling())

        state.cancel()

        assertTrue("a withdrawn request stayed on screen", idle(state))
        assertNull("a withdrawn request left an attempt running", state.owner.running())
        assertTrue("the withdrawn attempt never settled", delivery.awaitPosts(2))
        delivery.drain()
        assertTrue("a withdrawn request settled afterwards", idle(state))
        assertEquals(
            "a withdrawn request kept polling",
            0,
            transport.exchangesOf(DeviceEndpoint.ACCESS_TOKEN),
        )
    }

    private fun stateOf(
        transport: PendingTransport,
        clock: ParkingClock,
        delivery: PostedDelivery,
    ): GithubAuthorizationState {
        val owner =
            GithubAuthorizationOwner(
                requireNotNull(GithubApp.of(PROBE_CLIENT_ID, INSTALLATION_PAGE)),
                transport,
                clock,
                delivery,
            )
        owners.add(owner)
        return GithubAuthorizationState(owner, BrowserHandoff { false }, INSTALLATION_PAGE)
    }

    private fun verifying(state: GithubAuthorizationState): Boolean =
        state.phase is AuthorizationPhase.Verifying

    private fun idle(state: GithubAuthorizationState): Boolean =
        state.phase is AuthorizationPhase.Idle

    private fun arguments(
        stage: String? = null,
        results: String? = this.results,
        account: String? = null,
        consentSeconds: String? = null,
    ): Bundle =
        Bundle().apply {
            stage?.let { putString(STAGE_ARGUMENT, it) }
            results?.let { putString(RESULTS_ARGUMENT, it) }
            account?.let { putString(ACCOUNT_ARGUMENT, it) }
            consentSeconds?.let { putString(CONSENT_ARGUMENT, it) }
        }

    private fun refusalOf(arguments: Bundle): String =
        when (val request = liveRequestOf(arguments)) {
            is LiveRequest.Refused -> request.reason
            is LiveRequest.Given -> throw AssertionError("expected a refusal, got ${request.stage}")
        }

    private fun givenOf(arguments: Bundle): LiveRequest.Given =
        when (val request = liveRequestOf(arguments)) {
            is LiveRequest.Refused -> throw AssertionError("expected inputs: ${request.reason}")
            is LiveRequest.Given -> request
        }

    private fun LiveJourneyReport.withdrew() {
        configured = true
        codeShown = true
        cancellation = true
        ownedCleanup = true
    }

    private companion object {

        val REPORTED =
            listOf(
                "stage",
                "configured",
                "codeShown",
                "authorized",
                "identityMatch",
                "selectedInstallation",
                "vaultHandoff",
                "vaultReadback",
                "cancellation",
                "ownedCleanup",
                "refusal",
                "passed",
            )
    }
}
