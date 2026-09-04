package com.openbitfun.mobile.core.feature.pairing

/**
 * How this install identifies itself to the desktop.
 *
 * [installId] must be stable across launches — the desktop de-duplicates
 * devices by it — and must not be a hardware identifier. The app generates one
 * on first run and keeps it.
 */
public data class DeviceIdentity(
    val installId: String,
    val displayName: String,
)

/**
 * What the desktop reported once the handshake completed.
 *
 * This is deliberately not `InitialSyncResponse`: wire DTOs stay behind the
 * seam, so a defensive field the protocol layer added for a lenient server
 * cannot leak into a screen.
 */
public data class PairedWorkspace(
    /** Already truncated. The full room id never crosses this seam. */
    val roomLabel: String,
    val projectName: String?,
    val hasWorkspace: Boolean,
    val authenticatedUserId: String?,
)

/**
 * Why pairing did not complete, as a cause rather than a sentence.
 *
 * The app owns every user-facing string; this enum is what it switches on. The
 * split is by what the user can *do* next, which is why 401 and 403 collapse
 * into [Rejected] while 404 stays separate: one means "check your credentials",
 * the other means "the desktop is not listening on that room any more".
 */
public enum class PairingFailureReason {
    /** No pairing link was entered. */
    PairingLinkEmpty,

    /** The link parsed but carried no room id or no public key. */
    PairingLinkIncomplete,

    /** The link's query string could not be decoded. */
    PairingLinkUndecodable,

    /** The link's public key is not a usable X25519 point. */
    PairingLinkKeyUnusable,

    /** The room advertises `auth=account` and no username could be resolved. */
    AccountUsernameRequired,

    /** The room advertises `auth=account` and no password was supplied. */
    AccountPasswordRequired,

    /** The relay refused the pair request (401 / 403). */
    Rejected,

    /** No such room on the relay (404). The desktop is probably not sharing. */
    RoomNotFound,

    /** The relay is throttling this client (429). */
    RateLimited,

    /** The relay itself failed (5xx). */
    RelayUnavailable,

    /** The request never reached the relay. */
    NetworkUnreachable,

    /** The relay or the desktop did not answer in time. */
    Timeout,

    /**
     * A 200 that was not the encrypted envelope, or an envelope that would not
     * decrypt — a captive portal, or a desktop on an incompatible version.
     */
    ProtocolMismatch,

    /** The handshake reached the desktop and the desktop said no. */
    DesktopRejected,

    /**
     * Too many credentials were refused in a row, so pairing is on a cooldown.
     *
     * Ports the `MAX_FAILED_USER_ID_ATTEMPTS` / `USER_ID_LOCKOUT_MS` pair in
     * `ConnectionErrorPolicy`. The wait is in
     * [PairingFailure.retryAfterSeconds] — this is the one reason whose sentence
     * needs a number in it.
     */
    TooManyAttempts,
}

/**
 * @param remoteMessage verbatim text from the desktop, present only for
 * [PairingFailureReason.DesktopRejected]. It is not localized here and cannot
 * be — it was written by the peer. Apps show it as supporting detail under
 * their own heading for the reason, never as the whole message.
 * @param retryAfterSeconds how long the cooldown still has to run, for
 * [PairingFailureReason.TooManyAttempts] and `0` for everything else. Seconds
 * rather than a deadline: a screen shows a duration, and an instant would make
 * every caller do the same subtraction against the same clock.
 */
