package com.openbitfun.mobile.core.feature.connection

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ConnectionStatusPresenterTest {
    @Test
    fun aBlipReadsAsBusyRatherThanAnError() {
        assertEquals(ConnectionTone.BUSY, ConnectionStatusPresenter.tone(ConnectionPhase.RECONNECTING))
        assertEquals(ConnectionStatusLabel.RECONNECTING, ConnectionStatusPresenter.label(ConnectionPhase.RECONNECTING))
    }

    @Test
    fun havingNeverConnectedIsNotAnError() {
        assertEquals(ConnectionTone.MUTED, ConnectionStatusPresenter.tone(ConnectionPhase.IDLE))
        assertEquals(ConnectionStatusLabel.WAITING, ConnectionStatusPresenter.label(ConnectionPhase.IDLE))
        assertEquals(ConnectionTone.ERROR, ConnectionStatusPresenter.tone(ConnectionPhase.FAILED))
    }

    @Test
    fun sessionsStayReachableWhileReconnecting() {
        assertTrue(ConnectionStatusPresenter.canReachSessions(ConnectionPhase.CONNECTED))
        assertTrue(ConnectionStatusPresenter.canReachSessions(ConnectionPhase.RECONNECTING))
        assertFalse(ConnectionStatusPresenter.canReachSessions(ConnectionPhase.DISCONNECTED))
    }

}
