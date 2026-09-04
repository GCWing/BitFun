package com.openbitfun.mobile.app.ui.shell

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Locks the content-card boundary rules for the compact drawer without standing
 * up a Compose tree. [ContentCardCoordinator] is the single observable for the
 * rounded clip + shadow: [ContentCardPhase.Receded] means the static shadow/clip
 * modifier is attached, [ContentCardPhase.Flat] means the full-screen content is
 * drawn with neither.
 */
class ContentCardCoordinatorTest {
    @Test
    fun initialStateReflectsWhetherTheDrawerStartsOpen() {
        assertEquals(ContentCardPhase.Flat, ContentCardCoordinator(initialOpen = false).phase)
        assertEquals(ContentCardPhase.Receded, ContentCardCoordinator(initialOpen = true).phase)
    }

    @Test
    fun revealDoesNotRecedeTheFirstFullScreenFrame() {
        val coordinator = ContentCardCoordinator(initialOpen = false)

        coordinator.onRevealStarted()

        // The first frame after opening is still the content at rest, full
        // screen. The 28dp corner and shadow must not snap onto it.
        assertEquals(ContentCardPhase.Flat, coordinator.phase)
    }

    @Test
    fun zeroProgressNeverRecedes() {
        val coordinator = ContentCardCoordinator(initialOpen = false)

        coordinator.onRevealStarted()
        coordinator.onContentProgress(0f)

        // progress == 0 means the content is still at rest, full screen; the
        // corner/shadow must stay off. This pins the "only progress > 0 may
        // recede" invariant directly.
        assertEquals(ContentCardPhase.Flat, coordinator.phase)
    }

    @Test
    fun positiveProgressRecedesTheCard() {
        val coordinator = ContentCardCoordinator(initialOpen = false)

        coordinator.onRevealStarted()
        coordinator.onContentProgress(0.001f)

        assertEquals(ContentCardPhase.Receded, coordinator.phase)
    }

    @Test
    fun openThenCloseSettlesBackToFlat() {
        val coordinator = ContentCardCoordinator(initialOpen = false)

        coordinator.onRevealStarted()
        coordinator.onContentProgress(0.5f)
        assertEquals(ContentCardPhase.Receded, coordinator.phase)

        // Close only removes the shadow/clip once the content has settled back
        // to full screen — never on an earlier drawer-hide deadline.
        coordinator.onContentSettled()

        assertEquals(ContentCardPhase.Flat, coordinator.phase)
    }

    @Test
    fun rapidOpenCloseConverges() {
        val coordinator = ContentCardCoordinator(initialOpen = false)

        coordinator.onRevealStarted()
        coordinator.onContentProgress(0.4f)
        coordinator.onContentSettled()
        coordinator.onRevealStarted()
        coordinator.onContentProgress(0.4f)
        coordinator.onContentSettled()

        assertEquals(ContentCardPhase.Flat, coordinator.phase)
    }

    @Test
    fun reducedMotionCollapsesTimingButKeepsTheFinalState() {
        val coordinator = ContentCardCoordinator(initialOpen = false)

        // Under "Remove animations" the content animation completes instantly,
        // so the first observed progress is already 1f. The boundary events
        // still fire in the same order and the final card state must be flat
        // with no residual clip/shadow; a repeated settle is a no-op.
        coordinator.onRevealStarted()
        coordinator.onContentProgress(1f)
        assertEquals(ContentCardPhase.Receded, coordinator.phase)

        coordinator.onContentSettled()
        coordinator.onContentSettled()

        assertEquals(ContentCardPhase.Flat, coordinator.phase)
    }
}
