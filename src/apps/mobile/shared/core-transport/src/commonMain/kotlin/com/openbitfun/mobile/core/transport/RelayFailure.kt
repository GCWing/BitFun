package com.openbitfun.mobile.core.transport

/**
 * Why a relay exchange failed, as a value rather than a sentence.
 *
 * `RelayHttpClient.ets` maps HTTP status codes straight to localized strings
 * inside the transport, which is why its error handling then has to
 * string-compare those same translations to avoid double-wrapping them. Shared
 * code has no locale, so the mapping stops here and the app layer turns a
 * [RelayFailure] into text.
 */
public sealed interface RelayFailure {
    /** The relay accepted the request but the desktop refused to pair. 401 / 403. */
    public data object PairRejected : RelayFailure

    /** No such room, usually a stale or already-consumed pairing URL. 404. */
    public data object RoomNotFound : RelayFailure

    /** The desktop did not answer in time. 408 / 504, or a client-side timeout. */
    public data object Timeout : RelayFailure

    /** 429. */
    public data object RateLimited : RelayFailure

    /** The relay itself is unhealthy. 5xx. */
    public data class RelayUnavailable(val statusCode: Int) : RelayFailure

    /** Any other non-2xx status. */
    public data class UnexpectedStatus(val statusCode: Int) : RelayFailure

    /** The connection never got far enough to produce a status. */
    public data object NetworkUnreachable : RelayFailure

    /** A 2xx body that is not the envelope we expect, or fails to decrypt. */
    public data object MalformedResponse : RelayFailure

    /**
     * The desktop answered `{"resp":"error"}`.
     *
     * [message] is the peer's own text. It is already user-facing on the desktop
     * and there is no code to map it to, so it is passed through as-is rather
     * than invented here.
     */
    public data class RemoteRejected(val message: String?) : RelayFailure
}

/** Thrown by every transport entry point; carries a [failure] the UI can switch on. */
public class RelayTransportException(
    public val failure: RelayFailure,
    cause: Throwable? = null,
) : Exception(failure.toString(), cause)

internal fun httpFailureFor(statusCode: Int): RelayFailure = when {
    statusCode == 401 || statusCode == 403 -> RelayFailure.PairRejected
    statusCode == 404 -> RelayFailure.RoomNotFound
    statusCode == 408 || statusCode == 504 -> RelayFailure.Timeout
    statusCode == 429 -> RelayFailure.RateLimited
    statusCode >= 500 -> RelayFailure.RelayUnavailable(statusCode)
    else -> RelayFailure.UnexpectedStatus(statusCode)
}
