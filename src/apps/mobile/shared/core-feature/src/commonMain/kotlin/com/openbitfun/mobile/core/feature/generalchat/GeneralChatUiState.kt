package com.openbitfun.mobile.core.feature.generalchat

import com.openbitfun.mobile.core.domain.ChatTimelineState
import com.openbitfun.mobile.core.feature.markdown.MarkdownBlock
import com.openbitfun.mobile.core.feature.session.ComposerImage

public enum class GeneralChatFailureReason {
    UNCONFIGURED,
    AUTHENTICATION,
    RATE_LIMITED,
    SERVICE_UNAVAILABLE,
    INVALID_RESPONSE,
    NETWORK,
}

/**
 * The outcome of "test connection", which is not the outcome of saving.
 *
 * Kept apart from [GeneralChatUiState.configFailure] because the two answer
 * different questions: that one is whether the form is fillable, this one is
 * whether the endpoint on the other end of it answered. A probe that fails
 * leaves the draft exactly as it was, so the user can save anyway.
 */
public data class GeneralChatConnectionTestUi public constructor(
    public val running: Boolean,
    public val passed: Boolean,
    public val failure: GeneralChatFailureReason?,
) {
    public constructor() : this(false, false, null)
}

public data class GeneralChatUiState public constructor(
    public val configured: Boolean,
    public val config: GeneralChatConfigUi,
    public val configFailure: GeneralChatConfigFailure?,
    public val connectionTest: GeneralChatConnectionTestUi,
    /**
     * Every model that could answer, this phone's own first.
     *
     * [config] is not a member of this list: it is the local provider form,
     * which exists whether or not it has been filled in, while a row here is a
     * model that could take the next message.
     */
    public val models: List<GeneralChatModelUi>,
    /** Which of [models] the next message goes to; empty when none can. */
    public val activeModelId: String,
    public val sessionId: String,
    public val sessions: List<GeneralChatSessionUi>,
    public val timeline: ChatTimelineState,
    public val messages: List<GeneralChatMessageUi>,
    public val draft: String,
    public val busy: Boolean,
    public val failure: GeneralChatFailureReason?,
    public val export: GeneralChatExportUi?,
    /** Images selected for the next general-chat turn. */
    public val images: List<ComposerImage>,
)

/**
 * One row of the session list.
 *
 * [title] is empty for a conversation that has not been named and has no first
 * user message to borrow from; the screen supplies its own "untitled" wording.
 */
public data class GeneralChatSessionUi public constructor(
    public val id: String,
    public val title: String,
    public val status: String,
    public val pinned: Boolean,
    public val createdAt: String,
    public val updatedAt: String,
    public val messageCount: Int,
)

/** A finished export, waiting for the screen to hand it to a share sheet. */
public data class GeneralChatExportUi public constructor(
    public val sessionId: String,
    public val title: String,
    public val markdown: String,
)

public data class GeneralChatMessageUi public constructor(
    public val id: String,
    public val role: String,
    public val status: String,
    public val blocks: List<MarkdownBlock>,
)

public sealed interface GeneralChatIntent {
    /**
     * Writes the provider settings.
     *
     * A blank [apiKey] keeps whatever key is already stored, which is what lets
     * the panel show an empty key field without the user having to retype a
     * secret to change the model name. [clearApiKey] is the explicit "forget it".
     */
    public data class SaveConfig public constructor(
        public val baseUrl: String,
        public val model: String,
        public val apiKey: String,
        public val clearApiKey: Boolean,
    ) : GeneralChatIntent {
        override fun toString(): String =
            "SaveConfig(baseUrl=<redacted>, model=$model, apiKey=<redacted>, clearApiKey=$clearApiKey)"
    }

    public data object ClearConfigFailure : GeneralChatIntent

    /**
     * Asks the endpoint in the draft whether it is really there.
     *
     * Takes the draft rather than the saved configuration, because the point of
     * the button is to find out before committing: `SettingsController.test()`
     * validates and probes the same `update` the save path would write, and
     * writes nothing. A blank [apiKey] means the stored one, exactly as in
     * [SaveConfig] — so a user changing only the model name can still test.
     */
    public data class TestConnection public constructor(
        public val baseUrl: String,
        public val model: String,
        public val apiKey: String,
        public val clearApiKey: Boolean,
    ) : GeneralChatIntent {
        override fun toString(): String =
            "TestConnection(baseUrl=<redacted>, model=$model, apiKey=<redacted>, clearApiKey=$clearApiKey)"
    }

    public data object ClearConnectionTest : GeneralChatIntent

    public data class UpdateDraft public constructor(public val text: String) : GeneralChatIntent

    /** Replaces the pending image selection without touching the draft. */
    public data class SetImages public constructor(
        public val images: List<ComposerImage>,
    ) : GeneralChatIntent

    /** Selects a catalog id; credentials remain owned by the config store. */
    public data class SelectModel public constructor(public val modelId: String) : GeneralChatIntent

    public data object NewSession : GeneralChatIntent

    public data class SelectSession public constructor(public val sessionId: String) : GeneralChatIntent

    public data class DeleteSession public constructor(public val sessionId: String) : GeneralChatIntent

    public data class RenameSession public constructor(
        public val sessionId: String,
        public val title: String,
    ) : GeneralChatIntent

    public data class ArchiveSession public constructor(
        public val sessionId: String,
        public val archived: Boolean,
    ) : GeneralChatIntent

    /** Pinning one session unpins whichever session held the pin before. */
    public data class PinSession public constructor(
        public val sessionId: String,
        public val pinned: Boolean,
    ) : GeneralChatIntent

    /** Labels travel with the intent so the core keeps holding no wording. */
    public data class ExportSession public constructor(
        public val sessionId: String,
        public val untitledLabel: String,
        public val userLabel: String,
        public val assistantLabel: String,
    ) : GeneralChatIntent

    public data object ClearExport : GeneralChatIntent

    public data object Send : GeneralChatIntent
    public data object Cancel : GeneralChatIntent
    public data object ClearFailure : GeneralChatIntent
}
