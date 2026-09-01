package com.bitfun.mobile.app.ui.settings

import androidx.annotation.DrawableRes
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.chat.messageRes
import com.bitfun.mobile.app.ui.theme.bitFunColors
import com.bitfun.mobile.app.ui.theme.generated.MobileDesignGeometry
import com.bitfun.mobile.core.feature.generalchat.GeneralChatConfigFailure
import com.bitfun.mobile.core.feature.generalchat.GeneralChatConfigUi
import com.bitfun.mobile.core.feature.generalchat.GeneralChatConnectionTestUi
import com.bitfun.mobile.core.feature.generalchat.GeneralChatIntent
import com.bitfun.mobile.core.feature.generalchat.GeneralChatModelSource
import com.bitfun.mobile.core.feature.generalchat.GeneralChatModelUi

internal const val MODEL_SERVICE_TEST_TAG: String = "model-service"
internal const val MODEL_SERVICE_CURRENT_TEST_TAG: String = "model-service-current"
internal const val MODEL_SERVICE_ACCOUNT_TEST_TAG: String = "model-service-account"
internal const val MODEL_SERVICE_LOCAL_TEST_TAG: String = "model-service-local"
internal const val MODEL_SERVICE_BACK_TEST_TAG: String = "model-service-back"
internal const val MODEL_SERVICE_CLOSE_TEST_TAG: String = "model-service-close"
internal const val MODEL_SERVICE_URL_TEST_TAG: String = "model-service-url"
internal const val MODEL_SERVICE_KEY_TEST_TAG: String = "model-service-key"
internal const val MODEL_SERVICE_MODEL_TEST_TAG: String = "model-service-model"
internal const val MODEL_SERVICE_PROBE_TEST_TAG: String = "model-service-probe"
internal const val MODEL_SERVICE_SAVE_TEST_TAG: String = "model-service-save"

/**
 * The model panel, ported from `pages/components/ModelServiceSettingsPanel.ets`.
 *
 * Two pages behind one header, as the source is: an overview that says which
 * model is answering and where it came from, and — behind the local row — the
 * form that defines one. The form is not the landing page because most openings
 * of this panel are to check rather than to edit, and a page of empty credential
 * fields is a poor answer to "which model am I using".
 *
 * The middle section, the account's synced models, is a summary rather than a
 * list, exactly as the source draws it: it says how many arrived and whether one
 * of them is answering, and nothing on it is tappable. Picking among them is the
 * composer's model control, not this panel's.
 *
 * @param models every model that could answer, from [GeneralChatUiState]; the
 * account's arrive only while signed in.
 * @param activeModelId which of [models] is answering, empty when none can.
 * @param connectionTest the result of the last probe, deliberately not the result
 * of the last save — testing writes nothing either way.
 * @param onSave dispatches the save and answers whether the store took it, which
 * decides whether the form may close. Dispatch is synchronous, so the caller can
 * read the verdict back immediately; waiting for a recomposition here would close
 * the form over a refusal and leave the reason on a page that has no field to
 * put it beside.
 */
