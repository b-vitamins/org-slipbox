/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.git.manual

import android.app.Activity
import android.app.Instrumentation
import android.os.Bundle
import android.provider.Settings
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.navigation.ReadingReturnsStore
import io.github.b_vitamins.slipbox.navigation.ReadingTrailStore
import io.github.b_vitamins.slipbox.navigation.SlipboxRoute
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.SourceCatalogGateway
import io.github.b_vitamins.slipbox.sources.SourceCatalogResult
import java.io.File

/** Reads the generation left by [LiveGitJourney] after a real process restart. */
class LiveOfflineGitJourney : Instrumentation() {

    private var results: File? = null

    override fun onCreate(arguments: Bundle) {
        super.onCreate(arguments)
        results = resultDirectory(arguments)
        start()
    }

    override fun onStart() {
        val report = OfflineGitJourneyReport()
        try {
            walk(report)
        } catch (fault: Throwable) {
            report.refusal = fault.javaClass.name
        } finally {
            publish(checkNotNull(results), report)
        }
    }

    private fun walk(report: OfflineGitJourneyReport) {
        report.airplaneMode =
            Settings.Global.getInt(
                targetContext.contentResolver,
                Settings.Global.AIRPLANE_MODE_ON,
                0,
            ) == 1
        if (!report.airplaneMode) return

        val loaded = SourceCatalogGateway(targetContext).load()
        report.catalogRestored =
            loaded is SourceCatalogResult.Active &&
                loaded.ready.source.id == LiveGitContract.PRIVATE_SOURCE_ID
        if (loaded !is SourceCatalogResult.Active || !report.catalogRestored) return

        val first = proveReading(loaded.ready, report) ?: return
        val root =
            when (val result = SlipboxVault.privateRoot(targetContext)) {
                is VaultOutcome.Completed -> result.value
                is VaultOutcome.Failed -> return
            }
        val trail = ReadingTrailStore(root).load(loaded.ready.binding)
        val returns = ReadingReturnsStore(root).load(loaded.ready.binding)
        report.trailRestored =
            trail.singleOrNull().let { route ->
                route is SlipboxRoute.Reader && route.note.nodeKey == first.nodeKey
            }
        report.returnsRestored =
            returns.bookmarks.singleOrNull()?.note?.nodeKey == first.nodeKey &&
                returns.recents.singleOrNull()?.note?.nodeKey == first.nodeKey
    }

    private fun proveReading(
        ready: ReadySource,
        report: OfflineGitJourneyReport,
    ): NodeRecord? {
        val host = SlipboxEngineHost.packaged()
        return try {
            val read =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            val status = read.answer(ReadOperation.Status).await() as EngineAnswer.Status
            report.generationRestored =
                status.result.filesIndexed == ready.stats.filesIndexed &&
                    status.result.nodesIndexed == ready.stats.nodesIndexed &&
                    status.result.linksIndexed == ready.stats.linksIndexed
            if (!report.generationRestored) return null

            var notePosition: String? = null
            var noteTotal: Long? = null
            val notes = mutableListOf<NodeRecord>()
            var crossedNotePage = false
            do {
                val page =
                    read.answer(ReadOperation.ListNotes(PAGE_SIZE, notePosition)).await()
                        as EngineAnswer.ListNotes
                noteTotal = noteTotal?.also { check(it == page.result.total) } ?: page.result.total
                notes += page.result.notes
                crossedNotePage = crossedNotePage || page.result.hasMore
                notePosition = page.result.nextPosition
                check(page.result.hasMore == (notePosition != null))
            } while (notePosition != null)
            report.notesPaged =
                crossedNotePage &&
                    notes.size.toLong() == noteTotal &&
                    notes.map(NodeRecord::nodeKey).distinct().size == notes.size
            val first = notes.firstOrNull() ?: return null

            var glossaryPosition: String? = null
            var glossaryTotal: Long? = null
            var glossaryCount = 0L
            do {
                val page =
                    read.answer(ReadOperation.ListGlossaryTerms(PAGE_SIZE, glossaryPosition)).await()
                        as EngineAnswer.ListGlossaryTerms
                glossaryTotal = glossaryTotal?.also { check(it == page.result.total) }
                    ?: page.result.total
                glossaryCount += page.result.terms.size
                glossaryPosition = page.result.nextPosition
                check(page.result.hasMore == (glossaryPosition != null))
            } while (glossaryPosition != null)
            report.glossaryPaged = glossaryCount == glossaryTotal

            val search =
                read.answer(ReadOperation.SearchNodes(first.title, 10)).await()
                    as EngineAnswer.SearchNodes
            report.searchRead = search.result.nodes.any { it.nodeKey == first.nodeKey }
            val relations =
                read.answer(ReadOperation.DirectedRelations(first.nodeKey, PAGE_SIZE)).await()
                    as EngineAnswer.DirectedRelations
            report.relationsRead =
                relations.result.total >= relations.result.relations.size &&
                    relations.result.hasMore == (relations.result.nextPosition != null)
            first
        } finally {
            host.close()
            check(host.awaitDisposal(READ_TIMEOUT_MILLIS)?.isComplete == true)
        }
    }

    private fun resultDirectory(arguments: Bundle): File {
        val path =
            arguments
                .getString(LiveGitContract.RESULT_ARGUMENT)
                ?.takeIf { it.startsWith('/') }
                ?: throw IllegalArgumentException("the result directory is absent")
        val directory = File(path)
        require(directory.isDirectory || directory.mkdirs()) {
            "the result directory is unavailable"
        }
        return directory
    }

    private fun publish(directory: File, report: OfflineGitJourneyReport) {
        val json = report.json()
        File(directory, LiveGitContract.OFFLINE_RESULT_FILE).writeText("$json\n")
        val outcome = Bundle().apply { putString(STREAM, "${report.summary()}\n$json\n") }
        finish(if (report.passed) Activity.RESULT_OK else Activity.RESULT_CANCELED, outcome)
    }

    private companion object {

        const val STREAM = "stream"
        const val PAGE_SIZE = 7
        const val READ_TIMEOUT_MILLIS = 30_000L
    }
}
