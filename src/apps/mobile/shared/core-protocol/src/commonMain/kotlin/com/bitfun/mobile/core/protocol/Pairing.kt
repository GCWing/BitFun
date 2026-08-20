package com.bitfun.mobile.core.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Room pairing, ported from `services/RelayHttpClient.ets`.
 *
 * The exchange is: derive a shared key from the descriptor's public key, POST
 * [PairRequest] to `/api/rooms/{roomId}/pair`, decrypt the [PairChallengeResponse],
 * then POST an encrypted [ChallengeCommand] to `/api/rooms/{roomId}/command` and
 * decrypt the [InitialSyncResponse].
 */
@Serializable
public data class PairRequest(
    @SerialName("public_key") val publicKey: String,
    @SerialName("device_id") val deviceId: String,
    @SerialName("device_name") val deviceName: String,
)

/**
 * The desktop's challenge, matching `PairingChallenge` in
 * `services-integrations/src/remote_connect/pairing.rs`.
 *
 * [challenge] is 32 lowercase hex characters — the desktop compares the echo
 * byte for byte and rejects anything else. [timestamp] is unix seconds and the
 * desktop expires the challenge 120 seconds after it, but the client has no use
 * for it beyond diagnostics, so it stays optional rather than forcing a decode
 * failure on a peer that omits it.
 */
@Serializable
public data class PairChallengeResponse(
    @SerialName("challenge") val challenge: String,
    @SerialName("timestamp") val timestamp: Long? = null,
)

/**
 * The echo that proves the client decrypted the challenge.
 *
 * [password] is only present when the room requires account auth. It is a
 * credential: it must never be logged, and it must not be retained after the
 * command is sent.
 */
@Serializable
public data class ChallengeCommand(
    @SerialName("challenge_echo") val challengeEcho: String,
    @SerialName("device_id") val deviceId: String,
    @SerialName("device_name") val deviceName: String,
    @SerialName("mobile_install_id") val mobileInstallId: String,
    @SerialName("user_id") val userId: String,
    @SerialName("password") val password: String? = null,
) {
    /** Keeps the password out of crash reports and debug output. */
    override fun toString(): String =
        "ChallengeCommand(deviceId=$deviceId, userId=$userId, password=${if (password == null) "absent" else "redacted"})"
}

/**
 * The delegated identity a paired device uses for account-scoped RPC.
 *
 * [masterKey] and [token] are secrets. They belong in the platform secure store
 * (Keystore / Keychain), never in SQLDelight or plain preferences.
 */
@Serializable
public data class DelegatedIdentityResponse(
    @SerialName("resp") override val resp: String? = null,
    @SerialName("message") override val message: String? = null,
    @SerialName("token") val token: String? = null,
    @SerialName("user_id") val userId: String? = null,
    @SerialName("master_key") val masterKey: String? = null,
    @SerialName("device_id") val deviceId: String? = null,
) : CommandStatus {
    override fun toString(): String =
        "DelegatedIdentityResponse(resp=$resp, userId=$userId, deviceId=$deviceId, " +
            "token=${token.redactedLabel()}, masterKey=${masterKey.redactedLabel()})"
}

private fun String?.redactedLabel(): String = if (this == null) "absent" else "redacted"