@Composable
internal fun ModelServiceScreen(
    config: GeneralChatConfigUi,
    models: List<GeneralChatModelUi>,
    activeModelId: String,
    failure: GeneralChatConfigFailure?,
    connectionTest: GeneralChatConnectionTestUi,
    onIntent: (GeneralChatIntent) -> Unit,
    onSave: (GeneralChatIntent.SaveConfig) -> Boolean,
    onClose: () -> Unit,
    modifier: Modifier,
) {
    var page by rememberSaveable { mutableStateOf(ModelServicePage.OVERVIEW) }
    val leaveChild = {
        page = ModelServicePage.OVERVIEW
        onIntent(GeneralChatIntent.ClearConfigFailure)
        onIntent(GeneralChatIntent.ClearConnectionTest)
    }
    BackHandler(enabled = page != ModelServicePage.OVERVIEW, onBack = leaveChild)

    Column(modifier = modifier.testTag(MODEL_SERVICE_TEST_TAG)) {
        ModelServiceHeader(
            title = stringResource(
                when (page) {
                    ModelServicePage.OVERVIEW -> R.string.model_service_manage_title
                    ModelServicePage.ACCOUNT -> R.string.model_service_choose_account
                    ModelServicePage.LOCAL -> R.string.model_service_local_title
                },
            ),
            onBack = if (page == ModelServicePage.OVERVIEW) null else leaveChild,
            onClose = onClose,
        )

        when (page) {
            ModelServicePage.LOCAL -> LocalModelEditor(
                config = config,
                failure = failure,
                connectionTest = connectionTest,
                onIntent = onIntent,
                onSave = onSave,
                onSaved = { page = ModelServicePage.OVERVIEW },
                modifier = Modifier.weight(1f),
            )
            ModelServicePage.ACCOUNT -> AccountModelSelection(
                models = models.filter { it.source == GeneralChatModelSource.ACCOUNT },
                activeModelId = activeModelId,
                onSelect = {
                    onIntent(GeneralChatIntent.SelectModel(it))
                    page = ModelServicePage.OVERVIEW
                },
                modifier = Modifier.weight(1f),
            )
            ModelServicePage.OVERVIEW -> ModelOverview(
                config = config,
                models = models,
                activeModelId = activeModelId,
                onOpenAccount = { page = ModelServicePage.ACCOUNT },
                onSelectLocal = {
                    models.firstOrNull { it.source == GeneralChatModelSource.LOCAL }?.let { model ->
                        onIntent(GeneralChatIntent.SelectModel(model.id))
                    }
                },
                onEditLocal = {
                    page = ModelServicePage.LOCAL
                    onIntent(GeneralChatIntent.ClearConfigFailure)
                    onIntent(GeneralChatIntent.ClearConnectionTest)
                },
                modifier = Modifier.weight(1f),
            )
        }
    }
}

private enum class ModelServicePage { OVERVIEW, ACCOUNT, LOCAL }

/**
 * Back, title, close — the one row that stays put while the page under it swaps.
 *
 * [onBack] is null on the overview because there is nothing behind it, which is
 * the same `if (this.showLocalEditor)` that draws the source's chevron.
 */
