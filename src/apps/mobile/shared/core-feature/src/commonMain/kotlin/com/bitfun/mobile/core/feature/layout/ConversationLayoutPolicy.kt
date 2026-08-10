package com.bitfun.mobile.core.feature.layout

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
 * @param detailContentOffset how far into the detail pane its content starts,
 * so a conversation is not laid across a hinge. Zero on a flat screen.
 * @param collapsedDetailContentOffset the same, for the layout with no master
 * pane — the detail pane on its own, still avoiding the hinge.
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
 * **Two of the ArkTS parameters are deliberately gone.** `mediaQueryMatched`
 * carried the answer from a separate `matchMediaSync` channel, which exists
 * because ArkUI hands width and media queries to a component through different
 * doors; here the width *is* the query, so a second opinion about it would only
 * be a way for the two to disagree. `isFolded` guarded against a device closed
 * onto its cover screen, which on both platforms already reports a viewport far
 * under [WIDE_LAYOUT_MIN_WIDTH] — and whose crease list the ArkTS caller empties
 * for that same reason before calling in.
 */
public object ConversationLayoutPolicy {
    /** Under this, one pane, whatever the device is. */
    public const val WIDE_LAYOUT_MIN_WIDTH: Int = 720

    /** The master pane on a screen with no hinge to align to. */
    public const val FALLBACK_MASTER_PANE_WIDTH: Int = 344

    /** A master pane narrower than this is a column of ellipses. */
    public const val MIN_MASTER_PANE_WIDTH: Int = 280

    /** A detail pane narrower than this cannot hold a conversation. */
    public const val MIN_DETAIL_PANE_WIDTH: Int = 360

    /** Past this there is room for a third thing on screen. */
    public const val EXTRA_WIDE_MIN_WIDTH: Int = 1080

    /**
     * @param largeScreenDevice the platform's own answer to "is this a big
     * screen rather than a phone". HarmonyOS passes `deviceType == 'tablet'`;
     * Android passes whether the window is in the expanded width class, because
     * Android has no device-type string and the window is the whole answer there.
     *
     * A window with exactly one vertical crease stays single-pane on purpose:
     * the seam would land on the hinge, which puts the list on one leaf and the
     * conversation on the other and leaves the master pane half the device wide.
     * Two creases is a tri-fold, where the middle panel is a detail pane of its
     * own accord.
     */
    public fun useMasterDetail(
        viewportWidth: Int,
        largeScreenDevice: Boolean,
        creases: List<WindowCrease>,
    ): Boolean {
        if (viewportWidth < WIDE_LAYOUT_MIN_WIDTH) return false
        val visible = creases.visibleIn(viewportWidth)
        if (visible.isNotEmpty()) return visible.size >= 2
        return largeScreenDevice
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
        // The first hinge that both panes can live with. One too close to either
        // edge is worse than no hinge at all, so it is skipped rather than
        // squeezed against.
        val seam = visible.firstOrNull { crease ->
            crease.left >= MIN_MASTER_PANE_WIDTH &&
                crease.left + crease.width <= viewportWidth - MIN_DETAIL_PANE_WIDTH
        }
        val masterPaneWidth = seam?.left ?: FALLBACK_MASTER_PANE_WIDTH
        val masterDetailGap = seam?.width?.coerceAtLeast(0) ?: 0
        val detailStart = masterPaneWidth + masterDetailGap
        val content = detailSegments(viewportWidth, detailStart, visible).widest()
        val collapsed = detailSegments(viewportWidth, 0, visible).widest()
        return ConversationLayoutGeometry(
            masterPaneWidth = masterPaneWidth,
            masterDetailGap = masterDetailGap,
            isExtraWide = visible.size > 1 || viewportWidth >= EXTRA_WIDE_MIN_WIDTH,
            detailContentOffset = content?.let { it.left - detailStart } ?: 0,
            detailContentWidth = content?.width ?: 0,
            collapsedDetailContentOffset = collapsed?.left ?: 0,
            collapsedDetailContentWidth = collapsed?.width ?: 0,
        )
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
