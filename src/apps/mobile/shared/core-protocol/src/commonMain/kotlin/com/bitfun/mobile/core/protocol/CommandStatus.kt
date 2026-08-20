package com.bitfun.mobile.core.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonObject

/**
 * Every command reply carries these two fields.
 *
 * `CommandStatusResponse` is an `extends` base in `model/RemoteModels.ets`.
 * Kotlin data classes cannot inherit serializable properties, so each reply
 * declares `resp` and `message` itself and implements this interface — callers
 * that only need the status still get one type to work against.
 */
public interface CommandStatus {
    /** Peer-reported outcome. `"error"` means [message] explains why. */
    public val resp: String?
    public val message: String?
}

/** True when the peer signalled a failure rather than a result. */
public val CommandStatus.isError: Boolean get() = resp == "error"

/** A reply with no payload beyond the status. */
@Serializable
public data class CommandStatusResponse(
    @SerialName("resp") override val resp: String? = null,
    @SerialName("message") override val message: String? = null,
) : CommandStatus

internal fun Decoder.requireJsonObject(typeName: String): JsonObject {
    val input = this as? JsonDecoder
        ?: throw IllegalStateException("$typeName can only be read from JSON")
    return input.decodeJsonElement() as? JsonObject
        ?: throw IllegalStateException("$typeName expected a JSON object")
}

internal fun Encoder.requireJsonEncoder(typeName: String): JsonEncoder =
    this as? JsonEncoder ?: throw IllegalStateException("$typeName can only be written as JSON")
