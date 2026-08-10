package com.bitfun.mobile.core.protocol

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlinx.serialization.json.Json

class EncryptedPayloadTest {
    @Test
    fun decodesSnakeCaseWireNames() {
        val decoded = Json.decodeFromString<EncryptedPayload>(
            """{"encrypted_data":"AAAB","nonce":"CCCD"}""",
        )
        assertEquals("AAAB", decoded.encryptedData)
        assertEquals("CCCD", decoded.nonce)
    }

    @Test
    fun encodesBackToWireNames() {
        val encoded = Json.encodeToString(EncryptedPayload("AAAB", "CCCD"))
        assertEquals("""{"encrypted_data":"AAAB","nonce":"CCCD"}""", encoded)
    }
}
