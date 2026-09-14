/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import android.content.Context
import android.content.Intent
import androidx.core.net.toUri

fun interface BrowserHandoff {


    fun open(url: String): Boolean
}

/** Opens HTTPS verification pages in the system browser. */
class SystemBrowserHandoff(context: Context) : BrowserHandoff {

    private val context = context.applicationContext

    override fun open(url: String): Boolean {
        val intent = intentFor(url) ?: return false
        return try {
            context.startActivity(intent)
            true
        } catch (unavailable: RuntimeException) {
            false
        }
    }


    internal fun intentFor(url: String): Intent? {
        if (!url.startsWith(HTTPS)) {
            return null
        }
        val address = url.toUri()
        if (!address.isAbsolute || address.host.isNullOrEmpty()) {
            return null
        }
        return Intent(Intent.ACTION_VIEW, address)
            .addCategory(Intent.CATEGORY_BROWSABLE)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    private companion object {

        const val HTTPS = "https://"
    }
}
