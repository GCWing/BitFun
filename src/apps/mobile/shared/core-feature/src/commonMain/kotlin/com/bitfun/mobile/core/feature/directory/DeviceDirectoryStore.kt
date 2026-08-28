package com.bitfun.mobile.core.feature.directory

import com.bitfun.mobile.core.domain.RemoteSession
import com.bitfun.mobile.core.feature.account.AccountStore
import com.bitfun.mobile.core.feature.session.RemoteSessionFailureReason
import com.bitfun.mobile.core.feature.session.RemoteSessionIntent
import com.bitfun.mobile.core.feature.session.RemoteSessionStore
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceStore
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * A per-device directory fan-out.
 *
 * Each account device gets its own [RemoteSessionStore] and [RemoteWorkspaceStore]
 * keyed by the device id, so one device loading, failing, or stopping never
 * clears another. This store owns only the fan-out coordination; transport,
 * protocol, and persistence stay inside the stores it reuses.
 */
public class DeviceDirectoryStore internal constructor(
    private val scope: CoroutineScope,
    private val factory: DeviceStoreFactory,
) {
    private val _state = MutableStateFlow<DeviceDirectoryUiState>(DeviceDirectoryUiState(emptyList()))
    public val state: StateFlow<DeviceDirectoryUiState> = _state.asStateFlow()

    private val devices = linkedMapOf<String, DeviceDirectoryEntry>()
    private val slots = mutableMapOf<String, DeviceSlot>()
    private val loads = mutableMapOf<String, Job>()
    private val generations = mutableMapOf<String, Long>()
    private val epochs = mutableMapOf<String, Long>()

    public fun dispatch(intent: DeviceDirectoryIntent) {
        when (intent) {
            is DeviceDirectoryIntent.Sync -> sync(intent.devices)
            is DeviceDirectoryIntent.Load -> load(intent.deviceId)
            is DeviceDirectoryIntent.Expand -> expand(intent.deviceId)
            is DeviceDirectoryIntent.Collapse -> collapse(intent.deviceId)
            is DeviceDirectoryIntent.Retry -> retry(intent.deviceId)
            DeviceDirectoryIntent.Stop -> stop()
        }
    }

    /** Captures the membership epoch that a later confirmed create must match. */
    public fun reconcileKey(deviceId: String): DeviceDirectoryReconcileKey? {
        val id = deviceId.trim()
        val entry = devices[id] ?: return null
        if (!entry.online) return null
        return DeviceDirectoryReconcileKey(id, epochs[id] ?: return null)
    }

    /**
     * Merges one server-confirmed create into only its owning device and writes
     * that device's session-list persistence. A stale membership key is rejected
     * synchronously, so logout/removal/reconnect cannot resurrect an old row.
     */
    public fun reconcileCreatedSession(
        key: DeviceDirectoryReconcileKey,
        session: RemoteSession,
    ): Boolean {
        val id = key.deviceId.trim()
        val current = devices[id] ?: return false
        if (!current.online || epochs[id] != key.epoch || session.id.isBlank()) return false
        val slot = slotFor(id) ?: return false
        if (!slot.sessionStore.reconcileConfirmedCreatedSession(session)) return false
        val sessions = mergeSession(current.sessions, session)
        devices[id] = current.copy(sessions = sessions)
        publish()
        return true
    }

    /**
     * Cancels every running load and releases the underlying stores, while
     * keeping the directory entries themselves: already-loaded data stays on
     * screen and in memory, so a later stop/resume does not need a re-fetch.
     */
    public fun stop() {
        for (job in loads.values) job.cancel()
        loads.clear()
        for (id in devices.keys) {
            invalidate(id)
            invalidateEpoch(id)
            if (devices[id]?.status == DeviceDirectoryStatus.LOADING) {
                devices[id] = devices.getValue(id).copy(status = DeviceDirectoryStatus.IDLE, error = null)
            }
        }
        publish()
        for (slot in slots.values) stopSlot(slot)
        slots.clear()
    }

    private fun sync(incoming: List<DeviceDirectoryDevice>) {
        val ids = linkedSetOf<String>()
        val updated = linkedMapOf<String, DeviceDirectoryEntry>()
        for (device in incoming) {
            val id = device.deviceId.trim()
            if (id.isEmpty()) continue
            ids += id
            val existing = devices[id]
            if (existing == null) {
                invalidateEpoch(id)
            } else if (existing.online && !device.online) {
                invalidate(id)
                invalidateEpoch(id)
                loads.remove(id)?.cancel()
                slots.remove(id)?.let(::stopSlot)
            }
            updated[id] = if (existing == null) {
                DeviceDirectoryEntry.empty(id, device.deviceName, device.online)
            } else {
                existing.copy(
                    deviceName = device.deviceName,
                    online = device.online,
                    expanded = if (device.online) existing.expanded else false,
                    status = if (device.online) {
                        existing.status
                    } else if (existing.workspaces.isNotEmpty() || existing.sessions.isNotEmpty()) {
                        DeviceDirectoryStatus.CACHED
                    } else {
                        DeviceDirectoryStatus.IDLE
                    },
                    error = if (device.online) existing.error else null,
                )
            }
        }
        val removed = devices.keys - ids
        devices.clear()
        devices.putAll(updated)
        publish()
        for (id in removed) {
            invalidate(id)
            invalidateEpoch(id)
            loads.remove(id)?.cancel()
            slots.remove(id)?.let(::stopSlot)
        }
    }

    private fun load(deviceId: String) {
        val id = deviceId.trim()
        if (id.isEmpty()) return
        val entry = devices[id] ?: return
        if (!entry.online) return
        if (loads[id]?.isActive == true) return
        if (entry.status == DeviceDirectoryStatus.READY) return
        val slot = slotFor(id)
        if (slot == null) {
            setEntry(id) { it.copy(status = DeviceDirectoryStatus.FAILED, error = DeviceDirectoryFailure.NOT_SIGNED_IN) }
            return
        }
        startLoad(id, slot, entry)
    }

    private fun expand(deviceId: String) {
        val id = deviceId.trim()
        if (id.isEmpty()) return
        val entry = devices[id] ?: return
        devices[id] = entry.copy(expanded = true)
        publish()
        if (entry.status == DeviceDirectoryStatus.READY) return
        if (entry.online) load(id)
    }

    private fun collapse(deviceId: String) {
        val id = deviceId.trim()
        if (id.isEmpty()) return
        val entry = devices[id] ?: return
        devices[id] = entry.copy(expanded = false)
        publish()
    }

    private fun retry(deviceId: String) {
        val id = deviceId.trim()
        if (id.isEmpty()) return
        val entry = devices[id] ?: return
        if (!entry.online) return
        if (loads[id]?.isActive == true) return
        val expanded = entry.copy(expanded = true)
        devices[id] = expanded
        publish()
        val slot = slotFor(id)
        if (slot == null) {
            setEntry(id) { it.copy(status = DeviceDirectoryStatus.FAILED, error = DeviceDirectoryFailure.NOT_SIGNED_IN) }
            return
        }
        startLoad(id, slot, expanded)
    }

    private fun slotFor(id: String): DeviceSlot? {
        slots[id]?.let { return it }
        val sessionStore = factory.createSessionStore(scope, id) ?: return null
        val workspaceStore = try {
            factory.createWorkspaceStore(scope, id)
        } catch (error: Throwable) {
            sessionStore.stop()
            throw error
        }
        if (workspaceStore == null) {
            sessionStore.stop()
            return null
        }
        val slot = DeviceSlot(sessionStore, workspaceStore)
        slots[id] = slot
        return slot
    }

    private fun startLoad(id: String, slot: DeviceSlot, previous: DeviceDirectoryEntry) {
        val generation = nextGeneration(id)
        setEntry(id) { it.copy(status = DeviceDirectoryStatus.LOADING, error = null) }
        val job = scope.launch {
            try {
                runLoad(id, slot, generation)
            } catch (cancelled: CancellationException) {
                // Cancellation from stop/offline/remove must not restore an entry
                // after a newer generation has already started.
                val current = devices[id]
                if (isCurrent(id, generation) && current?.status == DeviceDirectoryStatus.LOADING) {
                    devices[id] = previous.copy(expanded = current.expanded)
                    publish()
                }
                throw cancelled
            } finally {
                // Never remove a newer job installed for the same device.
                if (loads[id] === coroutineContext[Job]) loads.remove(id)
            }
        }
        loads[id] = job
        if (!job.isActive && loads[id] === job) loads.remove(id)
    }

    private suspend fun runLoad(id: String, slot: DeviceSlot, generation: Long) {
        if (!isCurrent(id, generation)) return
        slot.sessionStore.dispatch(RemoteSessionIntent.Load)
        slot.workspaceStore.dispatch(RemoteWorkspaceIntent.Load)
        val sessionState = slot.sessionStore.state.first(::sessionSettled)
        if (!isCurrent(id, generation)) return
        val workspaceState = slot.workspaceStore.state.first(::workspaceSettled)
        if (!isCurrent(id, generation)) return
        projectResult(id, generation, sessionState, workspaceState)
    }

    private fun sessionSettled(state: RemoteSessionUiState): Boolean = when (state) {
        is RemoteSessionUiState.Failed -> true
        is RemoteSessionUiState.Ready -> !state.busy
        else -> false
    }

    private fun workspaceSettled(state: RemoteWorkspaceUiState): Boolean = when (state) {
        is RemoteWorkspaceUiState.Ready -> true
        is RemoteWorkspaceUiState.Failed -> true
        else -> false
    }

    private fun projectResult(
        id: String,
        generation: Long,
        sessionState: RemoteSessionUiState,
        workspaceState: RemoteWorkspaceUiState,
    ) {
        if (!isCurrent(id, generation)) return
        val current = devices[id] ?: return
        // A device that went offline while its load was in flight must not come
        // back as READY with stale data; it stays in whatever offline state
        // `sync` left it in and the in-flight result is dropped.
        if (!current.online) return
        val workspaces = (workspaceState as? RemoteWorkspaceUiState.Ready)?.workspaces.orEmpty()
        val sessions = (sessionState as? RemoteSessionUiState.Ready)?.sessions.orEmpty()
        val sessionFailed = sessionState as? RemoteSessionUiState.Failed
        val workspaceFailed = workspaceState as? RemoteWorkspaceUiState.Failed
        devices[id] = if (sessionFailed == null && workspaceFailed == null) {
            current.copy(
                status = DeviceDirectoryStatus.READY,
                error = null,
                workspaces = workspaces,
                sessions = sessions,
            )
        } else {
            current.copy(
                status = DeviceDirectoryStatus.FAILED,
                error = sessionFailed?.let { mapSessionFailure(it.reason) } ?: DeviceDirectoryFailure.LOAD_FAILED,
                workspaces = workspaces,
                sessions = sessions,
            )
        }
        publish()
    }

    private fun mapSessionFailure(reason: RemoteSessionFailureReason): DeviceDirectoryFailure = when (reason) {
        RemoteSessionFailureReason.NETWORK -> DeviceDirectoryFailure.NETWORK
        RemoteSessionFailureReason.TIMEOUT -> DeviceDirectoryFailure.TIMEOUT
        RemoteSessionFailureReason.RATE_LIMITED -> DeviceDirectoryFailure.RATE_LIMITED
        RemoteSessionFailureReason.NO_WORKSPACE -> DeviceDirectoryFailure.NO_WORKSPACE
        RemoteSessionFailureReason.REMOTE_REJECTED -> DeviceDirectoryFailure.REJECTED
        else -> DeviceDirectoryFailure.LOAD_FAILED
    }

    private fun nextGeneration(id: String): Long {
        val next = (generations[id] ?: 0L) + 1L
        generations[id] = next
        return next
    }

    private fun invalidate(id: String) {
        generations[id] = (generations[id] ?: 0L) + 1L
    }

    private fun invalidateEpoch(id: String) {
        epochs[id] = (epochs[id] ?: 0L) + 1L
    }

    private fun mergeSession(
        sessions: List<RemoteSession>,
        confirmed: RemoteSession,
    ): List<RemoteSession> =
        listOf(confirmed) + sessions.filterNot { it.id == confirmed.id }

    private fun isCurrent(id: String, generation: Long): Boolean = generations[id] == generation

    private inline fun setEntry(id: String, transform: (DeviceDirectoryEntry) -> DeviceDirectoryEntry) {
        val entry = devices[id] ?: return
        devices[id] = transform(entry)
        publish()
    }

    private fun stopSlot(slot: DeviceSlot) {
        slot.sessionStore.stop()
        slot.workspaceStore.stop()
    }

    private fun publish() {
        _state.value = DeviceDirectoryUiState(devices.values.toList())
    }

    public companion object {
        public fun create(scope: CoroutineScope, accountStore: AccountStore): DeviceDirectoryStore =
            DeviceDirectoryStore(scope, AccountDeviceStoreFactory(accountStore))

        internal fun create(scope: CoroutineScope, factory: DeviceStoreFactory): DeviceDirectoryStore =
            DeviceDirectoryStore(scope, factory)
    }
}

/** Creates a device-keyed store pair through [AccountStore]'s explicit-device entry points. */
private class AccountDeviceStoreFactory(
    private val accountStore: AccountStore,
) : DeviceStoreFactory {
    override fun createSessionStore(scope: CoroutineScope, deviceId: String): RemoteSessionStore? =
        accountStore.createSessionStore(scope, deviceId)

    override fun createWorkspaceStore(scope: CoroutineScope, deviceId: String): RemoteWorkspaceStore? =
        accountStore.createWorkspaceStore(scope, deviceId)
}

internal interface DeviceStoreFactory {
    fun createSessionStore(scope: CoroutineScope, deviceId: String): RemoteSessionStore?
    fun createWorkspaceStore(scope: CoroutineScope, deviceId: String): RemoteWorkspaceStore?
}

private class DeviceSlot(
    val sessionStore: RemoteSessionStore,
    val workspaceStore: RemoteWorkspaceStore,
)
