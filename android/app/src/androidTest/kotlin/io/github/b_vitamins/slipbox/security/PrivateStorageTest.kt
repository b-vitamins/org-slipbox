/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import android.system.Os
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.io.IOException

/**
 * Where this installation keeps private data, and what a cleanup on a real file
 * system is allowed to touch.
 *
 * The roots are the ones the platform gives this app, and the links are planted with
 * the platform's own linker: which directories an installation gets, and how they
 * resolve on the device's file system, is what only a device run answers.
 */
@RunWith(AndroidJUnit4::class)
class PrivateStorageTest {

    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    private val context = instrumentation.targetContext

    private val run = VaultProbeRun(context)

    @After
    fun releaseThisRun() {
        val remains = run.release()
        assertEquals(remains.aliases.toString(), emptyList<String>(), remains.aliases)
        assertFalse(run.policy.root.path, remains.rootExists)
    }

    @Test
    fun everyStoreOfThisInstallationSitsInsideItsOwnNoBackupRoot() {
        val applicationRoot = SlipboxVault.privateRoot(context).completed()
        assertEquals(context.noBackupFilesDir, applicationRoot)
        assertEquals(PRIVATE_ROOT, applicationRoot.name)
        assertTrue(applicationRoot.path, applicationRoot.isDirectory)
        val data = File(context.applicationInfo.dataDir).canonicalPath
        assertTrue(
            applicationRoot.canonicalPath,
            applicationRoot.canonicalPath.startsWith(data + File.separator),
        )
        val application = SlipboxVault.policy(context).completed()
        assertEquals(applicationRoot, application.root)

        val scope = run.scope()
        assertTrue(
            run.policy.root.path,
            run.policy.root.path.startsWith(applicationRoot.path + File.separator),
        )
        val directories = mutableSetOf<File>()
        for (store in PrivateStore.values()) {
            val directory = run.policy.directoryFor(store, scope).completed()
            directories.add(directory)
            assertTrue(
                directory.path,
                directory.path.startsWith(run.policy.root.path + File.separator),
            )
            assertTrue(directory.path, directory.isDirectory || directory.mkdirs())
            for (identity in IDENTITIES) {
                assertFalse("$store names $identity", directory.path.contains(identity))
            }
        }
        assertEquals(PrivateStore.values().size.toLong(), directories.size.toLong())

        // The application's own record for the same scope is outside this run and
        // stays absent: a test writes nothing through the shipped name space.
        val applicationRecord = application.recordFor(scope).completed()
        assertFalse(
            applicationRecord.path,
            applicationRecord.path.startsWith(run.policy.root.path + File.separator),
        )
        assertFalse(applicationRecord.path, applicationRecord.exists())
        Log.i(
            TAG,
            "private root ${applicationRoot.name} inside the app data directory," +
                " ${directories.size} store(s) of this run beneath it",
        )
    }

