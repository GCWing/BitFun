import { useEffect, useRef, type HTMLAttributes, type ReactNode } from "react";
import { ArrowLeft, Mic, MicOff, Phone, SlidersHorizontal, X } from "lucide-react";
import { classNames } from "../../internal/classNames";
import { OverflowText } from "../../primitives/OverflowText";
import { IconButton } from "../IconButton";
import { Tooltip } from "../Tooltip";
import { VoiceParticleLogo, type VoiceParticleAudioReader } from "../VoiceParticleLogo";
import styles from "./VoiceCallPanel.module.css";

export type VoiceCallPhase = "connecting" | "live" | "ending" | "error";
export interface VoiceCallLabels {
  back: string;
  close: string;
  mute: string;
  unmute: string;
  settings: string;
  end: string;
}
export interface VoiceCallPanelProps extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  title: string;
  labels: VoiceCallLabels;
  phase?: VoiceCallPhase;
  muted?: boolean;
  userTranscript?: string;
  assistantTranscript?: string;
  /** Localized connection, error or task status supplied by the application. */
  status?: ReactNode;
  readAudio?: VoiceParticleAudioReader;
  onBack: () => void;
  onClose: () => void;
  onToggleMute: () => void;
  onOpenSettings: () => void;
  onEnd: () => void;
}

/** Controlled call presentation. Contains no routes, stores, locale catalogs or media APIs. */
export function VoiceCallPanel({
  title, labels, phase = "live", muted = false, userTranscript, assistantTranscript,
  status, readAudio, onBack, onClose, onToggleMute, onOpenSettings, onEnd, className, ...props
}: VoiceCallPanelProps) {
  const conversationRef = useRef<HTMLDivElement>(null);
  const followsLatest = useRef(true);
  const connecting = phase === "connecting" || phase === "ending";
  useEffect(() => {
    const conversation = conversationRef.current;
    if (conversation && followsLatest.current) conversation.scrollTop = conversation.scrollHeight;
  }, [userTranscript, assistantTranscript, status]);

  return (
    <section {...props} className={classNames(styles.root, className)} aria-label={title}
      data-openbitfun-component="voice-call-panel" data-openbitfun-part="root" data-openbitfun-phase={phase}>
      <header className={styles.header} data-openbitfun-part="header">
        <Tooltip content={labels.back}>
          <IconButton className={styles.back} shape="circle" aria-label={labels.back}
            onClick={onBack} disabled={phase === "ending"} icon={<ArrowLeft size={20} />} />
        </Tooltip>
        <h2 className={styles.title} data-openbitfun-part="title"><OverflowText>{title}</OverflowText></h2>
        <Tooltip content={labels.close}>
          <IconButton className={styles.close} shape="circle" aria-label={labels.close}
            onClick={onClose} icon={<X size={20} />} />
        </Tooltip>
      </header>

      <div className={styles.visualizer} data-openbitfun-part="visualizer">
        <VoiceParticleLogo readAudio={readAudio} active={phase === "live" || phase === "connecting"} />
      </div>

      <div ref={conversationRef} className={styles.conversation} data-openbitfun-part="conversation"
        aria-live="polite" aria-relevant="additions text" onScroll={() => {
          const element = conversationRef.current;
          if (element) followsLatest.current = element.scrollHeight - element.clientHeight - element.scrollTop < 24;
        }}>
        {userTranscript && <div className={styles.user} data-openbitfun-part="userTranscript">{userTranscript}</div>}
        {assistantTranscript && <div className={styles.assistant} data-openbitfun-part="assistantTranscript">{assistantTranscript}</div>}
        {status && <div className={styles.status} role="status" data-openbitfun-part="status">{status}</div>}
      </div>

      <footer className={styles.controls} data-openbitfun-part="controls">
        <Tooltip content={muted ? labels.unmute : labels.mute}>
          <IconButton className={styles.control} shape="circle" aria-label={muted ? labels.unmute : labels.mute}
            aria-pressed={muted} disabled={connecting} onClick={onToggleMute}
            icon={muted ? <MicOff size={28} /> : <Mic size={28} />} />
        </Tooltip>
        <Tooltip content={labels.settings}>
          <IconButton className={styles.control} shape="circle" aria-label={labels.settings}
            onClick={onOpenSettings} icon={<SlidersHorizontal size={28} />} />
        </Tooltip>
        <Tooltip content={labels.end}>
          <IconButton className={styles.control} shape="circle" aria-label={labels.end}
            disabled={phase === "ending"} onClick={onEnd} icon={<Phone size={28} />} />
        </Tooltip>
      </footer>
    </section>
  );
}
