package com.bitfun.mobile.core.feature

/**
 * The logging seam. Core code cannot reach hilog, Logcat or OSLog, so the app
 * supplies the sink and the core supplies already-truncated lines.
 *
 * Everything handed to this interface is safe to print: room ids and request
 * ids are shortened before they get here, and keys, tokens and passwords never
 * do. An app may forward every line verbatim without leaking a full room id.
 */
public interface CoreLog {
    public fun info(message: String)

    public fun warn(message: String)

    public fun error(message: String)

    /** Discards everything. The default, so tests and previews need no sink. */
    public object None : CoreLog {
        override fun info(message: String) {}

        override fun warn(message: String) {}

        override fun error(message: String) {}
    }
}
