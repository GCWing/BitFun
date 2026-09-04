package com.openbitfun.mobile.core.domain

/**
 * A conversation as Markdown, ported from `GeneralChatExportFormatter.ets`.
 *
 * The role labels are arguments rather than constants: the exported file is read
 * by a person, so "user" has to be said in that person's language, and the core
 * holds no wording. Empty turns are dropped — a heading over nothing is noise in
 * a document someone is about to share.
 */
public object GeneralChatExportFormatter {
    public fun markdown(
        title: String,
        messages: List<ChatMessage>,
        userLabel: String,
        assistantLabel: String,
    ): String {
        val sections = mutableListOf("# " + title.trim())
        messages.forEach { message ->
            val content = message.text.trim()
            if (content.isEmpty()) return@forEach
            val label = if (message.role == "user") userLabel else assistantLabel
            sections += "## " + label + "\n\n" + content
        }
        return sections.joinToString("\n\n") + "\n"
    }
}
