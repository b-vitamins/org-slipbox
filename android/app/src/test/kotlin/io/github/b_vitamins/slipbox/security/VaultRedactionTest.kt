/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.security.Key

class VaultRedactionTest {

    private val vaults = OpenVaults()

    @After
    fun closeTheVaults() {
        vaults.closeAll()
    }

    @Test
    fun everyFailureSaysWhatWentWrongAndQuotesNothing() {
        for (failure in everyFailure()) {
            val printed = failure.toString()
            val name = failure.javaClass.simpleName
            assertTrue(name, printed.isNotEmpty())
            assertEquals(name, failure.summary, printed.substringBefore(";"))
            assertFalse(name, printed.contains("null"))
            assertFalse(name, printed.contains(MARKER))
        }
    }

    @Test
    fun everyKindOfFailureIsAccountedForHere() {
        val declared = VaultFailure::class.java.declaredClasses.map { it.simpleName }.toSet()
        val covered = everyFailure().map { it.javaClass.simpleName }.toSet()

        assertTrue(declared.toString(), declared.size >= EXPECTED_KINDS)
        assertEquals(declared, covered)
    }

    @Test
    fun aFailureFollowedByAFailedCleanupSaysBothAndDefersToTheFirst() {
        val primary = VaultFailure.WriteFailed(CommitPhase.BeforeCommit, ORIGIN)
        val cleanup = VaultFailure.CleanupFailed(ORIGIN)

        val carried = primary.after(cleanup)

        assertEquals(VaultFailure.CleanupAfter(primary, cleanup), carried)
        assertEquals(primary.summary, carried.summary)
        assertEquals(cleanup, carried.cleanup)
        assertEquals(primary.reauthorize, carried.reauthorize)
        assertTrue(carried.toString(), carried.toString().contains(cleanup.summary))
        assertEquals("a failure with nothing to clean up was wrapped", primary, primary.after(null))
    }

    @Test
    fun onlyTheFailuresARecoveryAnswersAskForOne() {
        val reauthorizing =
            everyFailure().filter { it.reauthorize }.map { it.javaClass.simpleName }.toSet()

        assertEquals(
            setOf(
                VaultFailure.CorruptRecord::class.java.simpleName,
                VaultFailure.AuthenticationFailed::class.java.simpleName,
                VaultFailure.KeyMissing::class.java.simpleName,
                VaultFailure.KeyInvalidated::class.java.simpleName,
                VaultFailure.CleanupAfter::class.java.simpleName,
            ),
            reauthorizing,
        )
    }

    @Test
    fun noFailureCarriesAPlatformErrorACredentialOrAKey() {
        val kinds = VaultFailure::class.java.declaredClasses
        assertTrue(kinds.size.toString(), kinds.isNotEmpty())
        for (kind in kinds) {
            for (field in kind.declaredFields) {
                val named = "${kind.simpleName}.${field.name}"
                assertFalse(named, Throwable::class.java.isAssignableFrom(field.type))
                assertFalse(named, StoredCredential::class.java.isAssignableFrom(field.type))
                assertFalse(named, ByteArray::class.java.isAssignableFrom(field.type))
                assertFalse(named, Key::class.java.isAssignableFrom(field.type))
            }
        }
    }

    @Test
    fun aRefusalNeverRepeatsTheValueItRefused() {
        val token = syntheticToken("access")
        val overlong = token.padEnd(StoredCredential.MAX_TOKEN_LENGTH + 1, 'x')

        val tooLong = encode(StoredCredential(overlong)).refusal()
        assertEquals(VaultFailure.RefusedInput("access token", InputDefect.TooLong), tooLong)
        assertFalse(tooLong.toString().contains(MARKER))
        assertFalse(tooLong.toString().contains("xxx"))

        val notText = encode(StoredCredential("$token secret")).refusal()
        assertEquals(VaultFailure.RefusedInput("access token", InputDefect.NotPlainText), notText)
        assertFalse(notText.toString().contains(MARKER))

        val policy = PrivateStoragePolicy(File("/tmp/slipbox-private"), VaultNamespace.Application)
        val traversing =
            policy.fileFor(PrivateStore.Assets, testScope(), "$token/../elsewhere").refusal()
        assertFalse(traversing.toString().contains(MARKER))
        assertEquals(
            VaultFailure.RefusedInput("file name", InputDefect.Traversing),
            traversing,
        )
    }

