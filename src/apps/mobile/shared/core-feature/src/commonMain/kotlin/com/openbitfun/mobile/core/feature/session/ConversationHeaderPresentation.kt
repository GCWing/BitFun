package com.openbitfun.mobile.core.feature.session

/**
 * The second line of a conversation header, ported from `headerContextTitle` in
 * `pages/components/RemoteChatHeader.ets`.
 *
 * It answers "where am I typing", which on a phone is not obvious: the same
 * session list can be reached through a paired desktop or through an account
 * device, and the answer is a device name in one case and a branch in the other.
 */
public object ConversationHeaderPresenter {
    /**
     * Not a localized string: it is the product's name, the same in every
     * locale, so it belongs here rather than in each app's string table.
     */
    private const val BRAND = "OpenBitFun"

    /**
     * @param desktopName the machine being driven, when one is named. It wins
     * over the branch because a user with two desktops needs to know which one
     * is answering before anything else about the workspace matters.
     */
    public fun contextTitle(desktopName: String, workspaceBranch: String): String {
        val desktop = desktopName.trim()
        if (desktop.isNotEmpty()) return desktop
        val branch = workspaceBranch.trim()
        return if (branch.isNotEmpty()) "$BRAND · $branch" else BRAND
    }
}
