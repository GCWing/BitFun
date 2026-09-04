package com.openbitfun.mobile.core.feature.workspace

/**
 * What a run of preview text is, for the purpose of colouring it.
 *
 * Ported from `services/CodeSyntaxHighlighter.ets`. The kinds are an enum rather
 * than colours because a colour is a theme's business: `core-feature` says
 * *keyword*, and each app decides what a keyword looks like.
 */
public enum class CodeSyntaxTokenKind {
    PLAIN,
    LINE_NUMBER,
    KEYWORD,
    STRING,
    NUMBER,
    COMMENT,
    FUNCTION,
    TYPE,
    CONSTANT,
    PROPERTY,
}

/**
 * One coloured run.
 *
 * @param lineNumber the 1-based line this run sits on, or `0` when the file was
 * too large to lex and arrived as a single plain block — a caller highlighting
 * the lines an agent referenced has nothing to match against in that case, which
 * is the honest answer rather than a wrong one.
 */
public data class CodeSyntaxToken public constructor(
    public val text: String,
    public val kind: CodeSyntaxTokenKind,
    public val lineNumber: Int,
)

/**
 * A bounded lexical colouriser for the file-preview surface.
 *
 * It deliberately builds no syntax tree: previews are a peek at someone else's
 * file in a dozen languages, and a real parser for each is both far more code
 * and far more ways to be wrong about text that is often a fragment. Past the
 * two limits below it gives up and returns one plain, line-numbered block, so a
 * pathological file costs a linear scan rather than thousands of spans.
 *
 * Two deliberate departures from the ArkTS original:
 *
 * - **No token ids and no cache class.** ArkUI's `ForEach` needs a stable key
 *   per child and re-runs the builder on every render, so the source carries a
 *   `syntax-N` id and a `CodeSyntaxHighlightCache`. Compose keys spans by
 *   position and has `remember` for the caching, so both would be dead weight.
 * - **The last line of a multi-line token keeps its number.** See
 *   [appendMultiline].
 */
public object CodeSyntaxHighlighter {
    /**
     * Splits [text] into coloured runs, using [fileName]'s extension to pick the
     * keyword set. An unrecognised extension is not an error — it just means no
     * language rules apply, and the file comes back plain but still numbered.
     */
    public fun tokenize(text: String, fileName: String): List<CodeSyntaxToken> {
        val extension = extensionOf(fileName)
        if (extension !in SUPPORTED_EXTENSIONS || text.length > MAX_CHARACTERS) return plain(text)

        val tokens = mutableListOf<CodeSyntaxToken>()
        val lineNumberWidth = (text.count { it == '\n' } + 1).toString().length
        var lineNumber = 1
        var index = 0
        var needsLineNumber = true

        while (index < text.length) {
            // Checked every turn rather than only after a punctuation run, as
            // the source does: the cap exists to bound the span count, and where
            // in the loop we notice we passed it should not decide whether we do.
            if (tokens.size > MAX_TOKENS) return plain(text)

            if (needsLineNumber) {
                tokens += lineNumberToken(lineNumber, lineNumberWidth)
                needsLineNumber = false
            }

            val character = text[index]
            if (character == '\n') {
                tokens += CodeSyntaxToken("\n", CodeSyntaxTokenKind.PLAIN, lineNumber)
                index += 1
                lineNumber += 1
                needsLineNumber = true
                continue
            }

            if (hasLineCommentAt(text, index, extension)) {
                val end = lineEnd(text, index)
                tokens += CodeSyntaxToken(text.substring(index, end), CodeSyntaxTokenKind.COMMENT, lineNumber)
                index = end
                continue
            }

            val blockCommentEnd = blockCommentEnd(text, index, extension)
            if (blockCommentEnd > index) {
                val block = text.substring(index, blockCommentEnd)
                lineNumber = appendMultiline(tokens, block, CodeSyntaxTokenKind.COMMENT, lineNumberWidth, lineNumber)
                needsLineNumber = block.endsWith('\n')
                index = blockCommentEnd
                continue
            }

            if (character == '\'' || character == '"' || character == '`') {
                val end = stringEnd(text, index, character)
                val value = text.substring(index, end)
                lineNumber = appendMultiline(tokens, value, CodeSyntaxTokenKind.STRING, lineNumberWidth, lineNumber)
                needsLineNumber = value.endsWith('\n')
                index = end
                continue
            }

            if (character.isAsciiDigit()) {
                val end = numberEnd(text, index)
                tokens += CodeSyntaxToken(text.substring(index, end), CodeSyntaxTokenKind.NUMBER, lineNumber)
                index = end
                continue
            }

            if (character.isIdentifierStart()) {
                val end = identifierEnd(text, index)
                val word = text.substring(index, end)
                tokens += CodeSyntaxToken(word, identifierKind(text, end, word, extension), lineNumber)
                index = end
                continue
            }

            val end = plainEnd(text, index)
            tokens += CodeSyntaxToken(text.substring(index, end), CodeSyntaxTokenKind.PLAIN, lineNumber)
            index = end
        }

        if (needsLineNumber) tokens += lineNumberToken(lineNumber, lineNumberWidth)
        return tokens
    }

