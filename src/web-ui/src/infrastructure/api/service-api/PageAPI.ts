import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export type PageVisibility = 'private' | 'relay' | 'public';

export interface PageInfo {
  slug: string;
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

  async listVersions(slug: string): Promise<PageVersionInfo[]> {
    try {
      return await api.invoke<PageVersionInfo[]>('page_list_versions', { request: { slug } });
    } catch (error) {
      throw createTauriCommandError('page_list_versions', error, { slug });
    }
  }

  async createOpenLink(slug: string, versionId?: string | null): Promise<PageOpenLink> {
    const request = { slug, version_id: versionId || null };
    try {
      return await api.invoke<PageOpenLink>('page_create_open_link', { request });
    } catch (error) {
      throw createTauriCommandError('page_create_open_link', error, request);
    }
  }

  async deploy(slug: string, versionId: string): Promise<PageInfo> {
    const request = { slug, version_id: versionId };
    try {
      return await api.invoke<PageInfo>('page_deploy', { request });
    } catch (error) {
      throw createTauriCommandError('page_deploy', error, request);
    }
  }

  async update(slug: string, changes: { visibility?: PageVisibility; title?: string }): Promise<PageInfo> {
    const request = { slug, ...changes };
    try {
      return await api.invoke<PageInfo>('page_update', { request });
    } catch (error) {
      throw createTauriCommandError('page_update', error, request);
    }
  }

  async deleteVersion(slug: string, versionId: string): Promise<void> {
    const request = { slug, version_id: versionId };
    try {
      await api.invoke<void>('page_delete_version', { request });
    } catch (error) {
      throw createTauriCommandError('page_delete_version', error, request);
    }
  }

  async unpublish(slug: string): Promise<void> {
    try {
      await api.invoke<void>('page_unpublish', { request: { slug } });
    } catch (error) {
      throw createTauriCommandError('page_unpublish', error, { slug });
    }
  }

  async deletePage(slug: string): Promise<void> {
    try {
      await api.invoke<void>('page_delete', { request: { slug } });
    } catch (error) {
      throw createTauriCommandError('page_delete', error, { slug });
    }
  }
}

export const pageAPI = new PageAPI();
