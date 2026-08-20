package com.bitfun.mobile.core.feature.session

/**
 * What one surface's composer is allowed to offer.
 *
 * Ported from `ChatComposerCapabilities.ets`. The composer is a single widget
 * used by two conversations with different rules — general chat has no desktop
 * to reach, while both current chat surfaces can carry image attachments — so
 * the differences are data rather than a second copy of the widget.
 */
public data class ChatComposerCapabilities public constructor(
    public val supportsAttachments: Boolean,
    public val requiresRemoteConnection: Boolean,
    public val showAddButton: Boolean,
    public val showVoiceInput: Boolean,
) {
    public companion object {
        /**
         * No connection gate: the provider call goes straight out over the
         * network the phone already has.
         */
        public val GeneralChat: ChatComposerCapabilities = ChatComposerCapabilities(
            supportsAttachments = true,
            requiresRemoteConnection = false,
            showAddButton = true,
            showVoiceInput = true,
        )

        public val RemoteChat: ChatComposerCapabilities = ChatComposerCapabilities(
            supportsAttachments = true,
            requiresRemoteConnection = true,
            showAddButton = true,
            showVoiceInput = true,
        )

        /**
         * The new-session screen: the same bar, minus attachments.
         *
         * There is no session yet to attach a photo to — the first message is
         * sent by the create call itself — so offering the picker would produce
         * an image with nowhere to go.
         */
        public val RemoteCreate: ChatComposerCapabilities = ChatComposerCapabilities(
            supportsAttachments = false,
            requiresRemoteConnection = true,
            showAddButton = false,
            showVoiceInput = true,
        )
    }
}
