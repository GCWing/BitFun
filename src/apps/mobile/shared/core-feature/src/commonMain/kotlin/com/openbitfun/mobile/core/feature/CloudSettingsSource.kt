package com.openbitfun.mobile.core.feature

/**
 * A way to ask the signed-in account for its settings document.
 *
 * The account owns the session that can open the blob; General Chat owns the
 * one thing inside it this app cares about, the model list. Neither store may
 * reach into the other, so the account hands out one of these and the chat
 * store takes one — which also means a chat store with no source behaves
 * exactly like a signed-out one, without either side having to model sign-in.
 *
 * The returned text is the desktop's whole configuration document and carries
 * the user's provider API keys. It must not be logged, persisted, or put into
 * any state a screen observes.
 *
 * @return the decrypted document, or null when the account has never synced
 * one. Throws when the answer could not be obtained at all, which is a
 * different thing from there being nothing to obtain.
 */
public fun interface CloudSettingsSource {
    public suspend fun plaintext(): String?
}
