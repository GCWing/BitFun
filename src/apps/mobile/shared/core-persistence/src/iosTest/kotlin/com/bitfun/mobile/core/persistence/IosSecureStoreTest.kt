package com.bitfun.mobile.core.persistence

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import platform.Security.errSecItemNotFound

/** Runs against the real Keychain on an iOS simulator/device, not a memory fake. */
class IosSecureStoreTest {
    @Test
    fun readStatusDistinguishesMissingFromFailure() {
        assertEquals(KeychainReadResult.FOUND, classifyKeychainReadStatus(0))
        assertEquals(KeychainReadResult.MISSING, classifyKeychainReadStatus(errSecItemNotFound))
        assertFailsWith<IllegalStateException> { classifyKeychainReadStatus(-50) }
    }

    @Test
    fun roundTripsUpdatesAndDeletesSecret() {
        val store = iosSecureStore("com.bitfun.mobile.tests")
        val key = "secure-store-test"
        val first = byteArrayOf(1, 2, 3, 4)
        val second = byteArrayOf(9, 8, 7)
        try {
            store.delete(key)
            store.write(key, first)
            assertContentEquals(first, store.read(key))
            store.write(key, second)
            assertContentEquals(second, store.read(key))
            store.write(key, byteArrayOf())
            assertContentEquals(byteArrayOf(), store.read(key))
        } finally {
            store.delete(key)
        }
        assertNull(store.read(key))
        store.delete(key)
    }
}
