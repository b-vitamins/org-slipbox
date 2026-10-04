/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.auth

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.PersistableBundle

internal fun interface VerificationCodeClipboard {


    fun copy(code: String): Boolean
}

/** Places the short-lived GitHub user code on Android's clipboard. */
internal class SystemVerificationCodeClipboard(context: Context) : VerificationCodeClipboard {

    private val clipboard = context.getSystemService(ClipboardManager::class.java)

    override fun copy(code: String): Boolean =
        try {
            val clip = ClipData.newPlainText(LABEL, code)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                clip.description.extras =
                    PersistableBundle().apply { putBoolean(EXTRA_IS_SENSITIVE, true) }
            }
            clipboard?.setPrimaryClip(clip)
            clipboard != null
        } catch (unavailable: RuntimeException) {
            false
        }

    private companion object {


        const val LABEL = "GitHub verification code"

        // ClipDescription.EXTRA_IS_SENSITIVE is API 33; Android recognizes this key earlier too.
        const val EXTRA_IS_SENSITIVE = "android.content.extra.IS_SENSITIVE"
    }
}
