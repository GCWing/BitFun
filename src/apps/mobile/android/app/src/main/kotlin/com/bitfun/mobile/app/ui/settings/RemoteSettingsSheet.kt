package com.bitfun.mobile.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Card
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.core.feature.session.ModelOption
import com.bitfun.mobile.core.feature.session.RemoteSessionIntent
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.session.modelOptions

internal const val REMOTE_SETTINGS_TEST_TAG: String = "remote-settings"

/**
 * The settings that belong to one open session.
 *
 * Everything a desktop carries as a whole — which desktop, how it was reached,
 * who is signed in, and what it is allowed to run without asking — lives on
 * [SettingsScreen] to match `RemoteControlSettingsSheet.ets`. What stays here is
 * addressed to a session id on the wire and so has nowhere else to live.
 */
@Composable
internal fun RemoteSettingsSheet(
    state: RemoteSessionUiState.Ready,
    sessionId: String,
    onIntent: (RemoteSessionIntent) -> Unit,
    modifier: Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .testTag(REMOTE_SETTINGS_TEST_TAG),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        ModelSection(state = state, sessionId = sessionId, onIntent = onIntent)
    }
}

@Composable
private fun ModelSection(
    state: RemoteSessionUiState.Ready,
    sessionId: String,
    onIntent: (RemoteSessionIntent) -> Unit,
) {
    val fallback = stringResource(R.string.models_unnamed)
    val options: List<ModelOption> = state.timeline?.modelOptions(fallback).orEmpty()
    if (options.isEmpty()) return

    Text(stringResource(R.string.models_title), style = MaterialTheme.typography.titleMedium)
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(vertical = 4.dp)) {
            options.forEachIndexed { index, option ->
                if (index > 0) HorizontalDivider()
                SelectableRow(
                    label = option.primaryLabel,
                    description = option.secondaryLabel,
                    selected = option.selected,
                    enabled = !state.busy && sessionId.isNotEmpty(),
                    onSelect = {
                        onIntent(RemoteSessionIntent.SelectModel(sessionId, option.id))
                    },
                )
            }
        }
    }
}

@Composable
private fun SelectableRow(
    label: String,
    description: String,
    selected: Boolean,
    enabled: Boolean,
    onSelect: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .selectable(selected = selected, enabled = enabled, onClick = onSelect)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        RadioButton(selected = selected, onClick = onSelect, enabled = enabled)
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge)
            Text(
                description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
