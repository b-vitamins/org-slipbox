/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.settings

import androidx.compose.runtime.Immutable
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import java.io.File
import java.io.FileInputStream
import java.io.FileNotFoundException
import java.io.IOException
import java.io.InputStream
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction

/** Device-local preferences. A missing record uses these defaults. */
@Immutable
internal data class ReadingPreferences(
    val appearance: SlipboxAppearance = SlipboxAppearance.System,
    val reduceMotion: Boolean = false,
)

internal enum class ReadingPreferenceFault {
    Unreadable,
    Oversized,
    Malformed,
    UnsupportedVersion,
    Unwritable,
}

/** An unusable record is reported and left untouched. */
internal sealed interface ReadingPreferencesRecord {
    data object Absent : ReadingPreferencesRecord

    data class Stored(val preferences: ReadingPreferences) : ReadingPreferencesRecord

    data class Unusable(val fault: ReadingPreferenceFault) : ReadingPreferencesRecord
}

internal interface ReadingPreferencesStore {
    fun read(): ReadingPreferencesRecord

    /** Records [preferences], returning the fault that stopped it or null on success. */
    fun write(preferences: ReadingPreferences): ReadingPreferenceFault?
}

/** Bounded preferences file, replaced through a temporary file on write. */
internal class ReadingPreferencesFile(private val file: File) : ReadingPreferencesStore {

    override fun read(): ReadingPreferencesRecord {
        val stream =
            try {
                FileInputStream(file)
            } catch (_: FileNotFoundException) {
                return absentOrUnreadable()
            } catch (_: SecurityException) {
                return unreadable()
            }
        val bytes =
            try {
                stream.use { readBounded(it) }
            } catch (_: IOException) {
                return unreadable()
            } catch (_: SecurityException) {
                return unreadable()
            }
        if (bytes == null) {
            return ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.Oversized)
        }
        return decode(bytes)
    }

    override fun write(preferences: ReadingPreferences): ReadingPreferenceFault? {
        val pending = File(file.parentFile, "${file.name}$PENDING_SUFFIX")
        val fault =
            try {
                file.parentFile?.mkdirs()
                pending.writeText(render(preferences))
                if (pending.renameTo(file)) null else ReadingPreferenceFault.Unwritable
            } catch (_: IOException) {
                ReadingPreferenceFault.Unwritable
            } catch (_: SecurityException) {
                ReadingPreferenceFault.Unwritable
            }
        if (fault != null) discard(pending)
        return fault
    }

    /** Opening failure alone does not establish absence; check the containing directory. */
    private fun absentOrUnreadable(): ReadingPreferencesRecord =
        try {
            val parent = file.parentFile ?: return unreadable()
            val entries = parent.list()
            when {
                entries == null -> if (parent.exists()) unreadable() else absent()
                file.name in entries -> unreadable()
                else -> absent()
            }
        } catch (_: SecurityException) {
            unreadable()
        }

    /** Enforce the limit on bytes read, not a potentially stale file length. */
    private fun readBounded(stream: InputStream): ByteArray? {
        val buffer = ByteArray(MAX_BYTES + 1)
        var filled = 0
        while (filled < buffer.size) {
            val read = stream.read(buffer, filled, buffer.size - filled)
            if (read < 0) break
            filled += read
        }
        return if (filled > MAX_BYTES) null else buffer.copyOf(filled)
    }

    private fun decode(bytes: ByteArray): ReadingPreferencesRecord {
        val text =
            try {
                Charsets.UTF_8
                    .newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(bytes))
                    .toString()
            } catch (_: CharacterCodingException) {
                return malformed()
            }
        return parse(text)
    }

    private fun discard(pending: File) {
        try {
            pending.delete()
        } catch (_: SecurityException) {
            // Preserve the write failure if cleanup is also denied.
        }
    }

    private fun render(preferences: ReadingPreferences): String =
        buildString {
            append(MAGIC).append(' ').append(VERSION).append('\n')
            append(APPEARANCE).append('=').append(preferences.appearance.name).append('\n')
            append(REDUCE_MOTION).append('=').append(preferences.reduceMotion).append('\n')
        }

    /** Check version before field shape so newer formats report UnsupportedVersion. */
    private fun parse(text: String): ReadingPreferencesRecord {
        if (!text.endsWith('\n')) return malformed()
        val lines = text.dropLast(1).split('\n')
        val header = lines.first().split(' ')
        if (header.size != 2 || header[0] != MAGIC) return malformed()
        val version = header[1].toIntOrNull() ?: return malformed()
        if (version != VERSION) {
            return ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.UnsupportedVersion)
        }
        if (lines.size != 1 + FIELDS) return malformed()

        var appearance: SlipboxAppearance? = null
        var reduceMotion: Boolean? = null
        for (line in lines.drop(1)) {
            val separator = line.indexOf('=')
            if (separator <= 0) return malformed()
            val value = line.substring(separator + 1)
            when (line.substring(0, separator)) {
                APPEARANCE -> {
                    if (appearance != null) return malformed()
                    appearance =
                        SlipboxAppearance.entries.firstOrNull { it.name == value }
                            ?: return malformed()
                }
                REDUCE_MOTION -> {
                    if (reduceMotion != null) return malformed()
                    reduceMotion = value.toBooleanStrictOrNull() ?: return malformed()
                }
                else -> return malformed()
            }
        }
        if (appearance == null || reduceMotion == null) return malformed()
        return ReadingPreferencesRecord.Stored(ReadingPreferences(appearance, reduceMotion))
    }

    private fun absent() = ReadingPreferencesRecord.Absent

    private fun unreadable() =
        ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.Unreadable)

    private fun malformed() = ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.Malformed)

    internal companion object {
        const val FILE_NAME = "reading-preferences"
        private const val MAGIC = "slipbox.reading"
        private const val VERSION = 1
        private const val APPEARANCE = "appearance"
        private const val REDUCE_MOTION = "reduce-motion"
        private const val PENDING_SUFFIX = ".pending"

        private const val FIELDS = 2

        private const val MAX_BYTES = 512
    }
}
