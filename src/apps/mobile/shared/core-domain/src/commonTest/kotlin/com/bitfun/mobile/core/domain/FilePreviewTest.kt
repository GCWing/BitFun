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
}
