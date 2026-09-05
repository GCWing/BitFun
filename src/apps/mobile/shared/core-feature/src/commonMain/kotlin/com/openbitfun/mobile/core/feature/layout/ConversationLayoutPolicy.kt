package com.openbitfun.mobile.core.feature.layout

import com.openbitfun.mobile.core.feature.layout.generated.MobileDesignBreakpoints

/**
 * A hinge crossing the window, in the same density-independent units as the
 * viewport width it is measured against.
 *
 * Only vertical creases belong here — a crease that runs across the window
 * rather than down it splits nothing the master/detail layout cares about, and
 * both platforms filter theirs before calling in.
 */
public data class WindowCrease(
    /** Distance from the window's leading edge to the near side of the hinge. */
    val left: Int,
    /** How wide the hinge itself is. Flat-folding devices report `0`. */
    val width: Int,
)

/**
 * Where the two panes go once [ConversationLayoutPolicy.useMasterDetail] says
 * there are two.
 *
 * @param detailContentOffset how far into the detail pane its content starts.
 * The current HarmonyOS policy uses the full span after the master seam, so
 * this is zero.
 * @param collapsedDetailContentOffset where content starts when the master pane
 * is hidden; asymmetric and multi-crease layouts may choose the dominant band.
 */
public data class ConversationLayoutGeometry(
    val masterPaneWidth: Int,
    val masterDetailGap: Int,
    val isExtraWide: Boolean,
    val detailContentOffset: Int,
    val detailContentWidth: Int,
    val collapsedDetailContentOffset: Int,
    val collapsedDetailContentWidth: Int,
)

/** A crease-free run of the window. */
private data class LayoutSegment(val left: Int, val width: Int)

/**
 * Whether this window is two panes or one, and where the seam falls.
 *
 * Ports `pages/policy/ConversationLayoutPolicy.ets`. The numbers are the
 * source's, and so is the order they are asked in.
 *
 * Platform adapters translate their fold APIs into the semantic booleans this
 * policy accepts. HarmonyOS fold-status numbers and Android WindowManager types
 * stay in their owning apps; the layout decision remains shared pure logic.
 */
public object ConversationLayoutPolicy {
    /** Official GridRow sm|md|lg|xl boundaries used by the HarmonyOS surface. */
    public const val MD_MIN_WIDTH: Int = MobileDesignBreakpoints.Wide
    public const val LG_MIN_WIDTH: Int = MobileDesignBreakpoints.ExtraWide
    public const val XL_MIN_WIDTH: Int = MobileDesignBreakpoints.Xl
    public const val WIDE_LAYOUT_MIN_WIDTH: Int = MD_MIN_WIDTH
    public const val EXTRA_WIDE_MIN_WIDTH: Int = LG_MIN_WIDTH

    /** The master pane on a screen with no hinge to align to. */
    public const val FALLBACK_MASTER_PANE_WIDTH: Int = 344

    /** A master pane narrower than this is a column of ellipses. */
    public const val MIN_MASTER_PANE_WIDTH: Int = 280

    /** A detail pane narrower than this cannot hold a conversation. */
    public const val MIN_DETAIL_PANE_WIDTH: Int = 360

    /** Width reserved for a foldable whose platform omitted its crease bounds. */
    public const val SYNTHETIC_HINGE_WIDTH: Int = 16

    /**
     * Whether a half-open posture should use the compact hover presentation.
     * A landscape window that is already md-wide remains master/detail.
     */
    public fun useHoverOperate(
        halfOpened: Boolean,
        viewportWidth: Int,
        viewportHeight: Int,
    ): Boolean {
        if (!halfOpened) return false
        if (viewportWidth >= MD_MIN_WIDTH && viewportWidth >= viewportHeight) return false
        return true
    }

    /** Whether the conversation should render a persistent master pane. */
    public fun useMasterDetail(
        viewportWidth: Int,
        wideViewportMatched: Boolean,
        isFolded: Boolean,
        creases: List<WindowCrease>,
        isExpandedFoldable: Boolean,
        isHover: Boolean,
    ): Boolean {
        if (isFolded || isHover) return false
        if (!hasWideViewport(viewportWidth, wideViewportMatched)) return false
        val visible = effectiveVerticalCreases(viewportWidth, creases, isExpandedFoldable)
        return canFitMasterDetail(viewportWidth, visible)
    }

    /**
     * Keep reported creases when available; otherwise give an expanded foldable
     * a centered synthetic hinge so both panes stay off the physical fold.
     */
    public fun effectiveVerticalCreases(
        viewportWidth: Int,
        creases: List<WindowCrease>,
        synthesizeCenterHinge: Boolean,
    ): List<WindowCrease> {
        val visible = creases.visibleIn(viewportWidth)
        if (visible.isNotEmpty()) return visible
        if (!synthesizeCenterHinge) return emptyList()
        return listOfNotNull(syntheticCenterCrease(viewportWidth))
    }

