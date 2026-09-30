/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.FileProvider
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import java.io.BufferedOutputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.io.InputStreamReader
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.util.Locale
import java.util.UUID

internal const val MAX_INLINE_ASSET_BYTES: Long = 12L * 1024L * 1024L
internal const val MAX_ATTACHMENT_BYTES: Long = 32L * 1024L * 1024L

private const val MAX_REPOSITORY_PATH_BYTES = 1024
private const val MAX_STAGED_ATTACHMENTS = 8
private const val COPY_BUFFER_BYTES = 64 * 1024

private val SCHEME = Regex("^[A-Za-z][A-Za-z0-9+.-]*:")
private val ORG_FILE = Regex("(?:^|/)[^/]+\\.org(?:::.*)?$", RegexOption.IGNORE_CASE)

internal sealed interface AttachmentOpenResult {
    data object Opened : AttachmentOpenResult

    data class Missing(val label: String) : AttachmentOpenResult

    data class Unsupported(val label: String) : AttachmentOpenResult

    data class Oversized(val label: String, val maxBytes: Long) : AttachmentOpenResult

    data class ViewerUnavailable(val label: String) : AttachmentOpenResult

    data class Failed(val label: String) : AttachmentOpenResult
}

internal fun interface ReaderAttachment {
    fun open(binding: DocumentBinding, target: String): AttachmentOpenResult
}

internal fun interface AttachmentLauncher {
    fun open(attachment: StagedAttachment): Boolean
}

internal data class StagedAttachment(
    val file: File,
    val displayName: String,
    val mimeType: String,
)

/** Exact-generation repository images and explicit read-only attachment handoffs. */
internal class RepositoryAssets(
    contentRoot: File,
    private val expected: GenerationBinding,
    stagingRoot: File,
    private val launcher: AttachmentLauncher,
) : DocumentAssetResolver, ReaderAttachment {

    private val locator = RepositoryAssetLocator(contentRoot, expected)
    private val stager = AttachmentStager(stagingRoot)

    override fun resolve(binding: DocumentBinding, target: String): DocumentAsset? =
        when (val found = locator.locate(binding, target, MAX_INLINE_ASSET_BYTES, imagesOnly = true)) {
            is AssetLookup.Available -> {
                val bytes = boundedBytes(found.file, MAX_INLINE_ASSET_BYTES) ?: return null
                DocumentAsset(found.mimeType, bytes)
            }
            else -> null
        }

    override fun open(binding: DocumentBinding, target: String): AttachmentOpenResult =
        when (val found = locator.locate(binding, target, MAX_ATTACHMENT_BYTES, imagesOnly = false)) {
            is AssetLookup.Available -> {
                val staged = stager.stage(found)
                    ?: return AttachmentOpenResult.Failed(found.label)
                if (launcher.open(staged)) {
                    AttachmentOpenResult.Opened
                } else {
                    AttachmentOpenResult.ViewerUnavailable(found.label)
                }
            }
            is AssetLookup.Missing -> AttachmentOpenResult.Missing(found.label)
            is AssetLookup.Unsupported -> AttachmentOpenResult.Unsupported(found.label)
            is AssetLookup.Oversized ->
                AttachmentOpenResult.Oversized(found.label, found.maxBytes)
            is AssetLookup.Failed -> AttachmentOpenResult.Failed(found.label)
        }

    internal companion object {
        fun packaged(context: Context, expected: GenerationBinding, contentRoot: String): RepositoryAssets {
            val application = context.applicationContext
            return RepositoryAssets(
                contentRoot = File(contentRoot),
                expected = expected,
                stagingRoot = File(application.cacheDir, "attachments"),
                launcher = SystemAttachmentLauncher(application),
            )
        }
    }
}

/** Whether the renderer routes this target to the repository-asset channel. */
internal fun isRepositoryAssetTarget(target: String): Boolean {
    if (target.isEmpty() || target.startsWith("*") || target.startsWith("#")) return false
    val path =
        if (target.startsWith("file:", ignoreCase = true)) {
            target.substring("file:".length)
        } else {
            if (SCHEME.containsMatchIn(target)) return false
            target
        }
    return path.isNotEmpty() && !ORG_FILE.matches(path)
}

internal class SystemAttachmentLauncher(context: Context) : AttachmentLauncher {

    private val context = context.applicationContext

    override fun open(attachment: StagedAttachment): Boolean {
        val intent = intentFor(attachment) ?: return false
        return try {
            context.startActivity(intent)
            true
        } catch (_: RuntimeException) {
            false
        }
    }

