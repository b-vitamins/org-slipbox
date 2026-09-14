/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github.manual

import android.os.Bundle
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ProviderJourneyTest {

    private val results =
        InstrumentationRegistry.getInstrumentation().targetContext.cacheDir.absolutePath

    @Test
    fun aRunWithNowhereToWriteIsRefusedByArgumentName() {
        val refusal = refusalOf(arguments(results = null))

        assertTrue(refusal, refusal.contains(RESULTS_ARGUMENT))
    }

    @Test
    fun aRelativeResultPathIsRefusedRatherThanResolvedSomewhere() {
        val refusal = refusalOf(arguments(results = "live-github"))

        assertTrue(refusal, refusal.contains(RESULTS_ARGUMENT))
    }

    @Test
    fun aRunWithNoExpectedAccountIsRefusedByArgumentName() {
        val refusal = refusalOf(arguments(account = null))

        assertTrue(refusal, refusal.contains(ACCOUNT_ARGUMENT))
    }

    @Test
    fun anExpectedAccountThatIsNotGitHubsNumberIsRefused() {
        val refusal = refusalOf(arguments(account = "octocat"))

        assertTrue(refusal, refusal.contains(ACCOUNT_ARGUMENT))
    }

    @Test
    fun aRepositoryThatIsNotOwnerAndNameIsRefused() {
        for (candidate in listOf("", "notes", "owner/notes/inbox", "owner/", "/notes")) {
            val refusal = refusalOf(arguments(repository = candidate))

            assertTrue(refusal, refusal.contains(REPOSITORY_ARGUMENT))
        }
    }

    @Test
    fun aBranchTheCoreWouldRejectIsRefused() {
        for (candidate in listOf("HEAD", "with space", "trail/")) {
            val refusal = refusalOf(arguments(branch = candidate))

            assertTrue(refusal, refusal.contains(BRANCH_ARGUMENT))
        }
    }

    @Test
    fun aFolderTheCoreWouldRejectIsRefused() {
        val refusal = refusalOf(arguments(folder = "notes/../escape"))

        assertTrue(refusal, refusal.contains(FOLDER_ARGUMENT))
    }

    @Test
    fun anAbsentFolderIsTheRepositoryRoot() {
        val given = givenOf(arguments())

        assertEquals("", given.folder)
        assertEquals(OWNER, given.owner)
        assertEquals(REPOSITORY, given.repository)
        assertEquals(BRANCH, given.branch)
        assertEquals(ACCOUNT, given.expectedAccountId)
        assertEquals(DEFAULT_CONSENT_SECONDS * 1000L, given.consentMillis)
    }

    @Test
    fun aWaitForConsentCannotOutliveTheCodeItIsWaitingFor() {
        val refusal =
            refusalOf(arguments(consentSeconds = (MAX_CONSENT_SECONDS + 1).toString()))

        assertTrue(refusal, refusal.contains(CONSENT_ARGUMENT))
    }

    @Test
    fun noRefusalRepeatsWhatItWasGiven() {
        val refusal = refusalOf(arguments(branch = "HEAD"))

        assertFalse(refusal, refusal.contains(OWNER))
        assertFalse(refusal, refusal.contains(REPOSITORY))
        assertFalse(refusal, refusal.contains(ACCOUNT))
        assertFalse(refusal, refusal.contains(results))
    }

    @Test
    fun aRunPassesOnlyWhenEveryOutcomeHolds() {
        val complete = discovered()

        assertTrue(complete.json(), complete.passed)
        assertFalse(
            "a run passed without reaching the selected repository",
            discovered().apply { repositoryDiscoverable = false }.passed,
        )
        assertFalse(
            "a run passed while an unauthorized repository stayed selectable",
            discovered().apply { unauthorizedRepositoryRefused = false }.passed,
        )
        assertFalse(
            "a run passed while its own material stayed on the device",
            discovered().apply { ownedCleanup = false }.passed,
        )
        assertFalse(
            "a faulted run passed",
            discovered().apply { refusal = "java.io.IOException" }.passed,
        )
    }

    @Test
    fun noReportCarriesAnythingButItsOutcomes() {
        val json = discovered().json()

        assertFalse(json, json.contains(OWNER))
        assertFalse(json, json.contains(REPOSITORY))
        assertFalse(json, json.contains(ACCOUNT))
        assertFalse(json, json.contains(results))
        assertEquals(
            "the report changed what it carries",
            REPORTED,
            Regex("\"([A-Za-z]+)\":").findAll(json).map { it.groupValues[1] }.toList(),
        )
    }

    private fun discovered(): ProviderJourneyReport =
        ProviderJourneyReport().apply {
            configured = true
            codeShown = true
            authorized = true
            identityMatch = true
            installationsListed = true
            repositoryDiscoverable = true
            branchesListed = true
            branchResolved = true
            folderListed = true
            selectionConfirmed = true
            foreignAccountRefused = true
            unauthorizedRepositoryRefused = true
            ownedCleanup = true
        }

    private fun arguments(
        results: String? = this.results,
        account: String? = ACCOUNT,
        repository: String? = "$OWNER/$REPOSITORY",
        branch: String? = BRANCH,
        folder: String? = null,
        consentSeconds: String? = null,
    ): Bundle =
        Bundle().apply {
            results?.let { putString(RESULTS_ARGUMENT, it) }
            account?.let { putString(ACCOUNT_ARGUMENT, it) }
            repository?.let { putString(REPOSITORY_ARGUMENT, it) }
            branch?.let { putString(BRANCH_ARGUMENT, it) }
            folder?.let { putString(FOLDER_ARGUMENT, it) }
            consentSeconds?.let { putString(CONSENT_ARGUMENT, it) }
        }

    private fun refusalOf(arguments: Bundle): String =
        when (val request = providerRequestOf(arguments)) {
            is ProviderRequest.Refused -> request.reason
            is ProviderRequest.Given -> throw AssertionError("expected a refusal")
        }

    private fun givenOf(arguments: Bundle): ProviderRequest.Given =
        when (val request = providerRequestOf(arguments)) {
            is ProviderRequest.Refused -> throw AssertionError("expected inputs: ${request.reason}")
            is ProviderRequest.Given -> request
        }

    private companion object {

        const val ACCOUNT = "42000001"

        const val OWNER = "synthetic-owner"

        const val REPOSITORY = "synthetic-notes"

        const val BRANCH = "master"

        val REPORTED =
            listOf(
                "configured",
                "codeShown",
                "authorized",
                "identityMatch",
                "installationsListed",
                "repositoryDiscoverable",
                "branchesListed",
                "branchResolved",
                "folderListed",
                "selectionConfirmed",
                "foreignAccountRefused",
                "unauthorizedRepositoryRefused",
                "ownedCleanup",
                "refusal",
                "passed",
            )
    }
}
