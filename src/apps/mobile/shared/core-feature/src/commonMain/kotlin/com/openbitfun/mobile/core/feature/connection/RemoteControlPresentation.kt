package com.openbitfun.mobile.core.feature.connection

/** The account directory owns remote device selection. */
public enum class RemoteControlSource {
    /** Nothing is paired and no account device is selected. */
    NONE,

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
    public fun summarize(
        accountDeviceId: String,
        accountDeviceName: String,
        accountPhase: ConnectionPhase,
    ): RemoteControlSummary = when {
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
