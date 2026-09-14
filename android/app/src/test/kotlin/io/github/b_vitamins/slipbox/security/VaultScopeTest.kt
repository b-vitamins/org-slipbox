/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class VaultScopeTest {

    @Test
    fun everyUnusableFieldIsRefusedBeforeAnyKeyOrPathExists() {
        assertEquals(
            VaultFailure.RefusedInput("source id", InputDefect.Empty),
            VaultScope.of("", "provider.example", "account-1", "credential-1").refusal(),
        )
        assertEquals(
            VaultFailure.RefusedInput("provider authority", InputDefect.TooLong),
            VaultScope
                .of("source-1", "p".repeat(VaultScope.MAX_FIELD_LENGTH + 1), "a", "c")
                .refusal(),
        )
        assertEquals(
            VaultFailure.RefusedInput("account id", InputDefect.NotPlainText),
            VaultScope.of("source-1", "provider.example", "account 1", "credential-1").refusal(),
        )
        assertEquals(
            VaultFailure.RefusedInput("credential reference", InputDefect.NotPlainText),
            VaultScope.of("source-1", "provider.example", "account-1", "credential\n1").refusal(),
        )
        assertEquals(
            VaultFailure.RefusedInput("credential reference", InputDefect.Empty),
            VaultScope.of("source-1", "provider.example", "account-1", "").refusal(),
        )
    }

    @Test
    fun theDigestIsFixedWidthHexAndDependsOnEveryField() {
        val scope = testScope()
        assertEquals(DIGEST_HEX_LENGTH, scope.digestHex.length.toLong())
        assertTrue(scope.digestHex, scope.digestHex.all { it in '0'..'9' || it in 'a'..'f' })

        val digests =
            setOf(
                scope.digestHex,
                testScope(sourceId = "source-2").digestHex,
                testScope(providerAuthority = "other.example").digestHex,
                testScope(accountId = "account-2").digestHex,
                testScope(credentialRef = "credential-2").digestHex,
            )
        assertEquals(5, digests.size.toLong())
    }

    @Test
    fun noTwoScopesShareADigestByRunningTheirFieldsTogether() {
        assertNotEquals(
            testScope(sourceId = "ab", providerAuthority = "c.example").digestHex,
            testScope(sourceId = "a", providerAuthority = "bc.example").digestHex,
        )
    }

    @Test
    fun theAccountDigestOutlivesTheCredentialReference() {
        val first = testScope(credentialRef = "credential-1")
        val second = testScope(credentialRef = "credential-2")
        assertEquals(first.accountDigestHex, second.accountDigestHex)
        assertNotEquals(first.digestHex, second.digestHex)
        assertNotEquals(first.digestHex, first.accountDigestHex)

        assertNotEquals(
            first.accountDigestHex,
            testScope(accountId = "account-2").accountDigestHex,
        )
    }

    @Test
    fun aScopePrintsItsAuthorityAndAShortDigestOnly() {
        val scope = testScope()
        val printed = scope.toString()
        assertTrue(printed, printed.contains("provider.example"))
        assertFalse(printed, printed.contains("source-1"))
        assertFalse(printed, printed.contains("account-1"))
        assertFalse(printed, printed.contains("credential-1"))
        assertFalse(printed, printed.contains(scope.digestHex))
    }

    @Test
    fun scopesAreTheSameScopeWhenTheirDigestsAgree() {
        assertEquals(testScope(), testScope())
        assertEquals(testScope().hashCode(), testScope().hashCode())
        assertNotEquals(testScope(), testScope(credentialRef = "credential-2"))
    }

    @Test
    fun theKeyOfAScopeIsNamedFromItsDigestItsNameSpaceAndItsGeneration() {
        val scope = testScope()
        val prefix = VaultNamespace.Application.aliasPrefixFor(scope)
        assertEquals("slipbox.vault.1.app.${scope.digestHex}.", prefix)

        for (generation in GENERATIONS) {
            val alias = VaultNamespace.Application.aliasFor(scope, generation)
            assertEquals(prefix + generation, alias)
            assertTrue(alias, alias.length <= VaultNamespace.MAX_ALIAS_LENGTH)
            assertTrue(alias, alias.all { it in '!'..'~' })
            assertFalse(alias, alias.contains("account-1"))
            assertFalse(alias, alias.contains("credential-1"))
        }
        assertFalse(prefix, prefix.contains(scope.accountDigestHex))
    }

    @Test
    fun everyGenerationOfAScopeIsNamedApartAndTheLastOneWrapsToTheFirst() {
        val scope = testScope()
        val prefix = VaultNamespace.Application.aliasPrefixFor(scope)
        val aliases = mutableSetOf<String>()

        var generation = VaultNamespace.nextGeneration(null)
        assertEquals(VaultNamespace.FIRST_GENERATION.toLong(), generation.toLong())
        repeat(VaultNamespace.LAST_GENERATION) {
            val alias = VaultNamespace.Application.aliasFor(scope, generation)
            assertTrue(alias, alias.startsWith(prefix))
            assertTrue(alias, alias.length <= VaultNamespace.MAX_ALIAS_LENGTH)
            aliases.add(alias)
            generation = VaultNamespace.nextGeneration(generation)
        }

        assertEquals(VaultNamespace.LAST_GENERATION.toLong(), aliases.size.toLong())
        assertEquals(
            "the generation after the last one does not start over",
            VaultNamespace.FIRST_GENERATION.toLong(),
            generation.toLong(),
        )
    }

    @Test
    fun aTestRunIsNamedApartFromTheApplicationInBothKeysAndFiles() {
        val scope = testScope()
        val run = VaultNamespace.forRun("run-1").completed()
        assertFalse(run.isApplication)
        assertTrue(VaultNamespace.Application.isApplication)

        val generation = VaultNamespace.FIRST_GENERATION
        assertNotEquals(
            VaultNamespace.Application.aliasFor(scope, generation),
            run.aliasFor(scope, generation),
        )
        assertTrue(run.aliasPrefix, run.aliasPrefix.startsWith(VaultNamespace.ALIAS_ROOT))
        assertFalse(
            run.aliasPrefix,
            VaultNamespace.Application.aliasPrefix.startsWith(run.aliasPrefix),
        )

        // The longest run this name space accepts, at its longest generation.
        val longest =
            VaultNamespace.forRun("r".repeat(VaultNamespace.MAX_RUN_LENGTH)).completed()
        val alias = longest.aliasFor(scope, VaultNamespace.LAST_GENERATION)
        assertTrue(alias, alias.length <= VaultNamespace.MAX_ALIAS_LENGTH)

        val root = File("/tmp/private")
        assertEquals(root, VaultNamespace.Application.rootIn(root))
        assertEquals(File(root, "test-run-1"), run.rootIn(root))
    }

    @Test
    fun aRunIdentifierIsRefusedUnlessItIsBothAnAliasAndADirectoryName() {
        for (refused in listOf("", "Run-1", "run_1", "run/1", "run 1", "r".repeat(25))) {
            assertTrue(
                "\"$refused\" was accepted as a run id",
                VaultNamespace.forRun(refused).refusal() is VaultFailure.RefusedInput,
            )
        }
        assertEquals("test-run-2", VaultNamespace.forRun("run-2").completed().label)
    }

    private companion object {

        const val DIGEST_HEX_LENGTH = 64L

        /** The first generation, one past it and the last one an alias names. */
        val GENERATIONS =
            listOf(
                VaultNamespace.FIRST_GENERATION,
                VaultNamespace.FIRST_GENERATION + 1,
                VaultNamespace.LAST_GENERATION,
            )
    }
}
