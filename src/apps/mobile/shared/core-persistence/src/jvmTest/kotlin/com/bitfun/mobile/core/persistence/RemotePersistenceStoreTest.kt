package com.bitfun.mobile.core.persistence

import app.cash.sqldelight.driver.jdbc.sqlite.JdbcSqliteDriver
import com.bitfun.mobile.core.persistence.db.MobileDatabase
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest

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
