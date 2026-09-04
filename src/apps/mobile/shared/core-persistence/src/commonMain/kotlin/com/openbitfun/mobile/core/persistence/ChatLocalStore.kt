package com.openbitfun.mobile.core.persistence

import app.cash.sqldelight.db.SqlDriver
import com.openbitfun.mobile.core.persistence.db.Chat_session
import com.openbitfun.mobile.core.persistence.db.MobileDatabase
import kotlinx.serialization.Serializable

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

@Serializable
public data class PersistedRemoteSession public constructor(
    public val sessionId: String = "",
    public val title: String = "",
    public val agentType: String = "",
    public val status: String = "",
    public val updatedAt: String = "",
    public val createdAt: String = "",
    public val messageCount: Int = 0,
    public val lastMessageId: String = "",
    public val workspacePath: String? = null,
    public val workspaceName: String? = null,
    /** True until a later server list observes this confirmed-created session id. */
    public val pendingConfirmed: Boolean = false,
)

@Serializable
public data class PersistedRemoteMessage public constructor(
    public val messageId: String = "",
    public val sessionId: String = "",
    public val role: String = "",
    public val text: String = "",
    public val status: String = "",
    public val timestamp: String? = null,
    public val thinking: String? = null,
    public val payloadJson: String = "{}",
)

@Serializable
public data class PersistedRemoteCursor public constructor(
    public val pollVersion: String = "",
    public val knownMessageCount: Int = 0,
    public val knownModelCatalogVersion: String = "",
)

public interface RemoteSessionListStore {
    public fun load(deviceKey: String): List<PersistedRemoteSession>
    public fun save(deviceKey: String, sessions: List<PersistedRemoteSession>, hasMore: Boolean = false)
    public fun hasMore(deviceKey: String): Boolean
}

public interface RemoteTranscriptStore {
    public fun load(deviceKey: String, sessionId: String): List<PersistedRemoteMessage>
    public fun append(deviceKey: String, sessionId: String, startSeq: Int, messages: List<PersistedRemoteMessage>)
    public fun replace(deviceKey: String, sessionId: String, messages: List<PersistedRemoteMessage>)
    public fun loadCursor(deviceKey: String, sessionId: String): PersistedRemoteCursor?
    public fun saveCursor(deviceKey: String, sessionId: String, cursor: PersistedRemoteCursor)
}

public class SqlDelightRemoteSessionListStore public constructor(
    driver: SqlDriver,
) : RemoteSessionListStore {
    private val queries = MobileDatabase(driver).mobileQueries
    private var lastSignature = ""

    override fun load(deviceKey: String): List<PersistedRemoteSession> =
        queries.selectRemoteSessions(deviceKey).executeAsList().map { row ->
            PersistedRemoteSession(row.session_id, row.title, row.agent_type, row.status,
                row.updated_at, row.created_at, row.message_count.toInt(), row.last_message_id,
                row.workspace_path, row.workspace_name, row.pending_confirmed == 1L)
        }

    override fun hasMore(deviceKey: String): Boolean =
        queries.selectRemoteSessions(deviceKey).executeAsList().firstOrNull()?.has_more == 1L

    override fun save(deviceKey: String, sessions: List<PersistedRemoteSession>, hasMore: Boolean) {
        if (deviceKey.isBlank()) return
        val kept = sessions.take(20)
        val signature = "$deviceKey|${hasMore}|${kept.joinToString { it.sessionId + ":" + it.updatedAt + ":" + it.messageCount + ":" + it.pendingConfirmed }}"
        if (signature == lastSignature) return
        queries.transaction {
            queries.deleteRemoteSessionsForDevice(deviceKey)
            kept.forEach { session -> queries.upsertRemoteSession(
                deviceKey, session.sessionId, session.title, session.agentType, session.status,
                session.updatedAt, session.createdAt, session.messageCount.toLong(), session.lastMessageId,
                session.workspacePath, session.workspaceName, if (hasMore) 1L else 0L,
                if (session.pendingConfirmed) 1L else 0L)
            }
        }
        lastSignature = signature
    }
}

public typealias RemoteSessionListRdbStore = SqlDelightRemoteSessionListStore

public typealias RemoteChatLocalRdbStore = SqlDelightRemoteTranscriptStore

