package com.openbitfun.mobile.app.ui.shell.sidebar

/** Path comparisons for remote workspaces, whose wire format is POSIX-like on every client. */
internal object RemoteWorkspacePathPolicy {
    fun equal(left: String, right: String): Boolean = normalize(left) == normalize(right)

    fun normalize(path: String): String {
        var value = path.trim()
        while (value.length > 1 && (value.endsWith('/') || value.endsWith('\\'))) {
            value = value.dropLast(1)
        }
        return value
    }
}
