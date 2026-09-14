/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.graphics.Bitmap
import android.os.SystemClock
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.core.graphics.createBitmap
import androidx.test.platform.app.InstrumentationRegistry
import java.io.ByteArrayOutputStream
import java.util.Collections
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.TimeUnit
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

internal const val DOCUMENT_TAG = "document-content"

internal fun documentViewIn(root: View): WebView? =
    when (root) {
        is WebView -> root
        is ViewGroup -> (0 until root.childCount).firstNotNullOfOrNull { documentViewIn(root.getChildAt(it)) }
        else -> null
    }

internal fun WebView.answer(script: String): String {
    val answered = ArrayBlockingQueue<String>(1)
    InstrumentationRegistry.getInstrumentation().runOnMainSync {
        evaluateJavascript(script) { value -> answered.offer(value ?: "null") }
    }
    return checkNotNull(answered.poll(ANSWER_SECONDS, TimeUnit.SECONDS)) {
        "the page did not answer $script"
    }
}

internal fun WebView.text(script: String): String {
    val answer = answer(script)
    val value = JSONTokener(answer).nextValue()
    check(value is String) { "$script answered $answer" }
    return value
}

internal fun WebView.number(script: String): Double {
    val answer = answer(script)
    val value = JSONTokener(answer).nextValue()
    check(value is Number) { "$script answered $answer" }
    return value.toDouble()
}

internal fun WebView.record(script: String): JSONObject = JSONObject(text(script))

internal fun WebView.strings(script: String): List<String> {
    val array = JSONArray(text(script))
    return (0 until array.length()).map { array.getString(it) }
}

internal fun WebView.tags(selector: String): List<String> =
    strings("JSON.stringify(Array.from(document.querySelectorAll(${selector.quoted()})).map(e => e.tagName))")

internal fun WebView.count(selector: String): Int =
    number("document.querySelectorAll(${selector.quoted()}).length").toInt()

internal fun WebView.awaitTrue(what: String, script: String) {
    val deadline = SystemClock.uptimeMillis() + AWAIT_MILLIS
    while (SystemClock.uptimeMillis() < deadline) {
        if (answer(script) == "true") {
            return
        }
        SystemClock.sleep(POLL_MILLIS)
    }
    error("$what: $script held ${answer(script)} to the last")
}

internal fun WebView.awaitState(state: String) =
    awaitTrue(
        "the page reached $state",
        "document.documentElement.dataset.slipboxState === ${state.quoted()}",
    )

internal fun WebView.awaitMounted() {
    awaitState("ready")
    awaitTrue(
        "the renderer built the document",
        "document.querySelectorAll('#document .org-document').length === 1",
    )
}

/** Image loads exercise the asset loader; CSP forbids fetch/XHR. */
internal fun WebView.probeImage(url: String): String {
    answer(
        """
        window.__probe = 'pending';
        (function () {
          const image = new Image();
          image.onload = function () {
            window.__probe = 'loaded ' + image.naturalWidth + 'x' + image.naturalHeight;
          };
          image.onerror = function () { window.__probe = 'failed'; };
          image.src = ${url.quoted()};
        })();
        """
            .trimIndent(),
    )
    awaitTrue("the image probe settled", "window.__probe !== 'pending'")
    return text("window.__probe")
}

internal fun WebView.fetched(): List<String> =
    strings("JSON.stringify(performance.getEntriesByType('resource').map(e => e.name))")

internal data class Requested(val name: String, val status: Int)

internal fun WebView.requests(): List<Requested> {
    val entries =
        JSONArray(
            text(
                "JSON.stringify(performance.getEntriesByType('resource')" +
                    ".map(e => ({ name: e.name, status: e.responseStatus })))",
            ),
        )
    return (0 until entries.length()).map {
        val entry = entries.getJSONObject(it)
        Requested(entry.getString("name"), entry.optInt("status"))
    }
}

internal fun WebView.mountToken(): String {
    val href = text("document.querySelector('a.org-link--asset').href")
    val prefix = "${DocumentOrigin.ORIGIN}${DocumentOrigin.ASSET_PREFIX}"
    check(href.startsWith(prefix)) { "$href carries no asset prefix" }
    return href.removePrefix(prefix).substringBefore('/')
}

internal fun String.quoted(): String = JSONObject.quote(this)

internal const val DOCUMENT_LINK = "#document a.org-link[role=\"link\"]"

internal const val PRESS_LINK =
    "(function () {" +
        "  const link = document.querySelector('$DOCUMENT_LINK');" +
        "  link.dispatchEvent(new PointerEvent('pointerdown', " +
        "    { pointerType: 'touch', bubbles: true, cancelable: true }));" +
        "  link.dispatchEvent(new MouseEvent('click', " +
        "    { detail: 1, button: 0, bubbles: true, cancelable: true }));" +
        "})();"