    /** Characters past which the file is shown plain. 256 KiB, the source's own cap. */
    private const val MAX_CHARACTERS: Int = 256 * 1024

    /** Runs past which the file is shown plain, so no surface mounts a span per word. */
    private const val MAX_TOKENS: Int = 12_000

    /**
     * Breaks a string literal or block comment across the lines it spans, so
     * every line still carries its own number.
     *
     * Where this differs from ArkTS: the original guards the interleaved line
     * number with `index < parts.length - 2`, which stops one line early — the
     * final line of every multi-line literal, and any blank line immediately
     * before it, silently lose their number, and the gutter stops lining up with
     * the file for everything below. The guard here instead skips only the case
     * the caller already handles: a token that *ends* on a newline, whose next
     * line number is emitted by the main loop's `needsLineNumber`.
     *
     * @return the line the token ends on.
     */
    private fun appendMultiline(
        tokens: MutableList<CodeSyntaxToken>,
        value: String,
        kind: CodeSyntaxTokenKind,
        lineNumberWidth: Int,
        firstLineNumber: Int,
    ): Int {
        val parts = value.split('\n')
        var lineNumber = firstLineNumber
        parts.forEachIndexed { index, part ->
            if (part.isNotEmpty()) tokens += CodeSyntaxToken(part, kind, lineNumber)
            if (index == parts.lastIndex) return@forEachIndexed
            tokens += CodeSyntaxToken("\n", CodeSyntaxTokenKind.PLAIN, lineNumber)
            lineNumber += 1
            val nextIsTrailingEmpty = index == parts.lastIndex - 1 && parts.last().isEmpty()
            if (!nextIsTrailingEmpty) tokens += lineNumberToken(lineNumber, lineNumberWidth)
        }
        return lineNumber
    }

    /**
     * The whole file as one run, numbers baked into the text.
     *
     * Line `0` on the token is the marker that per-line information is gone:
     * nothing downstream can highlight a target line here, and pretending
     * otherwise would put the highlight on the wrong rows.
     */
    private fun plain(text: String): List<CodeSyntaxToken> {
        val lines = text.split('\n')
        val width = lines.size.toString().length
        val numbered = lines.mapIndexed { index, line -> "${linePrefix(index + 1, width)}$line" }
        return listOf(CodeSyntaxToken(numbered.joinToString("\n"), CodeSyntaxTokenKind.PLAIN, 0))
    }

    private fun lineNumberToken(lineNumber: Int, width: Int): CodeSyntaxToken =
        CodeSyntaxToken(linePrefix(lineNumber, width), CodeSyntaxTokenKind.LINE_NUMBER, lineNumber)

    /** Right-aligned in the gutter, then two spaces — the file starts at a fixed column. */
    private fun linePrefix(lineNumber: Int, width: Int): String =
        "${lineNumber.toString().padStart(width, ' ')}  "

