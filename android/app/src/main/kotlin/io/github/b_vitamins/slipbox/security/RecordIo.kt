/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.InputStream

/** One record file open for writing, closed by whoever opened it. */
interface RecordSink {

    fun write(record: ByteArray)

    /** Flushes what was written to the storage the file lives on. */
    fun sync()

    fun close()
}

/** Platform IO seam; the record protocol classifies returned states and exceptions. */
interface RecordIo {

    /** The names in [directory], or null when it could not be listed. */
    fun entries(directory: File): Array<String>?

    /** Whether [directory] is a directory, creating it and its parents if absent. */
    fun createDirectory(directory: File): Boolean

    /** Opens [file] for writing from empty, creating it if absent. */
    fun open(file: File): RecordSink

    fun read(file: File): InputStream

    /** Renames [from] to [to] within one directory, reporting whether it did. */
    fun rename(from: File, to: File): Boolean

    fun remove(file: File): Boolean

    /** Whether [file] resolves to something that is there. False for a dead link. */
    fun exists(file: File): Boolean

    fun isDirectory(file: File): Boolean

    /** [file] with every link and relative step resolved. */
    fun canonicalPath(file: File): String
}

/** A directory that could not be listed, so what it holds is not known. */
internal const val UNLISTABLE_DIRECTORY: String = "unlistable directory"

/** What a directory listing says about one name in it. */
internal sealed class Presence {

    /** The directory holds the name. */
    object Present : Presence()

    /** The directory is there and does not hold the name. */
    object Absent : Presence()

    /** Neither, because [origin] stopped the listing that would have said. */
    data class Unknown(val origin: String) : Presence()
}

/** What one file-system call answered, or the type of the fault that stopped it. */
internal sealed class Observed<out T> {

    /** The call returned [value]. */
    class Answered<out T>(val value: T) : Observed<T>()

    /** The call threw a fault of type [origin]. */
    class Faulted(val origin: String) : Observed<Nothing>()
}

/** Retains exception types only: platform messages can contain private paths. */
internal inline fun <T> observing(work: () -> T): Observed<T> =
    try {
        Observed.Answered(work())
    } catch (error: Exception) {
        Observed.Faulted(originOf(error))
    }

/** Unreadable directories are unknown, not absent. */
internal fun RecordIo.presenceOf(directory: File, name: String): Presence =
    presenceIn(directory, name, PRESENCE_DEPTH)

/** Consult readable ancestors to distinguish missing directories from denied access. */
private fun RecordIo.presenceIn(directory: File, name: String, depth: Int): Presence {
    val names =
        when (val listed = observing { entries(directory) }) {
            is Observed.Faulted -> return Presence.Unknown(listed.origin)
            is Observed.Answered -> listed.value
        }
    if (names != null) {
        return if (names.contains(name)) Presence.Present else Presence.Absent
    }
    val parent = directory.parentFile
    if (parent == null || depth <= 0) {
        return Presence.Unknown(UNLISTABLE_DIRECTORY)
    }
    return when (val itself = presenceIn(parent, directory.name, depth - 1)) {
        Presence.Absent -> Presence.Absent
        is Presence.Unknown -> itself
        Presence.Present -> Presence.Unknown(UNLISTABLE_DIRECTORY)
    }
}

/** How far above a directory its own presence is asked about, bounding the walk. */
private const val PRESENCE_DEPTH = 64

/** The file system this app runs on. */
object SystemRecordIo : RecordIo {

    override fun entries(directory: File): Array<String>? = directory.list()

    override fun createDirectory(directory: File): Boolean =
        directory.isDirectory || directory.mkdirs()

    override fun open(file: File): RecordSink = FileRecordSink(FileOutputStream(file))

    override fun read(file: File): InputStream = FileInputStream(file)

    override fun rename(from: File, to: File): Boolean = from.renameTo(to)

    override fun remove(file: File): Boolean = file.delete()

    override fun exists(file: File): Boolean = file.exists()

    override fun isDirectory(file: File): Boolean = file.isDirectory

    override fun canonicalPath(file: File): String = file.canonicalPath
}

private class FileRecordSink(private val stream: FileOutputStream) : RecordSink {

    override fun write(record: ByteArray) {
        stream.write(record)
    }

    override fun sync() {
        stream.flush()
        stream.fd.sync()
    }

    override fun close() {
        stream.close()
    }
}
