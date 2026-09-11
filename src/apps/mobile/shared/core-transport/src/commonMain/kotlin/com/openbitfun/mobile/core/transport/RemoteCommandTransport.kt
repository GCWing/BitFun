package com.openbitfun.mobile.core.transport

import com.openbitfun.mobile.core.protocol.CommandStatus
import com.openbitfun.mobile.core.protocol.RemoteCommand
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.serializer

/** A command addressed to a device in the authenticated account directory. */
public interface RemoteCommandTransport {
    public suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long = RELAY_DEFAULT_TIMEOUT_MS,
    ): T
}

public suspend inline fun <reified T : CommandStatus> RemoteCommandTransport.send(
    command: RemoteCommand,
    timeoutMs: Long = RELAY_DEFAULT_TIMEOUT_MS,
): T = send(serializer(), command, timeoutMs)
