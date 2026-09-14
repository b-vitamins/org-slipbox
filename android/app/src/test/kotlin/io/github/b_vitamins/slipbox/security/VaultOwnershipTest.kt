/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

/**
 * Who may hold one record: the replacement protocol needs a single owner of a
 * path, which one thread per vault does not give it.
 */
class VaultOwnershipTest {

    @get:Rule
    val temporary = TemporaryFolder()

    private val vaults = OpenVaults()

    private val keys = FakeVaultKeys()

    private val scope = testScope()

    @After
    fun closeTheVaults() {
        vaults.closeAll()
    }

    @Test
    fun aSecondVaultOverOneRecordIsRefusedRatherThanInterleavedWithTheFirst() {
        val file = FakeRecordFile()
        val first = vaults.open(scope, keys, file).completed()
        first.replace(StoredCredential(syntheticToken("first"))).completed()

        assertEquals(VaultFailure.VaultHeld, vaults.open(scope, keys, file).refusal())
        assertEquals(1L, file.attempts.toLong())
    }

    @Test
    fun aRecordIsGrantedAgainOnceTheVaultThatHeldItHasFinished() {
        val file = FakeRecordFile()
        val credential = StoredCredential(syntheticToken("access"))
        val first = vaults.open(scope, keys, file).completed()
        first.replace(credential).completed()

        first.close()
        assertTrue(
            "the first vault never released its record",
            first.awaitClosed(CLOSE_TIMEOUT_MILLIS),
        )

        val second = vaults.open(scope, keys, file).completed()
        assertEquals(credential, second.read().completed())
    }

    @Test
    fun oneRecordAdmitsOneWriterAtATimeThroughEveryReplacement() {
        val io = FaultyRecordIo()
        val inside = AtomicInteger()
        val overlapped = AtomicBoolean(false)
        val both = CountDownLatch(2)
        io.beforeOpen = {
            if (inside.incrementAndGet() > 1) {
                overlapped.set(true)
            }
            both.countDown()
            both.await(OVERLAP_SECONDS, TimeUnit.SECONDS)
            inside.decrementAndGet()
        }
        val target = File(temporary.newFolder("private"), "credential.bin")
        val vault = vaults.open(scope, keys, source = source(target, io)).completed()

        assertEquals(
            VaultFailure.VaultHeld,
            vaults.open(scope, keys, source = source(target, io)).refusal(),
        )
        val outcomes = replaceFromTwoThreads(vault)

        assertFalse("two writers were inside one replacement", overlapped.get())
        assertEquals(REPLACEMENTS.toLong(), outcomes.count { it is VaultOutcome.Completed }.toLong())
    }

    @Test
    fun aRecordOfAnotherNameSpaceDoesNotOpenUnderThisOne() {
        val elsewhere = VaultNamespace.forRun("run-1").completed()
        val applicationFile = FakeRecordFile()
        val runFile = FakeRecordFile()
        val application = vaults.open(scope, keys, applicationFile).completed()
        val underTest = vaults.open(scope, keys, runFile, namespace = elsewhere).completed()
        application.replace(StoredCredential(syntheticToken("application"))).completed()
        underTest.replace(StoredCredential(syntheticToken("run"))).completed()

        assertEquals("one name space named the other's key", 2L, keys.aliases.size.toLong())
        runFile.record = requireNotNull(applicationFile.record).copyOf()
        assertEquals(VaultFailure.AuthenticationFailed, underTest.read().refusal())
    }

    @Test
    fun aPathThatStopsBeingThisVaultsIsRefusedRatherThanWrittenTo() {
        val file = FakeRecordFile()
        val source = FakeRecordSource(file)
        val vault = vaults.open(scope, keys, source = source).completed()
        vault.replace(StoredCredential(syntheticToken("first"))).completed()

        source.path = source.path + ".elsewhere"
        val escaping = VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping)
        val second = StoredCredential(syntheticToken("second"))

