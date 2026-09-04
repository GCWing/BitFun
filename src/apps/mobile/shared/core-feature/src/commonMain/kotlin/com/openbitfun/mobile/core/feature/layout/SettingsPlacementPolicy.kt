package com.openbitfun.mobile.core.feature.layout

/** A horizontal hinge crossing the window, in density-independent units. */
public data class HorizontalWindowCrease(
    val top: Int,
    val height: Int,
)

/** Platform-neutral window facts consumed by overlay placement policies. */
public data class AdaptiveLayoutInput(
    val viewportWidth: Int,
    val viewportHeight: Int,
    val isFolded: Boolean,
    val isExpandedFoldable: Boolean,
    val isHoverOperate: Boolean,
    val wideLayoutMatched: Boolean,
    val verticalCreases: List<WindowCrease>,
    val horizontalCreases: List<HorizontalWindowCrease>,
    val isRtl: Boolean,
)

public enum class SettingsPlacementMode {
    BOTTOM,
    SIDE,
    FOLD_OPERATE,
}

public enum class SettingsSheetKind {
    SETTINGS,
    CONNECT,
    SESSION_DETAILS,
    REMOTE_VIEW_SETTINGS,
}

public data class SettingsPlacement(
    val mode: SettingsPlacementMode,
    val width: Int,
    val height: Int,
    val maxHeight: Int,
)

/**
 * Overlay placement shared by HarmonyOS, Android and iOS.
 *
 * This is a direct port of HarmonyOS `SettingsPlacementPolicy.ets`. Platform
 * adapters still own their native sheet/popover lifecycle; the shared policy
 * only decides which physical region may contain the surface.
 */
public object SettingsPlacementPolicy {
    public const val TABLET_MIN_WIDTH: Int = 400
    public const val TABLET_MAX_WIDTH: Int = 520
    public const val SIDE_MAX_HEIGHT: Int = 760
    public const val SIDE_MIN_HEIGHT: Int = 560
    public const val SIDE_VERTICAL_MARGIN: Int = 80
    public const val SESSION_DETAILS_HEIGHT: Int = 560
    public const val REMOTE_VIEW_SETTINGS_HEIGHT: Int = 520

    private const val TABLET_WIDTH_RATIO: Double = 0.32
    private const val LANDSCAPE_WIDTH_RATIO: Double = 0.45

    public fun compactBottom(kind: SettingsSheetKind): SettingsPlacement = bottomPlacement(kind)

    public fun resolve(input: AdaptiveLayoutInput, kind: SettingsSheetKind): SettingsPlacement {
        if (input.isHoverOperate) return hoverPlacement(input, kind)
        if (input.isFolded) return bottomPlacement(kind)
        return creaseFreeOrSidePlacement(input, kind)
    }

    /** The leading coordinate of ArkUI/Compose/SwiftUI's trailing side sheet. */
    public fun sheetLeft(
        placement: SettingsPlacement,
        viewportWidth: Int,
        isRtl: Boolean,
    ): Int {
        if (placement.mode != SettingsPlacementMode.SIDE ||
            placement.width <= 0 || viewportWidth <= 0
        ) return 0
        return if (isRtl) 0 else maxOf(0, viewportWidth - placement.width)
    }

    public fun sheetIntersectsVerticalCrease(
        placement: SettingsPlacement,
        crease: WindowCrease,
        viewportWidth: Int,
        isRtl: Boolean,
    ): Boolean {
        if (placement.mode != SettingsPlacementMode.SIDE ||
            placement.width <= 0 || viewportWidth <= 0
        ) return false
        val left = sheetLeft(placement, viewportWidth, isRtl)
        val right = left + placement.width
        val creaseRight = crease.left + crease.width
        return left < creaseRight && right > crease.left
    }

    private fun hoverPlacement(
        input: AdaptiveLayoutInput,
        kind: SettingsSheetKind,
    ): SettingsPlacement {
        val horizontal = input.horizontalCreases
            .filter { it.top > 0 && it.height >= 0 && it.top + it.height < input.viewportHeight }
            .sortedBy { it.top }
            .firstOrNull()
        if (horizontal != null) {
            val maxHeight = maxOf(0, input.viewportHeight - horizontal.top - horizontal.height)
            return SettingsPlacement(
                mode = SettingsPlacementMode.FOLD_OPERATE,
                width = 0,
                height = kindOperateHeight(maxHeight, kind),
                maxHeight = maxHeight,
            )
        }
        return creaseFreeOrSidePlacement(input, kind)
    }

