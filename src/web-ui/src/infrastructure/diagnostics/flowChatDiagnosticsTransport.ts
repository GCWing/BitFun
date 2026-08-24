import { isTauriRuntime } from '@/infrastructure/runtime';
import { api } from '@/infrastructure/api/service-api/ApiClient';

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

  await api.invoke<number>('append_flow_chat_diagnostics', {
    request: { entries },
  });
}
