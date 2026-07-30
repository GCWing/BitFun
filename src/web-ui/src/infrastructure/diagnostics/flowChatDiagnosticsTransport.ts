import { isTauriRuntime } from '@/infrastructure/runtime';

export interface FlowChatDiagnosticTransportEntry {
  sequence: number;
  timestamp: string;
  performanceTimeMs: number;
  hypothesis: string;
  location: string;
  message: string;
  data?: Record<string, unknown>;
}

export async function appendFlowChatDiagnosticEntries(
  entries: FlowChatDiagnosticTransportEntry[],
): Promise<void> {
  if (entries.length === 0 || !isTauriRuntime()) {
    return;
  }

  const { invoke } = await import('@tauri-apps/api/core');
  await invoke<number>('append_flow_chat_diagnostics', {
    request: { entries },
  });
}
