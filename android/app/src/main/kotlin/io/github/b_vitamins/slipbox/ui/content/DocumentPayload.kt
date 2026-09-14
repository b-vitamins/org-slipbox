/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import androidx.compose.runtime.Immutable
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
import java.util.Locale

@Immutable
internal data class DocumentMount(val token: String, val source: DocumentSource) {
    val binding: DocumentBinding get() = source.binding
}

/** JSON transferred as a script expression; Org source remains parser input. */
internal object DocumentPayload {
    fun present(mount: DocumentMount, presentation: DocumentPresentation): String =
        "if (window.slipboxHost) window.slipboxHost.present(${of(mount, presentation)});"

    const val DISPOSE = "if (window.slipboxHost) window.slipboxHost.dispose();"

    fun of(mount: DocumentMount, presentation: DocumentPresentation): String =
        buildString {
            append("{\"token\":")
            appendQuoted(mount.token)
            append(",\"source\":{\"id\":")
            appendQuoted(mount.source.id)
            append(",\"generation\":").append(mount.source.generation)
            append(",\"org\":")
            appendQuoted(mount.source.org)
            append(",\"baseLevel\":").append(mount.source.baseLevel)
            append("},\"presentation\":").append(presentation.toJson())
            append(",\"assetBase\":")
            appendQuoted(DocumentOrigin.assetBase(mount.token))
            append('}')
        }
}

private const val LINE_SEPARATOR = '\u2028'
private const val PARAGRAPH_SEPARATOR = '\u2029'

private fun StringBuilder.appendQuoted(text: String): StringBuilder {
    append('"')
    for (character in text) {
        when {
            character == '"' -> append("\\\"")
            character == '\\' -> append("\\\\")
            character == '\n' -> append("\\n")
            character == '\r' -> append("\\r")
            character == '\t' -> append("\\t")
            character == '<' -> append("\\u003c")
            character == LINE_SEPARATOR -> append("\\u2028")
            character == PARAGRAPH_SEPARATOR -> append("\\u2029")
            character < ' ' -> append(String.format(Locale.ROOT, "\\u%04x", character.code))
            else -> append(character)
        }
    }
    return append('"')
}
