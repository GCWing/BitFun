package com.openbitfun.mobile.core.feature.session

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class CreateSessionPresentationTest {
    @Test
    fun noWorkspaceMeansChat() {
        assertEquals("Claw", CreateSessionPresenter.agentType(""))
    }

    @Test
    fun aPathOfNothingButSpaceIsStillNoWorkspace() {
        // The picker writes back whatever the desktop sent, and a padded path
        // would otherwise create a code session bound to nowhere.
        assertEquals("Claw", CreateSessionPresenter.agentType("   \n"))
    }

    @Test
    fun aPickedWorkspaceMeansTheCodeAgent() {
        assertEquals("code", CreateSessionPresenter.agentType("/Users/dev/project"))
    }

    @Test
    fun anInstructionAndADeviceAreBothRequired() {
        assertTrue(CreateSessionPresenter.canSubmit("fix the build", "device-1", submitting = false))
        assertFalse(CreateSessionPresenter.canSubmit("   ", "device-1", submitting = false))
        assertFalse(CreateSessionPresenter.canSubmit("fix the build", "", submitting = false))
    }

    @Test
    fun aRequestInFlightBlocksASecondOne() {
        // Two taps on the same button would otherwise create two sessions, and
        // the second one is invisible until the list reloads.
        assertFalse(CreateSessionPresenter.canSubmit("fix the build", "device-1", submitting = true))
    }
}
