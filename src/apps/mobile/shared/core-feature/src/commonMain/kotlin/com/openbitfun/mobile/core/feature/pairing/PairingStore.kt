package com.openbitfun.mobile.core.feature.pairing

import com.openbitfun.mobile.core.feature.CoreLog
import com.openbitfun.mobile.core.feature.session.RemoteSessionStore
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState
import com.openbitfun.mobile.core.feature.workspace.RemoteWorkspaceStore
import com.openbitfun.mobile.core.persistence.MobilePersistenceStores
import com.openbitfun.mobile.core.persistence.SecureStore
import com.openbitfun.mobile.core.protocol.CommandStatusResponse
import com.openbitfun.mobile.core.protocol.RemoteCommand
import com.openbitfun.mobile.core.transport.PairIdentity
import com.openbitfun.mobile.core.transport.PairedRoom
import com.openbitfun.mobile.core.transport.RelayDescriptor
import com.openbitfun.mobile.core.transport.RelayDescriptorException
import com.openbitfun.mobile.core.transport.RelayDescriptorParser
import com.openbitfun.mobile.core.transport.RelayDescriptorProblem
import com.openbitfun.mobile.core.transport.RelayFailure
import com.openbitfun.mobile.core.transport.RelayPairing
import com.openbitfun.mobile.core.transport.RelayTransportException
import com.openbitfun.mobile.core.transport.TransportLog
import com.openbitfun.mobile.core.transport.relayPairing
import com.openbitfun.mobile.core.transport.send
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * The pairing screen, as state.
 *
 * Holding [PairedRoom] here rather than handing it out is the point of the
 * module boundary: the transport, the descriptor and the shared key stay behind
 * the seam, and the screen sees a [PairedWorkspace]. When the session feature
 * lands it takes the room from here directly, still without the app touching it.
 */
