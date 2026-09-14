/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/** Where private data may live, and what a cleanup is allowed to delete. */
class PrivateStoragePolicyTest {

    @get:Rule
    val temporary = TemporaryFolder()

    private val scope = testScope()

    @Test
    fun everyStoreLivesBeneathTheRootUnderADigestedNameOfItsOwn() {
        val policy = policy()
        val root = policy.root
        val directories = mutableSetOf<File>()

        for (store in PrivateStore.values()) {
            val directory = directoryFor(policy, store)
            directories.add(directory)
            assertTrue(
                directory.path,
                directory.path.startsWith(root.path + File.separator),
            )
            for (identity in IDENTITIES) {
                assertFalse("$store names $identity", directory.path.contains(identity))
            }
        }
        assertEquals(PrivateStore.values().size.toLong(), directories.size.toLong())
    }

    @Test
    fun aTestRunKeepsItsPrivateFilesApartFromTheApplications() {
        val root = temporary.newFolder("private")
        val run = VaultNamespace.forRun("run-1").completed()

        assertEquals(root, PrivateStoragePolicy(root, VaultNamespace.Application).root)
        val runRoot = File(root, "test-run-1")
        assertEquals(runRoot, PrivateStoragePolicy(root, run).root)
        val directory =
            PrivateStoragePolicy(root, run).directoryFor(PrivateStore.Vault, scope).completed()
        assertTrue(directory.path, directory.path.startsWith(runRoot.path + File.separator))
    }

    @Test
    fun aCredentialStoreMovesWithItsCredentialAndAnAccountStoreDoesNot() {
        val policy = policy()
        val replaced = testScope(credentialRef = "credential-2")
        val elsewhere = testScope(accountId = "account-2")

        assertNotEquals(
            directoryFor(policy, PrivateStore.Vault),
            directoryFor(policy, PrivateStore.Vault, replaced),
        )
        for (store in PrivateStore.values().filter { !it.perCredential }) {
            assertEquals(
                store.name,
                directoryFor(policy, store),
                directoryFor(policy, store, replaced),
            )
            assertNotEquals(
                store.name,
                directoryFor(policy, store),
                directoryFor(policy, store, elsewhere),
            )
        }
    }

    @Test
    fun theRecordOfAScopeIsOneNamedFileInTheVaultStore() {
        val policy = policy()
        val record = policy.recordFor(scope).completed()

        assertEquals(PrivateStoragePolicy.RECORD_NAME, record.name)
        assertEquals(directoryFor(policy, PrivateStore.Vault), record.parentFile)
    }

    @Test
    fun aNameThatCouldLeaveItsDirectoryIsRefused() {
        val policy = policy()
        val refused =
            listOf(
                "" to InputDefect.Empty,
                "n".repeat(PrivateStoragePolicy.MAX_NAME_LENGTH + 1) to InputDefect.TooLong,
                "/etc/passwd" to InputDefect.Absolute,
                "../outside" to InputDefect.Traversing,
                "sub/name" to InputDefect.Traversing,
                "sub\\name" to InputDefect.Traversing,
                "." to InputDefect.Traversing,
                ".." to InputDefect.Traversing,
                "index..db" to InputDefect.Traversing,
                "Upper" to InputDefect.NotPlainText,
                ".hidden" to InputDefect.NotPlainText,
                "-lead" to InputDefect.NotPlainText,
                "na me" to InputDefect.NotPlainText,
                "na\tme" to InputDefect.NotPlainText,
            )

        for ((name, defect) in refused) {
            assertEquals(
                "\"$name\" was accepted",
                VaultFailure.RefusedInput("file name", defect),
                policy.fileFor(PrivateStore.Assets, scope, name).refusal(),
            )
        }
    }

    @Test
    fun anOrdinaryNameResolvesInsideItsOwnStore() {
        val policy = policy()
        val directory = directoryFor(policy, PrivateStore.Assets)

        for (name in listOf("a", "0", "credential.bin", "index-0.db", "note_1.org")) {
            assertEquals(
                File(directory, name),
                policy.fileFor(PrivateStore.Assets, scope, name).completed(),
            )
        }
    }

    @Test
    fun removingACredentialTakesItsOwnStoresAndLeavesTheAccountsAlone() {
        val policy = policy()
        val vault = directoryFor(policy, PrivateStore.Vault)
        val index = directoryFor(policy, PrivateStore.Index)
        seed(policy.recordFor(scope).completed())
        seed(File(File(vault, "replaced"), PrivateStoragePolicy.RECORD_NAME))
        seed(File(index, "index.db"))

        val removed = policy.removeCredential(scope).completed()

        assertFalse(vault.exists())
        assertTrue(index.isDirectory)
        assertEquals(REMOVED_ENTRIES.toLong(), removed.toLong())
    }

