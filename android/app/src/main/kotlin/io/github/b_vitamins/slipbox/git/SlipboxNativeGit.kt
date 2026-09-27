/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.git

import android.content.Context
import io.github.b_vitamins.slipbox.engine.SlipboxNativeEngine

internal interface NativeGitSeam {

    fun synchronize(request: ByteArray, credential: ByteArray?): ByteArray?

    fun cancel(operation: Long): Boolean
}

/** The Git-only entry points of the packaged native library. */
internal object SlipboxNativeGit {

    val loadFailure: String?
        get() = SlipboxNativeEngine.loadFailure

    val seam: NativeGitSeam =
        object : NativeGitSeam {
            override fun synchronize(request: ByteArray, credential: ByteArray?): ByteArray? =
                nativeSynchronize(request, credential)

            override fun cancel(operation: Long): Boolean = nativeCancel(operation)
        }

    fun initialize(context: Context): Boolean =
        try {
            nativeInitialize(context.applicationContext)
        } catch (_: Exception) {
            false
        }

    private external fun nativeInitialize(context: Context): Boolean

    private external fun nativeSynchronize(request: ByteArray, credential: ByteArray?): ByteArray?

    private external fun nativeCancel(operation: Long): Boolean
}
