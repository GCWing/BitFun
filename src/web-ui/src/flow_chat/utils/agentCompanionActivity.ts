import { FlowChatStore } from '../store/FlowChatStore';
import { stateMachineManager } from '../state-machine/SessionStateMachineManager';
import { ProcessingPhase, type SessionStateMachine } from '../state-machine/types';
import { deriveChatInputPetMood, type ChatInputPetMood } from './chatInputPetMood';
import type { DialogTurn, Session } from '../types/flow-chat';

export type AgentCompanionTaskState =
  | 'running'
  | 'waiting'
  | 'attention'
  | 'completed'
  | 'error'
  | 'interrupted';

export interface AgentCompanionTaskStatus {
  sessionId: string;
  title: string;
  mood: ChatInputPetMood;
  state: AgentCompanionTaskState;
  labelKey: string;
  defaultLabel: string;
  startedAt: number;
  updatedAt: number;
}

export interface AgentCompanionActivityPayload {
  mood: ChatInputPetMood;
  tasks: AgentCompanionTaskStatus[];
}

const EMPTY_ACTIVITY: AgentCompanionActivityPayload = {
  mood: 'rest',
  tasks: [],
};

const taskOrderBySessionId = new Map<string, number>();
let nextTaskOrder = 0;
const TRANSIENT_TURN_STATUSES = new Set<DialogTurn['status']>([
  'pending',
  'image_analyzing',
  'processing',
  'finishing',
  'cancelling',
]);

function ensureTaskOrder(sessionId: string): number {
  const existingOrder = taskOrderBySessionId.get(sessionId);
  if (existingOrder !== undefined) {
    return existingOrder;
  }

  const order = nextTaskOrder;
  nextTaskOrder += 1;
  taskOrderBySessionId.set(sessionId, order);
  return order;
}

function pruneTaskOrder(activeTasks: AgentCompanionTaskStatus[]): void {
  const activeSessionIds = new Set(activeTasks.map(task => task.sessionId));
  Array.from(taskOrderBySessionId.keys()).forEach(sessionId => {
    if (!activeSessionIds.has(sessionId)) {
      taskOrderBySessionId.delete(sessionId);
    }
  });
}

function sessionTitle(session: Session): string {
  return session.title?.trim() || 'Session';
}

function trackedDialogTurn(
  session: Session,
  snapshot: SessionStateMachine | null,
): DialogTurn | undefined {
  const trackedTurnId = snapshot?.context.currentDialogTurnId;
  if (trackedTurnId) {
    return session.dialogTurns.find(turn => turn.id === trackedTurnId);
  }

  return session.dialogTurns[session.dialogTurns.length - 1];
}

function hasActiveTrackedTurn(
  session: Session,
  snapshot: SessionStateMachine | null,
): boolean {
  if (!snapshot) {
    return false;
  }

  const turn = trackedDialogTurn(session, snapshot);
  return !!turn && TRANSIENT_TURN_STATUSES.has(turn.status);
}

function runningLabel(snapshot: SessionStateMachine | null): {
  state: AgentCompanionTaskState;
  labelKey: string;
  defaultLabel: string;
} {
  switch (snapshot?.context.processingPhase) {
    case ProcessingPhase.THINKING:
      return {
        state: 'running',
        labelKey: 'agentCompanion.activity.thinking',
        defaultLabel: 'Thinking',
      };
    case ProcessingPhase.TOOL_CALLING:
      return {
        state: 'waiting',
        labelKey: 'agentCompanion.activity.usingTools',
        defaultLabel: 'Using tools',
      };
    case ProcessingPhase.TOOL_CONFIRMING:
      return {
        state: 'attention',
        labelKey: 'agentCompanion.activity.waitingApproval',
        defaultLabel: 'Waiting for approval',
      };
    case ProcessingPhase.STREAMING:
      return {
        state: 'running',
        labelKey: 'agentCompanion.activity.writing',
        defaultLabel: 'Writing',
      };
    case ProcessingPhase.COMPACTING:
      return {
        state: 'running',
        labelKey: 'agentCompanion.activity.compacting',
        defaultLabel: 'Compacting context',
      };
    case ProcessingPhase.FINALIZING:
      return {
        state: 'running',
        labelKey: 'agentCompanion.activity.finishing',
        defaultLabel: 'Finishing',
      };
    case ProcessingPhase.STARTING:
      return {
        state: 'running',
        labelKey: 'agentCompanion.activity.starting',
        defaultLabel: 'Starting',
      };
    default:
      return {
        state: 'running',
        labelKey: 'agentCompanion.activity.working',
        defaultLabel: 'Working',
      };
  }
}