public data class PairingFailure(
    val reason: PairingFailureReason,
    val remoteMessage: String?,
    val statusCode: Int?,
    val retryAfterSeconds: Int,
) {
    /**
     * Most reasons carry no detail. A secondary constructor rather than a
     * default argument, because Swift does not see Kotlin defaults — see the
     * design doc §4.1.
     */
    public constructor(reason: PairingFailureReason) : this(reason, null, null, 0)

    /** Everything the relay can say, none of which is a cooldown. */
    public constructor(
        reason: PairingFailureReason,
        remoteMessage: String?,
        statusCode: Int?,
    ) : this(reason, remoteMessage, statusCode, 0)

    /**
     * Whether the pairing link itself is what needs fixing.
     *
     * Ports `ConnectionErrorResult.shouldShowRemoteUrlInput`, which is the
     * `expired_room` / `invalid_link` half of `failureKindFor`. A screen that
     * hides the link behind a scan button — as both clients do — has to put it
     * back on screen for exactly these, and only these: nothing the user can
     * type will help with a relay outage, and re-opening the field would only
     * suggest the link were at fault.
     *
     * [PairingFailureReason.Rejected] is deliberately not here. The ArkTS
     * source folds its `pairRejected` into `expired_room`, but only after
     * `protectedUserIdError` has already claimed the credential cases; here a
     * 401/403 *is* the credential case, so it counts toward the cooldown
     * instead — see [PairingFailureReason.TooManyAttempts].
     */
    public val reopensLinkInput: Boolean
        get() = when (reason) {
            PairingFailureReason.PairingLinkEmpty,
            PairingFailureReason.PairingLinkIncomplete,
            PairingFailureReason.PairingLinkUndecodable,
            PairingFailureReason.PairingLinkKeyUnusable,
            PairingFailureReason.RoomNotFound,
            PairingFailureReason.ProtocolMismatch,
            -> true

            else -> false
        }
}

/**
 * Whether the paired desktop is still answering.
 *
 * Ported from what `RemoteActivityViewModel.checkConnectionHealth` does with its
 * `connectionState`: a room that stops replying is not the same as no room at
 * all, so the pairing survives and only its liveness changes. [LOST] is
 * therefore recoverable — the room, its key and its transport are all still
 * here, and one successful ping puts it back to [LIVE].
 */
public enum class ConnectionLiveness {
    /** The last check succeeded. */
    LIVE,

    /** A check the user was told about is in flight. */
    CHECKING,

    /** The last check failed. Nothing was torn down; a retry may still succeed. */
    LOST,
}

/** The pairing screen's entire state. */
public sealed interface PairingUiState {
    /** Whether a submit would be accepted right now. */
    public val acceptsSubmit: Boolean

    public data object Idle : PairingUiState {
        override val acceptsSubmit: Boolean get() = true
    }

    public data object Connecting : PairingUiState {
        override val acceptsSubmit: Boolean get() = false
    }

    public data class Paired(
        val workspace: PairedWorkspace,
        val liveness: ConnectionLiveness,
    ) : PairingUiState {
        /**
         * A pairing that has just completed, which is live by definition — the
         * handshake was the round trip. A secondary constructor rather than a
         * default argument, because Swift does not see Kotlin defaults.
         */
        public constructor(workspace: PairedWorkspace) : this(workspace, ConnectionLiveness.LIVE)

        override val acceptsSubmit: Boolean get() = false
    }

    public data class Failed(val failure: PairingFailure) : PairingUiState {
        override val acceptsSubmit: Boolean get() = true
    }
}

/** Everything the pairing screen can ask for. */
public sealed interface PairingIntent {
    /**
     * @param password only read for rooms that advertise `auth=account`; it is
     * forwarded into one encrypted command and never retained or logged.
     */
    public data class Submit(
        val pairingUrl: String,
        val userId: String,
        val password: String,
    ) : PairingIntent {
        /**
         * A link with no credentials, which is every room that does not
         * advertise `auth=account`. Spelled as a constructor rather than as
         * default arguments so Swift sees both forms — see the design doc §4.1.
         */
        public constructor(pairingUrl: String) : this(pairingUrl, "", "")

        override fun toString(): String =
            "Submit(pairingUrl=<redacted>, userId=$userId, " +
                "password=${if (password.isEmpty()) "absent" else "redacted"})"
    }

    /** Clears a failure so the form is editable again. */
    public data object Dismiss : PairingIntent

    /** Abandons the paired room and returns to the form. */
    public data object Disconnect : PairingIntent

    /**
     * The surface came to the front: start the heartbeat and check the link once,
     * visibly. Ports `RemoteActivityViewModel.resume`.
     */
    public data object Foreground : PairingIntent

    /**
     * The surface went away: stop the heartbeat. Nothing else is torn down — the
     * room outlives a trip to the home screen, and the next [Foreground] says
     * whether it is still answering.
     */
    public data object Background : PairingIntent

    /**
     * Check the link now and show that it is being checked.
     *
     * This is a health check, not a re-pair: an account room's password is never
     * persisted, so a room that is genuinely gone has to be paired again by hand
     * rather than silently behind the user's back.
     */
    public data object Verify : PairingIntent
}
