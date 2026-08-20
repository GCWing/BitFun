package com.bitfun.mobile.core.domain

public enum class FileReferenceKind {
    REMOTE_WORKSPACE_FILE,
    HTTP_URL,
    ANCHOR,
    UNSUPPORTED_SCHEME,
    INVALID,
}

public data class FilePreviewTargetContext public constructor(
    public val sessionId: String,
    public val workspacePath: String,
    public val controlTargetEpoch: Int,
)

public data class FilePreviewTarget public constructor(
    public val path: String,
    public val remotePath: String,
    public val displayName: String,
    public val sessionId: String,
    public val workspacePath: String,
    public val controlTargetEpoch: Int,
    public val lineStart: Int,
    public val lineEnd: Int,
)

public data class FileReferenceResolution public constructor(
    public val kind: FileReferenceKind,
    public val target: FilePreviewTarget?,
)

public enum class FilePreviewFailureReason {
    NOT_FOUND,
    UNAVAILABLE,
    ACCESS_DENIED,
    TOO_LARGE,
    CONNECTION,
    LOAD_FAILED,
}

public data class FilePreviewFailure public constructor(
    public val reason: FilePreviewFailureReason,
    public val retryable: Boolean,
)

/**
 * Which of the preview surface's bodies a file gets. HarmonyOS decides this in
 * `RemoteFilePreviewController.rendererFor`; the choice is the same product
 * rule on both platforms, so it lives here rather than in either app.
 */
public enum class FilePreviewRenderer {
    IMAGE,
    MARKDOWN,
    TEXT,
    UNSUPPORTED,
}

public object FilePreviewPolicy {
    public const val TEXT_MAX_BYTES: Long = 2L * 1024L * 1024L
    public const val IMAGE_MAX_BYTES: Long = 12L * 1024L * 1024L

    public fun textReadLimit(fileSize: Long): Long =
        if (fileSize > 0) minOf(fileSize, TEXT_MAX_BYTES) else TEXT_MAX_BYTES

    public fun canPreviewImage(fileSize: Long): Boolean = fileSize <= IMAGE_MAX_BYTES

    /**
     * SVG is an image by MIME and a text file by everything the preview can
     * actually do with it, so it deliberately falls through to the text branch.
     */
    public fun rendererFor(name: String, mimeType: String): FilePreviewRenderer {
        val mime = mimeType.lowercase()
        val extension = extensionOf(name)
        if (mime.startsWith("image/") && extension != "svg") return FilePreviewRenderer.IMAGE
        if (mime == "text/markdown" || extension == "md" || extension == "mdx") {
            return FilePreviewRenderer.MARKDOWN
        }
        if (mime.startsWith("text/") || mime in TEXT_MIME_TYPES ||
            extension in TEXT_EXTENSIONS || fileNameOf(name) in TEXT_FILE_NAMES
        ) {
            return FilePreviewRenderer.TEXT
        }
        return FilePreviewRenderer.UNSUPPORTED
    }

    /**
     * A NUL byte in the first 4 KiB is how HarmonyOS decides the desktop handed
     * back something that only claimed to be text. Reading further would not
     * change the answer and the preview only ever holds the head of the file.
     */
    public fun looksBinary(bytes: ByteArray): Boolean {
        val limit = minOf(bytes.size, SNIFF_BYTES)
        for (index in 0 until limit) {
            if (bytes[index].toInt() == 0) return true
        }
        return false
    }

    /**
     * Text that decoded without a NUL but is still not text: a replacement
     * character means the bytes were not UTF-8, and more than 2% control
     * characters means they were never prose or source to begin with.
     */
    public fun looksUndecodable(bytes: ByteArray, text: String): Boolean {
        if (text.contains('\uFFFD')) return true
        val limit = minOf(bytes.size, SNIFF_BYTES)
        if (limit == 0) return false
        var controls = 0
        for (index in 0 until limit) {
            val value = bytes[index].toInt() and 0xFF
            if (value < 32 && value != 9 && value != 10 && value != 13) controls += 1
        }
        return controls.toDouble() / limit > CONTROL_RATIO_LIMIT
    }

