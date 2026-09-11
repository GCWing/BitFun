import type { ComponentMeta } from "../../registry.types";

export const voiceCallPanelMeta = {
  category: "feedback",
  name: "VoiceCallPanel",
  description: "A controlled live-call surface with a reactive particle logo, transcripts, and accessible call controls.",
  maturity: "stable",
  props: [
    { name: "title", type: "string" },
    { name: "labels", type: "VoiceCallLabels" },
    { name: "phase", type: "connecting | live | ending | error", defaultValue: "live" },
    { name: "muted", type: "boolean", defaultValue: "false" },
    { name: "userTranscript", type: "string" },
    { name: "assistantTranscript", type: "string" },
    { name: "status", type: "ReactNode" },
    { name: "readAudio", type: "VoiceParticleAudioReader" },
    { name: "onBack / onClose / onToggleMute / onOpenSettings / onEnd", type: "() => void" },
  ],
  states: ["connecting", "live", "muted", "ending", "error"],
  tokens: [
    "color.content.onDark", "color.content.onLight",
    "type.heading.panel", "type.body.lg", "type.modifier.leading.tight",
    "space.3", "space.6", "space.8", "radius.lg",
  ],
} as const satisfies ComponentMeta;
