package com.openbitfun.mobile.core.persistence

import androidx.test.platform.app.InstrumentationRegistry
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertNull

class AndroidSecureStoreTest {
    @Test
    fun encryptsRoundTripsAndDeletesSecret() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val store = androidSecureStore(context, "device_test")
        store.delete("session")
        val secret = "token-and-master-key".encodeToByteArray()

        store.write("session", secret)

        assertContentEquals(secret, store.read("session"))
        store.delete("session")
        assertNull(store.read("session"))
    }
}
