/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import android.os.Build
import android.system.Os
import android.system.OsConstants
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.BuildConfig
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.io.InputStream
import java.security.MessageDigest
import java.util.Locale
import java.util.zip.ZipEntry
import java.util.zip.ZipFile

@RunWith(AndroidJUnit4::class)
class NativeEngineProbeTest {

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext
    private val runningAbi = Build.SUPPORTED_ABIS.first()

    @Test
    fun thePackagedLibraryLoadsOnTheRunningKernelAndAbi() {
        assertNull(SlipboxNativeEngine.loadFailure)

        assertTrue(
            "$runningAbi is not qualified: ${BuildConfig.QUALIFIED_ABIS}",
            runningAbi in BuildConfig.QUALIFIED_ABIS.split(","),
        )
        // A library aligned for 16 KB pages must also load where pages are smaller.
        val pageSize = Os.sysconf(OsConstants._SC_PAGESIZE)
        Log.i(TAG, "kernel page size $pageSize, abi $runningAbi")
        assertTrue("unqualified kernel page size $pageSize", pageSize == 4096L || pageSize == 16384L)
    }

    @Test
    fun theEngineIndexesQueriesClosesAndReopensOneDatabase() {
        val parent = File(context.noBackupFilesDir, "fixture-probe")
        assertTrue("$parent is not available", parent.isDirectory || parent.mkdirs())

        val encoded = SlipboxNativeEngine.runFixtureProbe(parent)
        val report = JSONObject(encoded)
        val checks = report.getJSONArray("checks")
        val actual =
            (0 until checks.length()).associate { position ->
                val check = checks.getJSONObject(position)
                check.getString("name") to check.getString("actual")
            }
        val failed =
            (0 until checks.length())
                .map { checks.getJSONObject(it) }
                .filter { !it.getBoolean("passed") }
                .map { it.getString("name") }

        assertTrue(encoded, report.isNull("failure"))
        assertEquals(encoded, emptyList<String>(), failed)
        assertTrue(encoded, report.getBoolean("passed"))
        assertEquals(encoded, EXPECTED_CHECKS.toLong(), checks.length().toLong())

        assertEquals("3", actual["index.files"])
        assertEquals("5", actual["index.nodes"])
        assertEquals("1", actual["index.links"])
        assertEquals(FIXTURE_FILES, actual["indexed.notes.files"])
        assertEquals("[\"Riemann integral\"]", actual["indexed.glossary.terms"])

        // The reopened service answers from the durable file the first one wrote.
        assertEquals(FIXTURE_FILES, actual["reopened.notes.files"])
        assertEquals("[\"Target heading\"]", actual["reopened.notes.search"])
        assertEquals("[\"Riemann integral\"]", actual["reopened.glossary.terms"])
        assertEquals("true", actual["close.database_outlives_service"])

        // A mobile build authorizes no decryptor, and a second database over the
        // same root answers nothing.
        assertEquals("-32600", actual["authority.refusal_code"])
        assertEquals("[]", actual["control.notes.files"])

        assertEquals("Ok(())", actual["cleanup.workspace_removed"])
        assertEquals(emptyList<String>(), parent.list().orEmpty().toList())
        assertTrue("$parent survived the probe", parent.delete())
    }

    @Test
    fun theMainThreadIsRefusedTheProbeItWouldBlockOn() {
        var refusal: Throwable? = null
        instrumentation.runOnMainSync {
            refusal = runCatching { SlipboxNativeEngine.runFixtureProbe(context.noBackupFilesDir) }
                .exceptionOrNull()
        }

        val thrown = refusal
        assertTrue("the main thread was not refused: $thrown", thrown is IllegalStateException)
        assertTrue(
            thrown?.message.orEmpty(),
            thrown?.message.orEmpty().contains("main thread"),
        )
    }

    @Test
    fun theLoadedLibraryIsThePackagedEntryOfThisApk() {
        val apkPath = context.applicationInfo.sourceDir
        val entryName = SlipboxNativeEngine.libraryEntry(runningAbi)

        val digest =
            ZipFile(apkPath).use { apk ->
                val entry = requireNotNull(apk.getEntry(entryName)) { "$apkPath carries no $entryName" }
                // A stored entry is mapped out of the APK, so no extracted copy
                // can diverge from the bytes a release ships.
                assertEquals(entryName, ZipEntry.STORED.toLong(), entry.method.toLong())
                apk.getInputStream(entry).use { sha256(it) }
            }
        Log.i(TAG, "$entryName sha256 $digest")

        val mapped = File("/proc/self/maps").readLines().filter { it.contains(apkPath) }
        assertTrue("no mapping of $apkPath in this process", mapped.isNotEmpty())
        assertNull(SlipboxNativeEngine.loadFailure)
        assertEquals(digest, 64L, digest.length.toLong())
    }

    private fun sha256(stream: InputStream): String {
        val digest = MessageDigest.getInstance("SHA-256")
        val buffer = ByteArray(1 shl 16)
        while (true) {
            val read = stream.read(buffer)
            if (read < 0) {
                break
            }
            digest.update(buffer, 0, read)
        }
        return digest.digest().joinToString("") { "%02x".format(Locale.ROOT, it) }
    }

    private companion object {
        const val TAG = "SlipboxNativeEngine"

        const val EXPECTED_CHECKS = 35

        const val FIXTURE_FILES = "[\"alpha.org\", \"beta.org\", \"riemann.org\"]"
    }
}
