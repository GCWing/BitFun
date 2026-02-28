/**
 * Branch quick switch overlay.
 * Shown when clicking the branch badge in NavPanel Git item.
 * Supports search and checkout.
 */

import React, { useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { GitBranch, Check, Loader2 } from 'lucide-react';
import { type GitBranch as GitBranchType } from '../../../../infrastructure/api/service-api/GitAPI';
import { useI18n } from '@/infrastructure/i18n';
import { gitService, gitEventService } from '../../../../tools/git/services';
import { gitStateManager } from '../../../../tools/git/state/GitStateManager';
import { notificationService } from '../../../../shared/notification-system/services/NotificationService';
import { createLogger } from '@/shared/utils/logger';
import './BranchQuickSwitch.scss';

const log = createLogger('BranchQuickSwitch');

export interface BranchQuickSwitchProps {
  isOpen: boolean;
  onClose: () => void;
  repositoryPath: string;
  currentBranch: string;
  anchorRef: React.RefObject<HTMLElement>;
  onSwitchSuccess?: (branchName: string) => void;
}

export const BranchQuickSwitch: React.FC<BranchQuickSwitchProps> = ({
  isOpen,
  onClose,
  repositoryPath,
  currentBranch,
  anchorRef,
  onSwitchSuccess
}) => {
  const { t } = useI18n('panels/git');
  const [branches, setBranches] = useState<GitBranchType[]>([]);
  const [searchTerm, setSearchTerm] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [isSwitching, setIsSwitching] = useState(false);
  const [switchingBranch, setSwitchingBranch] = useState<string | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [position, setPosition] = useState<{ top: number; left: number }>({ top: 0, left: 0 });

  const inputRef = useRef<HTMLInputElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const PANEL_WIDTH = 280;
  const PANEL_MARGIN = 12;

  // Position relative to anchor (NavPanel item)
  useEffect(() => {
    if (isOpen && anchorRef.current) {
      const rect = anchorRef.current.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;

      let left = rect.right + 8;
      if (left + PANEL_WIDTH > viewportWidth - PANEL_MARGIN) {
        left = rect.left - PANEL_WIDTH - 8;
      }
      left = Math.max(PANEL_MARGIN, left);

      const panelHeight = 320;
      let top = rect.top;
      if (top + panelHeight > viewportHeight - PANEL_MARGIN) {
        top = viewportHeight - panelHeight - PANEL_MARGIN;
      }
      top = Math.max(PANEL_MARGIN, top);

      setPosition({ top, left });
    }
  }, [isOpen, anchorRef]);

  useEffect(() => {
    if (isOpen && repositoryPath) {
      loadBranches();
    }
  }, [isOpen, repositoryPath]);

  useEffect(() => {
    if (!isOpen) {
      setSearchTerm('');
      setSelectedIndex(0);
    } else {
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 10);
    return () => {
      clearTimeout(timer);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen, onClose]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.preventDefault(); onClose(); }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  const loadBranches = async () => {
    setIsLoading(true);
    try {
      const cachedState = gitStateManager.getState(repositoryPath);
      if (cachedState?.branches && cachedState.branches.length > 0) {
        setBranches(cachedState.branches.map(b => ({
          name: b.name, current: b.current, remote: b.remote,
          lastCommit: b.lastCommit, ahead: b.ahead, behind: b.behind,
        })));
        setIsLoading(false);
        gitStateManager.refresh(repositoryPath, { layers: ['detailed'], silent: true });
        return;
      }
      await gitStateManager.refresh(repositoryPath, { layers: ['detailed'], force: true });
      const updatedState = gitStateManager.getState(repositoryPath);
      if (updatedState?.branches) {
        setBranches(updatedState.branches.map(b => ({
          name: b.name, current: b.current, remote: b.remote,
          lastCommit: b.lastCommit, ahead: b.ahead, behind: b.behind,
        })));
      } else {
        const branchList = await gitService.getBranches(repositoryPath, false);
        setBranches(branchList);
      }
    } catch (err) {
      log.error('Failed to load branches', err);
    } finally {
      setIsLoading(false);
    }
  };

  const filteredBranches = useMemo(() => {
    let result = branches;
    if (searchTerm.trim()) {
      const lower = searchTerm.toLowerCase();
      result = result.filter(b => b.name.toLowerCase().includes(lower));
    }
    return [...result].sort((a, b) => {
      if (a.current) return -1;
      if (b.current) return 1;
      return a.name.localeCompare(b.name);
    });
  }, [branches, searchTerm]);

  useEffect(() => { setSelectedIndex(0); }, [filteredBranches.length]);

  const handleSwitchBranch = useCallback(async (branchName: string) => {
    if (branchName === currentBranch || isSwitching) return;
    setIsSwitching(true);
    setSwitchingBranch(branchName);
    try {
      const result = await gitService.checkoutBranch(repositoryPath, branchName);
      if (result.success) {
        notificationService.success(
          t('quickSwitch.notifications.switchSuccess', { branch: branchName }),
          { duration: 3000 }
        );
        gitEventService.emit('branch:changed', {
          repositoryPath,
          branch: { name: branchName, current: true, remote: false, ahead: 0, behind: 0 },
          timestamp: new Date(),
        });
        onSwitchSuccess?.(branchName);
        onClose();
      } else {
        let errorMessage = result.error
          ? t('quickSwitch.errors.switchFailedWithMessage', { error: result.error })
          : t('quickSwitch.errors.switchFailed');
        if (result.error?.includes('local changes')) errorMessage = t('quickSwitch.errors.localChanges');
        else if (result.error?.includes('resolve your current index first')) errorMessage = t('quickSwitch.errors.indexConflict');
        notificationService.error(errorMessage, { title: t('quickSwitch.errors.title'), duration: 5000 });
      }
    } catch (error) {
      log.error('Failed to switch branch', error);
      notificationService.error(t('quickSwitch.errors.unexpected'), { duration: 5000 });
    } finally {
      setIsSwitching(false);
      setSwitchingBranch(null);
    }
  }, [repositoryPath, currentBranch, isSwitching, onSwitchSuccess, onClose, t]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (filteredBranches.length === 0) return;
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setSelectedIndex(prev => prev < filteredBranches.length - 1 ? prev + 1 : prev);
        break;
      case 'ArrowUp':
        e.preventDefault();
        setSelectedIndex(prev => prev > 0 ? prev - 1 : prev);
        break;
      case 'Enter': {
        e.preventDefault();
        const sel = filteredBranches[selectedIndex];
        if (sel && !sel.current) handleSwitchBranch(sel.name);
        break;
      }
    }
  }, [filteredBranches, selectedIndex, handleSwitchBranch]);

  useEffect(() => {
    if (listRef.current && filteredBranches.length > 0) {
      const items = listRef.current.querySelectorAll('.branch-quick-switch__item');
      const selectedItem = items[selectedIndex] as HTMLElement;
      if (selectedItem) selectedItem.scrollIntoView({ block: 'nearest' });
    }
  }, [selectedIndex, filteredBranches.length]);

  if (!isOpen) return null;

  return (
    <div
      ref={panelRef}
      className="branch-quick-switch"
      style={{ top: position.top, left: position.left, width: PANEL_WIDTH }}
      onKeyDown={handleKeyDown}
    >
      <div className="branch-quick-switch__search">
        <input
          ref={inputRef}
          type="text"
          placeholder={t('quickSwitch.searchPlaceholder')}
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
          className="branch-quick-switch__input"
        />
      </div>
      <div ref={listRef} className="branch-quick-switch__list">
        {isLoading ? (
          <div className="branch-quick-switch__loading">
            <Loader2 size={16} className="branch-quick-switch__spinner" />
            <span>{t('quickSwitch.loading')}</span>
          </div>
        ) : filteredBranches.length === 0 ? (
          <div className="branch-quick-switch__empty">
            {searchTerm ? t('empty.noMatchingBranches') : t('empty.noBranches')}
          </div>
        ) : (
          filteredBranches.map((branch, index) => (
            <div
              key={branch.name}
              className={[
                'branch-quick-switch__item',
                branch.current && 'branch-quick-switch__item--current',
                index === selectedIndex && 'branch-quick-switch__item--selected',
                switchingBranch === branch.name && 'branch-quick-switch__item--switching',
              ].filter(Boolean).join(' ')}
              onClick={() => handleSwitchBranch(branch.name)}
              onMouseEnter={() => setSelectedIndex(index)}
            >
              <GitBranch size={14} className="branch-quick-switch__item-icon" />
              <span className="branch-quick-switch__item-name">{branch.name}</span>
              {branch.current && <Check size={14} className="branch-quick-switch__item-check" />}
              {switchingBranch === branch.name && <Loader2 size={14} className="branch-quick-switch__spinner" />}
            </div>
          ))
        )}
      </div>
    </div>
  );
};

export default BranchQuickSwitch;
