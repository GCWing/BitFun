package com.bitfun.mobile.core.persistence

import app.cash.sqldelight.db.SqlDriver
import com.bitfun.mobile.core.persistence.db.Chat_session
import com.bitfun.mobile.core.persistence.db.MobileDatabase

public data class PersistedChatSession public constructor(
    public val sessionId: String,
    public val title: String,
    public val agentType: String,
    public val status: String,
    public val updatedAt: String,
    public val createdAt: String,
    public val messageCount: Int,
    /** At most one session of a kind is pinned; see [ChatLocalStore.pinSession]. */
    public val pinned: Boolean,
)

public data class PersistedChatMessage public constructor(
    public val messageId: String,
    public val sessionId: String,
    public val role: String,
    public val text: String,
    public val status: String,
    public val timestamp: String?,
    public val thinking: String?,
    public val payloadJson: String,
)

public interface ChatLocalStore {
    /** Sessions of one kind, newest first. */
    public fun listSessions(agentType: String): List<PersistedChatSession>

    public fun loadSession(sessionId: String): PersistedChatSession?

    public fun loadMessages(sessionId: String): List<PersistedChatMessage>

    public fun saveSession(session: PersistedChatSession)

    public fun saveMessage(message: PersistedChatMessage)

    /**
     * Moves the pin, which at most one session of [agentType] may hold.
     *
     * Exclusive rather than a per-row flag because the sidebar shows a single
     * pinned slot above the recent list; two pinned rows would have no order
     * between them and no room to show both.
     */
    public fun pinSession(agentType: String, sessionId: String, pinned: Boolean)

    /** Writes only the status, so archiving cannot race a transcript being saved. */
    public fun setSessionStatus(sessionId: String, status: String)

    /** Removes the session row and every message that belonged to it. */
    public fun deleteSession(sessionId: String)
}

public class SqlDelightChatLocalStore public constructor(
    driver: SqlDriver,
) : ChatLocalStore {
    private val queries = MobileDatabase(driver).mobileQueries

    override fun listSessions(agentType: String): List<PersistedChatSession> =
        queries.selectSessionsByAgent(agentType).executeAsList().map(::session)

    override fun loadSession(sessionId: String): PersistedChatSession? =
        queries.selectSession(sessionId).executeAsOneOrNull()?.let(::session)

    override fun loadMessages(sessionId: String): List<PersistedChatMessage> =
        queries.selectMessages(sessionId).executeAsList().map { row ->
            PersistedChatMessage(
                messageId = row.message_id,
                sessionId = row.session_id,
                role = row.role,
                text = row.text,
                status = row.status,
                timestamp = row.timestamp,
                thinking = row.thinking,
                payloadJson = row.payload_json,
            )
        }

    override fun saveSession(session: PersistedChatSession) {
        queries.upsertSession(
            session.sessionId,
            session.title,
            session.agentType,
            session.status,
            session.updatedAt,
            session.createdAt,
            session.messageCount.toLong(),
            null,
            null,
            if (session.pinned) 1L else 0L,
        )
    }

    override fun pinSession(agentType: String, sessionId: String, pinned: Boolean) {
        queries.transaction {
            queries.clearPinnedSessions(agentType)
            if (pinned) queries.setPinnedSession(sessionId)
        }
    }

    override fun setSessionStatus(sessionId: String, status: String) {
        queries.setSessionStatus(status, sessionId)
    }

    override fun saveMessage(message: PersistedChatMessage) {
        queries.upsertMessage(
            message.messageId,
            message.sessionId,
            message.role,
            message.text,
            message.status,
            message.timestamp,
            message.thinking,
            message.payloadJson,
        )
    }

    override fun deleteSession(sessionId: String) {
        queries.transaction {
            queries.deleteMessagesForSession(sessionId)
            queries.deleteSession(sessionId)
        }
    }

    private fun session(row: Chat_session): PersistedChatSession = PersistedChatSession(
        sessionId = row.session_id,
        title = row.title,
        agentType = row.agent_type,
        status = row.status,
        updatedAt = row.updated_at,
        createdAt = row.created_at,
        messageCount = row.message_count.toInt(),
        pinned = row.pinned != 0L,
    )
}

public data class MobilePersistenceStores public constructor(
    public val drafts: DraftStore,
    public val chats: ChatLocalStore,
)

public fun mobilePersistenceStores(driver: SqlDriver): MobilePersistenceStores = MobilePersistenceStores(
    drafts = SqlDelightDraftStore(driver),
    chats = SqlDelightChatLocalStore(driver),
)
