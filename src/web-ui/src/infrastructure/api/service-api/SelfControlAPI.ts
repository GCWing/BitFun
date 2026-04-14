

import { api } from './ApiClient';

export interface SubmitSelfControlResponseRequest {
  requestId: string;
  success: boolean;
  result?: string;
  error?: string;
}

export class SelfControlAPI {
  static async submitSelfControlResponse(request: SubmitSelfControlResponseRequest): Promise<void> {
    return api.invoke('submit_self_control_response', { request });
  }
}
