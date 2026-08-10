package com.bitfun.mobile.core.crypto

import kotlinx.coroutines.test.runTest
import kotlin.io.encoding.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class CloudAccountKdfTest {
    @Test
    fun matchesRelayAccountArgon2idVector() = runTest {
        val salt = ByteArray(16) { it.toByte() }
        val output = PlatformArgon2id.derive(
            password = "correct horse battery staple",
            salt = salt,
            params = CloudAccountKdfParams(8 * 1024, 1, 1),
        )

        assertEquals("mu73UxPlhfSSwzxeEtgumtJTt914Yy1Tfomc1O3deJw=", Base64.Default.encode(output))
    }

    @Test
    fun rejectsUntrustedCostParametersBeforeAllocation() = runTest {
        assertFailsWith<IllegalArgumentException> {
            PlatformArgon2id.derive("password", ByteArray(7), CloudAccountKdfParams(8 * 1024, 1, 1))
        }
        assertFailsWith<IllegalArgumentException> {
            PlatformArgon2id.derive("password", ByteArray(16), CloudAccountKdfParams(512 * 1024, 1, 1))
        }
    }
}