    internal fun intentFor(attachment: StagedAttachment): Intent? {
        val canonicalRoot =
            try {
                File(context.cacheDir, "attachments").canonicalFile
            } catch (_: IOException) {
                return null
            } catch (_: SecurityException) {
                return null
            }
        val canonical =
            try {
                attachment.file.canonicalFile
            } catch (_: IOException) {
                return null
            } catch (_: SecurityException) {
                return null
            }
        if (!canonical.isFile || canonical.parentFile?.parentFile != canonicalRoot) return null
        val uri =
            try {
                FileProvider.getUriForFile(context, "${context.packageName}.attachments", canonical)
            } catch (_: RuntimeException) {
                return null
            }
        return attachmentIntent(uri, attachment)
    }

    private fun attachmentIntent(uri: Uri, attachment: StagedAttachment): Intent =
        Intent(Intent.ACTION_VIEW)
            .setDataAndType(uri, attachment.mimeType)
            .apply {
                clipData = ClipData.newRawUri(attachment.displayName, uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            }
}

private sealed interface AssetLookup {
    val label: String

    data class Available(
        val file: File,
        override val label: String,
        val mimeType: String,
    ) : AssetLookup

    data class Missing(override val label: String) : AssetLookup

    data class Unsupported(override val label: String) : AssetLookup

    data class Oversized(override val label: String, val maxBytes: Long) : AssetLookup

    data class Failed(override val label: String) : AssetLookup
}

private class RepositoryAssetLocator(
    contentRoot: File,
    private val expected: GenerationBinding,
) {

    private val root: File? =
        try {
            contentRoot.canonicalFile
        } catch (_: IOException) {
            null
        } catch (_: SecurityException) {
            null
        }

    fun locate(
        binding: DocumentBinding,
        target: String,
        maxBytes: Long,
        imagesOnly: Boolean,
    ): AssetLookup {
        val label = assetLabel(target)
        val root = root ?: return AssetLookup.Failed(label)
        if (
            binding.source != expected.source ||
                binding.generation != expected.generation ||
                maxBytes !in 1..MAX_ATTACHMENT_BYTES
        ) {
            return AssetLookup.Unsupported(label)
        }
        val relative = resolveRelative(binding.filePath, target)
            ?: return AssetLookup.Unsupported(label)
        val mimeType = assetMimeType(relative, imagesOnly)
            ?: return AssetLookup.Unsupported(label)
        val expectedFile = File(root, relative).absoluteFile
        val canonical =
            try {
                expectedFile.canonicalFile
            } catch (_: IOException) {
                return AssetLookup.Missing(label)
            } catch (_: SecurityException) {
                return AssetLookup.Failed(label)
            }
        return try {
            if (!canonical.startsInside(root) || canonical != expectedFile) {
                return AssetLookup.Unsupported(label)
            }
            if (!canonical.exists()) return AssetLookup.Missing(label)
            if (!canonical.isFile) return AssetLookup.Unsupported(label)
            val size = canonical.length()
            if (size < 0) return AssetLookup.Failed(label)
            if (size > maxBytes) return AssetLookup.Oversized(label, maxBytes)
            if (!matchesDeclaredType(canonical, mimeType)) {
                return AssetLookup.Unsupported(label)
            }
            AssetLookup.Available(canonical, label, mimeType)
        } catch (_: SecurityException) {
            AssetLookup.Failed(label)
        }
    }
}

private fun resolveRelative(document: String, target: String): String? {
    val documentParts = strictParts(document) ?: return null
    if (documentParts.isEmpty()) return null
    val raw =
        when {
            target.startsWith("file:", ignoreCase = true) -> target.substring("file:".length)
            SCHEME.containsMatchIn(target) -> return null
            else -> target
        }
    if (
        raw.isEmpty() ||
            raw.startsWith('/') ||
            raw.startsWith('~') ||
            "::" in raw ||
            '\\' in raw ||
            raw.any { it == '\u0000' || it.isISOControl() }
    ) {
        return null
    }
    val parts = documentParts.dropLast(1).toMutableList()
    for (part in raw.split('/')) {
        when (part) {
            "" -> return null
            "." -> Unit
            ".." -> if (parts.isEmpty()) return null else parts.removeAt(parts.lastIndex)
            else -> parts.add(part)
        }
    }
    if (parts.isEmpty()) return null
    val resolved = parts.joinToString("/")
    return resolved.takeIf { it.toByteArray(Charsets.UTF_8).size <= MAX_REPOSITORY_PATH_BYTES }
}

private fun strictParts(path: String): List<String>? {
    if (
        path.isEmpty() ||
            path.startsWith('/') ||
            '\\' in path ||
            path.any { it == '\u0000' || it.isISOControl() }
    ) {
        return null
    }
    return path.split('/').takeIf { parts ->
        parts.all { it.isNotEmpty() && it != "." && it != ".." }
    }
}

private fun assetMimeType(path: String, imagesOnly: Boolean): String? {
    val extension = path.substringAfterLast('.', "").lowercase(Locale.ROOT)
    return when (extension) {
        "png" -> "image/png"
        "jpg", "jpeg" -> "image/jpeg"
        "gif" -> "image/gif"
        "webp" -> "image/webp"
        "txt" -> "text/plain".takeUnless { imagesOnly }
        else -> null
    }
}

private fun matchesDeclaredType(file: File, mimeType: String): Boolean =
    try {
        if (mimeType == "text/plain") {
            val decoder =
                Charsets.UTF_8
                    .newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
            InputStreamReader(file.inputStream().buffered(), decoder).use { reader ->
                val buffer = CharArray(8192)
                while (reader.read(buffer) != -1) {
                    // Decoding the complete stream is the validation.
                }
            }
            true
        } else {
            val header = ByteArray(12)
            val read = file.inputStream().buffered().use { it.read(header) }
            when (mimeType) {
                "image/png" -> read >= 8 && header.copyOfRange(0, 8).contentEquals(PNG)
                "image/jpeg" -> read >= 3 && header[0] == 0xff.toByte() && header[1] == 0xd8.toByte() && header[2] == 0xff.toByte()
                "image/gif" -> read >= 6 && (header.ascii(6) == "GIF87a" || header.ascii(6) == "GIF89a")
                "image/webp" -> read >= 12 && header.ascii(4) == "RIFF" && header.ascii(4, 8) == "WEBP"
                else -> false
            }
        }
    } catch (_: IOException) {
        false
    } catch (_: RuntimeException) {
        false
    }

private fun ByteArray.ascii(length: Int, offset: Int = 0): String =
    copyOfRange(offset, offset + length).toString(Charsets.US_ASCII)

private fun File.startsInside(root: File): Boolean {
    var current: File? = this
    while (current != null) {
        if (current == root) return this != root
        current = current.parentFile
    }
    return false
}

private class AttachmentStager(private val root: File) {

