/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.security.ProviderException

class VaultRecoveryTest {

    private val provider = FakeKeystoreProvider()

    private val keys = AndroidKeystoreVaultKeys(NAMESPACE, provider)

    private val cipher = AesGcmVaultCipher(keys)

    private val file = FakeRecordFile()

    private val scope = testScope()

    private val vaults = OpenVaults()

    private val vault = vaults.open(scope, keys, file, cipher = cipher).completed()

    @After
    fun closeTheVaults() {
        vaults.closeAll()
    }

    @Test
    fun anInvalidatedKeyIsRefusedUntilARecoveryStoresUnderTheNextGeneration() {
        val first = StoredCredential(syntheticToken("first"))
        vault.replace(first).completed()
        val stored = requireNotNull(file.record).copyOf()
        provider.invalidate(aliasOf(FIRST))

        val refusal = vault.read().refusal()
        assertEquals(VaultFailure.KeyInvalidated, refusal)
        assertTrue("no recovery was offered", refusal.reauthorize)
        assertEquals(VaultFailure.KeyInvalidated, vault.replace(first).refusal())
        assertArrayEquals("a refused replacement wrote", stored, file.record)

        val second = StoredCredential(syntheticToken("second"), syntheticToken("refresh"), 1L)
        val recovery = vault.reauthorize(second).completed()

        assertEquals(VaultReauthorization(FIRST + 1, 1, null), recovery)
        assertEquals(second, vault.read().completed())
        assertEquals(setOf(aliasOf(FIRST + 1)), provider.aliases)
        assertEquals((FIRST + 1).toLong(), storedGeneration().toLong())
        assertFalse(
            "the record was left as it was",
            requireNotNull(file.record).contentEquals(stored),
        )
    }

    @Test
    fun aKeyTheProviderNoLongerHoldsIsRecoveredWithoutDeletingAnythingFirst() {
        vault.replace(StoredCredential(syntheticToken("first"))).completed()
        provider.forget(aliasOf(FIRST))

        assertEquals(VaultFailure.KeyMissing, vault.read().refusal())

        val second = StoredCredential(syntheticToken("second"))
        val recovery = vault.reauthorize(second).completed()

        assertEquals(VaultReauthorization(FIRST + 1, 0, null), recovery)
        assertEquals(second, vault.read().completed())
        assertEquals(setOf(aliasOf(FIRST + 1)), provider.aliases)
    }

    @Test
    fun aRecoveryOfAScopeWithNoRecordAuthorizesItsFirstKeyAlone() {
        val credential = StoredCredential(syntheticToken("first"))

        val recovery = vault.reauthorize(credential).completed()

        assertEquals(VaultReauthorization(FIRST, 0, null), recovery)
        assertEquals(credential, vault.read().completed())
        assertEquals(setOf(aliasOf(FIRST)), provider.aliases)
        assertEquals(FIRST.toLong(), storedGeneration().toLong())
    }

    @Test
    fun aRecordThatNoLongerParsesIsRecoveredOverTheGenerationItStillNames() {
        vault.replace(StoredCredential(syntheticToken("first"))).completed()
        file.record = requireNotNull(file.record).copyOf(DECLARATION_LENGTH)

        val refusal = vault.read().refusal()
        assertEquals(VaultFailure.CorruptRecord(RecordDefect.Truncated), refusal)
        assertTrue("no recovery was offered", refusal.reauthorize)

        val second = StoredCredential(syntheticToken("second"))
        val recovery = vault.reauthorize(second).completed()

        assertEquals(VaultReauthorization(FIRST + 1, 1, null), recovery)
        assertEquals(second, vault.read().completed())
        assertEquals(setOf(aliasOf(FIRST + 1)), provider.aliases)
    }

    @Test
    fun aRecoveryThatFailsBeforeItCommitsKeepsThePreviousRecordAndItsKey() {
        val first = StoredCredential(syntheticToken("first"))
        vault.replace(first).completed()
        val stored = requireNotNull(file.record).copyOf()
        val refusal = VaultFailure.WriteFailed(CommitPhase.BeforeCommit, ORIGIN)
        file.writeFailure = refusal

        val reported = vault.reauthorize(StoredCredential(syntheticToken("second"))).refusal()

        assertEquals(refusal, reported)
        assertArrayEquals("the previous record was written over", stored, file.record)
        assertEquals(
            "the key the recovery created outlived it",
            setOf(aliasOf(FIRST)),
            provider.aliases,
        )

        file.writeFailure = null
        assertEquals(first, vault.read().completed())
    }

