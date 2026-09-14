/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation

private val NoAssets = DocumentAssetResolver { _, _ -> null }

/** Bundled Org rendering with caller-owned navigation and source-bound assets. */
@Composable
internal fun DocumentContentView(
    source: DocumentSource,
    presentation: DocumentPresentation,
    modifier: Modifier = Modifier,
    onIntent: (DocumentIntent) -> Unit = {},
    resolveAsset: DocumentAssetResolver = NoAssets,
) {
    val context = LocalContext.current
    // Callbacks may change while the WebView remains mounted.
    val raised = rememberUpdatedState(onIntent)
    val resolver = rememberUpdatedState(resolveAsset)
    // A context change retires both the host and its view.
    key(context) {
        val host =
            remember {
                DocumentHost(
                    context = context,
                    onIntent = { intent -> raised.value(intent) },
                    resolver =
                        DocumentAssetResolver { binding, target ->
                            resolver.value.resolve(binding, target)
                        },
                )
            }
        AndroidView(
            factory = { host.view },
            modifier = modifier,
            update = { host.present(source, presentation) },
            onRelease = { host.dispose() },
        )
    }
}
