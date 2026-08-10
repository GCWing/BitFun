package com.bitfun.mobile.core.feature.shell

import kotlin.test.Test
import kotlin.test.assertEquals

class SidebarPresentationTest {
    @Test
    fun aPinnedConversationLeavesTheRecentListRatherThanAppearingTwice() {
        val sections = SidebarPresentation.sections(
            listOf(row("a", "Alpha", pinned = true), row("b", "Beta")),
            query = "",
        )

        assertEquals(listOf("a"), sections.pinned.map { it.id })
        assertEquals(listOf("b"), sections.recent.map { it.id })
    }

    @Test
    fun archivingWinsOverPinning() {
        // Filed away and held above the list are opposite claims about one row;
        // the sidebar has to pick one, and the archive is the later decision.
        val sections = SidebarPresentation.sections(
            listOf(row("a", "Alpha", pinned = true, status = "archived")),
            query = "",
        )

        assertEquals(emptyList(), sections.pinned.map { it.id })
        assertEquals(emptyList(), sections.recent.map { it.id })
        assertEquals(listOf("a"), sections.archived.map { it.id })
    }

    @Test
    fun searchIsCaseInsensitiveAndSpansEverySection() {
        val sections = SidebarPresentation.sections(
            listOf(
                row("a", "Migration notes", pinned = true),
                row("b", "MIGRATION plan"),
                row("c", "Old migration", status = "archived"),
                row("d", "Something else"),
            ),
            query = "  MiGrAtIoN ",
        )

        assertEquals(listOf("a"), sections.pinned.map { it.id })
        assertEquals(listOf("b"), sections.recent.map { it.id })
        assertEquals(listOf("c"), sections.archived.map { it.id })
    }

    @Test
    fun theArchiveCountIgnoresTheSearch() {
        // Otherwise a query that matches nothing archived would report an empty
        // archive, which is a different claim from "nothing here matches".
        val sections = SidebarPresentation.sections(
            listOf(
                row("a", "Alpha", status = "archived"),
                row("b", "Beta", status = "archived"),
                row("c", "Beta"),
            ),
            query = "beta",
        )

        assertEquals(listOf("b"), sections.archived.map { it.id })
        assertEquals(2, sections.archivedCount)
    }

    @Test
    fun aStatusTheStoreCasedDifferentlyStillReadsAsArchived() {
        val sections = SidebarPresentation.sections(
            listOf(row("a", "Alpha", status = "Archived")),
            query = "",
        )

        assertEquals(listOf("a"), sections.archived.map { it.id })
    }

    private fun row(
        id: String,
        title: String,
        pinned: Boolean = false,
        status: String = "ready",
    ) = SidebarSessionRow(
        id = id,
        title = title,
        status = status,
        pinned = pinned,
        createdAt = "2026-08-09T00:00:00Z",
        updatedAt = "2026-08-09T00:00:01Z",
        messageCount = 2,
    )
}
