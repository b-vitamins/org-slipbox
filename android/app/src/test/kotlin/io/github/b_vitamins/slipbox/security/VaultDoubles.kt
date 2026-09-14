/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.io.File
import java.io.InputStream
import java.security.GeneralSecurityException
import java.security.Key
import java.security.SecureRandom
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.SecretKeySpec

/**
 * Keys the JVM's own provider holds in memory, able to present the faults a platform
 * provider does.
 *
 * The keys are real AES keys and the cipher over them is the production one. Where a
 * key lives, whether it is exportable and which faults a provider raises are the
 * platform's, and only a device run answers for them.
 */
internal class FakeVaultKeys : VaultKeys {

    /** What every call fails with, or null to answer normally. */
    var failure: VaultFailure? = null

    /** What a deletion by prefix fails with, the other calls answering normally. */
    var sweepFailure: VaultFailure? = null

    /** What this provider says its platform errors mean. */
    var classified: VaultFailure? = null

    private val stored = ConcurrentHashMap<String, SecretKey>()

    /** The aliases this provider currently holds. */
    val aliases: Set<String>
        get() = stored.keys.toSet()

    override fun existing(alias: String): VaultOutcome<SecretKey> {
        failure?.let { return VaultOutcome.Failed(it) }
        val key = stored[alias] ?: return VaultOutcome.Failed(VaultFailure.KeyMissing)
        return VaultOutcome.Completed(key)
    }

    override fun provision(alias: String): VaultOutcome<SecretKey> {
        failure?.let { return VaultOutcome.Failed(it) }
        return VaultOutcome.Completed(
            stored.getOrPut(alias) {
                KeyGenerator.getInstance("AES").apply { init(KEY_SIZE) }.generateKey()
            },
        )
    }

    override fun delete(alias: String): VaultOutcome<Boolean> {
        failure?.let { return VaultOutcome.Failed(it) }
        return VaultOutcome.Completed(stored.remove(alias) != null)
    }

    override fun deleteAll(prefix: String, keep: String?): VaultOutcome<Int> {
        sweepFailure?.let { return VaultOutcome.Failed(it) }
        failure?.let { return VaultOutcome.Failed(it) }
        var removed = 0
        for (alias in stored.keys.toList()) {
            if (alias.startsWith(prefix) && alias != keep && stored.remove(alias) != null) {
                removed++
            }
        }
        return VaultOutcome.Completed(removed)
    }

    override fun classify(error: Throwable): VaultFailure? = classified

    /** Loses [alias] as a provider does when its key can no longer be held. */
    fun forget(alias: String) {
        stored.remove(alias)
    }

    /** Leaves [alias] holding a key the cipher provider will not accept. */
    fun spoil(alias: String) {
        stored[alias] = SecretKeySpec(ByteArray(UNUSABLE_KEY_SIZE), "AES")
    }

    private companion object {

        const val KEY_SIZE = 256

        const val UNUSABLE_KEY_SIZE = 7
    }
}

/**
 * One record held in memory, able to present the faults a real file does.
 *
 * A refused replacement leaves the previous record in place, as
 * [AtomicVaultRecordFile] does on a file system.
 */
internal class FakeRecordFile : AtomicRecordFile {

    /** The stored record, readable and writable by a test directly. */
    var record: ByteArray? = null

    var readFailure: VaultFailure? = null

    var writeFailure: VaultFailure? = null

    var deleteFailure: VaultFailure? = null

    /** How many replacements were attempted, refused ones included. */
    var attempts = 0
        private set

    /** Runs before each replacement, for a test that needs one in flight. */
    var beforeReplace: () -> Unit = {}

    override fun read(limit: Int): VaultOutcome<ByteArray?> {
        readFailure?.let { return VaultOutcome.Failed(it) }
        val stored = record ?: return VaultOutcome.Completed(null)
        if (stored.size > limit) {
            return VaultOutcome.Failed(VaultFailure.CorruptRecord(RecordDefect.Oversized))
        }
        return VaultOutcome.Completed(stored.copyOf())
    }

    override fun replace(record: ByteArray): VaultOutcome<Unit> {
        attempts++
        beforeReplace()
        writeFailure?.let { return VaultOutcome.Failed(it) }
        this.record = record.copyOf()
        return VaultOutcome.Completed(Unit)
    }

    override fun delete(): VaultOutcome<Boolean> {
        deleteFailure?.let { return VaultOutcome.Failed(it) }
        val existed = record != null
        record = null
        return VaultOutcome.Completed(existed)
    }
}

