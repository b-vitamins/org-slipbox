/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DocumentOriginTest {

    @Test
    fun theOriginIsTheReservedLocalOneOverHttps() {
        assertEquals("https://appassets.androidplatform.net", DocumentOrigin.ORIGIN)
        assertEquals(
            "https://appassets.androidplatform.net/bundle/index.html",
            DocumentOrigin.PAGE,
        )
        assertEquals(
            "https://appassets.androidplatform.net/asset/$TOKEN/",
            DocumentOrigin.assetBase(TOKEN),
        )
    }

    @Test
    fun onlyAPackagedFileIsServedFromTheBundle() {
        assertEquals("index.html", bundleEntry("index.html", PACKAGED))
        assertEquals(FONT, bundleEntry(FONT, PACKAGED))
        assertNull(bundleEntry("host.html", PACKAGED))
        assertNull(bundleEntry("", PACKAGED))
    }

    @Test
    fun aPathThatWalksAddressesNothing() {
        for (path in
            listOf(
                "../../../../etc/hosts",
                "..%2F..%2Fetc%2Fhosts",
                "assets/../../index.html",
                "assets//KaTeX_Main-Regular-D0ONP0R2.woff2",
                "/index.html",
                ".hidden",
                "index.html ",
            )
        ) {
            assertNull("$path was addressable", bundleEntry(path, PACKAGED))
        }
    }

    @Test
    fun onlyTheTypesTheDocumentNeedsAreServed() {
        assertEquals("text/html", bundleMimeType("index.html"))
        assertEquals("text/javascript", bundleMimeType("document.js"))
        assertEquals("text/css", bundleMimeType("document.css"))
        assertEquals("application/json", bundleMimeType("assets.json"))
        assertEquals("font/woff2", bundleMimeType(FONT))
        assertEquals("font/woff2", bundleMimeType("assets/KaTeX_Main-Regular-D0ONP0R2.WOFF2"))
        assertNull(bundleMimeType("diagram.svg"))
        assertNull(bundleMimeType("payload.wasm"))
        assertNull(bundleMimeType("LICENCE"))
    }

    @Test
    fun textIsServedAsUtf8AndAFontAsBytes() {
        assertEquals("utf-8", bundleEncoding("text/html"))
        assertEquals("utf-8", bundleEncoding("text/javascript"))
        assertEquals("utf-8", bundleEncoding("application/json"))
        assertNull(bundleEncoding("font/woff2"))
        assertNull(bundleEncoding("image/png"))
    }

    @Test
    fun anAssetRequestNamesOneMountAndOneTarget() {
        assertEquals(
            DocumentAssetRequest(TOKEN, "diagram.png"),
            documentAssetRequest("$TOKEN/diagram.png"),
        )
        assertEquals(
            DocumentAssetRequest(TOKEN, "figures/diagram.png"),
            documentAssetRequest("$TOKEN/figures/diagram.png"),
        )
    }

    @Test
    fun anAssetRequestNamingNoLiveMountOrWalkingOutIsRefused() {
        for (path in
            listOf(
                "diagram.png",
                "$TOKEN/",
                "$TOKEN/../../index.html",
                "$TOKEN/figures/../../../etc/hosts",
                "$TOKEN/./diagram.png",
                "$TOKEN//diagram.png",
                "${TOKEN.uppercase()}/diagram.png",
                "not-a-token/diagram.png",
                "$TOKEN-more/diagram.png",
                "/$TOKEN/diagram.png",
            )
        ) {
            assertNull("$path was answered", documentAssetRequest(path))
        }
    }

    @Test
    fun anAssetIsSomethingAReaderLooksAtAndNothingThatRuns() {
        assertEquals("image/png", admittedAssetType("image/png"))
        assertEquals("image/jpeg", admittedAssetType("IMAGE/JPEG"))
        assertEquals("text/plain", admittedAssetType("text/plain; charset=utf-8"))
        for (declared in
            listOf(
                "text/html",
                "image/svg+xml",
                "text/javascript",
                "application/pdf",
                "application/octet-stream",
            )
        ) {
            assertNull("$declared was served", admittedAssetType(declared))
        }
    }

    private companion object {
        const val TOKEN = "3f2a9c81-4d5e-4f60-9a1b-0c2d3e4f5061"
        const val FONT = "assets/KaTeX_Main-Regular-D0ONP0R2.woff2"
        val PACKAGED =
            setOf("index.html", "host.css", "host.js", "document.js", "document.css", FONT)
    }
}
