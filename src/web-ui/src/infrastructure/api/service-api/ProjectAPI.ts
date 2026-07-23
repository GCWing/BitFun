 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';


export class ProjectAPI {
   
  async analyzeProject(path: string, options?: any): Promise<any> {
    try {
      return await api.invoke('analyze_project', { 
        request: { path, options } 
      });
    } catch (error) {
      throw createTauriCommandError('analyze_project', error, { path, options });
    }
  }

   
  async getProjectStructure(path: string): Promise<any> {
    try {
      return await api.invoke('get_project_structure', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('get_project_structure', error, { path });
    }
  }

   
  async getDependencyGraph(path: string): Promise<any> {
    try {
      return await api.invoke('get_dependency_graph', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('get_dependency_graph', error, { path });
    }
  }

   
  async searchCode(query: string, options?: any): Promise<any[]> {
    try {
      return await api.invoke('search_code', { 
        request: { query, options } 
      });
    } catch (error) {
      throw createTauriCommandError('search_code', error, { query, options });
    }
  }

   
  async clearProjectCache(workspacePath: string): Promise<void> {
    try {
      await api.invoke('clear_project_cache', { 
        request: { workspacePath } 
      });
    } catch (error) {
      throw createTauriCommandError('clear_project_cache', error, { workspacePath });
    }
  }
}


export const projectAPI = new ProjectAPI();