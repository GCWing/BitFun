package com.bitfun.mobile.app.ui.chat

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.session.ChatComposerCapabilities
import com.bitfun.mobile.core.feature.session.ChatComposerPolicy
import com.bitfun.mobile.core.feature.session.ComposerImage
import com.bitfun.mobile.core.feature.session.ComposerPrimaryAction
import com.bitfun.mobile.core.feature.session.ModelOption

internal const val COMPOSER_TEST_TAG: String = "composer"
internal const val COMPOSER_INPUT_TEST_TAG: String = "composer-input"
internal const val COMPOSER_SEND_TEST_TAG: String = "composer-send"
internal const val MODEL_CONTROL_TEST_TAG: String = "composer-model-control"
internal const val MODEL_SELECTOR_TEST_TAG: String = "composer-model-selector"
internal const val MODEL_SELECTOR_OPTION_TEST_TAG_PREFIX: String = "composer-model-option:"

/** The relay refuses more than this, and refusing here is a better error. */
internal const val MAX_COMPOSER_IMAGES: Int = 4

// The measurements come straight from `ComposerBar.ets`, which sizes the bar in
// vp — the same unit as dp. Naming them keeps the two files diffable.
private val ActionSize = 40.dp
private val InputHeight = 42.dp
private val ExpandedInputHeight = 74.dp
private val CollapsedBarHeight = 52.dp
private val ExpandedInputRowHeight = 76.dp
private val ExpandedActionRowHeight = 44.dp

/**
 * The input bar, ported from `pages/components/ComposerBar.ets`.
 *
 * It is a floating pill rather than a docked band, and it has two forms: a
 * one-line row while the user is elsewhere, and a taller one — field on its own
 * row, controls beneath — once they are writing. Both are [ChatComposerPolicy]'s
 * call rather than this file's, along with which single action the round button
 * on the right is offering. The two clients must agree on all of it: a message
 * refused after the fact reads as a lost message.
 *
 * [capabilities] is what lets general chat reuse the bar unchanged: it has no
 * desktop to wait for and no attachment pipeline, and hiding those controls is
 * the difference between the two surfaces, not a second widget.
 *
 * There is no in-app listening state here, unlike HarmonyOS: Android dictation
 * is a system activity that draws its own listening UI on top of this one.
 */