    @Test
    fun aRecoveryWhoseSupersededKeyStaysSaysSoAndStillStoresTheCredential() {
        vault.replace(StoredCredential(syntheticToken("first"))).completed()
        provider.kept.add(aliasOf(FIRST))

        val second = StoredCredential(syntheticToken("second"))
        val recovery = vault.reauthorize(second).completed()

        assertEquals((FIRST + 1).toLong(), recovery.generation.toLong())
        assertEquals(0L, recovery.supersededKeys.toLong())
        val cleanup = requireNotNull(recovery.cleanup)
        assertTrue(cleanup.toString(), cleanup is VaultFailure.RemovalFailed)
        assertEquals(RemovalStage.Key, (cleanup as VaultFailure.RemovalFailed).stage)
        assertEquals(second, vault.read().completed())
        assertEquals(setOf(aliasOf(FIRST), aliasOf(FIRST + 1)), provider.aliases)
    }

    @Test
    fun aRecoveryWhoseSupersededKeyFaultsStillStoresTheCredentialAndSaysSo() {
        vault.replace(StoredCredential(syntheticToken("first"))).completed()
        provider.deleteFault = { ProviderException(QUOTED_INPUT) }

        val second = StoredCredential(syntheticToken("second"))
        val recovery = vault.reauthorize(second).completed()

        assertEquals((FIRST + 1).toLong(), recovery.generation.toLong())
        assertEquals(0L, recovery.supersededKeys.toLong())
        val cleanup = requireNotNull(recovery.cleanup)
        assertEquals(
            VaultFailure.RemovalFailed(RemovalStage.Key, ProviderException::class.java.name),
            cleanup,
        )
        assertFalse(cleanup.toString(), cleanup.toString().contains(QUOTED_INPUT))
        assertEquals(second, vault.read().completed())
        assertEquals(setOf(aliasOf(FIRST), aliasOf(FIRST + 1)), provider.aliases)
    }

    @Test
    fun aRecoveryAtTheLastGenerationTakesAGenerationNoKeyHolds() {
        val leftover = seed(FIRST, StoredCredential(syntheticToken("leftover")))
        val first = StoredCredential(syntheticToken("first"))
        val last = seed(LAST, first)
        assertEquals(first, vault.read().completed())

        val second = StoredCredential(syntheticToken("second"))
        val recovery = vault.reauthorize(second).completed()

        assertEquals(VaultReauthorization(FIRST + 1, 2, null), recovery)
        assertEquals(setOf(aliasOf(FIRST + 1)), provider.aliases)
        assertEquals(second, vault.read().completed())
        assertFalse(
            "the record of the last generation stayed",
            requireNotNull(file.record).contentEquals(last),
        )

        file.record = leftover
        assertEquals(VaultFailure.KeyMissing, vault.read().refusal())
    }

    @Test
    fun aRecoveryOverARecordItCannotParseKeepsTheKeyThatRecordNeeds() {
        val first = StoredCredential(syntheticToken("first"))
        vault.replace(first).completed()
        val stored = requireNotNull(file.record).copyOf()
        val damaged = stored.copyOf()
        damaged[VERSION_BYTE] = UNSUPPORTED_VERSION
        file.record = damaged
        val refusal = VaultFailure.WriteFailed(CommitPhase.BeforeCommit, ORIGIN)
        file.writeFailure = refusal

        val reported = vault.reauthorize(StoredCredential(syntheticToken("second"))).refusal()

        assertEquals(refusal, reported)
        assertArrayEquals("the record it could not parse was written over", damaged, file.record)
        assertEquals(
            "the key of the record it could not parse went",
            setOf(aliasOf(FIRST)),
            provider.aliases,
        )

        file.record = stored
        file.writeFailure = null
        assertEquals(first, vault.read().completed())
    }

    @Test
    fun aRecoveryWhoseNextGenerationIsHeldTakesTheFirstOneThatIsNot() {
        val first = StoredCredential(syntheticToken("first"))
        vault.replace(first).completed()
        val stored = requireNotNull(file.record).copyOf()
        keys.provision(aliasOf(FIRST + 1)).completed()

        val second = StoredCredential(syntheticToken("second"))
        val recovery = vault.reauthorize(second).completed()

        assertEquals(VaultReauthorization(FIRST + 2, 2, null), recovery)
        assertEquals(setOf(aliasOf(FIRST + 2)), provider.aliases)
        assertEquals(second, vault.read().completed())
        assertFalse(
            "the previous record stayed",
            requireNotNull(file.record).contentEquals(stored),
        )
    }

    @Test
    fun aRecoveryWithNoFreeGenerationIsRefusedRatherThanTakingAKeyInUse() {
        val first = StoredCredential(syntheticToken("first"))
        vault.replace(first).completed()
        val stored = requireNotNull(file.record).copyOf()
        for (generation in FIRST..LAST) {
            keys.provision(aliasOf(generation)).completed()
        }

        val reported = vault.reauthorize(StoredCredential(syntheticToken("second"))).refusal()

        assertEquals(VaultFailure.VaultAtCapacity(VaultBound.KeyGenerations), reported)
        assertArrayEquals("the previous record was written over", stored, file.record)
        assertEquals(LAST.toLong(), provider.aliases.size.toLong())
        assertEquals(first, vault.read().completed())
    }

