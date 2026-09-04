package com.openbitfun.mobile.core.transport

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/**
 * The first six cases are the ones already asserted for `RemoteDescriptorParser`
 * in `entry/src/test/TransportAndGeneralChatUnit.test.ets`, translated verbatim.
 * They are the evidence that this port is behaviour-equivalent, so their inputs
 * and expectations are copied rather than rewritten.
 */
class RelayDescriptorParserTest {
    @Test
    fun parsesHashRouteUrls() {
        val descriptor = RelayDescriptorParser.parse(
            "https://relay.example.com/r/mobile#/pair?room=room-a&pk=public-key",
        )
        assertEquals("room-a", descriptor.roomId)
        assertEquals("public-key", descriptor.publicKey)
        assertEquals("https://relay.example.com", descriptor.relayUrl)
    }

    @Test
    fun normalizesRelayWebsocketUrls() {
        val descriptor = RelayDescriptorParser.parse(
            "https://app.example.com/#/pair?room=room-b&pk=key-b&relay=wss%3A%2F%2Frelay.example.com%2Fws",
        )
        assertEquals("https://relay.example.com", descriptor.relayUrl)
    }

    @Test
    fun parsesAccountPairingMetadata() {
        val descriptor = RelayDescriptorParser.parse(
            "https://relay.example.com/#/pair?room=room-account&pk=key-account&auth=account&user=alice",
        )
        assertTrue(descriptor.accountAuth)
        assertEquals("alice", descriptor.accountUsername)
    }

    @Test
    fun parsesRawQueryStrings() {
        val descriptor = RelayDescriptorParser.parse(
            "room=room-c&pk=key%2Bc%3D&relay=http%3A%2F%2F127.0.0.1%3A30333",
        )
        assertEquals("room-c", descriptor.roomId)
        assertEquals("key+c=", descriptor.publicKey)
        assertEquals("http://127.0.0.1:30333", descriptor.relayUrl)
    }

    @Test
    fun rejectsUrlsMissingRoom() {
        val failure = assertFailsWith<RelayDescriptorException> {
            RelayDescriptorParser.parse("https://relay.example.com/#/pair?pk=key-only")
        }
        assertEquals(RelayDescriptorProblem.MissingParameters, failure.problem)
    }

    @Test
    fun rejectsUrlsMissingPublicKey() {
        val failure = assertFailsWith<RelayDescriptorException> {
            RelayDescriptorParser.parse("https://relay.example.com/#/pair?room=room-only")
        }
        assertEquals(RelayDescriptorProblem.MissingParameters, failure.problem)
    }

    @Test
    fun rejectsBlankInput() {
        val failure = assertFailsWith<RelayDescriptorException> {
            RelayDescriptorParser.parse("   \n ")
        }
        assertEquals(RelayDescriptorProblem.Empty, failure.problem)
    }

    /**
     * A base64 public key contains `+`, which `application/x-www-form-urlencoded`
     * decoding would turn into a space and silently corrupt the key. The desktop
     * and HarmonyOS both use `decodeURIComponent`, which does not.
     */
    @Test
    fun keepsPlusSignsInBase64Values() {
        val descriptor = RelayDescriptorParser.parse("room=room-d&pk=abc+def=")
        assertEquals("abc+def=", descriptor.publicKey)
    }

    @Test
    fun rejectsUndecodableEscapes() {
        val failure = assertFailsWith<RelayDescriptorException> {
            RelayDescriptorParser.parse("room=room-e&pk=%zz")
        }
        assertEquals(RelayDescriptorProblem.UndecodableQuery, failure.problem)
    }

    @Test
    fun trimsSurroundingWhitespaceFromPastedUrls() {
        val descriptor = RelayDescriptorParser.parse(
            "  https://relay.example.com/#/pair?room=room-f&pk=key-f\n",
        )
        assertEquals("room-f", descriptor.roomId)
    }

    @Test
    fun readsAccountHintsWithoutThrowingOnPartialInput() {
        assertFalse(RelayDescriptorParser.accountAuthRequired("https://relay.example.com/#/pair?ro"))
        assertEquals("", RelayDescriptorParser.accountUsername("not a url at all"))
        assertTrue(
            RelayDescriptorParser.accountAuthRequired(
                "https://relay.example.com/#/pair?room=r&pk=k&auth=account",
            ),
        )
    }

    /** A relay behind a path prefix keeps that prefix; only `/r/<room>` is cut. */
    @Test
    fun keepsAPathPrefixBeforeTheRelayRoute() {
        val descriptor = RelayDescriptorParser.parse(
            "https://example.com/openbitfun/r/mobile#/pair?room=room-g&pk=key-g",
        )
        assertEquals("https://example.com/openbitfun", descriptor.relayUrl)
    }

    @Test
    fun fallsBackToTheOriginWhenThereIsNoRelayRoute() {
        val descriptor = RelayDescriptorParser.parse(
            "https://relay.example.com/some/page?room=room-h&pk=key-h",
        )
        assertEquals("https://relay.example.com", descriptor.relayUrl)
    }

    /** The room id is a bearer capability for pairing; it must not print in full. */
    @Test
    fun toStringTruncatesTheRoomIdAndOmitsTheKey() {
        val descriptor = RelayDescriptorParser.parse(
            "room=0123456789abcdef&pk=secret-looking-key&relay=https%3A%2F%2Fr.example.com",
        )
        val rendered = descriptor.toString()
        assertFalse("0123456789abcdef" in rendered)
        assertFalse("secret-looking-key" in rendered)
        assertTrue("01234567" in rendered)
    }
}
