/**
 * Git state manager - central state management for Git repositories
 * 
 * Single source of truth for all repository Git states
 * 
 * Design patterns:
 * - Singleton: global unique instance
 * - Observer: state change notifications
 * - Strategy: configurable refresh strategies
 * 
 * Features:
 * - Layered caching: basic/status/detailed three-tier data
 * - Visibility-aware: adjust refresh based on panel visibility
 * - Event-driven: auto refresh after Git operations
 * - Debounce/throttle: avoid frequent refreshes
 */

import { gitAPI } from '@/infrastructure/api';
import { gitEventService } from '../services/GitEventService';
import { globalEventBus } from '@/infrastructure/event-bus';
import {
  GitState,
  GitStateLayer,
  GitStateSubscriber,
  RefreshOptions,
  RefreshReason,
  CacheConfig,
  SubscribeOptions,
  createInitialGitState,
  compareStates,
  DEFAULT_CACHE_CONFIG,
  GitStateChangedEventData,
} from './types';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('GitStateManager');

interface SubscriberEntry {
  callback: GitStateSubscriber;
  options: SubscribeOptions;
}

interface PendingRefresh {
  layers: Set<GitStateLayer>;
  force: boolean;
  reason: RefreshReason;
  resolve: () => void;
  reject: (error: Error) => void;
}

export class GitStateManager {
  private static instance: GitStateManager;

  private states = new Map<string, GitState>();
  private subscribers = new Map<string, Set<SubscriberEntry>>();
  private visibleRepositories = new Set<string>();
  private refreshDebounceTimers = new Map<string, ReturnType<typeof setTimeout>>();
  private refreshLocks = new Map<string, Promise<void>>();
  private pendingRefreshes = new Map<string, PendingRefresh>();
  private cacheConfig: CacheConfig = { ...DEFAULT_CACHE_CONFIG };
  private readonly DEBOUNCE_DELAY = 100;
  private globalListenersInitialized = false;

  private constructor() {
    this.setupGlobalListeners();
  }

  static getInstance(): GitStateManager {
    if (!GitStateManager.instance) {
      GitStateManager.instance = new GitStateManager();
    }
    return GitStateManager.instance;
  }

  /**
   * Reset instance (for testing only)
   */
  static resetInstance(): void {
    if (GitStateManager.instance) {
      GitStateManager.instance.dispose();
      GitStateManager.instance = undefined as any;
    }
  }

  /**
   * Subscribe to repository state changes.
   * @returns Unsubscribe function.
   */
  subscribe(
    repositoryPath: string,
    callback: GitStateSubscriber,
    options: SubscribeOptions = {}
  ): () => void {
    const normalizedPath = this.normalizePath(repositoryPath);

    if (!this.subscribers.has(normalizedPath)) {
      this.subscribers.set(normalizedPath, new Set());
    }

    const entry: SubscriberEntry = { callback, options };
    this.subscribers.get(normalizedPath)!.add(entry);

    if (options.immediate !== false) {
      const currentState = this.states.get(normalizedPath);
      if (currentState) {
        callback(currentState, null, ['basic', 'status', 'detailed']);
      }
    }

    return () => {
      this.subscribers.get(normalizedPath)?.delete(entry);
    };
  }

  /**
   * Get current state synchronously (cached).
   */
  getState(repositoryPath: string): GitState | null {
    const normalizedPath = this.normalizePath(repositoryPath);
    return this.states.get(normalizedPath) || null;
  }

  /**
   * Get state or create an initial one.
   */
  getOrCreateState(repositoryPath: string): GitState {
    const normalizedPath = this.normalizePath(repositoryPath);
    let state = this.states.get(normalizedPath);

    if (!state) {
      state = createInitialGitState();
      this.states.set(normalizedPath, state);
    }

    return state;
  }

  /**
   * Request a refresh.
   */
  async refresh(
    repositoryPath: string,
    options: RefreshOptions = {}
  ): Promise<void> {
    const normalizedPath = this.normalizePath(repositoryPath);
    const {
      force = false,
      layers = ['basic', 'status'],
      silent = false,
      reason = 'manual',
    } = options;

    return this.enqueueRefresh(normalizedPath, layers, force, reason, silent);
  }