    private fun extensionOf(name: String): String {
        val fileName = fileNameOf(name)
        val dot = fileName.lastIndexOf('.')
        return if (dot >= 0) fileName.substring(dot + 1) else ""
    }

    private fun fileNameOf(name: String): String =
        name.replace('\\', '/').substringAfterLast('/').lowercase()

    private const val SNIFF_BYTES: Int = 4096
    private const val CONTROL_RATIO_LIMIT: Double = 0.02

    private val TEXT_MIME_TYPES: Set<String> = setOf(
        "application/json",
        "application/xml",
        "application/javascript",
    )

    private val TEXT_EXTENSIONS: Set<String> = setOf(
        "js", "jsx", "ts", "tsx", "mjs", "cjs", "py", "rs", "go", "java", "kt", "kts", "c", "cpp",
        "h", "hpp", "cs", "rb", "php", "swift", "vue", "svelte", "css", "scss", "less", "json",
        "jsonc", "yaml", "yml", "toml", "xml", "csv", "tsv", "txt", "log", "sh", "bash", "zsh",
        "fish", "ps1", "bat", "cmd", "sql", "graphql", "gql", "proto", "lock", "env", "ini", "cfg",
        "conf", "ets", "gitignore", "editorconfig", "gradle", "properties", "svg", "cc",
    )

    private val TEXT_FILE_NAMES: Set<String> = setOf(
        "dockerfile", "makefile", "justfile", "gemfile", "rakefile", "procfile",
        "license", "readme", "changelog",
    )

    public fun failure(message: String): FilePreviewFailure {
        val text = message.lowercase()
        return when {
            listOf("file not found", "file does not exist", "not a regular file", "no such file")
                .any(text::contains) -> FilePreviewFailure(FilePreviewFailureReason.NOT_FOUND, true)
            text.contains("could not be resolved") ->
                FilePreviewFailure(FilePreviewFailureReason.UNAVAILABLE, true)
            listOf("access denied", "restricted", "outside").any(text::contains) ->
                FilePreviewFailure(FilePreviewFailureReason.ACCESS_DENIED, false)
            text.contains("file too large") || text.contains("too large") ->
                FilePreviewFailure(FilePreviewFailureReason.TOO_LARGE, false)
            listOf("timeout", "network", "connect", "unreachable", "offline").any(text::contains) ->
                FilePreviewFailure(FilePreviewFailureReason.CONNECTION, true)
            else -> FilePreviewFailure(FilePreviewFailureReason.LOAD_FAILED, true)
        }
    }
}

public object FileTargetResolver {
    public fun matchesRemotePath(reference: String, remotePath: String): Boolean {
        val raw = cleanReference(reference)
        if (raw.isEmpty() || remotePath.isEmpty() || raw.startsWith('#')) return false
        val range = extractLineRange(raw)
        if (hasUnsupportedScheme(range.path)) return false
        return normalizeRemoteFilePath(range.path) == remotePath
    }

    public fun resolve(
        reference: String,
        label: String,
        context: FilePreviewTargetContext,
    ): FileReferenceResolution {
        val raw = cleanReference(reference)
        if (raw.isEmpty()) return FileReferenceResolution(FileReferenceKind.INVALID, null)
        val lower = raw.lowercase()
        if (lower.startsWith("http://") || lower.startsWith("https://")) {
            return FileReferenceResolution(FileReferenceKind.HTTP_URL, null)
        }
        if (raw.startsWith('#')) return FileReferenceResolution(FileReferenceKind.ANCHOR, null)
        val range = extractLineRange(raw)
        if (hasUnsupportedScheme(range.path)) {
            return FileReferenceResolution(FileReferenceKind.UNSUPPORTED_SCHEME, null)
        }
        val remotePath = normalizeRemoteFilePath(range.path)
        if (remotePath.isEmpty() || remotePath == "/") {
            return FileReferenceResolution(FileReferenceKind.INVALID, null)
        }
        return FileReferenceResolution(
            FileReferenceKind.REMOTE_WORKSPACE_FILE,
            FilePreviewTarget(
                path = raw,
                remotePath = remotePath,
                displayName = label.trim().ifEmpty { basename(remotePath) },
                sessionId = context.sessionId,
                workspacePath = context.workspacePath,
                controlTargetEpoch = context.controlTargetEpoch,
                lineStart = range.start,
                lineEnd = range.end,
            ),
        )
    }

