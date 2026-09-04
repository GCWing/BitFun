package com.openbitfun.mobile.app.platform

import org.junit.Assert.assertEquals
import org.junit.Test

class AppLocaleResolverTest {
    @Test
    fun resolvesMissingOrUnknownLanguagesToEnglish() {
        listOf(null, "", "   ", "en", "fr").forEach { language ->
            assertEquals(AppLocale.ENGLISH, resolveAppLocale(language))
        }
    }

    @Test
    fun resolvesChineseLocaleTagsThroughTheirLanguageToken() {
        // Android exposes zh-CN and zh-Hans locale tags as the language token zh.
        listOf("zh-CN" to "zh", "zh-Hans" to "zh").forEach { (_, language) ->
            assertEquals(AppLocale.SIMPLIFIED_CHINESE, resolveAppLocale(language))
        }
    }
}