internal const val OPEN_LINK =
    "(function () {" +
        "  document.querySelector('$DOCUMENT_LINK').dispatchEvent(" +
        "    new KeyboardEvent('keydown', " +
        "      { key: 'Enter', bubbles: true, cancelable: true }));" +
        "})();"

internal const val WITHDRAW_PREVIEW =
    "(function () {" +
        "  document.querySelector('$DOCUMENT_LINK').dispatchEvent(" +
        "    new KeyboardEvent('keydown', " +
        "      { key: 'Escape', bubbles: true, cancelable: true }));" +
        "})();"

internal class Gestures : (DocumentIntent) -> Unit {
    private val raised = Collections.synchronizedList(mutableListOf<DocumentIntent>())

    override fun invoke(intent: DocumentIntent) {
        raised.add(intent)
    }

    fun reported(): List<DocumentIntent> = raised.toList()

    fun forget() = raised.clear()

    fun awaited(count: Int): List<DocumentIntent> {
        val deadline = SystemClock.uptimeMillis() + AWAIT_MILLIS
        while (SystemClock.uptimeMillis() < deadline && raised.size < count) {
            SystemClock.sleep(POLL_MILLIS)
        }
        return reported()
    }

    fun quiet(): List<DocumentIntent> {
        SystemClock.sleep(QUIET_MILLIS)
        return reported()
    }
}

internal data class AssetAsk(val binding: DocumentBinding, val target: String)

internal class AssetStore(private val held: Map<String, DocumentAsset> = emptyMap()) :
    DocumentAssetResolver {

    private val asked = Collections.synchronizedList(mutableListOf<AssetAsk>())

    override fun resolve(binding: DocumentBinding, target: String): DocumentAsset? {
        asked.add(AssetAsk(binding, target))
        return held[target]
    }

    fun asks(): List<AssetAsk> = asked.toList()

    fun forget() = asked.clear()
}

internal fun pngAsset(size: Int, colour: Int): DocumentAsset {
    val bitmap = createBitmap(size, size)
    bitmap.eraseColor(colour)
    val encoded = ByteArrayOutputStream()
    check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, encoded)) {
        "the synthetic asset could not be encoded"
    }
    return DocumentAsset("image/png", encoded.toByteArray())
}

internal object Notes {
    const val NOTE_ID = "5c4b3a29-1d0e-4f5a-8b7c-6d5e4f3a2b19"

    private const val PARAGRAPHS = 60

    val RICH =
        listOf(
            "A fixed point is a value a map returns unchanged, so \\(f(x) = x\\) is the",
            "whole of it. See [[id:$NOTE_ID][the settled column]] for the reading it came from,",
            "and [[file:diagram.png][the diagram]] beside it.",
            "",
            "* The measure",
            "",
            "One column, one measure, /set once/ and *kept*, with =625= as the width.",
            "",
            "\\[",
            "\\sum_{n=1}^{\\infty} \\frac{1}{n^2} = \\frac{\\pi^2}{6}",
            "\\]",
            "",
            "** The table it settles into",
            "",
            "| Quantity | Value |",
            "|----------+-------|",
            "| Measure  | 625   |",
            "| Rule     | 1     |",
            "",
            "#+begin_src kotlin",
            "fun settle(column: Column): Column = column.copy(measure = 625, rule = 1)",
            "#+end_src",
        )
            .joinToString("\n")

    val HOSTILE =
        listOf(
            "<script>window.slipboxOwned = true;</script>",
            "<img src=\"x\" onerror=\"window.slipboxOwned = true\" />",
            "",
            "A [[javascript:window.slipboxOwned = true][link that would run]] and a",
            "[[https://elsewhere.invalid/page][link somewhere else]].",
            "",
            "\\(\\href{javascript:window.slipboxOwned = true}{run me}\\)",
            "",
            "\\[",
            "\\includegraphics{https://elsewhere.invalid/diagram.png}",
            "\\]",
            "",
            "\\(\\def\\loop{\\loop}\\loop\\)",
        )
            .joinToString("\n")

    val TALL =
        (listOf(RICH) + (1..PARAGRAPHS).map { "Paragraph $it, which the column carries on down." })
            .joinToString("\n\n")

    val ASSETS =
        listOf(
            "[[file:diagram.png][the diagram]] and [[file:figures/nested.png][the nested one]],",
            "beside [[file:missing.png][one that is not there]] and",
            "[[file:../../etc/hosts][one that walks out]].",
        )
            .joinToString("\n")
}

private const val ANSWER_SECONDS = 10L
private const val AWAIT_MILLIS = 20_000L
private const val POLL_MILLIS = 50L

private const val QUIET_MILLIS = 750L
