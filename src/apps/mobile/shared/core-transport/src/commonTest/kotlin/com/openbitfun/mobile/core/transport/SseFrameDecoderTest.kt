package com.openbitfun.mobile.core.transport

import kotlin.test.Test
import kotlin.test.assertEquals

class SseFrameDecoderTest {
    @Test
    fun buffersSplitFramesAndJoinsMultipleDataLines() {
        val decoder = SseFrameDecoder()

        assertEquals(emptyList(), decoder.feed("event: update\ndata: {\"part\":"))
        assertEquals(
            listOf("{\"part\":\n\"value\"}"),
            decoder.feed("\ndata: \"value\"}\n\n"),
        )
    }

    @Test
    fun normalizesCrlfAndIgnoresDoneAndComments() {
        val decoder = SseFrameDecoder()
        val payloads = decoder.feed(
            ": heartbeat\r\ndata: {\"delta\":\"Hello\"}\r\n\r\n" +
                "data: [DONE]\r\n\r\n",
        )

        assertEquals(listOf("{\"delta\":\"Hello\"}"), payloads)
        assertEquals(emptyList(), decoder.finish())
    }

    @Test
    fun finishFlushesTrailingFrame() {
        val decoder = SseFrameDecoder()
        decoder.feed("data: trailing")
        assertEquals(listOf("trailing"), decoder.finish())
    }
}
