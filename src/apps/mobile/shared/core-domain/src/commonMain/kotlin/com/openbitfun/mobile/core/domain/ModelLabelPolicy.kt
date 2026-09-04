package com.openbitfun.mobile.core.domain

/**
 * Turns a desktop model record into the two lines a picker row shows.
 *
 * Ported from `pages/policy/ConversationModelPresentationPolicy.ets`. The
 * desktop names a model three different ways and any of them can be a bare
 * vendor name (`anthropic`, `openbitfun/claude-opus-4`, …), so both lines are
 * chosen rather than read: the first field that survives cleaning *and* says
 * something specific wins.
 *
 * The caller supplies [fallback] because the last resort is UI copy, and copy
 * lives in each app's resources — see the design doc section 4.3.
 */
public object ModelLabelPolicy {
    public fun primaryLabel(
        id: String,
        name: String,
        modelName: String,
        fallback: String,
    ): String {
        val cleanedModelName = cleanLabel(modelName)
        if (isSpecificLabel(cleanedModelName)) return cleanedModelName
        val cleanedName = cleanLabel(name)
        if (isSpecificLabel(cleanedName)) return cleanedName
        val cleanedId = cleanLabel(id)
        return cleanedId.ifEmpty { fallback }
    }

    /**
     * The supporting line, which must not repeat [primaryLabel].
     *
     * Falls back to the raw id: it is ugly but it is never the primary line, so
     * an id-titled row still tells the user which of two similar models this is.
     */
    public fun secondaryLabel(
        id: String,
        name: String,
        modelName: String,
        provider: String,
        fallback: String,
    ): String {
        val cleanedProvider = cleanLabel(provider)
        val cleanedName = cleanLabel(name)
        val primary = primaryLabel(id, name, modelName, fallback)
        if (
            cleanedProvider.isNotEmpty() &&
            cleanedName.isNotEmpty() &&
            cleanedName != primary &&
            cleanedName != cleanedProvider
        ) {
            return "$cleanedProvider · $cleanedName"
        }
        if (cleanedProvider.isNotEmpty() && cleanedProvider != primary) return cleanedProvider
        if (cleanedName.isNotEmpty() && cleanedName != primary) return cleanedName
        return id.ifEmpty { primary }
    }

    /** Drops a vendor prefix and keeps the last `/`- or `:`-separated segment. */
    public fun cleanLabel(value: String): String {
        val trimmed = value.trim()
        if (trimmed.isEmpty()) return ""
        val withoutScheme = trimmed
            .replaceFirst(OPENBITFUN_PREFIX, "")
            .replaceFirst(ANTHROPIC_PREFIX, "")
        val parts = withoutScheme.split('/', ':').filter(String::isNotEmpty)
        return parts.lastOrNull() ?: withoutScheme
    }

    /** Whether a cleaned label names a model rather than whoever ships it. */
    public fun isSpecificLabel(label: String): Boolean =
        label.isNotEmpty() && label.lowercase() !in VENDOR_NAMES

    private val OPENBITFUN_PREFIX = Regex("^openbitfun[:/_-]+", RegexOption.IGNORE_CASE)
    private val ANTHROPIC_PREFIX = Regex("^anthropic[:/_-]+", RegexOption.IGNORE_CASE)
    private val VENDOR_NAMES = setOf("openbitfun", "anthropic", "openai", "google", "azure")
}
