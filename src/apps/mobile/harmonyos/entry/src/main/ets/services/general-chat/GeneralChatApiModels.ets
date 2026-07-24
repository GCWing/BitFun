export interface GeneralChatCapabilityResponse {
  capabilities: string[];
}

export interface GeneralChatApiSession {
  id: string;
  title: string;
  status: string;
  created_at: string;
  updated_at: string;
  message_count: number;
}

export interface GeneralChatSessionPageResponse {
  sessions: GeneralChatApiSession[];
  next_cursor?: string;
}

export interface GeneralChatSessionResponse {
  session: GeneralChatApiSession;
}

export interface GeneralChatApiMessage {
  id: string;
  role: string;
  content: string;
  status: string;
  timestamp: string;
}

export interface GeneralChatMessagePageResponse {
  messages: GeneralChatApiMessage[];
  next_cursor?: string;
  version: number;
}

export interface GeneralChatTurnReceiptResponse {
  turn_id: string;
  accepted_at: string;
  version: number;
}

export interface GeneralChatCreateSessionRequest {
  title: string;
}

export interface GeneralChatRenameSessionRequest {
  title: string;
}

export interface GeneralChatSendTurnRequest {
  content: string;
  model: string;
}

export interface GeneralChatCancelTurnRequest {
  reason: string;
}

export interface GeneralChatApiTool {
  id: string;
  name: string;
  status: string;
  input_preview?: string;
  result_preview?: string;
  error_preview?: string;
}

export interface GeneralChatStreamEvent {
  event_id: string;
  version: number;
  session_id: string;
  turn_id: string;
  type: string;
  delta?: string;
  tool?: GeneralChatApiTool;
  error_code?: string;
  capability?: string;
}