/**
 * The real file system with one operation of it made to fail.
 *
 * Everything a test does not plant falls through to real files, so the commit protocol
 * under test is the production one; the planted fault stands in for the one a full or
 * failing disk raises.
 */
internal class FaultyRecordIo(private val real: RecordIo = SystemRecordIo) : RecordIo {

    var readFault: (() -> Throwable)? = null

    var openFault: (() -> Throwable)? = null

    var writeFault: (() -> Throwable)? = null

    var syncFault: (() -> Throwable)? = null

    var closeFault: (() -> Throwable)? = null

    var entriesFault: (() -> Throwable)? = null

    var renameFault: (() -> Throwable)? = null

    var canonicalFault: (() -> Throwable)? = null

    var removeFault: (() -> Throwable)? = null

    /** Whether a rename reports refusal instead of renaming. */
    var renameRefused = false

    /** Whether a rename reports success and leaves the name it renamed from. */
    var renameKeepsSource = false

    /** Paths this file system accepts a deletion of and keeps. */
    val kept = mutableSetOf<String>()

    /** Paths [removeFault] is raised for, every path when empty. */
    val faulting = mutableSetOf<String>()

    /** Directories this file system will not list. */
    val unlistable = mutableSetOf<String>()

    /** Runs at the start of every open, for a test that needs one in flight. */
    var beforeOpen: () -> Unit = {}

    /** Runs after a rename this file system carried out, for a fault planted there. */
    var afterRename: () -> Unit = {}

    override fun entries(directory: File): Array<String>? {
        entriesFault?.let { throw it() }
        return if (directory.path in unlistable) null else real.entries(directory)
    }

    override fun createDirectory(directory: File): Boolean = real.createDirectory(directory)

    override fun open(file: File): RecordSink {
        beforeOpen()
        openFault?.let { throw it() }
        return FaultySink(real.open(file))
    }

    override fun read(file: File): InputStream {
        readFault?.let { throw it() }
        return real.read(file)
    }

    override fun rename(from: File, to: File): Boolean {
        renameFault?.let { throw it() }
        return when {
            renameRefused -> false
            renameKeepsSource -> {
                to.writeBytes(from.readBytes())
                true
            }
            else -> real.rename(from, to).also { if (it) afterRename() }
        }
    }

    override fun remove(file: File): Boolean {
        if (faulting.isEmpty() || file.path in faulting) {
            removeFault?.let { throw it() }
        }
        return if (file.path in kept) false else real.remove(file)
    }

    override fun exists(file: File): Boolean = real.exists(file)

    override fun isDirectory(file: File): Boolean = real.isDirectory(file)

    override fun canonicalPath(file: File): String {
        canonicalFault?.let { throw it() }
        return real.canonicalPath(file)
    }

    private inner class FaultySink(private val sink: RecordSink) : RecordSink {

        override fun write(record: ByteArray) {
            writeFault?.let { throw it() }
            sink.write(record)
        }

        override fun sync() {
            syncFault?.let { throw it() }
            sink.sync()
        }

        override fun close() {
            closeFault?.let { throw it() }
            sink.close()
        }
    }
}

/**
 * A key store in memory, able to present the faults a platform provider does.
 *
 * The keys are real AES keys, so [AndroidKeystoreVaultKeys] and the production cipher
 * run over them here; what the platform provider itself does with an alias is left to
 * a device run.
 */
internal class FakeKeystoreProvider : KeystoreProvider {

    var openFault: (() -> Throwable)? = null

    var readFault: (() -> Throwable)? = null

    var generateFault: (() -> Throwable)? = null

    /** What a deletion raises, for the faults a keystore raises unchecked. */
    var deleteFault: (() -> Throwable)? = null

    private val stored = mutableMapOf<String, SecretKey>()

    private val invalidated = mutableSetOf<String>()

    /** Aliases this provider accepts a deletion of and keeps. */
    val kept = mutableSetOf<String>()

    /** Every alias this provider holds, invalidated ones included. */
    val aliases: Set<String>
        get() = stored.keys + invalidated

    override fun open(): KeystoreHandle {
        openFault?.let { throw it() }
        return Handle()
    }

    override fun generate(alias: String, size: Int): SecretKey {
        generateFault?.let { throw it() }
        invalidated.remove(alias)
        val key = KeyGenerator.getInstance("AES").apply { init(size) }.generateKey()
        stored[alias] = key
        return key
    }

    override fun meaning(error: Throwable): VaultFailure? =
        if (error is InvalidatedKey) VaultFailure.KeyInvalidated else null

