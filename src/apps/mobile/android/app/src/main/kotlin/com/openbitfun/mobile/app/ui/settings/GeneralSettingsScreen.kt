package com.openbitfun.mobile.app.ui.settings

import androidx.annotation.DrawableRes
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.platform.AppLocale
import com.openbitfun.mobile.app.platform.AppLocaleController
import com.openbitfun.mobile.app.ui.theme.generated.MobileDesignGeometry
import com.openbitfun.mobile.core.feature.generalchat.GeneralChatConfigFailure
import com.openbitfun.mobile.core.feature.generalchat.GeneralChatConfigUi
import com.openbitfun.mobile.core.feature.generalchat.GeneralChatConnectionTestUi
import com.openbitfun.mobile.core.feature.generalchat.GeneralChatIntent
import com.openbitfun.mobile.core.feature.generalchat.GeneralChatModelUi

internal const val GENERAL_SETTINGS_TEST_TAG: String = "general-settings"
internal const val GENERAL_SETTINGS_PROFILE_TEST_TAG: String = "general-settings-profile"
internal const val GENERAL_SETTINGS_MODEL_TEST_TAG: String = "general-settings-model"
internal const val GENERAL_SETTINGS_CLOSE_TEST_TAG: String = "general-settings-close"

/**
 * The app's own settings page, ported from `pages/components/SettingsSheet.ets`.
 *
 * The counterpart of [SettingsScreen]. The source's sidebar gear always opens
 * this root page; remote-control settings is reached by its own remote action.
 * This page is about the phone — who is signed in, which model the app talks to
 * on its own, and what build this is — and mentions no desktop anywhere.
 *
 * Its chrome is deliberately not the remote page's. The source rounds these cards
 * at 8 rather than 24 and left-aligns the title rather than centring it: this page
 * is a list of settings, and that page is a report on one connection.
 *
 * @param accountUsername what the row says underneath "Profile", falling back to
 * whether anyone is signed in at all when the session has no name to give —
 * `this.accountUsername || (this.authenticatedUserId.length > 0 ? … : …)`.
 * @param accountUserId only whether it is blank, which is that fallback's
 * question. The account surface behind the row loads its own store.
 * @param config the general-chat provider, shown as the model row's value and
 * edited in the panel the row opens.
 * @param connectionTest belongs to that panel rather than to this page, and is
 * threaded through because the panel is drawn over this one, as does
 * [onSaveConfig] — whose Boolean is the panel's own "was that accepted".
 */
