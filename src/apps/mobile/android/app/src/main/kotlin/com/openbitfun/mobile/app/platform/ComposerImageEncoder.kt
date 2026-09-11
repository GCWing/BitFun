package com.openbitfun.mobile.app.platform

import android.content.ContentResolver
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.graphics.Matrix
import android.media.ExifInterface
import android.net.Uri
import android.os.Build
import android.util.Base64
import com.openbitfun.mobile.core.feature.session.ComposerImage
import java.io.ByteArrayOutputStream
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Decode camera formats and orientation on the device before crossing the relay. */
internal suspend fun prepareComposerImage(resolver: ContentResolver, uri: Uri): ComposerImage =
    withContext(Dispatchers.IO) {
        val longestSide = 1920
        var bitmap = if (Build.VERSION.SDK_INT >= 28) {
            ImageDecoder.decodeBitmap(ImageDecoder.createSource(resolver, uri)) { decoder, info, _ ->
                val scale = minOf(1.0, longestSide.toDouble() / maxOf(info.size.width, info.size.height))
                decoder.setTargetSize(maxOf(1, (info.size.width * scale).toInt()), maxOf(1, (info.size.height * scale).toInt()))
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
            }
        } else {
            val options = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, options) }
            require(options.outWidth > 0 && options.outHeight > 0) { "Invalid image" }
            options.inSampleSize = 1
            while (maxOf(options.outWidth, options.outHeight) / options.inSampleSize > longestSide) options.inSampleSize *= 2
            options.inJustDecodeBounds = false
            val decoded = resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, options) }
                ?: error("Unable to decode image")
            val orientation = resolver.openInputStream(uri)?.use {
                ExifInterface(it).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)
            } ?: ExifInterface.ORIENTATION_NORMAL
            val matrix = Matrix().apply {
                when (orientation) {
                    ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> setScale(-1f, 1f)
                    ExifInterface.ORIENTATION_ROTATE_180 -> setRotate(180f)
                    ExifInterface.ORIENTATION_FLIP_VERTICAL -> setScale(1f, -1f)
                    ExifInterface.ORIENTATION_TRANSPOSE -> { setRotate(90f); postScale(-1f, 1f) }
                    ExifInterface.ORIENTATION_ROTATE_90 -> setRotate(90f)
                    ExifInterface.ORIENTATION_TRANSVERSE -> { setRotate(270f); postScale(-1f, 1f) }
                    ExifInterface.ORIENTATION_ROTATE_270 -> setRotate(270f)
                }
            }
            Bitmap.createBitmap(decoded, 0, 0, decoded.width, decoded.height, matrix, true).also {
                if (it !== decoded) decoded.recycle()
            }
        }
        try {
            var quality = 85
            while (true) {
                val encoded = ByteArrayOutputStream().use {
                    check(bitmap.compress(Bitmap.CompressFormat.JPEG, quality, it)) { "Image compression failed" }
                    it.toByteArray()
                }
                if (encoded.size <= 1024 * 1024) {
                    return@withContext ComposerImage(
                        id = "android-${UUID.randomUUID()}",
                        dataUrl = "data:image/jpeg;base64," + Base64.encodeToString(encoded, Base64.NO_WRAP),
                        mimeType = "image/jpeg",
                    )
                }
                if (quality > 45) { quality -= 10; continue }
                check(minOf(bitmap.width, bitmap.height) > 64) { "Image exceeds upload limit" }
                val smaller = Bitmap.createScaledBitmap(bitmap, maxOf(1, bitmap.width * 3 / 4), maxOf(1, bitmap.height * 3 / 4), true)
                bitmap.recycle()
                bitmap = smaller
                quality = 75
            }
            @Suppress("UNREACHABLE_CODE")
            error("Image encoding did not complete")
        } finally {
            bitmap.recycle()
        }
    }