    /** Keeps [alias] as an entry whose key the provider will no longer produce. */
    fun invalidate(alias: String) {
        stored.remove(alias)
        invalidated.add(alias)
    }

    /** Loses [alias] entirely, as a provider does when its entry is gone. */
    fun forget(alias: String) {
        stored.remove(alias)
        invalidated.remove(alias)
    }

    private inner class Handle : KeystoreHandle {

        override fun key(alias: String): Key? {
            readFault?.let { throw it() }
            if (alias in invalidated) {
                throw InvalidatedKey()
            }
            return stored[alias]
        }

        override fun contains(alias: String): Boolean = alias in aliases

        override fun delete(alias: String) {
            deleteFault?.let { throw it() }
            if (alias in kept) {
                return
            }
            stored.remove(alias)
            invalidated.remove(alias)
        }

        override fun aliases(): List<String> = aliases.toList()
    }

    /** What a platform provider raises for a key its own conditions invalidated. */
    internal class InvalidatedKey : GeneralSecurityException("synthetic invalidated key")
}

/**
 * Links [link] to [target] with the system linker.
 *
 * The link is a real symbolic link on the file system the test runs on.
 */
internal fun link(link: File, target: File) {
    run("ln", "-s", target.path, link.path)
}

/**
 * Takes every permission from [directory], runs [work], and gives them back.
 *
 * A directory this process cannot traverse is the one thing that makes a stat of a
 * name beneath it answer false while that name is there; no fault planted above the
 * file system reproduces it.
 */
internal fun <T> withoutAccess(directory: File, work: () -> T): T {
    run("chmod", NO_ACCESS, directory.path)
    return try {
        work()
    } finally {
        run("chmod", OWNER_ACCESS, directory.path)
    }
}

/**
 * Runs [command] and waits for it, failing when it does not exit cleanly.
 *
 * `java.nio.file` and the bounded [Process.waitFor] are both above this app's minimum
 * API, so the file system is asked the way the shell asks it and the wait is polled.
 */
private fun run(vararg command: String) {
    val running = ProcessBuilder(*command).start()
    val deadline = System.currentTimeMillis() + COMMAND_TIMEOUT_MILLIS
    var status: Int? = null
    while (status == null) {
        status = exitStatusOf(running)
        if (status == null) {
            if (System.currentTimeMillis() > deadline) {
                running.destroy()
                throw AssertionError("${command.first()} did not finish")
            }
            Thread.sleep(COMMAND_POLL_MILLIS)
        }
    }
    if (status != 0) {
        throw AssertionError("${command.joinToString(" ")} exited $status")
    }
}

/** The status [process] exited with, or null while it is still running. */
private fun exitStatusOf(process: Process): Int? =
    try {
        process.exitValue()
    } catch (running: IllegalThreadStateException) {
        null
    }

/** A foreground thread a test can move under its own feet. */
internal class FakeForegroundThread(var current: Boolean = false) : ForegroundThread {

    override fun isCurrent(): Boolean = current
}

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

/**
 * The result of [work], which a fault may not escape.
 *
 * The escaping fault itself is not the report: only its type is named, so a planted
 * message never reaches the test report.
 */
internal fun <T> answered(work: () -> T): T =
    try {
        work()
    } catch (fault: Throwable) {
        throw AssertionError("a fault escaped as ${fault.javaClass.name}")
    }

/** A scope of synthetic identities, differing only where a test says. */
internal fun testScope(
    sourceId: String = "source-1",
    providerAuthority: String = "provider.example",
    accountId: String = "account-1",
    credentialRef: String = "credential-1",
): VaultScope = VaultScope.of(sourceId, providerAuthority, accountId, credentialRef).completed()

/**
 * One record file at a path a test can move under a vault's feet.
 *
 * The default path is derived from [file], so two sources over one record name it
 * identically and the second vault over it is refused as one over a real path is.
 */
internal class FakeRecordSource(
    private val file: AtomicRecordFile,
    var path: String = pathOf(file),
) : RecordSource {

    /** What resolving fails with, or null to answer normally. */
    var failure: VaultFailure? = null

    /** How many times a vault has resolved this record. */
    var resolutions = 0
        private set

    override fun resolve(): VaultOutcome<ResolvedRecord> {
        synchronized(this) { resolutions++ }
        failure?.let { return VaultOutcome.Failed(it) }
        return VaultOutcome.Completed(ResolvedRecord(path, file))
    }

    private companion object {

        fun pathOf(file: AtomicRecordFile): String =
            "/private/vault/${System.identityHashCode(file)}/credential.bin"
    }
}

