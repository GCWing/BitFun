import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export type PageVisibility = 'private' | 'relay' | 'public';

export interface PageInfo {
  slug: string;
  generation: string;
  visibility: PageVisibility;
  title: string;
  file_count: number;
  total_bytes: number;
  created_at: number;
  updated_at: number;
  url_path: string;
  preview_url_path?: string | null;
  deployed_version_id?: string | null;
}

export interface PageVersionInfo {
  generation: string;
  version_id: string;
  title: string;
  file_count: number;
  total_bytes: number;
  has_worker: boolean;
  note: string;
  created_at: number;
  deployed: boolean;
  preview_url_path: string;
}

export interface PageOpenLink {
  open_url: string;
  page_url: string;
  expires_in_seconds: number;
}

class PageAPI {
  async listPages(): Promise<PageInfo[]> {
    try {
      return await api.invoke<PageInfo[]>('page_list');
    } catch (error) {
      throw createTauriCommandError('page_list', error);
    }
  }

  async listVersions(slug: string, generation: string): Promise<PageVersionInfo[]> {
    try {
      return await api.invoke<PageVersionInfo[]>('page_list_versions', { request: { slug, generation } });
    } catch (error) {
      throw createTauriCommandError('page_list_versions', error, { slug, generation });
    }
  }

  async createOpenLink(slug: string, generation: string, versionId?: string | null): Promise<PageOpenLink> {
    const request = { slug, generation, version_id: versionId || null };
    try {
      return await api.invoke<PageOpenLink>('page_create_open_link', { request });
    } catch (error) {
      throw createTauriCommandError('page_create_open_link', error, request);
    }
  }

  async deploy(slug: string, generation: string, versionId: string): Promise<PageInfo> {
    const request = { slug, generation, version_id: versionId };
    try {
      return await api.invoke<PageInfo>('page_deploy', { request });
    } catch (error) {
      throw createTauriCommandError('page_deploy', error, request);
    }
  }

  async update(slug: string, generation: string, changes: { visibility?: PageVisibility; title?: string }): Promise<PageInfo> {
    const request = { slug, generation, ...changes };
    try {
      return await api.invoke<PageInfo>('page_update', { request });
    } catch (error) {
      throw createTauriCommandError('page_update', error, request);
    }
  }

  async deleteVersion(slug: string, generation: string, versionId: string): Promise<void> {
    const request = { slug, generation, version_id: versionId };
    try {
      await api.invoke<void>('page_delete_version', { request });
    } catch (error) {
      throw createTauriCommandError('page_delete_version', error, request);
    }
  }

  async unpublish(slug: string, generation: string): Promise<void> {
    try {
      await api.invoke<void>('page_unpublish', { request: { slug, generation } });
    } catch (error) {
      throw createTauriCommandError('page_unpublish', error, { slug, generation });
    }
  }

  async deletePage(slug: string, generation: string): Promise<void> {
    try {
      await api.invoke<void>('page_delete', { request: { slug, generation } });
    } catch (error) {
      throw createTauriCommandError('page_delete', error, { slug, generation });
    }
  }
}

export const pageAPI = new PageAPI();
