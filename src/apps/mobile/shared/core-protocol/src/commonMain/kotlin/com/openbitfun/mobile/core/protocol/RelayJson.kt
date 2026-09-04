package com.openbitfun.mobile.core.protocol

import kotlinx.serialization.json.Json

/**
 * The single [Json] configuration every relay exchange goes through.
 *
 * `ignoreUnknownKeys` is not a convenience here — it is the contract. The relay
 * peer is a desktop build that ships independently of the mobile apps, so an
 * older client will always be reading payloads with fields it has never heard
 * of. Failing on those would turn every desktop feature release into a forced
 * mobile update.
 *
 * `explicitNulls = false` keeps outbound commands minimal: the HarmonyOS client
 * omits absent optional fields rather than sending `"field": null`, and some
 * command handlers distinguish the two.
 */
public val RelayJson: Json = Json {
    ignoreUnknownKeys = true
    explicitNulls = false
    isLenient = false
    encodeDefaults = false
}
