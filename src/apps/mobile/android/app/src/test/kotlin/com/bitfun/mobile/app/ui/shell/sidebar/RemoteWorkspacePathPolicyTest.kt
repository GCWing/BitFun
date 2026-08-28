package com.bitfun.mobile.app.ui.shell.sidebar

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RemoteWorkspacePathPolicyTest {
    @Test
    fun trailingSeparatorsDoNotChangeEquality() {
        assertTrue(RemoteWorkspacePathPolicy.equal("/workspace", "/workspace/"))
        assertTrue(RemoteWorkspacePathPolicy.equal("/workspace///", "/workspace"))
        assertTrue(RemoteWorkspacePathPolicy.equal("/workspace\\", "/workspace"))
    }

    @Test
    fun surroundingWhitespaceIsIgnored() {
        assertTrue(RemoteWorkspacePathPolicy.equal("  /workspace  ", "/workspace"))
    }

    @Test
    fun rootEmptyBlankAndSingleCharacterEdgesArePreserved() {
        assertEquals("/", RemoteWorkspacePathPolicy.normalize("///"))
        assertEquals("\\", RemoteWorkspacePathPolicy.normalize("\\\\"))
        assertEquals("", RemoteWorkspacePathPolicy.normalize("   "))
        assertEquals("/", RemoteWorkspacePathPolicy.normalize("/"))
        assertTrue(RemoteWorkspacePathPolicy.equal("", "   "))
        assertFalse(RemoteWorkspacePathPolicy.equal("/", ""))
    }

    @Test
    fun realWorkspacePathWithTrailingSlashEqualsCanonicalPath() {
        val path = "/home/user/project/src"
        assertTrue(RemoteWorkspacePathPolicy.equal(path, "$path/"))
    }

    @Test
    fun distinctPathsRemainUnequal() {
        assertFalse(RemoteWorkspacePathPolicy.equal("/home/user/project", "/home/user/other"))
        assertFalse(RemoteWorkspacePathPolicy.equal("/workspace/a", "/workspace/ab"))
    }
}
