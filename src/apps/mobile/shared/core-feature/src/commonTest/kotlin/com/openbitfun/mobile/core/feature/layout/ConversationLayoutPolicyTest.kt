package com.openbitfun.mobile.core.feature.layout

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

private val TRI_FOLD = listOf(
    WindowCrease(left = 352, width = 8),
    WindowCrease(left = 712, width = 8),
)

class ConversationLayoutPolicyTest {
    @Test
    fun aFlatWindowNeedsBothTheMdBoundaryAndTwoUsablePanes() {
        assertFalse(useMasterDetail(599, wideViewportMatched = false))
        assertFalse(useMasterDetail(600, wideViewportMatched = true))
        assertFalse(useMasterDetail(639, wideViewportMatched = true))
        assertTrue(useMasterDetail(640, wideViewportMatched = true))
        assertTrue(useMasterDetail(960, wideViewportMatched = false))
    }

    @Test
    fun foldedAndHoverSurfacesStayCompact() {
        assertFalse(useMasterDetail(900, wideViewportMatched = true, isFolded = true))
        assertFalse(useMasterDetail(900, wideViewportMatched = true, isHover = true))
    }

    @Test
    fun halfOpenedPortraitUsesHoverButLandscapeWideDoesNot() {
        assertTrue(ConversationLayoutPolicy.useHoverOperate(true, 480, 800))
        assertFalse(ConversationLayoutPolicy.useHoverOperate(true, 1107, 776))
        assertFalse(ConversationLayoutPolicy.useHoverOperate(false, 480, 800))
    }

    @Test
    fun oneUsableCreaseCanOwnTheMasterDetailBoundary() {
        val crease = listOf(WindowCrease(left = 520, width = 8))

        assertTrue(useMasterDetail(900, creases = crease))
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(900, crease)
        assertEquals(520, geometry.masterPaneWidth)
        assertEquals(8, geometry.masterDetailGap)
        assertEquals(372, geometry.detailContentWidth)
    }

    @Test
    fun expandedFoldableSynthesizesAMissingCenterHinge() {
        val synthetic = ConversationLayoutPolicy.syntheticCenterCrease(2210)

        assertEquals(WindowCrease(1097, 16), synthetic)
        assertNull(ConversationLayoutPolicy.syntheticCenterCrease(500))
        assertEquals(
            listOf(WindowCrease(1097, 16)),
            ConversationLayoutPolicy.effectiveVerticalCreases(2210, emptyList(), true),
        )
        assertTrue(useMasterDetail(2210, isExpandedFoldable = true))
    }

    @Test
    fun aReportedCreaseWinsOverTheSyntheticFallback() {
        val reported = listOf(WindowCrease(1100, 16))

        assertEquals(
            reported,
            ConversationLayoutPolicy.effectiveVerticalCreases(2210, reported, true),
        )
    }

    @Test
    fun aFlatScreenUsesTheAdaptiveFallbackMasterPane() {
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(820, emptyList())

        assertEquals(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH, geometry.masterPaneWidth)
        assertEquals(0, geometry.masterDetailGap)
        assertFalse(geometry.isExtraWide)
        assertEquals(0, geometry.detailContentOffset)
        assertEquals(476, geometry.detailContentWidth)
        assertEquals(0, geometry.collapsedDetailContentOffset)
        assertEquals(820, geometry.collapsedDetailContentWidth)

        val minimum = ConversationLayoutPolicy.resolveWideGeometry(640, emptyList())
        assertEquals(280, minimum.masterPaneWidth)
        assertEquals(360, minimum.detailContentWidth)
    }

    @Test
    fun aNarrowViewportHasNoDetailBandButKeepsFullCollapsedContent() {
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(300, emptyList())

        assertEquals(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH, geometry.masterPaneWidth)
        assertEquals(0, geometry.detailContentWidth)
        assertEquals(0, geometry.collapsedDetailContentOffset)
        assertEquals(300, geometry.collapsedDetailContentWidth)
    }

