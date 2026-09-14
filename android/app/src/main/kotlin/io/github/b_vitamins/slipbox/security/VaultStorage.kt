/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.io.File

/** Whole-record storage with explicit replacement state. Access requires one serialized owner. */
interface AtomicRecordFile {

    /** Returns null only for established absence; rejects records larger than [limit]. */
    fun read(limit: Int): VaultOutcome<ByteArray?>

    /** Replacement failures retain commit phase and any abandonment failure. */
    fun replace(record: ByteArray): VaultOutcome<Unit>

    /** Deletes the stored record and reports whether one was there. */
    fun delete(): VaultOutcome<Boolean>
}

/** Entry validation at use; the private policy additionally checks the trusted root. */
fun interface RecordEntries {

    /** [entry] itself, or why it is not the entry it names. */
    fun contained(entry: File): VaultOutcome<File>

    companion object {

        /** Holds each entry to the name its own directory gives it, over [io]. */
        fun named(io: RecordIo = SystemRecordIo): RecordEntries = NamedEntries(io)
    }
}

private class NamedEntries(private val io: RecordIo) : RecordEntries {

    override fun contained(entry: File): VaultOutcome<File> {
        val parent = entry.parentFile ?: return VaultOutcome.Completed(entry)
        val expected =
            when (val resolved = observing { io.canonicalPath(parent) }) {
                is Observed.Faulted -> return unread(resolved.origin)
                is Observed.Answered -> resolved.value + File.separator + entry.name
            }
        val itself =
            when (val resolved = observing { io.canonicalPath(entry) }) {
                is Observed.Faulted -> return unread(resolved.origin)
                is Observed.Answered -> resolved.value
            }
        if (itself != expected) {
            return VaultOutcome.Failed(
                VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping),
            )
        }
        io.refuseLinkToNowhere(entry)?.let { return VaultOutcome.Failed(it) }
        return VaultOutcome.Completed(entry)
    }

    private fun unread(origin: String): VaultOutcome.Failed =
        VaultOutcome.Failed(VaultFailure.ReadFailed(origin))
}

/** Canonical paths alone do not reveal dangling links; compare listed names with resolution. */
internal fun RecordIo.refuseLinkToNowhere(entry: File): VaultFailure? {
    val parent = entry.parentFile ?: return null
    when (val there = presenceOf(parent, entry.name)) {
        Presence.Absent -> return null
        is Presence.Unknown -> return VaultFailure.ReadFailed(there.origin)
        Presence.Present -> Unit
    }
    return when (val resolved = observing { exists(entry) }) {
        is Observed.Faulted -> VaultFailure.ReadFailed(resolved.origin)
        is Observed.Answered ->
            if (resolved.value) {
                null
            } else {
                VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping)
            }
    }
}
