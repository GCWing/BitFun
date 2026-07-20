/**
 * BitFun Page API — Save Version → Deploy on the account relay.
 */

import { api } from './ApiClient';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('BitFunPageAPI');

export type PageVisibility = 'private' | 'relay' | 'public';

export interface PageInfo {
  slug: string;
  visibility: PageVisibility | string;
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

export interface PageSaveVersionResult {
  slug: string;
  visibility: string;
  title: string;
  version_id: string;
  file_count: number;
  total_bytes: number;
  has_worker: boolean;
  preview_url_path: string;
  deployed: boolean;
}

/** @deprecated Prefer PageSaveVersionResult; kept for older callers. */
export type PagePublishResult = PageSaveVersionResult;

export interface PageSaveVersionRequest {
  directory: string;
  slug: string;
  visibility: PageVisibility;
  title?: string;
  note?: string;
}

/** @deprecated Prefer PageSaveVersionRequest. */
export type PagePublishRequest = PageSaveVersionRequest;

export interface PageUpdateRequest {
  slug: string;
  visibility?: PageVisibility;
  title?: string;
}

export interface PageDeployRequest {
  slug: string;
  version_id: string;
}

class BitFunPageAPI {
  async saveVersion(request: PageSaveVersionRequest): Promise<PageSaveVersionResult> {
    try {
      return await api.invoke<PageSaveVersionResult>('page_save_version', { request });
    } catch (e) {
      log.warn('page_save_version failed', e);
      throw e;
    }
  }

  /** Legacy alias: saves a version only (does not deploy). */
  async publish(request: PagePublishRequest): Promise<PagePublishResult> {
    return this.saveVersion(request);
  }

  async list(): Promise<PageInfo[]> {
    try {
      return await api.invoke<PageInfo[]>('page_list', {});
    } catch (e) {
      log.warn('page_list failed', e);
      throw e;
    }
  }

  async listVersions(slug: string): Promise<PageVersionInfo[]> {
    try {
      return await api.invoke<PageVersionInfo[]>('page_list_versions', {
        request: { slug },
      });
    } catch (e) {
      log.warn('page_list_versions failed', e);
      throw e;
    }
  }

  async deploy(request: PageDeployRequest): Promise<PageInfo> {
    try {
      return await api.invoke<PageInfo>('page_deploy', { request });
    } catch (e) {
      log.warn('page_deploy failed', e);
      throw e;
    }
  }

  async deleteVersion(slug: string, versionId: string): Promise<void> {
    try {
      await api.invoke('page_delete_version', {
        request: { slug, version_id: versionId },
      });
    } catch (e) {
      log.warn('page_delete_version failed', e);
      throw e;
    }
  }

  async update(request: PageUpdateRequest): Promise<PageInfo> {
    try {
      return await api.invoke<PageInfo>('page_update', { request });
    } catch (e) {
      log.warn('page_update failed', e);
      throw e;
    }
  }

  async unpublish(slug: string): Promise<void> {
    try {
      await api.invoke('page_unpublish', { request: { slug } });
    } catch (e) {
      log.warn('page_unpublish failed', e);
      throw e;
    }
  }
}

export const bitFunPageAPI = new BitFunPageAPI();

/** Client-side slug check matching relay rules: `^[a-z0-9][a-z0-9-]{0,63}$` */
export function isValidPageSlug(slug: string): boolean {
  return /^[a-z0-9][a-z0-9-]{0,63}$/.test(slug);
}
