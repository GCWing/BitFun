package com.bitfun.mobile.core.persistence

import app.cash.sqldelight.db.QueryResult
import app.cash.sqldelight.driver.jdbc.sqlite.JdbcSqliteDriver
import com.bitfun.mobile.core.persistence.db.MobileDatabase
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

class RemotePersistenceStoreTest {
    private suspend fun stores(): Pair<RemoteSessionListStore, RemoteTranscriptStore> {
        val driver = JdbcSqliteDriver(JdbcSqliteDriver.IN_MEMORY)
        MobileDatabase.Schema.create(driver).await()
        return SqlDelightRemoteSessionListStore(driver) to SqlDelightRemoteTranscriptStore(driver)
    }

    @Test
    fun roundTripsSessionListAndTranscriptInSequenceOrder() = runTest {
        val (sessions, transcript) = stores()
        sessions.save("device-a", listOf(session("s1", "2026-01-01")), hasMore = true)
        assertEquals("s1", sessions.load("device-a").single().sessionId)
        assertTrue(sessions.hasMore("device-a"))
        transcript.append("device-a", "s1", 0, listOf(message("m0", "zero"), message("m1", "one")))
        assertEquals(listOf("zero", "one"), transcript.load("device-a", "s1").map { it.text })
    }

    @Test
    fun pendingConfirmedMarkerRoundTripsAndLegacySerializationRemainsCompatible() = runTest {
        val (sessions, _) = stores()
        sessions.save(
            "device-a",
            listOf(session("pending", "2026-01-01").copy(pendingConfirmed = true)),
        )
        assertTrue(sessions.load("device-a").single().pendingConfirmed)

        val legacyPayload = """{"sessionId":"legacy","title":"Legacy"}"""
        val decoded = Json.decodeFromString<PersistedRemoteSession>(legacyPayload)
        assertEquals(false, decoded.pendingConfirmed)
        val currentPayload = Json.encodeToString(decoded)
        val redecoded = Json.decodeFromString<PersistedRemoteSession>(currentPayload)
        assertEquals(decoded, redecoded)
        assertEquals(false, redecoded.pendingConfirmed)
    }

    @Test
    fun migratesV3RemoteSessionRowToV4AndCurrentStoreCanLoadAndSaveIt() = runTest {
        val driver = JdbcSqliteDriver(JdbcSqliteDriver.IN_MEMORY)
        driver.execute(
            identifier = null,
            sql = """
                CREATE TABLE remote_session_list (
                    device_key TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    agent_type TEXT NOT NULL,
                    status TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    message_count INTEGER NOT NULL,
                    last_message_id TEXT NOT NULL,
                    workspace_path TEXT,
                    workspace_name TEXT,
                    has_more INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (device_key, session_id)
                )
            """.trimIndent(),
            parameters = 0,
        ).await()
        driver.execute(
            identifier = null,
            sql = """
                INSERT INTO remote_session_list(
                    device_key, session_id, title, agent_type, status, updated_at, created_at,
                    message_count, last_message_id, workspace_path, workspace_name, has_more
                ) VALUES (
                    'device-v3', 'session-v3', 'Legacy title', 'remote', 'ready',
                    '2026-02-03', '2026-01-02', 7, 'message-7', '/legacy/workspace', 'Legacy workspace', 1
                )
            """.trimIndent(),
            parameters = 0,
        ).await()

        MobileDatabase.Schema.migrate(driver, 3, 4).await()
        val migratedPendingValue = driver.executeQuery(
            identifier = null,
            sql = "SELECT pending_confirmed FROM remote_session_list WHERE session_id = 'session-v3'",
            mapper = { cursor ->
                check(cursor.next().value)
                QueryResult.Value(cursor.getLong(0))
            },
            parameters = 0,
        ).await()
        assertEquals(0L, migratedPendingValue)

        val sessions = SqlDelightRemoteSessionListStore(driver)
        val migrated = sessions.load("device-v3").single()
        assertEquals("session-v3", migrated.sessionId)
        assertEquals("Legacy title", migrated.title)
        assertEquals("remote", migrated.agentType)
        assertEquals("ready", migrated.status)
        assertEquals("2026-02-03", migrated.updatedAt)
        assertEquals("2026-01-02", migrated.createdAt)
        assertEquals(7, migrated.messageCount)
        assertEquals("message-7", migrated.lastMessageId)
        assertEquals("/legacy/workspace", migrated.workspacePath)
        assertEquals("Legacy workspace", migrated.workspaceName)
        assertEquals(false, migrated.pendingConfirmed)
        assertTrue(sessions.hasMore("device-v3"))

        sessions.save(
            "device-v3",
            listOf(migrated.copy(title = "Current title", pendingConfirmed = true)),
        )
        assertEquals(
            migrated.copy(title = "Current title", pendingConfirmed = true),
            sessions.load("device-v3").single(),
        )
    }

    @Test
    fun emptyServerListClearsCachedSessionsOnColdStart() = runTest {
        val (sessions, _) = stores()
        sessions.save("device-a", listOf(session("s1", "2026-01-01")), hasMore = true)
        assertTrue(sessions.load("device-a").isNotEmpty())
        sessions.save("device-a", emptyList())
        assertTrue(sessions.load("device-a").isEmpty())
        assertEquals(false, sessions.hasMore("device-a"))
    }

    @Test
    fun appendIsIdempotentWhenRetried() = runTest {
        val (_, transcript) = stores()
        val value = listOf(message("m0", "zero"))
        transcript.append("device-a", "s1", 0, value)
        transcript.append("device-a", "s1", 0, value)
        assertEquals(listOf("m0"), transcript.load("device-a", "s1").map { it.messageId })
    }

    @Test
    fun legacyAndCorruptPayloadsRemainOpaqueAndRetained() = runTest {
        val (_, transcript) = stores()
        transcript.replace("device-a", "s1", listOf(message("legacy", "column text").copy(payloadJson = "{}")))
        transcript.append("device-a", "s1", 1, listOf(message("broken", "safe text").copy(payloadJson = "not-json")))
        assertEquals(listOf("column text", "safe text"), transcript.load("device-a", "s1").map { it.text })
        assertEquals(listOf("legacy", "broken"), transcript.load("device-a", "s1").map { it.messageId })
        assertEquals(listOf("{}", "not-json"), transcript.load("device-a", "s1").map { it.payloadJson })
    }

    @Test
    fun sessionListPrunesOldestPerDevice() = runTest {
        val (sessions, _) = stores()
        sessions.save("device-a", (20 downTo 0).map { session("s$it", "%04d".format(it)) })
        assertEquals(20, sessions.load("device-a").size)
        assertTrue(sessions.load("device-a").none { it.sessionId == "s0" })
    }

    @Test
    fun cursorRoundTripsPollAndCatalogVersions() = runTest {
        val (_, transcript) = stores()
        transcript.saveCursor("device-a", "s1", PersistedRemoteCursor("poll-7", 12, "models-3"))
        assertEquals(PersistedRemoteCursor("poll-7", 12, "models-3"), transcript.loadCursor("device-a", "s1"))
    }

    private fun session(id: String, updated: String) = PersistedRemoteSession(
        sessionId = id, title = "Title $id", agentType = "remote", status = "ready",
        updatedAt = updated, createdAt = updated, messageCount = 1, lastMessageId = "m0",
    )

    private fun message(id: String, text: String) = PersistedRemoteMessage(
        messageId = id, sessionId = "s1", role = "assistant", text = text,
        status = "completed", timestamp = id, thinking = null, payloadJson = "{}",
    )
}
