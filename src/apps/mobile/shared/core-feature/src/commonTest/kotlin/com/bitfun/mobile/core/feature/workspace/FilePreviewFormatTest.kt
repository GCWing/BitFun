package com.bitfun.mobile.core.feature.workspace

import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * The preview header and its truncation banner say sizes on both platforms, and
 * a parity capture compares the two strings directly. These cases are the ones
 * HarmonyOS' `RemoteUiState.formatBytes` decides differently from a naive
 * truncating divide: the unit boundaries, and a half that has to round up.
 */
class FilePreviewFormatTest {
    @Test
    fun bytesBelowAKilobyteKeepTheirOwnUnit() {
        assertEquals("0 B", FilePreviewFormat.bytes(0))
        assertEquals("1023 B", FilePreviewFormat.bytes(1023))
    }

    @Test
    fun kilobytesAreWholeAndRoundHalvesUp() {
        assertEquals("1 KB", FilePreviewFormat.bytes(1024))
        assertEquals("2 KB", FilePreviewFormat.bytes(1536))
        assertEquals("1024 KB", FilePreviewFormat.bytes(1024 * 1024 - 1))
    }

    @Test
    fun aWholeMegabyteCrossesIntoTheLargerUnit() {
        assertEquals("1 MB", FilePreviewFormat.bytes(1024L * 1024))
        assertEquals("3 MB", FilePreviewFormat.bytes(1024L * 1024 * 5 / 2))
    }
}