  /**
   * Track panel visibility for a repository.
   *
   * Note: this does not trigger a refresh. Refresh scheduling is owned by `useGitState`
   * to avoid duplicate refreshes.
   */
  setVisibility(repositoryPath: string, visible: boolean): void {
    const normalizedPath = this.normalizePath(repositoryPath);
    const wasVisible = this.visibleRepositories.has(normalizedPath);

    if (visible) {
      if (!wasVisible) {
        this.visibleRepositories.add(normalizedPath);
      }
    } else {
      if (wasVisible) {
        this.visibleRepositories.delete(normalizedPath);
      }
    }
  }

  /**
   * Whether the repository is currently visible.
   */
  isVisible(repositoryPath: string): boolean {
    return this.visibleRepositories.has(this.normalizePath(repositoryPath));
  }

  /**
   * Invalidate cache by resetting last refresh timestamps.
   */
  invalidateCache(
    repositoryPath: string,
    layers: GitStateLayer[] = ['basic', 'status', 'detailed']
  ): void {
    const normalizedPath = this.normalizePath(repositoryPath);
    const state = this.states.get(normalizedPath);

    if (state) {
      const newState = { ...state };
      for (const layer of layers) {
        newState.lastRefreshTime = {
          ...newState.lastRefreshTime,
          [layer]: 0,
        };
      }
      this.states.set(normalizedPath, newState);
    }
  }

  /**
   * Override cache TTL configuration.
   */
  setCacheConfig(config: Partial<CacheConfig>): void {
    this.cacheConfig = { ...this.cacheConfig, ...config };
  }

  /**
   * Cleanup resources (testing / shutdown).
   */
  dispose(): void {

    for (const timer of this.refreshDebounceTimers.values()) {
      clearTimeout(timer);
    }
    this.refreshDebounceTimers.clear();


    this.subscribers.clear();


    this.states.clear();

    this.visibleRepositories.clear();
  }

  // -------------------------------------------------------------------------

  // -------------------------------------------------------------------------

  /**
   * Enqueue a refresh request (debounced and merged).
   */
  private async enqueueRefresh(
    repositoryPath: string,
    layers: GitStateLayer[],
    force: boolean,
    reason: RefreshReason,
    silent: boolean
  ): Promise<void> {

    const existingTimer = this.refreshDebounceTimers.get(repositoryPath);
    if (existingTimer) {
      clearTimeout(existingTimer);
    }


    let pending = this.pendingRefreshes.get(repositoryPath);
    if (pending) {

      for (const layer of layers) {
        pending.layers.add(layer);
      }
      if (force) {
        pending.force = true;
      }
    } else {

      pending = {
        layers: new Set(layers),
        force,
        reason,
        resolve: () => {},
        reject: () => {},
      };
      this.pendingRefreshes.set(repositoryPath, pending);
    }


    return new Promise<void>((resolve, reject) => {
      const currentPending = this.pendingRefreshes.get(repositoryPath)!;
      const originalResolve = currentPending.resolve;
      const originalReject = currentPending.reject;

      currentPending.resolve = () => {
        originalResolve();
        resolve();
      };
      currentPending.reject = (error: Error) => {
        originalReject(error);
        reject(error);
      };


      const timer = setTimeout(() => {
        this.executePendingRefresh(repositoryPath, silent);
      }, this.DEBOUNCE_DELAY);

      this.refreshDebounceTimers.set(repositoryPath, timer);
    });
  }

  /**
   * Execute a pending refresh.
   */
  private async executePendingRefresh(
    repositoryPath: string,
    silent: boolean
  ): Promise<void> {
    const pending = this.pendingRefreshes.get(repositoryPath);
    if (!pending) return;

    this.pendingRefreshes.delete(repositoryPath);
    this.refreshDebounceTimers.delete(repositoryPath);

    const { layers, force, reason, resolve, reject } = pending;

    try {
      await this.doRefresh(
        repositoryPath,
        Array.from(layers),
        force,
        reason,
        silent
      );
      resolve();
    } catch (error) {
      reject(error as Error);
    }
  }

