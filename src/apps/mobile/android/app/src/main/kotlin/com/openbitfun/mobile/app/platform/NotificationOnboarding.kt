package com.openbitfun.mobile.app.platform

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.app.Activity
import android.content.ContextWrapper

/** Installation-scoped choice, independent of account and remote task state. */
internal class NotificationOnboarding(private val context: Context) {
    private val preferences = context.getSharedPreferences("notification_onboarding", Context.MODE_PRIVATE)

    fun shouldOffer(): Boolean {
        if (Build.VERSION.SDK_INT < 33 || preferences.getBoolean("completed_v1", false)) return false
        if (context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) return false
        var owner: Context = context
        while (owner is ContextWrapper && owner !is Activity) owner = owner.baseContext
        return (owner as? Activity)?.shouldShowRequestPermissionRationale(
            Manifest.permission.POST_NOTIFICATIONS,
        ) != true
    }

    fun complete() {
        preferences.edit().putBoolean("completed_v1", true).apply()
    }
}