@Composable
private fun ModelServiceHeader(title: String, onBack: (() -> Unit)?, onClose: () -> Unit) {
    Column {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(MobileDesignGeometry.SheetHeaderHeight)
                .padding(start = 16.dp, end = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            onBack?.let { back ->
                IconButton(
                    onClick = back,
                    modifier = Modifier
                        .padding(end = 8.dp)
                        .size(42.dp)
                        .testTag(MODEL_SERVICE_BACK_TEST_TAG),
                ) {
                    Icon(
                        painterResource(R.drawable.ic_symbol_chevron_left),
                        contentDescription = stringResource(R.string.common_back),
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
            Text(
                title,
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                // One weight, not a weighted title beside a weighted spacer:
                // two of them split the free space evenly and ellipsise a title
                // that had room to spare.
                modifier = Modifier.weight(1f).padding(end = 12.dp),
            )
            IconButton(
                onClick = onClose,
                modifier = Modifier.size(MobileDesignGeometry.SelectionCloseSize)
                    .testTag(MODEL_SERVICE_CLOSE_TEST_TAG),
            ) {
                Icon(
                    painterResource(R.drawable.ic_symbol_xmark),
                    contentDescription = stringResource(R.string.common_close),
                    modifier = Modifier.size(18.dp),
                )
            }
        }
        HorizontalDivider()
    }
}

@Composable
private fun ModelOverview(
    config: GeneralChatConfigUi,
    models: List<GeneralChatModelUi>,
    activeModelId: String,
    onOpenAccount: () -> Unit,
    onSelectLocal: () -> Unit,
    onEditLocal: () -> Unit,
    modifier: Modifier,
) {
    val complete = config.baseUrl.isNotBlank() && config.model.isNotBlank() && config.hasApiKey
    val notConfigured = stringResource(R.string.model_service_not_configured)
    val localSource = stringResource(R.string.model_service_local_source)
    val accountSource = stringResource(R.string.model_service_account_source)
    val active = models.firstOrNull { it.id == activeModelId }
    val accountModels = models.filter { it.source == GeneralChatModelSource.ACCOUNT }

    Column(
        modifier = modifier
            .verticalScroll(rememberScrollState())
            .padding(
                start = 16.dp,
                end = 16.dp,
                top = MobileDesignGeometry.ModelOverviewTopPadding,
                bottom = MobileDesignGeometry.ModelOverviewBottomPadding,
            ),
        verticalArrangement = Arrangement.spacedBy(MobileDesignGeometry.ModelSectionGap),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            OverviewSectionHeader(stringResource(R.string.model_service_current))
            OverviewRow(
                icon = R.drawable.ic_symbol_checkmark_circle_fill,
                iconSize = 21,
                iconTint = if (active != null) {
                    MaterialTheme.colorScheme.onSurface
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant
                },
                title = active?.label ?: notConfigured,
                // Where it came from, not what it is called: the same model name
                // can arrive from this phone's own form and from the account, and
                // which of the two is answering decides whose key is being spent.
                subtitle = when (active?.source) {
                    GeneralChatModelSource.LOCAL -> localSource
                    GeneralChatModelSource.ACCOUNT -> accountSource
                    null -> ""
                },
                minHeight = MobileDesignGeometry.ModelCurrentRowHeight.value.toInt(),
                selected = false,
                chevron = false,
                onClick = null,
                modifier = Modifier.testTag(MODEL_SERVICE_CURRENT_TEST_TAG),
            )
        }

        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            OverviewSectionHeader(stringResource(R.string.model_service_sources))
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(MobileDesignGeometry.SettingsCompactCardRadius))
                    .background(MaterialTheme.colorScheme.surfaceVariant),
            ) {
                ModelSourceRow(
                    icon = R.drawable.ic_symbol_cloud,
                    iconTint = if (accountModels.isEmpty()) {
                        MaterialTheme.colorScheme.outline
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    title = stringResource(R.string.model_service_account_summary),
                    subtitle = if (accountModels.isEmpty()) {
                        stringResource(R.string.model_service_account_empty)
                    } else {
                        stringResource(R.string.model_service_account_synced, accountModels.size)
                    },
                    onBodyClick = onOpenAccount,
                    onChevronClick = onOpenAccount,
                    modifier = Modifier.testTag(MODEL_SERVICE_ACCOUNT_TEST_TAG),
                )
                HorizontalDivider(modifier = Modifier.padding(start = 56.dp))
                ModelSourceRow(
                    icon = R.drawable.ic_symbol_wrench_and_screwdriver,
                    iconTint = MaterialTheme.colorScheme.onSurfaceVariant,
                    title = if (complete) config.model else notConfigured,
                    subtitle = if (complete) localSource else "",
                    onBodyClick = if (complete) onSelectLocal else onEditLocal,
                    onChevronClick = onEditLocal,
                    modifier = Modifier.testTag(MODEL_SERVICE_LOCAL_TEST_TAG),
                )
            }
        }
    }
}