/**
 * The plaintext buffers a vault took, kept for inspection after it finished, so a test
 * reads what is left in them instead of taking the zeroing on trust.
 */
internal class RecordingBuffers : VaultBuffers {

    private val handed = mutableListOf<ByteArray>()

    /** How many buffers were taken. */
    val taken: Int
        get() = synchronized(handed) { handed.size }

    /** The lengths asked for, in the order they were asked for. */
    val lengths: List<Int>
        get() = synchronized(handed) { handed.map { it.size } }

    /** How many buffers still carry a byte that is not zero. */
    val uncleared: Int
        get() = synchronized(handed) { handed.count { bytes -> bytes.any { it != 0.toByte() } } }

    override fun take(length: Int): ByteArray =
        ByteArray(length).also { synchronized(handed) { handed.add(it) } }
}

/**
 * The delivery faults a vault reported, by type and in order.
 *
 * [fault] makes the observer itself throw once it has recorded, standing in for a policy
 * that fails while being told about a failure.
 */
internal class FakeDeliveryFaults(private val fault: (() -> Throwable)? = null) :
    VaultDeliveryFaults {

    private val reported = CopyOnWriteArrayList<String>()

    val origins: List<String>
        get() = reported.toList()

    override fun onDeliveryFault(origin: String) {
        reported.add(origin)
        fault?.let { throw it() }
    }
}

/**
 * The vaults one test opened, released together when it ends.
 *
 * A claim on a record outlives [CredentialVault.close] until the vault's thread has
 * terminated, and the process holds at most [VaultOwners.LIMIT] records at once, so
 * a suite that leaks a vault exhausts that table for every suite after it. Each
 * suite here keeps one of these and calls [closeAll] when its test ends.
 */
internal class OpenVaults {

    private val opened = mutableListOf<CredentialVault>()

    /**
     * A vault of [scope] over [source] with the production cipher, or why there is
     * none.
     */
    fun open(
        scope: VaultScope = testScope(),
        keys: VaultKeys = FakeVaultKeys(),
        file: AtomicRecordFile = FakeRecordFile(),
        foreground: ForegroundThread = ForegroundThread.None,
        namespace: VaultNamespace = VaultNamespace.Application,
        faults: VaultDeliveryFaults = VaultDeliveryFaults.Ignored,
        buffers: VaultBuffers = VaultBuffers.Direct,
        source: RecordSource = FakeRecordSource(file),
        cipher: VaultCipher = AesGcmVaultCipher(keys),
    ): VaultOutcome<CredentialVault> {
        val outcome =
            CredentialVault.opened(
                scope = scope,
                namespace = namespace,
                keys = keys,
                cipher = cipher,
                source = source,
                foreground = foreground,
                faults = faults,
                buffers = buffers,
            )
        if (outcome is VaultOutcome.Completed) {
            synchronized(opened) { opened.add(outcome.value) }
        }
        return outcome
    }

    /** Closes every vault this test opened and waits for each to release. */
    fun closeAll() {
        val vaults = synchronized(opened) { opened.toList().also { opened.clear() } }
        for (vault in vaults) {
            vault.close()
            if (!vault.awaitClosed(CLOSE_TIMEOUT_MILLIS)) {
                throw AssertionError("a vault of this test never released its record")
            }
        }
    }
}

/**
 * A synthetic token, marked as one and random per run.
 *
 * No fixture here can be a real credential, and no test prints one of these:
 * assertions carry the marker and the outcome, never the value.
 */
internal fun syntheticToken(marker: String): String {
    val tail = ByteArray(TOKEN_TAIL_BYTES)
    SecureRandom().nextBytes(tail)
    return "synthetic-$marker-${tail.toHex()}"
}

private const val TOKEN_TAIL_BYTES = 16

private const val COMMAND_TIMEOUT_MILLIS = 10_000L

private const val COMMAND_POLL_MILLIS = 10L

private const val NO_ACCESS = "000"

private const val OWNER_ACCESS = "700"

/**
 * The message a planted platform fault carries.
 *
 * It stands in for whatever a real platform exception quotes, and no failure or trace a
 * test reports may repeat it. It is not a credential, so a failing test that prints its
 * own trace leaks nothing.
 */
internal const val QUOTED_INPUT = "an error message that quotes its input"

/** Long enough for accepted work to finish, short enough to end a test. */
internal const val CLOSE_TIMEOUT_MILLIS = 10_000L
