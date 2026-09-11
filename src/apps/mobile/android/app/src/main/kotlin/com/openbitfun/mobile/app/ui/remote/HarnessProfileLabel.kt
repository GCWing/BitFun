package com.openbitfun.mobile.app.ui.remote

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.core.feature.session.HarnessProfile

@Composable
internal fun HarnessProfileLabel(profile: HarnessProfile) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(Modifier.size(22.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(2.dp, Alignment.CenterHorizontally)) {
            repeat(profile.ordinal + 1) { index ->
                Box(Modifier.width(4.dp).height((8 + index * 5).dp).background(MaterialTheme.colorScheme.onSurface, RoundedCornerShape(2.dp)))
            }
        }
        Text(stringResource(when (profile) {
            HarnessProfile.MINIMAL -> R.string.harness_minimal
            HarnessProfile.STANDARD -> R.string.harness_standard
            HarnessProfile.ULTIMATE -> R.string.harness_ultimate
        }), style = MaterialTheme.typography.titleSmall)
    }
}
