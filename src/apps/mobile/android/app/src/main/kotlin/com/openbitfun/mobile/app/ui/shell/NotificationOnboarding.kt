package com.openbitfun.mobile.app.ui.shell

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.platform.NotificationOnboarding as OnboardingStore

@Composable
internal fun NotificationOnboarding() {
    val context = LocalContext.current
    val store = remember(context) { OnboardingStore(context) }
    var visible by remember { mutableStateOf(store.shouldOffer()) }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
    fun dismiss() {
        store.complete()
        visible = false
    }
    if (visible) {
        AlertDialog(
            onDismissRequest = { dismiss() },
            title = { Text(stringResource(R.string.notification_onboarding_title)) },
            text = { Text(stringResource(R.string.notification_onboarding_body)) },
            confirmButton = {
                TextButton(onClick = {
                    dismiss()
                    permission.launch(Manifest.permission.POST_NOTIFICATIONS)
                }) { Text(stringResource(R.string.notification_onboarding_enable)) }
            },
            dismissButton = {
                TextButton(onClick = { dismiss() }) { Text(stringResource(R.string.notification_onboarding_later)) }
            },
        )
    }
}