    @Test
    fun removingAnAccountTakesTheAccountStoresAndLeavesTheCredential() {
        val policy = policy()
        val vault = directoryFor(policy, PrivateStore.Vault)
        seed(policy.recordFor(scope).completed())
        for (store in PrivateStore.values().filter { !it.perCredential }) {
            seed(File(directoryFor(policy, store), "content"))
        }

        val removed = policy.removeAccount(scope).completed()

        assertTrue(vault.isDirectory)
        assertTrue(policy.recordFor(scope).completed().isFile)
        for (store in PrivateStore.values().filter { !it.perCredential }) {
            assertFalse(store.name, directoryFor(policy, store).exists())
        }
        assertEquals(ACCOUNT_ENTRIES.toLong(), removed.toLong())
    }

    @Test
    fun removingWhatWasNeverThereRemovesNothing() {
        val policy = policy()

        assertEquals(0L, policy.removeCredential(scope).completed().toLong())
        assertEquals(0L, policy.removeAccount(scope).completed().toLong())
        assertFalse(directoryFor(policy, PrivateStore.Vault).exists())
    }

    @Test
    fun aTreeDeeperThanThePolicyRemovesIsRefusedRatherThanWalked() {
        val policy = policy()
        val vault = directoryFor(policy, PrivateStore.Vault)
        var deep = vault
        repeat(BEYOND_DEPTH) { deep = File(deep, "d") }
        assertTrue(deep.mkdirs())

        val failure = policy.removeCredential(scope).refusal()

        assertTrue(failure.toString(), failure is VaultFailure.RemovalFailed)
        val refused = failure as VaultFailure.RemovalFailed
        assertEquals(RemovalStage.Directory, refused.stage)
        assertTrue(refused.origin, refused.origin.contains("deeper"))
        assertTrue(vault.isDirectory)
    }

    @Test
    fun aDirectoryThatCannotBeListedIsUnknownRatherThanEmpty() {
        val io = FaultyRecordIo()
        val root = temporary.newFolder()
        val policy = PrivateStoragePolicy(root, VaultNamespace.Application, io)
        val vault = policy.directoryFor(PrivateStore.Vault, scope).completed()
        seed(File(vault, "content"))
        io.unlistable.add(vault.path)

        val failure = policy.removeCredential(scope).refusal()

        assertTrue(failure.toString(), failure is VaultFailure.RemovalFailed)
        val refused = failure as VaultFailure.RemovalFailed
        assertEquals(RemovalStage.Directory, refused.stage)
        assertTrue(refused.origin, refused.origin.contains("unlistable"))
        assertEquals(0L, refused.removed.toLong())
        assertTrue(vault.isDirectory)
    }

    @Test
    fun anEntryTheFileSystemKeepsIsReportedWithWhatWasAlreadyRemoved() {
        val io = FaultyRecordIo()
        val root = temporary.newFolder()
        val policy = PrivateStoragePolicy(root, VaultNamespace.Application, io)
        val vault = policy.directoryFor(PrivateStore.Vault, scope).completed()
        val nested = File(vault, "sub")
        seed(File(nested, "gone"))
        io.kept.add(nested.path)

        val failure = policy.removeCredential(scope).refusal()

        assertTrue(failure.toString(), failure is VaultFailure.RemovalFailed)
        val refused = failure as VaultFailure.RemovalFailed
        assertEquals(RemovalStage.Directory, refused.stage)
        assertEquals("the file it did remove went uncounted", 1L, refused.removed.toLong())
        assertFalse(File(nested, "gone").exists())
        assertTrue(nested.isDirectory)
    }

    /** The directory of [store], which must be a private path to be returned. */
    private fun directoryFor(
        policy: PrivateStoragePolicy,
        store: PrivateStore,
        of: VaultScope = scope,
    ): File = policy.directoryFor(store, of).completed()

    private fun policy(): PrivateStoragePolicy =
        PrivateStoragePolicy(temporary.newFolder(), VaultNamespace.Application)

    private fun seed(file: File) {
        val parent = requireNotNull(file.parentFile)
        assertTrue(parent.path, parent.isDirectory || parent.mkdirs())
        file.writeText("synthetic private content")
    }

    private companion object {

        val IDENTITIES = listOf("source-1", "provider.example", "account-1", "credential-1")

        /** Two records, the directory of the second and the store itself. */
        const val REMOVED_ENTRIES = 4

        /** One file in each account store, and each of those stores. */
        const val ACCOUNT_ENTRIES = 8

        /** Deeper than the policy walks, so the refusal is the bound, not the disk. */
        const val BEYOND_DEPTH = 70
    }
}
