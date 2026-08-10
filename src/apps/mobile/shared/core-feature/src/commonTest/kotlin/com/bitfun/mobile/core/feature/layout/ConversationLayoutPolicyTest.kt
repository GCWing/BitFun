package com.bitfun.mobile.core.feature.layout

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** A tri-fold's two hinges, on a 1400 dp window with three roughly equal panels. */
private val TRI_FOLD = listOf(WindowCrease(left = 460, width = 12), WindowCrease(left = 930, width = 12))

class ConversationLayoutPolicyTest {
    @Test
    fun aPhoneWidthIsOnePaneWhateverTheDeviceClaims() {
        assertFalse(ConversationLayoutPolicy.useMasterDetail(719, true, emptyList()))
        assertTrue(ConversationLayoutPolicy.useMasterDetail(720, true, emptyList()))
    }

    @Test
    fun aFlatWindowAsksTheDevice() {
        assertTrue(ConversationLayoutPolicy.useMasterDetail(1000, true, emptyList()))
        assertFalse(ConversationLayoutPolicy.useMasterDetail(1000, false, emptyList()))
    }

    /**
     * One hinge is a book fold: splitting there would put the list on one leaf
     * and leave a master pane half the device wide. Two hinges is a tri-fold,
     * where the panels are already the panes.
     */
    @Test
    fun creasesOutrankTheDeviceInBothDirections() {
        val single = listOf(WindowCrease(left = 500, width = 20))
        assertFalse(ConversationLayoutPolicy.useMasterDetail(1000, true, single))
        assertTrue(ConversationLayoutPolicy.useMasterDetail(1400, false, TRI_FOLD))
    }

    /**
     * A hinge the window does not cover is not a hinge. Both edges count: a
     * crease at 0 is behind the window, and one flush with the far edge splits
     * nothing.
     */
    @Test
    fun aCreaseOutsideTheWindowIsNotACrease() {
        val outside = listOf(
            WindowCrease(left = 0, width = 12),
            WindowCrease(left = 1400, width = 12),
            WindowCrease(left = 1390, width = 10),
        )
        assertTrue(
            ConversationLayoutPolicy.useMasterDetail(1400, true, outside),
            "only the device's own answer was left to go on",
        )
        assertEquals(
            ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH,
            ConversationLayoutPolicy.resolveWideGeometry(1400, outside).masterPaneWidth,
        )
    }

    @Test
    fun aFlatScreenFallsBackToTheFixedMasterPane() {
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(1000, emptyList())

        assertEquals(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH, geometry.masterPaneWidth)
        assertEquals(0, geometry.masterDetailGap)
        // No hinge to dodge, so the detail content is the detail pane.
        assertEquals(0, geometry.detailContentOffset)
        assertEquals(1000 - ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH, geometry.detailContentWidth)
        assertEquals(0, geometry.collapsedDetailContentOffset)
        assertEquals(1000, geometry.collapsedDetailContentWidth)
    }

    @Test
    fun theSeamLandsOnTheHingeAndTheContentDodgesTheNextOne() {
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(1400, TRI_FOLD)

        assertEquals(460, geometry.masterPaneWidth)
        assertEquals(12, geometry.masterDetailGap)
        assertTrue(geometry.isExtraWide)
        // The detail pane runs 472..1400 and is cut by the second hinge into
        // 472..930 (458) and 942..1400 (458). Equal, so the later one wins, and
        // its offset is measured from the detail pane rather than the window.
        assertEquals(942 - 472, geometry.detailContentOffset)
        assertEquals(458, geometry.detailContentWidth)
        // With no master pane the first panel is 0..460, which is the widest of
        // the three by two dp.
        assertEquals(0, geometry.collapsedDetailContentOffset)
        assertEquals(460, geometry.collapsedDetailContentWidth)
    }

    /**
     * A hinge that would leave either pane unusable is skipped rather than
     * squeezed against — the fallback pane is a better master than 120 dp of it.
     */
    @Test
    fun aHingeTooCloseToAnEdgeIsNotASeam() {
        val nearEdge = listOf(WindowCrease(left = 120, width = 10))
        val geometry = ConversationLayoutPolicy.resolveWideGeometry(900, nearEdge)

        assertEquals(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH, geometry.masterPaneWidth)
        assertEquals(0, geometry.masterDetailGap)
        // Skipped as a seam, but still a hinge: it is behind the master pane, so
        // the detail pane it does not touch is one whole segment.
        assertEquals(0, geometry.detailContentOffset)
        assertEquals(900 - ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH, geometry.detailContentWidth)
    }

    @Test
    fun extraWideIsEitherASecondHingeOrEnoughRoom() {
        assertFalse(ConversationLayoutPolicy.resolveWideGeometry(1079, emptyList()).isExtraWide)
        assertTrue(ConversationLayoutPolicy.resolveWideGeometry(1080, emptyList()).isExtraWide)
        assertTrue(ConversationLayoutPolicy.resolveWideGeometry(1000, TRI_FOLD).isExtraWide)
    }

    /** Creases arrive in whatever order the platform enumerated its panels. */
    @Test
    fun creasesAreSortedBeforeAnythingIsMeasured() {
        val shuffled = listOf(TRI_FOLD[1], TRI_FOLD[0])

        assertEquals(
            ConversationLayoutPolicy.resolveWideGeometry(1400, TRI_FOLD),
            ConversationLayoutPolicy.resolveWideGeometry(1400, shuffled),
        )
    }
}
