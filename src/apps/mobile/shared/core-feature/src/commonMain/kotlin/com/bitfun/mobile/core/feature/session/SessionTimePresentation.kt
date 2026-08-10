package com.bitfun.mobile.core.feature.session

import kotlinx.datetime.LocalDate
import kotlinx.datetime.LocalDateTime
import kotlinx.datetime.TimeZone
import kotlinx.datetime.atStartOfDayIn
import kotlinx.datetime.number
import kotlinx.datetime.toInstant
import kotlinx.datetime.toLocalDateTime
import kotlin.time.Instant

/**
 * How old a timestamp reads next to a session row.
 *
 * A typed result rather than a string, for the usual reason: the wording is a
 * resource on each client, and core does not ship prose. The units are the ones
 * `TimeFormat.ets` picks, so both clients bucket the same instant the same way.
 */
public sealed interface RelativeTime {
    /** The peer sent nothing parseable — not "just now", which would be a lie. */
    public data object Unknown : RelativeTime

    public data object JustNow : RelativeTime

    public data class MinutesAgo public constructor(public val minutes: Int) : RelativeTime

    public data class HoursAgo public constructor(public val hours: Int) : RelativeTime

    public data class DaysAgo public constructor(public val days: Int) : RelativeTime

    /** A calendar date in the reader's own zone, for anything older than a week. */
    public data class OnDate public constructor(
        public val year: Int,
        public val month: Int,
        public val day: Int,
    ) : RelativeTime
}

/**
 * Port of `services/TimeFormat.ets`.
 *
 * This sits in `core-feature` rather than `core-domain` because it exists to
 * decide what a row *says*; the app has to `when` over [RelativeTime], and the
 * architecture guardrail keeps `core-domain` off the app's import list — the
 * same split as `SessionAgentTypes` / `SessionAgentFilter`.
 */
public object SessionTimePresentation {
    private const val MINUTE_MS: Long = 60L * 1000L
    private const val HOUR_MS: Long = 60L * MINUTE_MS
    private const val DAY_MS: Long = 24L * HOUR_MS

    /**
     * Reads the desktop's timestamp, whatever scale it chose.
     *
     * Peers have shipped seconds, milliseconds, microseconds and nanoseconds
     * under the same key, so a bare number is classified by magnitude the way
     * the ArkTS version does. Returns null when nothing parses.
     */
    public fun timestampMs(value: String): Long? {
        val text = value.trim()
        if (text.isEmpty()) return null

        if (text.isNumeric()) {
            val numeric = text.toDoubleOrNull()
            if (numeric != null) {
                val scaled = when {
                    numeric < 100_000_000_000.0 -> numeric * 1000.0
                    numeric < 100_000_000_000_000.0 -> numeric
                    numeric < 100_000_000_000_000_000.0 -> numeric / 1000.0
                    else -> numeric / 1_000_000.0
                }
                return scaled.toLong()
            }
        }

        return text.parseInstantMs() ?: text.replaceFirst(' ', 'T').parseInstantMs()
    }

    /** Buckets [value] against [nowMs]; see [RelativeTime] for the units. */
    public fun relative(value: String, nowMs: Long): RelativeTime {
        val timestamp = timestampMs(value) ?: return RelativeTime.Unknown
        val rawDiff = nowMs - timestamp
        // A clock a minute or more ahead of ours is a skewed peer, not the
        // future: show the date instead of counting down to it.
        if (rawDiff < -MINUTE_MS) return onDate(timestamp)
        val diff = if (rawDiff < 0) 0L else rawDiff
        return when {
            diff < MINUTE_MS -> RelativeTime.JustNow
            diff < HOUR_MS -> RelativeTime.MinutesAgo(maxOf(1L, diff / MINUTE_MS).toInt())
            diff < DAY_MS -> RelativeTime.HoursAgo((diff / HOUR_MS).toInt())
            diff < 7L * DAY_MS -> RelativeTime.DaysAgo((diff / DAY_MS).toInt())
            else -> onDate(timestamp)
        }
    }

    private fun onDate(timestamp: Long): RelativeTime {
        val local = Instant.fromEpochMilliseconds(timestamp)
            .toLocalDateTime(TimeZone.currentSystemDefault())
        return RelativeTime.OnDate(local.year, local.month.number, local.day)
    }

    private fun String.isNumeric(): Boolean {
        var seenDot = false
        var digits = 0
        for (char in this) {
            when {
                char in '0'..'9' -> digits++
                char == '.' && !seenDot && digits > 0 -> seenDot = true
                else -> return false
            }
        }
        return digits > 0
    }

    /**
     * ISO-8601, with a bare date and an offset-less date-time both read as UTC.
     *
     * The ArkTS version leans on `Date.parse`, which resolves an offset-less
     * date-time in the *reader's* zone. Reading it as UTC instead keeps two
     * phones in different zones from disagreeing about when a session was
     * touched; every relay build in tree sends an offset, so this only decides
     * what happens to a peer that does not.
     */
    private fun String.parseInstantMs(): Long? {
        runCatching { return Instant.parse(this).toEpochMilliseconds() }
        runCatching { return LocalDateTime.parse(this).toInstant(TimeZone.UTC).toEpochMilliseconds() }
        runCatching { return LocalDate.parse(this).atStartOfDayIn(TimeZone.UTC).toEpochMilliseconds() }
        return null
    }
}
