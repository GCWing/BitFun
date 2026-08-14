const AGENTIC_FRONTEND_EVENT_NAMES: Readonly<Record<string, string>> = {
  SessionCreated: 'agentic://session-created',
  SessionDeleted: 'agentic://session-deleted',
  SessionStateChanged: 'agentic://session-state-changed',
  SessionHistoryChanged: 'agentic://session-history-changed',
  SessionTitleGenerated: 'session_title_generated',
  ImageAnalysisStarted: 'agentic://image-analysis-started',
  ImageAnalysisCompleted: 'agentic://image-analysis-completed',
  DialogTurnStarted: 'agentic://dialog-turn-started',
  SubagentSessionLinked: 'agentic://subagent-session-linked',
  ModelRoundStarted: 'agentic://model-round-started',
  ModelRoundCompleted: 'agentic://model-round-completed',
  ModelRoundAttemptSuperseded: 'agentic://model-round-attempt-superseded',
  TextChunk: 'agentic://text-chunk',
  ThinkingChunk: 'agentic://text-chunk',
  ToolEvent: 'agentic://tool-event',
  DialogTurnCompleted: 'agentic://dialog-turn-completed',
  DialogTurnFailed: 'agentic://dialog-turn-failed',
  DialogTurnCancelled: 'agentic://dialog-turn-cancelled',
  TokenUsageUpdated: 'agentic://token-usage-updated',
  ContextCompressionStarted: 'agentic://context-compression-started',
  ContextCompressionCompleted: 'agentic://context-compression-completed',
  ContextCompressionFailed: 'agentic://context-compression-failed',
  ThreadGoalUpdated: 'agentic://thread-goal-updated',
  DeepReviewQueueStateChanged: 'agentic://deep-review-queue-state-changed',
  SessionModelAutoMigrated: 'agentic://session-model-auto-migrated',
  SessionReasoningPresetAutoCleared: 'agentic://session-reasoning-preset-auto-cleared',
  UserSteeringInjected: 'agentic://user-steering-injected',
};

function camelKey(key: string): string {
  return key.replace(/_([a-z])/g, (_match, letter: string) => letter.toUpperCase());
}

function projectTopLevelFields(value: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(value).map(([key, nested]) => [camelKey(key), nested]),
  );
}

export interface AgenticFrontendEventProjection {
  eventName: string;
  payload: Record<string, unknown>;
  envelopeId?: string;
}

/** Project one authoritative AgenticEventEnvelope into the established Web event bus. */
export function projectAgenticEventEnvelope(
  envelope: unknown,
): AgenticFrontendEventProjection | null {
  if (!envelope || typeof envelope !== 'object') {
    return null;
  }
  const envelopeRecord = envelope as Record<string, unknown>;
  if (!envelopeRecord.event || typeof envelopeRecord.event !== 'object') {
    return null;
  }
  const raw = envelopeRecord.event as Record<string, unknown>;
  const rawType = typeof raw.type === 'string' ? raw.type : '';
  const eventName = AGENTIC_FRONTEND_EVENT_NAMES[rawType];
  if (!eventName) {
    return null;
  }

  // Match the Rust frontend projection: only AgenticEvent variant fields are
  // renamed. Nested contracts such as ToolEventData, tool params, metadata,
  // token details, and error detail remain owned by their own serde schema.
  const payload = projectTopLevelFields(raw);
  delete payload.type;
  if (rawType === 'ThinkingChunk') {
    payload.text = payload.content;
    payload.contentType = 'thinking';
    payload.isThinkingEnd = payload.isEnd;
    delete payload.content;
    delete payload.isEnd;
  }
  if (rawType === 'SessionTitleGenerated' && payload.timestamp === undefined) {
    payload.timestamp = Date.now();
  }

  return {
    eventName,
    payload,
    envelopeId: typeof envelopeRecord.id === 'string' ? envelopeRecord.id : undefined,
  };
}