    private fun creaseFreeOrSidePlacement(
        input: AdaptiveLayoutInput,
        kind: SettingsSheetKind,
    ): SettingsPlacement {
        val usable = input.verticalCreases
            .filter { it.left > 0 && it.width >= 0 && it.left + it.width < input.viewportWidth }
            .sortedBy { it.left }
        if (usable.isNotEmpty() && canUseSideSheet(input)) {
            val crease = if (input.isRtl) usable.first() else usable.last()
            val width = if (input.isRtl) {
                maxOf(0, crease.left)
            } else {
                maxOf(0, input.viewportWidth - crease.left - crease.width)
            }
            return SettingsPlacement(
                SettingsPlacementMode.SIDE,
                width,
                kindSideHeight(input.viewportHeight, kind),
                0,
            )
        }
        if (isHeightScarceLandscape(input) && canUseSideSheet(input)) {
            return SettingsPlacement(
                SettingsPlacementMode.SIDE,
                (input.viewportWidth * LANDSCAPE_WIDTH_RATIO).roundToInt(),
                kindSideHeight(input.viewportHeight, kind),
                0,
            )
        }
        if (isWideViewport(input) && canUseSideSheet(input)) {
            val ratioWidth = (input.viewportWidth * TABLET_WIDTH_RATIO).roundToInt()
            val width = minOf(input.viewportWidth, ratioWidth.coerceIn(TABLET_MIN_WIDTH, TABLET_MAX_WIDTH))
            return SettingsPlacement(
                SettingsPlacementMode.SIDE,
                width,
                kindSideHeight(input.viewportHeight, kind),
                0,
            )
        }
        return bottomPlacement(kind)
    }

    private fun bottomPlacement(kind: SettingsSheetKind): SettingsPlacement = SettingsPlacement(
        mode = SettingsPlacementMode.BOTTOM,
        width = 0,
        height = if (kind == SettingsSheetKind.REMOTE_VIEW_SETTINGS) REMOTE_VIEW_SETTINGS_HEIGHT else 0,
        maxHeight = 0,
    )

    private fun kindSideHeight(viewportHeight: Int, kind: SettingsSheetKind): Int {
        val computed = sideHeight(viewportHeight)
        return when (kind) {
            SettingsSheetKind.SESSION_DETAILS -> minOf(SESSION_DETAILS_HEIGHT, computed)
            SettingsSheetKind.REMOTE_VIEW_SETTINGS -> minOf(REMOTE_VIEW_SETTINGS_HEIGHT, computed)
            else -> computed
        }
    }

    private fun kindOperateHeight(maxHeight: Int, kind: SettingsSheetKind): Int = when (kind) {
        SettingsSheetKind.REMOTE_VIEW_SETTINGS -> minOf(REMOTE_VIEW_SETTINGS_HEIGHT, maxHeight)
        SettingsSheetKind.SESSION_DETAILS -> minOf(SESSION_DETAILS_HEIGHT, maxHeight)
        else -> maxHeight
    }

    private fun sideHeight(viewportHeight: Int): Int {
        if (viewportHeight <= 0) return SIDE_MAX_HEIGHT
        return (viewportHeight - SIDE_VERTICAL_MARGIN).coerceIn(SIDE_MIN_HEIGHT, SIDE_MAX_HEIGHT)
    }

    private fun canUseSideSheet(input: AdaptiveLayoutInput): Boolean =
        input.viewportWidth >= ConversationLayoutPolicy.MD_MIN_WIDTH

    private fun isWideViewport(input: AdaptiveLayoutInput): Boolean =
        input.wideLayoutMatched || input.viewportWidth >= ConversationLayoutPolicy.MD_MIN_WIDTH

    private fun isHeightScarceLandscape(input: AdaptiveLayoutInput): Boolean =
        input.viewportWidth > input.viewportHeight &&
            input.viewportHeight > 0 &&
            input.viewportHeight < ConversationLayoutPolicy.MD_MIN_WIDTH

    private fun Double.roundToInt(): Int = (this + 0.5).toInt()
}
