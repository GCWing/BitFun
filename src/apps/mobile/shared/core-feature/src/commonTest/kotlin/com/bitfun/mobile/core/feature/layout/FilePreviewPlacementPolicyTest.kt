package com.bitfun.mobile.core.feature.layout

import kotlin.test.Test
import kotlin.test.assertEquals

private val TRI_FOLD = listOf(WindowCrease(left = 460, width = 12), WindowCrease(left = 930, width = 12))

/** Every pane and every gap, which must be the window and nothing else. */
private fun FilePreviewLayout.spannedWidth(): Int =
    masterPaneWidth + masterConversationGap + conversationPaneWidth +
        conversationPreviewGap + previewPaneWidth

class FilePreviewPlacementPolicyTest {
    @Test
    fun nothingToShowIsNoGeometryAtAll() {
        val layout = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = false,
            largeScreenLayout = true,
            viewportWidth = 1400,
            creases = TRI_FOLD,
        )

        assertEquals(FilePreviewLayout(FilePreviewPlacement.Hidden), layout)
    }

    /**
     * On a phone the preview covers the conversation, and both panes are the
     * whole window: the layout is a crossfade, not a split, so each has to be
     * able to hold the page on its own.
     */
    @Test
    fun aPhoneCoversTheConversationWithThePreview() {
        val layout = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = false,
            viewportWidth = 411,
            creases = emptyList(),
        )

        assertEquals(FilePreviewPlacement.CompactFullPage, layout.placement)
        assertEquals(411, layout.conversationPaneWidth)
        assertEquals(411, layout.previewPaneWidth)
        assertEquals(0, layout.masterPaneWidth)
    }

    @Test
    fun aWideFlatScreenSplitsThreeWaysAroundTheListItAlreadyHad() {
        val layout = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = true,
            viewportWidth = 1400,
            creases = emptyList(),
            preferredMasterWidth = 344,
        )

        assertEquals(FilePreviewPlacement.WideTriplePane, layout.placement)
        assertEquals(344, layout.masterPaneWidth, "the list must not jump when a preview opens")
        assertEquals(1, layout.masterConversationGap)
        assertEquals(527, layout.conversationPaneWidth)
        assertEquals(527, layout.previewPaneWidth)
        assertEquals(1400, layout.spannedWidth())
    }

    @Test
    fun thePreferredListWidthIsClampedFromBothSides() {
        val tooNarrow = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = true,
            viewportWidth = 1400,
            creases = emptyList(),
            preferredMasterWidth = 100,
        )
        assertEquals(FilePreviewPlacementPolicy.MIN_MASTER_WIDTH, tooNarrow.masterPaneWidth)

        // 1010 is two dp over the minimum for three panes, so the list gets the
        // two dp and both documents get exactly their floor.
        val tooWide = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = true,
            viewportWidth = 1010,
            creases = emptyList(),
            preferredMasterWidth = 600,
        )
        assertEquals(288, tooWide.masterPaneWidth)
        assertEquals(FilePreviewPlacementPolicy.MIN_CONVERSATION_WIDTH, tooWide.conversationPaneWidth)
        assertEquals(FilePreviewPlacementPolicy.MIN_PREVIEW_WIDTH, tooWide.previewPaneWidth)
        assertEquals(1010, tooWide.spannedWidth())
    }

    /** Not enough for three documents, so the list is what goes. */
    @Test
    fun aWindowThatCannotHoldThreePanesGivesUpTheList() {
        val layout = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = true,
            viewportWidth = 900,
            creases = emptyList(),
        )

        assertEquals(FilePreviewPlacement.WideFocusSplit, layout.placement)
        assertEquals(0, layout.masterPaneWidth)
        assertEquals(449, layout.conversationPaneWidth)
        assertEquals(450, layout.previewPaneWidth, "the odd dp goes to the document being read")
        assertEquals(900, layout.spannedWidth())
    }

    @Test
    fun aWindowThatCannotHoldTwoFallsBackToCovering() {
        val layout = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = true,
            viewportWidth = 700,
            creases = emptyList(),
        )

        assertEquals(FilePreviewPlacement.CompactFullPage, layout.placement)
        assertEquals(700, layout.conversationPaneWidth)
        assertEquals(700, layout.previewPaneWidth)
    }

    /** Three panels, three panes, and no divider drawn where the device has a hinge. */
    @Test
    fun aTriFoldPutsEachPaneOnItsOwnPanel() {
        val layout = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = true,
            viewportWidth = 1400,
            creases = TRI_FOLD,
            preferredMasterWidth = 344,
        )

        assertEquals(FilePreviewPlacement.WideTriplePane, layout.placement)
        // The hinges, not the preference: the device's proportions win.
        assertEquals(460, layout.masterPaneWidth)
        assertEquals(12, layout.masterConversationGap)
        assertEquals(458, layout.conversationPaneWidth)
        assertEquals(12, layout.conversationPreviewGap)
        assertEquals(458, layout.previewPaneWidth)
        assertEquals(1400, layout.spannedWidth())
    }

    /**
     * Hinges that carve out a panel too small to be a list are declined rather
     * than argued with — and a hinged window never falls back to drawn
     * dividers, because a divider next to a fold is a seam the device already has.
     */
    @Test
    fun panelsTheContentDoesNotFitAreDeclinedNotResized() {
        val lopsided = listOf(WindowCrease(left = 200, width = 12), WindowCrease(left = 930, width = 12))

        val layout = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = true,
            viewportWidth = 1400,
            creases = lopsided,
        )

        assertEquals(FilePreviewPlacement.WideFocusSplit, layout.placement)
        assertEquals(0, layout.masterPaneWidth)
        assertEquals(1400, layout.spannedWidth())
    }

    @Test
    fun theShorterOverloadsAgreeWithTheLongOne() {
        val full = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = true,
            viewportWidth = 1400,
            creases = emptyList(),
            preferredMasterWidth = FilePreviewPlacementPolicy.MIN_MASTER_WIDTH,
        )
        val short = FilePreviewPlacementPolicy.resolveLayout(
            previewVisible = true,
            largeScreenLayout = true,
            viewportWidth = 1400,
            creases = emptyList(),
        )

        assertEquals(full, short)
        assertEquals(
            full.placement,
            FilePreviewPlacementPolicy.resolve(
                previewVisible = true,
                largeScreenLayout = true,
                viewportWidth = 1400,
                creases = emptyList(),
            ),
        )
    }
}
