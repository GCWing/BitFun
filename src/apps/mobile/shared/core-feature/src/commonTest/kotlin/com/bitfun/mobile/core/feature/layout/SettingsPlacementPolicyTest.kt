package com.bitfun.mobile.core.feature.layout

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse

class SettingsPlacementPolicyTest {
    @Test
    fun phoneUsesBottomSheet() {
        assertEquals(SettingsPlacementMode.BOTTOM, resolve(width = 390, height = 844).mode)
    }

    @Test
    fun tabletUsesClampedTrailingSideSheet() {
        assertEquals(SettingsPlacement(SettingsPlacementMode.SIDE, 400, 760, 0), resolve(1024, 1366))
        assertEquals(SettingsPlacement(SettingsPlacementMode.SIDE, 520, 760, 0), resolve(2000, 1200))
    }

    @Test
    fun scarceLandscapeUsesWiderSideSheet() {
        assertEquals(SettingsPlacement(SettingsPlacementMode.SIDE, 360, 560, 0), resolve(800, 500))
    }

    @Test
    fun sideSheetFitsTrailingPhysicalLeaf() {
        val input = input(1000, 800, vertical = listOf(WindowCrease(480, 20)))
        val placement = SettingsPlacementPolicy.resolve(input, SettingsSheetKind.SETTINGS)
        assertEquals(SettingsPlacement(SettingsPlacementMode.SIDE, 500, 720, 0), placement)
        assertFalse(
            SettingsPlacementPolicy.sheetIntersectsVerticalCrease(
                placement,
                input.verticalCreases[0],
                1000,
                false,
            ),
        )
    }

    @Test
    fun rtlSideSheetFitsLeadingPhysicalLeaf() {
        val input = input(1000, 800, vertical = listOf(WindowCrease(480, 20)), isRtl = true)
        val placement = SettingsPlacementPolicy.resolve(input, SettingsSheetKind.SETTINGS)
        assertEquals(SettingsPlacement(SettingsPlacementMode.SIDE, 480, 720, 0), placement)
        assertEquals(0, SettingsPlacementPolicy.sheetLeft(placement, 1000, isRtl = true))
    }

    @Test
    fun hoverUsesOperateRegionAndKindCap() {
        val input = input(
            width = 700,
            height = 900,
            hover = true,
            horizontal = listOf(HorizontalWindowCrease(420, 20)),
        )
        assertEquals(
            SettingsPlacement(SettingsPlacementMode.FOLD_OPERATE, 0, 460, 460),
            SettingsPlacementPolicy.resolve(input, SettingsSheetKind.SETTINGS),
        )
        assertEquals(
            SettingsPlacement(SettingsPlacementMode.FOLD_OPERATE, 0, 460, 460),
            SettingsPlacementPolicy.resolve(input, SettingsSheetKind.SESSION_DETAILS),
        )
    }

    @Test
    fun invalidCreaseFallsBackWithoutExceedingViewport() {
        val placement = SettingsPlacementPolicy.resolve(
            input(640, 900, vertical = listOf(WindowCrease(640, 20))),
            SettingsSheetKind.SETTINGS,
        )
        assertEquals(SettingsPlacement(SettingsPlacementMode.SIDE, 400, 760, 0), placement)
    }

    @Test
    fun foldedWindowUsesBottomEvenWhenWide() {
        val placement = SettingsPlacementPolicy.resolve(
            input(900, 900, folded = true),
            SettingsSheetKind.REMOTE_VIEW_SETTINGS,
        )
        assertEquals(SettingsPlacement(SettingsPlacementMode.BOTTOM, 0, 520, 0), placement)
    }

    private fun resolve(width: Int, height: Int): SettingsPlacement =
        SettingsPlacementPolicy.resolve(input(width, height), SettingsSheetKind.SETTINGS)

    private fun input(
        width: Int,
        height: Int,
        folded: Boolean = false,
        hover: Boolean = false,
        vertical: List<WindowCrease> = emptyList(),
        horizontal: List<HorizontalWindowCrease> = emptyList(),
        isRtl: Boolean = false,
    ): AdaptiveLayoutInput = AdaptiveLayoutInput(
        viewportWidth = width,
        viewportHeight = height,
        isFolded = folded,
        isExpandedFoldable = vertical.isNotEmpty() || horizontal.isNotEmpty(),
        isHoverOperate = hover,
        wideLayoutMatched = width >= ConversationLayoutPolicy.MD_MIN_WIDTH,
        verticalCreases = vertical,
        horizontalCreases = horizontal,
        isRtl = isRtl,
    )
}
