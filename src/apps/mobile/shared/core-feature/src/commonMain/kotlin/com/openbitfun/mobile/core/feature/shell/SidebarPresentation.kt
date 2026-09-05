package com.openbitfun.mobile.core.feature.shell

/**
 * One conversation as the sidebar needs it.
 *
 * Everything here is either something a row draws or something the details sheet
 * reads back; the transcript itself never reaches the drawer.
 */
public data class SidebarSessionRow public constructor(
    public val id: String,
    public val title: String,
    public val status: String,
    public val pinned: Boolean,
    public val createdAt: String,
    public val updatedAt: String,
    public val messageCount: Int,
)

/**
 * The three lists a sidebar draws, plus the count its archive row shows.
 *
 * [archivedCount] is deliberately not `archived.size`: the disclosure row says
 * how many archived conversations exist, and a search that matches none of them
 * must not make the archive look empty.
 */
public data class SidebarSections public constructor(
    public val pinned: List<SidebarSessionRow>,
    public val recent: List<SidebarSessionRow>,
    public val archived: List<SidebarSessionRow>,
    public val archivedCount: Int,
)

/**
 * How the sidebar splits its conversations, ported from the four filter helpers
 * in `pages/components/AppSidebar.ets`.
 *
 * A pinned conversation leaves the recent list rather than appearing twice, and
 * archiving wins over pinning — an archived conversation is filed away, and a
 * pin above the recent list is the opposite of filed away.
 */
public object SidebarPresentation {
    public fun sections(sessions: List<SidebarSessionRow>, query: String): SidebarSections {
        val needle = query.trim().lowercase()
        val matching = if (needle.isEmpty()) {
            sessions
        } else {
            sessions.filter { it.title.lowercase().contains(needle) }
        }
        return SidebarSections(
            pinned = matching.filter { it.pinned && !isArchived(it) },
            recent = matching.filter { !it.pinned && !isArchived(it) },
            archived = matching.filter(::isArchived),
            archivedCount = sessions.count(::isArchived),
        )
    }

    private fun isArchived(session: SidebarSessionRow): Boolean =
        session.status.equals(ARCHIVED, ignoreCase = true)

    private const val ARCHIVED = "archived"
}
