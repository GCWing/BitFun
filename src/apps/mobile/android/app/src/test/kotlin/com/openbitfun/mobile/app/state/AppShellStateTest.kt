package com.openbitfun.mobile.app.state

import androidx.compose.runtime.saveable.SaverScope
import org.junit.Assert.*
import org.junit.Test

class AppShellStateTest {
    private fun state() = AppShellState(
        MobileSurface.REMOTE, false, SettingsMode.GENERAL, false, false, false, "",
    )

    @Test fun disconnectedHomeIsSeparateFromConnectionChooser() {
        val shell = state()
        assertFalse(shell.remoteConnectOpen)
        shell.openRemoteConnect()
        assertTrue(shell.remoteConnectOpen)
        assertFalse(shell.remoteScanRequested)
        shell.openRemoteScanner()
        shell.closeRemoteScanner()
        assertTrue(shell.remoteConnectOpen)
        assertFalse(shell.remoteScanRequested)
        shell.closeRemoteConnect()
        assertFalse(shell.remoteConnectOpen)
    }

    @Test fun selectingADeviceOrSessionClosesConnectionFlow() {
        val shell = state()
        shell.openRemoteScanner()
        shell.closeRemoteSession()
        assertFalse(shell.remoteScanRequested)
        assertFalse(shell.remoteConnectOpen)
        shell.openRemoteConnect()
        shell.openRemoteSession("session")
        assertEquals("session", shell.remoteSessionId)
        assertFalse(shell.remoteConnectOpen)
    }

    @Test fun restoreAcceptsOldSavedStateAndRetainsNewConnectionRoute() {
        val legacy = listOf("REMOTE", false, "GENERAL", false, false, false, "", null, false, false)
        assertFalse(AppShellState.Saver.restore(legacy)!!.remoteConnectOpen)
        val legacyScanner = legacy.dropLast(1) + true
        assertTrue(AppShellState.Saver.restore(legacyScanner)!!.remoteConnectOpen)
        val shell = state().apply { openRemoteScanner() }
        val scope = object : SaverScope {
            override fun canBeSaved(value: Any): Boolean = true
        }
        val saved = with(AppShellState.Saver) { scope.save(shell) }!!
        val restored = AppShellState.Saver.restore(saved)!!
        assertTrue(restored.remoteConnectOpen)
        assertTrue(restored.remoteScanRequested)
    }
}
