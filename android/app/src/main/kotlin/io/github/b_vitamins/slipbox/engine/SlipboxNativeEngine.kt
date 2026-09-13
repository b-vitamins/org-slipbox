/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import android.os.Looper
import java.io.File

/**
 * The load seam of the packaged native engine.
 *
 * The library exposes one entry point, a self-contained fixture probe that
 * qualifies the packaged engine and its bundled SQLite on a device. Reading
 * notes and maintaining an index are not offered here.
 */
object SlipboxNativeEngine {

    /** Cargo names the shared library after the crate, with dashes replaced. */
    const val LIBRARY_NAME: String = "slipbox_android"

    /**
     * The dynamic linker's message, or null once the library is loaded.
     *
     * Loading is attempted once, on first use of this object, and a missing or
     * unusable library is reported rather than thrown from an initializer.
     */
    val loadFailure: String? =
        try {
            System.loadLibrary(LIBRARY_NAME)
            null
        } catch (error: UnsatisfiedLinkError) {
            error.message ?: error.toString()
        }

    fun libraryEntry(abi: String): String = "lib/$abi/lib$LIBRARY_NAME.so"

    /**
     * Runs the fixture probe under [parentDirectory] and returns its JSON report.
     *
     * [parentDirectory] must be an existing application-private directory: the
     * probe claims one fresh subdirectory of it, never an existing path, and
     * writes a synthetic corpus and database there. A run that claims that
     * subdirectory attempts to remove it before returning and reports what the
     * removal returned as its `cleanup.workspace_removed` check, on the failing
     * paths as well as the passing one. A run refused before that claim owns
     * nothing and reports no removal, and neither does one that panicked.
     * Indexing is not main thread work, so a main thread caller is refused.
     */
    fun runFixtureProbe(parentDirectory: File): String {
        check(loadFailure == null) { "the packaged engine did not load: $loadFailure" }
        check(Looper.myLooper() !== Looper.getMainLooper()) {
            "the fixture probe indexes a corpus and must not run on the main thread"
        }
        require(parentDirectory.isDirectory) {
            "the probe parent must be an existing directory, was $parentDirectory"
        }
        val path = parentDirectory.absolutePath.toByteArray(Charsets.UTF_8)
        val report = checkNotNull(nativeRunFixtureProbe(path)) {
            "the packaged engine could not allocate its report"
        }
        return report.toString(Charsets.UTF_8)
    }

    private external fun nativeRunFixtureProbe(parentDirectory: ByteArray): ByteArray?
}
