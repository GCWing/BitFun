export interface RemoteDescriptor {
  relayUrl: string;
  roomId: string;
  publicKey: string;
}

export interface PairState {
  descriptor?: RemoteDescriptor;
  connected: boolean;
  deviceId: string;
  userId: string;
  message: string;
}

export class EncryptedPayload {
  encrypted_data: string = '';
  nonce: string = '';
}

export interface WorkspaceInfo {
  path: string;
  name: string;
  hasWorkspace: boolean;
  gitBranch: string;
  workspaceKind: string;
  assistantId?: string;
}

export interface InitialSyncResult {
  workspace: WorkspaceInfo;
  sessions: RemoteSession[];
  hasMoreSessions: boolean;
  authenticatedUserId: string;
}

export interface RecentWorkspaceEntry {
  path: string;
  name: string;
  lastOpened: string;
  workspaceKind: string;
}

export interface RemoteSession {
  id: string;
  title: string;
  agentType: string;
  status: string;
  updatedAt: string;
  createdAt: string;
  messageCount: number;
  workspacePath?: string;
  workspaceName?: string;
}

export interface ChatMessage {
  id: string;
  role: string;
  text: string;
  status: string;
  renderVersion?: number;
  turnId?: string;
  detail?: string;
  timestamp?: string;
  thinking?: string;
  tools?: RemoteToolStatusResponse[];
  items?: ChatMessageItemResponse[];
  images?: ImageAttachment[];
}

export interface SessionSummary {
  sessionId: string;
  title: string;
  workspacePath: string;
  agentType: string;
  initialTurnId?: string;
}

export interface CreateSessionOptions {
  agentType: string;
  title: string;
  instruction: string;
}

export interface PairRequest {
  public_key: string;
  device_id: string;
  device_name: string;
}

export interface ChallengeCommand {
  challenge_echo: Object;
  device_id: string;
  device_name: string;
  mobile_install_id: string;
  user_id: string;
}

export interface RemoteCommand {
  cmd: string;
  _request_id?: string;
  session_id?: string;
  content?: string;
  workspace_path?: string;
  path?: string;
  agent_type?: string;
  session_name?: string;
  title?: string;
  limit?: number;
  offset?: number;
  query?: string;
  before_message_id?: string;
  since_version?: number;
  known_msg_count?: number;
  known_model_catalog_version?: number;
  turn_id?: string;
  tool_id?: string;
  model_id?: string;
  reason?: string;
  answers?: Object;
  image_contexts?: RemoteImageContext[];
  images?: ImageAttachment[];
}

export interface PairChallengeResponse {
  challenge: Object;
}

export interface CommandStatusResponse {
  resp?: string;
  message?: string;
}

export interface WorkspaceInfoResponse extends CommandStatusResponse {
  has_workspace?: boolean;
  workspace_path?: string;
  workspace_name?: string;
  path?: string;
  project_name?: string;
  git_branch?: string;
  workspace_kind?: string;
  assistant_id?: string;
}

export interface RecentWorkspaceEntryResponse {
  path?: string;
  name?: string;
  last_opened?: string | number;
  lastOpened?: string | number;
  updated_at?: string | number;
  updatedAt?: string | number;
  workspace_kind?: string;
}

export interface RecentWorkspaceListResponse extends CommandStatusResponse {
  workspaces?: RecentWorkspaceEntryResponse[];
}

export interface SetWorkspaceResponse extends CommandStatusResponse {
  success?: boolean;
  path?: string;
  project_name?: string;
  error?: string;
}

export interface AssistantEntry {
  path: string;
  name: string;
  assistant_id?: string;
}

export interface AssistantListResponse extends CommandStatusResponse {
  assistants?: AssistantEntry[];
}

export interface SetAssistantResponse extends CommandStatusResponse {
  success?: boolean;
  path?: string;
  name?: string;
  error?: string;
}

export interface SessionItemResponse {
  id?: string;
  session_id?: string;
  title?: string;
  name?: string;
  agent_type?: string;
  status?: string;
  updated_at?: string | number;
  updatedAt?: string | number;
  updated?: string | number;
  updated_time?: string | number;
  update_time?: string | number;
  modified_at?: string | number;
  last_active_at?: string | number;
  last_activity_at?: string | number;
  last_message_at?: string | number;
  last_interaction_at?: string | number;
  lastUpdated?: string | number;
  lastActivityAt?: string | number;
  lastMessageAt?: string | number;
  created_at?: string | number;
  createdAt?: string | number;
  created?: string | number;
  created_time?: string | number;
  create_time?: string | number;
  inserted_at?: string | number;
  started_at?: string | number;
  start_time?: string | number;
  timestamp?: string | number;
  message_count?: number;
  workspace_path?: string;
  workspace_name?: string;
}