    private fun identifierKind(
        text: String,
        end: Int,
        word: String,
        extension: String,
    ): CodeSyntaxTokenKind {
        if (isKeyword(word, extension)) return CodeSyntaxTokenKind.KEYWORD
        if (word.lowercase() in CONSTANTS) return CodeSyntaxTokenKind.CONSTANT
        // What follows the word is all the context there is without a parser:
        // an open paren means it is being called, a colon means it is a key.
        return when (nextNonBlank(text, end)) {
            '(', '!' -> CodeSyntaxTokenKind.FUNCTION
            ':' -> CodeSyntaxTokenKind.PROPERTY
            else -> if (word[0] in 'A'..'Z') CodeSyntaxTokenKind.TYPE else CodeSyntaxTokenKind.PLAIN
        }
    }

    private fun isKeyword(word: String, extension: String): Boolean =
        word.lowercase() in (LANGUAGE_KEYWORDS[extension] ?: BASE_KEYWORDS)

    private fun hasLineCommentAt(text: String, index: Int, extension: String): Boolean = when {
        extension in HASH_COMMENT_EXTENSIONS && text[index] == '#' -> true
        extension == "sql" && text.startsWith("--", index) -> true
        else -> text.startsWith("//", index)
    }

    /** The index just past the block comment starting at [index], or [index] if there is none. */
    private fun blockCommentEnd(text: String, index: Int, extension: String): Int {
        if (text.startsWith("<!--", index)) {
            val end = text.indexOf("-->", index + 4)
            return if (end >= 0) end + 3 else text.length
        }
        // Python has no `/* */`, and `/` there is division — one file type is
        // enough of an exception to be worth the special case the source makes.
        if (extension != "py" && text.startsWith("/*", index)) {
            val end = text.indexOf("*/", index + 2)
            return if (end >= 0) end + 2 else text.length
        }
        return index
    }

    /**
     * The index just past the literal opened at [start] with [quote].
     *
     * An unterminated quote stops at the end of its line rather than swallowing
     * the rest of the file — except for a backtick, which is a template literal
     * in every language that has one and is meant to span lines.
     */
    private fun stringEnd(text: String, start: Int, quote: Char): Int {
        var index = start + 1
        var escaped = false
        while (index < text.length) {
            val character = text[index]
            when {
                escaped -> escaped = false
                character == '\\' -> escaped = true
                character == quote -> return index + 1
                character == '\n' && quote != '`' -> return index
            }
            index += 1
        }
        return text.length
    }

    /** Loose on purpose: `0xFF`, `1_000`, `1.5e3` and friends are all one run. */
    private fun numberEnd(text: String, start: Int): Int {
        var index = start + 1
        while (index < text.length) {
            val character = text[index].lowercaseChar()
            val continues = character.isAsciiDigit() || character == '.' || character == '_' ||
                character == 'x' || character in 'a'..'f'
            if (!continues) break
            index += 1
        }
        return index
    }

    private fun identifierEnd(text: String, start: Int): Int {
        var index = start + 1
        while (index < text.length && text[index].isIdentifierPart()) index += 1
        return index
    }

    /** Runs to the next thing that could start a token of its own. */
    private fun plainEnd(text: String, start: Int): Int {
        var index = start + 1
        while (index < text.length) {
            val character = text[index]
            val stops = character == '\n' || character == '\'' || character == '"' || character == '`' ||
                character == '#' || character.isAsciiDigit() || character.isIdentifierStart() ||
                text.startsWith("//", index) || text.startsWith("/*", index) || text.startsWith("--", index)
            if (stops) break
            index += 1
        }
        return index
    }

    private fun lineEnd(text: String, start: Int): Int {
        val end = text.indexOf('\n', start)
        return if (end >= 0) end else text.length
    }

    private fun nextNonBlank(text: String, start: Int): Char? {
        var index = start
        while (index < text.length && (text[index] == ' ' || text[index] == '\t')) index += 1
        return if (index < text.length) text[index] else null
    }

    private fun extensionOf(fileName: String): String {
        val name = fileName.replace('\\', '/').substringAfterLast('/')
        val dot = name.lastIndexOf('.')
        return if (dot >= 0) name.substring(dot + 1).lowercase() else ""
    }

    private fun Char.isAsciiDigit(): Boolean = this in '0'..'9'

