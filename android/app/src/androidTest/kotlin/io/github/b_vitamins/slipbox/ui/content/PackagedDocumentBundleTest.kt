/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.content.res.AssetManager
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.ui.Evidence
import io.github.b_vitamins.slipbox.ui.Record
import java.security.MessageDigest
import java.util.Locale
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PackagedDocumentBundleTest {

    private val assets: AssetManager =
        InstrumentationRegistry.getInstrumentation().targetContext.assets

    @Test
    fun everyDeclaredFileIsPackagedWithTheBytesTheBuildDeclared() {
        val inventory = inventory()
        val declared = inventory.getJSONArray(FILES)
        val host = inventory.getString(HOST)
        val record = Record().count("declared", declared.length()).text("host", host)
        var checked = 0
        for (index in 0 until declared.length()) {
            val file = declared.getJSONObject(index)
            val path = file.getString("path")
            if (path == host) {
                continue
            }
            val bytes = read(path)
            assertEquals("$path carries the declared length", file.getInt("bytes"), bytes.size)
            assertEquals("$path carries the declared digest", file.getString("sha256"), sha256(bytes))
            checked++
        }
        assertEquals("the renderer's entry", "document.js", inventory.getString("entry"))
        assertEquals("the renderer's stylesheet", "document.css", inventory.getString("stylesheet"))
        assertTrue("the inventory declares the bundle", checked > 0)
        Evidence.record("content-bundle", record.count("checked", checked))
    }

    @Test
    fun thePackagedTreeIsTheAppsOwnPageAndTheBundleBesideIt() {
        val packaged = packaged()
        val inventory = inventory()
        val declared =
            (0 until inventory.getJSONArray(FILES).length())
                .map { inventory.getJSONArray(FILES).getJSONObject(it).getString("path") }
                .toSet()
        val host = inventory.getString(HOST)
        assertTrue("the renderer declares a host page of its own", host in declared)
        assertFalse("the renderer's host page is packaged", host in packaged)
        assertEquals(
            "the packaged tree is the inventory, the app's own page and nothing else",
            declared - host + APP_FILES + INVENTORY,
            packaged,
        )
        for (path in packaged) {
            assertNotNull("$path is served as no type", bundleMimeType(path))
        }
        assertTrue("the fonts are packaged", packaged.count { it.endsWith(".woff2") } > 1)
    }

    private fun inventory(): JSONObject = JSONObject(String(read(INVENTORY), Charsets.UTF_8))

    private fun read(path: String): ByteArray =
        assets.open("${DocumentOrigin.BUNDLE_DIRECTORY}/$path").use { it.readBytes() }

    private fun packaged(): Set<String> {
        val found = sortedSetOf<String>()
        val pending = ArrayDeque(listOf(""))
        while (pending.isNotEmpty()) {
            val relative = pending.removeFirst()
            val here =
                if (relative.isEmpty()) {
                    DocumentOrigin.BUNDLE_DIRECTORY
                } else {
                    "${DocumentOrigin.BUNDLE_DIRECTORY}/$relative"
                }
            val children = assets.list(here).orEmpty()
            if (children.isEmpty() && relative.isNotEmpty()) {
                found.add(relative)
            }
            for (child in children) {
                pending.addLast(if (relative.isEmpty()) child else "$relative/$child")
            }
        }
        return found
    }

    private companion object {
        const val FILES = "files"
        const val HOST = "host"
        const val INVENTORY = "assets.json"

        val APP_FILES = setOf(DocumentOrigin.PAGE_FILE, "host.css", "host.js")

        fun sha256(bytes: ByteArray): String =
            MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") {
                String.format(Locale.ROOT, "%02x", it)
            }
    }
}
