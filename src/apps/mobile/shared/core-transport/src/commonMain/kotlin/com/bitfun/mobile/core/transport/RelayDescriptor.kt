package com.bitfun.mobile.core.transport

import io.ktor.http.decodeURLQueryComponent

/**
 * Everything a pairing URL (or QR code) carries, ported from `RemoteDescriptor`
 * in `model/RemoteModels.ets`.
 *
 * [publicKey] is the desktop's X25519 public key — public by construction — but
 * [roomId] is a bearer capability for the pairing exchange, which is why
 * [toString] truncates it. See [RelayDescriptorParser] for the accepted shapes.
 */
public data class RelayDescriptor(
    val relayUrl: String,
    val roomId: String,
    val publicKey: String,
    val accountAuth: Boolean,
    val accountUsername: String,
) {
    override fun toString(): String =
        "RelayDescriptor(relayUrl=$relayUrl, roomId=${shortRoomId(roomId)}, " +
            "accountAuth=$accountAuth, accountUsername=$accountUsername)"
}

/** Why a pairing URL could not be read. */
public enum class RelayDescriptorProblem {
    /** Blank input. */
    Empty,

    /** `room` or `pk` is absent or empty. */
    MissingParameters,

    /** A percent-escape in the query is not decodable. */
    UndecodableQuery,
}

public class RelayDescriptorException(
    public val problem: RelayDescriptorProblem,
    cause: Throwable? = null,
) : Exception(problem.name, cause)

/**
 * Reads the pairing descriptor out of whatever the desktop put on screen.
 *
 * Three shapes are accepted, in the order the HarmonyOS parser tries them:
 * a hash route (`https://host/r/x#/pair?room=…`), any URL with a query string,
 * and a bare query string with no URL around it. The last one exists because
 * the desktop's own pairing text has been pasted by hand.
 */
public object RelayDescriptorParser {
    private const val PAIR_ROUTE = "#/pair?"

    private val WSS_SCHEME = Regex("^wss://")
    private val WS_SCHEME = Regex("^ws://")
    private val WS_SUFFIX = Regex("/ws/?$")
    private val TRAILING_SLASH = Regex("/$")
    private val LAST_PATH_SEGMENT = Regex("/[^/]*$")

    /**
     * @throws RelayDescriptorException when [input] is blank, is missing `room`
     * or `pk`, or contains an invalid percent-escape.
     */
    public fun parse(input: String): RelayDescriptor {
        val value = input.trim()
        if (value.isEmpty()) throw RelayDescriptorException(RelayDescriptorProblem.Empty)

        val params = parseQuery(extractQuery(value))
        val roomId = params["room"].orEmpty()
        val publicKey = params["pk"].orEmpty()
        if (roomId.isEmpty() || publicKey.isEmpty()) {
            throw RelayDescriptorException(RelayDescriptorProblem.MissingParameters)
        }

        return RelayDescriptor(
            relayUrl = resolveRelayBaseUrl(value, params["relay"].orEmpty()),
            roomId = roomId,
            publicKey = publicKey,
            accountAuth = params["auth"] == "account",
            accountUsername = params["user"].orEmpty().trim(),
        )
    }

    /**
     * Whether [input] describes a room that requires account credentials.
     *
     * Returns `false` for anything unparseable, matching the HarmonyOS helper:
     * the pairing screen calls this while the user is still typing, so a partial
     * URL is an ordinary state rather than an error.
     */
    public fun accountAuthRequired(input: String): Boolean = parseOrNull(input)?.accountAuth == true

    /** The `user` hint from [input], or `""` when there is none. */
    public fun accountUsername(input: String): String =
        parseOrNull(input)?.accountUsername.orEmpty()

    private fun parseOrNull(input: String): RelayDescriptor? =
        try {
            parse(input)
        } catch (_: RelayDescriptorException) {
            null
        }

    private fun extractQuery(value: String): String {
        val pairIndex = value.indexOf(PAIR_ROUTE)
        if (pairIndex >= 0) return value.substring(pairIndex + PAIR_ROUTE.length)
        val questionIndex = value.indexOf('?')
        if (questionIndex >= 0) return value.substring(questionIndex + 1)
        return value
    }

    private fun parseQuery(query: String): Map<String, String> {
        val params = mutableMapOf<String, String>()
        for (pair in query.split('&')) {
            val eq = pair.indexOf('=')
            val rawKey = if (eq >= 0) pair.substring(0, eq) else pair
            val rawValue = if (eq >= 0) pair.substring(eq + 1) else ""
            if (rawKey.isEmpty()) continue
            params[decodeComponent(rawKey)] = decodeComponent(rawValue)
        }
        return params
    }

    // decodeURLQueryComponent defaults to plusIsSpace = false, which is what
    // matches the JS decodeURIComponent the desktop and HarmonyOS both use. A
    // literal '+' in a base64 public key must survive as '+'.
    private fun decodeComponent(raw: String): String =
        try {
            raw.decodeURLQueryComponent()
        } catch (cause: Throwable) {
            throw RelayDescriptorException(RelayDescriptorProblem.UndecodableQuery, cause)
        }

    private fun resolveRelayBaseUrl(fullUrl: String, relayParam: String): String {
        // An explicit relay= wins, after being normalised from the websocket URL
        // the desktop advertises to the HTTP origin this client posts to.
        val relay = relayParam
            .let { WSS_SCHEME.replaceFirst(it, "https://") }
            .let { WS_SCHEME.replaceFirst(it, "http://") }
            .let { WS_SUFFIX.replaceFirst(it, "") }
            .let { TRAILING_SLASH.replaceFirst(it, "") }
        if (relay.isNotEmpty()) return relay

        val withoutHash = fullUrl.substringBefore('#')
        val withoutQuery = withoutHash.substringBefore('?')

        // A relay-hosted pairing page lives under /r/<room>, so the origin is
        // everything before that segment — which may include a path prefix when
        // the relay is mounted behind a reverse proxy.
        val relayRouteIndex = withoutQuery.indexOf("/r/")
        if (relayRouteIndex >= 0) {
            return TRAILING_SLASH.replaceFirst(withoutQuery.substring(0, relayRouteIndex), "")
        }

        val schemeIndex = withoutQuery.indexOf("://")
        if (schemeIndex >= 0) {
            val pathIndex = withoutQuery.indexOf('/', schemeIndex + 3)
            val origin = if (pathIndex >= 0) withoutQuery.substring(0, pathIndex) else withoutQuery
            return TRAILING_SLASH.replaceFirst(origin, "")
        }

        return TRAILING_SLASH.replaceFirst(LAST_PATH_SEGMENT.replaceFirst(withoutQuery, ""), "")
    }
}

/** First 8 characters — enough to correlate log lines, not enough to replay. */
internal fun shortRoomId(roomId: String): String =
    if (roomId.length <= 8) roomId else roomId.substring(0, 8)

/** Last 12 characters; request ids are generated with the entropy at the end. */
internal fun shortRequestId(requestId: String): String = when {
    requestId.isEmpty() -> "none"
    requestId.length <= 12 -> requestId
    else -> requestId.substring(requestId.length - 12)
}