    private fun Char.isIdentifierStart(): Boolean =
        this in 'a'..'z' || this in 'A'..'Z' || this == '_' || this == '$'

    private fun Char.isIdentifierPart(): Boolean = isIdentifierStart() || isAsciiDigit()

    private val CONSTANTS: Set<String> =
        setOf("true", "false", "null", "undefined", "none", "nil", "self", "this", "super")

    /** Control flow, which every language in the list below spells much the same way. */
    private val BASE_KEYWORDS: Set<String> = setOf(
        "if", "else", "for", "while", "do", "switch", "case", "break", "continue",
        "return", "throw", "try", "catch", "finally", "new", "in", "of",
    )

    private val LANGUAGE_KEYWORDS: Map<String, Set<String>> = listOf(
        listOf("js", "jsx", "ts", "tsx", "mjs", "cjs", "ets", "vue", "svelte") to setOf(
            "const", "let", "var", "function", "class", "extends", "implements", "interface",
            "type", "enum", "import", "export", "from", "as", "async", "await", "yield",
            "default", "delete", "instanceof", "typeof", "void", "public", "private",
            "protected", "readonly", "static", "get", "set", "declare", "namespace",
        ),
        listOf("rs") to setOf(
            "fn", "let", "mut", "struct", "enum", "impl", "trait", "use", "mod", "pub",
            "crate", "where", "match", "move", "ref", "async", "await", "dyn", "unsafe",
            "extern", "const", "static", "type", "loop",
        ),
        listOf("py", "pyw", "pyi") to setOf(
            "def", "class", "import", "from", "as", "lambda", "with", "async", "await",
            "yield", "raise", "pass", "global", "nonlocal", "assert", "del", "elif",
            "except", "finally", "is", "not", "and", "or",
        ),
        listOf("go") to setOf(
            "func", "package", "import", "defer", "go", "select", "chan", "map", "range",
            "struct", "interface", "type", "var", "const", "fallthrough",
        ),
        listOf(
            "java", "kt", "kts", "scala", "groovy", "c", "cpp", "cc", "cxx",
            "h", "hpp", "hxx", "hh", "cs", "swift",
        ) to setOf(
            "class", "struct", "interface", "enum", "namespace", "using", "import",
            "package", "public", "private", "protected", "static", "final", "virtual",
            "override", "abstract", "const", "var", "val", "fun", "func", "operator",
            "template", "typename", "extends", "implements", "throws",
        ),
        listOf("sh", "bash", "zsh", "fish", "ps1", "bat", "cmd") to setOf(
            "then", "fi", "elif", "done", "function", "select", "until",
            "export", "local", "readonly", "declare", "set", "unset",
        ),
        listOf("sql") to setOf(
            "select", "insert", "update", "delete", "create", "alter", "drop", "from",
            "join", "inner", "left", "right", "on", "where", "group", "order", "by",
            "having", "limit", "offset", "union", "all", "distinct", "into", "values",
            "table", "index", "view", "and", "or", "not", "null",
        ),
    ).flatMap { (extensions, keywords) ->
        extensions.map { extension -> extension to BASE_KEYWORDS + keywords }
    }.toMap()

    private val HASH_COMMENT_EXTENSIONS: Set<String> = setOf(
        "py", "pyw", "pyi", "rb", "sh", "bash", "zsh", "fish",
        "yaml", "yml", "toml", "conf", "cfg", "ini",
    )

    /** Everything else falls through to [plain] — numbered, but uncoloured. */
    private val SUPPORTED_EXTENSIONS: Set<String> = setOf(
        "js", "jsx", "ts", "tsx", "mjs", "cjs", "ets", "vue", "svelte", "rs",
        "py", "pyw", "pyi", "rb", "go", "java", "kt", "kts", "scala", "groovy",
        "c", "cpp", "cc", "cxx", "h", "hpp", "hxx", "hh", "cs", "swift", "php",
        "css", "scss", "less", "json", "jsonc", "yaml", "yml", "toml", "xml",
        "html", "htm", "sh", "bash", "zsh", "fish", "ps1", "bat", "cmd",
        "sql", "graphql", "gql", "proto",
    )
}
