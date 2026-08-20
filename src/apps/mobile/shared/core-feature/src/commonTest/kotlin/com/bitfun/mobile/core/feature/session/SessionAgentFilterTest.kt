package com.bitfun.mobile.core.feature.session

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class SessionAgentFilterTest {
    @Test
    fun codeKeepsTheLegacyAgenticSpelling() {
        assertTrue(SessionAgentFilter.CODE.matches("code"))
        assertTrue(SessionAgentFilter.CODE.matches("Agentic"))
        assertFalse(SessionAgentFilter.CODE.matches("cowork"))
    }

    @Test
    fun coworkMatchesOnlyCowork() {
        assertTrue(SessionAgentFilter.COWORK.matches("cowork"))
        assertFalse(SessionAgentFilter.COWORK.matches("code"))
    }

    @Test
    fun allKeepsAgentTypesTheAppHasNoTabFor() {
        // The desktop is free to add agent kinds; the unfiltered tab must not hide them.
        assertTrue(SessionAgentFilter.ALL.matches("claw"))
        assertTrue(SessionAgentFilter.ALL.matches(""))
    }
}
