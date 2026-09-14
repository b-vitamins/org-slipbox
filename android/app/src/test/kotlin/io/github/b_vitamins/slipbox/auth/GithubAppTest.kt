/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import io.github.b_vitamins.slipbox.BuildConfig
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class GithubAppTest {

    @Test
    fun aBuildWithNoClientIdCarriesNoRegistration() {
        assertNull(GithubApp.of("", INSTALLATION_URL))
    }

    @Test
    fun aClientIdIsTheWholeOfWhatARegistrationNeeds() {
        val app = requireNotNull(GithubApp.of(CLIENT_ID, ""))

        assertEquals(CLIENT_ID, app.clientId)
        assertNull("an absent installation page was invented", app.installationUrl)
    }

    @Test
    fun anInstallationPageIsKeptOnlyWhenItIsHttps() {
        assertEquals(INSTALLATION_URL, GithubApp.of(CLIENT_ID, INSTALLATION_URL)?.installationUrl)
        assertNull(GithubApp.of(CLIENT_ID, "http://github.com/apps/slipbox")?.installationUrl)
        assertNull(GithubApp.of(CLIENT_ID, "github.com/apps/slipbox")?.installationUrl)
        assertNotNull(GithubApp.of(CLIENT_ID, "http://github.com/apps/slipbox"))
    }

    @Test
    fun configurationThatCannotBeSentIsNoRegistrationAtAll() {
        assertNull(GithubApp.of(" ", INSTALLATION_URL))
        assertNull(GithubApp.of("client id", INSTALLATION_URL))
        assertNull(GithubApp.of("client\nid", INSTALLATION_URL))
        assertNull(GithubApp.of("x".repeat(GithubApp.MAX_CONFIGURATION_LENGTH + 1), ""))
        assertNotNull(GithubApp.of("x".repeat(GithubApp.MAX_CONFIGURATION_LENGTH), ""))
    }

    @Test
    fun theRegistrationNamesTheAuthorityItsCredentialsAreScopedBy() {
        assertEquals("github.com", GithubApp.PROVIDER_AUTHORITY)
    }


    @Test
    fun thePackagedConfigurationIsPublicRatherThanACredential() {
        val clientId = BuildConfig.GITHUB_CLIENT_ID
        val installationUrl = BuildConfig.GITHUB_INSTALLATION_URL

        for (prefix in CREDENTIAL_PREFIXES) {
            assertFalse(
                "the packaged client id is spelled like a credential beginning $prefix",
                clientId.startsWith(prefix),
            )
            assertFalse(
                "the packaged installation page is spelled like a credential beginning $prefix",
                installationUrl.startsWith(prefix),
            )
        }
        assertTrue(
            "the packaged client id is longer than a client id",
            clientId.length <= GithubApp.MAX_CONFIGURATION_LENGTH,
        )
        assertTrue(
            "the packaged installation page is not an https address",
            installationUrl.isEmpty() || installationUrl.startsWith("https://"),
        )
        assertEquals(clientId.isNotEmpty(), GithubApp.packaged() != null)
    }

    private companion object {


        val CREDENTIAL_PREFIXES =
            listOf("ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_")
    }
}
