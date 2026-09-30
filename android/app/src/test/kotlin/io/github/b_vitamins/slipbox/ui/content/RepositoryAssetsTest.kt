/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.annotation.SuppressLint
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import java.io.File
import java.io.RandomAccessFile
import java.nio.file.Files
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class RepositoryAssetsTest {

    private lateinit var workspace: File
    private lateinit var content: File
    private lateinit var staging: File
    private lateinit var launcher: RecordingLauncher
    private lateinit var assets: RepositoryAssets

    @Before
    fun setUp() {
        workspace =
            File(System.getProperty("java.io.tmpdir"), "slipbox-assets-${UUID.randomUUID()}")
                .apply { check(mkdirs()) }
        content = File(workspace, "content").apply { mkdirs() }
        staging = File(workspace, "cache/attachments")
        File(content, "notes/chapter").mkdirs()
        File(content, "notes/chapter/note.org").writeText("A note.\n")
        launcher = RecordingLauncher()
        assets =
            RepositoryAssets(
                contentRoot = content,
                expected = GenerationBinding(SOURCE, GENERATION),
                stagingRoot = staging,
                launcher = launcher,
            )
    }

    @After
    fun tearDown() {
        workspace.deleteRecursively()
    }

    @Test
    fun aRelativeImageOutsideTheNotesFolderIsReadFromTheBoundGeneration() {
        val bytes = png("offline image")
        write("assets/diagram.png", bytes)

        val resolved = assets.resolve(binding(), "file:../../assets/diagram.png")

        assertEquals("image/png", resolved?.mimeType)
        assertArrayEquals(bytes, resolved?.bytes)
    }

    @Test
    fun anotherGenerationAnEscapeAndASymlinkCannotCrossTheResolver() {
        write("assets/diagram.png", png("held"))
        symbolicLink(
            link = File(content, "assets/linked.png"),
            target = File(content, "assets/diagram.png"),
        )

        assertNull(
            assets.resolve(
                binding().copy(generation = "generation-elsewhere"),
                "file:../../assets/diagram.png",
            ),
        )
        assertNull(assets.resolve(binding(), "file:../../../outside.png"))
        assertNull(assets.resolve(binding(), "file:../../assets/linked.png"))
    }

    @Test
    fun missingUnsupportedDisguisedAndOversizedImagesAreNeverServed() {
        write("assets/page.png", "<script>run()</script>".toByteArray())
        val large = File(content, "assets/large.png")
        large.parentFile?.mkdirs()
        RandomAccessFile(large, "rw").use { it.setLength(MAX_INLINE_ASSET_BYTES + 1) }

        assertNull(assets.resolve(binding(), "file:../../assets/missing.png"))
        assertNull(assets.resolve(binding(), "file:../../assets/diagram.svg"))
        assertNull(assets.resolve(binding(), "file:../../assets/page.png"))
        assertNull(assets.resolve(binding(), "file:../../assets/large.png"))
    }

    @Test
    fun anAttachmentIsCopiedToAnOpaqueCacheLocationBeforeItsReadOnlyHandoff() {
        val text = "A repository attachment.\n".toByteArray()
        write("assets/context.txt", text)

        val result = assets.open(binding(), "file:../../assets/context.txt")

        assertEquals(AttachmentOpenResult.Opened, result)
        val opened = checkNotNull(launcher.opened)
        assertEquals("context.txt", opened.displayName)
        assertEquals("text/plain", opened.mimeType)
        assertArrayEquals(text, opened.file.readBytes())
        assertEquals(staging.canonicalFile, opened.file.parentFile?.parentFile?.canonicalFile)
        assertFalse(opened.file.path.contains(content.path))
    }

    @Test
    fun attachmentFailuresRetainUsefulNamesWithoutExposingSourcePaths() {
        write("assets/diagram.pdf", "%PDF".toByteArray())
        write("assets/diagram.png", png("image"))
        val large = File(content, "assets/large.txt")
        RandomAccessFile(large, "rw").use { it.setLength(MAX_ATTACHMENT_BYTES + 1) }
        launcher.accept = false

        assertEquals(
            AttachmentOpenResult.Missing("missing.txt"),
            assets.open(binding(), "file:../../assets/missing.txt"),
        )
        assertEquals(
            AttachmentOpenResult.Unsupported("diagram.pdf"),
            assets.open(binding(), "file:../../assets/diagram.pdf"),
        )
        assertEquals(
            AttachmentOpenResult.Oversized("large.txt", MAX_ATTACHMENT_BYTES),
            assets.open(binding(), "file:../../assets/large.txt"),
        )
        assertEquals(
            AttachmentOpenResult.ViewerUnavailable("diagram.png"),
            assets.open(binding(), "file:../../assets/diagram.png"),
        )
    }

    @Test
    fun onlyNonOrgRepositoryTargetsEnterTheAssetFlow() {
        for (target in listOf("file:diagram.png", "../assets/context.txt", "file:manual.pdf")) {
            assertTrue(target, isRepositoryAssetTarget(target))
        }
        for (target in
            listOf(
                "file:next.org",
                "file:next.org::*Heading",
                "next.org",
                "id:abc",
                "https://example.org/image.png",
                "#custom",
                "*Heading",
            )
        ) {
            assertFalse(target, isRepositoryAssetTarget(target))
        }
    }

    private fun binding(): DocumentBinding =
        DocumentBinding(SOURCE, GENERATION, "heading:notes/chapter/note.org:1", "notes/chapter/note.org")

    private fun write(relative: String, bytes: ByteArray) {
        File(content, relative).apply {
            parentFile?.mkdirs()
            writeBytes(bytes)
        }
    }

    /** This host-JVM test API is never loaded by an Android process. */
    @SuppressLint("NewApi")
    private fun symbolicLink(link: File, target: File) {
        Files.createSymbolicLink(link.toPath(), target.toPath())
    }

    private class RecordingLauncher : AttachmentLauncher {
        var accept = true
        var opened: StagedAttachment? = null

        override fun open(attachment: StagedAttachment): Boolean {
            opened = attachment
            return accept
        }
    }

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val GENERATION = "generation-7"

        fun png(payload: String): ByteArray = PNG + payload.toByteArray()

        val PNG = byteArrayOf(
            0x89.toByte(),
            0x50,
            0x4e,
            0x47,
            0x0d,
            0x0a,
            0x1a,
            0x0a,
        )
    }
}