function attentionTask(session: Session): AgentCompanionTaskStatus | null {
  if (session.needsUserAttention === 'ask_user') {
    return {
      sessionId: session.sessionId,
      title: sessionTitle(session),
      mood: 'waiting',
      state: 'attention',
      labelKey: 'agentCompanion.activity.needsInput',
      defaultLabel: 'Needs input',
      startedAt: session.lastActiveAt || session.updatedAt || session.createdAt,
      updatedAt: session.updatedAt || session.lastActiveAt || session.createdAt,
    };
  }

  if (session.needsUserAttention === 'tool_confirm') {
    return {
      sessionId: session.sessionId,
      title: sessionTitle(session),
      mood: 'waiting',
      state: 'attention',
      labelKey: 'agentCompanion.activity.needsApproval',
      defaultLabel: 'Needs approval',
      startedAt: session.lastActiveAt || session.updatedAt || session.createdAt,
      updatedAt: session.updatedAt || session.lastActiveAt || session.createdAt,
    };
  }

  return null;
}

function completionTask(session: Session): AgentCompanionTaskStatus | null {
  if (!session.hasUnreadCompletion) {
    return null;
  }

  const base = {
    sessionId: session.sessionId,
    title: sessionTitle(session),
    mood: 'rest' as ChatInputPetMood,
    startedAt: session.lastFinishedAt || session.updatedAt || session.lastActiveAt || session.createdAt,
    updatedAt: session.lastFinishedAt || session.updatedAt || session.lastActiveAt || session.createdAt,
  };

  if (session.hasUnreadCompletion === 'completed') {
    return {
      ...base,
      state: 'completed',
      labelKey: 'agentCompanion.activity.completed',
      defaultLabel: 'Completed',
    };
  }

  if (session.hasUnreadCompletion === 'interrupted') {
    return {
      ...base,
      state: 'interrupted',
      labelKey: 'agentCompanion.activity.interrupted',
      defaultLabel: 'Interrupted',
    };
  }

  return {
    ...base,
    state: 'error',
    labelKey: 'agentCompanion.activity.failed',
    defaultLabel: 'Failed',
  };
}

function taskStableOrder(task: AgentCompanionTaskStatus): number {
  return ensureTaskOrder(task.sessionId);
}

function aggregateMood(tasks: AgentCompanionTaskStatus[]): ChatInputPetMood {
  if (tasks.some(task => task.mood === 'waiting')) {
    return 'waiting';
  }
  if (tasks.some(task => task.mood === 'analyzing')) {
    return 'analyzing';
  }
  if (tasks.some(task => task.mood === 'working')) {
    return 'working';
  }
  return 'rest';
}

export function buildAgentCompanionActivity(): AgentCompanionActivityPayload {
  const sessions = Array.from(FlowChatStore.getInstance().getState().sessions.values())
    .filter(session => !session.isTransient);
  const tasks: AgentCompanionTaskStatus[] = [];

  sessions.forEach(session => {
    const snapshot = stateMachineManager.getSnapshot(session.sessionId);
    const mood = hasActiveTrackedTurn(session, snapshot)
      ? deriveChatInputPetMood(snapshot)
      : 'rest';

    if (mood !== 'rest') {
      const label = runningLabel(snapshot);
      tasks.push({
        sessionId: session.sessionId,
        title: sessionTitle(session),
        mood,
        state: label.state,
        labelKey: label.labelKey,
        defaultLabel: label.defaultLabel,
        startedAt: snapshot?.context.stats.startTime || session.lastActiveAt || session.updatedAt || session.createdAt,
        updatedAt: snapshot?.context.lastUpdateTime || session.updatedAt || session.lastActiveAt || session.createdAt,
      });
      return;
    }

    const attention = attentionTask(session);
    if (attention) {
      tasks.push(attention);
      return;
    }

    const completion = completionTask(session);
    if (completion) {
      tasks.push(completion);
    }
  });

  if (!tasks.length) {
    pruneTaskOrder(tasks);
    return EMPTY_ACTIVITY;
  }

  [...tasks]
    .sort((a, b) => a.startedAt - b.startedAt)
    .forEach(task => {
      ensureTaskOrder(task.sessionId);
    });
  pruneTaskOrder(tasks);

  const sortedTasks = tasks
    .sort((a, b) => taskStableOrder(a) - taskStableOrder(b))
    .slice(0, 4);

  return {
    mood: aggregateMood(sortedTasks),
    tasks: sortedTasks,
  };
}

export function subscribeAgentCompanionActivity(
  listener: (payload: AgentCompanionActivityPayload) => void,
): () => void {
  const emitCurrent = () => {
    listener(buildAgentCompanionActivity());
  };

  const unsubscribeStore = FlowChatStore.getInstance().subscribe(emitCurrent);
  const unsubscribeMachines = stateMachineManager.subscribeGlobal(emitCurrent);

  emitCurrent();

  return () => {
    unsubscribeStore();
    unsubscribeMachines();
  };
}
