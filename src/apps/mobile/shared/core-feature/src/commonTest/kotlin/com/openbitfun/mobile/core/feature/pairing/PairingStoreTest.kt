package com.openbitfun.mobile.core.feature.pairing

import com.openbitfun.mobile.core.feature.CoreLog
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.feature.connection.connectionPhase
import com.openbitfun.mobile.core.persistence.SecureStore
import com.openbitfun.mobile.core.protocol.InitialSyncResponse
import com.openbitfun.mobile.core.transport.RelayPairing
import com.openbitfun.mobile.core.transport.relayHttpClient
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.http.HttpStatusCode
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue

private val DEVICE = DeviceIdentity(installId = "android-install-42", displayName = "Pixel")

/** Mirrors `PairingStore`'s own interval, which is private and stays that way. */
private const val HEARTBEAT_INTERVAL_MS = 15_000L

/** The same, for `UserIdProtection`'s two constants. */
private const val MAX_FAILED_ATTEMPTS = 3
private const val LOCKOUT_MS = 60_000L

@OptIn(ExperimentalCoroutinesApi::class)
class PairingStoreTest {
    @Test
    fun reachesPairedAndExposesOnlyATruncatedRoomId() = runTest {
        val desktop = FakeDesktop.create()
        val store = storeOver(
            desktop.engine(
                InitialSyncResponse(
                    resp = "ok",
                    hasWorkspace = true,
                    projectName = "OpenBitFun",
                    authenticatedUserId = "alice",
                ),
            ),
        )

        store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))

        val paired = assertIs<PairingUiState.Paired>(store.settle())
        assertEquals("OpenBitFun", paired.workspace.projectName)
        assertEquals("alice", paired.workspace.authenticatedUserId)
        assertTrue(paired.workspace.hasWorkspace)
        assertEquals("01234567", paired.workspace.roomLabel)
        assertFalse(paired.acceptsSubmit)
    }

    @Test
    fun aMalformedLinkFailsWithoutTouchingTheNetwork() = runTest {
        val engine = MockEngine { error("the store must not reach the relay") }
        val store = storeOver(engine)

        store.dispatch(PairingIntent.Submit(pairingUrl = "  "))

        val failed = assertIs<PairingUiState.Failed>(store.state.value)
        assertEquals(PairingFailureReason.PairingLinkEmpty, failed.failure.reason)
        assertEquals(0, engine.requestHistory.size)
    }

    @Test
    fun aLinkWithoutAKeyIsIncomplete() = runTest {
        val store = storeOver(MockEngine { error("unreachable") })

        store.dispatch(PairingIntent.Submit(pairingUrl = "$RELAY_URL/#/pair?room=$ROOM_ID"))

        val failed = assertIs<PairingUiState.Failed>(store.state.value)
        assertEquals(PairingFailureReason.PairingLinkIncomplete, failed.failure.reason)
    }

    @Test
    fun anAccountRoomWithoutAPasswordStopsBeforePairing() = runTest {
        val desktop = FakeDesktop.create()
        val engine = desktop.engine(InitialSyncResponse(resp = "ok"))
        val store = storeOver(engine)

        store.dispatch(
            PairingIntent.Submit(pairingUrl = "${pairingUrl(desktop)}&auth=account&user=alice"),
        )

        val failed = assertIs<PairingUiState.Failed>(store.state.value)
        assertEquals(PairingFailureReason.AccountPasswordRequired, failed.failure.reason)
        assertEquals(0, engine.requestHistory.size)
    }

    @Test
    fun anAccountRoomFallsBackToTheUsernameInTheLink() = runTest {
        val desktop = FakeDesktop.create()
        val store = storeOver(desktop.engine(InitialSyncResponse(resp = "ok")))

        store.dispatch(
            PairingIntent.Submit(
                pairingUrl = "${pairingUrl(desktop)}&auth=account&user=alice",
                userId = "",
                password = "s3cret",
            ),
        )

        assertIs<PairingUiState.Paired>(store.settle())
    }

    @Test
    fun aMissingRoomIsDistinctFromARejection() = runTest {
        for ((status, reason) in listOf(
            404 to PairingFailureReason.RoomNotFound,
            401 to PairingFailureReason.Rejected,
            429 to PairingFailureReason.RateLimited,
            503 to PairingFailureReason.RelayUnavailable,
        )) {
            val desktop = FakeDesktop.create()
            val store = storeOver(MockEngine { respond("", HttpStatusCode.fromValue(status)) })

            store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))

            val failed = assertIs<PairingUiState.Failed>(store.settle(), "status $status")
            assertEquals(reason, failed.failure.reason, "status $status")
        }
    }

    @Test
    fun aDesktopRefusalKeepsItsOwnWording() = runTest {
        val desktop = FakeDesktop.create()
        val store = storeOver(
            desktop.engine(
                InitialSyncResponse(
                    resp = "error",
                    message = "This remote URL is already protected by a different user ID.",
                ),
            ),
        )

        store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))

        val failed = assertIs<PairingUiState.Failed>(store.settle())
        assertEquals(PairingFailureReason.DesktopRejected, failed.failure.reason)
        assertEquals(
            "This remote URL is already protected by a different user ID.",
            failed.failure.remoteMessage,
        )
    }

    /**
     * The cooldown is only a defence if the attempt it refuses never reaches the
     * relay — a request that is sent has already been a guess, whatever the
     * screen then says about it.
     */
    @Test
    fun aRunOfRefusedCredentialsStopsTheNextAttemptBeforeTheNetwork() = runTest {
        val desktop = FakeDesktop.create()
        val engine = MockEngine { respond("", HttpStatusCode.Unauthorized) }
        val store = storeOver(engine)

        repeat(MAX_FAILED_ATTEMPTS - 1) { attempt ->
            store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
            val failed = assertIs<PairingUiState.Failed>(store.settle(), "attempt $attempt")
            assertEquals(PairingFailureReason.Rejected, failed.failure.reason, "attempt $attempt")
            store.dispatch(PairingIntent.Dismiss)
        }

        store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
        val locked = assertIs<PairingUiState.Failed>(store.settle()).failure
        assertEquals(PairingFailureReason.TooManyAttempts, locked.reason)
        assertEquals((LOCKOUT_MS / 1_000).toInt(), locked.retryAfterSeconds)

        val sent = engine.requestHistory.size
        store.dispatch(PairingIntent.Dismiss)
        store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))

        val refused = assertIs<PairingUiState.Failed>(store.state.value).failure
        assertEquals(PairingFailureReason.TooManyAttempts, refused.reason)
        assertEquals(sent, engine.requestHistory.size, "a locked submit reached the relay")
    }

    /**
     * A counter that only lives in memory is defeated by force-quitting the app
     * between attempts, so the one that matters is the one a fresh store reads
     * back out of the keystore.
     */
    @Test
    fun theCooldownOutlivesTheProcessThatEarnedIt() = runTest {
        val desktop = FakeDesktop.create()
        val secure = MemorySecureStore()
        lockOut(desktop, secure)

        val engine = MockEngine { error("a relaunched store must not reach the relay either") }
        val relaunched = storeOver(engine, protection = secure)
        relaunched.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))

        val failure = assertIs<PairingUiState.Failed>(relaunched.state.value).failure
        assertEquals(PairingFailureReason.TooManyAttempts, failure.reason)
        assertTrue(failure.retryAfterSeconds in 1..(LOCKOUT_MS / 1_000).toInt())
    }

    /**
     * Waiting it out gives the attempts back as well: this is a cooldown, and a
     * ratchet on the only way into the app would be worse than what it guards.
     */
    @Test
    fun anExpiredCooldownTakesTheFailureCountWithIt() = runTest {
        val desktop = FakeDesktop.create()
        val secure = MemorySecureStore()
        lockOut(desktop, secure)

        testScheduler.advanceTimeBy(LOCKOUT_MS + 1)

        val store = storeOver(MockEngine { respond("", HttpStatusCode.Unauthorized) }, protection = secure)
        store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))

        // Rejected rather than TooManyAttempts: one refusal after the wait is the
        // first of a new run, not the fourth of the old one.
        val failed = assertIs<PairingUiState.Failed>(store.settle())
        assertEquals(PairingFailureReason.Rejected, failed.failure.reason)
    }

    @Test
    fun aPairThatSucceedsEndsTheRun() = runTest {
        val desktop = FakeDesktop.create()
        val secure = MemorySecureStore()
        val rejecting = storeOver(MockEngine { respond("", HttpStatusCode.Unauthorized) }, protection = secure)
        repeat(MAX_FAILED_ATTEMPTS - 1) {
            rejecting.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
            assertIs<PairingUiState.Failed>(rejecting.settle())
            rejecting.dispatch(PairingIntent.Dismiss)
        }

        val paired = storeOver(desktop.engine(InitialSyncResponse(resp = "ok")), protection = secure)
        paired.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
        assertIs<PairingUiState.Paired>(paired.settle())

        // Two typos and then the right password is an ordinary evening, so the
        // typos must not still be waiting for a third.
        val later = storeOver(MockEngine { respond("", HttpStatusCode.Unauthorized) }, protection = secure)
        later.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
        assertEquals(
            PairingFailureReason.Rejected,
            assertIs<PairingUiState.Failed>(later.settle()).failure.reason,
        )
    }

    /**
     * Only a peer's refusal counts. Tapping Connect on a field left empty is
     * caught before anything is sent, so locking the user out for it would guard
     * nothing at all.
     */
    @Test
    fun anEmptyFieldIsNotAnAttempt() = runTest {
        val desktop = FakeDesktop.create()
        val store = storeOver(desktop.engine(InitialSyncResponse(resp = "ok")))
        val accountUrl = "${pairingUrl(desktop)}&auth=account&user=alice"

        repeat(MAX_FAILED_ATTEMPTS + 1) { attempt ->
            store.dispatch(PairingIntent.Submit(pairingUrl = accountUrl))
            val failed = assertIs<PairingUiState.Failed>(store.state.value, "attempt $attempt")
            assertEquals(
                PairingFailureReason.AccountPasswordRequired,
                failed.failure.reason,
                "attempt $attempt",
            )
            store.dispatch(PairingIntent.Dismiss)
        }

        store.dispatch(
            PairingIntent.Submit(pairingUrl = accountUrl, userId = "", password = "s3cret"),
        )
        assertIs<PairingUiState.Paired>(store.settle())
    }

    @Test
    fun dismissClearsAFailureAndDisconnectDropsThePairedRoom() = runTest {
        val desktop = FakeDesktop.create()
        val store = storeOver(desktop.engine(InitialSyncResponse(resp = "ok")))

        store.dispatch(PairingIntent.Submit(pairingUrl = "  "))
        store.dispatch(PairingIntent.Dismiss)
        assertEquals(PairingUiState.Idle, store.state.value)

        store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
        assertIs<PairingUiState.Paired>(store.settle())

        store.dispatch(PairingIntent.Disconnect)
        assertEquals(PairingUiState.Idle, store.state.value)
    }

    /**
     * The heartbeat belongs to a surface that is on screen. A store nobody has
     * foregrounded — the account sheet's, before a device is picked — must never
     * wake the radio on its own.
     */
    @Test
    fun aStoreThatWasNeverForegroundedNeverPings() = runTest {
        val desktop = FakeDesktop.create()
        val engine = desktop.engine(InitialSyncResponse(resp = "ok"))
        val store = storeOver(engine)

        store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
        assertIs<PairingUiState.Paired>(store.settle())

        val afterPairing = engine.requestHistory.size
        testScheduler.advanceTimeBy(HEARTBEAT_INTERVAL_MS * 4)
        testScheduler.runCurrent()

        assertEquals(afterPairing, engine.requestHistory.size)
    }

    /**
     * A desktop that stops answering has not un-paired: the room, its key and its
     * transport are all still here, so the state stays [PairingUiState.Paired]
     * and one later ping is enough to put it back.
     */
    @Test
    fun aFailedCheckLosesTheLinkWithoutLosingTheRoom() = runTest {
        val desktop = FakeDesktop.create()
        val store = storeOver(desktop.engine(InitialSyncResponse(resp = "ok", projectName = "OpenBitFun")))

        store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
        assertIs<PairingUiState.Paired>(store.settle())

        desktop.offline = true
        store.dispatch(PairingIntent.Foreground)

        val lost = store.awaitLiveness(ConnectionLiveness.LOST)
        assertEquals("OpenBitFun", lost.workspace.projectName)
        assertEquals(ConnectionPhase.FAILED, lost.connectionPhase())
        assertFalse(lost.acceptsSubmit, "a lost link is still a pairing, not a form")

        desktop.offline = false
        store.dispatch(PairingIntent.Verify)

        assertEquals(ConnectionLiveness.LIVE, store.awaitLiveness(ConnectionLiveness.LIVE).liveness)
    }

    /** The timer recovers a lost link on its own, and only while foregrounded. */
    @Test
    fun theHeartbeatTicksUntilTheSurfaceGoesAway() = runTest {
        val desktop = FakeDesktop.create()
        val store = storeOver(desktop.engine(InitialSyncResponse(resp = "ok")))

        store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
        assertIs<PairingUiState.Paired>(store.settle())

        desktop.offline = true
        store.dispatch(PairingIntent.Foreground)
        store.awaitLiveness(ConnectionLiveness.LOST)

        // Nothing dispatched between here and the assertion: the desktop coming
        // back is silent, so only a tick of the store's own timer can notice.
        desktop.offline = false
        testScheduler.advanceTimeBy(HEARTBEAT_INTERVAL_MS + 1_000)
        assertEquals(ConnectionLiveness.LIVE, store.awaitLiveness(ConnectionLiveness.LIVE).liveness)

        desktop.offline = true
        store.dispatch(PairingIntent.Background)
        testScheduler.advanceTimeBy(HEARTBEAT_INTERVAL_MS * 4)
        testScheduler.runCurrent()

        val paired = assertIs<PairingUiState.Paired>(store.state.value)
        assertEquals(ConnectionLiveness.LIVE, paired.liveness, "a stopped timer must not ping")
    }

    @Test
    fun neitherTheRoomIdNorThePasswordReachesTheLog() = runTest {
        val desktop = FakeDesktop.create()
        val log = RecordingCoreLog()
        val store = storeOver(desktop.engine(InitialSyncResponse(resp = "ok")), log)

        store.dispatch(
            PairingIntent.Submit(
                pairingUrl = "${pairingUrl(desktop)}&auth=account&user=alice",
                userId = "",
                password = "s3cret",
            ),
        )
        assertIs<PairingUiState.Paired>(store.settle())

        val joined = log.lines.joinToString("\n")
        assertTrue(log.lines.isNotEmpty())
        assertFalse(ROOM_ID in joined, "the full room id reached the log")
        assertFalse("s3cret" in joined, "the password reached the log")
        assertContains(joined, "room=01234567")
    }

    // --- helpers -----------------------------------------------------------

    // The shape the desktop actually advertises: a hash route whose query
    // carries the room and the key. '+' is escaped because a raw one would be
    // read as a literal '+' either way, and escaping it is what the desktop does.
    private fun pairingUrl(desktop: FakeDesktop) =
        "$RELAY_URL/#/pair?room=$ROOM_ID&pk=${desktop.publicKeyBase64.replace("+", "%2B")}"

    /**
     * Spends the whole run of attempts against [secure], leaving it locked.
     *
     * [PairingIntent.Dismiss] between attempts is not decoration: two identical
     * `Failed` values in a row are conflated by the flow, so without a trip
     * through `Idle` the second wait would return the first attempt's state.
     */
    private suspend fun TestScope.lockOut(desktop: FakeDesktop, secure: SecureStore) {
        val store = storeOver(MockEngine { respond("", HttpStatusCode.Unauthorized) }, protection = secure)
        repeat(MAX_FAILED_ATTEMPTS) {
            store.dispatch(PairingIntent.Submit(pairingUrl = pairingUrl(desktop)))
            assertIs<PairingUiState.Failed>(store.settle())
            store.dispatch(PairingIntent.Dismiss)
        }
    }

    private val scopes = mutableListOf<CoroutineScope>()

    @AfterTest
    fun cancelStoreScopes() {
        scopes.forEach(CoroutineScope::cancel)
    }

    private fun TestScope.storeOver(
        engine: MockEngine,
        log: CoreLog = CoreLog.None,
        protection: SecureStore = MemorySecureStore(),
    ): PairingStore {
        val scope = CoroutineScope(UnconfinedTestDispatcher(testScheduler))
        scopes += scope
        return PairingStore(
            scope = scope,
            device = DEVICE,
            pairing = RelayPairing(relayHttpClient(engine), log.asTransportLog()),
            log = log,
            // Put on virtual time so a sixty-second cooldown can be waited out
            // in a test; in the app it reads the wall clock, which is the only
            // one still running after the process is killed and restarted.
            protection = UserIdProtection(protection) { testScheduler.currentTime },
        )
    }

    /**
     * Waits for the request to land. The mock engine hops off the test
     * dispatcher, so the terminal state has to be awaited rather than reached by
     * advancing virtual time.
     */
    private suspend fun PairingStore.settle(): PairingUiState =
        state.first { it !is PairingUiState.Connecting }

    /** The same wait, for the health check: liveness moves without leaving [PairingUiState.Paired]. */
    private suspend fun PairingStore.awaitLiveness(liveness: ConnectionLiveness): PairingUiState.Paired =
        state.first { it is PairingUiState.Paired && it.liveness == liveness } as PairingUiState.Paired
}

/** The keystore, minus the keystore: what survives here is a store, not a process. */
private class MemorySecureStore : SecureStore {
    private val values = mutableMapOf<String, ByteArray>()

    override fun read(key: String): ByteArray? = values[key]?.copyOf()

    override fun write(key: String, value: ByteArray) {
        values[key] = value.copyOf()
    }

    override fun delete(key: String) {
        values.remove(key)
    }
}
