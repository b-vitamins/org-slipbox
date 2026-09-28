/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.content.Context
import android.content.Intent
import androidx.core.net.toUri

internal fun interface ExternalLinkHandoff {
    fun open(url: String): Boolean
}

/** A credential-free browser handoff for links already validated by the engine. */
internal class SystemExternalLinkHandoff(context: Context) : ExternalLinkHandoff {

    private val context = context.applicationContext

    override fun open(url: String): Boolean {
        val intent = intentFor(url) ?: return false
        return try {
            context.startActivity(intent)
            true
        } catch (_: RuntimeException) {
            false
        }
    }

    internal fun intentFor(url: String): Intent? {
        if (url.trim() != url || url.any { it.isWhitespace() || it.isISOControl() } || '\\' in url) {
            return null
        }
        val address = url.toUri()
        if (
            !address.isAbsolute ||
            !address.isHierarchical ||
            address.scheme?.lowercase() !in SCHEMES ||
            address.host.isNullOrEmpty() ||
            address.userInfo != null
        ) {
            return null
        }
        return Intent(Intent.ACTION_VIEW, address)
            .addCategory(Intent.CATEGORY_BROWSABLE)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    private companion object {
        val SCHEMES = setOf("http", "https")
    }
}