    @Test
    fun aCredentialPrintsAsARedactionOfItself() {
        val whole =
            StoredCredential(syntheticToken("access"), syntheticToken("refresh"), 1_700_000L)
        val printed = whole.toString()
        assertFalse(printed, printed.contains(MARKER))
        assertTrue(printed, printed.contains("redacted"))
        assertTrue(printed, printed.contains("1700000"))

        val bare = StoredCredential(syntheticToken("access")).toString()
        assertFalse(bare, bare.contains(MARKER))
        assertTrue(bare, bare.contains("absent"))
        assertTrue(bare, bare.contains("unstated"))
    }

    @Test
    fun nothingTheVaultPrintsCarriesWhatItHolds() {
        val keys = FakeVaultKeys()
        val file = FakeRecordFile()
        val scope = testScope()
        val vault = vaults.open(scope, keys, file).completed()
        val access = syntheticToken("access")
        val refresh = syntheticToken("refresh")
        vault.replace(StoredCredential(access, refresh)).completed()

        val printed =
            keys.aliases + listOf(vault.toString(), scope.toString(), vault.read().toString())
        for (each in printed) {
            assertFalse(each, each.contains(MARKER))
            assertFalse(each, each.contains(access))
            assertFalse(each, each.contains(refresh))
        }
        val stored = String(requireNotNull(file.record), Charsets.ISO_8859_1)
        assertFalse("the record carries a marker", stored.contains(MARKER))
        assertFalse("the record carries the token", stored.contains(access))
    }

    /** The body of [credential], on the buffers the production caller hands over. */
    private fun encode(credential: StoredCredential): VaultOutcome<ByteArray> =
        VaultRecordBody.encode(credential, VaultBuffers.Direct)

    private fun everyFailure(): List<VaultFailure> {
        val failures = mutableListOf<VaultFailure>()
        for (defect in InputDefect.values()) {
            failures.add(VaultFailure.RefusedInput("access token", defect))
        }
        for (defect in RecordDefect.values()) {
            failures.add(VaultFailure.CorruptRecord(defect))
        }
        for (stage in KeyStage.values()) {
            failures.add(VaultFailure.KeyUnavailable(stage, ORIGIN))
        }
        for (stage in CipherStage.values()) {
            failures.add(VaultFailure.CipherUnavailable(stage, ORIGIN))
        }
        for (phase in CommitPhase.values()) {
            failures.add(VaultFailure.WriteFailed(phase, ORIGIN))
        }
        for (stage in RemovalStage.values()) {
            failures.add(VaultFailure.RemovalFailed(stage, ORIGIN))
            failures.add(VaultFailure.RemovalFailed(stage, ORIGIN, removed = 2))
        }
        for (bound in VaultBound.values()) {
            failures.add(VaultFailure.VaultAtCapacity(bound))
        }
        failures.add(
            VaultFailure.CleanupAfter(
                VaultFailure.CorruptRecord(RecordDefect.Truncated),
                VaultFailure.CleanupFailed(ORIGIN),
            ),
        )
        failures.add(VaultFailure.AuthenticationFailed)
        failures.add(VaultFailure.KeyMissing)
        failures.add(VaultFailure.KeyInvalidated)
        failures.add(VaultFailure.ReadFailed(ORIGIN))
        failures.add(VaultFailure.CleanupFailed(ORIGIN))
        failures.add(VaultFailure.VaultClosed)
        failures.add(VaultFailure.VaultHeld)
        failures.add(VaultFailure.ForegroundRefused)
        failures.add(VaultFailure.WorkFailed(ORIGIN))
        return failures
    }

    private companion object {

        /** Every synthetic token a test makes begins with this. */
        const val MARKER = "synthetic-"

        const val ORIGIN = "java.io.IOException"

        /** The failure kinds this package declares, as a floor against silent loss. */
        const val EXPECTED_KINDS = 17
    }
}
