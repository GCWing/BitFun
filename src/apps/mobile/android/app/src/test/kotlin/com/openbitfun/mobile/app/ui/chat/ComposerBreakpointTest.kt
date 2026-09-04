package com.openbitfun.mobile.app.ui.chat

import com.openbitfun.mobile.app.ui.theme.generated.MobileDesignBreakpoints
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ComposerBreakpointTest {
    @Test
    fun generatedBreakpointTokensRemainStable() {
        assertEquals(600, MobileDesignBreakpoints.Wide)
        assertEquals(840, MobileDesignBreakpoints.ExtraWide)
        assertEquals(1440, MobileDesignBreakpoints.Xl)
    }

    @Test
    fun composerWideBreakpointHasNoOffByOne() {
        assertFalse(composerIsWide(599))
        assertTrue(composerIsWide(600))
    }

    @Test
    fun generatedBreakpointBoundariesRemainOrdered() {
        assertTrue(839 < MobileDesignBreakpoints.ExtraWide)
        assertEquals(MobileDesignBreakpoints.ExtraWide, 840)
        assertTrue(1439 < MobileDesignBreakpoints.Xl)
        assertEquals(MobileDesignBreakpoints.Xl, 1440)
    }
}
