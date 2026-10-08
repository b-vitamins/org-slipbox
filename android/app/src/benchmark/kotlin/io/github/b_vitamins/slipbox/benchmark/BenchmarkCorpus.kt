/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.benchmark

import android.content.Context
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.MaintenanceOperation
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import java.io.File
import java.util.Properties

internal data class BenchmarkCorpus(
    val requestedNodes: Int,
    val ready: ReadySource,
    val bytes: Long,
)

/** Deterministic, benchmark-variant-only corpus built through the packaged Rust engine. */
internal object BenchmarkCorpora {

    private const val SCHEMA = 1
    private const val SOURCE_ID = "62656e63686d61726b2d636f72707573"
    private const val GENERATION = "android-benchmark-v1"
    private const val ACTIVE_PREFERENCE = "active-nodes"
    private const val PREFERENCES = "slipbox-benchmark"
    private const val MARKER = "fixture.properties"
    private const val DATABASE = "index.sqlite3"
    private const val MUTABLE_NOTE = "mutable.org"
    private const val FIXED_STORAGE_BYTES = 4L * 1024 * 1024
    private const val STORAGE_BYTES_PER_NODE = 12L * 1024

    val supportedSizes: List<Int> = listOf(250, 2_500, 10_000)

    fun prepare(context: Context, requestedNodes: Int): BenchmarkCorpus {
        require(requestedNodes in supportedSizes) {
            "unsupported benchmark corpus: $requestedNodes"
        }
        val parent = File(context.noBackupFilesDir, "performance-fixtures")
        check(parent.mkdirs() || parent.isDirectory) { "cannot create $parent" }
        val workspace = File(parent, "nodes-$requestedNodes")
        val marker = File(workspace, MARKER)
        val existing = marker.takeIf(File::isFile)?.let { read(it, workspace) }
        val corpus = bounded(existing ?: rebuild(workspace, requestedNodes))
        parent.listFiles()
            ?.filter { it.isDirectory && it != workspace }
            ?.forEach { stale -> check(stale.deleteRecursively()) { "cannot remove $stale" } }
        context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
            .edit()
            .putInt(ACTIVE_PREFERENCE, requestedNodes)
            .commit()
        return corpus
    }

    fun active(context: Context): BenchmarkCorpus? {
        val nodes =
            context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
                .getInt(ACTIVE_PREFERENCE, 0)
        if (nodes !in supportedSizes) return null
        val workspace = File(context.noBackupFilesDir, "performance-fixtures/nodes-$nodes")
        return File(workspace, MARKER).takeIf(File::isFile)?.let { read(it, workspace) }
    }

    fun updateActive(context: Context, rounds: Int) {
        val corpus = requireNotNull(active(context)) { "no active benchmark corpus" }
        val root = File(corpus.ready.contentRoot)
        val note = File(root, MUTABLE_NOTE)
        val binding = corpus.ready.binding
        val sessionContext = SessionContext(root.absolutePath, corpus.ready.database)
        SlipboxEngineHost.packaged().use { host ->
            val session = host.openMaintenance(binding, sessionContext).await()
            try {
                repeat(rounds) { round ->
                    note.writeText(mutableNote(round))
                    session.carryOut(MaintenanceOperation.IndexFile(MUTABLE_NOTE)).await()
                }
            } finally {
                session.retire().await()
            }
        }
    }

    private fun rebuild(workspace: File, requestedNodes: Int): BenchmarkCorpus {
        if (workspace.exists()) check(workspace.deleteRecursively()) { "cannot reset $workspace" }
        val root = File(workspace, "notes")
        check(root.mkdirs()) { "cannot create $root" }
        writeCorpus(root, requestedNodes)

        val database = File(workspace, DATABASE)
        val binding = GenerationBinding(SOURCE_ID, GENERATION)
        val context = SessionContext(root.absolutePath, database.absolutePath)
        val stats =
            SlipboxEngineHost.packaged().use { host ->
                val session = host.openMaintenance(binding, context).await()
                try {
                    val answer = session.carryOut(MaintenanceOperation.Index).await()
                    (answer as EngineAnswer.Index).result
                } finally {
                    session.retire().await()
                }
            }
        val ready =
            ReadySource(
                source = source(),
                binding = binding,
                revision = "fixture-$SCHEMA-$requestedNodes",
                contentRoot = root.absolutePath,
                database = database.absolutePath,
                stats =
                    ReadySourceStats(
                        filesIndexed = stats.filesIndexed,
                        nodesIndexed = stats.nodesIndexed,
                        linksIndexed = stats.linksIndexed,
                    ),
            )
        val marker = File(workspace, MARKER)
        write(marker, requestedNodes, ready)
        return BenchmarkCorpus(requestedNodes, ready, workspace.bytes())
    }

    private fun writeCorpus(root: File, requestedNodes: Int) {
        File(root, "momentum.org").writeText(richNote())
        File(root, "reference.org").writeText(referenceNote())
        File(root, MUTABLE_NOTE).writeText(mutableNote(0))
        File(root, "glossary-concept.org").writeText(glossaryNote())

        // One file node plus 24 heading nodes keeps large fixtures realistic
        // without turning file discovery into the thing being benchmarked.
        val genericFiles = ((requestedNodes - 4).coerceAtLeast(0) + 24) / 25
        repeat(genericFiles) { file ->
            val serial = file.toString().padStart(4, '0')
            val title =
                if (file % 12 == 0) {
                    "Glossary concept $serial"
                } else {
                    "Notebook $serial"
                }
            val glossary = if (file % 12 == 0) "#+glossary: t\n" else ""
            val text = buildString {
                append("#+title: $title\n")
                append(glossary)
                append(":PROPERTIES:\n:ID: benchmark-file-$file\n:END:\n\n")
                append("A bounded synthetic reading note for Android performance qualification.\n")
                repeat(24) { heading ->
                    val serial = file * 24 + heading
                    append("\n* Observation ${serial.toString().padStart(5, '0')}\n")
                    append(":PROPERTIES:\n:ID: benchmark-node-$serial\n:END:\n")
                    append("Indexed prose about memory, inference, and attention token-$serial. ")
                    append("[[id:benchmark-reference][Reference frame]].\n")
                }
            }
            File(root, "notebook-$serial.org").writeText(text)
        }
    }

