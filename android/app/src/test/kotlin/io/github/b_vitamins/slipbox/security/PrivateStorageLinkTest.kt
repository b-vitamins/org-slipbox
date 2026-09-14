/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class PrivateStorageLinkTest {

    @get:Rule
    val temporary = TemporaryFolder()

    private val scope = testScope()

    private val keys = FakeVaultKeys()

    private lateinit var outside: File

    private lateinit var sentinel: File

    @Test
    fun aNameUnderALinkedAncestorIsRefusedAsThePathItResolvedTo() {
        val policy = policy()
        val store = File(policy.root, PrivateStore.Assets.directoryName)
        assertTrue(policy.root.mkdirs())
        link(store, outside)

        val failure = policy.fileFor(PrivateStore.Assets, scope, "asset.png").refusal()

        assertEquals(ESCAPING, failure)
        assertSentinelKept()
    }

    @Test
    fun aRecordUnderALinkedAncestorIsRefusedBeforeAnyWrite() {
        val policy = policy()
        val store = File(policy.root, PrivateStore.Vault.directoryName)
        assertTrue(policy.root.mkdirs())
        link(store, outside)

        assertEquals(ESCAPING, record(policy).refusal())
        assertEquals(ESCAPING, policy.recordFor(scope).refusal())
        assertEquals(ESCAPING, policy.directoryFor(PrivateStore.Vault, scope).refusal())
        assertSentinelKept()
    }

    @Test
    fun aVaultWhoseRecordBecomesALinkIsRefusedRatherThanWritingThroughIt() {
        val policy = policy()
        val vaults = OpenVaults()
        try {
            val vault =
                vaults.open(scope, keys, source = PolicyRecordSource(policy, scope)).completed()
            vault.replace(StoredCredential(syntheticToken("access"))).completed()
            val directory = directory(policy, PrivateStore.Vault)
            assertTrue(File(directory, PrivateStoragePolicy.RECORD_NAME).delete())
            assertTrue(directory.delete())
            link(directory, outside)

            val refusal = vault.replace(StoredCredential(syntheticToken("second"))).refusal()

            assertEquals(ESCAPING, refusal)
            assertEquals(ESCAPING, vault.read().refusal())
            assertSentinelKept()
            assertFalse(
                "a record was written through the link",
                File(outside, PrivateStoragePolicy.RECORD_NAME).exists(),
            )
        } finally {
            vaults.closeAll()
        }
    }

    @Test
    fun aVaultWhoseStagingRecordIsALinkRefusesRatherThanWritingThroughIt() {
        val policy = policy()
        val vaults = OpenVaults()
        try {
            val vault =
                vaults.open(scope, keys, source = PolicyRecordSource(policy, scope)).completed()
            vault.replace(StoredCredential(syntheticToken("access"))).completed()
            val record = record(policy).completed()
            val stored = record.readBytes()
            link(staging(record), sentinel)

            val refusal = vault.replace(StoredCredential(syntheticToken("second"))).refusal()

            assertEquals(ESCAPING, refusal)
            assertSentinelKept()
            assertArrayEquals("the stored record was replaced", stored, record.readBytes())
        } finally {
            vaults.closeAll()
        }
    }

    @Test
    fun aStagingLinkOntoAnotherScopesRecordLeavesThatRecordAlone() {
        val policy = policy()
        val other = testScope(credentialRef = "credential-2")
        val vaults = OpenVaults()
        try {
            val mine =
                vaults.open(scope, keys, source = PolicyRecordSource(policy, scope)).completed()
            val theirs =
                vaults.open(other, keys, source = PolicyRecordSource(policy, other)).completed()
            val credential = StoredCredential(syntheticToken("theirs"))
            mine.replace(StoredCredential(syntheticToken("mine"))).completed()
            theirs.replace(credential).completed()
            val neighbour =
                policy.fileFor(PrivateStore.Vault, other, PrivateStoragePolicy.RECORD_NAME)
                    .completed()
            val kept = neighbour.readBytes()
            link(staging(record(policy).completed()), neighbour)

            val refusal = mine.replace(StoredCredential(syntheticToken("second"))).refusal()

            assertEquals(ESCAPING, refusal)
            assertArrayEquals("another scope's record was written", kept, neighbour.readBytes())
            assertEquals(credential, theirs.read().completed())
        } finally {
            vaults.closeAll()
        }
    }

    @Test
    fun aStagingRecordThatIsADeadLinkCreatesNothingWhereItPointed() {
        val policy = policy()
        val vaults = OpenVaults()
        val never = File(outside, "never-there")
        try {
            val vault =
                vaults.open(scope, keys, source = PolicyRecordSource(policy, scope)).completed()
            vault.replace(StoredCredential(syntheticToken("access"))).completed()
            val record = record(policy).completed()
            val stored = record.readBytes()
            link(staging(record), never)

            val refusal = vault.replace(StoredCredential(syntheticToken("second"))).refusal()

            assertEquals(ESCAPING, refusal)
            assertFalse("a record was created outside the root", never.exists())
            assertArrayEquals("the stored record was replaced", stored, record.readBytes())
        } finally {
            vaults.closeAll()
        }
    }

    @Test
    fun aVaultKeepsItsKeysWhenTheFileSystemWillNotSayWhetherAWriteCommitted() {
        val io = FaultyRecordIo()
        val policy = policy(io)
        val vaults = OpenVaults()
        try {
            val vault =
                vaults.open(scope, keys, source = PolicyRecordSource(policy, scope, io)).completed()
            vault.replace(StoredCredential(syntheticToken("access"))).completed()
            val held = keys.aliases
            io.renameFault = { SecurityException(QUOTED_INPUT) }

            val second = StoredCredential(syntheticToken("second"))
            val failure = answered { vault.reauthorize(second) }.refusal()

            assertTrue(failure.toString(), failure is VaultFailure.WriteFailed)
            assertEquals(CommitPhase.Unknown, (failure as VaultFailure.WriteFailed).phase)
            assertFalse(failure.toString(), failure.toString().contains(QUOTED_INPUT))
            assertTrue("a key this scope may still need went", keys.aliases.containsAll(held))
        } finally {
            vaults.closeAll()
        }
    }

    @Test
    fun aCleanupDoesNotFollowALinkItStartsAt() {
        val policy = policy()
        val vault = directory(policy, PrivateStore.Vault)
        assertTrue(requireNotNull(vault.parentFile).mkdirs())
        link(vault, outside)

        val removed = policy.removeCredential(scope)

        assertSentinelKept()
        assertTrue(outside.isDirectory)
        assertFalse("the link was left behind", listed(requireNotNull(vault.parentFile), vault.name))
        assertEquals(1L, removed.completed().toLong())
    }

    @Test
    fun aCleanupUnlinksADeadLinkRatherThanCallingItAbsent() {
        val policy = policy()
        val vault = directory(policy, PrivateStore.Vault)
        val parent = requireNotNull(vault.parentFile)
        assertTrue(parent.mkdirs())
        link(vault, File(outside, "never-there"))

        val removed = policy.removeCredential(scope).completed()

        assertFalse("the dead link was left behind", listed(parent, vault.name))
        assertEquals(1L, removed.toLong())
    }

    @Test
    fun aCleanupThatCannotListADirectoryReportsThatRatherThanEmptiness() {
        val io = FaultyRecordIo()
        val policy = policy(io)
        val vault = directory(policy, PrivateStore.Vault)
        seed(File(vault, PrivateStoragePolicy.RECORD_NAME))
        io.unlistable.add(vault.path)

        val failure = policy.removeCredential(scope).refusal()

        assertTrue(failure.toString(), failure is VaultFailure.RemovalFailed)
        val refused = failure as VaultFailure.RemovalFailed
        assertEquals(RemovalStage.Directory, refused.stage)
        assertTrue(refused.origin, refused.origin.contains("list"))
    }

    @Test
    fun aCleanupUnlinksALinkedChildAndLeavesWhatItPointsAt() {
        val policy = policy()
        val vault = directory(policy, PrivateStore.Vault)
        seed(File(vault, PrivateStoragePolicy.RECORD_NAME))
        link(File(vault, "elsewhere"), outside)

        val removed = policy.removeCredential(scope).completed()

        assertSentinelKept()
        assertTrue(outside.isDirectory)
        assertFalse(vault.exists())
        assertEquals(LINKED_CHILD_ENTRIES.toLong(), removed.toLong())
    }

    @Test
    fun aCleanupLeavesADirectoryWhoseNameOnlyBeginsLikeThisScopes() {
        val policy = policy()
        val vault = directory(policy, PrivateStore.Vault)
        val sibling = File(requireNotNull(vault.parentFile), vault.name + "-other")
        seed(File(vault, PrivateStoragePolicy.RECORD_NAME))
        seed(File(sibling, PrivateStoragePolicy.RECORD_NAME))

        policy.removeCredential(scope).completed()

        assertFalse(vault.exists())
        assertTrue(File(sibling, PrivateStoragePolicy.RECORD_NAME).isFile)
    }

    private fun policy(io: RecordIo = SystemRecordIo): PrivateStoragePolicy {
        outside = temporary.newFolder("outside")
        sentinel = File(outside, "sentinel.bin")
        sentinel.writeBytes(SENTINEL)
        val root = File(temporary.newFolder("private"), "root")
        return PrivateStoragePolicy(root, VaultNamespace.Application, io)
    }

    private fun directory(policy: PrivateStoragePolicy, store: PrivateStore): File =
        policy.directoryFor(store, scope).completed()

    private fun record(policy: PrivateStoragePolicy): VaultOutcome<File> =
        policy.fileFor(PrivateStore.Vault, scope, PrivateStoragePolicy.RECORD_NAME)

    /** The name [record] is written under before it is renamed into place. */
    private fun staging(record: File): File = File(record.path + STAGING_SUFFIX)

    private fun seed(file: File) {
        val parent = requireNotNull(file.parentFile)
        assertTrue(parent.path, parent.isDirectory || parent.mkdirs())
        file.writeText("synthetic private content")
    }

    private fun listed(directory: File, name: String): Boolean =
        directory.list()?.contains(name) == true

    private fun assertSentinelKept() {
        assertTrue("the sentinel outside the root went", sentinel.isFile)
        assertArrayEquals(SENTINEL, sentinel.readBytes())
    }

    private companion object {

        /** What a path leaving the private root is refused as, whoever asked. */
        val ESCAPING = VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping)

        /** Bytes of a file no vault owns, in a directory no vault owns. */
        val SENTINEL = ByteArray(16) { 9 }

        const val STAGING_SUFFIX = ".new"

        /** The record, the link and the store directory itself. */
        const val LINKED_CHILD_ENTRIES = 3
    }
}
