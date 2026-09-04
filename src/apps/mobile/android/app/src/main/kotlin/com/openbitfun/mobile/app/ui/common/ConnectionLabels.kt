package com.openbitfun.mobile.app.ui.common

import androidx.annotation.StringRes
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.feature.connection.ConnectionStatusLabel
import com.openbitfun.mobile.core.feature.connection.ConnectionStatusPresenter

/** Android resource adapter for the platform-neutral connection presentation. */
internal fun ConnectionPhase.labelRes(): Int = when (ConnectionStatusPresenter.label(this)) {
    ConnectionStatusLabel.CONNECTED -> R.string.connection_connected
    ConnectionStatusLabel.RECONNECTING -> R.string.connection_reconnecting
    ConnectionStatusLabel.CONNECTING -> R.string.connection_connecting
    ConnectionStatusLabel.ERROR -> R.string.connection_error
    ConnectionStatusLabel.DISCONNECTED -> R.string.connection_disconnected
    ConnectionStatusLabel.WAITING -> R.string.connection_waiting
}

/** Resource keys for HarmonyOS' conversation status-strip title and detail. */
internal data class ChatStatusBarCopy(
    @StringRes val title: Int,
    @StringRes val detail: Int,
)

internal fun ConnectionPhase.chatStatusBarCopy(canStop: Boolean): ChatStatusBarCopy {
    val title = when {
        canStop -> R.string.chat_status_executing
        this == ConnectionPhase.FAILED -> R.string.connection_error
        this == ConnectionPhase.RECONNECTING -> R.string.chat_status_restoring_connection
        else -> R.string.chat_status_messages_synced
    }
    val detail = when {
        canStop -> title
        this == ConnectionPhase.RECONNECTING -> R.string.connection_reconnecting_desktop
        this == ConnectionPhase.FAILED -> R.string.connection_unavailable_reconnect
        this == ConnectionPhase.DISCONNECTED -> R.string.connection_desktop_disconnected
        else -> labelRes()
    }
    return ChatStatusBarCopy(title, detail)
}