    @Test
    fun aBalancedBookFoldKeepsCollapsedContentAtTheWindowOrigin() {
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(
            2210,
            listOf(WindowCrease(1097, 16)),
        )

        assertEquals(1097, geometry.masterPaneWidth)
        assertEquals(16, geometry.masterDetailGap)
        assertEquals(1097, geometry.detailContentWidth)
        assertEquals(0, geometry.collapsedDetailContentOffset)
        assertEquals(2210, geometry.collapsedDetailContentWidth)
    }

    @Test
    fun anAsymmetricFoldKeepsCollapsedContentOnTheDominantBand() {
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(
            1107,
            listOf(WindowCrease(351, 44)),
        )

        assertEquals(351, geometry.masterPaneWidth)
        assertEquals(44, geometry.masterDetailGap)
        assertEquals(712, geometry.detailContentWidth)
        assertEquals(395, geometry.collapsedDetailContentOffset)
        assertEquals(712, geometry.collapsedDetailContentWidth)
    }

    @Test
    fun triFoldDetailSpansBothRemainingPanels() {
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(1080, TRI_FOLD)

        assertEquals(352, geometry.masterPaneWidth)
        assertEquals(8, geometry.masterDetailGap)
        assertTrue(geometry.isExtraWide)
        assertEquals(0, geometry.detailContentOffset)
        assertEquals(720, geometry.detailContentWidth)
        assertEquals(360, geometry.collapsedDetailContentOffset)
        assertEquals(720, geometry.collapsedDetailContentWidth)
    }

    @Test
    fun extraWideUsesTheLgBoundaryOrMultipleCreases() {
        assertFalse(ConversationLayoutPolicy.resolveWideGeometry(839, emptyList()).isExtraWide)
        assertTrue(ConversationLayoutPolicy.resolveWideGeometry(840, emptyList()).isExtraWide)
        assertTrue(ConversationLayoutPolicy.resolveWideGeometry(800, TRI_FOLD).isExtraWide)
    }

    @Test
    fun anUnusableOrOutsideCreaseFallsBackWithoutChangingDetailGeometry() {
        val expected = ConversationLayoutPolicy.resolveWideGeometry(900, emptyList())
        val unusable = listOf(
            WindowCrease(-12, 8),
            WindowCrease(120, 10),
            WindowCrease(420, -2),
            WindowCrease(910, 8),
        )
        val actual = ConversationLayoutPolicy.resolveWideGeometry(900, unusable)

        assertEquals(expected.masterPaneWidth, actual.masterPaneWidth)
        assertEquals(expected.masterDetailGap, actual.masterDetailGap)
        assertEquals(expected.detailContentWidth, actual.detailContentWidth)
    }

    @Test
    fun zeroWidthCreaseIsAValidSeam() {
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(
            900,
            listOf(WindowCrease(352, 0)),
        )

        assertEquals(352, geometry.masterPaneWidth)
        assertEquals(0, geometry.masterDetailGap)
        assertEquals(548, geometry.detailContentWidth)
    }

    @Test
    fun creasesAreSortedBeforeTheFirstUsableSeamIsChosen() {
        assertEquals(
            ConversationLayoutPolicy.resolveWideGeometry(1080, TRI_FOLD),
            ConversationLayoutPolicy.resolveWideGeometry(1080, TRI_FOLD.reversed()),
        )
    }

    private fun useMasterDetail(
        viewportWidth: Int,
        wideViewportMatched: Boolean = viewportWidth >= ConversationLayoutPolicy.MD_MIN_WIDTH,
        isFolded: Boolean = false,
        creases: List<WindowCrease> = emptyList(),
        isExpandedFoldable: Boolean = false,
        isHover: Boolean = false,
    ): Boolean = ConversationLayoutPolicy.useMasterDetail(
        viewportWidth = viewportWidth,
        wideViewportMatched = wideViewportMatched,
        isFolded = isFolded,
        creases = creases,
        isExpandedFoldable = isExpandedFoldable,
        isHover = isHover,
    )
}
