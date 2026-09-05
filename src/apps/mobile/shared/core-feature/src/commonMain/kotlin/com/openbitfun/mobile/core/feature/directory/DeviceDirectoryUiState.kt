package com.openbitfun.mobile.core.feature.directory

import com.openbitfun.mobile.core.domain.RecentWorkspace
import com.openbitfun.mobile.core.domain.RemoteSession

/**
 * Where one device's directory entry is in its load lifecycle.
 *
 * [CACHED] means an offline device still has non-empty workspace or session
 * data retained from an earlier successful load. An offline device with no
 * retained data remains [IDLE]; `online = false` carries the offline fact.
 * [CACHED] is therefore not a generic offline marker or a promise of disk
 * hydration. Online entries transition IDLE -> LOADING -> READY/FAILED;
 * online -> offline changes READY/LOADING to CACHED only when data exists, and
 * offline -> online permits a new load/retry.
 */
public enum class DeviceDirectoryStatus {
    IDLE,
    CACHED,
    LOADING,
    READY,
    FAILED,
}

/** Why a device's directory content cannot be shown. */
public enum class DeviceDirectoryFailure {
    NOT_SIGNED_IN,
    NO_WORKSPACE,
    REJECTED,
    NETWORK,
    TIMEOUT,
    RATE_LIMITED,
    LOAD_FAILED,
}

/** One device the account directory knows about. */
public data class DeviceDirectoryDevice public constructor(
    public val deviceId: String,
    public val deviceName: String,
    public val online: Boolean,
) {
    /** A device whose display name is unknown; the id stands in until one arrives. */
    public constructor(deviceId: String, online: Boolean) : this(deviceId, deviceId, online)
}

/** One device's directory state: identity plus the content it loaded. */
public data class DeviceDirectoryEntry public constructor(
    public val deviceId: String,
    public val deviceName: String,
    public val online: Boolean,
    public val expanded: Boolean,
    public val status: DeviceDirectoryStatus,
    public val error: DeviceDirectoryFailure?,
    public val workspaces: List<RecentWorkspace>,
    public val sessions: List<RemoteSession>,
) {
    public companion object {
        public fun empty(deviceId: String, deviceName: String, online: Boolean): DeviceDirectoryEntry =
            DeviceDirectoryEntry(
                deviceId = deviceId,
                deviceName = deviceName,
                online = online,
                expanded = false,
                status = DeviceDirectoryStatus.IDLE,
                error = null,
                workspaces = emptyList(),
                sessions = emptyList(),
            )
    }
}

/** The whole device directory a sidebar renders. */
public data class DeviceDirectoryUiState public constructor(
    public val devices: List<DeviceDirectoryEntry>,
) {
    public fun device(deviceId: String): DeviceDirectoryEntry? =
        devices.firstOrNull { it.deviceId == deviceId }
}

/**
 * Capability for reconciling a create result into one authenticated device row.
 *
 * The opaque [epoch] binds a result to the device membership snapshot in which
 * the create started. Callers obtain this immediately before creating and must
 * return the same key with the confirmed session.
 */
public data class DeviceDirectoryReconcileKey public constructor(
    public val deviceId: String,
    public val epoch: Long,
)

/** Intents the directory store handles. */
public sealed interface DeviceDirectoryIntent {
    /** Replace the device list from the account's latest device projection. */
    public data class Sync public constructor(
        public val devices: List<DeviceDirectoryDevice>,
    ) : DeviceDirectoryIntent

    public data class Load public constructor(
        public val deviceId: String,
    ) : DeviceDirectoryIntent

    public data class Expand public constructor(
        public val deviceId: String,
    ) : DeviceDirectoryIntent

    public data class Collapse public constructor(
        public val deviceId: String,
    ) : DeviceDirectoryIntent

    public data class Retry public constructor(
        public val deviceId: String,
    ) : DeviceDirectoryIntent

    public data object Stop : DeviceDirectoryIntent
}
