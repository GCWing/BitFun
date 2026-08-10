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
 * `AppShellState.ets` keeps the same field for the same reason: the sidebar's
 * gear opens two different pages depending on what the conversation behind it
 * is, so "settings is open" is not enough to know what to draw.
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

    internal fun show(next: MobileSurface) {
        surface = next
    }

    /**
     * Which page depends on the conversation the gear was pressed over, as
     * `AppRootOverlaySurfaces.openSettings()` decides it: a remote conversation
     * asks about the desktop it is driving, and anything else asks about the app.
     */
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
    )
}