@Composable
internal fun ComposerBar(
    draft: String,
    images: List<ComposerImage>,
    busy: Boolean,
    streaming: Boolean,
    phase: ConnectionPhase,
    model: ModelOption?,
    modelOptions: List<ModelOption> = emptyList(),
    capabilities: ChatComposerCapabilities,
    /**
     * What the empty field says. Every surface asks for something different —
     * a message into an open session, or the first instruction of a session
     * that does not exist yet — and the source gives each its own wording.
     */
    placeholder: String,
    onDraftChange: (String) -> Unit,
    onRemoveImage: (String) -> Unit,
    onAttach: () -> Unit,
    onVoice: () -> Unit,
    onSend: () -> Unit,
    onStop: () -> Unit,
    onOpenModels: () -> Unit,
    onSelectModel: (String) -> Unit = {},
    modifier: Modifier,
) {
    var modelSelectorOpen by remember { mutableStateOf(false) }
    var focused by remember { mutableStateOf(false) }
    val expanded = ChatComposerPolicy.isExpanded(
        text = draft,
        inputFocused = focused,
        // Android has no quick-action panel — the add button opens the system
        // photo picker directly — and the model list is a sheet that covers the
        // bar rather than a popover anchored to it.
        quickActionsOpen = false,
        modelSelectorOpen = modelSelectorOpen,
    )
    val action = ChatComposerPolicy.primaryAction(
        text = draft,
        attachmentCount = images.size,
        busy = busy,
        streaming = streaming,
        requiresRemoteConnection = capabilities.requiresRemoteConnection,
        phase = phase,
        showVoiceInput = capabilities.showVoiceInput,
    )

    Column(
        verticalArrangement = Arrangement.spacedBy(8.dp),
        modifier = modifier
            .fillMaxWidth()
            .padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 14.dp)
            .testTag(COMPOSER_TEST_TAG),
    ) {
        if (images.isNotEmpty()) {
            AttachmentStrip(images = images, enabled = !busy, onRemove = onRemoveImage)
        }

        Surface(
            shape = RoundedCornerShape(if (expanded) 18.dp else 26.dp),
            color = MaterialTheme.colorScheme.surface,
            shadowElevation = 2.dp,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(
                    PaddingValues(
                        start = 8.dp,
                        end = 8.dp,
                        top = if (expanded) 4.dp else 0.dp,
                        bottom = if (expanded) 2.dp else 0.dp,
                    ),
                ),
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(if (expanded) 0.dp else 5.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(if (expanded) ExpandedInputRowHeight else CollapsedBarHeight),
                ) {
                    // While expanded both side controls move to the row below,
                    // so the field gets the full width for what is being typed.
                    if (!expanded && capabilities.showAddButton) {
                        AddButton(enabled = !busy && images.size < MAX_COMPOSER_IMAGES, onClick = onAttach)
                    }
                    ComposerField(
                        draft = draft,
                        enabled = !busy,
                        expanded = expanded,
                        placeholder = placeholder,
                        onDraftChange = onDraftChange,
                        onFocusChange = { focused = it },
                        modifier = Modifier.weight(1f),
                    )
                    if (!expanded) {
                        PrimaryActionButton(
                            action = action,
                            onVoice = onVoice,
                            onSend = onSend,
                            onStop = onStop,
                        )
                    }
                }

                if (expanded) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(ExpandedActionRowHeight)
                            .padding(start = 2.dp),
                    ) {
                        if (capabilities.showAddButton) {
                            AddButton(
                                enabled = !busy && images.size < MAX_COMPOSER_IMAGES,
                                onClick = onAttach,
                            )
                        }
                        // The model belongs to the session, but it is chosen
                        // here: it is the one setting a user changes mid-turn.
                        if (model != null) {
                            ModelControl(
                                model = model,
                                enabled = !busy,
                                modelOptions = modelOptions,
                                onClick = {
                                    if (modelOptions.isEmpty()) onOpenModels() else modelSelectorOpen = true
                                },
                                onSelectModel = { id ->
                                    onSelectModel(id)
                                    modelSelectorOpen = false
                                },
                                selectorOpen = modelSelectorOpen,
                                onSelectorDismiss = { modelSelectorOpen = false },
                            )
                        }
                        Box(modifier = Modifier.weight(1f))
                        PrimaryActionButton(
                            action = action,
                            onVoice = onVoice,
                            onSend = onSend,
                            onStop = onStop,
                        )
                    }
                }
            }
        }
    }
}

/**
 * The field itself: a bare text field on the pill's own background.
 *
 * A Material `OutlinedTextField` would draw a second border and a floating label
 * inside a shape that is already the input, which is why the placeholder is
 * hand-drawn here.
 */
@Composable
private fun ComposerField(
    draft: String,
    enabled: Boolean,
    expanded: Boolean,
    placeholder: String,
    onDraftChange: (String) -> Unit,
    onFocusChange: (Boolean) -> Unit,
    modifier: Modifier,
) {
    val colors = MaterialTheme.colorScheme
    BasicTextField(
        value = draft,
        onValueChange = onDraftChange,
        enabled = enabled,
        // Collapsed the bar is one line tall, so the field must not grow past
        // it; expanded it stops at four and scrolls, as the other client does.
        maxLines = if (expanded) 4 else 1,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = colors.onSurface),
        cursorBrush = SolidColor(colors.onSurface),
        modifier = modifier
            .heightIn(min = if (expanded) ExpandedInputHeight else InputHeight)
            .onFocusChanged { onFocusChange(it.isFocused) }
            .testTag(COMPOSER_INPUT_TEST_TAG),
        decorationBox = { field ->
            Box(
                contentAlignment = Alignment.CenterStart,
                modifier = Modifier.padding(
                    start = 4.dp,
                    end = 4.dp,
                    top = if (expanded) 10.dp else 9.dp,
                    bottom = if (expanded) 8.dp else 9.dp,
                ),
            ) {
                if (draft.isEmpty()) {
                    Text(
                        placeholder,
                        style = MaterialTheme.typography.bodyLarge,
                        color = colors.onSurfaceVariant,
                    )
                }
                field()
            }
        },
    )
}

