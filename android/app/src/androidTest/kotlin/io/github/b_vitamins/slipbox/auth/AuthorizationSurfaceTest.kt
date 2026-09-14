/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.Content
import io.github.b_vitamins.slipbox.ui.Evidence
import io.github.b_vitamins.slipbox.ui.Record
import io.github.b_vitamins.slipbox.ui.VISUAL_CASES
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPanel
import io.github.b_vitamins.slipbox.ui.auth.GithubAuthorizationState
import io.github.b_vitamins.slipbox.ui.bounds
import io.github.b_vitamins.slipbox.ui.hex
import io.github.b_vitamins.slipbox.ui.paper
import io.github.b_vitamins.slipbox.ui.pixels
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class AuthorizationSurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private val requesting = context.getString(R.string.auth_requesting)
    private val codeName = context.getString(R.string.auth_code_name)
    private val instruction =
        context.getString(R.string.auth_verification_instruction, VERIFICATION_PAGE)
    private val open = context.getString(R.string.auth_action_open)
    private val cancel = context.getString(R.string.auth_action_cancel)
    private val retry = context.getString(R.string.auth_action_retry)
    private val install = context.getString(R.string.auth_action_install)
    private val unavailableBrowser = context.getString(R.string.auth_browser_unavailable)
    private val authorized = context.getString(R.string.auth_authorized, PROBE_LOGIN)
    private val notInstalled = context.getString(R.string.auth_not_installed)
    private val expired = context.getString(R.string.auth_expired)

    private val owners = mutableListOf<GithubAuthorizationOwner>()

    private lateinit var density: Density

    private var shown by mutableStateOf(LIGHT)

    @After
    fun releaseTheOwners() {
        owners.forEach { it.close() }
    }

    @Test
    fun anIdleSurfaceHasNoPanelAtAll() {
        show(stateOf())

        composeRule.onNodeWithText(requesting).assertDoesNotExist()
        composeRule.onNodeWithText(codeName).assertDoesNotExist()
        composeRule.onNodeWithText(cancel).assertDoesNotExist()
    }

    @Test
    fun aSurfaceAskingForACodeSaysOnlyThat() {
        val state = stateOf()
        show(state)

        composeRule.runOnIdle { state.begin() }
        composeRule.waitForIdle()

        composeRule.onNodeWithText(requesting).assertIsDisplayed()
        composeRule.onNodeWithText(codeName).assertDoesNotExist()
        capture("auth-requesting-light", "requesting")
    }

    @Test
    fun aVerifyingSurfaceShowsTheCodeTheWayToUseItAndTheWayOut() {
        val state = stateOf()
        show(state)

        verifying(state)

        composeRule.onNodeWithText(codeName).assertIsDisplayed()
        composeRule.onNodeWithText(PLACEHOLDER_CODE).assertIsDisplayed()
        composeRule.onNodeWithText(instruction).assertIsDisplayed()
        composeRule.onNodeWithText(cancel).assertIsDisplayed()
        val target = composeRule.onNodeWithText(open).bounds().height / density.density
        assertTrue(
            "the way to the browser keeps the touch floor: ${target}dp",
            target >= SlipboxTokens.Geometry.TOUCH_TARGET_DP - 1f,
        )
        val light = capture("auth-verifying-light", "verifying")
        assertEquals("the light page", hex(paper(dark = false)), hex(light))

        composeRule.runOnIdle { shown = DARK }
        composeRule.waitForIdle()
        composeRule.onNodeWithText(PLACEHOLDER_CODE).assertIsDisplayed()
        val dark = capture("auth-verifying-dark", "verifying")
        assertEquals("the dark page", hex(paper(dark = true)), hex(dark))
    }

    @Test
    fun aBrowserThatTookNothingIsSaidOnTheSurfaceWithTheCodeStillThere() {
        val state = stateOf()
        show(state)
        verifying(state)

        composeRule.runOnIdle { state.openVerification() }
        composeRule.waitForIdle()

        composeRule.onNodeWithText(unavailableBrowser).assertIsDisplayed()
        composeRule.onNodeWithText(PLACEHOLDER_CODE).assertIsDisplayed()
        capture("auth-verifying-no-browser-light", "verifying")
    }

    @Test
    fun anAuthorizedSurfaceNamesTheAccountAndOffersNothingFurther() {
        val state = stateOf()
        show(state)

        settle(state, AuthorizationOutcome.Authorized(syntheticAuthorization()))

        composeRule.onNodeWithText(authorized).assertIsDisplayed()
        composeRule.onNodeWithText(notInstalled).assertDoesNotExist()
        composeRule.onNodeWithText(retry).assertDoesNotExist()
        capture("auth-authorized-light", "authorized")
    }

    @Test
    fun anAccountWithNoInstallationIsToldSoAndOfferedThePage() {
        val state = stateOf()
        show(state)

        settle(
            state,
            AuthorizationOutcome.Authorized(
                syntheticAuthorization(installations = 0, selection = RepositorySelection.None),
            ),
        )

        composeRule.onNodeWithText(authorized).assertIsDisplayed()
        composeRule.onNodeWithText(notInstalled).assertIsDisplayed()
        composeRule.onNodeWithText(install).assertIsDisplayed()
        capture("auth-not-installed-light", "authorized")
    }

    @Test
    fun aBuildWithNoInstallationPageOffersNoInstallation() {
        val state = stateOf(installationUrl = null)
        show(state)

        settle(
            state,
            AuthorizationOutcome.Authorized(
                syntheticAuthorization(installations = 0, selection = RepositorySelection.None),
            ),
        )

        composeRule.onNodeWithText(notInstalled).assertIsDisplayed()
        composeRule.onNodeWithText(install).assertDoesNotExist()
    }

    @Test
    fun anExpiredCodeIsSaidOnceAndOffersToTryAgain() {
        val state = stateOf()
        show(state)

        settle(state, AuthorizationOutcome.Expired)

        composeRule.onNodeWithText(expired).assertIsDisplayed()
        composeRule.onNodeWithText(retry).assertIsDisplayed()
        composeRule.onNodeWithText(codeName).assertDoesNotExist()
        capture("auth-expired-light", "settled")
    }


    private fun verifying(state: GithubAuthorizationState) {
        composeRule.runOnIdle { state.onVerificationWaiting(syntheticGrant()) }
        composeRule.waitForIdle()
    }


    private fun settle(state: GithubAuthorizationState, outcome: AuthorizationOutcome) {
        composeRule.runOnIdle { state.onSettled(outcome) }
        composeRule.waitForIdle()
    }


    private fun show(state: GithubAuthorizationState) {
        composeRule.setContent {
            shown.Content {
                density = LocalDensity.current
                Column(
                    modifier =
                        Modifier.fillMaxSize()
                            .background(SlipboxTheme.colors.paper)
                            .padding(SlipboxDimensions.readingPadding),
                ) {
                    AuthorizationPanel(state = state)
                }
            }
        }
        composeRule.waitForIdle()
    }


    private fun stateOf(installationUrl: String? = INSTALLATION_PAGE): GithubAuthorizationState {
        val owner =
            GithubAuthorizationOwner(
                requireNotNull(GithubApp.of(PROBE_CLIENT_ID, INSTALLATION_PAGE)),
                AuthorizationTransport { AuthorizationReply.Failed("probe.no.transport") },
                ParkingClock(),
                AuthorizationDelivery { _ -> },
            )
        owners.add(owner)
        return GithubAuthorizationState(owner, BrowserHandoff { false }, installationUrl)
    }


    private fun capture(name: String, phase: String): Color {
        val image: ImageBitmap = composeRule.onRoot().captureToImage()
        Evidence.image(name, image)
        val pixels = image.pixels()
        val canvas = pixels[2, 2]
        Evidence.record(
            name,
            Record()
                .text("phase", phase)
                .flag("codeRedacted", true)
                .flag("syntheticValues", true)
                .colour("canvas", canvas)
                .size("density", density.density)
                .size("widthDp", pixels.width / density.density)
                .size("heightDp", pixels.height / density.density),
        )
        return canvas
    }

    private companion object {

        val LIGHT = VISUAL_CASES.first { !it.dark && it.fontScale == 1f }

        val DARK = VISUAL_CASES.first { it.dark && it.fontScale == 1f }
    }
}
