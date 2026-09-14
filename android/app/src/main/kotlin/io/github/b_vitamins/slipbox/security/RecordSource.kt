/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

/** One record file and the path it resolved to. */
class ResolvedRecord(val path: String, val file: AtomicRecordFile)

/** Re-resolved before each vault operation; a changed path cannot replace its lease. */
interface RecordSource {

    /** The record as it resolves now, or why it is not available. */
    fun resolve(): VaultOutcome<ResolvedRecord>
}

/** Canonical record identity plus trusted-root checks for every opened entry. */
class PolicyRecordSource(
    private val policy: PrivateStoragePolicy,
    private val scope: VaultScope,
    private val io: RecordIo = SystemRecordIo,
) : RecordSource {

    override fun resolve(): VaultOutcome<ResolvedRecord> {
        val record =
            when (val outcome = policy.recordFor(scope)) {
                is VaultOutcome.Failed -> return outcome
                is VaultOutcome.Completed -> outcome.value
            }
        return try {
            VaultOutcome.Completed(
                ResolvedRecord(
                    io.canonicalPath(record),
                    AtomicVaultRecordFile(record, io, policy.entries),
                ),
            )
        } catch (error: Exception) {
            VaultOutcome.Failed(VaultFailure.ReadFailed(originOf(error)))
        }
    }
}

/** Caller-resolved storage; callers must provide one canonical identity per record. */
class FixedRecordSource(private val path: String, private val file: AtomicRecordFile) :
    RecordSource {

    override fun resolve(): VaultOutcome<ResolvedRecord> =
        VaultOutcome.Completed(ResolvedRecord(path, file))
}
