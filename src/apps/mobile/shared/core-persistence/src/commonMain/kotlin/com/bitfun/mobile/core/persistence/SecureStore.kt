package com.bitfun.mobile.core.persistence

/** Platform-backed secret storage. Implementations must never persist plaintext values. */
public interface SecureStore {
    public fun read(key: String): ByteArray?

    public fun write(key: String, value: ByteArray)

    public fun delete(key: String)
}