  /**
   * Execute an actual refresh.
   */
  private async doRefresh(
    repositoryPath: string,
    layers: GitStateLayer[],
    force: boolean,
    reason: RefreshReason,
    silent: boolean
  ): Promise<void> {
    const existingLock = this.refreshLocks.get(repositoryPath);
    if (existingLock && !force) {
      await existingLock;
      return;
    }


    let state = this.getOrCreateState(repositoryPath);
    const prevState = { ...state };


    const now = Date.now();
    const layersToRefresh = force
      ? layers
      : layers.filter((layer) => this.isCacheExpired(state, layer, now));

    if (layersToRefresh.length === 0) {
      return;
    }

    log.debug('Starting refresh', { repositoryPath, layersToRefresh, reason });

    const refreshPromise = (async () => {
      try {

        if (!silent) {
          state = this.updateState(repositoryPath, {
            isRefreshing: true,
            refreshingLayers: new Set(layersToRefresh),
            error: null,
          });
        }


        if (layersToRefresh.includes('basic') || layersToRefresh.includes('status')) {
          await this.refreshBasicAndStatus(repositoryPath);
        }

        if (layersToRefresh.includes('detailed')) {
          await this.refreshDetailed(repositoryPath);
        }


        state = this.getOrCreateState(repositoryPath);
        const newLastRefreshTime = { ...state.lastRefreshTime };
        for (const layer of layersToRefresh) {
          newLastRefreshTime[layer] = now;
        }

        this.updateState(repositoryPath, {
          lastRefreshTime: newLastRefreshTime,
          isRefreshing: false,
          refreshingLayers: new Set(),
        });


        const finalState = this.getState(repositoryPath)!;
        const comparison = compareStates(prevState, finalState);
        if (comparison.hasChanges) {
          this.notifySubscribers(repositoryPath, finalState, prevState, comparison.changedLayers);
        }


        this.emitStateChanged(repositoryPath, finalState, layersToRefresh, reason);

      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : i18nService.t('panels/git:errors.refreshFailed');
        log.error('Refresh failed', { repositoryPath, layersToRefresh, error });

        this.updateState(repositoryPath, {
          isRefreshing: false,
          refreshingLayers: new Set(),
          error: errorMessage,
        });

        throw error;
      } finally {
        this.refreshLocks.delete(repositoryPath);
      }
    })();

    this.refreshLocks.set(repositoryPath, refreshPromise);
    await refreshPromise;
  }

  /**
   * Refresh basic + status layers.
   */
  private async refreshBasicAndStatus(repositoryPath: string): Promise<void> {
    try {
      const isRepo = await gitAPI.isGitRepository(repositoryPath);

      if (!isRepo) {
        this.updateState(repositoryPath, {
          isRepository: false,
          currentBranch: null,
          ahead: 0,
          behind: 0,
          hasChanges: false,
          staged: [],
          unstaged: [],
          untracked: [],
          conflicts: [],
        });
        return;
      }

      const status = await gitAPI.getStatus(repositoryPath);

      const hasChanges =
        (status.staged?.length || 0) > 0 ||
        (status.unstaged?.length || 0) > 0 ||
        (status.untracked?.length || 0) > 0;

      this.updateState(repositoryPath, {
        isRepository: true,
        currentBranch: status.current_branch || null,
        ahead: status.ahead || 0,
        behind: status.behind || 0,
        hasChanges,
        staged: status.staged || [],
        unstaged: status.unstaged || [],
        untracked: status.untracked || [],
        conflicts: status.conflicts || [],
      });

    } catch (error) {
      log.error('Failed to refresh basic and status', { repositoryPath, error });
      throw error;
    }
  }

  /**
   * Refresh detailed layer (branches/commits).
   */
  private async refreshDetailed(repositoryPath: string): Promise<void> {
    const state = this.getState(repositoryPath);
    if (!state?.isRepository) return;

    try {

      const [branches, commits] = await Promise.all([
        gitAPI.getBranches(repositoryPath, true).catch(() => []),
        gitAPI.getCommits(repositoryPath, { maxCount: 20 }).catch(() => []),
      ]);

      this.updateState(repositoryPath, {
        branches: branches.map((b: any) => ({
          name: b.name,
          current: !!b.current,
          remote: !!b.remote,
          remoteName: b.remoteName,
          upstream: b.upstream,
          ahead: b.ahead ?? 0,
          behind: b.behind ?? 0,
          lastCommit: b.lastCommit,
          lastCommitDate: b.lastCommitDate ? new Date(b.lastCommitDate) : undefined,
        })),
        commits: commits.map((c) => ({
          hash: c.hash,
          shortHash: c.hash.substring(0, 7),
          message: c.message,
          author: c.author,
          authorEmail: '',
          date: new Date(c.date),
          parents: [],
        })),
      });

    } catch (error) {
      log.warn('Failed to refresh detailed info', { repositoryPath, error });
    }
  }

  // -------------------------------------------------------------------------

  // -------------------------------------------------------------------------