/** The attachment control: a plain glyph, sized to match the primary action. */
@Composable
private fun AddButton(enabled: Boolean, onClick: () -> Unit) {
    Box(
        contentAlignment = Alignment.Center,
        modifier = Modifier
            .size(ActionSize)
            .clip(CircleShape)
            .clickable(enabled = enabled, onClick = onClick),
    ) {
        Icon(
            painterResource(R.drawable.ic_symbol_plus),
            contentDescription = stringResource(R.string.message_attach_image),
            tint = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.size(22.dp).alpha(if (enabled) 1f else DimmedAlpha),
        )
    }
}

/**
 * The current model, as a label the user can press rather than a chip.
 *
 * It only exists in the expanded form, matching the other client: a model swap
 * is something you do while writing the message it applies to.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ModelControl(
    model: ModelOption,
    enabled: Boolean,
    modelOptions: List<ModelOption>,
    onClick: () -> Unit,
    onSelectModel: (String) -> Unit,
    selectorOpen: Boolean,
    onSelectorDismiss: () -> Unit,
) {
    val label = stringResource(R.string.models_title)
    val wide = LocalConfiguration.current.screenWidthDp >= 600
    Box {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(3.dp),
            modifier = Modifier
                .height(34.dp)
                .widthIn(max = 220.dp)
                .clip(RoundedCornerShape(9.dp))
                .clickable(enabled = enabled, onClick = onClick)
                .padding(horizontal = 4.dp)
                .testTag(MODEL_CONTROL_TEST_TAG)
                .semantics { contentDescription = "$label · ${model.primaryLabel}" },
        ) {
            Text(
                model.primaryLabel,
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Icon(
                painterResource(
                    if (selectorOpen) R.drawable.ic_symbol_chevron_up
                    else R.drawable.ic_symbol_chevron_down,
                ),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.size(14.dp).alpha(0.68f),
            )
        }
        if (wide) {
            DropdownMenu(expanded = selectorOpen, onDismissRequest = onSelectorDismiss) {
                ModelSelectorContent(
                    options = modelOptions,
                    onSelect = onSelectModel,
                    compact = true,
                )
            }
        }
    }
    if (!wide && selectorOpen) {
        ModalBottomSheet(
            onDismissRequest = onSelectorDismiss,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            containerColor = MaterialTheme.colorScheme.surface,
        ) {
            ModelSelectorContent(options = modelOptions, onSelect = onSelectModel, compact = false)
        }
    }
}

@Composable
private fun ModelSelectorContent(
    options: List<ModelOption>,
    onSelect: (String) -> Unit,
    compact: Boolean,
) {
    Column(
        modifier = Modifier
            .then(if (compact) Modifier.width(300.dp) else Modifier.fillMaxWidth())
            .padding(vertical = if (compact) 4.dp else 12.dp)
            .testTag(MODEL_SELECTOR_TEST_TAG),
    ) {
        Text(
            stringResource(R.string.model_selector_title),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(horizontal = 18.dp, vertical = 10.dp),
        )
        if (options.isEmpty()) {
            Text(
                stringResource(R.string.model_selector_empty),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 18.dp, vertical = 14.dp),
            )
        } else {
            options.forEachIndexed { index, option ->
                DropdownMenuItem(
                    modifier = Modifier.testTag(MODEL_SELECTOR_OPTION_TEST_TAG_PREFIX + option.id),
                    text = {
                        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            Text(option.primaryLabel, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(
                                option.secondaryLabel,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    },
                    leadingIcon = {
                        Icon(
                            painterResource(
                                if (option.secondaryLabel == stringResource(R.string.model_selector_local)) {
                                    R.drawable.ic_symbol_gearshape
                                } else {
                                    R.drawable.ic_symbol_cloud
                                },
                            ),
                            contentDescription = null,
                            modifier = Modifier.size(19.dp),
                        )
                    },
                    trailingIcon = if (option.selected) {
                        {
                            Icon(
                                painterResource(R.drawable.ic_symbol_checkmark_circle_fill),
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.primary,
                                modifier = Modifier.size(19.dp),
                            )
                        }
                    } else null,
                    onClick = { onSelect(option.id) },
                )
                if (!compact && index != options.lastIndex) HorizontalDivider()
            }
        }
    }
}

/** What a control that is offered but not usable right now looks like. */
private const val DimmedAlpha: Float = 0.38f

/**
 * One slot, never two actions.
 *
 * Which action it holds is [ChatComposerPolicy.primaryAction]'s decision; this
 * only draws it. Stop is the one state with a filled background, because it is
 * the one state where pressing the button undoes something.
 */
