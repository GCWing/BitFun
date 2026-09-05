package com.openbitfun.mobile.core.feature.layout

/** Where a file preview goes, given how much window there is. */
public enum class FilePreviewPlacement {
    /** Nothing to place. */
    Hidden,

    /** Over the conversation, edge to edge. The only option on a phone. */
    CompactFullPage,

    /** Conversation and preview side by side, with no room left for the list. */
    WideFocusSplit,

    /** List, conversation and preview, all three at once. */
    WideTriplePane,
}

/**
 * The resolved pane widths, in the same units as the viewport width.
 *
 * The gaps are what a divider or a hinge occupies between two panes, so the
 * widths and the gaps sum to the viewport rather than overlapping it. Everything
 * except [placement] is `0` when there is nothing to lay out.
 */
public data class FilePreviewLayout(
    val placement: FilePreviewPlacement,
    val masterPaneWidth: Int,
    val masterConversationGap: Int,
    val conversationPaneWidth: Int,
    val conversationPreviewGap: Int,
    val previewPaneWidth: Int,
) {
    /**
     * A placement with no geometry. A secondary constructor rather than default
     * arguments, because Swift does not see Kotlin defaults — see the design
     * doc §4.1.
     */
    public constructor(placement: FilePreviewPlacement) : this(placement, 0, 0, 0, 0, 0)
}

/**
 * How a file preview shares the window with the conversation that opened it.
 *
 * Ports `pages/policy/FilePreviewPlacementPolicy.ets`, including the order the
 * three shapes are tried in: align to the hinges if there are two, else split a
 * flat screen three ways if it is wide enough, else give up the list and split
 * two ways, else cover the conversation.
 *
 * The minimums are the whole point. A preview is a second document, and two
 * documents in less than [MIN_CONVERSATION_WIDTH] + [MIN_PREVIEW_WIDTH] are two
 * columns of hyphenation — at which point one document on top of the other is
 * the better answer, which is what [FilePreviewPlacement.CompactFullPage] is.
 */
public object FilePreviewPlacementPolicy {
    public const val MIN_MASTER_WIDTH: Int = 280
    public const val MIN_CONVERSATION_WIDTH: Int = 360
    public const val MIN_PREVIEW_WIDTH: Int = 360

    /** A hairline between panes, which still has to come out of somebody's width. */
    public const val PANE_DIVIDER_WIDTH: Int = 1

    /** Just the shape, for a caller that only switches on it. */
    public fun resolve(
        previewVisible: Boolean,
        largeScreenLayout: Boolean,
        viewportWidth: Int,
        creases: List<WindowCrease>,
    ): FilePreviewPlacement =
        resolveLayout(previewVisible, largeScreenLayout, viewportWidth, creases).placement

    /**
     * The shape and its widths.
     *
     * @param preferredMasterWidth what the list pane would like to be, usually
     * whatever [ConversationLayoutPolicy.resolveWideGeometry] last gave it, so
     * the list does not jump when a preview opens beside it. It is clamped
     * between [MIN_MASTER_WIDTH] and whatever is left once both documents have
     * theirs.
     */
    public fun resolveLayout(
        previewVisible: Boolean,
        largeScreenLayout: Boolean,
        viewportWidth: Int,
        creases: List<WindowCrease>,
        preferredMasterWidth: Int,
    ): FilePreviewLayout {
        if (!previewVisible) return FilePreviewLayout(FilePreviewPlacement.Hidden)
        if (!largeScreenLayout) return fullPage(viewportWidth)

        creaseAlignedTriplePane(viewportWidth, creases)?.let { return it }
        flatTriplePane(viewportWidth, creases, preferredMasterWidth)?.let { return it }

        val gap = PANE_DIVIDER_WIDTH
        if (viewportWidth < MIN_CONVERSATION_WIDTH + MIN_PREVIEW_WIDTH + gap) {
            return fullPage(viewportWidth)
        }
        val content = maxOf(0, viewportWidth - gap)
        val conversation = content / 2
        return FilePreviewLayout(
            placement = FilePreviewPlacement.WideFocusSplit,
            masterPaneWidth = 0,
            masterConversationGap = 0,
            conversationPaneWidth = conversation,
            conversationPreviewGap = gap,
            // The remainder, not another halving: an odd width has to land
            // somewhere, and the preview is the pane that can take the extra.
            previewPaneWidth = content - conversation,
        )
    }

