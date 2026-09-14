/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class GithubFolderTest {

    @Test
    fun theRootFolderIsReadAtTheVerifiedBranchCommit() {
        val transport =
            RecordedApiTransport()
                .answers(
                    treeUrl(),
                    answered(
                        treeBody(
                            treeEntry("notes"),
                            treeEntry("README.org", type = "blob", mode = "100644"),
                        ),
                    ),
                )

        val listing = testProvider(transport).folder(testRepository(), testBranch(), "").read()

        assertEquals(listOf("notes", "README.org"), listing.entries.map { it.name })
        assertEquals(listOf("notes", "README.org"), listing.entries.map { it.path })
        assertEquals(
            listOf(GithubEntryKind.Folder, GithubEntryKind.File),
            listing.entries.map { it.kind },
        )
        assertTrue(listing.isComplete)
        assertEquals(listOf(treeUrl()), transport.urls)
    }

    @Test
    fun aNestedFolderReadsOnlyTheTreesAlongItsPath() {
        val transport =
            RecordedApiTransport()
                .answers(treeUrl(), answered(treeBody(treeEntry("notes"), treeEntry("assets"))))
                .answers(
                    treeUrl(sha = FOLDER_TREE_SHA),
                    answered(treeBody(treeEntry("inbox", sha = NESTED_TREE_SHA))),
                )
                .answers(
                    treeUrl(sha = NESTED_TREE_SHA),
                    answered(treeBody(treeEntry("today.org", type = "blob", mode = "100644"))),
                )

        val listing =
            testProvider(transport).folder(testRepository(), testBranch(), "notes/inbox").read()

        assertEquals(listOf("notes/inbox/today.org"), listing.entries.map { it.path })
        assertEquals(
            "one non-recursive read per level and no recursive tree",
            listOf(treeUrl(), treeUrl(sha = FOLDER_TREE_SHA), treeUrl(sha = NESTED_TREE_SHA)),
            transport.urls,
        )
        assertTrue(transport.urls.none { it.contains("recursive") })
    }

    @Test
    fun aFolderNeedingEncodingIsMatchedByNameAndNeverByUrl() {
        val transport =
            RecordedApiTransport()
                .answers(treeUrl(), answered(treeBody(treeEntry("notes café"))))
                .answers(
                    treeUrl(sha = FOLDER_TREE_SHA),
                    answered(treeBody(treeEntry("draft one", type = "blob", mode = "100644"))),
                )

        val listing =
            testProvider(transport).folder(testRepository(), testBranch(), "notes café").read()

        assertEquals(listOf("notes café/draft one"), listing.entries.map { it.path })
        assertTrue(
            "a folder name reaches the provider as a tree digest, never as a path",
            transport.urls.all { it.startsWith("$API/repos/$OWNER/$REPOSITORY/git/trees/") },
        )
    }

    @Test
    fun aTruncatedDirectoryIsReportedAsTruncatedNotAsComplete() {
        val transport =
            RecordedApiTransport()
                .answers(treeUrl(), answered(treeBody(treeEntry("notes"), truncated = true)))

        val listing = testProvider(transport).folder(testRepository(), testBranch(), "").read()

        assertEquals(1, listing.entries.size)
        assertEquals(GithubCompleteness.ProviderTruncated, listing.completeness)
        assertFalse("truncation is never passed off as completeness", listing.isComplete)
    }

    @Test
    fun aNameMissingFromATruncatedLevelIsIncompleteRatherThanHidden() {
        val transport =
            RecordedApiTransport()
                .answers(treeUrl(), answered(treeBody(treeEntry("assets"), truncated = true)))

        val refusal = testProvider(transport).folder(testRepository(), testBranch(), "notes").refusal()

        assertEquals(
            GithubRefusal.ListingIncomplete(
                GithubStage.Folder,
                GithubCompleteness.ProviderTruncated,
            ),
            refusal,
        )
    }

    @Test
    fun aNameMissingFromACompleteLevelIsHidden() {
        val transport =
            RecordedApiTransport().answers(treeUrl(), answered(treeBody(treeEntry("assets"))))

        val refusal = testProvider(transport).folder(testRepository(), testBranch(), "notes").refusal()

        assertEquals(GithubRefusal.ResourceHidden(GithubStage.Folder), refusal)
    }

    @Test
    fun aSymlinkSubmoduleOrFileOnThePathIsNotFollowed() {
        val cases =
            listOf(
                treeEntry("notes", type = "blob", mode = "120000") to GithubEntryKind.Symlink,
                treeEntry("notes", type = "commit", mode = "160000") to GithubEntryKind.Submodule,
                treeEntry("notes", type = "blob", mode = "100644") to GithubEntryKind.File,
            )

        for ((entry, kind) in cases) {
            val transport = RecordedApiTransport().answers(treeUrl(), answered(treeBody(entry)))

            val refusal =
                testProvider(transport).folder(testRepository(), testBranch(), "notes").refusal()

            assertEquals(GithubRefusal.EntryNotFolder(GithubStage.Folder, kind), refusal)
            assertEquals("no folder was invented past it", listOf(treeUrl()), transport.urls)
        }
    }

    @Test
    fun symlinksAndSubmodulesAreListedAsWhatTheyAre() {
        val transport =
            RecordedApiTransport()
                .answers(
                    treeUrl(),
                    answered(
                        treeBody(
                            treeEntry("elsewhere", type = "blob", mode = "120000"),
                            treeEntry("vendor", type = "commit", mode = "160000"),
                        ),
                    ),
                )

        val listing = testProvider(transport).folder(testRepository(), testBranch(), "").read()

        assertEquals(
            listOf(GithubEntryKind.Symlink, GithubEntryKind.Submodule),
            listing.entries.map { it.kind },
        )
    }

    @Test
    fun anEmptyFolderIsEmptyAndComplete() {
        val transport = RecordedApiTransport().answers(treeUrl(), answered(treeBody()))

        val listing = testProvider(transport).folder(testRepository(), testBranch(), ".").read()

        assertTrue(listing.entries.isEmpty())
        assertTrue(listing.isComplete)
    }

    @Test
    fun aMissingTreeIsNotAnEmptyFolder() {
        val transport = RecordedApiTransport().answers(treeUrl(), answered("{}"))

        val refusal = testProvider(transport).folder(testRepository(), testBranch(), "").refusal()

        assertEquals(
            GithubRefusal.MalformedAnswer(GithubStage.Folder, GithubDefect.FieldUnusable),
            refusal,
        )
    }

    @Test
    fun aFolderPathTheCoreWouldRejectIsRefusedBeforeAnyRequest() {
        val transport = RecordedApiTransport()

        val refusal =
            testProvider(transport).folder(testRepository(), testBranch(), "notes/../escape").refusal()

        assertEquals(
            GithubRefusal.SelectionUnusable(GithubStage.Folder, SelectionField.Folder),
            refusal,
        )
        assertTrue(transport.urls.isEmpty())
    }

    @Test
    fun anEntryNamingAPathSeparatorIsNotAcceptedAsOneName() {
        val transport =
            RecordedApiTransport().answers(treeUrl(), answered(treeBody(treeEntry("notes/inbox"))))

        val refusal = testProvider(transport).folder(testRepository(), testBranch(), "").refusal()

        assertEquals(
            GithubRefusal.MalformedAnswer(GithubStage.Folder, GithubDefect.FieldUnusable),
            refusal,
        )
    }
}
