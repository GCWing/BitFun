package com.bitfun.mobile.core.crypto

/**
 * Any failure in the relay handshake or in payload encryption.
 *
 * Messages carry field names and lengths only. Key material, nonces and
 * plaintext must never reach a message, because these propagate into user-facing
 * errors and into logs.
 */
public class RemoteCryptoException(
    message: String,
    cause: Throwable? = null,
) : Exception(message, cause)
