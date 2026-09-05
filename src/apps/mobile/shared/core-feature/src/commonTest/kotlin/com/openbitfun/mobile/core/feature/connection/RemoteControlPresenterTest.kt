package com.openbitfun.mobile.core.feature.connection

import kotlin.test.Test
import kotlin.test.assertEquals

class RemoteControlPresenterTest {
    @Test
    fun withNothingPairedTheCardHasNoDesktopAndNoAction() {
        val summary = RemoteControlPresenter.summarize(
            pairingPhase = ConnectionPhase.IDLE,
            pairedRoomLabel = "",
            accountDeviceId = "",
            accountDeviceName = "",
            accountPhase = ConnectionPhase.IDLE,
        )

        assertEquals(RemoteControlSource.NONE, summary.source)
        assertEquals("", summary.desktopName)
        assertEquals(RemoteControlAction.NONE, summary.action)
    }

    @Test
    fun aLiveRoomOffersLeavingItAndAStalledOneOffersTryingAgain() {
        val live = RemoteControlPresenter.summarize(
            pairingPhase = ConnectionPhase.CONNECTED,
            pairedRoomLabel = "ab12",
            accountDeviceId = "",
            accountDeviceName = "",
            accountPhase = ConnectionPhase.IDLE,
        )
        assertEquals(RemoteControlSource.QR_PAIRING, live.source)
        assertEquals("ab12", live.desktopName)
        assertEquals(RemoteControlAction.DISCONNECT, live.action)

        // A room that stopped answering is still paired, so the card keeps
        // naming it and swaps the action rather than emptying itself.
        val lost = RemoteControlPresenter.summarize(
            pairingPhase = ConnectionPhase.FAILED,
            pairedRoomLabel = "ab12",
            accountDeviceId = "",
            accountDeviceName = "",
            accountPhase = ConnectionPhase.IDLE,
        )
        assertEquals(RemoteControlSource.QR_PAIRING, lost.source)
        assertEquals(RemoteControlAction.RECONNECT, lost.action)
    }

    @Test
    fun aSelectedAccountDeviceIsTheDesktopWhenNoRoomIsPaired() {
        val summary = RemoteControlPresenter.summarize(
            pairingPhase = ConnectionPhase.IDLE,
            pairedRoomLabel = "",
            accountDeviceId = "device-1",
            accountDeviceName = "Studio",
            accountPhase = ConnectionPhase.CONNECTED,
        )

        assertEquals(RemoteControlSource.ACCOUNT_DEVICE, summary.source)
        assertEquals("Studio", summary.desktopName)
        assertEquals(ConnectionPhase.CONNECTED, summary.phase)
        assertEquals(RemoteControlAction.RECONNECT, summary.action)
    }

    @Test
    fun aDeviceTheRelayOnlyKnowsByIdIsNamedByThatId() {
        val summary = RemoteControlPresenter.summarize(
            pairingPhase = ConnectionPhase.IDLE,
            pairedRoomLabel = "",
            accountDeviceId = "device-1",
            accountDeviceName = "  ",
            accountPhase = ConnectionPhase.RECONNECTING,
        )

        assertEquals("device-1", summary.desktopName)
    }

    @Test
    fun aPairedRoomWinsOverASelectedDevice() {
        val summary = RemoteControlPresenter.summarize(
            pairingPhase = ConnectionPhase.CONNECTED,
            pairedRoomLabel = "ab12",
            accountDeviceId = "device-1",
            accountDeviceName = "Studio",
            accountPhase = ConnectionPhase.FAILED,
        )

        assertEquals(RemoteControlSource.QR_PAIRING, summary.source)
        assertEquals("ab12", summary.desktopName)
    }

    @Test
    fun aSelectedAccountDevicePublishesItsTransportPhase() {
        val summary = RemoteControlPresenter.summarize(
            pairingPhase = ConnectionPhase.IDLE,
            pairedRoomLabel = "",
            accountDeviceId = "device-1",
            accountDeviceName = "Studio",
            accountPhase = ConnectionPhase.RECONNECTING,
        )

        assertEquals(ConnectionPhase.RECONNECTING, summary.phase)
    }
}
