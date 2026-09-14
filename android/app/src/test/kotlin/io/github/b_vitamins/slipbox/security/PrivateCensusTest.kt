/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * The census `tools/verify-private-storage.sh` evaluates against the rules an APK
 * packages, generated here and committed for that script to read.
 *
 * The verifier is a shell script and cannot call the path policy, so the private
 * paths it evaluates are produced by [PrivateStoragePolicy] here rather than spelled
 * again there: a store, an owner digest or a record name that moves fails this test
 * instead of leaving the verifier evaluating a path the app no longer writes.
 */
class PrivateCensusTest {

    @get:Rule
    val temporary = TemporaryFolder()

    @Test
    fun theCommittedCensusIsWhatTheProductionPolicyNamesForItsScope() {
        assertEquals(
            "regenerate ${fixture().path} as the expected text below",
            census(),
            fixture().readText(),
        )
    }

    @Test
    fun theCensusReachesEveryStoreEveryDomainAndPastTheIntrinsicRoots() {
        val entries = entriesOf(fixture().readText())

        assertEquals(DOMAINS.sorted(), entries.map { it.domain }.distinct().sorted())
        assertEquals(
            PrivateStore.values().map { it.directoryName }.sorted(),
            entries.filter { it.origin == POLICY }.map { it.path.split('/')[1] }.distinct().sorted(),
        )

        // The private root, the cache and the code cache leave the app's directory
        // under no mode whatever the rules say, so a census of those alone would hold
        // the packaged rules to nothing.
        // https://developer.android.com/identity/data/autobackup
        val ordinary =
            entries.filter { entry ->
                entry.domain in ROOT_DOMAINS && INTRINSIC.none { entry.path.startsWith(it) }
            }
        assertTrue("no root entry the rules alone must exclude", ordinary.isNotEmpty())

        for (entry in entries) {
            assertFalse(entry.path, entry.path.startsWith("/"))
            assertFalse(entry.path, entry.path.split('/').contains(".."))
            assertTrue(entry.path, entry.what.isNotEmpty())
            for (identity in IDENTITIES) {
                assertFalse("$identity names ${entry.path}", entry.path.contains(identity))
            }
        }
    }

    @Test
    fun theCensusNamesTheRecordAVaultWritesInBothOfTheRootDomains() {
        val record = relative(policy().recordFor(scope()).completed())

        val records = entriesOf(fixture().readText()).filter { it.path == record }

        assertEquals(ROOT_DOMAINS.sorted(), records.map { it.domain }.sorted())
        assertEquals(listOf(POLICY, POLICY), records.map { it.origin })
    }

    /** The census as the verifier reads it, from the policy that names the paths. */
    private fun census(): String {
        val policy = policy()
        val scope = scope()
        val lines = mutableListOf<String>()
        for (store in PrivateStore.values()) {
            for ((name, what) in NAMES.getValue(store)) {
                val file = policy.fileFor(store, scope, name).completed()
                lines.add("$ROOT_DOMAIN|${relative(file)}|$POLICY|$what")
            }
        }
        val record = relative(policy.recordFor(scope).completed())
        lines.add("$DEVICE_ROOT_DOMAIN|$record|$POLICY|$PROTECTED_RECORD")
        for (platform in PLATFORM_PATHS) {
            val (domain, path, what) = platform.split('|')
            lines.add("$domain|$path|$PLATFORM|$what")
        }
        return HEADER + lines.joinToString("") { "$it\n" }
    }

    /** A policy over an empty data directory, rooted as the platform roots one. */
    private fun policy(): PrivateStoragePolicy =
        PrivateStoragePolicy(File(temporary.root, PRIVATE_ROOT), VaultNamespace.Application)

    private fun scope(): VaultScope =
        VaultScope.of(SOURCE_ID, PROVIDER_AUTHORITY, ACCOUNT_ID, CREDENTIAL_REF).completed()

    /** [file] as the census names it: relative to the app's own directory. */
    private fun relative(file: File): String {
        val prefix = temporary.root.path + File.separator
        assertTrue(file.path, file.path.startsWith(prefix))
        return file.path.removePrefix(prefix).replace(File.separatorChar, '/')
    }

    private fun entriesOf(census: String): List<CensusEntry> {
        val entries =
            census
                .lineSequence()
                .filter { it.isNotEmpty() && !it.startsWith("#") }
                .map { it.split('|') }
                .onEach { assertEquals(it.toString(), FIELDS.toLong(), it.size.toLong()) }
                .map { CensusEntry(it[0], it[1], it[2], it[3]) }
                .toList()
        assertTrue("the census is empty", entries.isNotEmpty())
        return entries
    }

