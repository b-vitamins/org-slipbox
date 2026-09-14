/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.io.ByteArrayOutputStream
import java.io.File

/** Same-directory staged replacement with explicit commit/cleanup outcomes; no power-loss guarantee. */
class AtomicVaultRecordFile(
    private val target: File,
    private val io: RecordIo = SystemRecordIo,
    private val entries: RecordEntries = RecordEntries.named(io),
) : AtomicRecordFile {

    private val directory: File? = target.parentFile

    private val staging = File(target.path + STAGING_SUFFIX)

    private val legacy = File(target.path + LEGACY_SUFFIX)

    override fun read(limit: Int): VaultOutcome<ByteArray?> {
        when (val held = entries.contained(target)) {
            is VaultOutcome.Failed -> return held
            is VaultOutcome.Completed -> Unit
        }
        when (val there = presenceOf(target)) {
            Presence.Absent -> return VaultOutcome.Completed(null)
            is Presence.Unknown -> return VaultOutcome.Failed(VaultFailure.ReadFailed(there.origin))
            Presence.Present -> Unit
        }
        when (val kind = observing { io.isDirectory(target) }) {
            is Observed.Faulted -> return VaultOutcome.Failed(VaultFailure.ReadFailed(kind.origin))
            is Observed.Answered ->
                if (kind.value) {
                    return VaultOutcome.Failed(VaultFailure.ReadFailed(NOT_A_FILE))
                }
        }
        val stream =
            when (val opened = observing { io.read(target) }) {
                is Observed.Faulted ->
                    return VaultOutcome.Failed(VaultFailure.ReadFailed(opened.origin))
                is Observed.Answered -> opened.value
            }
        return try {
            stream.use { reading ->
                val collected = ByteArrayOutputStream()
                val chunk = ByteArray(CHUNK)
                while (true) {
                    val read = reading.read(chunk)
                    if (read < 0) {
                        break
                    }
                    collected.write(chunk, 0, read)
                    if (collected.size() > limit) {
                        return VaultOutcome.Failed(
                            VaultFailure.CorruptRecord(RecordDefect.Oversized),
                        )
                    }
                }
                VaultOutcome.Completed(collected.toByteArray())
            }
        } catch (error: Exception) {
            VaultOutcome.Failed(VaultFailure.ReadFailed(originOf(error)))
        }
    }

    override fun replace(record: ByteArray): VaultOutcome<Unit> {
        if (directory != null) {
            when (val made = observing { io.createDirectory(directory) }) {
                is Observed.Faulted -> return notStarted(made.origin)
                is Observed.Answered ->
                    if (!made.value) {
                        return notStarted(UNAVAILABLE_DIRECTORY)
                    }
            }
        }
        when (val kind = observing { io.isDirectory(target) }) {
            is Observed.Faulted -> return notStarted(kind.origin)
            is Observed.Answered ->
                if (kind.value) {
                    return notStarted(NOT_A_FILE)
                }
        }
        for (entry in listOf(target, staging)) {
            when (val held = entries.contained(entry)) {
                is VaultOutcome.Failed -> return held
                is VaultOutcome.Completed -> Unit
            }
        }
        val sink =
            when (val opened = observing { io.open(staging) }) {
                is Observed.Faulted -> return notStarted(opened.origin)
                is Observed.Answered -> opened.value
            }

        try {
            sink.write(record)
            sink.sync()
            sink.close()
        } catch (error: Exception) {
            return VaultOutcome.Failed(
                VaultFailure
                    .WriteFailed(CommitPhase.BeforeCommit, originOf(error))
                    .after(abandon(sink)),
            )
        }

        when (val renamed = observing { io.rename(staging, target) }) {
            is Observed.Faulted ->
                return VaultOutcome.Failed(
                    VaultFailure
                        .WriteFailed(CommitPhase.Unknown, renamed.origin)
                        .after(abandon(null)),
                )
            is Observed.Answered ->
                if (!renamed.value) {
                    return VaultOutcome.Failed(
                        VaultFailure
                            .WriteFailed(CommitPhase.BeforeCommit, UNRENAMED_RECORD)
                            .after(abandon(null)),
                    )
                }
        }
        // Later observation faults cannot undo an established commit.
        return when (val left = presenceOf(staging)) {
            Presence.Absent -> VaultOutcome.Completed(Unit)
            Presence.Present ->
                VaultOutcome.Failed(
                    VaultFailure
                        .WriteFailed(CommitPhase.Committed, SURVIVING_STAGING)
                        .after(abandon(null)),
                )
            is Presence.Unknown ->
                VaultOutcome.Failed(VaultFailure.WriteFailed(CommitPhase.Committed, left.origin))
        }
    }

    override fun delete(): VaultOutcome<Boolean> {
        var existed = false
        var removed = 0
        for (owned in listOf(target, staging, legacy)) {
            when (val there = presenceOf(owned)) {
                Presence.Absent -> continue
                is Presence.Unknown -> return undeleted(there.origin, removed)
                Presence.Present -> Unit
            }
            when (val gone = observing { io.remove(owned) }) {
                is Observed.Faulted -> return undeleted(gone.origin, removed)
                is Observed.Answered ->
                    if (!gone.value && presenceOf(owned) != Presence.Absent) {
                        return undeleted(UNDELETED_RECORD, removed)
                    }
            }
            removed++
            existed = existed || owned == target
        }
        return VaultOutcome.Completed(existed)
    }

    private fun abandon(sink: RecordSink?): VaultFailure? {
        var cleanup: VaultFailure? = null
        if (sink != null) {
            try {
                sink.close()
            } catch (error: Exception) {
                cleanup = VaultFailure.CleanupFailed(originOf(error))
            }
        }
        when (val gone = observing { io.remove(staging) }) {
            is Observed.Faulted -> return cleanup ?: VaultFailure.CleanupFailed(gone.origin)
            is Observed.Answered ->
                if (!gone.value && presenceOf(staging) != Presence.Absent) {
                    return cleanup ?: VaultFailure.CleanupFailed(UNDELETED_STAGING)
                }
        }
        return cleanup
    }

    private fun presenceOf(file: File): Presence {
        val parent = directory ?: return standalone(file)
        return io.presenceOf(parent, file.name)
    }

    private fun standalone(file: File): Presence =
        when (val there = observing { io.exists(file) }) {
            is Observed.Faulted -> Presence.Unknown(there.origin)
            is Observed.Answered -> if (there.value) Presence.Present else Presence.Absent
        }

    private fun notStarted(origin: String): VaultOutcome.Failed =
        VaultOutcome.Failed(VaultFailure.WriteFailed(CommitPhase.NotStarted, origin))

    private fun undeleted(origin: String, removed: Int): VaultOutcome.Failed =
        VaultOutcome.Failed(VaultFailure.RemovalFailed(RemovalStage.Record, origin, removed))

    private companion object {

        const val CHUNK = 4096

        const val STAGING_SUFFIX = ".new"

        /** The sidecar the platform's own atomic file used to keep beside a record. */
        const val LEGACY_SUFFIX = ".bak"

        const val UNAVAILABLE_DIRECTORY = "unavailable directory"

        const val NOT_A_FILE = "record path is not a file"

        const val UNRENAMED_RECORD = "unrenamed staging record"

        const val SURVIVING_STAGING = "surviving staging record"

        const val UNDELETED_STAGING = "undeleted staging record"

        const val UNDELETED_RECORD = "undeleted record"
    }
}