  /**
   * Update cached state for a repository.
   */
  private updateState(
    repositoryPath: string,
    partial: Partial<GitState>
  ): GitState {
    const currentState = this.getOrCreateState(repositoryPath);
    const newState: GitState = {
      ...currentState,
      ...partial,

      lastRefreshTime: partial.lastRefreshTime ?? currentState.lastRefreshTime,
      refreshingLayers: partial.refreshingLayers ?? currentState.refreshingLayers,
    };

    this.states.set(repositoryPath, newState);
    return newState;
  }

  /**
   * Notify subscribers.
   */
  private notifySubscribers(
    repositoryPath: string,
    state: GitState,
    prevState: GitState | null,
    changedLayers: GitStateLayer[]
  ): void {
    const entries = this.subscribers.get(repositoryPath);
    if (!entries || entries.size === 0) return;

    for (const entry of entries) {
      const { callback, options } = entry;


      if (options.layers && options.layers.length > 0) {
        const hasRelevantChange = changedLayers.some((layer) =>
          options.layers!.includes(layer)
        );
        if (!hasRelevantChange) {
          continue;
        }
      }

      try {
        callback(state, prevState, changedLayers);
      } catch (error) {
        log.error('Subscriber callback error', { repositoryPath, error });
      }
    }
  }

  /**
   * Emit state-changed events (EventBus + GitEventService).
   */
  private emitStateChanged(
    repositoryPath: string,
    state: GitState,
    changedLayers: GitStateLayer[],
    reason: RefreshReason
  ): void {
    const eventData: GitStateChangedEventData = {
      repositoryPath,
      state,
      changedLayers,
      reason,
      timestamp: Date.now(),
    };


    globalEventBus.emit('git:state:changed', eventData);


    gitEventService.emit('status:changed', {
      repositoryPath,
      status: {
        current_branch: state.currentBranch || '',
        staged: state.staged,
        unstaged: state.unstaged,
        untracked: state.untracked,
        ahead: state.ahead,
        behind: state.behind,
      },
      timestamp: new Date(),
    });
  }

  // -------------------------------------------------------------------------

  // -------------------------------------------------------------------------

  /**
   * Check whether the cache is expired for a layer.
   */
  private isCacheExpired(
    state: GitState,
    layer: GitStateLayer,
    now: number
  ): boolean {
    const lastRefresh = state.lastRefreshTime[layer];
    const ttl = this.cacheConfig[layer];

    if (ttl === Infinity) {

      return lastRefresh === 0;
    }

    return now - lastRefresh > ttl;
  }

  // -------------------------------------------------------------------------

  // -------------------------------------------------------------------------

  /**
   * Setup global event listeners once.
   */
  private setupGlobalListeners(): void {
    if (this.globalListenersInitialized) return;
    this.globalListenersInitialized = true;

    if (typeof window !== 'undefined') {
      window.addEventListener('focus', this.handleWindowFocus);
    }

    gitEventService.on('operation:completed', this.handleGitOperationCompleted);
    gitEventService.on('branch:changed', this.handleBranchChanged);
  }

  private handleWindowFocus = (): void => {
    for (const repoPath of this.visibleRepositories) {
      this.refresh(repoPath, {
        layers: ['basic', 'status'],
        reason: 'window-focus',
        silent: true,
      });
    }
  };

  private handleGitOperationCompleted = (event: any): void => {
    const { repositoryPath, operationType } = event.data;
    if (!repositoryPath) return;

    this.invalidateCache(repositoryPath, ['basic', 'status']);

    const detailedOps = ['commit', 'merge', 'rebase', 'cherry-pick', 'branch'];
    if (detailedOps.includes(operationType)) {
      this.invalidateCache(repositoryPath, ['detailed']);
    }

    this.refresh(repositoryPath, {
      layers: ['basic', 'status'],
      reason: 'operation',
      force: true,
    });
  };

  private handleBranchChanged = (event: any): void => {
    const { repositoryPath } = event.data;
    if (!repositoryPath) return;

    this.invalidateCache(repositoryPath, ['basic', 'status', 'detailed']);
    this.refresh(repositoryPath, {
      layers: ['basic', 'status', 'detailed'],
      reason: 'operation',
      force: true,
    });
  };

  // -------------------------------------------------------------------------

  // -------------------------------------------------------------------------

  /**
   * Normalize path separators for stable map keys.
   */
  private normalizePath(path: string): string {
    return path.replace(/\\/g, '/');
  }
}


export const gitStateManager = GitStateManager.getInstance();