public class PairingStore internal constructor(
    private val scope: CoroutineScope,
    private val device: DeviceIdentity,
    private val pairing: RelayPairing,
    private val log: CoreLog,
    private val protection: UserIdProtection,
    private val persistence: MobilePersistenceStores? = null,
) {
    private val _state = MutableStateFlow<PairingUiState>(PairingUiState.Idle)
    public val state: StateFlow<PairingUiState> = _state.asStateFlow()

    private var inFlight: Job? = null
    private var heartbeat: Job? = null
    private var healthCheck: Job? = null

    /**
     * Whether a surface is currently on screen.
     *
     * Kept rather than inferred so the order of `Foreground` and a successful
     * pair does not matter: pairing while foregrounded starts the heartbeat, and
     * foregrounding while paired does too.
     */
    private var foregrounded: Boolean = false

    /** The paired room, once there is one. Consumed by the session feature. */
    internal var room: PairedRoom? = null
        private set

    /**
     * The session store built over [room], kept only to read its busy flag.
     *
     * Ports the `isBusy()` guard in `RemoteActivityViewModel.checkConnectionHealth`:
     * a silent tick that lands behind a long-running command would time out on
     * the queue rather than on the link, and report a busy desktop as a lost one.
     */
    private var sessionStore: RemoteSessionStore? = null

    /** Builds the session feature without exposing the paired transport to UI. */
    public fun createSessionStore(scope: CoroutineScope): RemoteSessionStore? =
        room?.let {
            RemoteSessionStore.create(
                scope,
                it,
                deviceKey = it.descriptor.roomId,
                persistence = persistence,
            ).also { store -> sessionStore = store }
        }

    /** Builds workspace and file-preview features over the same paired transport. */
    public fun createWorkspaceStore(scope: CoroutineScope): RemoteWorkspaceStore? =
        room?.let { RemoteWorkspaceStore.create(scope, it.transport) }

    public fun dispatch(intent: PairingIntent) {
        when (intent) {
            is PairingIntent.Submit -> submit(intent)
            PairingIntent.Dismiss -> if (_state.value is PairingUiState.Failed) {
                _state.value = PairingUiState.Idle
            }

            PairingIntent.Disconnect -> {
                inFlight?.cancel()
                inFlight = null
                stopHeartbeat()
                sessionStore = null
                room = null
                _state.value = PairingUiState.Idle
            }

            PairingIntent.Foreground -> {
                foregrounded = true
                startHeartbeat()
                // The user just came back to a screen that may have been away for
                // hours, so this one is announced: a blank pause reads as a hang.
                checkHealth(announce = true)
            }

            PairingIntent.Background -> {
                foregrounded = false
                stopHeartbeat()
            }

            PairingIntent.Verify -> checkHealth(announce = true)
        }
    }

    private fun submit(intent: PairingIntent.Submit) {
        if (!_state.value.acceptsSubmit) return

        // Before the link is even parsed, as in `RemoteConnectionController.connect`:
        // a cooldown is about how often credentials may be tried, so it must not
        // be something a differently-malformed link can step around.
        val locked = protection.lockedSeconds()
        if (locked > 0) {
            fail(PairingFailure(PairingFailureReason.TooManyAttempts, null, null, locked))
            return
        }

        // Everything that can be decided without the network is decided before
        // the spinner appears, so a malformed link fails instantly instead of
        // after a connect timeout.
        val descriptor = try {
            RelayDescriptorParser.parse(intent.pairingUrl)
        } catch (cause: RelayDescriptorException) {
            fail(PairingFailure(cause.problem.asReason()))
            return
        }

        val identity = identityFor(descriptor, intent) ?: return

        _state.value = PairingUiState.Connecting
        inFlight = scope.launch {
            try {
                val paired = pairing.pair(
                    descriptor = descriptor,
                    deviceId = device.installId,
                    deviceName = device.displayName,
                    identity = identity,
                )
                room = paired
                protection.recordSuccess()
                _state.value = PairingUiState.Paired(paired.asWorkspace())
                startHeartbeat()
            } catch (cause: CancellationException) {
                throw cause
            } catch (cause: RelayTransportException) {
                fail(cause.failure.asFailure())
            }
        }
    }

    /**
     * Ports `RemotePairingPolicy.identityForConnect`. Auto-reconnect is absent
     * because nothing reconnects yet; when it lands it must still refuse an
     * account room, since the password is never persisted.
     */
    private fun identityFor(
        descriptor: RelayDescriptor,
        intent: PairingIntent.Submit,
    ): PairIdentity? {
        if (!descriptor.accountAuth) {
            // An empty user id means "this device", which is what the desktop
            // shows in its device list.
            return PairIdentity(userId = intent.userId.trim().ifEmpty { device.installId })
        }

        val username = intent.userId.trim().ifEmpty { descriptor.accountUsername.trim() }
        if (username.isEmpty()) {
            fail(PairingFailure(PairingFailureReason.AccountUsernameRequired))
            return null
        }
        if (intent.password.isEmpty()) {
            fail(PairingFailure(PairingFailureReason.AccountPasswordRequired))
            return null
        }
        return PairIdentity(userId = username, password = intent.password)
    }

    /**
     * Reports a refusal, and counts it if it was a peer refusing credentials.
     *
     * The failure that trips the cooldown is *replaced* rather than annotated:
     * at that point the reason the user has to act on is the wait, and the
     * refusal that caused it is the same one they have already been shown twice.
     */
    private fun fail(failure: PairingFailure) {
        val lockedFor = if (failure.reason.countsTowardLockout()) protection.recordFailure() else 0
        val reported = if (lockedFor > 0) {
            PairingFailure(PairingFailureReason.TooManyAttempts, null, null, lockedFor)
        } else {
            failure
        }
        log.warn("pair failed reason=${reported.reason}")
        _state.value = PairingUiState.Failed(reported)
    }

    /**
     * Ports `RemoteHeartbeatController`: one timer, started when a paired surface
     * is on screen and stopped when it leaves.
     *
     * The interval is the source's default and the ping's timeout is deliberately
     * shorter, so a tick can never still be in flight when the next one is due.
     */
    private fun startHeartbeat() {
        if (!foregrounded || _state.value !is PairingUiState.Paired) return
        if (heartbeat?.isActive == true) return
        heartbeat = scope.launch {
            while (isActive) {
                delay(HEARTBEAT_INTERVAL_MS)
                if (sessionIsBusy()) continue
                // Silent: a tick the user did not ask for should not flicker the
                // status dot on its way to the same answer as last time.
                runHealthCheck(announce = false)
            }
        }
    }

    private fun stopHeartbeat() {
        heartbeat?.cancel()
        heartbeat = null
        healthCheck?.cancel()
        healthCheck = null
    }

    private fun checkHealth(announce: Boolean) {
        if (_state.value !is PairingUiState.Paired) return
        if (healthCheck?.isActive == true) return
        healthCheck = scope.launch { runHealthCheck(announce) }
    }

    /**
     * One `ping`, and what its outcome means for [PairingUiState.Paired.liveness].
     *
     * A failure is not a pairing failure: the room stays, so the state stays
     * [PairingUiState.Paired] and only its liveness moves. Dropping to the
     * connect form here would throw away a link that a retry ten seconds later
     * may well find alive.
     */
    private suspend fun runHealthCheck(announce: Boolean) {
        val transport = room?.transport ?: return
        if (announce) setLiveness(ConnectionLiveness.CHECKING)
        try {
            transport.send<CommandStatusResponse>(
                command = RemoteCommand(cmd = "ping"),
                timeoutMs = HEARTBEAT_TIMEOUT_MS,
            )
            setLiveness(ConnectionLiveness.LIVE)
        } catch (cause: CancellationException) {
            throw cause
        } catch (cause: RelayTransportException) {
            // The failure's own class, not its message: a `RemoteRejected` carries
            // text the desktop wrote, which belongs on screen and not in a log.
            log.warn("heartbeat failed reason=${cause.failure::class.simpleName}")
            setLiveness(ConnectionLiveness.LOST)
        }
    }

    private fun setLiveness(liveness: ConnectionLiveness) {
        val paired = _state.value as? PairingUiState.Paired ?: return
        if (paired.liveness != liveness) _state.value = paired.copy(liveness = liveness)
    }

    /** See [sessionStore]. `Loading` is the first list arriving, which is a command too. */
    private fun sessionIsBusy(): Boolean = when (val session = sessionStore?.state?.value) {
        RemoteSessionUiState.Loading -> true
        is RemoteSessionUiState.Ready -> session.busy
        else -> false
    }

    private fun PairedRoom.asWorkspace() = PairedWorkspace(
        roomLabel = descriptor.roomId.take(ROOM_LABEL_LENGTH),
        projectName = initialSync.projectName,
        hasWorkspace = initialSync.hasWorkspace == true,
        authenticatedUserId = initialSync.authenticatedUserId,
    )

    public companion object {
        /** Matches the transport's own log truncation, so the two agree on screen. */
        private const val ROOM_LABEL_LENGTH = 8

        /** `RemoteHeartbeatController`'s own default. */
        private const val HEARTBEAT_INTERVAL_MS = 15_000L

        /** Under the interval on purpose — see [startHeartbeat]. */
        private const val HEARTBEAT_TIMEOUT_MS = 10_000L

        /**
         * Builds a store over the platform's default HTTP engine — OkHttp on
         * Android, Darwin on iOS, both pulled in by `:core-transport`.
         *
         * @param protection where the credential cooldown is kept across
         * launches. See [UserIdProtection]; a store that forgets on restart is
         * not a cooldown.
         */
        public fun create(
            scope: CoroutineScope,
            device: DeviceIdentity,
            protection: SecureStore,
            log: CoreLog,
            persistence: MobilePersistenceStores?,
        ): PairingStore = PairingStore(
            scope = scope,
            device = device,
            pairing = relayPairing(log.asTransportLog()),
            log = log,
            protection = UserIdProtection(protection),
            persistence = persistence,
        )

        /**
         * The same store with logging off. An overload rather than a default
         * argument: Kotlin defaults do not survive into Swift, so an iOS caller
         * would be forced to name a logger it does not want — see the design
         * doc §4.1.
         */
        public fun create(
            scope: CoroutineScope,
            device: DeviceIdentity,
            protection: SecureStore,
        ): PairingStore = create(scope, device, protection, CoreLog.None, null)
    }
}

