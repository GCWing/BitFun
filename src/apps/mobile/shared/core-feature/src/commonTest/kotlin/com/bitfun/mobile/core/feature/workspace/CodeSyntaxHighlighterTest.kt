package com.bitfun.mobile.core.feature.workspace

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class CodeSyntaxHighlighterTest {
    /**
     * The gutter is the one thing every preview gets, coloured or not: a
     * reference like `Foo.kt:80` is only useful if the reader can find line 80.
     */
    @Test
    fun everyLineIsNumberedAndTheGutterIsRightAligned() {
        val tokens = CodeSyntaxHighlighter.tokenize((1..11).joinToString("\n") { "x" }, "a.kt")

        val gutter = tokens.filter { it.kind == CodeSyntaxTokenKind.LINE_NUMBER }
        assertEquals((1..11).toList(), gutter.map { it.lineNumber })
        // Two digits at the widest, so line 1 is padded to line 11's column.
        assertEquals(" 1  ", gutter.first().text)
        assertEquals("11  ", gutter.last().text)
    }

    @Test
    fun aKeywordAStringANumberAndACommentAreEachTheirOwnKind() {
        val tokens = CodeSyntaxHighlighter.tokenize("""val n = 42 // note "q"""", "a.kt")

        assertEquals(CodeSyntaxTokenKind.KEYWORD, tokens.kindOf("val"))
        assertEquals(CodeSyntaxTokenKind.NUMBER, tokens.kindOf("42"))
        // The comment swallows the rest of the line, quotes and all — otherwise
        // an apostrophe in prose would open a string that never closes.
        assertEquals(CodeSyntaxTokenKind.COMMENT, tokens.kindOf("""// note "q""""))
    }

    /**
     * Without a parser, what follows a word is the whole of the context: this is
     * where `fetch(` becomes a call and `name:` becomes a key.
     */
    @Test
    fun aWordIsReadFromWhatFollowsIt() {
        val tokens = CodeSyntaxHighlighter.tokenize("Widget.fetch (name: value)", "a.ts")

        assertEquals(CodeSyntaxTokenKind.TYPE, tokens.kindOf("Widget"))
        assertEquals(CodeSyntaxTokenKind.FUNCTION, tokens.kindOf("fetch"))
        assertEquals(CodeSyntaxTokenKind.PROPERTY, tokens.kindOf("name"))
        assertEquals(CodeSyntaxTokenKind.PLAIN, tokens.kindOf("value"))
    }

    /** `def` is Python's and `fn` is Rust's; neither is the other's. */
    @Test
    fun theKeywordSetFollowsTheExtension() {
        assertEquals(CodeSyntaxTokenKind.KEYWORD, CodeSyntaxHighlighter.tokenize("def f(x)", "a.py").kindOf("def"))
        assertEquals(CodeSyntaxTokenKind.PLAIN, CodeSyntaxHighlighter.tokenize("def f(x)", "a.rs").kindOf("def"))
        assertEquals(CodeSyntaxTokenKind.KEYWORD, CodeSyntaxHighlighter.tokenize("fn f(x)", "a.rs").kindOf("fn"))
    }

    /** `#` is a comment in Python and a fragment marker in a stylesheet. */
    @Test
    fun aHashCommentsOnlyWhereItComments() {
        assertEquals(CodeSyntaxTokenKind.COMMENT, CodeSyntaxHighlighter.tokenize("# note", "a.py").kindOf("# note"))
        assertTrue(
            CodeSyntaxHighlighter.tokenize("#note", "a.css").none { it.kind == CodeSyntaxTokenKind.COMMENT },
            "a stylesheet id selector is not a comment",
        )
    }

    /**
     * The bug this port fixes: ArkTS stops emitting interleaved line numbers one
     * line early, so the last line of every multi-line literal — and the gutter
     * for the whole rest of the file — slides out of step with the source.
     */
    @Test
    fun aMultiLineLiteralKeepsANumberOnEveryLineItCovers() {
        val tokens = CodeSyntaxHighlighter.tokenize("val a = \"\"\"one\ntwo\n\nthree\"\"\"\nval b = 1", "a.kt")

        val gutter = tokens.filter { it.kind == CodeSyntaxTokenKind.LINE_NUMBER }
        assertEquals(listOf(1, 2, 3, 4, 5), gutter.map { it.lineNumber })
        // And the line after the literal is still numbered 5, not 3.
        assertEquals(5, tokens.first { it.text == "val" && it.lineNumber > 1 }.lineNumber)
    }

    /** A literal that ends on a newline hands the next number back to the main scan, once. */
    @Test
    fun aLiteralEndingOnANewlineIsNotNumberedTwice() {
        val tokens = CodeSyntaxHighlighter.tokenize("x = `a\nb\n`\ny = 1", "a.js")

        val gutter = tokens.filter { it.kind == CodeSyntaxTokenKind.LINE_NUMBER }
        assertEquals(listOf(1, 2, 3, 4), gutter.map { it.lineNumber })
    }

    /**
     * An unterminated quote must not swallow the file — except a backtick, which
     * is a template literal everywhere it exists and is meant to span lines.
     */
    @Test
    fun anUnterminatedQuoteStopsAtItsLine() {
        val tokens = CodeSyntaxHighlighter.tokenize("a = 'oops\nb = 1", "a.js")

        assertEquals(CodeSyntaxTokenKind.STRING, tokens.kindOf("'oops"))
        assertEquals(CodeSyntaxTokenKind.NUMBER, tokens.kindOf("1"))
    }

    /**
     * An unknown extension is not an error: the file is still worth reading, it
     * just has no language rules to read it by.
     */
    @Test
    fun anUnknownExtensionFallsBackToOnePlainNumberedBlock() {
        val tokens = CodeSyntaxHighlighter.tokenize("if x\nif y", "notes.made-up")

        assertEquals(1, tokens.size)
        assertEquals(CodeSyntaxTokenKind.PLAIN, tokens.single().kind)
        assertEquals("1  if x\n2  if y", tokens.single().text)
        assertEquals(0, tokens.single().lineNumber, "the fallback has no per-line information to offer")
    }

    /** The size cap is what keeps a generated file from becoming thousands of spans. */
    @Test
    fun anEnormousFileIsNotLexed() {
        val huge = "val x = 1\n".repeat(40_000)

        val tokens = CodeSyntaxHighlighter.tokenize(huge, "a.kt")

        assertEquals(1, tokens.size)
        assertEquals(CodeSyntaxTokenKind.PLAIN, tokens.single().kind)
    }

    /** Whatever the input, the runs must still spell the file back exactly. */
    @Test
    fun theTokensReassembleIntoTheSourcePlusItsGutter() {
        val source = "/* head\n   more */\nfun f() {\n  return \"a\\\"b\"\n}\n"

        val text = CodeSyntaxHighlighter.tokenize(source, "F.kt")
            .filter { it.kind != CodeSyntaxTokenKind.LINE_NUMBER }
            .joinToString("") { it.text }

        assertEquals(source, text)
    }

    private fun List<CodeSyntaxToken>.kindOf(text: String): CodeSyntaxTokenKind =
        first { it.text == text }.kind
}
