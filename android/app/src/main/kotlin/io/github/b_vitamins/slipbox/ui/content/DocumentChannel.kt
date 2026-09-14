/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

internal sealed interface DocumentEvent {
    data class Raised(val intent: DocumentIntent) : DocumentEvent

    data class Refused(val reason: DocumentRefusal) : DocumentEvent
}

internal enum class DocumentRefusal {
    Origin,

    Frame,

    Binding,

    Shape,
}

/** Accepts only main-frame navigation messages for the live mount token. */
internal object DocumentChannel {
    const val OBJECT_NAME = "slipboxDocument"

    const val MESSAGE_LIMIT = 4096

    private val FORMAT =
        Json {
            ignoreUnknownKeys = false
            isLenient = false
        }

    fun read(origin: String?, mainFrame: Boolean, message: String?, token: String?): DocumentEvent {
        if (origin?.trimEnd('/') != DocumentOrigin.ORIGIN) {
            return DocumentEvent.Refused(DocumentRefusal.Origin)
        }
        if (!mainFrame) {
            return DocumentEvent.Refused(DocumentRefusal.Frame)
        }
        if (message == null || message.length > MESSAGE_LIMIT) {
            return DocumentEvent.Refused(DocumentRefusal.Shape)
        }
        val raised =
            try {
                FORMAT.decodeFromString<RaisedIntent>(message)
            } catch (malformed: SerializationException) {
                return DocumentEvent.Refused(DocumentRefusal.Shape)
            }
        if (token == null || raised.token != token) {
            return DocumentEvent.Refused(DocumentRefusal.Binding)
        }
        val intent = raised.intent() ?: return DocumentEvent.Refused(DocumentRefusal.Shape)
        return DocumentEvent.Raised(intent)
    }
}

@Serializable
private data class RaisedLink(
    val id: String? = null,
    val target: String,
    val reference: String,
)

@Serializable
private data class RaisedIntent(
    val token: String,
    val verb: String,
    val link: RaisedLink? = null,
    val gesture: String? = null,
)

private fun RaisedIntent.intent(): DocumentIntent? =
    when (verb) {
        "glance" -> {
            val asked = DocumentGesture.of(gesture)
            val target = link?.link()
            if (asked == null || target == null) null else DocumentIntent.Glance(target, asked)
        }
        "pin" -> link?.link()?.takeIf { gesture == null }?.let(DocumentIntent::Pin)
        "go" -> link?.link()?.takeIf { gesture == null }?.let(DocumentIntent::Go)
        "dismiss" -> DocumentIntent.Dismiss.takeIf { link == null && gesture == null }
        else -> null
    }

private fun RaisedLink.link(): DocumentLink? {
    if (target.isEmpty() || reference.isEmpty() || id?.isEmpty() == true) {
        return null
    }
    return DocumentLink(id = id, target = target, reference = reference)
}