    private fun richNote(): String =
        """
        #+title: Momentum, memory, and proof
        :PROPERTIES:
        :ID: benchmark-momentum
        :END:

        Momentum is useful only when it remains legible under sustained reading.
        Follow the [[id:benchmark-reference][Reference frame]] for the linked argument.

        * A compact equation

        The update is \(v_{t+1}=\beta v_t + \nabla f(x_t)\), with

        \[
        x_{t+1}=x_t-\eta v_{t+1}.
        \]

        * A code specimen

        #+begin_src rust
        fn momentum(beta: f64, previous: f64, gradient: f64) -> f64 {
            beta * previous + gradient
        }
        #+end_src

        * A dense table

        | surface  | query shape | expected property |
        |----------+-------------+-------------------|
        | search   | bounded     | sublinear         |
        | reading  | one note    | stable frames     |
        | glossary | one page    | bounded           |

        * Long-form reading

        ${List(48) { paragraph ->
            "Paragraph ${paragraph + 1}. A calm reading surface preserves measure, hierarchy, " +
                "and rhythm while links, equations, code, and prose share the page."
        }.joinToString("\n\n")}
        """.trimIndent() + "\n"

    private fun referenceNote(): String =
        """
        #+title: Reference frame
        :PROPERTIES:
        :ID: benchmark-reference
        :END:

        A linked note used to measure recursive reading navigation.
        Return to [[id:benchmark-momentum][Momentum, memory, and proof]].
        """.trimIndent() + "\n"

    private fun glossaryNote(): String =
        """
        #+title: Gradient flow
        #+glossary: t
        :PROPERTIES:
        :ID: benchmark-gradient-flow
        :ROAM_ALIASES: "Steepest descent flow"
        :END:

        The continuous-time limit \(\dot{x}=-\nabla f(x)\) of gradient descent.
        """.trimIndent() + "\n"

    private fun mutableNote(round: Int): String =
        """
        #+title: Incremental update lane
        :PROPERTIES:
        :ID: benchmark-mutable
        :END:

        Background publication round $round. Readers must remain responsive while this
        file is indexed.
        """.trimIndent() + "\n"

    private fun source(): RefreshSource =
        RefreshSource(
            id = SOURCE_ID,
            displayName = "Benchmark corpus",
            provider = RefreshProvider.GENERIC_HTTPS,
            visibility = RefreshVisibility.PUBLIC,
            remote = "https://example.invalid/slipbox-benchmark.git",
            branch = "main",
            notesFolder = "",
        )

    private fun write(marker: File, requestedNodes: Int, ready: ReadySource) {
        val properties =
            Properties().apply {
                setProperty("schema", SCHEMA.toString())
                setProperty("requestedNodes", requestedNodes.toString())
                setProperty("filesIndexed", ready.stats.filesIndexed.toString())
                setProperty("nodesIndexed", ready.stats.nodesIndexed.toString())
                setProperty("linksIndexed", ready.stats.linksIndexed.toString())
            }
        val temporary = File(marker.parentFile, "$MARKER.tmp")
        temporary.outputStream().use { properties.store(it, null) }
        check(temporary.renameTo(marker)) { "cannot publish $marker" }
    }

    private fun read(marker: File, workspace: File): BenchmarkCorpus? {
        val properties = Properties()
        marker.inputStream().use(properties::load)
        val schema = properties.getProperty("schema")?.toIntOrNull() ?: return null
        val requested = properties.getProperty("requestedNodes")?.toIntOrNull() ?: return null
        if (schema != SCHEMA || requested !in supportedSizes) return null
        val root = File(workspace, "notes")
        val database = File(workspace, DATABASE)
        if (!root.isDirectory || !database.isFile) return null
        val stats =
            ReadySourceStats(
                filesIndexed = properties.long("filesIndexed") ?: return null,
                nodesIndexed = properties.long("nodesIndexed") ?: return null,
                linksIndexed = properties.long("linksIndexed") ?: return null,
            )
        val ready =
            ReadySource(
                source = source(),
                binding = GenerationBinding(SOURCE_ID, GENERATION),
                revision = "fixture-$SCHEMA-$requested",
                contentRoot = root.absolutePath,
                database = database.absolutePath,
                stats = stats,
            )
        return BenchmarkCorpus(requested, ready, workspace.bytes())
    }

    private fun Properties.long(name: String): Long? = getProperty(name)?.toLongOrNull()

    private fun bounded(corpus: BenchmarkCorpus): BenchmarkCorpus {
        val indexed = corpus.ready.stats.nodesIndexed
        val expected = corpus.requestedNodes.toLong()..corpus.requestedNodes + 24L
        check(indexed in expected) {
            "fixture indexed $indexed nodes for ${corpus.requestedNodes} requested"
        }
        val storageLimit = FIXED_STORAGE_BYTES + corpus.requestedNodes * STORAGE_BYTES_PER_NODE
        check(corpus.bytes <= storageLimit) {
            "fixture uses ${corpus.bytes} bytes; " +
                "the ${corpus.requestedNodes}-node limit is $storageLimit"
        }
        return corpus
    }

    private fun File.bytes(): Long = walkTopDown().filter(File::isFile).sumOf(File::length)
}
