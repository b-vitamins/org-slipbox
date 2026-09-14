/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class GithubAddressTest {

    @Test
    fun anAddressCarriesThePinnedOriginAndTheRequestedPageSize() {
        val address = GithubAddress.paged("/user/installations")

        assertEquals("$API/user/installations?per_page=100", address?.url)
    }

    @Test
    fun onlyTheApiOriginOnItsDefaultPortIsAccepted() {
        val installations = requireNotNull(GithubAddress.of("/user/installations"))

        assertNull(
            "another host is not the provider",
            installations.continuation("https://api.github.example/user/installations?page=2"),
        )
        assertNull(
            "a credential is not carried to plain HTTP",
            installations.continuation("http://api.github.com/user/installations?page=2"),
        )
        assertNull(
            "an alternate port is a different origin",
            installations.continuation("https://api.github.com:8443/user/installations?page=2"),
        )
        assertNull(
            "an authority may not name a user",
            installations.continuation("https://evil@api.github.com/user/installations?page=2"),
        )
        assertNotNull(
            "the same endpoint on the next page continues",
            installations.continuation("https://api.github.com/user/installations?page=2"),
        )
    }

    @Test
    fun aContinuationMustStayOnTheEndpointThatProducedIt() {
        val branches = requireNotNull(GithubAddress.of("/repos/$OWNER/$REPOSITORY/branches"))

        assertNull(
            "paging may not walk to another endpoint",
            branches.continuation("https://api.github.com/user/repos?page=2"),
        )
        assertNotNull(
            branches.continuation("https://api.github.com/repos/$OWNER/$REPOSITORY/branches?page=3"),
        )
    }

    @Test
    fun embeddedDotsInARepositoryNameAreNotTraversalSegments() {
        val path = "/repos/$OWNER/notes..archive/branches"
        val address = GithubAddress.of(path)

        assertNotNull(address)
        assertNotNull(address?.continuation("$API$path?page=2"))
        assertNull(GithubAddress.of("/repos/$OWNER/../branches"))
        assertNull(GithubAddress.of("/repos/$OWNER/./branches"))
    }

    @Test
    fun theNextRelationIsReadPastOtherRelations() {
        val header =
            """<https://api.github.com/user/installations?page=1>; rel="prev", """ +
                """<https://api.github.com/user/installations?page=3>; rel=next, """ +
                """<https://api.github.com/user/installations?page=9>; rel="last""""

        assertEquals(
            "https://api.github.com/user/installations?page=3",
            GithubAddress.nextTarget(header),
        )
    }

    @Test
    fun aLinkWithoutANextRelationEndsThePaging() {
        val header =
            """<https://api.github.com/user/installations?page=1>; rel="first", """ +
                """<https://api.github.com/user/installations?page=9>; rel="last""""

        assertNull(GithubAddress.nextTarget(header))
        assertNull(GithubAddress.nextTarget(null))
        assertNull("an oversized header is not parsed", GithubAddress.nextTarget("<".repeat(5_000)))
    }

    @Test
    fun pathPartsKeepSeparatorsAndSegmentsDoNot() {
        assertEquals("feature/notes%20draft", GithubEncoding.path("feature/notes draft"))
        assertEquals("feature%2Fnotes", GithubEncoding.segment("feature/notes"))
        assertEquals("caf%C3%A9", GithubEncoding.segment("café"))
        assertEquals("a%23b%3Fc", GithubEncoding.segment("a#b?c"))
        assertEquals("plain-name_1.org", GithubEncoding.segment("plain-name_1.org"))
    }
}
