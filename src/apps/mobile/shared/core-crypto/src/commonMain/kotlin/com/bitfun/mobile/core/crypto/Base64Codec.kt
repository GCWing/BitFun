package com.bitfun.mobile.core.crypto

import kotlin.io.encoding.Base64

/**
 * Base64 as the relay protocol uses it: standard alphabet, padded on the way
 * out, tolerant of missing padding on the way in.
 *
 * The tolerance is deliberate. Every base64 string we decode arrives from a peer
 * we do not ship with, and a padding mismatch would surface as a failed pairing
 * or an undecryptable message — a failure the user has no way to act on. Since
 * unpadded input decodes to exactly the same bytes, rejecting it buys nothing.
 */
internal object Base64Codec {
    private val codec: Base64 = Base64.Default.withPadding(Base64.PaddingOption.PRESENT_OPTIONAL)

    fun encode(bytes: ByteArray): String = Base64.Default.encode(bytes)

    /** @throws RemoteCryptoException when [text] is not valid base64. */
    fun decode(text: String, field: String): ByteArray =
        try {
            codec.decode(text)
        } catch (cause: IllegalArgumentException) {
            // The message names the field but never echoes the value: a decode
            // failure here can be a mangled key or nonce.
            throw RemoteCryptoException("Field '$field' is not valid base64.", cause)
        }
}
