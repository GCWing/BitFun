package com.openbitfun.mobile.core.feature.connection

/**
 * Where the desktop currently being driven came from.
 *
 * Ports `controlTargetType` in `RemoteControlSettingsSheet.ets`, which is
 * `'none' | 'room' | 'account_device'`. A shell shows it as a badge under the
 * card, so the user can tell a one-off pairing from a device the account owns
 * without opening either screen.
 */
public enum class RemoteControlSource {
    /** Nothing is paired and no account device is selected. */
    NONE,

    /** A room reached through a pairing link, which the desktop showed as a QR code. */
    QR_PAIRING,

    /** A desktop registered to the signed-in account. */
    ACCOUNT_DEVICE,
}

/**
 * The one action the current-control card offers.
 *
 * Ports `ConnectionAction()`: a live link is something to leave, a link that
 * exists but is not answering is something to try again, and a card with no
 * desktop behind it has neither to offer.
 */
public enum class RemoteControlAction {
    NONE,
    DISCONNECT,
    RECONNECT,
}

/**
 * Everything the current-control card renders, minus its wording.
 *
 * @param desktopName the desktop's own name, or `""` when there is none — the
 * app supplies its own "no desktop yet" sentence rather than receiving one, per
 * design doc §4.3.
 */
public data class RemoteControlSummary public constructor(
    public val source: RemoteControlSource,
    public val desktopName: String,
    public val phase: ConnectionPhase,
    public val action: RemoteControlAction,
)

/** Ported from the `connectionTitle` / `connectionSource` / `ConnectionAction` trio. */
public object RemoteControlPresenter {
    /**
     * Reduces the two ways this app can be driving a desktop into the one card
     * that describes it.
     *
     * A paired room wins over an account device when both exist. They are
     * separate stores here — unlike the source, which keeps a single control
     * target — and a room is the more deliberate of the two: it was pasted or
     * scanned for this session, while a selected device outlives every sign-in.
     *
     * @param pairedRoomLabel the already-truncated room label, or `""`. The full
     * room id never crosses this seam.
     * @param accountDeviceName may be blank for a device the relay only knows by
     * id, in which case the id is what the card can name.
     * @param accountPhase the selected device store's latest real command/poll
     * phase; selection alone is not evidence that the device is reachable.
     */
    public fun summarize(
        pairingPhase: ConnectionPhase,
        pairedRoomLabel: String,
        accountDeviceId: String,
        accountDeviceName: String,
        accountPhase: ConnectionPhase,
    ): RemoteControlSummary = when {
        pairedRoomLabel.isNotBlank() -> RemoteControlSummary(
            source = RemoteControlSource.QR_PAIRING,
            desktopName = pairedRoomLabel,
            phase = pairingPhase,
            action = when (pairingPhase) {
                ConnectionPhase.CONNECTED,
                ConnectionPhase.CONNECTING,
                ConnectionPhase.RECONNECTING,
                -> RemoteControlAction.DISCONNECT

                ConnectionPhase.FAILED,
                ConnectionPhase.DISCONNECTED,
                ConnectionPhase.IDLE,
                -> RemoteControlAction.RECONNECT
            },
        )

        accountDeviceId.isNotBlank() -> RemoteControlSummary(
            source = RemoteControlSource.ACCOUNT_DEVICE,
            desktopName = accountDeviceName.ifBlank { accountDeviceId },
            phase = accountPhase,
            // There is no release for an account device — the binding is the
            // selection — so the only thing left to offer is re-binding it,
            // which is exactly what re-tapping its row in the account does.
            action = RemoteControlAction.RECONNECT,
        )

        else -> RemoteControlSummary(
            source = RemoteControlSource.NONE,
            desktopName = "",
            phase = ConnectionPhase.IDLE,
            action = RemoteControlAction.NONE,
        )
    }
}
