package com.bitfun.mobile.core.persistence

import app.cash.sqldelight.driver.jdbc.sqlite.JdbcSqliteDriver
import com.bitfun.mobile.core.persistence.db.MobileDatabase
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlinx.coroutines.test.runTest

class DraftStoreTest {
    @Test
    fun roundTripsAndDeletesDrafts() = runTest {
        val driver = JdbcSqliteDriver(JdbcSqliteDriver.IN_MEMORY)
        MobileDatabase.Schema.create(driver).await()
        val store = SqlDelightDraftStore(driver)

        assertNull(store.load("session-1"))
        store.save("session-1", "draft text")
        assertEquals("draft text", store.load("session-1"))
        store.save("session-1", "updated")
        assertEquals("updated", store.load("session-1"))
        store.delete("session-1")
        assertNull(store.load("session-1"))
    }

    @Test
    fun roundTripsGeneralChatSessionAndMessages() = runTest {
        val driver = JdbcSqliteDriver(JdbcSqliteDriver.IN_MEMORY)
        MobileDatabase.Schema.create(driver).await()
        val store = SqlDelightChatLocalStore(driver)
        store.saveSession(
            PersistedChatSession(
                sessionId = "general-chat",
                title = "Question",
                agentType = "general_chat",
                status = "ready",
                updatedAt = "2026-08-09T00:00:01Z",
                createdAt = "2026-08-09T00:00:00Z",
                messageCount = 1,
                pinned = false,
            ),
        )
        store.saveMessage(
            PersistedChatMessage(
                messageId = "message-1",
                sessionId = "general-chat",
                role = "assistant",
                text = "answer",
                status = "completed",
                timestamp = "2026-08-09T00:00:01Z",
                thinking = null,
                payloadJson = "{}",
            ),
        )

        assertEquals(
            listOf("answer"),
            store.loadMessages("general-chat").map { it.text },
        )
    }
}
