/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

sealed interface SlipboxDestination {

    /** Saved-state identity; keep stable across releases. */
    val id: String

    data object Library : SlipboxDestination {
        override val id: String = "library"
    }

    data object About : SlipboxDestination {
        override val id: String = "about"
    }

    companion object {
        val start: SlipboxDestination = Library

        private val byId: Map<String, SlipboxDestination> =
            listOf(Library, About).associateBy(SlipboxDestination::id)

        fun fromId(id: String): SlipboxDestination? = byId[id]
    }
}