public class SqlDelightRemoteTranscriptStore public constructor(
    driver: SqlDriver,
) : RemoteTranscriptStore {
    private val queries = MobileDatabase(driver).mobileQueries
    private val resident = LinkedHashMap<String, List<PersistedRemoteMessage>>()

    override fun load(deviceKey: String, sessionId: String): List<PersistedRemoteMessage> {
        val key = "$deviceKey::$sessionId"
        resident[key]?.let { return it }
        val result = queries.selectRemoteMessages(deviceKey, sessionId).executeAsList().map { row ->
            PersistedRemoteMessage(
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
        remember(key, result)
        return result
    }

    override fun append(deviceKey: String, sessionId: String, startSeq: Int, messages: List<PersistedRemoteMessage>) {
        if (messages.isEmpty()) return
        queries.transaction {
            // Replacing the range makes retries idempotent and safely repairs a partial append.
            queries.deleteRemoteMessagesFrom(deviceKey, sessionId, startSeq.toLong())
            messages.forEachIndexed { index, message -> saveRow(deviceKey, sessionId, startSeq + index, message) }
        }
        resident.remove("$deviceKey::$sessionId")
    }

    override fun replace(deviceKey: String, sessionId: String, messages: List<PersistedRemoteMessage>) {
        queries.transaction {
            queries.deleteRemoteMessages(deviceKey, sessionId)
            messages.forEachIndexed { index, message -> saveRow(deviceKey, sessionId, index, message) }
        }
        remember("$deviceKey::$sessionId", messages)
    }

    override fun loadCursor(deviceKey: String, sessionId: String): PersistedRemoteCursor? =
        queries.selectRemoteCursor(deviceKey, sessionId).executeAsOneOrNull()?.let {
            PersistedRemoteCursor(it.poll_version, it.known_message_count.toInt(), it.known_model_catalog_version)
        }

    override fun saveCursor(deviceKey: String, sessionId: String, cursor: PersistedRemoteCursor) {
        queries.upsertRemoteCursor(deviceKey, sessionId, cursor.pollVersion,
            cursor.knownMessageCount.toLong(), cursor.knownModelCatalogVersion)
    }

    private fun saveRow(deviceKey: String, sessionId: String, seq: Int, message: PersistedRemoteMessage) {
        queries.upsertRemoteMessage(deviceKey, sessionId, seq.toLong(), message.messageId, message.role,
            message.text, message.status, message.timestamp, message.thinking, message.payloadJson)
    }

    private fun remember(key: String, messages: List<PersistedRemoteMessage>) {
        resident[key] = messages
        while (resident.size > 3) resident.remove(resident.entries.first().key)
    }
}

private object EmptyRemoteSessionListStore : RemoteSessionListStore {
    override fun load(deviceKey: String): List<PersistedRemoteSession> = emptyList()
    override fun save(deviceKey: String, sessions: List<PersistedRemoteSession>, hasMore: Boolean) = Unit
    override fun hasMore(deviceKey: String): Boolean = false
}

private object EmptyRemoteTranscriptStore : RemoteTranscriptStore {
    override fun load(deviceKey: String, sessionId: String): List<PersistedRemoteMessage> = emptyList()
    override fun append(deviceKey: String, sessionId: String, startSeq: Int, messages: List<PersistedRemoteMessage>) = Unit
    override fun replace(deviceKey: String, sessionId: String, messages: List<PersistedRemoteMessage>) = Unit
    override fun loadCursor(deviceKey: String, sessionId: String): PersistedRemoteCursor? = null
    override fun saveCursor(deviceKey: String, sessionId: String, cursor: PersistedRemoteCursor) = Unit
}

public data class MobilePersistenceStores public constructor(
    public val drafts: DraftStore,
    public val chats: ChatLocalStore,
    public val remoteSessions: RemoteSessionListStore = EmptyRemoteSessionListStore,
    public val remoteTranscripts: RemoteTranscriptStore = EmptyRemoteTranscriptStore,
)

public fun mobilePersistenceStores(driver: SqlDriver): MobilePersistenceStores = MobilePersistenceStores(
    drafts = SqlDelightDraftStore(driver), chats = SqlDelightChatLocalStore(driver),
    remoteSessions = SqlDelightRemoteSessionListStore(driver),
    remoteTranscripts = SqlDelightRemoteTranscriptStore(driver),
)
