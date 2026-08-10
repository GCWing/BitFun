package com.bitfun.mobile.core.feature.pairing

import com.bitfun.mobile.core.persistence.SecureStore
import kotlin.time.Clock

/**
 * The consecutive-failure cooldown on pairing credentials.
 *
 * Ports the `MAX_FAILED_USER_ID_ATTEMPTS` / `USER_ID_LOCKOUT_MS` half of
 * `ConnectionErrorPolicy` together with `MobileIdentityStore.saveUserIdProtection`.
 * A desktop that protects its user id is the thing being guarded here: without a
 * cooldown, a device that can reach the relay can try user ids as fast as it can
 * send requests.
 *
 * **It is persisted, and that is the whole point.** A counter that lives only in
 * memory is defeated by force-quitting the app between attempts, which is not a
 * defence at all. [SecureStore] is the store because it is the one that is not a
 * plaintext preference a rooted shell can reset with a single `am` command —
 * this is state about a defence, not a secret, but it deserves the same tamper
 * resistance.
 *
 * An expired lock clears the count as well, so this is a cooldown rather than a
 * ratchet: waiting it out gives the *user* their attempts back too, and a
 * permanent lock on a device that has no other way in would be worse than the
 * attack it prevents.
 */
internal class UserIdProtection(
    private val store: SecureStore?,
    private val now: () -> Long = { Clock.System.now().toEpochMilliseconds() },
) {
    private var failureCount: Int = 0
    private var lockedUntil: Long = 0
    private var restored: Boolean = false

    /** How much of the cooldown is left, or `0` when pairing may be attempted. */
    fun lockedSeconds(): Int {
        restore()
        // A run in progress is not an expired lock. Reading `lockedUntil == 0`
        // as "the cooldown is over" would clear the count on the way into every
        // attempt, and the third failure would never be the third.
        if (lockedUntil == 0L) return 0
        val remaining = lockedUntil - now()
        if (remaining <= 0) {
            // Expired: forget the attempts that produced it, so the next three
            // are the user's again.
            clear()
            return 0
        }
        // Rounded up and never zero: "try again in 0 seconds" next to a button
        // that refuses is worse than saying nothing.
        return ((remaining + MILLIS_PER_SECOND - 1) / MILLIS_PER_SECOND).toInt()
    }

    /**
     * Counts one refused credential.
     *
     * @return the cooldown this failure started, in seconds, or `0` if there are
     * attempts left.
     */
    fun recordFailure(): Int {
        restore()
        failureCount += 1
        lockedUntil = if (failureCount >= MAX_FAILED_ATTEMPTS) now() + LOCKOUT_MS else 0
        persist()
        return if (lockedUntil > 0) (LOCKOUT_MS / MILLIS_PER_SECOND).toInt() else 0
    }

    /** A pairing succeeded, so the run of failures is over. */
    fun recordSuccess() {
        restore()
        if (failureCount != 0 || lockedUntil != 0L) clear()
    }

    private fun restore() {
        if (restored) return
        restored = true
        val stored = store?.read(KEY)?.decodeToString()?.split(SEPARATOR) ?: return
        if (stored.size != 2) return
        failureCount = stored[0].toIntOrNull() ?: 0
        lockedUntil = stored[1].toLongOrNull() ?: 0
    }

    private fun persist() {
        store?.write(KEY, "$failureCount$SEPARATOR$lockedUntil".encodeToByteArray())
    }

    private fun clear() {
        failureCount = 0
        lockedUntil = 0
        store?.delete(KEY)
    }

    private companion object {
        /** `ConnectionErrorPolicy.MAX_FAILED_USER_ID_ATTEMPTS`. */
        const val MAX_FAILED_ATTEMPTS = 3

        /** `ConnectionErrorPolicy.USER_ID_LOCKOUT_MS`. */
        const val LOCKOUT_MS = 60_000L

        const val MILLIS_PER_SECOND = 1_000L

        const val KEY = "user_id_protection"
        const val SEPARATOR = ":"
    }
}

/**
 * Whether a refusal is the desktop or relay saying "not with those credentials".
 *
 * Only a peer's refusal counts. The ArkTS `protectedUserIdError` also matches
 * its own `Missing password` / `Missing username` messages, which are raised
 * before anything is sent — locking someone out of their own app for tapping
 * Connect on an empty field guards nothing, because no attempt ever left the
 * device.
 */
internal fun PairingFailureReason.countsTowardLockout(): Boolean =
    this == PairingFailureReason.Rejected || this == PairingFailureReason.DesktopRejected
