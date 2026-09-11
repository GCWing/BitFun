package com.openbitfun.mobile.app

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Color
import androidx.test.platform.app.InstrumentationRegistry
import com.google.android.gms.tasks.Tasks
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import org.junit.Assert.*
import org.junit.Test
import java.util.concurrent.TimeUnit

class InlineQrDecoderTest {
    @Test fun bundledDecoderReadsQrAndIgnoresBlankFrames() {
        val scanner = BarcodeScanning.getClient(BarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE).build())
        val bitmap = InstrumentationRegistry.getInstrumentation().context.assets.open("pairing-qr.png")
            .use { BitmapFactory.decodeStream(it) }
        val blank = Bitmap.createBitmap(320, 320, Bitmap.Config.ARGB_8888).apply { eraseColor(Color.WHITE) }
        try {
            val codes = Tasks.await(scanner.process(InputImage.fromBitmap(bitmap, 0)), 20, TimeUnit.SECONDS)
            assertEquals(listOf("openbitfun-qr-decoder-fixture"), codes.map { it.rawValue })
            val empty = Tasks.await(scanner.process(InputImage.fromBitmap(blank, 0)), 20, TimeUnit.SECONDS)
            assertTrue(empty.isEmpty())
        } finally {
            scanner.close()
            bitmap.recycle()
            blank.recycle()
        }
    }
}
