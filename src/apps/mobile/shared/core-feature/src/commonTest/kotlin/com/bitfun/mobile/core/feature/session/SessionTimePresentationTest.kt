package com.bitfun.mobile.core.feature.session

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class SessionTimePresentationTest {
    private val now = 1_754_726_400_000L // 2025-08-09T08:00:00Z

    @Test
    fun readsEveryScaleAPeerMightSend() {
        val seconds = 1_754_726_400L
        assertEquals(now, SessionTimePresentation.timestampMs(seconds.toString()))
        assertEquals(now, SessionTimePresentation.timestampMs(now.toString()))
        assertEquals(now, SessionTimePresentation.timestampMs((now * 1_000L).toString()))
        assertEquals(now, SessionTimePresentation.timestampMs((now * 1_000_000L).toString()))
    }

    @Test
    fun readsIsoWithAnOffsetAndWithoutOne() {
        assertEquals(now, SessionTimePresentation.timestampMs("2025-08-09T08:00:00Z"))
        assertEquals(now, SessionTimePresentation.timestampMs("2025-08-09T08:00:00"))
        // The spelling with a space instead of a `T`, which some peers emit.
        assertEquals(now, SessionTimePresentation.timestampMs("2025-08-09 08:00:00"))
        assertEquals(now - 8L * 60L * 60L * 1000L, SessionTimePresentation.timestampMs("2025-08-09"))
    }

    @Test
    fun refusesWhatItCannotRead() {
        assertNull(SessionTimePresentation.timestampMs(""))
        assertNull(SessionTimePresentation.timestampMs("   "))
        assertNull(SessionTimePresentation.timestampMs("yesterday"))
        assertNull(SessionTimePresentation.timestampMs("1.2.3"))
    }

    @Test
    fun bucketsByTheLargestUnitThatFits() {
        fun at(offsetMs: Long) =
            SessionTimePresentation.relative(instant(now - offsetMs), now)

        assertEquals(RelativeTime.JustNow, at(30L * 1000L))
        assertEquals(RelativeTime.MinutesAgo(3), at(3L * 60L * 1000L))
        assertEquals(RelativeTime.HoursAgo(5), at(5L * 60L * 60L * 1000L))
        assertEquals(RelativeTime.DaysAgo(2), at(2L * 24L * 60L * 60L * 1000L))
    }

    @Test
    fun anythingOlderThanAWeekBecomesADate() {
        val row = SessionTimePresentation.relative(instant(now - 8L * 24L * 60L * 60L * 1000L), now)
        // 2025-08-01 in UTC; the assertion only pins the shape, because the
        // calendar date is resolved in whatever zone the reader is in.
        val date = row as RelativeTime.OnDate
        assertEquals(2025, date.year)
        assertEquals(8, date.month)
    }

    @Test
    fun aClockAheadOfOursShowsTheDateRatherThanCountingDown() {
        val ahead = SessionTimePresentation.relative(instant(now + 10L * 60L * 1000L), now)
        assertEquals(RelativeTime.OnDate::class, ahead::class)
        // Inside the tolerance it is still "just now": a few seconds of skew is
        // not worth showing a date for.
        assertEquals(RelativeTime.JustNow, SessionTimePresentation.relative(instant(now + 5_000L), now))
    }

    @Test
    fun anUnparseableTimestampIsUnknownRatherThanNow() {
        assertEquals(RelativeTime.Unknown, SessionTimePresentation.relative("", now))
        assertEquals(RelativeTime.Unknown, SessionTimePresentation.relative("soon", now))
    }

    private fun instant(millis: Long): String = millis.toString()
}
