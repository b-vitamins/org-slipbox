/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.ProviderException

/**
 * What the keystore's own faults mean to a caller.
 *
 * The provider here holds real AES keys and raises the faults a platform provider
 * raises, so [AndroidKeystoreVaultKeys] classifies them by the same code that runs
 * on a device.
 */
class KeystoreRecoveryTest {

    private val namespace = VaultNamespace.Application

    private val provider = FakeKeystoreProvider()

    private val keys = AndroidKeystoreVaultKeys(namespace, provider)

    private val scope = testScope()

    private val prefix = namespace.aliasPrefixFor(scope)

    private val alias = namespace.aliasFor(scope, VaultNamespace.FIRST_GENERATION)

    @Test
    fun aScopeWithNoKeyIsMissingAKeyRatherThanUnavailable() {
        assertEquals(VaultFailure.KeyMissing, keys.existing(alias).refusal())
    }

    @Test
    fun anInvalidatedKeyIsReportedAsInvalidatedWhenItIsRetrieved() {
        keys.provision(alias).completed()
        provider.invalidate(alias)

        assertEquals(VaultFailure.KeyInvalidated, keys.existing(alias).refusal())
    }

    @Test
    fun anInvalidatedKeyIsNotQuietlyReplacedByProvisioning() {
        keys.provision(alias).completed()
        provider.invalidate(alias)

        assertEquals(VaultFailure.KeyInvalidated, keys.provision(alias).refusal())
        assertEquals(setOf(alias), provider.aliases)
    }

    @Test
    fun aFaultOnlyItsTypeIsKnownFromStaysUnknown() {
        keys.provision(alias).completed()
        provider.readFault = { GeneralSecurityException("synthetic provider fault") }

        assertEquals(
            VaultFailure.KeyUnavailable(KeyStage.Read, GeneralSecurityException::class.java.name),
            keys.existing(alias).refusal(),
        )
    }

    @Test
    fun aKeystoreThatWillNotOpenFailsAtOpening() {
        provider.openFault = { IOException("synthetic keystore fault") }

        assertEquals(
            VaultFailure.KeyUnavailable(KeyStage.Load, IOException::class.java.name),
            keys.existing(alias).refusal(),
        )
    }

    @Test
    fun aKeyThatCannotBeCreatedFailsAtCreation() {
        provider.generateFault = { GeneralSecurityException("synthetic provider fault") }

        val failure = keys.provision(alias).refusal()

        assertTrue(failure.toString(), failure is VaultFailure.KeyUnavailable)
        assertEquals(KeyStage.Create, (failure as VaultFailure.KeyUnavailable).stage)
        assertTrue(provider.aliases.isEmpty())
    }

    @Test
    fun anAliasOfAnotherNameSpaceIsRefusedBeforeTheProviderIsOpened() {
        val elsewhere = VaultNamespace.forRun("run-1").completed()
        val foreign = elsewhere.aliasFor(scope, VaultNamespace.FIRST_GENERATION)

        val refused =
            listOf(
                keys.existing(foreign),
                keys.delete(foreign),
                keys.deleteAll(elsewhere.aliasPrefixFor(scope)),
            )
        for (outcome in refused) {
            assertEquals(
                VaultFailure.RefusedInput("key alias", InputDefect.Foreign),
                outcome.refusal(),
            )
        }
    }

    @Test
    fun aScopesSupersededKeysGoAndTheOneToKeepAndOtherScopesStay() {
        val superseded = namespace.aliasFor(scope, VaultNamespace.FIRST_GENERATION)
        val current = namespace.aliasFor(scope, VaultNamespace.FIRST_GENERATION + 1)
        val elsewhere =
            namespace.aliasFor(
                testScope(credentialRef = "credential-2"),
                VaultNamespace.FIRST_GENERATION,
            )
        for (each in listOf(superseded, current, elsewhere)) {
            keys.provision(each).completed()
        }

        assertEquals(1L, keys.deleteAll(prefix, keep = current).completed().toLong())
        assertEquals(setOf(current, elsewhere), provider.aliases)
    }

    @Test
    fun aScopeWithNoSupersededKeyHasNothingToDelete() {
        keys.provision(alias).completed()

        assertEquals(0L, keys.deleteAll(prefix, keep = alias).completed().toLong())
        assertEquals(setOf(alias), provider.aliases)
    }

    @Test
    fun aKeyTheProviderKeepsIsReportedWithWhatWasAlreadyDeleted() {
        val second = namespace.aliasFor(scope, VaultNamespace.FIRST_GENERATION + 1)
        keys.provision(alias).completed()
        keys.provision(second).completed()
        provider.kept.add(second)

        val failure = keys.deleteAll(prefix).refusal()

        assertTrue(failure.toString(), failure is VaultFailure.RemovalFailed)
        val refused = failure as VaultFailure.RemovalFailed
        assertEquals(RemovalStage.Key, refused.stage)
        assertEquals(1L, refused.removed.toLong())
        assertEquals(setOf(second), provider.aliases)
    }

    @Test
    fun aFaultTheProviderRaisesUncheckedIsReportedRatherThanThrown() {
        keys.provision(alias).completed()
        val unchecked = ProviderException::class.java.name

        provider.deleteFault = { ProviderException(QUOTED_INPUT) }
        val removal = keys.delete(alias).refusal()
        assertEquals(VaultFailure.RemovalFailed(RemovalStage.Key, unchecked), removal)
        val swept = keys.deleteAll(prefix).refusal()
        assertEquals(VaultFailure.RemovalFailed(RemovalStage.Key, unchecked), swept)

        provider.deleteFault = null
        provider.readFault = { ProviderException(QUOTED_INPUT) }
        assertEquals(
            VaultFailure.KeyUnavailable(KeyStage.Read, unchecked),
            keys.existing(alias).refusal(),
        )

        provider.readFault = null
        provider.openFault = { ProviderException(QUOTED_INPUT) }
        assertEquals(
            VaultFailure.KeyUnavailable(KeyStage.Load, unchecked),
            keys.deleteAll(prefix).refusal(),
        )
        assertFalse(removal.toString(), removal.toString().contains(QUOTED_INPUT))
    }

    @Test
    fun theApplicationsOwnKeysAreNeverEnumeratedForACleanup() {
        assertEquals(
            VaultFailure.RefusedInput("key name space", InputDefect.Foreign),
            keys.releaseNamespace().refusal(),
        )
    }

    @Test
    fun aTestNameSpaceReleasesOnlyItsOwnKeys() {
        val run = VaultNamespace.forRun("run-1").completed()
        val runKeys = AndroidKeystoreVaultKeys(run, provider)
        keys.provision(alias).completed()
        runKeys.provision(run.aliasFor(scope, VaultNamespace.FIRST_GENERATION)).completed()

        assertEquals(1L, runKeys.releaseNamespace().completed().toLong())
        assertEquals(setOf(alias), provider.aliases)
    }

    @Test
    fun deletingWhatWasNeverThereDeletesNothing() {
        assertEquals(false, keys.delete(alias).completed())
    }
}
