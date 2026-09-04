package com.openbitfun.mobile.core.feature.connection

import com.openbitfun.mobile.core.feature.pairing.ConnectionLiveness
import com.openbitfun.mobile.core.feature.pairing.PairingUiState

/**
 * How far the link to a desktop has got.
 *
 * The HarmonyOS shell keeps this as a bare string (`'idle' | 'parsing' |
 * 'pairing' | 'connected' | 'reconnecting' | 'disconnected' | 'failed'`).
 * `parsing` and `pairing` are one phase for anything that renders it, so they
 * collapse into [CONNECTING] here.
 */
public enum class ConnectionPhase {
    IDLE,
    CONNECTING,
    CONNECTED,
    RECONNECTING,
    DISCONNECTED,
    FAILED,
}

/**
 * Whether a command sent right now could still be answered.
 *
 * The same three phases HarmonyOS' `RemoteUiState.canUseRemote` allows: a link
 * that is up, one that is coming back, and one still being paired. Offering a
 * refresh or a download over a dropped link would only queue work that nothing
 * is going to collect.
 */
public fun ConnectionPhase.allowsRemoteCommands(): Boolean = when (this) {
    ConnectionPhase.CONNECTED, ConnectionPhase.RECONNECTING, ConnectionPhase.CONNECTING -> true
    ConnectionPhase.IDLE, ConnectionPhase.DISCONNECTED, ConnectionPhase.FAILED -> false
}

/**
 * The heading a shell shows for a phase.
 *
 * A cause rather than a sentence, because the wording lives in each app's
 * resources — see the design doc §4.3.
 */
public enum class ConnectionStatusLabel {
    CONNECTED,
    RECONNECTING,
    CONNECTING,
    ERROR,
    DISCONNECTED,

    /** Nothing has been attempted yet. */
    WAITING,
}

/** How prominent the status should look; drives the sidebar dot's colour. */
public enum class ConnectionTone {
    OK,
    BUSY,
    ERROR,
    MUTED,
}

/** Ported from `services/ConnectionStatusPresenter.ets`, minus its i18n lookups. */
public object ConnectionStatusPresenter {
    public fun label(phase: ConnectionPhase): ConnectionStatusLabel = when (phase) {
        ConnectionPhase.CONNECTED -> ConnectionStatusLabel.CONNECTED
        ConnectionPhase.RECONNECTING -> ConnectionStatusLabel.RECONNECTING
        ConnectionPhase.CONNECTING -> ConnectionStatusLabel.CONNECTING
        ConnectionPhase.FAILED -> ConnectionStatusLabel.ERROR
        ConnectionPhase.DISCONNECTED -> ConnectionStatusLabel.DISCONNECTED
        ConnectionPhase.IDLE -> ConnectionStatusLabel.WAITING
    }

    public fun tone(phase: ConnectionPhase): ConnectionTone = when (phase) {
        ConnectionPhase.CONNECTED -> ConnectionTone.OK
        ConnectionPhase.RECONNECTING, ConnectionPhase.CONNECTING -> ConnectionTone.BUSY
        ConnectionPhase.FAILED -> ConnectionTone.ERROR
        ConnectionPhase.IDLE, ConnectionPhase.DISCONNECTED -> ConnectionTone.MUTED
    }

    /**
     * Whether a session opened from the shell can be reached right now.
     *
     * `AppSidebar.openSession` treats `reconnecting` as usable so a tap during a
     * blip queues against the session rather than bouncing the user back to the
     * connect screen.
     */
    public fun canReachSessions(phase: ConnectionPhase): Boolean =
        phase == ConnectionPhase.CONNECTED || phase == ConnectionPhase.RECONNECTING
}

/**
 * The phase a pairing state implies.
 *
 * A paired room reports its own liveness, so all three of [ConnectionPhase]'s
 * live values come from here: an announced health check is [ConnectionPhase.RECONNECTING]
 * and a failed one is [ConnectionPhase.FAILED]. The latter is still a *paired*
 * state — the room and its key are intact, which is why the shell keeps showing
 * the session list under an error heading rather than dropping back to the form.
 * What is still absent is an automatic re-pair (§11.11): an account room's
 * password is never persisted, so the user has to do that one by hand.
 */
public fun PairingUiState.connectionPhase(): ConnectionPhase = when (this) {
    PairingUiState.Idle -> ConnectionPhase.IDLE
    PairingUiState.Connecting -> ConnectionPhase.CONNECTING
    is PairingUiState.Paired -> when (liveness) {
        ConnectionLiveness.LIVE -> ConnectionPhase.CONNECTED
        ConnectionLiveness.CHECKING -> ConnectionPhase.RECONNECTING
        ConnectionLiveness.LOST -> ConnectionPhase.FAILED
    }

    is PairingUiState.Failed -> ConnectionPhase.FAILED
}
