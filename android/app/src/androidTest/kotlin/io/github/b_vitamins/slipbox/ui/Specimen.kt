/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.content.Context
import android.content.ContextWrapper
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.DarkMode
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.FontScale
import androidx.compose.ui.test.ForcedSize
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.then
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferenceFault
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferences
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesRecord
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesStore
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale
import java.io.File

/**
 * The one synthetic note every visual case is measured against: a title, prose
 * carrying a link and an inline fragment, both headings, a source line wider than
 * the measure, display math and the control that opens the appearance reveal.
 */
internal object Specimen {
    const val TITLE = "Fixed points"
    const val HEADING = "The settled column"
    const val LEAD = "A note is set in one measured column, so a line keeps its rhythm. "
    const val LINK = "Another note"
    const val JOIN = " carries the argument on. Its identifier is "
    const val IDENTIFIER = "20260914T090000"
    const val TAIL = ", which the index resolves."
    // Both are set wider than the widest measure, so every case exercises the scroll.
    const val CODE =
        "fun settle(column: Column): Column = column.copy(state = Resting, measure = 625, rule = 1)"
    const val MATH =
        "\\int_0^1 x^2 \\, dx = \\frac{1}{3} \\quad \\text{and} \\quad " +
            "\\sum_{n=1}^{\\infty} \\frac{1}{n^2} = \\frac{\\pi^2}{6}"

    const val SURFACE = "specimen-surface"
    const val COLUMN = "specimen-column"
    const val HEAD = "specimen-heading"
    const val PROSE = "specimen-prose"
    const val BLOCK = "specimen-block"
    const val DISPLAY = "specimen-math"
}

/** The specimen, built from the same components the ordinary surfaces are built from. */
@Composable
internal fun SpecimenNote(
    settings: ReadingSettings,
    modifier: Modifier = Modifier,
    onBack: () -> Unit = {},
    onLink: () -> Unit = {},
) {
    var revealed by rememberSaveable { mutableStateOf(false) }
    val opened = remember { FocusRequester() }
    val motion = SlipboxMotion(rememberPlatformMotionScale(), settings.preferences.reduceMotion)
    val linkStyles = proseLinkStyles()
    val codeStyle = inlineCodeSpan()
    val prose = remember(linkStyles, codeStyle) { specimenProse(linkStyles, codeStyle, onLink) }
    ReadingSurface(
        title = Specimen.TITLE,
        modifier = modifier.testTag(Specimen.SURFACE),
        obscured = revealed,
        leading = {
            IconControl(
                icon = painterResource(R.drawable.ic_back),
                label = stringResource(R.string.action_back),
                onClick = onBack,
            )
        },
        trailing = {
            TextControl(
                label = stringResource(R.string.action_appearance),
                onClick = { revealed = true },
                modifier = Modifier.focusRequester(opened),
            )
        },
        overlay = {
            AppearanceSheet(
                visible = revealed,
                settings = settings,
                motion = motion,
                onDismiss = { revealed = false },
                restoreFocusTo = opened,
            )
        },
    ) {
        // Filled rather than wrapped, so this is the column itself and not its widest line.
        Column(
            modifier = Modifier.fillMaxWidth().testTag(Specimen.COLUMN),
            verticalArrangement = Arrangement.spacedBy(SlipboxDimensions.readingPadding),
        ) {
            Text(
                text = Specimen.HEADING,
                style = MaterialTheme.typography.headlineLarge,
                color = SlipboxTheme.colors.ink,
                modifier = Modifier.testTag(Specimen.HEAD),
            )
            ProseText(text = prose, modifier = Modifier.testTag(Specimen.PROSE))
            MonoBlock(text = Specimen.CODE, modifier = Modifier.testTag(Specimen.BLOCK))
            MathBlock(source = Specimen.MATH, modifier = Modifier.testTag(Specimen.DISPLAY))
        }
    }
}

