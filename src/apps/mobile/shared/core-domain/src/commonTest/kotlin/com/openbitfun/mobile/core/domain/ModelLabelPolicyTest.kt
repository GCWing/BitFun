package com.openbitfun.mobile.core.domain

import kotlin.test.Test
import kotlin.test.assertEquals

class ModelLabelPolicyTest {
    @Test
    fun theVendorPrefixIsNotPartOfTheName() {
        assertEquals("claude-opus-4", ModelLabelPolicy.cleanLabel("anthropic/claude-opus-4"))
        assertEquals("claude-opus-4", ModelLabelPolicy.cleanLabel("openbitfun:claude-opus-4"))
        assertEquals("gpt-5", ModelLabelPolicy.cleanLabel("  openai/gpt-5  "))
    }

    @Test
    fun aBareVendorNameIsNotAModelName() {
        // The desktop sends the vendor in `model_name` for some providers, and a
        // picker listing three rows all reading "Anthropic" is unusable.
        assertEquals(
            "claude-opus-4",
            ModelLabelPolicy.primaryLabel(
                id = "m-1",
                name = "claude-opus-4",
                modelName = "anthropic",
                fallback = "Model",
            ),
        )
    }

    @Test
    fun theIdCarriesTheRowWhenEveryNameIsAVendor() {
        assertEquals(
            "opus",
            ModelLabelPolicy.primaryLabel(
                id = "anthropic/opus",
                name = "anthropic",
                modelName = "",
                fallback = "Model",
            ),
        )
    }

    @Test
    fun anEmptyRecordFallsBackToTheAppsCopy() {
        assertEquals(
            "Model",
            ModelLabelPolicy.primaryLabel(id = "", name = "", modelName = "", fallback = "Model"),
        )
    }

    @Test
    fun theSecondLineNeverRepeatsTheFirst() {
        // provider + name, because neither is the primary line.
        assertEquals(
            "Anthropic · Opus",
            ModelLabelPolicy.secondaryLabel(
                id = "m-1",
                name = "Opus",
                modelName = "claude-opus-4",
                provider = "Anthropic",
                fallback = "Model",
            ),
        )
        // `name` is already the primary line, so only the provider is left.
        assertEquals(
            "Anthropic",
            ModelLabelPolicy.secondaryLabel(
                id = "m-1",
                name = "Opus",
                modelName = "",
                provider = "Anthropic",
                fallback = "Model",
            ),
        )
        // Nothing distinguishing is left; the raw id still separates two rows
        // that would otherwise read identically.
        assertEquals(
            "m-1",
            ModelLabelPolicy.secondaryLabel(
                id = "m-1",
                name = "Opus",
                modelName = "",
                provider = "",
                fallback = "Model",
            ),
        )
    }

    @Test
    fun everyKnownVendorIsRejectedAsALabel() {
        listOf("openbitfun", "Anthropic", "OPENAI", "google", "azure").forEach {
            assertEquals(false, ModelLabelPolicy.isSpecificLabel(it), it)
        }
        assertEquals(true, ModelLabelPolicy.isSpecificLabel("claude-opus-4"))
        assertEquals(false, ModelLabelPolicy.isSpecificLabel(""))
    }
}
