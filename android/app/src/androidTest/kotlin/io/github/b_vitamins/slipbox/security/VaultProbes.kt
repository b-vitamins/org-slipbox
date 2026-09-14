/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import android.content.Context
import java.io.File
import java.io.IOException
import java.security.KeyStore
import java.security.SecureRandom
import java.util.Locale

/**
 * One device run's own keys and private files.
 *
 * Every alias and directory a probe creates is named under a run of its own, so
 * [release] removes exactly what that run made. Nothing here enumerates or
 * deletes an entry of the application's name space, whose keys and records a test
 * leaves as it found them.
 */
internal class VaultProbeRun(private val context: Context) {

    /** This run's name space, apart from the application's in alias and path. */
    val namespace: VaultNamespace = VaultNamespace.forRun(runId()).completed()

    /** The platform keystore, restricted to this run's aliases. */
    val keys: AndroidKeystoreVaultKeys = SlipboxVault.keys(namespace)

    /** Where this run may keep private files on the device it runs on. */
    val policy: PrivateStoragePolicy = SlipboxVault.policy(context, namespace).completed()

    private val used = mutableListOf<VaultScope>()

    /** A scope of synthetic identities, which [release] then cleans up after. */
    fun scope(
        sourceId: String = "probe-source-1",
        providerAuthority: String = "probe.example",
        accountId: String = "probe-account-1",
        credentialRef: String = "probe-credential-1",
    ): VaultScope =
        VaultScope.of(sourceId, providerAuthority, accountId, credentialRef).completed().also {
            used.add(it)
        }

    /** A vault of this run for [scope], over the platform keystore and a real file. */
    fun vault(scope: VaultScope): CredentialVault =
        SlipboxVault.open(context, namespace, scope).completed()

    /**
     * [work] over a vault of [scope], which has released that record before this
     * returns.
     *
     * A claim on a record outlives [CredentialVault.close] until the vault's thread
     * has terminated, so a test that opens a second vault over the same record waits
     * here rather than racing that release.
     */
    fun <T> using(scope: VaultScope, work: (CredentialVault) -> T): T {
        val vault = vault(scope)
        return try {
            work(vault)
        } finally {
            vault.close()
            if (!vault.awaitClosed(CLOSE_TIMEOUT_MILLIS)) {
                throw AssertionError("a vault of this run never released its record")
            }
        }
    }

    /** The sealed record file of [scope], as this run's policy places it. */
    fun record(scope: VaultScope): File = policy.recordFor(scope).completed()

    /** The alias of [scope]'s [generation] key in this run's name space. */
    fun alias(scope: VaultScope, generation: Int = VaultNamespace.FIRST_GENERATION): String =
        namespace.aliasFor(scope, generation)

    /** The aliases the platform keystore holds for this run. */
    fun aliases(): List<String> = storedAliases().filter { it.startsWith(namespace.aliasPrefix) }

    /** How many aliases the keystore holds that are not this run's. */
    fun otherAliases(): Int = storedAliases().count { !it.startsWith(namespace.aliasPrefix) }

    /**
     * Deletes this run's keys and private files and reports what is left.
     *
     * The stores go through the production removal. What can remain beneath the
     * run root is then empty directories, and [File.delete] refuses a directory
     * that still holds anything, so a file this run did not account for leaves
     * the root standing to be reported rather than being deleted unseen.
     */
    fun release(): VaultProbeRemains {
        val removedKeys = keys.releaseNamespace().completed()
        var removedEntries = 0
        for (scope in used) {
            removedEntries += policy.removeCredential(scope).completed()
            removedEntries += policy.removeAccount(scope).completed()
        }
        for (child in policy.root.listFiles().orEmpty()) {
            child.delete()
        }
        policy.root.delete()
        return VaultProbeRemains(removedKeys, removedEntries, aliases(), policy.root.exists())
    }

    private fun storedAliases(): List<String> {
        val store = KeyStore.getInstance(PROVIDER)
        store.load(null)
        return store.aliases().toList()
    }

    private companion object {

        const val PROVIDER = "AndroidKeyStore"

        /** Long enough for accepted work to finish, short enough to end a test. */
        const val CLOSE_TIMEOUT_MILLIS = 10_000L
    }
}

/**
 * This device's own file system with the write of a record made to fail partway.
 *
 * The fault stands in for the one a full or failing disk raises, which a device run
 * cannot otherwise reach; every other operation is the real one, so the commit
 * protocol under test is the one the app runs here.
 */
internal class HalfWrittenRecordIo(private val message: String) : RecordIo by SystemRecordIo {

    override fun open(file: File): RecordSink = HalfWritingSink(SystemRecordIo.open(file))

    private inner class HalfWritingSink(private val sink: RecordSink) : RecordSink {

        override fun write(record: ByteArray) {
            sink.write(record.copyOf(1))
            throw IOException(message)
        }

        override fun sync() {
            sink.sync()
        }

        override fun close() {
            sink.close()
        }
    }
}

/** What releasing a run left behind: no alias and no root, when it was exact. */
internal class VaultProbeRemains(
    val keysRemoved: Int,
    val entriesRemoved: Int,
    val aliases: List<String>,
    val rootExists: Boolean,
)

/** The value of an outcome a test expects to have completed. */
internal fun <T> VaultOutcome<T>.completed(): T =
    when (this) {
        is VaultOutcome.Completed -> value
        is VaultOutcome.Failed -> throw AssertionError("expected a value, got $failure")
    }

/** The failure of an outcome a test expects to have failed. */
internal fun <T> VaultOutcome<T>.refusal(): VaultFailure =
    when (this) {
        is VaultOutcome.Completed -> throw AssertionError("expected a failure, got $value")
        is VaultOutcome.Failed -> failure
    }

/** These bytes as lowercase hex, for reading a record header independently. */
internal fun hexOf(bytes: ByteArray): String =
    bytes.joinToString("") { "%02x".format(Locale.ROOT, it) }

/**
 * A synthetic token, marked as one and random per run.
 *
 * No fixture on the device can be a real credential, and nothing logs one of
 * these: an observation carries the marker's absence and a length, never a value.
 */
internal fun syntheticToken(marker: String): String {
    val tail = ByteArray(TOKEN_TAIL_BYTES)
    SecureRandom().nextBytes(tail)
    return "$TOKEN_MARKER$marker-${hexOf(tail)}"
}

/** Random per run, so one run never reads or deletes another's entries. */
private fun runId(): String {
    val tail = ByteArray(RUN_TAIL_BYTES)
    SecureRandom().nextBytes(tail)
    return "probe-${hexOf(tail)}"
}

/** What every synthetic token a probe makes begins with. */
internal const val TOKEN_MARKER = "synthetic-"

private const val TOKEN_TAIL_BYTES = 16

private const val RUN_TAIL_BYTES = 5
