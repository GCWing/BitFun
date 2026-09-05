package com.openbitfun.mobile.core.crypto

/**
 * Hex helpers for published test vectors, which are always quoted in hex.
 *
 * Kept in the test source set on purpose: nothing on the wire is hex, so
 * production code has no reason to carry this.
 */
internal fun String.hexToBytes(): ByteArray {
    require(length % 2 == 0) { "Hex string must have an even length." }
    return ByteArray(length / 2) { index ->
        substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }
}

internal fun ByteArray.toHex(): String = joinToString("") { byte ->
    val value = byte.toInt() and 0xFF
    value.toString(16).padStart(2, '0')
}