@Composable
internal fun GeneralSettingsScreen(
    modifier: Modifier,
    accountUserId: String?,
    accountUsername: String,
    config: GeneralChatConfigUi,
    models: List<GeneralChatModelUi>,
    activeModelId: String,
    configFailure: GeneralChatConfigFailure?,
    connectionTest: GeneralChatConnectionTestUi,
    onChatIntent: (GeneralChatIntent) -> Unit,
    onSaveConfig: (GeneralChatIntent.SaveConfig) -> Boolean,
    onOpenAccount: () -> Unit,
    onClose: () -> Unit,
) {
    // The provider editor covers this page rather than opening beside it, the way
    // `if (this.showModelService) { this.ModelServicePanel() }` stacks it over the
    // settings column. A second bottom sheet on top of this one would be a sheet
    // over a sheet, which Compose will draw and no phone can make sense of.
    var showModelService by rememberSaveable { mutableStateOf(false) }
    var showLanguagePicker by rememberSaveable { mutableStateOf(false) }
    val context = LocalContext.current
    val selectedLocale = AppLocaleController.current(LocalConfiguration.current)

    BackHandler(enabled = showLanguagePicker || showModelService) {
        showLanguagePicker = false
        showModelService = false
    }

    Box(modifier = modifier.fillMaxSize().testTag(GENERAL_SETTINGS_TEST_TAG)) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                // The title starts below the close button rather than beside it,
                // which is the source's `top: 64` over a floating `CloseButton()`:
                // a 28sp heading and a 44dp circle on one line read as a top bar,
                // and this page is not one — nothing here goes back anywhere.
                .padding(start = 16.dp, end = 16.dp, top = 64.dp, bottom = 34.dp),
        ) {
            Text(
                stringResource(R.string.settings_title),
                fontSize = 28.sp,
                lineHeight = 34.sp,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.fillMaxWidth().padding(bottom = 30.dp),
            )

            SettingsCard(
                modifier = Modifier.testTag(GENERAL_SETTINGS_PROFILE_TEST_TAG),
                radius = MobileDesignGeometry.SettingsCompactCardRadius,
                bordered = false,
            ) {
                AccountEntryRow(
                    subtitle = accountUsername.ifBlank {
                        if (accountUserId.isNullOrBlank()) {
                            stringResource(R.string.settings_account_signed_out)
                        } else {
                            stringResource(R.string.remote_settings_account_signed_in)
                        }
                    },
                    onClick = onOpenAccount,
                )
            }

            GeneralSectionTitle(stringResource(R.string.settings_general_section))
            SettingsCard(
                modifier = Modifier,
                radius = MobileDesignGeometry.SettingsCompactCardRadius,
                bordered = false,
            ) {
                Column(modifier = Modifier.padding(vertical = 5.dp)) {
                    LanguageSettingsRow(
                        value = when (selectedLocale) {
                            AppLocale.ENGLISH -> stringResource(R.string.settings_language_english)
                            AppLocale.SIMPLIFIED_CHINESE -> stringResource(R.string.settings_language_chinese)
                        },
                        onClick = { showLanguagePicker = true },
                    )
                    HorizontalDivider(
                        modifier = Modifier.fillMaxWidth(0.84f).align(Alignment.CenterHorizontally),
                    )
                    GeneralSettingsRow(
                        icon = R.drawable.ic_symbol_square_grid_2x2,
                        title = stringResource(R.string.model_service_title),
                        // `modelServiceStatus()`: the model that would answer, which
                        // is not the local form's model name — with no local model
                        // configured, an account model is still an answer, and the
                        // row would otherwise read "not configured" beside a chat
                        // that works.
                        value = models.firstOrNull { it.id == activeModelId }?.label
                            ?: stringResource(R.string.model_service_not_configured),
                        onClick = { showModelService = true },
                        modifier = Modifier.testTag(GENERAL_SETTINGS_MODEL_TEST_TAG),
                    )
                }
            }

            GeneralSectionTitle(stringResource(R.string.settings_about_section))
            SettingsCard(
                modifier = Modifier,
                radius = MobileDesignGeometry.SettingsCompactCardRadius,
                bordered = false,
            ) {
                Column(modifier = Modifier.padding(vertical = 5.dp)) {
                    StaticSettingsRow(
                        title = stringResource(R.string.settings_about_product),
                        value = stringResource(R.string.settings_about_product_value),
                    )
                    // Short of the card's width and centred, as the source's
                    // 84%-wide rule is: a divider that reached the corners would
                    // read as two cards rather than as two rows of one.
                    HorizontalDivider(
                        modifier = Modifier.fillMaxWidth(0.84f).align(Alignment.CenterHorizontally),
                    )
                    StaticSettingsRow(
                        title = stringResource(R.string.settings_about_version),
                        value = appVersionName(),
                    )
                }
            }
        }

        // The source's `CloseButton()`, in the same corner on both settings pages.
        Surface(
            onClick = onClose,
            shape = androidx.compose.foundation.shape.CircleShape,
            color = MaterialTheme.colorScheme.surface,
            shadowElevation = 3.dp,
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(top = 22.dp, end = 18.dp)
                .size(50.dp)
                .testTag(GENERAL_SETTINGS_CLOSE_TEST_TAG),
        ) {
            Box(contentAlignment = Alignment.Center) {
                Icon(
                    painterResource(R.drawable.ic_symbol_xmark),
                    contentDescription = stringResource(R.string.common_close),
                    modifier = Modifier.size(21.dp),
                )
            }
        }

        if (showLanguagePicker) {
            LanguagePickerOverlay(
                selected = selectedLocale,
                onDismiss = { showLanguagePicker = false },
                onSelect = { locale ->
                    showLanguagePicker = false
                    AppLocaleController.set(context, locale)
                },
            )
        }

        if (showModelService) {
            // Opaque and full-bleed rather than a card floating on the settings
            // column: it is the only thing to interact with while it is up, and
            // letting the rows behind it show through would invite a tap that
            // lands on a page it is covering. Its own header carries the way out,
            // so this page adds no chrome of its own.
            ModelServiceScreen(
                config = config,
                models = models,
                activeModelId = activeModelId,
                failure = configFailure,
                connectionTest = connectionTest,
                onIntent = onChatIntent,
                onSave = onSaveConfig,
                onClose = { showModelService = false },
                modifier = Modifier
                    .fillMaxSize()
                    .background(MaterialTheme.colorScheme.background),
            )
        }
    }
}

@Composable
private fun LanguageSettingsRow(value: String, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .defaultMinSize(minHeight = 52.dp)
            .padding(horizontal = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(
            "Aa",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontSize = 20.sp,
            modifier = Modifier.widthIn(min = 23.dp),
        )
        Text(
            stringResource(R.string.settings_language_title),
            style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
        )
        Spacer(Modifier.weight(1f))
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.widthIn(max = 130.dp),
        )
        SettingsChevron()
    }
}