    /** A safe fallback for unfolded devices that report posture but no bounds. */
    public fun syntheticCenterCrease(viewportWidth: Int): WindowCrease? {
        val minimum = MIN_MASTER_PANE_WIDTH + MIN_DETAIL_PANE_WIDTH + SYNTHETIC_HINGE_WIDTH
        if (viewportWidth < minimum) return null
        val centered = (viewportWidth - SYNTHETIC_HINGE_WIDTH) / 2
        val maximumMaster = viewportWidth - SYNTHETIC_HINGE_WIDTH - MIN_DETAIL_PANE_WIDTH
        val masterPaneWidth = maxOf(MIN_MASTER_PANE_WIDTH, minOf(centered, maximumMaster))
        return WindowCrease(masterPaneWidth, SYNTHETIC_HINGE_WIDTH)
    }

    /**
     * Where the seam falls, and where the detail pane's content sits inside it.
     *
     * Answered for any width, including ones [useMasterDetail] would refuse: a
     * caller that animates between the two layouts needs the wide geometry
     * before it is wide.
     */
    public fun resolveWideGeometry(
        viewportWidth: Int,
        creases: List<WindowCrease>,
    ): ConversationLayoutGeometry {
        val visible = creases.visibleIn(viewportWidth)
        val seam = firstUsableCrease(viewportWidth, visible)
        val masterPaneWidth = seam?.left ?: fallbackMasterPaneWidth(viewportWidth)
        val masterDetailGap = seam?.width?.coerceAtLeast(0) ?: 0
        val detailStart = masterPaneWidth + masterDetailGap
        val detailWidth = maxOf(0, viewportWidth - detailStart)
        val collapsed = collapsedContentSegment(viewportWidth, visible, detailStart, detailWidth)
        return ConversationLayoutGeometry(
            masterPaneWidth = masterPaneWidth,
            masterDetailGap = masterDetailGap,
            isExtraWide = visible.size > 1 || viewportWidth >= EXTRA_WIDE_MIN_WIDTH,
            detailContentOffset = 0,
            detailContentWidth = detailWidth,
            collapsedDetailContentOffset = collapsed.left,
            collapsedDetailContentWidth = collapsed.width,
        )
    }

    private fun collapsedContentSegment(
        viewportWidth: Int,
        visible: List<WindowCrease>,
        detailStart: Int,
        detailWidth: Int,
    ): LayoutSegment {
        if (visible.isEmpty()) return LayoutSegment(0, maxOf(0, viewportWidth))
        if (visible.size == 1) {
            val widest = detailSegments(viewportWidth, 0, visible).widest()
            if (widest != null && widest.width <= viewportWidth * 0.55f) {
                return LayoutSegment(0, maxOf(0, viewportWidth))
            }
        }
        return LayoutSegment(detailStart, detailWidth)
    }

    /** The detail pane cut into the runs between its hinges. */
    private fun detailSegments(
        viewportWidth: Int,
        detailStart: Int,
        visible: List<WindowCrease>,
    ): List<LayoutSegment> {
        if (viewportWidth <= detailStart) return emptyList()
        val segments = mutableListOf<LayoutSegment>()
        var segmentStart = detailStart
        for (crease in visible.filter { it.left >= detailStart }) {
            if (crease.left > segmentStart) {
                segments += LayoutSegment(segmentStart, crease.left - segmentStart)
            }
            segmentStart = maxOf(segmentStart, crease.left + crease.width)
        }
        segments += LayoutSegment(segmentStart, viewportWidth - segmentStart)
        return segments
    }

    /** The last of the equally widest, as in the source's `>=` reducer. */
    private fun List<LayoutSegment>.widest(): LayoutSegment? =
        fold(null as LayoutSegment?) { widest, segment ->
            if (widest == null || segment.width >= widest.width) segment else widest
        }

    private fun canFitMasterDetail(viewportWidth: Int, visible: List<WindowCrease>): Boolean {
        val seam = firstUsableCrease(viewportWidth, visible)
        val masterPaneWidth = seam?.left ?: fallbackMasterPaneWidth(viewportWidth)
        val masterDetailGap = seam?.width?.coerceAtLeast(0) ?: 0
        return masterPaneWidth >= MIN_MASTER_PANE_WIDTH &&
            viewportWidth - masterPaneWidth - masterDetailGap >= MIN_DETAIL_PANE_WIDTH
    }

    private fun fallbackMasterPaneWidth(viewportWidth: Int): Int {
        val maximumMaster = viewportWidth - MIN_DETAIL_PANE_WIDTH
        if (maximumMaster < MIN_MASTER_PANE_WIDTH) return FALLBACK_MASTER_PANE_WIDTH
        return minOf(FALLBACK_MASTER_PANE_WIDTH, maximumMaster)
    }

    private fun firstUsableCrease(
        viewportWidth: Int,
        visible: List<WindowCrease>,
    ): WindowCrease? = visible.firstOrNull { crease ->
        crease.left >= MIN_MASTER_PANE_WIDTH &&
            crease.left + crease.width <= viewportWidth - MIN_DETAIL_PANE_WIDTH
    }

    private fun hasWideViewport(viewportWidth: Int, wideViewportMatched: Boolean): Boolean =
        wideViewportMatched || viewportWidth >= MD_MIN_WIDTH
}

/**
 * The creases that actually cross this window, in the order they cross it.
 *
 * A hinge at or past either edge is not a hinge in the layout — it belongs to a
 * part of the device the window does not cover.
 */
internal fun List<WindowCrease>.visibleIn(viewportWidth: Int): List<WindowCrease> =
    filter { it.left > 0 && it.width >= 0 && it.left + it.width < viewportWidth }
        .sortedBy { it.left }
