package com.openbitfun.mobile.core.feature.session

/** Canonical host agent IDs, matching the HarmonyOS creation menu. */
public enum class HarnessProfile(public val agentType: String) {
    MINIMAL("minimal"), STANDARD("agentic"), ULTIMATE("Ultra"),
}

public object HarnessProfilePolicy {
    public fun supported(capabilities: List<String>): Boolean = "harness_profiles_v1" in capabilities
    public fun creationAgent(profile: HarnessProfile, capabilities: List<String>): String =
        if (supported(capabilities)) profile.agentType else "code"
}

/** Replaces partial speech against the original draft without destroying whitespace. */
public object VoiceDraftPolicy {
    public fun merge(base: String, transcript: String): String {
        val spoken = transcript.trim()
        if (base.isEmpty()) return spoken
        if (spoken.isEmpty()) return base
        val space = base.last() in '!'..'~' && (spoken.first() in 'a'..'z' ||
            spoken.first() in 'A'..'Z' || spoken.first() in '0'..'9')
        return base + (if (space) " " else "") + spoken
    }
}
