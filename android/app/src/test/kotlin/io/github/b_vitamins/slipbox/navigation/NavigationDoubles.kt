/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import io.github.b_vitamins.slipbox.engine.GenerationBinding

internal const val ALPHA = "0123456789abcdef0123456789abcdef"
internal const val BETA = "fedcba9876543210fedcba9876543210"

internal fun bind(source: String, generation: String = "g1") =
    GenerationBinding(source, generation)

internal fun note(source: String, key: String, generation: String = "g1") =
    SlipboxRoute.Reader(BoundNote(bind(source, generation), key))

internal fun destinations(vararg surfaces: SlipboxSurface): SlipboxDestinations =
    slipboxDestinations {
        surface(SlipboxSurface.Library) { _, _ -> }
        for (declared in surfaces) {
            if (declared != SlipboxSurface.Library) surface(declared) { _, _ -> }
        }
    }

internal fun ready(vararg sources: Pair<String, String>): SourceGenerations {
    val generations = sources.toMap()
    return SourceGenerations { generations[it] }
}