@Composable
private fun PrimaryActionButton(
    action: ComposerPrimaryAction,
    onVoice: () -> Unit,
    onSend: () -> Unit,
    onStop: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val enabled = action == ComposerPrimaryAction.STOP ||
        action == ComposerPrimaryAction.SEND ||
        action == ComposerPrimaryAction.VOICE
    val description = when (action) {
        ComposerPrimaryAction.STOP -> R.string.message_stop
        ComposerPrimaryAction.VOICE, ComposerPrimaryAction.VOICE_BLOCKED ->
            R.string.message_voice_input
        else -> R.string.message_send
    }

    Box(
        contentAlignment = Alignment.Center,
        modifier = Modifier
            .size(ActionSize)
            .clip(CircleShape)
            .background(if (action == ComposerPrimaryAction.STOP) colors.error else Color.Transparent)
            .clickable(enabled = enabled) {
                when (action) {
                    ComposerPrimaryAction.STOP -> onStop()
                    ComposerPrimaryAction.SEND -> onSend()
                    ComposerPrimaryAction.VOICE -> onVoice()
                    else -> Unit
                }
            }
            .testTag(COMPOSER_SEND_TEST_TAG),
    ) {
        when (action) {
            // A square on the red disc, not a glyph: it reads as "halt" at this
            // size where a stop icon reads as a smudge.
            ComposerPrimaryAction.STOP -> Box(
                modifier = Modifier
                    .size(13.dp)
                    .clip(RoundedCornerShape(3.dp))
                    .background(colors.onError),
            )

            ComposerPrimaryAction.VOICE, ComposerPrimaryAction.VOICE_BLOCKED -> Icon(
                painterResource(R.drawable.ic_symbol_mic),
                contentDescription = stringResource(description),
                tint = colors.onSurface,
                modifier = Modifier.size(22.dp).alpha(
                    if (action == ComposerPrimaryAction.VOICE) 1f else DimmedAlpha,
                ),
            )

            else -> Icon(
                painterResource(R.drawable.ic_symbol_arrow_up),
                contentDescription = stringResource(description),
                tint = colors.onSurface,
                modifier = Modifier.size(23.dp).alpha(
                    if (action == ComposerPrimaryAction.SEND) 1f else DimmedAlpha,
                ),
            )
        }
    }
}

/**
 * The pending attachments, each with its own remove control.
 *
 * A count alone was not enough: the picker can only add, so without a per-image
 * remove the only way to drop a wrong photo was to send it.
 */
@Composable
private fun AttachmentStrip(
    images: List<ComposerImage>,
    enabled: Boolean,
    onRemove: (String) -> Unit,
) {
    LazyRow(
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        contentPadding = PaddingValues(end = 4.dp),
    ) {
        items(images, key = { it.id }) { image ->
            Box(modifier = Modifier.size(64.dp)) {
                val bitmap = remember(image.dataUrl) { decodeInlineImage(image.dataUrl) }
                if (bitmap != null) {
                    Image(
                        bitmap = bitmap.asImageBitmap(),
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.size(64.dp).clip(RoundedCornerShape(14.dp)),
                    )
                } else {
                    Surface(
                        color = MaterialTheme.colorScheme.surfaceVariant,
                        shape = RoundedCornerShape(14.dp),
                        modifier = Modifier.size(64.dp),
                    ) {
                        Text(
                            stringResource(R.string.chat_image),
                            style = MaterialTheme.typography.labelSmall,
                            modifier = Modifier.padding(4.dp),
                        )
                    }
                }
                // The badge carries its own scrim rather than borrowing the
                // theme's: it sits on whatever the photo happens to be, and a
                // pale photo would swallow a surface-coloured control.
                val removeLabel = stringResource(R.string.message_remove_image)
                Box(
                    contentAlignment = Alignment.Center,
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(4.dp)
                        .size(22.dp)
                        .clip(CircleShape)
                        .background(BadgeScrim)
                        .clickable(enabled = enabled) { onRemove(image.id) }
                        .semantics { contentDescription = removeLabel },
                ) {
                    Text(
                        "×",
                        style = MaterialTheme.typography.labelMedium,
                        color = Color.White,
                    )
                }
            }
        }
    }
}

/** `#AA222222` from the other client — a scrim, so it is not a theme role. */
private val BadgeScrim = Color(0xAA222222)
