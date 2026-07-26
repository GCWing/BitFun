import { CreateSessionOptions, RemoteCommand, RemoteImageContext, RemoteQuestionAnswerPayload } from '../model/RemoteModels';

export class RemoteCommandFactory {
  static getWorkspaceInfo(): RemoteCommand {
    return { cmd: 'get_workspace_info' };
  }

  static listRecentWorkspaces(): RemoteCommand {
    return { cmd: 'list_recent_workspaces' };
  }

  static setWorkspace(path: string): RemoteCommand {
    return {
      cmd: 'set_workspace',
      path
    };
  }

  static listAssistants(): RemoteCommand {
    return { cmd: 'list_assistants' };
  }

  static setAssistant(path: string): RemoteCommand {
    return {
      cmd: 'set_assistant',
      path
    };
  }

  static listSessions(workspacePath: string, limit: number, offset: number, query: string): RemoteCommand {
    const command: RemoteCommand = {
      cmd: 'list_sessions',
      workspace_path: workspacePath,
      limit,
      offset
    };
    if (query.trim().length > 0) {
      command.query = query.trim();
    }
    return command;
  }

  static createSession(options: CreateSessionOptions, workspacePath: string): RemoteCommand {
    const defaultTitle = `${options.agentType === 'cowork' ? 'Remote Cowork' : 'Remote Code'} Session`;
    return {
      cmd: 'create_session',
      agent_type: options.agentType,
      session_name: options.title.trim() || defaultTitle,
      workspace_path: workspacePath
    };
  }

  static sendMessage(sessionId: string, text: string, agentType?: string, imageContexts?: RemoteImageContext[]): RemoteCommand {
    const command: RemoteCommand = {
      cmd: 'send_message',
      session_id: sessionId,
      content: text
    };
    if (agentType && agentType.length > 0) {
      command.agent_type = agentType;
    }
    if (imageContexts && imageContexts.length > 0) {
      command.image_contexts = imageContexts;
    }
    return command;
  }

  static getSessionMessages(sessionId: string): RemoteCommand {
    return {
      cmd: 'get_session_messages',
      session_id: sessionId
    };
  }

  static getModelCatalog(sessionId?: string): RemoteCommand {
    const command: RemoteCommand = { cmd: 'get_model_catalog' };
    if (sessionId && sessionId.length > 0) {
      command.session_id = sessionId;
    }
    return command;
  }

  static setSessionModel(sessionId: string, modelId: string): RemoteCommand {
    return {
      cmd: 'set_session_model',
      session_id: sessionId,
      model_id: modelId
    };
  }

  static pollSession(
    sessionId: string,
    sinceVersion: number,
    knownMessageCount: number,
    knownModelCatalogVersion: number
  ): RemoteCommand {
    return {
      cmd: 'poll_session',
      session_id: sessionId,
      since_version: sinceVersion,
      known_msg_count: knownMessageCount,
      known_model_catalog_version: knownModelCatalogVersion
    };
  }

  static cancelTask(sessionId: string, turnId?: string): RemoteCommand {
    const command: RemoteCommand = {
      cmd: 'cancel_task',
      session_id: sessionId
    };
    if (turnId && turnId.length > 0) {
      command.turn_id = turnId;
    }
    return command;
  }

  static renameSession(sessionId: string, title: string): RemoteCommand {
    return {
      cmd: 'update_session_title',
      session_id: sessionId,
      title
    };
  }

  static deleteSession(sessionId: string): RemoteCommand {
    return {
      cmd: 'delete_session',
      session_id: sessionId
    };
  }

  static confirmTool(toolId: string): RemoteCommand {
    return {
      cmd: 'confirm_tool',
      tool_id: toolId
    };
  }

  static rejectTool(toolId: string, reason: string): RemoteCommand {
    return {
      cmd: 'reject_tool',
      tool_id: toolId,
      reason
    };
  }

  static cancelTool(toolId: string, reason: string): RemoteCommand {
    return {
      cmd: 'cancel_tool',
      tool_id: toolId,
      reason
    };
  }

  static answerQuestion(toolId: string, answers: RemoteQuestionAnswerPayload): RemoteCommand {
    return {
      cmd: 'answer_question',
      tool_id: toolId,
      answers
    };
  }

  static getFileInfo(path: string, sessionId?: string): RemoteCommand {
    const command: RemoteCommand = {
      cmd: 'get_file_info',
      path
    };
    if (sessionId && sessionId.length > 0) {
      command.session_id = sessionId;
    }
    return command;
  }

  static readFileChunk(path: string, offset: number, limit: number, sessionId?: string): RemoteCommand {
    const command: RemoteCommand = {
      cmd: 'read_file_chunk',
      path,
      offset,
      limit
    };
    if (sessionId && sessionId.length > 0) {
      command.session_id = sessionId;
    }
    return command;
  }

  static ping(): RemoteCommand {
    return { cmd: 'ping' };
  }
}