/** One cell of the required matrix: a measure, a scheme and a platform font scale. */
internal data class VisualCase(
    val label: String,
    val width: Dp,
    val height: Dp,
    val dark: Boolean,
    val fontScale: Float,
)

/** Every cell of it: narrow and wide, light and dark, unscaled and doubled. */
internal val VISUAL_CASES: List<VisualCase> =
    buildList {
        for ((width, height) in listOf(320.dp to 640.dp, 900.dp to 700.dp)) {
            for (dark in listOf(false, true)) {
                for (fontScale in listOf(1f, 2f)) {
                    val scheme = if (dark) "dark" else "light"
                    val scale = if (fontScale == 1f) "1_0" else "2_0"
                    add(
                        VisualCase(
                            label = "${width.value.toInt()}-$scheme-$scale",
                            width = width,
                            height = height,
                            dark = dark,
                            fontScale = fontScale,
                        ),
                    )
                }
            }
        }
    }

/** One cell's device configuration, without a theme: a mount brings its own. */
@Composable
internal fun VisualCase.Overridden(content: @Composable () -> Unit) {
    DeviceConfigurationOverride(
        DeviceConfigurationOverride.ForcedSize(DpSize(width, height))
            then DeviceConfigurationOverride.FontScale(fontScale)
            then DeviceConfigurationOverride.DarkMode(dark),
        content = content,
    )
}

/**
 * The content of one cell. [appearance] defaults to the platform's own scheme, so a
 * capture shows the production resolution of a reader who has chosen nothing.
 */
@Composable
internal fun VisualCase.Content(
    appearance: SlipboxAppearance = SlipboxAppearance.System,
    content: @Composable () -> Unit,
) {
    Overridden { SlipboxTheme(appearance = appearance, content = content) }
}

/** A subtree a reveal has withheld: still composed, and not offered to accessibility. */
internal val WITHHELD: SemanticsMatcher =
    SemanticsMatcher.keyIsDefined(SemanticsProperties.HideFromAccessibility)

/** The page one resolved scheme sets its columns on. */
internal fun paper(dark: Boolean): Color =
    Color(if (dark) SlipboxTokens.Palette.PAPER_DARK else SlipboxTokens.Palette.PAPER_LIGHT)

/** The tone a column is painted in, which is the canvas a document mounts on. */
internal fun surface(dark: Boolean): Color =
    Color(if (dark) SlipboxTokens.Palette.SURFACE_DARK else SlipboxTokens.Palette.SURFACE_LIGHT)

/** A store that answers from memory and keeps what it was asked to record. */
internal class MemoryStore(
    private var record: ReadingPreferencesRecord = ReadingPreferencesRecord.Absent,
    private val fault: ReadingPreferenceFault? = null,
) : ReadingPreferencesStore {

    val written = mutableListOf<ReadingPreferences>()

    override fun read(): ReadingPreferencesRecord = record

    override fun write(preferences: ReadingPreferences): ReadingPreferenceFault? {
        if (fault != null) return fault
        written += preferences
        record = ReadingPreferencesRecord.Stored(preferences)
        return null
    }
}

/**
 * A context that answers with a directory of the suite's own and counts being asked for
 * it. The record the device's reader owns is left where it is, neither read nor written,
 * and the count is what shows that the ask is made once per context rather than per frame.
 */
internal class RecordingContext(base: Context, private val directory: File) :
    ContextWrapper(base) {

    var asked = 0
        private set

    override fun getNoBackupFilesDir(): File {
        asked++
        directory.mkdirs()
        return directory
    }
}

private fun specimenProse(
    linkStyles: TextLinkStyles,
    codeStyle: SpanStyle,
    onLink: () -> Unit,
): AnnotatedString =
    buildAnnotatedString {
        append(Specimen.LEAD)
        withLink(LinkAnnotation.Clickable(Specimen.LINK, linkStyles) { onLink() }) {
            append(Specimen.LINK)
        }
        append(Specimen.JOIN)
        withStyle(codeStyle) { append(Specimen.IDENTIFIER) }
        append(Specimen.TAIL)
    }
