package com.bitfun.mobile.app

import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.unit.dp
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

/**
 * Every glyph the app draws, drawn once.
 *
 * These are the HarmonyOS `sys.symbol` set redrawn by hand as vectors, and a
 * vector with a malformed `pathData` compiles, links, and only then throws — on
 * the one screen that happens to use it. Half of them live on surfaces that need
 * a paired desktop to reach, so inflating the whole set here is the only check
 * that does not depend on which screen someone remembered to open.
 */
class SymbolDrawableTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun everySymbolDrawableInflatesAndDraws() {
        val ids = symbolDrawableIds()
        // A guard on the guard: were the field scan to come back empty, the test
        // would pass while checking nothing at all.
        assertTrue("Expected the redrawn symbol set, found ${ids.size}", ids.size >= 40)

        composeRule.setContent {
            ids.forEach { (_, id) ->
                Icon(
                    painterResource(id),
                    contentDescription = null,
                    modifier = Modifier.size(24.dp),
                )
            }
        }
        composeRule.waitForIdle()
    }

    /** Every `R.drawable.ic_symbol_*`, read off the generated class. */
    private fun symbolDrawableIds(): List<Pair<String, Int>> {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        return R.drawable::class.java.fields
            .filter { it.name.startsWith("ic_symbol_") }
            .map { it.name to it.getInt(null) }
            .onEach { (name, id) ->
                assertTrue(name, context.resources.getResourceEntryName(id) == name)
            }
            .sortedBy { it.first }
    }
}
