/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.io.Closeable

/** One scope's encrypted credential, with exclusive record ownership and serialized background work. */
class CredentialVault private constructor(
    val scope: VaultScope,
    private val namespace: VaultNamespace,
    private val keys: VaultKeys,
    private val cipher: VaultCipher,
    private val source: RecordSource,
    private val path: String,
    private val buffers: VaultBuffers,
    foreground: ForegroundThread,
    faults: VaultDeliveryFaults,
) : Closeable {

    private val aliasPrefix: String = namespace.aliasPrefixFor(scope)

    private val session = VaultSession(foreground, faults) { VaultOwners.release(path) }

    /** The stored credential, or null when this scope has none. */
    fun read(): VaultOutcome<StoredCredential?> = session.await { load() }

    /** Replaces the record using its existing key; missing or invalidated keys require [reauthorize]. */
    fun replace(credential: StoredCredential): VaultOutcome<Unit> =
        session.await { store(credential) }

    /** Replaces under a fresh key, retaining previous keys until commit. Obtaining credentials is external. */
    fun reauthorize(credential: StoredCredential): VaultOutcome<VaultReauthorization> =
        session.await { renew(credential) }

    /** Deletes this scope's keys before its record; partial failure can leave unreadable ciphertext. */
    fun remove(): VaultOutcome<VaultRemoval> = session.await { erase() }

    fun readAsync(recipient: VaultRecipient<StoredCredential?>) {
        session.submit({ load() }, recipient)
    }

    fun replaceAsync(credential: StoredCredential, recipient: VaultRecipient<Unit>) {
        session.submit({ store(credential) }, recipient)
    }

    fun reauthorizeAsync(
        credential: StoredCredential,
        recipient: VaultRecipient<VaultReauthorization>,
    ) {
        session.submit({ renew(credential) }, recipient)
    }

    fun removeAsync(recipient: VaultRecipient<VaultRemoval>) {
        session.submit({ erase() }, recipient)
    }

    override fun close() {
        session.close()
    }

    /** Waits up to [timeoutMillis] for work accepted before [close] to finish. */
    fun awaitClosed(timeoutMillis: Long): Boolean = session.awaitClosed(timeoutMillis)

    private fun load(): VaultOutcome<StoredCredential?> {
        val record =
            when (val current = current()) {
                is VaultOutcome.Failed -> return current
                is VaultOutcome.Completed -> current.value
            }
        val stored =
            when (val bytes = bytesIn(record)) {
                is VaultOutcome.Failed -> return bytes
                is VaultOutcome.Completed -> bytes.value
            } ?: return VaultOutcome.Completed(null)
        val parsed =
            when (val envelope = VaultEnvelope.parse(stored, scope)) {
                is VaultOutcome.Failed -> return envelope
                is VaultOutcome.Completed -> envelope.value
            }
        val body =
            when (
                val opened =
                    cipher.open(
                        namespace.aliasFor(scope, parsed.generation),
                        parsed.aad,
                        parsed.nonce,
                        parsed.ciphertext,
                    )
            ) {
                is VaultOutcome.Failed -> return opened
                is VaultOutcome.Completed -> opened.value
            }
        return try {
            VaultRecordBody.decode(body, buffers)
        } finally {
            body.fill(0)
        }
    }

    private fun store(credential: StoredCredential): VaultOutcome<Unit> {
        val record =
            when (val current = current()) {
                is VaultOutcome.Failed -> return current
                is VaultOutcome.Completed -> current.value
            }
        val stored =
            when (val bytes = bytesIn(record)) {
                is VaultOutcome.Failed -> return bytes
                is VaultOutcome.Completed -> bytes.value
            }
        if (stored == null) {
            val first = VaultNamespace.FIRST_GENERATION
            return write(record, first, credential, KeySource.Provisioned)
        }
        val parsed =
            when (val envelope = VaultEnvelope.parse(stored, scope)) {
                is VaultOutcome.Failed -> return envelope
                is VaultOutcome.Completed -> envelope.value
            }
        return write(record, parsed.generation, credential, KeySource.Existing)
    }

    private fun renew(credential: StoredCredential): VaultOutcome<VaultReauthorization> {
        val record =
            when (val current = current()) {
                is VaultOutcome.Failed -> return current
                is VaultOutcome.Completed -> current.value
            }
        val stored =
            when (val bytes = bytesIn(record)) {
                is VaultOutcome.Failed -> return bytes
                is VaultOutcome.Completed -> bytes.value
            }
        val previous = stored?.let { VaultEnvelope.generationIn(it) }
        val generation =
            when (val free = available(VaultNamespace.nextGeneration(previous))) {
                is VaultOutcome.Failed -> return free
                is VaultOutcome.Completed -> free.value
            }
        val candidate = namespace.aliasFor(scope, generation)

        when (val written = write(record, generation, credential, KeySource.Provisioned)) {
            is VaultOutcome.Completed -> Unit
            is VaultOutcome.Failed ->
                return VaultOutcome.Failed(
                    if (mayHaveCommitted(written.failure)) {
                        written.failure
                    } else {
                        written.failure.after(discard(candidate))
                    },
                )
        }
        return VaultOutcome.Completed(
            when (val superseded = keys.deleteAll(aliasPrefix, keep = candidate)) {
                is VaultOutcome.Completed ->
                    VaultReauthorization(generation, superseded.value, null)
                is VaultOutcome.Failed ->
                    VaultReauthorization(generation, 0, superseded.failure)
            },
        )
    }

    private fun erase(): VaultOutcome<VaultRemoval> {
        val record =
            when (val current = current()) {
                is VaultOutcome.Failed -> return current
                is VaultOutcome.Completed -> current.value
            }
        val keysRemoved =
            when (val deleted = keys.deleteAll(aliasPrefix)) {
                is VaultOutcome.Failed -> return deleted
                is VaultOutcome.Completed -> deleted.value
            }
        val recordRemoved =
            when (val deleted = record.delete()) {
                is VaultOutcome.Failed -> return deleted
                is VaultOutcome.Completed -> deleted.value
            }
        return VaultOutcome.Completed(VaultRemoval(keysRemoved, recordRemoved))
    }

    /** Resolves again without allowing an operation to leave the leased record path. */
    private fun current(): VaultOutcome<AtomicRecordFile> {
        val resolved =
            when (val outcome = source.resolve()) {
                is VaultOutcome.Failed -> return outcome
                is VaultOutcome.Completed -> outcome.value
            }
        if (resolved.path != path) {
            return VaultOutcome.Failed(
                VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping),
            )
        }
        return VaultOutcome.Completed(resolved.file)
    }

    private fun bytesIn(record: AtomicRecordFile): VaultOutcome<ByteArray?> =
        record.read(VaultEnvelope.MAX_LENGTH)

    private fun write(
        record: AtomicRecordFile,
        generation: Int,
        credential: StoredCredential,
        key: KeySource,
    ): VaultOutcome<Unit> {
        val alias = namespace.aliasFor(scope, generation)
        val body =
            when (val encoded = VaultRecordBody.encode(credential, buffers)) {
                is VaultOutcome.Failed -> return encoded
                is VaultOutcome.Completed -> encoded.value
            }
        val sealed =
            try {
                when (
                    val outcome =
                        cipher.seal(alias, VaultEnvelope.aadFor(scope, generation), body, key)
                ) {
                    is VaultOutcome.Failed -> return outcome
                    is VaultOutcome.Completed -> outcome.value
                }
            } finally {
                body.fill(0)
            }
        val composed =
            when (
                val envelope =
                    VaultEnvelope.compose(scope, generation, sealed.nonce, sealed.ciphertext)
            ) {
                is VaultOutcome.Failed -> return envelope
                is VaultOutcome.Completed -> envelope.value
            }
        return record.replace(composed)
    }

    /** Occupied or invalidated aliases are never cleared to make room for recovery. */
    private fun available(from: Int): VaultOutcome<Int> {
        var generation = from
        for (asked in 1..VaultNamespace.GENERATIONS) {
            when (val held = keys.existing(namespace.aliasFor(scope, generation))) {
                is VaultOutcome.Completed -> Unit
                is VaultOutcome.Failed ->
                    when (held.failure) {
                        VaultFailure.KeyMissing -> return VaultOutcome.Completed(generation)
                        VaultFailure.KeyInvalidated -> Unit
                        else -> return held
                    }
            }
            generation = VaultNamespace.nextGeneration(generation)
        }
        return VaultOutcome.Failed(VaultFailure.VaultAtCapacity(VaultBound.KeyGenerations))
    }

    private fun discard(alias: String): VaultFailure? =
        when (val deleted = keys.delete(alias)) {
            is VaultOutcome.Failed -> deleted.failure
            is VaultOutcome.Completed -> null
        }

    private fun mayHaveCommitted(failure: VaultFailure): Boolean {
        val primary = if (failure is VaultFailure.CleanupAfter) failure.primary else failure
        return primary is VaultFailure.WriteFailed && primary.phase.mayHaveCommitted
    }

    companion object {

        /** Claims one record off the foreground thread, refusing an existing owner. */
        fun open(
            scope: VaultScope,
            namespace: VaultNamespace,
            keys: VaultKeys,
            cipher: VaultCipher,
            source: RecordSource,
            foreground: ForegroundThread,
            faults: VaultDeliveryFaults = VaultDeliveryFaults.Ignored,
        ): VaultOutcome<CredentialVault> =
            opened(scope, namespace, keys, cipher, source, foreground, faults, VaultBuffers.Direct)

        internal fun opened(
            scope: VaultScope,
            namespace: VaultNamespace,
            keys: VaultKeys,
            cipher: VaultCipher,
            source: RecordSource,
            foreground: ForegroundThread,
            faults: VaultDeliveryFaults,
            buffers: VaultBuffers,
        ): VaultOutcome<CredentialVault> {
            if (foreground.isCurrent()) {
                return VaultOutcome.Failed(VaultFailure.ForegroundRefused)
            }
            val resolved =
                when (val outcome = source.resolve()) {
                    is VaultOutcome.Failed -> return outcome
                    is VaultOutcome.Completed -> outcome.value
                }
            VaultOwners.claim(resolved.path)?.let { return VaultOutcome.Failed(it) }
            return VaultOutcome.Completed(
                CredentialVault(
                    scope = scope,
                    namespace = namespace,
                    keys = keys,
                    cipher = cipher,
                    source = source,
                    path = resolved.path,
                    buffers = buffers,
                    foreground = foreground,
                    faults = faults,
                ),
            )
        }
    }
}

data class VaultRemoval(val keysRemoved: Int, val recordRemoved: Boolean)

/** The replacement committed even if superseded-key [cleanup] failed. */
data class VaultReauthorization(
    val generation: Int,
    val supersededKeys: Int,
    val cleanup: VaultFailure?,
)