    @Synchronized
    fun stage(asset: AssetLookup.Available): StagedAttachment? =
        try {
            if ((!root.exists() && !root.mkdirs()) || !root.isDirectory) return null
            prune()
            val directory = File(root, UUID.randomUUID().toString())
            if (!directory.mkdir()) return null
            val destination = File(directory, safeFileName(asset.label))
            val temporary = File(directory, ".partial")
            asset.file.inputStream().buffered().use { input ->
                FileOutputStream(temporary).use { fileOutput ->
                    val output = BufferedOutputStream(fileOutput)
                    val buffer = ByteArray(COPY_BUFFER_BYTES)
                    var total = 0L
                    while (true) {
                        val read = input.read(buffer)
                        if (read < 0) break
                        total += read
                        if (total > MAX_ATTACHMENT_BYTES) throw IOException("attachment grew")
                        output.write(buffer, 0, read)
                    }
                    output.flush()
                    fileOutput.fd.sync()
                }
            }
            if (!temporary.renameTo(destination)) throw IOException("attachment stage failed")
            StagedAttachment(destination, asset.label, asset.mimeType)
        } catch (_: IOException) {
            null
        } catch (_: SecurityException) {
            null
        }

    private fun prune() {
        val held = root.listFiles()?.filter(File::isDirectory)?.sortedByDescending(File::lastModified).orEmpty()
        for (stale in held.drop(MAX_STAGED_ATTACHMENTS - 1)) stale.deleteRecursively()
    }
}

private fun boundedBytes(file: File, limit: Long): ByteArray? =
    try {
        file.inputStream().buffered().use { input ->
            val output = ByteArrayOutputStream(minOf(file.length(), limit).toInt())
            val buffer = ByteArray(COPY_BUFFER_BYTES)
            var total = 0L
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                total += read
                if (total > limit) return null
                output.write(buffer, 0, read)
            }
            output.toByteArray()
        }
    } catch (_: IOException) {
        null
    } catch (_: SecurityException) {
        null
    }

private fun assetLabel(target: String): String {
    val path =
        if (target.startsWith("file:", ignoreCase = true)) {
            target.substring("file:".length)
        } else {
            target
        }.substringBefore("::")
    val name = path.substringAfterLast('/').ifBlank { "Attachment" }
    val clean = name.filterNot { it.isISOControl() || it == '/' || it == '\\' }.trim()
    return clean.ifEmpty { "Attachment" }.take(96)
}

private fun safeFileName(label: String): String =
    label.map { character ->
        when {
            character.isLetterOrDigit() || character in ". _-" -> character
            else -> '_'
        }
    }.joinToString("").trim('.', ' ').ifEmpty { "attachment" }.take(96)

private val PNG = byteArrayOf(
    0x89.toByte(),
    0x50,
    0x4e,
    0x47,
    0x0d,
    0x0a,
    0x1a,
    0x0a,
)
