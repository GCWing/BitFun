package com.bitfun.mobile.core.transport

import com.bitfun.mobile.core.protocol.ImageAttachment
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals

class ModelProviderStreamTest {
    @Test
    fun resolvesProviderUrlsWithoutDuplicatingPaths() {
        assertEquals(ModelProviderProtocol.ANTHROPIC, ModelProviderRequest.resolveProtocol("https://api.openbitfun.com"))
        assertEquals("https://api.example.com/v1/messages", ModelProviderRequest.resolveUrl("https://api.example.com", ModelProviderProtocol.ANTHROPIC))
        assertEquals("https://api.example.com/v1/chat/completions", ModelProviderRequest.resolveUrl("https://api.example.com/v1", ModelProviderProtocol.OPEN_AI))
        assertEquals("https://api.example.com/v1/chat/completions", ModelProviderRequest.resolveUrl("https://api.example.com/v1/chat/completions", ModelProviderProtocol.OPEN_AI))
    }

    @Test
    fun buildsProtocolSpecificRequestBodies() {
        val messages = listOf(ModelProviderMessage("user", "hello"))
        val anthropic = ModelProviderRequest.body("claude", messages, ModelProviderProtocol.ANTHROPIC, 40, "system facts")
        val openAi = ModelProviderRequest.body("gpt", messages, ModelProviderProtocol.OPEN_AI, 40, "system facts")

        assertContains(anthropic, "\"system\":\"system facts\"")
        assertContains(anthropic, "\"messages\":[{\"role\":\"user\"")
        assertContains(openAi, "{\"role\":\"system\",\"content\":\"system facts\"}")
        assertContains(openAi, "{\"role\":\"user\",\"content\":\"hello\"}")
    }

    @Test
    fun buildsProviderSpecificImageContent() {
        val image = ImageAttachment("photo.jpg", "data:image/png;base64,AAAA")
        val message = ModelProviderMessage("user", "look", listOf(image))

        val anthropic = ModelProviderRequest.body(
            "claude",
            listOf(message),
            ModelProviderProtocol.ANTHROPIC,
            40,
            "system facts",
        )
        val openAi = ModelProviderRequest.body(
            "gpt",
            listOf(message),
            ModelProviderProtocol.OPEN_AI,
            40,
            "system facts",
        )

        assertContains(anthropic, "\"type\":\"image\"")
        assertContains(anthropic, "\"media_type\":\"image/png\"")
        assertContains(anthropic, "\"data\":\"AAAA\"")
        assertContains(openAi, "\"type\":\"image_url\"")
        assertContains(openAi, "\"url\":\"data:image/png;base64,AAAA\"")
    }

    @Test
    fun parsesAnthropicDeltasSplitAcrossChunks() {
        val parser = ModelProviderSseParser(ModelProviderProtocol.ANTHROPIC)
        val first = parser.feed(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg-1\"}}\n\n" +
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_",
        )
        val second = parser.feed("delta\",\"text\":\"hello\"}}\n\n")

        assertEquals(emptyList(), first)
        assertEquals(listOf("hello"), second)
        assertEquals("msg-1", parser.responseId())
    }

    @Test
    fun parsesOpenAiCrlfAndIgnoresDoneMarker() {
        val parser = ModelProviderSseParser(ModelProviderProtocol.OPEN_AI)
        val deltas = parser.feed(
            "data: {\"id\":\"chat-1\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\r\n\r\n" +
                "data: {\"id\":\"chat-1\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\r\n\r\n" +
                "data: [DONE]\r\n\r\n",
        )

        assertEquals("Hello world", deltas.joinToString(""))
        assertEquals("chat-1", parser.responseId())
        assertEquals(emptyList(), parser.finish())
    }
}
