/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.benchmark

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.compose.ReportDrawnWhen
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import io.github.b_vitamins.slipbox.SlipboxApp
import io.github.b_vitamins.slipbox.sources.SourceCatalogListing
import io.github.b_vitamins.slipbox.sources.SourceCatalogListingResult
import io.github.b_vitamins.slipbox.sources.SourceCatalogResult
import io.github.b_vitamins.slipbox.sources.SourceLibraryCatalog
import io.github.b_vitamins.slipbox.sources.SourceLibraryState

class BenchmarkCorpusActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val status = TextView(this).apply { text = "Preparing benchmark fixture" }
        setContentView(status)
        val nodes = intent.getIntExtra(EXTRA_NODES, 0)
        Thread(
                {
                    val result = runCatching { BenchmarkCorpora.prepare(applicationContext, nodes) }
                    runOnUiThread {
                        status.text =
                            result.fold(
                                onSuccess = { corpus ->
                                    "Benchmark fixture ready: requested=${corpus.requestedNodes} " +
                                        "indexed=${corpus.ready.stats.nodesIndexed} " +
                                        "bytes=${corpus.bytes}"
                                },
                                onFailure = {
                                    "Benchmark fixture failed: ${it.javaClass.simpleName}"
                                },
                            )
                    }
                },
                "slipbox-benchmark-fixture",
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    companion object {
        const val EXTRA_NODES = "nodes"
    }
}

class BenchmarkSlipboxActivity : ComponentActivity() {

    private var library: SourceLibraryState? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val corpus = BenchmarkCorpora.active(applicationContext)
        if (corpus == null) {
            setContentView(TextView(this).apply { text = "Benchmark fixture missing" })
            return
        }
        val state = SourceLibraryState(FixedCatalog(corpus.ready))
        library = state
        setContent {
            ReportDrawnWhen {
                state.phase is io.github.b_vitamins.slipbox.sources.SourceLibraryPhase.Ready
            }
            SlipboxApp(state)
        }
    }

    override fun onDestroy() {
        library?.close()
        super.onDestroy()
    }
}

class BenchmarkIndexReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACTION_INDEX) return
        Thread(
                { BenchmarkCorpora.updateActive(context.applicationContext, ROUNDS) },
                "slipbox-benchmark-index",
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    companion object {
        const val ACTION_INDEX = "io.github.b_vitamins.slipbox.benchmark.INDEX"
        private const val ROUNDS = 24
    }
}

private class FixedCatalog(private val ready: io.github.b_vitamins.slipbox.sources.ReadySource) :
    SourceLibraryCatalog {

    private val listing = SourceCatalogListing(1, listOf(ready.source), ready.source)

    override fun list(): SourceCatalogListingResult = SourceCatalogListingResult.Loaded(listing)

    override fun load(listing: SourceCatalogListing): SourceCatalogResult =
        SourceCatalogResult.Active(listing.revision, ready)

    override fun activate(
        expectedRevision: Long,
        source: io.github.b_vitamins.slipbox.sync.RefreshSource,
    ): SourceCatalogResult = SourceCatalogResult.Active(expectedRevision, ready)
}
