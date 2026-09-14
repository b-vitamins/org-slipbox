/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class GithubSelectionTest {

    @Test
    fun aConfirmedSelectionCarriesIdentifiersRemoteBranchAndFolder() {
        val transport =
            RecordedApiTransport()
                .answers(installationsUrl(), answered(installationsBody(installationEntry())))
                .answers(repositoriesUrl(), answered(repositoriesBody(repositoryEntry())))
                .answers(branchUrl(), answered(branchEntry()))
                .answers(treeUrl(), answered(treeBody(treeEntry("notes"))))
                .answers(treeUrl(sha = FOLDER_TREE_SHA), answered(treeBody()))

        val selection = testProvider(transport).confirm(request(folder = "notes")).read()

        assertEquals(ACCOUNT_ID, selection.accountId)
        assertEquals(ACCOUNT_ID, selection.ownerAccountId)
        assertEquals(INSTALLATION_ID, selection.installationId)
        assertEquals(REPOSITORY_ID, selection.repositoryId)
        assertEquals(OWNER, selection.owner)
        assertEquals(REPOSITORY, selection.name)
        assertEquals("https://github.com/$OWNER/$REPOSITORY.git", selection.remoteUrl)
        assertEquals(BRANCH, selection.branch)
        assertEquals("notes", selection.notesFolder)
        assertFalse("a confirmed remote is credential free", selection.remoteUrl.contains(ACCESS_TOKEN))
    }

    @Test
    fun theRootFolderConfirmsAsTheEmptyPathWithoutATreeRead() {
        val transport =
            RecordedApiTransport()
                .answers(installationsUrl(), answered(installationsBody(installationEntry())))
                .answers(repositoriesUrl(), answered(repositoriesBody(repositoryEntry())))
                .answers(branchUrl(), answered(branchEntry()))

        val selection = testProvider(transport).confirm(request(folder = ".")).read()

        assertEquals("", selection.notesFolder)
        assertTrue(transport.urls.none { it.contains("/git/trees/") })
    }

    @Test
    fun aRenamedRepositoryStillConfirmsUnderItsIdentifier() {
        val renamed = "renamed-notes"
        val transport =
            RecordedApiTransport()
                .answers(installationsUrl(), answered(installationsBody(installationEntry())))
                .answers(
                    repositoriesUrl(),
                    answered(repositoriesBody(repositoryEntry(name = renamed))),
                )
                .answers(branchUrl(repository = renamed), answered(branchEntry()))

        val selection = testProvider(transport).confirm(request()).read()

        assertEquals("a display name is not identity", REPOSITORY_ID, selection.repositoryId)
        assertEquals(renamed, selection.name)
        assertEquals("https://github.com/$OWNER/$renamed.git", selection.remoteUrl)
    }

    @Test
    fun aSelectionMadeUnderAnotherAccountIsRefusedBeforeAnyRequest() {
        val transport = RecordedApiTransport()

        val refusal = testProvider(transport).confirm(request(accountId = "52000099")).refusal()

        assertEquals(
            GithubRefusal.SelectionStale(GithubStage.Selection, StaleSelection.Account),
            refusal,
        )
        assertTrue("a foreign account is never queried", transport.urls.isEmpty())
    }

    @Test
    fun anInstallationThatIsGoneIsRefusedAsStale() {
        val transport =
            RecordedApiTransport()
                .answers(
                    installationsUrl(),
                    answered(installationsBody(installationEntry(id = "77000099"))),
                )

        val refusal = testProvider(transport).confirm(request()).refusal()

        assertEquals(
            GithubRefusal.SelectionStale(GithubStage.Selection, StaleSelection.Installation),
            refusal,
        )
    }

    @Test
    fun aRepositoryOutsideTheInstallationCannotBeSelected() {
        val transport =
            RecordedApiTransport()
                .answers(installationsUrl(), answered(installationsBody(installationEntry())))
                .answers(
                    repositoriesUrl(),
                    answered(repositoriesBody(repositoryEntry(id = "88000099", name = "other"))),
                )

        val refusal = testProvider(transport).confirm(request()).refusal()

        val restricted = refusal as GithubRefusal.AccessRestricted
        assertEquals(GithubStage.Selection, restricted.stage)
        assertEquals(AccessRestriction.RepositoryUnselected, restricted.restriction)
        assertEquals(GithubRecoveryAction.Install, restricted.recovery?.action)
        assertTrue(
            "an unauthorized repository is never read",
            transport.urls.none { it.startsWith("$API/repos/") },
        )
    }

    @Test(timeout = WAIT_MILLIS)
    fun anIncompleteListingCannotProveASelectionIsGone() {
        val endless = "${installationsUrl()}&page=2"
        val transport =
            RecordedApiTransport()
                .answers(
                    installationsUrl(),
                    answered(
                        installationsBody(installationEntry(id = "77000099")),
                        link = nextLink(endless),
                    ),
                )
                .answers(
                    endless,
                    answered(
                        installationsBody(installationEntry(id = "77000099")),
                        link = nextLink(endless),
                    ),
                )

        val refusal = testProvider(transport).confirm(request()).refusal()

        assertEquals(
            GithubRefusal.ListingIncomplete(
                GithubStage.Selection,
                GithubCompleteness.LocallyCapped,
            ),
            refusal,
        )
    }

    @Test
    fun aBranchThatIsGoneIsRefused() {
        val transport =
            RecordedApiTransport()
                .answers(installationsUrl(), answered(installationsBody(installationEntry())))
                .answers(repositoriesUrl(), answered(repositoriesBody(repositoryEntry())))
                .answers(branchUrl(), answered("", status = 404))

        val refusal = testProvider(transport).confirm(request()).refusal()

        assertEquals(GithubRefusal.ResourceHidden(GithubStage.Branch), refusal)
    }

    @Test
    fun aNotesFolderThatIsGoneIsRefused() {
        val transport =
            RecordedApiTransport()
                .answers(installationsUrl(), answered(installationsBody(installationEntry())))
                .answers(repositoriesUrl(), answered(repositoriesBody(repositoryEntry())))
                .answers(branchUrl(), answered(branchEntry()))
                .answers(treeUrl(), answered(treeBody(treeEntry("assets"))))

        val refusal = testProvider(transport).confirm(request(folder = "notes")).refusal()

        assertEquals(GithubRefusal.ResourceHidden(GithubStage.Folder), refusal)
    }

    @Test
    fun aStoredFolderTheCoreWouldRejectIsRefusedAsUnusable() {
        val transport =
            RecordedApiTransport()
                .answers(installationsUrl(), answered(installationsBody(installationEntry())))
                .answers(repositoriesUrl(), answered(repositoriesBody(repositoryEntry())))
                .answers(branchUrl(), answered(branchEntry()))

        val refusal = testProvider(transport).confirm(request(folder = "notes/.git")).refusal()

        assertEquals(
            GithubRefusal.SelectionUnusable(GithubStage.Selection, SelectionField.Folder),
            refusal,
        )
    }

    private fun request(
        accountId: String = ACCOUNT_ID,
        installationId: String = INSTALLATION_ID,
        repositoryId: String = REPOSITORY_ID,
        branch: String = BRANCH,
        folder: String = "",
    ): GithubSelectionRequest =
        GithubSelectionRequest(accountId, installationId, repositoryId, branch, folder)
}
