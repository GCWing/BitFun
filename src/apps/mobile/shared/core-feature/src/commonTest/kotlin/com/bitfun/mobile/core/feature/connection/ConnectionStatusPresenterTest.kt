package com.bitfun.mobile.core.feature.connection

import com.bitfun.mobile.core.feature.pairing.ConnectionLiveness
import com.bitfun.mobile.core.feature.pairing.PairedWorkspace
import com.bitfun.mobile.core.feature.pairing.PairingFailure
import com.bitfun.mobile.core.feature.pairing.PairingFailureReason
import com.bitfun.mobile.core.feature.pairing.PairingUiState
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

    @Test
    fun pairingStatesMapOntoPhases() {
        assertEquals(ConnectionPhase.IDLE, PairingUiState.Idle.connectionPhase())
        assertEquals(ConnectionPhase.CONNECTING, PairingUiState.Connecting.connectionPhase())
        assertEquals(
            ConnectionPhase.CONNECTED,
            PairingUiState.Paired(PairedWorkspace("room-ab", null, true, null)).connectionPhase(),
        )
        assertEquals(
            ConnectionPhase.FAILED,
            PairingUiState.Failed(PairingFailure(PairingFailureReason.RoomNotFound)).connectionPhase(),
        )
    }

    /**
     * A paired room reports its own liveness, and that is where RECONNECTING now
     * comes from: an announced health check, not a second handshake. A lost one
     * reads as an error even though the pairing itself is untouched — from the
     * shell's side there is nothing to reach either way.
     */
    @Test
    fun aPairedRoomsLivenessDecidesItsPhase() {
        val workspace = PairedWorkspace("room-ab", null, true, null)
        assertEquals(
            ConnectionPhase.RECONNECTING,
            PairingUiState.Paired(workspace, ConnectionLiveness.CHECKING).connectionPhase(),
        )
        assertEquals(
            ConnectionPhase.FAILED,
            PairingUiState.Paired(workspace, ConnectionLiveness.LOST).connectionPhase(),
        )
    }
}