        assertEquals(escaping, vault.read().refusal())
        assertEquals(escaping, vault.replace(second).refusal())
        assertEquals(escaping, vault.reauthorize(second).refusal())
        assertEquals(escaping, vault.remove().refusal())
        assertEquals("a refused path reached the file", 1L, file.attempts.toLong())
        assertEquals("a refused path reached the keys", 1L, keys.aliases.size.toLong())
    }

    @Test
    fun aRecordThatDoesNotResolveIsReportedByEveryOperationAndByOpening() {
        val held = VaultOwners.held
        val failure = VaultFailure.ReadFailed("java.io.IOException")
        val source = FakeRecordSource(FakeRecordFile())
        source.failure = failure

        assertEquals(failure, vaults.open(scope, keys, source = source).refusal())
        assertEquals("a record that did not resolve was held", held.toLong(), VaultOwners.held.toLong())

        source.failure = null
        val vault = vaults.open(scope, keys, source = source).completed()
        source.failure = failure
        assertEquals(failure, vault.read().refusal())
        assertEquals(failure, vault.remove().refusal())
    }

    @Test
    fun theForegroundThreadIsRefusedTheOpenItWouldBlockOn() {
        val held = VaultOwners.held
        val source = FakeRecordSource(FakeRecordFile())

        val refusal =
            vaults
                .open(scope, keys, foreground = FakeForegroundThread(current = true), source = source)
                .refusal()

        assertEquals(VaultFailure.ForegroundRefused, refusal)
        assertEquals("the foreground thread resolved a record", 0L, source.resolutions.toLong())
        assertEquals(held.toLong(), VaultOwners.held.toLong())
    }

    @Test
    fun moreRecordsThanThisProcessHoldsAtOnceIsRefusedRatherThanGranted() {
        repeat(VaultOwners.LIMIT - VaultOwners.held) {
            vaults.open(scope, keys, FakeRecordFile()).completed()
        }
        assertEquals(VaultOwners.LIMIT.toLong(), VaultOwners.held.toLong())

        assertEquals(
            VaultFailure.VaultAtCapacity(VaultBound.OpenRecords),
            vaults.open(scope, keys, FakeRecordFile()).refusal(),
        )
    }

    @Test
    fun everyRecordThisSuiteHeldIsGivenUpWhenItsVaultsFinish() {
        val held = VaultOwners.held
        val vault = vaults.open(scope, keys, FakeRecordFile()).completed()
        assertEquals((held + 1).toLong(), VaultOwners.held.toLong())

        vaults.closeAll()

        assertEquals(held.toLong(), VaultOwners.held.toLong())
        assertEquals(VaultFailure.VaultClosed, vault.read().refusal())
    }

    private fun source(target: File, io: RecordIo): RecordSource =
        FixedRecordSource(target.path, AtomicVaultRecordFile(target, io))

    private fun replaceFromTwoThreads(vault: CredentialVault): List<VaultOutcome<Unit>> {
        val credential = StoredCredential(syntheticToken("owner"))
        val outcomes = mutableListOf<VaultOutcome<Unit>>()
        val started = CountDownLatch(REPLACEMENTS)
        val threads =
            (1..REPLACEMENTS).map {
                Thread {
                    started.countDown()
                    started.await(OVERLAP_SECONDS, TimeUnit.SECONDS)
                    val outcome = vault.replace(credential)
                    synchronized(outcomes) { outcomes.add(outcome) }
                }
            }
        threads.forEach { it.start() }
        threads.forEach { it.join(TimeUnit.SECONDS.toMillis(JOIN_SECONDS)) }
        threads.forEach { assertFalse("a replacement never returned", it.isAlive) }
        return outcomes
    }

    private companion object {

        /** Long enough for a second entry to arrive, short enough to end a test. */
        const val OVERLAP_SECONDS = 2L

        const val JOIN_SECONDS = 20L

        /** Two callers replacing at once, which one vault must not let overlap. */
        const val REPLACEMENTS = 2
    }
}