    public fun normalizeRemoteFilePath(path: String): String = path
        .removePrefix("computer://")
        .removePrefix("file://")
        .trim()

    private fun extractLineRange(reference: String): FileTargetLineRange {
        val hash = reference.lastIndexOf('#')
        if (hash > 0) {
            val parsed = parseLineMarker(reference.substring(hash + 1))
            if (parsed.start > 0) return FileTargetLineRange(reference.substring(0, hash), parsed.start, parsed.end)
        }
        val colon = COLON_RANGE.matchEntire(reference)
        if (colon != null && !isWindowsDrivePrefix(colon.groupValues[1])) {
            return FileTargetLineRange(
                colon.groupValues[1],
                colon.groupValues[2].toInt(),
                colon.groupValues[3].toIntOrNull() ?: 0,
            )
        }
        return FileTargetLineRange(reference, 0, 0)
    }

    private fun parseLineMarker(marker: String): FileTargetLineRange {
        val match = LINE_MARKER.matchEntire(marker) ?: return FileTargetLineRange("", 0, 0)
        return FileTargetLineRange("", match.groupValues[1].toInt(), match.groupValues[2].toIntOrNull() ?: 0)
    }

    private fun hasUnsupportedScheme(reference: String): Boolean {
        val scheme = SCHEME.find(reference)?.groupValues?.get(1) ?: return false
        if (scheme.length == 1 && reference.length >= 3 && (reference[2] == '/' || reference[2] == '\\')) {
            return false
        }
        return scheme.lowercase() != "computer" && scheme.lowercase() != "file"
    }

    private fun isWindowsDrivePrefix(value: String): Boolean = value.length == 1 && value[0].isLetter()

    private fun cleanReference(reference: String): String {
        var clean = reference.trim()
        while (clean.lastOrNull() in TRAILING_PUNCTUATION) clean = clean.dropLast(1)
        return percentDecode(clean)
    }

    private fun percentDecode(value: String): String {
        if ('%' !in value) return value
        val result = StringBuilder()
        var index = 0
        while (index < value.length) {
            if (value[index] != '%' || index + 2 >= value.length) {
                result.append(value[index])
                index += 1
                continue
            }
            val bytes = mutableListOf<Byte>()
            while (index + 2 < value.length && value[index] == '%') {
                val byte = value.substring(index + 1, index + 3).toIntOrNull(16) ?: break
                bytes += byte.toByte()
                index += 3
            }
            if (bytes.isEmpty()) {
                result.append('%')
                index += 1
            } else {
                result.append(bytes.toByteArray().decodeToString())
            }
        }
        return result.toString()
    }

    private fun basename(path: String): String = path.replace('\\', '/').substringAfterLast('/').ifEmpty { "file" }

    private data class FileTargetLineRange(val path: String, val start: Int, val end: Int)

    private val COLON_RANGE = Regex("^(.+):(\\d+)(?:-(\\d+))?$")
    private val LINE_MARKER = Regex("^L?(\\d+)(?:-L?(\\d+))?$", RegexOption.IGNORE_CASE)
    private val SCHEME = Regex("^([A-Za-z][A-Za-z0-9+.-]*):")
    private val TRAILING_PUNCTUATION = setOf(',', '.', ';', ':', ')', ']', '}', '>', '，', '。', '；', '：')
}