private fun RelayDescriptorProblem.asReason(): PairingFailureReason = when (this) {
    RelayDescriptorProblem.Empty -> PairingFailureReason.PairingLinkEmpty
    RelayDescriptorProblem.MissingParameters -> PairingFailureReason.PairingLinkIncomplete
    RelayDescriptorProblem.UndecodableQuery -> PairingFailureReason.PairingLinkUndecodable
}

/**
 * `MalformedResponse` covers two situations the transport cannot tell apart but
 * the user can act on differently, so it stays one reason here and the screen
 * says so: either the link's key is unusable, or the peer answered with
 * something that is not the envelope. The former is caught before any request,
 * so by the time a [RelayTransportException] carries it, only the latter is left.
 */
private fun RelayFailure.asFailure(): PairingFailure = when (this) {
    RelayFailure.PairRejected -> PairingFailure(PairingFailureReason.Rejected)
    RelayFailure.RoomNotFound -> PairingFailure(PairingFailureReason.RoomNotFound)
    RelayFailure.Timeout -> PairingFailure(PairingFailureReason.Timeout)
    RelayFailure.RateLimited -> PairingFailure(PairingFailureReason.RateLimited)
    RelayFailure.NetworkUnreachable -> PairingFailure(PairingFailureReason.NetworkUnreachable)
    RelayFailure.MalformedResponse -> PairingFailure(PairingFailureReason.ProtocolMismatch)
    is RelayFailure.RelayUnavailable ->
        PairingFailure(PairingFailureReason.RelayUnavailable, remoteMessage = null, statusCode = statusCode)

    is RelayFailure.UnexpectedStatus ->
        PairingFailure(PairingFailureReason.RelayUnavailable, remoteMessage = null, statusCode = statusCode)

    is RelayFailure.RemoteRejected ->
        PairingFailure(PairingFailureReason.DesktopRejected, remoteMessage = message, statusCode = null)
}

internal fun CoreLog.asTransportLog(): TransportLog = object : TransportLog {
    override fun info(message: String) = this@asTransportLog.info(message)

    override fun warn(message: String) = this@asTransportLog.warn(message)

    override fun error(message: String) = this@asTransportLog.error(message)
}