    @Test
    fun aRecoveryThatFailsAfterItCommittedKeepsTheKeyThatOpensWhatItWrote() {
        vault.replace(StoredCredential(syntheticToken("first"))).completed()
        val refusal = VaultFailure.WriteFailed(CommitPhase.Committed, ORIGIN)
        file.writeFailure = refusal

        val reported = vault.reauthorize(StoredCredential(syntheticToken("second"))).refusal()

        assertEquals(refusal, reported)
        assertTrue(
            "the key of a record that may be stored went",
            provider.aliases.contains(aliasOf(FIRST + 1)),
        )
    }

    @Test
    fun aRecoveryOfACredentialNoRecordCanHoldTouchesNoKeyAtAll() {
        val first = StoredCredential(syntheticToken("first"))
        vault.replace(first).completed()
        val stored = requireNotNull(file.record).copyOf()

        val reported = vault.reauthorize(StoredCredential("")).refusal()

        assertEquals(VaultFailure.RefusedInput(ACCESS_TOKEN, InputDefect.Empty), reported)
        assertEquals(setOf(aliasOf(FIRST)), provider.aliases)
        assertArrayEquals("a refused recovery wrote", stored, file.record)
        assertEquals(first, vault.read().completed())
    }

    @Test
    fun aRecoveryThatCannotTellWhetherAGenerationIsFreeIsRefusedAndTakesNothing() {
        vault.replace(StoredCredential(syntheticToken("first"))).completed()
        val stored = requireNotNull(file.record).copyOf()
        provider.readFault = { IllegalStateException(QUOTED_INPUT) }

        val second = StoredCredential(syntheticToken("second"))
        val reported = answered { vault.reauthorize(second) }.refusal()

        assertFalse(reported.toString(), reported.toString().contains(QUOTED_INPUT))
        assertEquals(setOf(aliasOf(FIRST)), provider.aliases)
        assertArrayEquals("a refused recovery wrote", stored, file.record)
    }

    @Test
    fun aRecoveryLeavesEveryOtherScopeAndItsKeysAsTheyWere() {
        val other = testScope(credentialRef = "credential-2")
        val theirFile = FakeRecordFile()
        val theirVault = vaults.open(other, keys, theirFile, cipher = cipher).completed()
        val theirs = StoredCredential(syntheticToken("theirs"))
        theirVault.replace(theirs).completed()
        val theirRecord = requireNotNull(theirFile.record).copyOf()
        vault.replace(StoredCredential(syntheticToken("first"))).completed()
        provider.invalidate(aliasOf(FIRST))

        vault.reauthorize(StoredCredential(syntheticToken("second"))).completed()

        assertArrayEquals("another scope's record was written", theirRecord, theirFile.record)
        assertEquals(theirs, theirVault.read().completed())
        assertTrue(
            "another scope's key went with this one's",
            provider.aliases.contains(NAMESPACE.aliasFor(other, FIRST)),
        )
    }

    /** The alias of this scope's [generation] key, as a vault names it. */
    private fun aliasOf(generation: Int): String = NAMESPACE.aliasFor(scope, generation)

    /** The generation the stored record declares. */
    private fun storedGeneration(): Int =
        requireNotNull(VaultEnvelope.generationIn(requireNotNull(file.record)))

    /** Stores [credential] at [generation] the way a vault of that generation would. */
    private fun seed(generation: Int, credential: StoredCredential): ByteArray {
        val body = VaultRecordBody.encode(credential, VaultBuffers.Direct).completed()
        val sealed =
            cipher
                .seal(
                    aliasOf(generation),
                    VaultEnvelope.aadFor(scope, generation),
                    body,
                    KeySource.Provisioned,
                ).completed()
        val record =
            VaultEnvelope.compose(scope, generation, sealed.nonce, sealed.ciphertext).completed()
        file.record = record
        return record
    }

    private companion object {

        val NAMESPACE = VaultNamespace.Application

        const val FIRST = VaultNamespace.FIRST_GENERATION

        const val LAST = VaultNamespace.LAST_GENERATION

        /** A record cut down to the version and the generation it declares. */
        const val DECLARATION_LENGTH = 2

        /** Where a record declares which layout the rest of it follows. */
        const val VERSION_BYTE = 0

        /** A version this vault does not know, so the generation beside it is not one. */
        val UNSUPPORTED_VERSION: Byte = (VaultEnvelope.VERSION + 1).toByte()

        /** The input a credential with no access token is refused as. */
        const val ACCESS_TOKEN = "access token"

        const val ORIGIN = "java.io.IOException"
    }
}
