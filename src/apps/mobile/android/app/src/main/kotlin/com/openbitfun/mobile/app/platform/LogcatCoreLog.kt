package com.openbitfun.mobile.app.platform

import android.util.Log
import com.openbitfun.mobile.core.feature.CoreLog

/**
 * Forwards core log lines to Logcat verbatim.
 *
 * Verbatim is safe by construction: room ids and request ids are truncated
 * before they leave the core, and keys, tokens and passwords never reach it.
 * If that ever stops being true, the tests next to `PairingStore` fail first.
 */
internal object LogcatCoreLog : CoreLog {
    private const val TAG = "OpenBitFunCore"

    override fun info(message: String) {
        Log.i(TAG, message)
    }

    override fun warn(message: String) {
        Log.w(TAG, message)
    }

    override fun error(message: String) {
        Log.e(TAG, message)
    }
}
