package com.bitfun.mobile.core.domain

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class ChatSessionControllerTest {
    @Test
    fun ignoresResponseThatCompletesAfterControllerStops() = runTest {
        val deferred = CompletableDeferred<PollSessionResult>()
        val poller = object : ChatSessionPoller {
            override suspend fun pollSession(
                sessionId: String,
                sinceVersion: Int,
                knownMessageCount: Int,
                knownModelCatalogVersion: Long,
            ): PollSessionResult = deferred.await()
        }
        val callbacks = RecordingCallbacks()
        val controller = ChatSessionController.create(this, poller, callbacks)

        controller.start("session-stale", ChatSessionCursor(0, 0, 0))
        runCurrent()
        controller.stop()
        deferred.complete(result(version = 1, changed = true, messages = listOf(message("stale", "assistant", "ignore"))))
        runCurrent()

        assertTrue(callbacks.snapshots.isEmpty())
        assertTrue(callbacks.errors.isEmpty())
    }

    /**
     * Creating a session sends the first message while the session's opening
     * poll is still open, so the nudge that follows lands mid-request. That
     * nudge used to cancel the poll, and the cancellation came back as an error
     * — which stopped the loop dead and left the screen on "replying" with an
     * empty transcript until the user reopened the session.
     */
    @Test
    fun nudgingWhileAPollIsInFlightKeepsTheLoopAlive() = runTest {
        val opening = CompletableDeferred<PollSessionResult>()
        val poller = object : ChatSessionPoller {
            var calls = 0

            override suspend fun pollSession(
                sessionId: String,
                sinceVersion: Int,
                knownMessageCount: Int,
                knownModelCatalogVersion: Long,
            ): PollSessionResult {
                calls += 1
                return if (calls == 1) opening.await() else result(version = 2, changed = true)
            }
        }
        val callbacks = RecordingCallbacks()
        val controller = ChatSessionController.create(this, poller, callbacks)

        controller.start("session-1", ChatSessionCursor(0, 0, 0))
        runCurrent()
        controller.nudge()
        opening.complete(result(version = 1, changed = true))
        runCurrent()
        // Long enough for the active-turn interval the nudge asked for.
        advanceTimeBy(1_000)
        runCurrent()
        controller.stop()

        assertTrue(callbacks.errors.isEmpty())
        // How many polls follow depends on the interval the result settles to,
        // which is tuning; that a second one happened at all is the contract.
        assertTrue(poller.calls >= 2)
    }

    @Test
    fun preservesActiveTurnWhenUnchangedPollOmitsIt() = runTest {
        val poller = QueuePoller(result(version = 1, changed = false, state = "running"))
        val callbacks = RecordingCallbacks()
        val controller = ChatSessionController.create(this, poller, callbacks)

        controller.start(
            "session-1",
            ChatSessionCursor(1, 1, 0),
            message("active-turn-1", "assistant", "Working", "active"),
        )
        runCurrent()
        controller.stop(false)

        assertEquals(1, callbacks.snapshots.size)
        assertFalse(callbacks.snapshots[0].changed)
        assertEquals("active-turn-1", callbacks.snapshots[0].activeTurn?.id)
    }

    @Test
    fun keepsCompletedTurnUntilFinalAssistantArrives() = runTest {
        val poller = QueuePoller(
            result(
                version = 2,
                changed = true,
                activeTurn = message("active-turn-2", "assistant", "Final text", "completed"),
            ),
        )
        val callbacks = RecordingCallbacks()
        val controller = ChatSessionController.create(this, poller, callbacks)
        controller.start(
            "session-1",
            ChatSessionCursor(1, 1, 0),
            message("active-turn-2", "assistant", "Final text", "active"),
        )
        runCurrent()

        poller.results += result(version = 3, changed = true)
        controller.pollNow()
        poller.results += result(
            version = 4,
            changed = true,
            messages = listOf(message("assistant-final-1", "assistant", "Final text")),
            count = 2,
        )
        controller.pollNow()
        controller.stop(false)

        assertEquals(3, callbacks.snapshots.size)
        assertEquals("completed", callbacks.snapshots[0].activeTurn?.status)
        assertEquals("active-turn-2", callbacks.snapshots[1].activeTurn?.id)
        assertEquals(null, callbacks.snapshots[2].activeTurn)
    }

    @Test
    fun requestsResyncThroughoutTurnEndSettleWindow() = runTest {
        val poller = QueuePoller(
            result(
                version = 2,
                changed = true,
                messages = listOf(message("turn-1-assistant", "assistant", "")),
                count = 2,
            ),
        )
        val callbacks = RecordingCallbacks()
        val controller = ChatSessionController.create(this, poller, callbacks)
        controller.start(
            "session-1",
            ChatSessionCursor(1, 1, 0),
            message("active-turn-1", "assistant", "", "active"),
        )
        runCurrent()

        poller.results += result(version = 3, changed = true, count = 2)
        controller.pollNow()
        controller.stop(false)

        assertEquals(2, callbacks.snapshots.size)
        assertTrue(callbacks.snapshots[0].shouldSyncAfterTurnEnded)
        assertTrue(callbacks.snapshots[1].shouldSyncAfterTurnEnded)
    }

    private class QueuePoller(vararg initial: PollSessionResult) : ChatSessionPoller {
        val results = ArrayDeque(initial.toList())

        override suspend fun pollSession(
            sessionId: String,
            sinceVersion: Int,
            knownMessageCount: Int,
            knownModelCatalogVersion: Long,
        ): PollSessionResult = results.removeFirst()
    }

    private class RecordingCallbacks : ChatSessionControllerCallbacks {
        val snapshots = mutableListOf<ChatSessionSnapshot>()
        val errors = mutableListOf<Throwable>()

        override fun onSnapshot(snapshot: ChatSessionSnapshot) {
            snapshots += snapshot
        }

        override fun onError(error: Throwable) {
            errors += error
        }

        override fun canPoll(sessionId: String): Boolean = true
    }

    private fun result(
        version: Int,
        changed: Boolean,
        state: String = "idle",
        messages: List<ChatMessage> = emptyList(),
        count: Int = 1,
        activeTurn: ChatMessage? = null,
    ): PollSessionResult = PollSessionResult(
        version = version,
        changed = changed,
        sessionState = state,
        title = "Session",
        newMessages = messages,
        totalMessageCount = count,
        activeTurn = activeTurn,
        modelCatalog = null,
    )

    private fun message(
        id: String,
        role: String,
        text: String,
        status: String = if (role == "assistant") "done" else "sent",
    ): ChatMessage = ChatMessage(
        id = id,
        role = role,
        text = text,
        status = status,
        renderVersion = null,
        turnId = null,
        detail = null,
        timestamp = null,
        thinking = null,
        tools = null,
        items = null,
        images = null,
    )
}
