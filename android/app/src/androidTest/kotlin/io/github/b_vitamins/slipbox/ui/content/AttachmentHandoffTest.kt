/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class AttachmentHandoffTest {

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private lateinit var root: File

    @Before
    fun setUp() {
        root = File(context.cacheDir, "attachments")
        root.deleteRecursively()
        root.mkdirs()
    }

    @After
    fun tearDown() {
        root.deleteRecursively()
    }

    @Test
    fun oneStagedFileReceivesOneReadOnlyContentGrant() {
        val bytes = "Offline attachment.\n".toByteArray()
        val directory = File(root, UUID.randomUUID().toString()).apply { mkdir() }
        val file = File(directory, "context.txt").apply { writeBytes(bytes) }

        val intent =
            checkNotNull(
                SystemAttachmentLauncher(context).intentFor(
                    StagedAttachment(file, "context.txt", "text/plain"),
                ),
            )

        assertEquals(Intent.ACTION_VIEW, intent.action)
        assertEquals("text/plain", intent.type)
        assertEquals("content", intent.data?.scheme)
        assertEquals("${context.packageName}.attachments", intent.data?.authority)
        assertTrue(intent.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION != 0)
        assertEquals(1, intent.clipData?.itemCount)
        assertEquals(intent.data, intent.clipData?.getItemAt(0)?.uri)
        assertFalse(intent.data.toString().contains(context.filesDir.parentFile?.path.orEmpty()))
        val received = context.contentResolver.openInputStream(checkNotNull(intent.data)).use {
            checkNotNull(it).readBytes()
        }
        assertArrayEquals(bytes, received)
    }

    @Test
    fun theProviderIsPrivateAndFilesOutsideItsNarrowCacheRootAreUnaddressable() {
        val authority = "${context.packageName}.attachments"
        val provider =
            if (Build.VERSION.SDK_INT >= 33) {
                context.packageManager.resolveContentProvider(
                    authority,
                    PackageManager.ComponentInfoFlags.of(PackageManager.GET_META_DATA.toLong()),
                )
            } else {
                @Suppress("DEPRECATION")
                context.packageManager.resolveContentProvider(authority, PackageManager.GET_META_DATA)
            }
        assertEquals(false, provider?.exported)
        assertEquals(true, provider?.grantUriPermissions)

        val outside = File(context.cacheDir, "not-an-attachment.txt").apply { writeText("private") }
        try {
            assertNull(
                SystemAttachmentLauncher(context).intentFor(
                    StagedAttachment(outside, "not-an-attachment.txt", "text/plain"),
                ),
            )
        } finally {
            outside.delete()
        }
    }
}
