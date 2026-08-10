package com.bitfun.mobile.core.transport

/**
 * Where the transport writes its breadcrumbs, mirroring `RemoteLogger.ets`.
 *
 * Shared code cannot reach hilog / Logcat / OSLog, so the app supplies the sink.
 * The redaction discipline stays here rather than at the sink: every call site
 * in this module passes ids through [shortRoomId] / [shortRequestId] first, so
 * a full room id, token or key cannot reach an implementation of this interface
 * even if the app logs everything it is handed.
 */
public interface TransportLog {
    public fun info(message: String)

    public fun warn(message: String)

    public fun error(message: String)

    /** The default: the transport is silent unless the app asks for output. */
    public object None : TransportLog {
        override fun info(message: String) {}

        override fun warn(message: String) {}

        override fun error(message: String) {}
    }
}
