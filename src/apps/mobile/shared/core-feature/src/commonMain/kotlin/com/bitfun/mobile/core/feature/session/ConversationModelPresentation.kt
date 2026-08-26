package com.bitfun.mobile.core.feature.session

import com.bitfun.mobile.core.domain.ChatTimelineState
import com.bitfun.mobile.core.domain.ModelLabelPolicy
import com.bitfun.mobile.core.protocol.RemoteModelCatalog

/**
 * One row of the model picker.
 *
 * Plain strings rather than the protocol's `RemoteModelConfig`: the picker shows
 * two lines and sends an id back, and nothing above this seam should have to
 * know the wire shape. See the design doc section 2.
 */
public data class ModelOption public constructor(
    public val id: String,
    public val primaryLabel: String,
    public val secondaryLabel: String,
    public val selected: Boolean,
)

/**
 * The models this session may switch to, already labelled.
 *
 * Disabled models are dropped rather than greyed out — the desktop refuses them,
 * so showing them would only offer a control that cannot work. [fallbackLabel]
 * names a model whose every field is blank; the copy comes from the app.
 */
public fun ChatTimelineState.modelOptions(fallbackLabel: String): List<ModelOption> {
    val selectedId = selectedModelOptionId()
    return modelCatalog.models.filter { it.enabled }.map { model ->
        ModelOption(
            id = model.id,
            primaryLabel = ModelLabelPolicy.primaryLabel(
                model.id,
                model.name,
                model.modelName,
                fallbackLabel,
            ),
            secondaryLabel = ModelLabelPolicy.secondaryLabel(
                model.id,
                model.name,
                model.modelName,
                model.provider,
                fallbackLabel,
            ),
            selected = model.id == selectedId,
        )
    }
}

/** The option the composer's model chip names, or null when nothing is usable. */
public fun ChatTimelineState.selectedModelOption(fallbackLabel: String): ModelOption? =
    modelOptions(fallbackLabel).firstOrNull { it.selected }

/** Models available while creating a session, before a transcript exists. */
public fun RemoteSessionUiState.Ready.createModelOptions(fallbackLabel: String): List<ModelOption> {
    val catalog = modelCatalog ?: timeline?.modelCatalog ?: return emptyList()
    val selectedId = listOf(
        timeline?.selectedModelId,
        catalog.sessionModelId,
        catalog.defaultModels.primary,
    ).firstNotNullOfOrNull { candidate ->
        candidate?.takeIf { id -> catalog.models.any { model -> model.id == id && model.enabled } }
    }
    return catalog.presentationOptions(fallbackLabel, selectedId)
}

private fun RemoteModelCatalog.presentationOptions(
    fallbackLabel: String,
    selectedId: String?,
): List<ModelOption> = models.filter { it.enabled }.map { model ->
    ModelOption(
        id = model.id,
        primaryLabel = ModelLabelPolicy.primaryLabel(
            model.id,
            model.name,
            model.modelName,
            fallbackLabel,
        ),
        secondaryLabel = ModelLabelPolicy.secondaryLabel(
            model.id,
            model.name,
            model.modelName,
            model.provider,
            fallbackLabel,
        ),
        selected = model.id == selectedId,
    )
}

/**
 * Which model the desktop would actually use.
 *
 * The explicit selection wins, then the session's own model, then the account
 * default — each only if the catalog still has it *and* still has it enabled,
 * which is why this is a search rather than a read. Ported from
 * `ConversationModelPresentationPolicy.selectedModel`.
 */
private fun ChatTimelineState.selectedModelOptionId(): String? {
    val candidates = listOf(
        selectedModelId,
        modelCatalog.sessionModelId,
        modelCatalog.defaultModels.primary,
    )
    candidates.forEach { candidate ->
        if (candidate.isNullOrEmpty()) return@forEach
        val match = modelCatalog.models.firstOrNull { it.id == candidate && it.enabled }
        if (match != null) return match.id
    }
    return null
}
