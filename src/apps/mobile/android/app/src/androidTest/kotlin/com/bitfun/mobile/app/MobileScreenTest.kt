package com.bitfun.mobile.app

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
import androidx.test.platform.app.InstrumentationRegistry
import com.bitfun.mobile.app.ui.chat.COMPOSER_INPUT_TEST_TAG
import com.bitfun.mobile.app.ui.chat.COMPOSER_SEND_TEST_TAG
import com.bitfun.mobile.app.ui.chat.CONVERSATION_BACK_TEST_TAG
import com.bitfun.mobile.app.ui.chat.CONVERSATION_LIST_TEST_TAG
import com.bitfun.mobile.app.ui.chat.GENERAL_CHAT_MODEL_TEST_TAG
import com.bitfun.mobile.app.ui.remote.CONNECTION_RETRY_TEST_TAG
import com.bitfun.mobile.app.ui.remote.CONNECT_MANUAL_TEST_TAG
import com.bitfun.mobile.app.ui.remote.FILE_PREVIEW_TEST_TAG
import com.bitfun.mobile.app.ui.remote.SESSION_LIST_TEST_TAG
import com.bitfun.mobile.app.ui.settings.CURRENT_CONTROL_TEST_TAG
import com.bitfun.mobile.app.ui.settings.GENERAL_SETTINGS_CLOSE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.GENERAL_SETTINGS_MODEL_TEST_TAG
import com.bitfun.mobile.app.ui.settings.GENERAL_SETTINGS_PROFILE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.MODEL_SERVICE_ACCOUNT_TEST_TAG
import com.bitfun.mobile.app.ui.settings.MODEL_SERVICE_KEY_TEST_TAG
import com.bitfun.mobile.app.ui.settings.MODEL_SERVICE_LOCAL_TEST_TAG
import com.bitfun.mobile.app.ui.settings.MODEL_SERVICE_MODEL_TEST_TAG
import com.bitfun.mobile.app.ui.settings.MODEL_SERVICE_PROBE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.MODEL_SERVICE_SAVE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.MODEL_SERVICE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.MODEL_SERVICE_URL_TEST_TAG
import com.bitfun.mobile.app.ui.settings.QR_CONNECT_TEST_TAG
import com.bitfun.mobile.app.ui.settings.SETTINGS_CLOSE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.SETTINGS_PROFILE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.VIEW_SETTINGS_TEST_TAG
import com.bitfun.mobile.app.ui.settings.VIEW_SETTINGS_TOGGLE_TEST_TAG
import com.bitfun.mobile.app.ui.shell.MENU_TEST_TAG
import com.bitfun.mobile.app.ui.shell.sidebar.SIDEBAR_CODE_TEST_TAG
import com.bitfun.mobile.app.ui.shell.sidebar.SIDEBAR_SETTINGS_TEST_TAG
import com.bitfun.mobile.app.ui.shell.sidebar.SIDEBAR_TEST_TAG
import com.bitfun.mobile.core.feature.layout.ConversationLayoutPolicy
import java.net.HttpURLConnection
import java.net.URL
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test

/** Covers the Android-to-feature seam without requiring a relay or credentials. */
class MobileScreenTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun theDrawerSwitchesSurfacesAndClosesBehindTheTap() {
        composeRule.onNodeWithTag(SIDEBAR_TEST_TAG).assertIsNotDisplayed()

        composeRule.onNodeWithTag(MENU_TEST_TAG).performClick()
        composeRule.onNodeWithTag(SIDEBAR_TEST_TAG).assertIsDisplayed()
        // The recent list holds general-chat sessions, and a session is only
        // stored once something has been sent to it — so a device that has never
        // chatted shows the empty state rather than a blank row.
        composeRule.onNodeWithText("No conversations yet.").assertIsDisplayed()

        composeRule.onNodeWithTag(SIDEBAR_CODE_TEST_TAG).performClick()