    @Test
    fun aNameThatResolvesThroughALinkOutOfItsStoreIsRefused() {
        val scope = run.scope()
        val store = run.policy.directoryFor(PrivateStore.Assets, scope).completed()
        assertTrue(store.path, store.isDirectory || store.mkdirs())
        val outside = File(run.policy.root, OUTSIDE_NAME)
        seed(outside)
        Os.symlink(outside.path, File(store, LINK_NAME).path)

        // A name this policy will not spell is refused as a name; a name it spells
        // whose path leaves the root is refused as a path.
        assertEquals(
            VaultFailure.RefusedInput(FILE_FIELD, InputDefect.Traversing),
            run.policy.fileFor(PrivateStore.Assets, scope, TRAVERSING_NAME).refusal(),
        )
        assertEquals(
            VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping),
            run.policy.fileFor(PrivateStore.Assets, scope, LINK_NAME).refusal(),
        )
        assertEquals(
            File(store, PLAIN_NAME),
            run.policy.fileFor(PrivateStore.Assets, scope, PLAIN_NAME).completed(),
        )
        assertEquals(FILLER, outside.readText())
    }

    @Test
    fun aStoreThatIsItselfALinkOutOfTheRootIsRefusedAndSoIsEveryNameBeneathIt() {
        val scope = run.scope()
        val store = run.policy.directoryFor(PrivateStore.Index, scope).completed()
        val parent = requireNotNull(store.parentFile)
        assertTrue(parent.path, parent.isDirectory || parent.mkdirs())
        val outsideDirectory = File(run.policy.root, OUTSIDE_DIRECTORY)
        val outsideEntry = File(outsideDirectory, PLAIN_NAME)
        seed(outsideEntry)
        Os.symlink(outsideDirectory.path, store.path)

        val escaping = VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping)
        assertEquals(escaping, run.policy.directoryFor(PrivateStore.Index, scope).refusal())
        assertEquals(escaping, run.policy.fileFor(PrivateStore.Index, scope, PLAIN_NAME).refusal())

        // The account's stores go through a cleanup, which unlinks the planted store
        // and leaves what it pointed at.
        assertEquals(
            LINKED_STORE_ENTRIES.toLong(),
            run.policy.removeAccount(scope).completed().toLong(),
        )
        assertFalse(store.path, store.exists())
        assertEquals(FILLER, outsideEntry.readText())
        Log.i(TAG, "a store that was a link was refused, unlinked, and its target left")

        // Planted outside every store, so this test removes it rather than a
        // production cleanup.
        assertTrue(outsideEntry.delete())
        assertTrue(outsideDirectory.delete())
    }

    @Test
    fun aCleanupUnlinksAPlantedLinkAndLeavesWhatItPointsAt() {
        val scope = run.scope()
        val store = run.policy.directoryFor(PrivateStore.Vault, scope).completed()
        seed(run.record(scope))
        val outsideFile = File(run.policy.root, OUTSIDE_NAME)
        seed(outsideFile)
        val outsideDirectory = File(run.policy.root, OUTSIDE_DIRECTORY)
        val outsideEntry = File(outsideDirectory, PLAIN_NAME)
        seed(outsideEntry)
        Os.symlink(outsideFile.path, File(store, LINK_NAME).path)
        Os.symlink(outsideDirectory.path, File(store, LINK_DIRECTORY).path)

        val removed = run.policy.removeCredential(scope).completed()

        assertFalse(store.path, store.exists())
        assertEquals(FILLER, outsideFile.readText())
        assertEquals(FILLER, outsideEntry.readText())
        assertEquals(LINKED_ENTRIES.toLong(), removed.toLong())
        Log.i(TAG, "a cleanup unlinked two planted links and left both targets in place")

        // Planted outside every store, so this test removes it rather than a
        // production cleanup.
        assertTrue(outsideEntry.delete())
        assertTrue(outsideDirectory.delete())
    }

    @Test
    fun aReplacementThatFailsBeforeItCommitsLeavesThePreviousRecordReadable() {
        val scope = run.scope()
        val record = run.record(scope)
        val directory = requireNotNull(record.parentFile)
        assertTrue(directory.path, directory.isDirectory || directory.mkdirs())

        val file = AtomicVaultRecordFile(record)
        file.replace(RECORD_ONE).completed()
        assertArrayEquals(RECORD_ONE, file.read(LIMIT).completed())

        val failing = AtomicVaultRecordFile(record, HalfWrittenRecordIo(QUOTED_INPUT))
        val failure = failing.replace(RECORD_TWO).refusal()

        assertEquals(
            VaultFailure.WriteFailed(CommitPhase.BeforeCommit, IOException::class.java.name),
            failure,
        )
        assertFalse(failure.toString(), failure.toString().contains(QUOTED_INPUT))
        assertArrayEquals(RECORD_ONE, file.read(LIMIT).completed())
        assertEquals(listOf(record.name), directory.list().orEmpty().sorted())

        file.replace(RECORD_TWO).completed()
        assertArrayEquals(RECORD_TWO, file.read(LIMIT).completed())
        assertTrue(file.delete().completed())
        assertFalse(file.delete().completed())
        assertNull(file.read(LIMIT).completed())
        Log.i(TAG, "a failure before the commit left the previous record and no staging file")
    }

    @Test
    fun aVaultWhoseStagingRecordIsALinkRefusesRatherThanWritingThroughIt() {
        val scope = run.scope()
        val outside = File(run.policy.root, OUTSIDE_NAME)
        seed(outside)
        val credential = StoredCredential(syntheticToken("access"))

        run.using(scope) { vault ->
            vault.replace(credential).completed()
            val record = run.record(scope)
            val stored = record.readBytes()
            Os.symlink(outside.path, File(record.path + STAGING_SUFFIX).path)

            val refusal = vault.replace(StoredCredential(syntheticToken("second"))).refusal()

            assertEquals(
                VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping),
                refusal,
            )
            assertEquals(FILLER, outside.readText())
            assertArrayEquals("the stored record was replaced", stored, record.readBytes())
            assertEquals(credential, vault.read().completed())
        }
        Log.i(TAG, "a staging record that was a link was refused and its target left")
    }

    @Test
    fun removingOneScopeTakesItsOwnEntriesAndLeavesAnotherAccountsAlone() {
        val scope = run.scope()
        val other = run.scope(accountId = "probe-account-2")
        for (store in PrivateStore.values()) {
            seed(run.policy.fileFor(store, scope, PLAIN_NAME).completed())
            seed(run.policy.fileFor(store, other, PLAIN_NAME).completed())
        }

        assertEquals(
            CREDENTIAL_ENTRIES.toLong(),
            run.policy.removeCredential(scope).completed().toLong(),
        )
        assertFalse(run.policy.directoryFor(PrivateStore.Vault, scope).completed().exists())
        assertEquals(0L, run.policy.removeCredential(scope).completed().toLong())

        assertEquals(ACCOUNT_ENTRIES.toLong(), run.policy.removeAccount(scope).completed().toLong())
        assertEquals(0L, run.policy.removeAccount(scope).completed().toLong())

        for (store in PrivateStore.values()) {
            val file = run.policy.fileFor(store, other, PLAIN_NAME).completed()
            assertEquals(store.name, FILLER, file.readText())
        }
        Log.i(
            TAG,
            "removed $CREDENTIAL_ENTRIES credential and $ACCOUNT_ENTRIES account entries," +
                " another account's ${PrivateStore.values().size} store(s) untouched",
        )
    }

    private fun seed(file: File) {
        val parent = requireNotNull(file.parentFile)
        assertTrue(parent.path, parent.isDirectory || parent.mkdirs())
        file.writeText(FILLER)
    }

    private companion object {

        const val TAG = "SlipboxVaultProbe"

        /** The directory name the platform excludes from backup by itself. */
        const val PRIVATE_ROOT = "no_backup"

        /** The synthetic identities of a probe scope, none of which may name a path. */
        val IDENTITIES =
            listOf("probe-source-1", "probe.example", "probe-account-1", "probe-credential-1")

        const val PLAIN_NAME = "content.bin"

        /** What a refusal of a name, rather than of the path it resolves to, names. */
        const val FILE_FIELD = "file name"

        /** A name no store spells, refused before any path is resolved. */
        const val TRAVERSING_NAME = "../content.bin"

        const val LINK_NAME = "linked"

        const val LINK_DIRECTORY = "linked-tree"

        const val OUTSIDE_NAME = "outside.bin"

        const val OUTSIDE_DIRECTORY = "outside-tree"

        /** What a record protocol names the entry it stages a replacement in. */
        const val STAGING_SUFFIX = ".new"

        const val FILLER = "probe private content"

        /** A record, a link, a link to a directory and the store itself. */
        const val LINKED_ENTRIES = 4

        /** A store that was a link, unlinked and counted as the one entry it was. */
        const val LINKED_STORE_ENTRIES = 1

        /** One file in the vault store, and that store. */
        const val CREDENTIAL_ENTRIES = 2

        /** One file in each of the four account stores, and each of those stores. */
        const val ACCOUNT_ENTRIES = 8

        /** Well above these fixtures, so no read here is refused for its size. */
        const val LIMIT = 4096

        /** A message a failure must not repeat, standing in for any platform text. */
        const val QUOTED_INPUT = "an error message that quotes its input"

        val RECORD_ONE = ByteArray(64) { 1 }

        val RECORD_TWO = ByteArray(96) { 2 }
    }
}
