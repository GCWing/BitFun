package com.bitfun.mobile.app.ui.shell

import com.bitfun.mobile.app.R
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.connection.ConnectionStatusLabel
import com.bitfun.mobile.core.feature.connection.ConnectionStatusPresenter

/** Shared by the drawer row and the top bar so both name the state the same way. */
internal fun ConnectionPhase.labelRes(): Int = when (ConnectionStatusPresenter.label(this)) {
    ConnectionStatusLabel.CONNECTED -> R.string.connection_connected
    ConnectionStatusLabel.RECONNECTING -> R.string.connection_reconnecting
    ConnectionStatusLabel.CONNECTING -> R.string.connection_connecting
    ConnectionStatusLabel.ERROR -> R.string.connection_error
    ConnectionStatusLabel.DISCONNECTED -> R.string.connection_disconnected
    ConnectionStatusLabel.WAITING -> R.string.connection_waiting
}
