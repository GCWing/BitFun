package com.openbitfun.mobile.core.feature.markdown

import kotlin.test.Test
import kotlin.test.assertEquals

class MarkdownParserTest {
    @Test
    fun parsesCommonMarkdownBlocksIntoStableBlockTypes() {
        val blocks = MarkdownParser.parse(
            "# Title\n\n- first\n- **second**\n\n> quote\n\n```ts\nconst value = 1;\n```\n\n| A | B |\n| --- | --- |\n| 1 | 2 |",
        )

        assertEquals(5, blocks.size)
        assertEquals("heading", blocks[0].type)
        assertEquals(1, blocks[0].level)
        assertEquals("list", blocks[1].type)
        assertEquals(2, blocks[1].items.size)
        assertEquals("second", blocks[1].items[1].text)
        assertEquals("quote", blocks[2].type)
        assertEquals("code", blocks[3].type)
        assertEquals("ts", blocks[3].language)
        assertEquals("table", blocks[4].type)
    }

    @Test
    fun formatsInlineCodeEmphasisAndLinksWithoutControlText() {
        val blocks = MarkdownParser.parse(
            "Use `pnpm test`, **bold text**, and [docs](https://example.test).",
        )

        assertEquals(1, blocks.size)
        assertEquals("paragraph", blocks[0].type)
        assertEquals("Use pnpm test, bold text, and docs (https://example.test).", blocks[0].text)
        assertEquals(7, blocks[0].inlines.size)
        assertEquals("code", blocks[0].inlines[1].type)
        assertEquals("strong", blocks[0].inlines[3].type)
        assertEquals("link", blocks[0].inlines[5].type)
        assertEquals("https://example.test", blocks[0].inlines[5].url)
    }
}