        waitForText("Connect to a desktop")
        composeRule.onNodeWithTag(SIDEBAR_TEST_TAG).assertIsNotDisplayed()
    }

    /**
     * The account has to be reachable, and the drawer has to get out of its way.
     *
     * Worth an instrumentation test because the account was once reachable only
     * from a footer that the signed-in drawer replaces, which left a signed-in
     * user with no way back to it at all. The rule that a sheet closes whatever
     * opened it lives in `AppShellState`; this is what it looks like on a phone.
     *
     * Both footers are walked because a device that has signed in once stays
     * signed in: pinning the test to the signed-out door would mean it only
     * passes on a fresh install, which is the one state the regression it
     * guards against cannot happen in.
     */
    @Test
    fun theDrawerOpensTheAccountAndStepsOutOfTheWay() {
        composeRule.onNodeWithTag(MENU_TEST_TAG).performClick()
        composeRule.onNodeWithTag(SIDEBAR_TEST_TAG).assertIsDisplayed()

        val signedOut = composeRule.onAllNodesWithText("Sign in to BitFun account")
            .fetchSemanticsNodes()
            .isNotEmpty()
        if (signedOut) {
            composeRule.onNodeWithText("Sign in to BitFun account").performClick()
        } else {
            // The signed-in exchange: settings first, and the profile row there
            // is what leads on to the account. The drawer is over the general
            // chat, so the gear lands on the app's own settings page.
            composeRule.onNodeWithTag(SIDEBAR_SETTINGS_TEST_TAG).performClick()
            waitForText("Settings")
            composeRule.onNodeWithTag(GENERAL_SETTINGS_PROFILE_TEST_TAG).performClick()
        }

        waitForText(if (signedOut) "Relay URL" else "Account")
        composeRule.onNodeWithTag(SIDEBAR_TEST_TAG).assertIsNotDisplayed()
    }

    /**
     * The settings page closes by its own button.
     *
     * The sheet has no drag handle — the source draws none, and the page centres
     * its title where one would sit — so this button is the only way out that does
     * not depend on guessing where a downward drag will be read as a dismissal
     * rather than as a scroll. Losing it would strand the page.
     */
    @Test
    fun theSettingsPageClosesByItsOwnButton() {
        composeRule.onNodeWithTag(MENU_TEST_TAG).performClick()
        openSettingsFromAuthenticatedDrawerOrSkip()
        waitForText("Settings")

        composeRule.onNodeWithTag(GENERAL_SETTINGS_CLOSE_TEST_TAG).performClick()

        waitForNoText("About")
    }

    /**
     * One gear, two pages, chosen by what it was pressed over.
     *
     * `AppRootOverlaySurfaces.openSettings()` branches on the conversation behind
     * the sidebar, and the two pages have no section in common: the general one
     * names a model and a build, the remote one names a desktop. A gear that
     * always opened the remote page — which is what this app did until the
     * general page existed — leaves the model settings reachable only from the
     * composer, and tells a user with no desktop about a connection they have
     * never made.
     */
    @Test
    fun theGearOpensThePageThatMatchesTheSurface() {
        composeRule.onNodeWithTag(MENU_TEST_TAG).performClick()
        openSettingsFromAuthenticatedDrawerOrSkip()

        waitForText("Settings")
        composeRule.onNodeWithTag(GENERAL_SETTINGS_MODEL_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(GENERAL_SETTINGS_CLOSE_TEST_TAG).performClick()
        waitForNoText("About")

        composeRule.onNodeWithTag(MENU_TEST_TAG).performClick()
        composeRule.onNodeWithTag(SIDEBAR_CODE_TEST_TAG).performClick()
        waitForText("Connect to a desktop")

        composeRule.onNodeWithTag(MENU_TEST_TAG).performClick()
        openSettingsFromAuthenticatedDrawerOrSkip()

        waitForText("Remote control")
        composeRule.onNodeWithTag(CURRENT_CONTROL_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(QR_CONNECT_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(SETTINGS_PROFILE_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(SETTINGS_CLOSE_TEST_TAG).performClick()
        waitForNoText("Remote control")
    }

    /**
     * The provider editor is the only general-chat path that can be driven
     * without a real model endpoint, and it is the one that touches the
     * keystore — so it is worth having on a device rather than only in the
     * shared store's tests.
     */
    @Test
    fun modelServiceSheetRefusesAnInvalidApiUrl() {
        composeRule.onNodeWithTag(GENERAL_CHAT_MODEL_TEST_TAG).performClick()
        composeRule.onNodeWithTag(MODEL_SERVICE_TEST_TAG).assertIsDisplayed()

        // The panel opens on the overview, so the form is one row in.
        composeRule.onNodeWithTag(MODEL_SERVICE_LOCAL_TEST_TAG).performClick()
        composeRule.onNodeWithTag(MODEL_SERVICE_URL_TEST_TAG).performTextInput("api.example.com")
        composeRule.onNodeWithTag(MODEL_SERVICE_MODEL_TEST_TAG).performTextInput("chat-model")
        composeRule.onNodeWithTag(MODEL_SERVICE_KEY_TEST_TAG).performTextInput("instrumentation-key")
        composeRule.onNodeWithTag(MODEL_SERVICE_SAVE_TEST_TAG).performClick()

        // Refused, and still on the form: the reason has to sit next to the field
        // that caused it, which the overview has none of.
        composeRule.onNodeWithText("The API URL must start with http:// or https://.")
            .assertIsDisplayed()
        composeRule.onNodeWithTag(MODEL_SERVICE_URL_TEST_TAG).assertIsDisplayed()
    }

    /**
     * The account section is a row that is always there, saying what it has.
     *
     * Whether it has anything depends on who is signed in on the device running
     * this, so the assertion is that the section exists at all: "nothing synced"
     * and "no such feature" look identical to a user unless the row is present to
     * tell them apart, which is exactly what this panel used to get wrong by
     * omitting the section entirely.
     */
    @Test
    fun theModelPanelSaysWhatTheAccountHasSynced() {
        composeRule.onNodeWithTag(GENERAL_CHAT_MODEL_TEST_TAG).performClick()

        composeRule.onNodeWithText("Account sync").assertIsDisplayed()
        composeRule.onNodeWithTag(MODEL_SERVICE_ACCOUNT_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithText("Account models").assertIsDisplayed()
    }

    /**
     * A probe with nothing to send is refused by the button, not by the endpoint.
     *
     * Without this the button is live on an empty form, and the first thing a new
     * user learns about their provider is a 401 that says nothing about the fact
     * that this app never sent a credential — the request was always going to fail
     * and the failure describes the wrong thing.
     */
    @Test
    fun theConnectionTestWaitsForSomethingToAuthenticateWith() {
        composeRule.onNodeWithTag(GENERAL_CHAT_MODEL_TEST_TAG).performClick()
        composeRule.onNodeWithTag(MODEL_SERVICE_LOCAL_TEST_TAG).performClick()

        composeRule.onNodeWithTag(MODEL_SERVICE_PROBE_TEST_TAG).assertIsNotEnabled()
        composeRule.onNodeWithText("Keep or enter an API key to test the connection.")
            .assertIsDisplayed()

        composeRule.onNodeWithTag(MODEL_SERVICE_KEY_TEST_TAG).performTextInput("instrumentation-key")

        composeRule.onNodeWithTag(MODEL_SERVICE_PROBE_TEST_TAG).assertIsEnabled()
    }

    @Test
    fun remoteFormReportsEmptyPairingLink() {
        openRemote()
        // Scanning is the front door; the fields live one step behind it.
        composeRule.onNodeWithTag(CONNECT_MANUAL_TEST_TAG).performClick()
        composeRule.onNodeWithText("Connect").performClick()
        composeRule.onNodeWithText("Enter a pairing link first.").assertIsDisplayed()
    }

    @Test
    fun fakeRelaySupportsPairingReadingAndSending() {
        val pairingUrl = InstrumentationRegistry.getArguments().getString("bitfunPairingUrl").orEmpty()
        assumeTrue("bitfunPairingUrl instrumentation argument is required", pairingUrl.isNotBlank())

        openRemote()
        composeRule.onNodeWithTag(CONNECT_MANUAL_TEST_TAG).performClick()
        composeRule.onNodeWithText("Pairing link").performTextInput(pairingUrl)
        composeRule.onNodeWithText("Connect").performClick()

        // The shell's top bar names the connection state once pairing lands.
        waitForText("Connected")
        waitForText("完善鸿蒙端远程控制", substring = true)
        composeRule.onNodeWithText("完善鸿蒙端远程控制", substring = true)
            .performScrollTo()
            .performClick()

        // Opening a session replaces the list with the conversation, whose
        // composer is pinned below the transcript rather than scrolled to.
        waitForText("我来帮你完善鸿蒙端远程控制流程。", substring = true)
        // By tag rather than by placeholder: the placeholder is a sibling of the
        // field now, and typing into a label is not a thing.
        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).performTextInput("Android relay end-to-end")
        composeRule.onNodeWithTag(COMPOSER_SEND_TEST_TAG).performClick()

        waitForText("收到，我会继续处理这条指令。", substring = true)

        composeRule.onNodeWithTag(CONVERSATION_BACK_TEST_TAG).performClick()
        composeRule.onNodeWithTag(SESSION_LIST_TEST_TAG).assertIsDisplayed()
    }

    /**
     * An agent turn is markdown, and it has to arrive as blocks rather than as
     * the punctuation that describes them.
     *
     * The fixture reply carries a fenced `ts` block, so the fence is the cheapest
     * thing to test against: if the parser never ran, the three backticks are on
     * screen and the language is part of a paragraph instead of a card's label.
     */
    @Test
    fun anAgentTurnArrivesAsRenderedMarkdown() {
        val pairingUrl = InstrumentationRegistry.getArguments().getString("bitfunPairingUrl").orEmpty()
        assumeTrue("bitfunPairingUrl instrumentation argument is required", pairingUrl.isNotBlank())

        openRemote()
        composeRule.onNodeWithTag(CONNECT_MANUAL_TEST_TAG).performClick()
        composeRule.onNodeWithText("Pairing link").performTextInput(pairingUrl)
        composeRule.onNodeWithText("Connect").performClick()

        waitForText("完善鸿蒙端远程控制", substring = true)
        composeRule.onNodeWithText("完善鸿蒙端远程控制", substring = true)
            .performScrollTo()
            .performClick()

        waitForText("我来帮你完善鸿蒙端远程控制流程。", substring = true)
        // Through the list, and aimed at the paragraph above the card rather than
        // at the code: the code sits in its own horizontal scroller and does not
        // wrap, so it is both the wrong scrollable to move and a node whose
        // bounds run off the side of everything.
        composeRule.onNodeWithTag(CONVERSATION_LIST_TEST_TAG)
            .performScrollToNode(hasText("我来帮你完善鸿蒙端远程控制流程。", substring = true))

        // The language became the card's label, and next to it is the copy that
        // is the whole point of showing a command on a phone.
        composeRule.onNodeWithText("ts").assertIsDisplayed()
        composeRule.onNodeWithText("Copy").assertIsDisplayed()
        // The code itself is only asserted to exist: it does not wrap, so its own
        // bounds run past the card it scrolls inside and "displayed" is not a
        // question that has an answer for it. The card around it is on screen.
        composeRule.onNodeWithText("await manager.listSessions", substring = true).assertExists()
        // And the fence that described the block did not survive as text.
        composeRule.onAllNodesWithText("```", substring = true).assertCountEquals(0)
    }

    /**
     * The view settings only pay off against a desktop's own sessions: the
     * groupings are about dates and workspaces the fake relay supplies, and none
     * of it can be reached from an unpaired app.
     */
    @Test
    fun viewSettingsRegroupTheSessionListAndAddRowDetail() {
        val pairingUrl = InstrumentationRegistry.getArguments().getString("bitfunPairingUrl").orEmpty()
        assumeTrue("bitfunPairingUrl instrumentation argument is required", pairingUrl.isNotBlank())

        openRemote()
        composeRule.onNodeWithTag(CONNECT_MANUAL_TEST_TAG).performClick()
        composeRule.onNodeWithText("Pairing link").performTextInput(pairingUrl)
        composeRule.onNodeWithText("Connect").performClick()
        waitForText("完善鸿蒙端远程控制", substring = true)

        composeRule.onNodeWithTag(VIEW_SETTINGS_TOGGLE_TEST_TAG).performScrollTo().performClick()
        composeRule.onNodeWithTag(VIEW_SETTINGS_TEST_TAG).assertIsDisplayed()

        // Every fixture session sits in the one open workspace, so the project
        // grouping says nothing; dating them apart is what proves the regroup.
        composeRule.onNodeWithText("Newest first").performScrollTo().performClick()
        waitForText("Yesterday")
        composeRule.onNodeWithText("Today").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Earlier").performScrollTo().assertIsDisplayed()

        // A row is its title until asked otherwise; the oldest fixtures are days
        // old, so the relative time is the part that shows up.
        composeRule.onNodeWithText("Updated").performScrollTo().performClick()
        waitForText("days ago", substring = true)
    }

    /**
     * A preview is only useful if the reader can find the line the agent named,
     * so this follows the gutter all the way from the relay's bytes to the
     * screen. Which kind each run got is the shared lexer's own tests' business —
     * a span colour is not in the semantics tree, so there is nothing here to
     * assert it with.
     */
    @Test
    fun aSourceFilePreviewArrivesNumberedAndTokenised() {
        val pairingUrl = InstrumentationRegistry.getArguments().getString("bitfunPairingUrl").orEmpty()
        assumeTrue("bitfunPairingUrl instrumentation argument is required", pairingUrl.isNotBlank())

        openRemote()
        composeRule.onNodeWithTag(CONNECT_MANUAL_TEST_TAG).performClick()
        composeRule.onNodeWithText("Pairing link").performTextInput(pairingUrl)
        composeRule.onNodeWithText("Connect").performClick()
        // The workspace panel only appears once the desktop has answered with
        // one, which is the same round trip the file field then depends on.
        waitForText("Remote file path")

        composeRule.onNodeWithText("Remote file path").performScrollTo().performTextInput("src/preview.rs")
        composeRule.onNodeWithText("Open file").performScrollTo().performClick()

        waitForText("preview.rs")
        // Wherever it lands the preview is a surface of its own, not a card at
        // the bottom of a long page: no scrolling to reach it.
        composeRule.onNodeWithTag(FILE_PREVIEW_TEST_TAG).assertIsDisplayed()
        // Two lines rather than one: a number in front of the second line is
        // what proves the gutter counts the file rather than labelling its top.
        composeRule.onNodeWithText("1  // bitfun preview fixture\n2  fn main() {", substring = true)
            .assertIsDisplayed()

        // On a narrow window `FilePreviewPlacementPolicy` has nothing to split,
        // so the file is the page — it covers the list, and the card's own
        // button is the only way back. A wide one keeps both and has neither
        // claim to make, which is why this is a branch and not an assumption.
        val widthDp = composeRule.activity.resources.configuration.screenWidthDp
        if (widthDp < ConversationLayoutPolicy.WIDE_LAYOUT_MIN_WIDTH) {
            composeRule.onNodeWithTag(SESSION_LIST_TEST_TAG).assertDoesNotExist()
            composeRule.onNodeWithText("Close").performClick()
            composeRule.onNodeWithTag(SESSION_LIST_TEST_TAG).assertIsDisplayed()
        }
    }

    /**
     * The heartbeat, end to end: the relay stops answering commands while the
     * room itself stays, which is the one situation the unit tests can only
     * simulate — here it is a real 15-second timer against a real socket.
     */
    @Test
    fun aDesktopThatStopsAnsweringIsReportedAndRecovers() {
        val pairingUrl = InstrumentationRegistry.getArguments().getString("bitfunPairingUrl").orEmpty()
        assumeTrue("bitfunPairingUrl instrumentation argument is required", pairingUrl.isNotBlank())

        openRemote()
        composeRule.onNodeWithTag(CONNECT_MANUAL_TEST_TAG).performClick()
        composeRule.onNodeWithText("Pairing link").performTextInput(pairingUrl)
        composeRule.onNodeWithText("Connect").performClick()
        waitForText("Connected")

        try {
            setRelayReachable(pairingUrl, reachable = false)
            // No tap in between: only the store's own timer can notice, and it
            // has fifteen seconds to run plus the ping it then has to fail.
            waitForText("Connection error", timeoutMillis = 60_000)

            setRelayReachable(pairingUrl, reachable = true)
            composeRule.onNodeWithTag(CONNECTION_RETRY_TEST_TAG).performScrollTo().performClick()
            waitForText("Connected", timeoutMillis = 40_000)
        } finally {
            setRelayReachable(pairingUrl, reachable = true)
        }
    }

    /** The remote surface is reached through the drawer now that the tab bar is gone. */
    private fun openRemote() {
        composeRule.onNodeWithTag(MENU_TEST_TAG).performClick()
        composeRule.onNodeWithTag(SIDEBAR_CODE_TEST_TAG).performClick()
        waitForText("Connect to a desktop")
    }

    /**
     * Drives the fake relay's `/control/reachable` switch, on the same origin the
     * pairing link names — the app reaches it through `adb reverse`, and so does
     * this, since the instrumentation runs in the app's own process.
     */
    private fun setRelayReachable(pairingUrl: String, reachable: Boolean) {
        val origin = pairingUrl.substringBefore("/#/")
        val connection = URL("$origin/control/reachable").openConnection() as HttpURLConnection
        try {
            connection.requestMethod = "POST"
            connection.doOutput = true
            connection.setRequestProperty("Content-Type", "application/json")
            connection.outputStream.use { it.write("""{"reachable":$reachable}""".toByteArray()) }
            check(connection.responseCode == 200) { "relay control returned ${connection.responseCode}" }
        } finally {
            connection.disconnect()
        }
    }

    private fun waitForText(text: String, substring: Boolean = false, timeoutMillis: Long = 20_000) {
        composeRule.waitUntil(timeoutMillis = timeoutMillis) {
            composeRule.onAllNodesWithText(text, substring = substring)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
    }

    /** The other half of [waitForText]: a sheet leaves over an animation, not at once. */
    private fun waitForNoText(text: String, timeoutMillis: Long = 20_000) {
        composeRule.waitUntil(timeoutMillis = timeoutMillis) {
            composeRule.onAllNodesWithText(text).fetchSemanticsNodes().isEmpty()
        }
    }

    private fun openSettingsFromAuthenticatedDrawerOrSkip() {
        val settingsNodes = composeRule.onAllNodesWithTag(SIDEBAR_SETTINGS_TEST_TAG)
            .fetchSemanticsNodes()
        assumeTrue(
            "Settings footer is only available when the instrumentation device is signed in",
            settingsNodes.isNotEmpty(),
        )
        composeRule.onNodeWithTag(SIDEBAR_SETTINGS_TEST_TAG).performClick()
    }
}
