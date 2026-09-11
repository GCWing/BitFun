package com.openbitfun.mobile.core.crypto

import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertContentEquals
import kotlin.test.assertFailsWith

class DeviceIdentityTest {
    @Test
    fun deviceKeysMatchRustAndBrowserVector() = runTest {
        val a = ByteArray(32) { 7 }
        val b = ByteArray(32) { 11 }
        val key = DeviceIdentity.messageKey(a, DeviceIdentity.publicKey(b))
        assertEquals("6e8f5da837e91e9ddb09c5aa7dee229e731fc94499d29d10dcf5f1437193f56c", key.joinToString("") { (it.toInt() and 255).toString(16).padStart(2, '0') })
        assertContentEquals(key, DeviceIdentity.messageKey(b, DeviceIdentity.publicKey(a)))
        assertFailsWith<Exception> { DeviceIdentity.messageKey(a, ByteArray(32)) }
    }
}