export interface SessionListResponse extends CommandStatusResponse {
  sessions?: SessionItemResponse[];
  has_more?: boolean;
}

export interface InitialSyncResponse extends CommandStatusResponse {
  has_workspace?: boolean;
  path?: string;
  project_name?: string;
  git_branch?: string;
  workspace_kind?: string;
  assistant_id?: string;
  sessions?: SessionItemResponse[];
  has_more_sessions?: boolean;
  authenticated_user_id?: string;
}

export interface SessionListResult {
  sessions: RemoteSession[];
  hasMore: boolean;
}

export interface CreateSessionResponse extends CommandStatusResponse {
  session_id?: string;
  id?: string;
  title?: string;
}

export interface SendMessageResponse extends CommandStatusResponse {
  turn_id?: string;
}

export interface RemoteModelConfig {
  id: string;
  name: string;
  provider: string;
  base_url: string;
  model_name: string;
  context_window?: number;
  enabled: boolean;
  capabilities: string[];
  enable_thinking_process?: boolean;
  reasoning_mode?: string;
  reasoning_effort?: string;
}

export interface RemoteDefaultModels {
  primary?: string;
  fast?: string;
  search?: string;
  image_understanding?: string;
}

export interface RemoteModelCatalog {
  version: number;
  models: RemoteModelConfig[];
  default_models: RemoteDefaultModels;
  session_model_id?: string;
}

export interface ModelCatalogResponse extends CommandStatusResponse {
  catalog?: RemoteModelCatalog;
}

export interface SetSessionModelResponse extends CommandStatusResponse {
  session_id?: string;
  model_id?: string;
}

export interface RemoteToolStatusResponse {
  id?: string;
  name?: string;
  status?: string;
  duration_ms?: number;
  start_ms?: number;
  input_preview?: string;
  tool_input?: Object;
  stdout?: string;
  stderr?: string;
  tool_output?: Object;
  result_preview?: string;
  error_preview?: string;
  exit_code?: number;
}

export interface RemoteQuestionAnswerPayload {
  answer: string;
  '0': string;
}

export interface ChatMessageItemResponse {
  type?: string;
  content?: string;
  tool?: RemoteToolStatusResponse;
  is_subagent?: boolean;
  subItems?: ChatMessageItemResponse[];
}

export interface ImageAttachment {
  name: string;
  data_url: string;
}

export interface SelectedImageAttachment extends ImageAttachment {
  id: string;
  uri: string;
  mime_type: string;
  size: number;
  original_size: number;
  compressed: boolean;
}

export interface RemoteImageContext {
  id: string;
  image_path?: string;
  data_url?: string;
  mime_type: string;
  metadata?: Object;
}

export interface ChatMessageResponse {
  id?: string;
  message_id?: string;
  role: string;
  content: string;
  timestamp?: string;
  metadata?: Object;
  thinking?: string;
  items?: ChatMessageItemResponse[];
  tools?: RemoteToolStatusResponse[];
  images?: ImageAttachment[];
}

export interface ActiveTurnSnapshotResponse {
  turn_id: string;
  status: string;
  text?: string;
  thinking?: string;
  tools?: RemoteToolStatusResponse[];
  round_index?: number;
  items?: ChatMessageItemResponse[];
}

export interface SessionMessagesResponse extends CommandStatusResponse {
  messages?: ChatMessageResponse[];
  has_more?: boolean;
}

export interface SessionMessagesResult {
  messages: ChatMessage[];
  hasMore: boolean;
}

export interface PollSessionResponse extends CommandStatusResponse {
  version: number;
  changed: boolean;
  session_state?: string;
  title?: string;
  new_messages?: ChatMessageResponse[];
  total_msg_count?: number;
  active_turn?: ActiveTurnSnapshotResponse;
  model_catalog?: RemoteModelCatalog;
}

export interface PollSessionResult {
  version: number;
  changed: boolean;
  sessionState: string;
  title: string;
  newMessages: ChatMessage[];
  totalMessageCount: number;
  activeTurn?: ChatMessage;
  modelCatalog?: RemoteModelCatalog;
}

export interface FileInfoResponse extends CommandStatusResponse {
  name?: string;
  size?: number;
  mime_type?: string;
}

export interface FileInfo {
  name: string;
  size: number;
  mimeType: string;
}

export interface ReadFileChunkResponse extends CommandStatusResponse {
  name?: string;
  chunk_base64?: string;
  offset?: number;
  chunk_size?: number;
  total_size?: number;
  mime_type?: string;
}

export interface ReadFileResult {
  name: string;
  contentBase64: string;
  mimeType: string;
  size: number;
}
