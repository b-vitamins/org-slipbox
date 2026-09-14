/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class GithubNamesTest {

    @Test
    fun aBranchFollowsTheReferenceGrammarTheCoreEnforces() {
        assertEquals("master", GithubNames.branch("master"))
        assertEquals("feature/notes", GithubNames.branch("feature/notes"))
        assertEquals("release-1.2", GithubNames.branch("release-1.2"))
    }

    @Test
    fun referenceNamesTheCoreWouldRejectNeverReachTheProvider() {
        val refused =
            listOf(
                "",
                "HEAD",
                "@",
                "a..b",
                "main@{1}",
                "-lead",
                "/lead",
                "trail/",
                "trail.",
                "double//slash",
                "with space",
                "with~tilde",
                "with^caret",
                "with:colon",
                "with?question",
                "with*star",
                "with[bracket",
                "with\\backslash",
                ".hidden/name",
                "name/.hidden",
                "name.lock",
                "a".repeat(256),
            )

        for (candidate in refused) {
            assertNull("refused: $candidate", GithubNames.branch(candidate))
        }
    }

    @Test
    fun theRootFolderIsTheEmptyPath() {
        assertEquals("", GithubNames.folder(""))
        assertEquals("", GithubNames.folder("."))
        assertEquals("notes", GithubNames.folder("notes"))
        assertEquals("notes/inbox", GithubNames.folder("notes/inbox"))
    }

    @Test
    fun aFolderCannotEscapeTheRepositoryOrNameGitData() {
        val refused =
            listOf(
                "/absolute",
                "notes/",
                "notes//inbox",
                "notes/../escape",
                "..",
                "notes/./inbox",
                ".git",
                "notes/.git",
                "notes\\inbox",
                "notes:inbox",
                "a".repeat(513),
                (1..33).joinToString("/") { "deep" },
            )

        for (candidate in refused) {
            assertNull("refused: $candidate", GithubNames.folder(candidate))
        }
    }

    @Test
    fun ownerAndRepositoryNamesStayUrlSafe() {
        assertEquals(OWNER, GithubNames.name(OWNER))
        assertEquals("notes.org", GithubNames.name("notes.org"))
        assertNull(GithubNames.name(""))
        assertNull(GithubNames.name("."))
        assertNull(GithubNames.name(".."))
        assertNull(GithubNames.name("owner/repository"))
        assertNull(GithubNames.name("owner name"))
        assertNull(GithubNames.name("a".repeat(101)))
    }

    @Test
    fun onlyAFullCommitDigestPinsAFolderRead() {
        assertEquals(COMMIT_SHA, GithubNames.commit(COMMIT_SHA))
        assertNull(GithubNames.commit(COMMIT_SHA.take(7)))
        assertNull(GithubNames.commit(COMMIT_SHA.uppercase()))
        assertNull(GithubNames.commit("z".repeat(40)))
    }
}
