 

import { api } from './ApiClient';

 
export type MCPServerStatus = 
  | 'Uninitialized'
  | 'Starting'
  | 'Connected'
  | 'Healthy'
  | 'Reconnecting'
  | 'Failed'
  | 'Stopping'
  | 'Stopped';

 
export interface MCPServerInfo {
  id: string;
  name: string;
  status: string;
  serverType: string;
  enabled: boolean;
  autoStart: boolean;
}

 
export interface MCPResource {
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
  metadata?: Record<string, any>;
}

 
export interface MCPPrompt {
  name: string;
  description?: string;
  arguments?: Array<{
    name: string;
    description?: string;
    required: boolean;
  }>;
}

 
export interface MCPTool {
  name: string;
  description?: string;
  inputSchema: any;
}

 
export class MCPAPI {
   
  static async initializeServers(): Promise<void> {
    return api.invoke('initialize_mcp_servers');
  }

   
  static async initializeServersNonDestructive(): Promise<void> {
    return api.invoke('initialize_mcp_servers_non_destructive');
  }

   
  static async getServers(): Promise<MCPServerInfo[]> {
    return api.invoke('get_mcp_servers');
  }

   
  static async startServer(serverId: string): Promise<void> {
    return api.invoke('start_mcp_server', { serverId });
  }

   
  static async stopServer(serverId: string): Promise<void> {
    return api.invoke('stop_mcp_server', { serverId });
  }

   
  static async restartServer(serverId: string): Promise<void> {
    return api.invoke('restart_mcp_server', { serverId });
  }

   
  static async getServerStatus(serverId: string): Promise<string> {
    return api.invoke('get_mcp_server_status', { serverId });
  }

   
  static async loadMCPJsonConfig(): Promise<string> {
    return api.invoke('load_mcp_json_config');
  }

   
  static async saveMCPJsonConfig(jsonConfig: string): Promise<void> {
    return api.invoke('save_mcp_json_config', { jsonConfig });
  }
}

export default MCPAPI;
