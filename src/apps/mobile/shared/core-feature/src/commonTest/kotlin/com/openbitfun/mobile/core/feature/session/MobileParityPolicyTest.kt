package com.openbitfun.mobile.core.feature.session

import com.openbitfun.mobile.core.protocol.RelayJson
import com.openbitfun.mobile.core.protocol.WorkspaceInfoResponse
import com.openbitfun.mobile.core.domain.SessionAgentTypes
import kotlin.test.*

class MobileParityPolicyTest {
    @Test fun oldWorkspacePayloadKeepsLegacyCreation() {
        val old = """{"resp":"ok","path":"/work","has_workspace":true}"""
        val decoded = RelayJson.decodeFromString<WorkspaceInfoResponse>(old)
        assertFalse(HarnessProfilePolicy.supported(decoded.capabilities))
        assertEquals("code", HarnessProfilePolicy.creationAgent(HarnessProfile.ULTIMATE, decoded.capabilities))
        assertEquals(decoded, RelayJson.decodeFromString<WorkspaceInfoResponse>(RelayJson.encodeToString(decoded)))
        val current = RelayJson.decodeFromString<WorkspaceInfoResponse>("""{"resp":"ok","capabilities":["harness_profiles_v1","future"]}""")
        assertTrue(HarnessProfilePolicy.supported(current.capabilities))
        assertEquals(listOf("minimal", "agentic", "Ultra"), HarnessProfile.entries.map { HarnessProfilePolicy.creationAgent(it, current.capabilities) })
        assertTrue(HarnessProfile.entries.all { SessionAgentTypes.isCode(it.agentType) })
    }

    @Test fun speechPreservesDraftAndReplacesPartialTranscripts() {
        assertEquals("\u8bf7\u68c0\u67e5\u4ee3\u7801", VoiceDraftPolicy.merge("\u8bf7\u68c0\u67e5", "\u4ee3\u7801"))
        assertEquals("read file", VoiceDraftPolicy.merge("read", "file"))
        assertEquals("line\nnext", VoiceDraftPolicy.merge("line\n", "next"))
        assertEquals("  draft  ", VoiceDraftPolicy.merge("  draft  ", " "))
        assertEquals("read filename", VoiceDraftPolicy.merge("read", "filename"))
    }

    @Test fun notificationRequiresObservedSuccessfulTurnAndDeduplicatesReplay() {
        val policy = TaskCompletionPolicy()
        val turn = TaskCompletionIdentity("desktop-a", "session", "turn")
        policy.setBackground(true)
        assertNull(policy.finish(turn, "completed"))
        policy.track(turn)
        assertNull(policy.finish(turn.copy(target = "desktop-b"), "completed"))
        assertEquals(turn, policy.finish(turn, "completed"))
        policy.track(turn)
        assertNull(policy.finish(turn, "completed"))
        val next = turn.copy(turnId = "next")
        policy.track(next)
        assertNull(policy.finish(next, "failed"))
        assertFalse(policy.hasPending())
        policy.track(next.copy(turnId = "cancelled"))
        policy.reset()
        assertFalse(policy.hasPending())
    }

    @Test fun foregroundAndCancellationStayQuiet() {
        val policy = TaskCompletionPolicy()
        val turn = TaskCompletionIdentity("desktop", "session", "turn")
        policy.track(turn)
        assertNull(policy.finish(turn, "completed"))
        policy.setBackground(true)
        val next = turn.copy(turnId = "next")
        policy.track(next)
        assertNull(policy.finish(next, "cancelled"))
        assertFalse(policy.hasPending())
    }
}
