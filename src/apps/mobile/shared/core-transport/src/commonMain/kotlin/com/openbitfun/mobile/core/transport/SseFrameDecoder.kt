package com.openbitfun.mobile.core.transport

/** Incremental SSE frame decoder. Comments and non-data fields are ignored. */
public class SseFrameDecoder public constructor() {
    private var buffer: String = ""

    public fun feed(chunk: String): List<String> {
        buffer += chunk.replace("\r\n", "\n")
        val payloads = mutableListOf<String>()
        while (true) {
            val boundary = buffer.indexOf("\n\n")
            if (boundary < 0) break
            val frame = buffer.substring(0, boundary)
            buffer = buffer.substring(boundary + 2)
            val data = frame.lineSequence()
                .filter { it.startsWith("data:") }
                .map { it.removePrefix("data:").removePrefix(" ") }
                .joinToString("\n")
            if (data.isNotEmpty() && data != "[DONE]") payloads += data
        }
        return payloads
    }

    public fun finish(): List<String> {
        if (buffer.isEmpty()) return emptyList()
        val trailing = feed("\n\n")
        buffer = ""
        return trailing
    }
}