    /** [resolveLayout] with the list pane at its minimum. An overload, not a default. */
    public fun resolveLayout(
        previewVisible: Boolean,
        largeScreenLayout: Boolean,
        viewportWidth: Int,
        creases: List<WindowCrease>,
    ): FilePreviewLayout =
        resolveLayout(previewVisible, largeScreenLayout, viewportWidth, creases, MIN_MASTER_WIDTH)

    private fun fullPage(viewportWidth: Int): FilePreviewLayout {
        val width = maxOf(0, viewportWidth)
        return FilePreviewLayout(
            placement = FilePreviewPlacement.CompactFullPage,
            masterPaneWidth = 0,
            masterConversationGap = 0,
            conversationPaneWidth = width,
            conversationPreviewGap = 0,
            previewPaneWidth = width,
        )
    }

    /**
     * Three panes on a flat screen, with the dividers drawn rather than folded.
     *
     * Returns `null` if there is a hinge, because then the hinges decide — a
     * divider a few dp from a fold is a seam the device already has.
     */
    private fun flatTriplePane(
        viewportWidth: Int,
        creases: List<WindowCrease>,
        preferredMasterWidth: Int,
    ): FilePreviewLayout? {
        if (creases.visibleIn(viewportWidth).isNotEmpty()) return null
        val divider = PANE_DIVIDER_WIDTH
        val minimum = MIN_MASTER_WIDTH + MIN_CONVERSATION_WIDTH + MIN_PREVIEW_WIDTH + divider * 2
        if (viewportWidth < minimum) return null

        val maximumMaster = viewportWidth - divider * 2 - MIN_CONVERSATION_WIDTH - MIN_PREVIEW_WIDTH
        val master = maxOf(MIN_MASTER_WIDTH, minOf(preferredMasterWidth, maximumMaster))
        val detail = viewportWidth - master - divider * 2
        val conversation = detail / 2
        return FilePreviewLayout(
            placement = FilePreviewPlacement.WideTriplePane,
            masterPaneWidth = master,
            masterConversationGap = divider,
            conversationPaneWidth = conversation,
            conversationPreviewGap = divider,
            previewPaneWidth = detail - conversation,
        )
    }

    /**
     * Three panes on a device that already has three panels.
     *
     * Every pane boundary is a hinge, so nothing is drawn to separate them and
     * no pane is laid across a fold. If any of the three panels is too small for
     * what would go in it, this is not the shape — the device's own proportions
     * cannot be argued with, only declined.
     */
    private fun creaseAlignedTriplePane(
        viewportWidth: Int,
        creases: List<WindowCrease>,
    ): FilePreviewLayout? {
        val visible = creases.visibleIn(viewportWidth)
        if (visible.size < 2) return null
        val first = visible[0]
        val second = visible[1]
        val master = first.left
        val conversation = second.left - first.left - first.width
        val preview = viewportWidth - second.left - second.width
        if (master < MIN_MASTER_WIDTH ||
            conversation < MIN_CONVERSATION_WIDTH ||
            preview < MIN_PREVIEW_WIDTH
        ) {
            return null
        }
        return FilePreviewLayout(
            placement = FilePreviewPlacement.WideTriplePane,
            masterPaneWidth = master,
            masterConversationGap = first.width,
            conversationPaneWidth = conversation,
            conversationPreviewGap = second.width,
            previewPaneWidth = preview,
        )
    }
}