    private class CensusEntry(
        val domain: String,
        val path: String,
        val origin: String,
        val what: String,
    )

    private companion object {

        /** Where the verifier reads the census, within the module above this suite. */
        const val FIXTURE = "tools/fixtures/private-census.txt"

        const val PRIVATE_ROOT = "no_backup"

        const val FIELDS = 4

        const val ROOT_DOMAIN = "root"

        const val DEVICE_ROOT_DOMAIN = "device_root"

        /** The domains that carry the app's own directory, private root and all. */
        val ROOT_DOMAINS = listOf(ROOT_DOMAIN, DEVICE_ROOT_DOMAIN)

        /** What those domains keep out of every mode whatever the rules say. */
        val INTRINSIC = listOf("$PRIVATE_ROOT/", "cache/", "code_cache/")

        /** A path this policy named, as against one the platform assigns. */
        const val POLICY = "policy"

        const val PLATFORM = "platform"

        const val SOURCE_ID = "census-source-1"

        const val PROVIDER_AUTHORITY = "census.example"

        const val ACCOUNT_ID = "census-account-1"

        const val CREDENTIAL_REF = "census-credential-1"

        /** The census scope's synthetic identities, none of which may name a path. */
        val IDENTITIES = listOf(SOURCE_ID, PROVIDER_AUTHORITY, ACCOUNT_ID, CREDENTIAL_REF)

        const val PROTECTED_RECORD = "the credential record in device-protected storage"

        const val PREFERENCES = "io.github.b_vitamins.slipbox_preferences.xml"

        /**
         * The file names each store carries in the census.
         *
         * A store the app declares without an entry here fails this suite, so a new
         * store reaches the verifier with the census rather than after it.
         */
        val NAMES: Map<PrivateStore, List<Pair<String, String>>> =
            mapOf(
                PrivateStore.Vault to
                    listOf(PrivateStoragePolicy.RECORD_NAME to "the sealed credential record"),
                PrivateStore.Checkout to listOf("inbox.org" to "a working copy of a source"),
                PrivateStore.Index to
                    listOf(
                        "index.db" to "a derived index of a source",
                        "index.db-wal" to "an index write-ahead log",
                        "index.db-shm" to "an index shared-memory file",
                        "index.db-journal" to "an index rollback journal",
                    ),
                PrivateStore.Assets to listOf("diagram.png" to "an attachment of a note"),
                PrivateStore.Reading to listOf("positions.json" to "the reading state of a source"),
            )

        /**
         * What a backup would otherwise collect outside the private root, as
         * "domain|path|what" and as the platform names each per domain.
         *
         * No code here writes these. They are where a preference, a platform helper
         * or a store outside this policy lands, and the rules must exclude them too.
         */
        val PLATFORM_PATHS =
            listOf(
                "root|files/sources.json|public source configuration",
                "root|cache/index-scratch.bin|a cached scratch file",
                "file|sources.json|source configuration",
                "database|index.db|an index database",
                "database|index.db-wal|an index write-ahead log",
                "database|index.db-shm|an index shared-memory file",
                "database|index.db-journal|an index rollback journal",
                "sharedpref|$PREFERENCES|source configuration",
                "external|assets/diagram.png|an attachment on shared storage",
                "device_file|positions.json|device-protected reading state",
                "device_database|index.db|a device-protected index database",
                "device_sharedpref|$PREFERENCES|device-protected configuration",
            )

        /** Every domain a credential, an index, a checkout or a note can reach. */
        val DOMAINS =
            listOf(
                "root",
                "file",
                "database",
                "sharedpref",
                "external",
                "device_root",
                "device_file",
                "device_database",
                "device_sharedpref",
            )

        val HEADER =
            """
            # The private entries tools/verify-private-storage.sh evaluates against the
            # rules an APK packages, as domain|path|origin|what.
            #
            # Generated by PrivateCensusTest: a `policy` path comes from
            # PrivateStoragePolicy for one synthetic census scope, a `platform` path is
            # one the platform assigns and no code here names. The digests are of that
            # scope alone; no real source, account or credential is named anywhere.
            """
                .trimIndent() + "\n"

        /** The committed census, found from the module this suite runs in. */
        fun fixture(): File {
            var directory: File? = File("").absoluteFile
            while (directory != null) {
                val candidate = File(directory, FIXTURE)
                if (candidate.isFile) {
                    return candidate
                }
                directory = directory.parentFile
            }
            throw AssertionError("no $FIXTURE above ${File("").absolutePath}")
        }
    }
}
