/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sources.SourceCatalogListing
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
@OptIn(ExperimentalTestApi::class)
class SourceSettingsSurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun detailsAndDestructiveConsequencesStayOnTheSourceSurface() {
        composeRule.setContent {
            SlipboxTheme {
                SourceSettingsScreen(
                    sourceId = ALPHA,
                    catalog = listing(),
                    ready = ready(),
                    onBack = {},
                    onOpenSource = {},
                    onSelect = { _, _ -> },
                    onReady = { _, _ -> },
                    onCacheRemoved = {},
                    onSourceRemoved = { _, _, _ -> },
                )
            }
        }

        composeRule.onNodeWithText("Repository: https://github.com/owner/notes.git").assertIsDisplayed()
        composeRule
            .onNodeWithText("Ready revision: 012345678901")
            .performScrollTo()
            .assertIsDisplayed()
        composeRule
            .onNodeWithText("Freshness: Ready for offline reading")
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.regression("source-settings")

        composeRule.onNodeWithText("Remove downloaded data").performScrollTo().performClick()
        composeRule
            .onNodeWithText(
                "Remove owner/notes downloaded repository files, derived index, assets, " +
                    "bookmarks, and recent reading? The source configuration and credential stay.",
            )
            .assertExists()
        composeRule.onNodeWithText("Confirm").performScrollTo().assertIsDisplayed()
        composeRule.accessibleRegression("source-cache-confirmation")
    }

    @Test
    fun anotherConfiguredRepositoryOpensWithoutSelectingIt() {
        var opened: String? = null
        composeRule.setContent {
            SlipboxTheme {
                SourceSettingsScreen(
                    sourceId = ALPHA,
                    catalog = listing(),
                    ready = ready(),
                    onBack = {},
                    onOpenSource = { opened = it.id },
                    onSelect = { _, _ -> },
                    onReady = { _, _ -> },
                    onCacheRemoved = {},
                    onSourceRemoved = { _, _, _ -> },
                )
            }
        }

        composeRule.onNodeWithText("Public archive").performClick()
        composeRule.runOnIdle { assertEquals(BETA, opened) }
    }

    private fun listing(): SourceCatalogListing {
        val source = source()
        return SourceCatalogListing(
            revision = 4,
            sources = listOf(source, publicSource()),
            activeSource = source,
        )
    }

    private fun ready(): ReadySource =
        ReadySource(
            source = source(),
            binding = GenerationBinding(ALPHA, "generation-8"),
            revision = "0123456789012345678901234567890123456789",
            contentRoot = "/private/source",
            database = "/private/index/slipbox.db",
            stats = ReadySourceStats(4, 9, 5),
        )

    private fun source(): RefreshSource =
        RefreshSource(
            id = ALPHA,
            displayName = "owner/notes",
            provider = RefreshProvider.GITHUB,
            visibility = RefreshVisibility.PRIVATE,
            providerRepositoryId = "9001",
            account = "42",
            remote = "https://github.com/owner/notes.git",
            branch = "main",
            notesFolder = "notes",
            credential = "slipbox.source.$ALPHA",
        )

    private fun publicSource(): RefreshSource =
        RefreshSource(
            id = BETA,
            displayName = "Public archive",
            provider = RefreshProvider.GENERIC_HTTPS,
            visibility = RefreshVisibility.PUBLIC,
            remote = "https://example.com/archive.git",
            branch = "main",
            notesFolder = "",
        )

    private companion object {
        const val ALPHA = "0123456789abcdef0123456789abcdef"
        const val BETA = "fedcba9876543210fedcba9876543210"
    }
}
