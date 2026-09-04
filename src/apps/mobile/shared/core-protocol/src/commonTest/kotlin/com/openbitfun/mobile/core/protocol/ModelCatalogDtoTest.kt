package com.openbitfun.mobile.core.protocol

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlinx.serialization.json.JsonPrimitive

class ModelCatalogDtoTest {
    @Test
    fun oldShapeCatalogUsesSafeDefaultsForMissingFields() {
        val decoded = RelayJson.decodeFromString<RemoteModelCatalog>(
            """{"models":[{"id":"legacy-id","name":"Legacy Model"}]}""",
        )

        assertEquals(0L, decoded.version)
        assertEquals(1, decoded.models.size)
        assertEquals(
            RemoteModelConfig(
                id = "legacy-id",
                name = "Legacy Model",
            ),
            decoded.models.single(),
        )
    }

    @Test
    fun fullCurrentShapeCatalogPreservesAllValues() {
        val decoded = RelayJson.decodeFromString<RemoteModelCatalog>(
            """
            {
              "version": 1700000000123,
              "models": [
                {
                  "id": "cloud-main",
                  "name": "Cloud Main",
                  "provider": "cloud",
                  "base_url": "https://models.example.test/v1",
                  "model_name": "main-v2",
                  "context_window": 128000,
                  "enabled": true,
                  "capabilities": ["chat", "vision"],
                  "reasoning": {
                    "status": "ready",
                    "default_preset": "balanced",
                    "presets": [
                      {
                        "id": "balanced",
                        "label": "Balanced",
                        "order": 1,
                        "actions": [
                          {"type": "set_effort", "value": "medium", "enabled": true}
                        ],
                        "source": "catalog"
                      }
                    ]
                  }
                }
              ],
              "default_models": {
                "primary": "cloud-main",
                "fast": "cloud-fast",
                "search": "cloud-search",
                "image_understanding": "cloud-vision"
              },
              "session_model_id": "cloud-main"
            }
            """.trimIndent(),
        )

        val expected = RemoteModelCatalog(
            version = 1700000000123L,
            models = listOf(
                RemoteModelConfig(
                    id = "cloud-main",
                    name = "Cloud Main",
                    provider = "cloud",
                    baseUrl = "https://models.example.test/v1",
                    modelName = "main-v2",
                    contextWindow = 128000,
                    enabled = true,
                    capabilities = listOf("chat", "vision"),
                    reasoning = RemoteReasoningCatalogProjection(
                        status = "ready",
                        defaultPreset = "balanced",
                        presets = listOf(
                            RemoteReasoningPresetDescriptor(
                                id = "balanced",
                                label = "Balanced",
                                order = 1,
                                actions = listOf(
                                    RemoteReasoningPresetAction(
                                        type = "set_effort",
                                        value = JsonPrimitive("medium"),
                                        enabled = true,
                                    ),
                                ),
                                source = "catalog",
                            ),
                        ),
                    ),
                ),
            ),
            defaultModels = RemoteDefaultModels(
                primary = "cloud-main",
                fast = "cloud-fast",
                search = "cloud-search",
                imageUnderstanding = "cloud-vision",
            ),
            sessionModelId = "cloud-main",
        )

        assertEquals(expected, decoded)
    }

    @Test
    fun missingOrEmptyModelsDecodeToAnEmptyList() {
        val empty = RelayJson.decodeFromString<RemoteModelCatalog>(
            """{"version":1,"models":[]}""",
        )
        val absent = RelayJson.decodeFromString<RemoteModelCatalog>(
            """{"version":1}""",
        )

        assertEquals(emptyList(), empty.models)
        assertEquals(emptyList(), absent.models)
    }
}
