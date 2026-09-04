package com.openbitfun.mobile.core.crypto

import com.openbitfun.mobile.core.protocol.EncryptedPayload
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.Serializable
import kotlin.random.Random
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

@Serializable
private data class Probe(val cmd: String, val sessionId: String)

class RemoteCryptoTest {
    private suspend fun pairedSessions(): Pair<RemoteCryptoSession, RemoteCryptoSession> {
        val client = RemoteHandshake.create(random = Random(1))
        val peer = RemoteHandshake.create(random = Random(2))
        return client.accept(peer.publicKeyBase64) to peer.accept(client.publicKeyBase64)
    }

    @Test
    fun aPairedPeerDecryptsWhatTheClientEncrypted() = runTest {
        val (client, peer) = pairedSessions()
        val payload = client.encryptJson(Probe.serializer(), Probe("poll_session", "s1"))

        assertEquals(Probe("poll_session", "s1"), peer.decryptJson(Probe.serializer(), payload))
    }

    @Test
    fun everyEnvelopeCarriesAFreshNonce() = runTest {
        // Two envelopes under the same key sharing a nonce would leak both
        // plaintexts, so this is a correctness property and not a style check.
        val (client, _) = pairedSessions()
        val message = Probe("poll_session", "s1")
        val first = client.encryptJson(Probe.serializer(), message)
        val second = client.encryptJson(Probe.serializer(), message)

        assertTrue(first.nonce != second.nonce)
        assertTrue(first.encryptedData != second.encryptedData)
    }

    @Test
    fun theEnvelopeIsBase64OfNonceAndTaggedCiphertext() = runTest {
        val (client, _) = pairedSessions()
        val payload = client.encryptJson(Probe.serializer(), Probe("ping", "s1"))

        assertEquals(GCM_NONCE_SIZE, Base64Codec.decode(payload.nonce, "nonce").size)
        val plaintextSize = """{"cmd":"ping","sessionId":"s1"}""".length
        assertEquals(
            plaintextSize + GCM_TAG_SIZE,
            Base64Codec.decode(payload.encryptedData, "encrypted_data").size,
        )
    }

    @Test
    fun anUnrelatedPeerCannotDecrypt() = runTest {
        val (client, _) = pairedSessions()
        val (eavesdropper, _) = pairedSessions()
        val payload = client.encryptJson(Probe.serializer(), Probe("ping", "s1"))

        assertFailsWith<RemoteCryptoException> {
            eavesdropper.decryptJson(Probe.serializer(), payload)
        }
    }

    @Test
    fun unpaddedBase64FromThePeerStillDecodes() = runTest {
        val (client, peer) = pairedSessions()
        val payload = client.encryptJson(Probe.serializer(), Probe("ping", "s1"))
        val unpadded = EncryptedPayload(
            encryptedData = payload.encryptedData.trimEnd('='),
            nonce = payload.nonce.trimEnd('='),
        )

        assertEquals(Probe("ping", "s1"), peer.decryptJson(Probe.serializer(), unpadded))
    }

    @Test
    fun aMalformedEnvelopeNamesTheFieldWithoutEchoingIt() = runTest {
        val (_, peer) = pairedSessions()
        val failure = assertFailsWith<RemoteCryptoException> {
            peer.decryptJson(
                Probe.serializer(),
                EncryptedPayload(encryptedData = "!!not base64!!", nonce = "AAAAAAAAAAAAAAAA"),
            )
        }
        assertTrue("encrypted_data" in failure.message.orEmpty())
        assertTrue("!!not base64!!" !in failure.message.orEmpty())
    }

    @Test
    fun aPeerKeyThatIsNotBase64FailsTheHandshake() = runTest {
        val client = RemoteHandshake.create()
        val failure = assertFailsWith<RemoteCryptoException> { client.accept("!!!") }
        assertTrue("peer public key" in failure.message.orEmpty())
    }

    @Test
    fun theHandshakePublicKeyIsThirtyTwoBase64EncodedBytes() = runTest {
        val client = RemoteHandshake.create()
        assertEquals(
            X25519_KEY_SIZE,
            Base64Codec.decode(client.publicKeyBase64, "public key").size,
        )
    }
}
