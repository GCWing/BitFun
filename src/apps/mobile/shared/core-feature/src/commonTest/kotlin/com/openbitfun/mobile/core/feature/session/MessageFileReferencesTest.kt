package com.openbitfun.mobile.core.feature.session

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MessageFileReferencesTest {
    @Test
    fun takesBothTheLinkedAndTheBareReferenceAndNamesEachByItsFile() {
        val references = MessageFileReferenceProjector.project(
            "See [the readme](computer:///repo/README.md), then computer:///repo/src/main.kt.",
        )

        assertEquals(2, references.size)
        assertEquals("/repo/README.md", references[0].remotePath)
        // The link's own text is not the label: the source resolves with an empty
        // label so every card is named by the file, not by the prose around it.
        assertEquals("README.md", references[0].label)
        assertEquals("/repo/src/main.kt", references[1].remotePath)
        assertEquals("main.kt", references[1].label)
    }

    @Test
    fun oneFileNamedFourTimesIsOneCard() {
        val references = MessageFileReferenceProjector.project(
            "computer:///repo/a.kt and computer:///repo/a.kt\n\n- computer:///repo/a.kt",
        )

        assertEquals(1, references.size)
        assertEquals("/repo/a.kt", references[0].remotePath)
    }

    @Test
    fun listItemsAreReadAndTheCountStopsAtTheLimit() {
        val references = MessageFileReferenceProjector.project(
            (1..8).joinToString("\n") { "- computer:///repo/file$it.kt" },
        )

        assertEquals(MessageFileReferenceProjector.DEFAULT_LIMIT, references.size)
        assertEquals("/repo/file1.kt", references[0].remotePath)
        assertEquals("/repo/file4.kt", references[3].remotePath)
    }

    @Test
    fun leavesAloneWhatIsNotAFileOnTheDesktop() {
        val references = MessageFileReferenceProjector.project(
            "Read [the docs](https://example.com/README.md), run `src/main.kt`, " +
                "and see computer:// or the [anchor](#top).",
        )

        assertTrue(references.isEmpty())
    }

    @Test
    fun theCacheAnswersTheSameTextWithTheSameList() {
        val cache = MessageFileReferenceCache()
        val first = cache.referencesFor("computer:///repo/a.kt")
        val again = cache.referencesFor("computer:///repo/a.kt")
        val other = cache.referencesFor("computer:///repo/b.kt")

        assertTrue(first === again)
        assertEquals("/repo/b.kt", other.single().remotePath)
    }
}