@Composable
private fun ModelSourceRow(
    @DrawableRes icon: Int,
    iconTint: Color,
    title: String,
    subtitle: String,
    onBodyClick: () -> Unit,
    onChevronClick: () -> Unit,
    modifier: Modifier,
) {
    Row(modifier = modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Row(
            modifier = Modifier
                .weight(1f)
                .height(MobileDesignGeometry.ModelSourceRowHeight)
                .clickable(onClick = onBodyClick)
                .padding(start = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Icon(
                painterResource(icon),
                contentDescription = null,
                tint = iconTint,
                modifier = Modifier.size(22.dp),
            )
            Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Text(
                    title,
                    style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (subtitle.isNotEmpty()) {
                    Text(
                        subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
            }
        }
        IconButton(
            onClick = onChevronClick,
            modifier = Modifier.size(44.dp),
        ) {
            Icon(
                painterResource(R.drawable.ic_symbol_chevron_right),
                contentDescription = stringResource(R.string.common_open),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(16.dp),
            )
        }
    }
}

@Composable
private fun AccountModelSelection(
    models: List<GeneralChatModelUi>,
    activeModelId: String,
    onSelect: (String) -> Unit,
    modifier: Modifier,
) {
    if (models.isEmpty()) {
        Text(
            stringResource(R.string.model_service_account_empty),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = modifier
                .fillMaxWidth()
                .heightIn(min = MobileDesignGeometry.ModelEmptyAccountHeight)
                .padding(horizontal = 16.dp, vertical = MobileDesignGeometry.ModelListTopPadding),
        )
        return
    }
    Column(
        modifier = modifier
            .verticalScroll(rememberScrollState())
            .padding(
                start = 10.dp,
                end = 10.dp,
                top = MobileDesignGeometry.ModelListTopPadding,
                bottom = MobileDesignGeometry.ModelListBottomPadding,
            ),
        verticalArrangement = Arrangement.spacedBy(MobileDesignGeometry.ModelAccountRowGap),
    ) {
        models.forEach { model ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(MobileDesignGeometry.ModelAccountRowHeight)
                    .clip(RoundedCornerShape(9.dp))
                    .background(
                        if (model.id == activeModelId) MaterialTheme.colorScheme.surfaceVariant
                        else bitFunColors.transparent,
                    )
                    .clickable { onSelect(model.id) }
                    .padding(horizontal = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Box(modifier = Modifier.size(20.dp), contentAlignment = Alignment.Center) {
                    if (model.id == activeModelId) {
                        Icon(
                            painterResource(R.drawable.ic_symbol_checkmark_circle),
                            contentDescription = null,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                }
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        model.label,
                        style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        stringResource(R.string.model_service_account_source),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

/**
 * The body both pages share: centred when it fits, scrolled when it does not.
 *
 * The source's `Scroll` centres its content, and on a phone neither of these
 * pages fills the sheet — the overview is two rows and the form is three fields,
 * so a top-aligned column would leave the whole lower half empty and hang the
 * content off the header. `heightIn(min = maxHeight)` is what lets
 * [Arrangement.Center] mean anything inside a scroller, whose own height is
 * unbounded: it floors the column at the viewport, and anything taller than that
 * scrolls from the top as usual.
 */
@Composable
private fun CentredPage(modifier: Modifier, content: @Composable ColumnScope.() -> Unit) {
    BoxWithConstraints(modifier = modifier) {
        // A sheet can hand its content an unbounded height, and `maxHeight` is
        // then a number no page should be floored at. Nothing to centre against
        // in that case, so the column simply wraps.
        val floor = if (constraints.hasBoundedHeight) maxHeight else 0.dp
        Column(
            modifier = Modifier
                .verticalScroll(rememberScrollState())
                .heightIn(min = floor)
                .padding(start = 22.dp, end = 22.dp, top = 18.dp, bottom = 30.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp, Alignment.CenterVertically),
            content = content,
        )
    }
}

@Composable
private fun OverviewSectionHeader(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 4.dp),
    )
}

/**
 * One soft-filled row of the overview.
 *
 * [selected] draws the check the source puts on whichever model is actually in
 * use, so the answer to "which one" survives there being more than one row.
 */
@Composable
private fun OverviewRow(
    @DrawableRes icon: Int,
    iconSize: Int,
    iconTint: Color,
    title: String,
    subtitle: String,
    minHeight: Int,
    selected: Boolean,
    chevron: Boolean,
    onClick: (() -> Unit)?,
    modifier: Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .then(if (onClick == null) Modifier else Modifier.clickable(onClick = onClick))
            .defaultMinSize(minHeight = minHeight.dp)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // A fixed 28dp box rather than the glyph's own size: the source's icons
        // are drawn at different weights and the text column has to start in the
        // same place on every row regardless.
        Box(modifier = Modifier.size(28.dp), contentAlignment = Alignment.Center) {
            Icon(
                painterResource(icon),
                contentDescription = null,
                tint = iconTint,
                modifier = Modifier.size(iconSize.dp),
            )
        }
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (subtitle.isNotEmpty()) {
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (selected) {
            Icon(
                painterResource(R.drawable.ic_symbol_checkmark_circle_fill),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.size(18.dp),
            )
        }
        if (chevron) {
            Icon(
                painterResource(R.drawable.ic_symbol_chevron_right),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(16.dp),
            )
        }
    }
}

/**
 * The provider form.
 *
 * The key field starts empty even when a key is stored, and a blank field means
 * "keep it": a saved key is never read back out of the keystore for display, so
 * there is nothing here for a screenshot or an accessibility dump to leak.
 * Forgetting a key is an explicit choice rather than an empty field.
 *
 * The two visible fields survive process death and the key deliberately does not
 * — saved instance state is a plain file, and no secret goes into one.
 */
@Composable
private fun LocalModelEditor(
    config: GeneralChatConfigUi,
    failure: GeneralChatConfigFailure?,
    connectionTest: GeneralChatConnectionTestUi,
    onIntent: (GeneralChatIntent) -> Unit,
    onSave: (GeneralChatIntent.SaveConfig) -> Boolean,
    onSaved: () -> Unit,
    modifier: Modifier,
) {
    var apiUrl by rememberSaveable(config.baseUrl) { mutableStateOf(config.baseUrl) }
    var modelName by rememberSaveable(config.model) { mutableStateOf(config.model) }
    var apiKey by remember(config.hasApiKey) { mutableStateOf("") }
    var clearApiKey by rememberSaveable(config.hasApiKey) { mutableStateOf(false) }
    // Not saveable either: which way the eye is pointing is only interesting
    // while the field is on screen, and restoring it as "revealed" would put a
    // key back in the clear on a screen the user has just returned to.
    var revealKey by remember(config.hasApiKey) { mutableStateOf(false) }
    val busy = connectionTest.running
    // The button is disabled rather than left to fail: a probe with no credential
    // to send would be answered 401 by a perfectly healthy endpoint.
    val testable = apiKey.isNotBlank() || (config.hasApiKey && !clearApiKey)
    // Any edit invalidates the last verdict — it was about the endpoint as it was
    // typed then, and leaving "passed" under a changed URL would vouch for one
    // the app has never called.
    val edited = { onIntent(GeneralChatIntent.ClearConnectionTest) }

    CentredPage(modifier = modifier) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            FieldLabel(stringResource(R.string.model_service_api_url), trailing = "")
            SoftTextField(
                value = apiUrl,
                onValueChange = { apiUrl = it; edited() },
                placeholder = stringResource(R.string.model_service_api_url_placeholder),
                isError = failure == GeneralChatConfigFailure.INVALID_URL,
                keyboardType = KeyboardType.Uri,
                password = false,
                trailing = null,
                enabled = !busy,
                modifier = Modifier.testTag(MODEL_SERVICE_URL_TEST_TAG),
            )
        }

        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            FieldLabel(
                stringResource(R.string.model_service_api_key),
                trailing = if (config.hasApiKey && !clearApiKey) {
                    stringResource(R.string.model_service_key_saved)
                } else {
                    ""
                },
            )
            SoftTextField(
                value = apiKey,
                onValueChange = {
                    apiKey = it
                    // Typing a key and asking to forget one are contradictory, and
                    // the newer of the two is the one the user meant.
                    if (it.isNotEmpty()) clearApiKey = false
                    edited()
                },
                placeholder = stringResource(
                    if (config.hasApiKey && !clearApiKey) {
                        R.string.model_service_api_key_keep_placeholder
                    } else {
                        R.string.model_service_api_key_placeholder
                    },
                ),
                isError = failure == GeneralChatConfigFailure.API_KEY_REQUIRED,
                keyboardType = KeyboardType.Password,
                password = !revealKey,
                // The source's eye, which exists because an API key is pasted or
                // typed once and a mistyped one fails as an authentication error
                // hours later, with nothing on screen to check it against.
                trailing = {
                    IconButton(onClick = { revealKey = !revealKey }, enabled = !busy) {
                        Icon(
                            painterResource(
                                if (revealKey) {
                                    R.drawable.ic_symbol_eye
                                } else {
                                    R.drawable.ic_symbol_eye_slash
                                },
                            ),
                            contentDescription = stringResource(
                                if (revealKey) {
                                    R.string.model_service_hide_key
                                } else {
                                    R.string.model_service_show_key
                                },
                            ),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(21.dp),
                        )
                    }
                },
                enabled = !busy,
                modifier = Modifier.testTag(MODEL_SERVICE_KEY_TEST_TAG),
            )
            if (config.hasApiKey) {
                // A line of text rather than a switch, as the source draws it: the
                // choice is one-off and destructive, and it reads as the sentence
                // it is — "clear the saved key" — rather than as a setting whose
                // on-state has to be decoded before it can be trusted.
                Text(
                    stringResource(
                        if (clearApiKey) {
                            R.string.model_service_keep_key
                        } else {
                            R.string.model_service_clear_key
                        },
                    ),
                    style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
                    color = if (clearApiKey) {
                        MaterialTheme.colorScheme.onSurface
                    } else {
                        MaterialTheme.colorScheme.error
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(enabled = !busy) {
                            clearApiKey = !clearApiKey
                            apiKey = ""
                            edited()
                        }
                        .padding(vertical = 4.dp),
                )
            }
        }

        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            FieldLabel(stringResource(R.string.model_service_model_name), trailing = "")
            SoftTextField(
                value = modelName,
                onValueChange = { modelName = it; edited() },
                placeholder = stringResource(R.string.model_service_model_placeholder),
                isError = failure == GeneralChatConfigFailure.MODEL_REQUIRED,
                keyboardType = KeyboardType.Text,
                password = false,
                trailing = null,
                enabled = !busy,
                modifier = Modifier.testTag(MODEL_SERVICE_MODEL_TEST_TAG),
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            FilledTonalButton(
                onClick = {
                    onIntent(GeneralChatIntent.TestConnection(apiUrl, modelName, apiKey, clearApiKey))
                },
                enabled = !busy && testable,
                shape = RoundedCornerShape(25.dp),
                modifier = Modifier.weight(1f).height(44.dp).testTag(MODEL_SERVICE_PROBE_TEST_TAG),
            ) {
                Text(
                    stringResource(
                        if (busy) R.string.model_service_testing else R.string.model_service_test_connection,
                    ),
                )
            }
            Button(
                onClick = {
                    val accepted = onSave(
                        GeneralChatIntent.SaveConfig(apiUrl, modelName, apiKey, clearApiKey),
                    )
                    if (accepted) onSaved()
                },
                enabled = !busy,
                shape = RoundedCornerShape(25.dp),
                modifier = Modifier.weight(1f).height(44.dp).testTag(MODEL_SERVICE_SAVE_TEST_TAG),
            ) {
                Text(stringResource(R.string.model_service_save))
            }
        }

        if (!testable) {
            Text(
                stringResource(R.string.model_service_test_needs_key),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        failure?.let { reason ->
            Text(
                stringResource(reason.messageRes()),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
            )
        }

        connectionTest.failure?.let { reason ->
            Text(
                stringResource(reason.messageRes()),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
            )
        }

        if (connectionTest.passed) {
            Text(
                stringResource(R.string.model_service_test_success),
                style = MaterialTheme.typography.bodySmall,
                color = bitFunColors.statusSuccess,
            )
        }
    }
}

/** A field's name, and on the right whatever the field admits about itself. */
@Composable
private fun FieldLabel(text: String, trailing: String) {
    Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(
            text,
            style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
            color = MaterialTheme.colorScheme.onSurface,
        )
        Spacer(Modifier.weight(1f))
        if (trailing.isNotEmpty()) {
            Text(
                trailing,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/**
 * A filled box with no label of its own and no underline, which is how the
 * source's `TextInput` on `SOFT` reads. Material's floating label would put the
 * field's name in two places at once and move one of them while the user types.
 */
@Composable
private fun SoftTextField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    isError: Boolean,
    keyboardType: KeyboardType,
    password: Boolean,
    trailing: (@Composable () -> Unit)?,
    enabled: Boolean,
    modifier: Modifier,
) {
    TextField(
        value = value,
        onValueChange = onValueChange,
        placeholder = { Text(placeholder, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        trailingIcon = trailing,
        singleLine = true,
        enabled = enabled,
        isError = isError,
        shape = RoundedCornerShape(8.dp),
        visualTransformation = if (password) PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
        colors = TextFieldDefaults.colors(
            focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
            unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
            disabledContainerColor = MaterialTheme.colorScheme.surfaceVariant,
            focusedIndicatorColor = bitFunColors.transparent,
            unfocusedIndicatorColor = bitFunColors.transparent,
            disabledIndicatorColor = bitFunColors.transparent,
        ),
        modifier = modifier.fillMaxWidth().defaultMinSize(minHeight = 48.dp),
    )
}

internal fun GeneralChatConfigFailure.messageRes(): Int = when (this) {
    GeneralChatConfigFailure.INVALID_URL -> R.string.model_service_invalid_url
    GeneralChatConfigFailure.MODEL_REQUIRED -> R.string.model_service_model_required
    GeneralChatConfigFailure.API_KEY_REQUIRED -> R.string.model_service_api_key_required
    GeneralChatConfigFailure.SECURE_STORAGE -> R.string.model_service_secure_store_failed
}
