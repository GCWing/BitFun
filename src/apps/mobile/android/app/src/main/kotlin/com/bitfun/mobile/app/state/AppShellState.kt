package com.bitfun.mobile.app.state

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue

/**
 * What the content area shows.
 *
 * Only two, because the shell moved settings and the account onto sheets the
 * way `AppShell.ets` binds them — a sheet is not a destination, so neither can
 * displace the conversation the user was reading.
 */
internal enum class MobileSurface {
    GENERAL_CHAT,
    REMOTE,
}

/**
 * Which settings page the one settings sheet is showing.
 *
 * `AppShellState.ets` keeps the same field because root settings, remote-control
 * settings, and account details share one overlay host. The sidebar gear always
 * selects [GENERAL]; remote-specific entry points select [REMOTE].
 */
internal enum class SettingsMode {
    GENERAL,
    REMOTE,
}

/**
 * The shell's own navigation and overlay state, ported from
 * `pages/state/AppShellState.ets`.
 *
 * The transitions live here rather than in the composable for the reason the
 * source keeps them in a class: "opening the account closes settings" is a rule
 * about the shell, and a rule spread across the call sites that trigger it is a
 * rule each new call site can get wrong. The view reads the properties and calls
 * the verbs; nothing outside sets a field.
 */
@Stable
internal class AppShellState(
    surface: MobileSurface,
    showSettings: Boolean,
    settingsMode: SettingsMode,
    showAccount: Boolean,
    accountReturnsToSettings: Boolean,
    searchOpen: Boolean,
    sidebarQuery: String,
    remoteSessionId: String? = null,
    remoteCreating: Boolean = false,
    remoteScanRequested: Boolean = false,
) {
    internal var surface: MobileSurface by mutableStateOf(surface)
        private set

    internal var showSettings: Boolean by mutableStateOf(showSettings)
        private set

    internal var settingsMode: SettingsMode by mutableStateOf(settingsMode)
        private set

    /** Whether closing the account lands back on the page that opened it. */
    private var accountReturnsToSettings: Boolean by mutableStateOf(accountReturnsToSettings)

    internal var showAccount: Boolean by mutableStateOf(showAccount)
        private set

    internal var searchOpen: Boolean by mutableStateOf(searchOpen)
        private set

    internal var sidebarQuery: String by mutableStateOf(sidebarQuery)
        private set

    internal var remoteSessionId: String? by mutableStateOf(remoteSessionId)
        private set

    internal var remoteCreating: Boolean by mutableStateOf(remoteCreating)
        private set

    internal var remoteScanRequested: Boolean by mutableStateOf(remoteScanRequested)
        private set

    internal fun show(next: MobileSurface) {
        surface = next
    }

    internal fun openRemoteSession(sessionId: String) {
        surface = MobileSurface.REMOTE
        remoteCreating = false
        remoteSessionId = sessionId
    }

    internal fun createRemoteSession() {
        surface = MobileSurface.REMOTE
        remoteScanRequested = false
        remoteCreating = true
        remoteSessionId = null
    }

    internal fun closeRemoteSession() {
        remoteCreating = false
        remoteSessionId = null
    }

    /**
     * Opens the remote surface's connect page without launching the scanner.
     *
     * The sidebar's "Connect a computer" row is a door to the choose-connection
     * page, not a camera trigger: scanning stays a named action the user taps.
     * [openRemoteScanner] remains for entry points whose whole job is to scan.
     */
    internal fun openRemoteConnect() {
        surface = MobileSurface.REMOTE
        remoteCreating = false
        remoteSessionId = null
        remoteScanRequested = false
    }

    internal fun openRemoteScanner() {
        surface = MobileSurface.REMOTE
        remoteCreating = false
        remoteSessionId = null
        remoteScanRequested = true
    }

    internal fun closeRemoteScanner() {
        remoteScanRequested = false
    }

    /** Opens the requested settings surface in the shared overlay host. */
    internal fun openSettings(mode: SettingsMode) {
        settingsMode = mode
        showSettings = true
    }

    internal fun dismissSettings() {
        showSettings = false
    }

    /**
     * One sheet at a time: the account replaces settings rather than stacking on
     * it. It is remembered as the page to come back to, though — the source's
     * `accountReturnMode` — because the account is reached through a row on a
     * settings page and closing it should put that page back rather than drop the
     * user onto the conversation two steps below.
     */
    internal fun openAccount() {
        accountReturnsToSettings = showSettings
        showSettings = false
        showAccount = true
    }

    internal fun dismissAccount() {
        showAccount = false
        if (accountReturnsToSettings) {
            accountReturnsToSettings = false
            showSettings = true
        }
    }

    internal fun search(query: String) {
        sidebarQuery = query
    }

    /** Closing the field clears it, so reopening it never resumes an old search. */
    internal fun toggleSearch() {
        searchOpen = !searchOpen
        if (!searchOpen) sidebarQuery = ""
    }

    internal companion object {
        // Enums are not saveable, so the surface crosses as its name — a stable
        // identifier, unlike an ordinal, if a case is ever inserted.
        val Saver: Saver<AppShellState, Any> = listSaver(
            save = {
                listOf(
                    it.surface.name,
                    it.showSettings,
                    it.settingsMode.name,
                    it.showAccount,
                    it.accountReturnsToSettings,
                    it.searchOpen,
                    it.sidebarQuery,
                    it.remoteSessionId,
                    it.remoteCreating,
                    it.remoteScanRequested,
                )
            },
            restore = {
                AppShellState(
                    surface = MobileSurface.valueOf(it[0] as String),
                    showSettings = it[1] as Boolean,
                    settingsMode = SettingsMode.valueOf(it[2] as String),
                    showAccount = it[3] as Boolean,
                    accountReturnsToSettings = it[4] as Boolean,
                    searchOpen = it[5] as Boolean,
                    sidebarQuery = it[6] as String,
                    remoteSessionId = it.getOrNull(7) as String?,
                    remoteCreating = it.getOrNull(8) as? Boolean ?: false,
                    remoteScanRequested = it.getOrNull(9) as? Boolean ?: false,
                )
            },
        )
    }
}

@Composable
internal fun rememberAppShellState(): AppShellState = rememberSaveable(saver = AppShellState.Saver) {
    AppShellState(
        surface = MobileSurface.GENERAL_CHAT,
        showSettings = false,
        settingsMode = SettingsMode.GENERAL,
        showAccount = false,
        accountReturnsToSettings = false,
        searchOpen = false,
        sidebarQuery = "",
        remoteSessionId = null,
        remoteCreating = false,
        remoteScanRequested = false,
    )
}
