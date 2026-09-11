package com.openbitfun.mobile.app.platform

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import com.openbitfun.mobile.app.MainActivity
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState
import com.openbitfun.mobile.core.feature.session.TaskCompletionPolicy

/** Best-effort notification while this controller remains alive; execution belongs to the host. */
internal class TaskCompletionNotifier(private val context: Context) {
    private val policy = TaskCompletionPolicy()
    fun setBackground(value: Boolean) { policy.setBackground(value) }
    fun reset() { policy.reset() }
    fun observe(state: RemoteSessionUiState, target: String) {
        val completed = policy.observe(state, target) ?: return
        if (Build.VERSION.SDK_INT >= 33 && context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) return
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel("task_completion", context.getString(R.string.task_completed_title), NotificationManager.IMPORTANCE_DEFAULT))
        val open = PendingIntent.getActivity(context, 0, Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val notification = Notification.Builder(context, "task_completion")
            .setSmallIcon(R.drawable.ic_symbol_checkmark_circle_fill)
            .setContentTitle(context.getString(R.string.task_completed_title))
            .setContentText(context.getString(R.string.task_completed_body))
            .setContentIntent(open).setAutoCancel(true).build()
        manager.notify((completed.target + completed.sessionId + completed.turnId).hashCode(), notification)
    }
}
