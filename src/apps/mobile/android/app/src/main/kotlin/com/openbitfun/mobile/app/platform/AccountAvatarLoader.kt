package com.openbitfun.mobile.app.platform

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URI

internal suspend fun loadAccountAvatar(value: String?): Bitmap? = withContext(Dispatchers.IO) {
    val uri = runCatching { URI(value ?: return@withContext null) }.getOrNull() ?: return@withContext null
    if (uri.scheme != "https" || uri.host != "avatars.githubusercontent.com") return@withContext null
    val connection = uri.toURL().openConnection() as HttpURLConnection
    try {
        connection.connectTimeout = 10000
        connection.readTimeout = 10000
        connection.instanceFollowRedirects = false
        if (connection.responseCode != 200) return@withContext null
        connection.inputStream.use { stream ->
            val output = java.io.ByteArrayOutputStream()
            val buffer = ByteArray(8192)
            while (true) {
                val count = stream.read(buffer)
                if (count < 0) break
                if (output.size() + count > 2 * 1024 * 1024) return@withContext null
                output.write(buffer, 0, count)
            }
            val bytes = output.toByteArray()
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return@withContext null
            val options = BitmapFactory.Options()
            while (bounds.outWidth / options.inSampleSize > 256 || bounds.outHeight / options.inSampleSize > 256) options.inSampleSize *= 2
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)
        }
    } catch (_: Exception) { null } finally { connection.disconnect() }
}
