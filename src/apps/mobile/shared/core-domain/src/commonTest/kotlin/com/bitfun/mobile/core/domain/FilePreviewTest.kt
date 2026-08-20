package com.bitfun.mobile.core.domain

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class FilePreviewTest {
    @Test
    fun resolvesRemoteFileReferencesAndLineRanges() {
        val context = FilePreviewTargetContext("session-1", "/workspace/BitFun", 4)
        val computer = FileTargetResolver.resolve("computer://src/main.rs#L42-L58", "", context)
        val relative = FileTargetResolver.resolve("README.md:12-18", "Readme", context)
        val windows = FileTargetResolver.resolve("C:\\workspace\\main.cpp#L9", "", context)

        assertEquals(FileReferenceKind.REMOTE_WORKSPACE_FILE, computer.kind)
        assertEquals("src/main.rs", computer.target?.remotePath)
        assertEquals(42, computer.target?.lineStart)
        assertEquals(58, computer.target?.lineEnd)
        assertEquals("Readme", relative.target?.displayName)
        assertEquals(12, relative.target?.lineStart)
        assertEquals("C:\\workspace\\main.cpp", windows.target?.remotePath)
        assertEquals(9, windows.target?.lineStart)
        assertTrue(FileTargetResolver.matchesRemotePath("computer://src/main.rs#L42-L58", "src/main.rs"))
        assertTrue(FileTargetResolver.matchesRemotePath("src/main.rs:42", "src/main.rs"))
        assertFalse(FileTargetResolver.matchesRemotePath("src/other.rs", "src/main.rs"))
    }

    @Test
    fun normalizesSchemesEncodedPathsAndTrailingPunctuation() {
        val context = FilePreviewTargetContext("session-1", "/workspace/BitFun", 4)
        assertEquals(
            "/workspace/Makefile",
            FileTargetResolver.resolve("file:///workspace/Makefile", "", context).target?.remotePath,
        )
        assertEquals(
            "docs/My File.md",
            FileTargetResolver.resolve("computer://docs/My%20File.md),", "", context).target?.remotePath,
        )
        listOf("/workspace/Dockerfile", ".env", "LICENSE").forEach { path ->
            assertEquals(FileReferenceKind.REMOTE_WORKSPACE_FILE, FileTargetResolver.resolve(path, "", context).kind)
        }
    }

    @Test
    fun classifiesReferencesWithoutFileTargets() {
        val context = FilePreviewTargetContext("session-1", "/workspace/BitFun", 1)
        assertEquals(FileReferenceKind.HTTP_URL, FileTargetResolver.resolve("https://example.com", "", context).kind)
        assertEquals(FileReferenceKind.ANCHOR, FileTargetResolver.resolve("#section", "", context).kind)
        assertEquals(
            FileReferenceKind.UNSUPPORTED_SCHEME,
            FileTargetResolver.resolve("mailto:test@example.com", "", context).kind,
        )
        assertEquals(FileReferenceKind.INVALID, FileTargetResolver.resolve("", "", context).kind)
    }

    @Test
    fun centralizesFileLimitsAndTypedFailures() {
        assertEquals(128, FilePreviewPolicy.textReadLimit(128))
        assertEquals(2L * 1024L * 1024L, FilePreviewPolicy.textReadLimit(0))
        assertEquals(2L * 1024L * 1024L, FilePreviewPolicy.textReadLimit(3L * 1024L * 1024L))
        assertTrue(FilePreviewPolicy.canPreviewImage(12L * 1024L * 1024L))
        assertFalse(FilePreviewPolicy.canPreviewImage(12L * 1024L * 1024L + 1))
        assertEquals(FilePreviewFailureReason.NOT_FOUND, FilePreviewPolicy.failure("File not found").reason)
        assertTrue(FilePreviewPolicy.failure("File not found").retryable)
        assertEquals(FilePreviewFailureReason.ACCESS_DENIED, FilePreviewPolicy.failure("outside workspace").reason)
        assertFalse(FilePreviewPolicy.failure("outside workspace").retryable)
        assertEquals(FilePreviewFailureReason.TOO_LARGE, FilePreviewPolicy.failure("file too large").reason)
        assertEquals(FilePreviewFailureReason.LOAD_FAILED, FilePreviewPolicy.failure("backend detail").reason)
    }

    /**
     * The renderer is chosen the same way as HarmonyOS'
     * `RemoteFilePreviewController.rendererFor`, including the two cases where
     * the MIME type on its own gives the wrong answer.
     */
    @Test
    fun theRendererFollowsTheNameWhereTheTypeIsNotEnough() {
        assertEquals(FilePreviewRenderer.IMAGE, FilePreviewPolicy.rendererFor("logo.png", "image/png"))
        // SVG is an image the preview can only show as its source.
        assertEquals(FilePreviewRenderer.TEXT, FilePreviewPolicy.rendererFor("logo.svg", "image/svg+xml"))
        // Desktops report Markdown as plain text, so the extension decides.
        assertEquals(FilePreviewRenderer.MARKDOWN, FilePreviewPolicy.rendererFor("README.md", "text/plain"))
        assertEquals(FilePreviewRenderer.MARKDOWN, FilePreviewPolicy.rendererFor("guide.mdx", ""))
        assertEquals(FilePreviewRenderer.MARKDOWN, FilePreviewPolicy.rendererFor("notes", "text/markdown"))
        assertEquals(FilePreviewRenderer.TEXT, FilePreviewPolicy.rendererFor("main.rs", "application/octet-stream"))
        // A build file with no extension at all.
        assertEquals(FilePreviewRenderer.TEXT, FilePreviewPolicy.rendererFor("/repo/Dockerfile", ""))
        assertEquals(FilePreviewRenderer.TEXT, FilePreviewPolicy.rendererFor("data.json", "application/json"))
        assertEquals(FilePreviewRenderer.UNSUPPORTED, FilePreviewPolicy.rendererFor("app.apk", "application/zip"))
    }

    /**
     * A desktop that calls a binary `text/plain` would otherwise fill the
     * preview with replacement characters.
     */
    @Test
    fun bytesThatAreNotTextAreRecognisedAsSuch() {
        val binary = byteArrayOf(0x7F, 0x45, 0x4C, 0x46, 0x00, 0x01)
        assertTrue(FilePreviewPolicy.looksBinary(binary))
        assertFalse(FilePreviewPolicy.looksBinary("fn main() {}\n".encodeToByteArray()))

        val source = "fn main() {\n\tprintln!(\"hi\");\r\n}\n".encodeToByteArray()
        assertFalse(FilePreviewPolicy.looksUndecodable(source, source.decodeToString()))
        // Tabs, newlines, and carriage returns are text; other controls are not.
        val controls = ByteArray(100) { if (it % 4 == 0) 0x01 else 0x61 }
        assertTrue(FilePreviewPolicy.looksUndecodable(controls, controls.decodeToString()))
        assertTrue(FilePreviewPolicy.looksUndecodable(byteArrayOf(0x61), "a\uFFFD"))
        assertFalse(FilePreviewPolicy.looksUndecodable(ByteArray(0), ""))
    }
}