@Composable
private fun LanguagePickerOverlay(
    selected: AppLocale,
    onDismiss: () -> Unit,
    onSelect: (AppLocale) -> Unit,
) {
    Surface(
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(
            topStart = MobileDesignGeometry.SelectionTopRadius,
            topEnd = MobileDesignGeometry.SelectionTopRadius,
        ),
        modifier = Modifier.fillMaxSize(),
    ) {
        Column(Modifier.fillMaxSize()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(MobileDesignGeometry.SheetHeaderHeight)
                    .padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    stringResource(R.string.settings_language_choose),
                    fontSize = 18.sp,
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Spacer(Modifier.weight(1f))
                Surface(
                    onClick = onDismiss,
                    shape = androidx.compose.foundation.shape.CircleShape,
                    color = MaterialTheme.colorScheme.surface,
                    modifier = Modifier.size(MobileDesignGeometry.SelectionCloseSize),
                ) {
                    Box(contentAlignment = Alignment.Center) {
                        Icon(
                            painterResource(R.drawable.ic_symbol_xmark),
                            contentDescription = stringResource(R.string.common_close),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
            HorizontalDivider()
            Column(Modifier.padding(top = 8.dp, bottom = 28.dp)) {
                LanguageChoiceRow(
                    label = stringResource(R.string.settings_language_chinese),
                    selected = selected == AppLocale.SIMPLIFIED_CHINESE,
                    onClick = { onSelect(AppLocale.SIMPLIFIED_CHINESE) },
                )
                LanguageChoiceRow(
                    label = stringResource(R.string.settings_language_english),
                    selected = selected == AppLocale.ENGLISH,
                    onClick = { onSelect(AppLocale.ENGLISH) },
                )
            }
        }
    }
}

@Composable
private fun LanguageChoiceRow(label: String, selected: Boolean, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(MobileDesignGeometry.SelectionRowHeight)
            .clickable(onClick = onClick)
            .padding(horizontal = 22.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            label,
            fontSize = 16.sp,
            fontWeight = FontWeight.Medium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        if (selected) {
            Icon(
                painterResource(R.drawable.ic_symbol_list_checkmark),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

/** The 18sp Bold MUTED heading, indented onto the card's own text column. */
@Composable
private fun GeneralSectionTitle(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.fillMaxWidth().padding(start = 12.dp, top = 24.dp, bottom = 8.dp),
    )
}

/**
 * The account row: avatar, "Profile", and who that is underneath.
 *
 * The subtitle is what makes this row different from the remote page's, which
 * carries none — there the account is one item among a desktop's details, here it
 * is the first thing the page says about the phone.
 */
@Composable
private fun AccountEntryRow(subtitle: String, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .defaultMinSize(minHeight = 64.dp)
            .padding(horizontal = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        SettingsAvatar(icon = R.drawable.ic_symbol_person, diameter = 34)
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                stringResource(R.string.remote_settings_profile),
                style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
            )
            Text(
                subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        SettingsChevron()
    }
}

/** A row that opens something, with the current answer beside the chevron. */
@Composable
private fun GeneralSettingsRow(
    @DrawableRes icon: Int,
    title: String,
    value: String,
    onClick: () -> Unit,
    modifier: Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .defaultMinSize(minHeight = 52.dp)
            .padding(horizontal = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Icon(
            painterResource(icon),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(23.dp),
        )
        Text(title, style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium))
        Spacer(Modifier.weight(1f))
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            // Capped so a long model id ellipsises rather than pushing the title
            // off its left edge, which is what `.constraintSize({ maxWidth: 130 })`
            // is protecting in the source.
            modifier = Modifier.widthIn(max = 130.dp),
        )
        SettingsChevron()
    }
}

/** A row that only reports: no chevron, nothing to tap. */
@Composable
private fun StaticSettingsRow(title: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .defaultMinSize(minHeight = 52.dp)
            .padding(horizontal = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(title, style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium))
        Spacer(Modifier.weight(1f))
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun SettingsChevron() {
    Icon(
        painterResource(R.drawable.ic_symbol_chevron_right),
        contentDescription = null,
        tint = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.size(16.dp),
    )
}

/**
 * The version the user is actually running, asked of the package rather than
 * written into a string: a hardcoded number is right exactly once.
 */
@Composable
private fun appVersionName(): String {
    val context = LocalContext.current
    val unknown = stringResource(R.string.common_unknown)
    return remember(context) {
        runCatching {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName
        }.getOrNull().orEmpty().ifBlank { unknown }
    }
}
