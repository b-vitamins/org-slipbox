/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import io.github.b_vitamins.slipbox.auth.RepositorySelection
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class GithubProviderTest {

    @Test
    fun installationsBeyondTheFirstPageAreNotLost() {
        val second = "${installationsUrl()}&page=2"
        val transport =
            RecordedApiTransport()
                .answers(
                    installationsUrl(),
                    answered(
                        installationsBody(installationEntry()),
                        link = nextLink(second),
                    ),
                )
                .answers(
                    second,
                    answered(installationsBody(installationEntry(id = "77000009"))),
                )

        val listing = testProvider(transport).installations().read()

        assertEquals(listOf(INSTALLATION_ID, "77000009"), listing.entries.map { it.id })
        assertTrue("both pages were read", listing.isComplete)
        assertEquals(listOf(installationsUrl(), second), transport.urls)
    }

    @Test
    fun everyProviderRequestCarriesTheAuthorizationBearer() {
        val transport =
            RecordedApiTransport()
                .answers(installationsUrl(), answered(installationsBody(installationEntry())))

        testProvider(transport).installations().read()

        assertEquals(listOf(ACCESS_TOKEN), transport.requests.map { it.bearer })
    }

    @Test
    fun repositoriesComeFromTheInstallationTheUserTokenReaches() {
        val second = "${repositoriesUrl()}&page=2"
        val transport =
            RecordedApiTransport()
                .answers(
                    repositoriesUrl(),
                    answered(
                        repositoriesBody(repositoryEntry()),
                        link = nextLink(second),
                    ),
                )
                .answers(
                    second,
                    answered(
                        repositoriesBody(repositoryEntry(id = "88000009", name = "second-notes")),
                    ),
                )

        val listing = testProvider(transport).repositories(testInstallation()).read()

        assertEquals(listOf(REPOSITORY_ID, "88000009"), listing.entries.map { it.id })
        assertTrue(listing.isComplete)
        assertTrue(
            "no installation-token or account-wide endpoint is used",
            transport.urls.all { it.startsWith("$API/user/installations/") },
        )
    }

    @Test
    fun aRepositoryCarriesItsOwnCredentialFreeRemote() {
        val transport =
            RecordedApiTransport()
                .answers(repositoriesUrl(), answered(repositoriesBody(repositoryEntry())))

        val repository = testProvider(transport).repositories(testInstallation()).read().entries.single()

        assertEquals("https://github.com/$OWNER/$REPOSITORY.git", repository.remoteUrl)
        assertFalse("a remote never carries a token", repository.remoteUrl.contains(ACCESS_TOKEN))
        assertEquals(ACCOUNT_ID, repository.ownerAccountId)
        assertEquals(INSTALLATION_ID, repository.installationId)
        assertTrue(repository.isPrivate)
    }

    @Test
    fun anAccountWithoutRepositoriesIsEmptyRatherThanRefused() {
        val transport =
            RecordedApiTransport().answers(repositoriesUrl(), answered(repositoriesBody()))
        val installation = testInstallation(selection = RepositorySelection.All)

        val listing = testProvider(transport).repositories(installation).read()

        assertTrue(listing.entries.isEmpty())
        assertTrue("an unrestricted installation genuinely has none", listing.isComplete)
    }

    @Test
    fun aRestrictedInstallationShowingNothingIsARefusalWithRecovery() {
        val transport =
            RecordedApiTransport().answers(repositoriesUrl(), answered(repositoriesBody()))

        val refusal = testProvider(transport).repositories(testInstallation()).refusal()

        val restricted = refusal as GithubRefusal.AccessRestricted
        assertEquals(AccessRestriction.RepositoryUnselected, restricted.restriction)
        assertEquals(GithubRecoveryAction.Install, restricted.recovery?.action)
        assertEquals(INSTALLATION_URL, restricted.recovery?.url)
    }

    @Test
    fun anAccountWithNoInstallationIsARefusalNamingTheInstallationPage() {
        val transport =
            RecordedApiTransport().answers(installationsUrl(), answered(installationsBody()))

        val refusal = testProvider(transport).installations().refusal()

        val restricted = refusal as GithubRefusal.AccessRestricted
        assertEquals(AccessRestriction.InstallationMissing, restricted.restriction)
        assertEquals(INSTALLATION_URL, restricted.recovery?.url)
    }

    @Test
    fun anUnconfiguredAppOffersNoFabricatedRecoveryPage() {
        val transport =
            RecordedApiTransport().answers(installationsUrl(), answered(installationsBody()))

        val refusal = testProvider(transport, app = null).installations().refusal()

        assertEquals(GithubRecoveryAction.Install, refusal.recovery?.action)
        assertEquals(null, refusal.recovery?.url)
    }

    @Test
    fun branchesBeyondTheFirstPageAreNotLost() {
        val second = "${branchesUrl()}&page=2"
        val transport =
            RecordedApiTransport()
                .answers(
                    branchesUrl(),
                    answered(branchesBody(branchEntry()), link = nextLink(second)),
                )
                .answers(
                    second,
                    answered(
                        branchesBody(branchEntry(name = "feature/notes", commitSha = NESTED_TREE_SHA)),
                    ),
                )

        val listing = testProvider(transport).branches(testRepository()).read()

        assertEquals(listOf(BRANCH, "feature/notes"), listing.entries.map { it.name })
        assertTrue(listing.isComplete)
    }

    @Test
    fun anEmptyRepositoryHasNoBranchesAndSaysSo() {
        val transport = RecordedApiTransport().answers(branchesUrl(), answered(branchesBody()))

        val listing = testProvider(transport).branches(testRepository()).read()

        assertTrue(listing.entries.isEmpty())
        assertTrue(listing.isComplete)
    }

    @Test
    fun anExplicitBranchIsVerifiedAndItsPathIsEncoded() {
        val encoded = branchUrl(branch = "feature/notes-caf%C3%A9")
        val transport =
            RecordedApiTransport()
                .answers(
                    encoded,
                    answered(branchEntry(name = "feature/notes-café", commitSha = COMMIT_SHA)),
                )

        val branch = testProvider(transport).branch(testRepository(), "feature/notes-café").read()

        assertEquals("feature/notes-café", branch.name)
        assertEquals(COMMIT_SHA, branch.commitSha)
        assertEquals(listOf(encoded), transport.urls)
    }

    @Test
    fun aBranchAnsweringUnderAnotherNameIsNotAccepted() {
        val transport =
            RecordedApiTransport().answers(branchUrl(), answered(branchEntry(name = "other")))

        val refusal = testProvider(transport).branch(testRepository(), BRANCH).refusal()

        assertEquals(
            GithubRefusal.MalformedAnswer(GithubStage.Branch, GithubDefect.FieldUnusable),
            refusal,
        )
    }

    @Test
    fun aBranchNameTheCoreWouldRejectIsRefusedBeforeAnyRequest() {
        val transport = RecordedApiTransport()

        val refusal = testProvider(transport).branch(testRepository(), "../escape").refusal()

        assertEquals(
            GithubRefusal.SelectionUnusable(GithubStage.Branch, SelectionField.Branch),
            refusal,
        )
        assertTrue("nothing was requested", transport.urls.isEmpty())
    }

    @Test
    fun aPaginationDestinationOffThePinnedOriginIsRefused() {
        val transport =
            RecordedApiTransport()
                .answers(
                    installationsUrl(),
                    answered(
                        installationsBody(installationEntry()),
                        link = nextLink("https://api.github.example/user/installations?page=2"),
                    ),
                )

        val refusal = testProvider(transport).installations().refusal()

        assertEquals(
            GithubRefusal.MalformedAnswer(
                GithubStage.Installations,
                GithubDefect.ForeignDestination,
            ),
            refusal,
        )
        assertEquals("the foreign page was never fetched", 1, transport.urls.size)
    }

    @Test
    fun aPaginationDestinationOnAnotherEndpointIsRefused() {
        val transport =
            RecordedApiTransport()
                .answers(
                    installationsUrl(),
                    answered(
                        installationsBody(installationEntry()),
                        link = nextLink("$API/user/repos?page=2"),
                    ),
                )

        val refusal = testProvider(transport).installations().refusal()

        assertEquals(
            GithubRefusal.MalformedAnswer(
                GithubStage.Installations,
                GithubDefect.ForeignDestination,
            ),
            refusal,
        )
    }

    @Test(timeout = WAIT_MILLIS)
    fun aLocalPageCapIsReportedAsIncompleteNotAsComplete() {
        val endless = "${installationsUrl()}&page=2"
        val transport =
            RecordedApiTransport()
                .answers(
                    installationsUrl(),
                    answered(installationsBody(installationEntry()), link = nextLink(endless)),
                )
                .answers(
                    endless,
                    answered(
                        installationsBody(installationEntry(id = "77000099")),
                        link = nextLink(endless),
                    ),
                )

        val listing = testProvider(transport).installations().read()

        assertEquals(GithubCompleteness.LocallyCapped, listing.completeness)
        assertFalse("a local safety cap is not completeness", listing.isComplete)
        assertEquals(listing.entries.size, transport.urls.size)
    }

    @Test
    fun aRedirectIsRefusedRatherThanFollowed() {
        val transport = RecordedApiTransport().answers(installationsUrl(), answered("", status = 302))

        val refusal = testProvider(transport).installations().refusal()

        assertEquals(GithubRefusal.Redirected(GithubStage.Installations), refusal)
    }

    @Test
    fun aTraversalAbandonedByItsOwnerStopsBetweenPages() {
        val second = "${installationsUrl()}&page=2"
        val transport =
            RecordedApiTransport()
                .answers(
                    installationsUrl(),
                    answered(installationsBody(installationEntry()), link = nextLink(second)),
                )
        var live = true
        transport.beforeFetch = { live = false }

        val refusal = testProvider(transport, live = { live }).installations().refusal()

        assertEquals(GithubRefusal.Cancelled(GithubStage.Installations), refusal)
        assertEquals(listOf(installationsUrl()), transport.urls)
    }
}
