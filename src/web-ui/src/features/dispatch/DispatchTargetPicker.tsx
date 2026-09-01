import React, {
  lazy,
  Suspense,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';
import { FolderGit2, Laptop, Loader2, MonitorSmartphone, Server } from 'lucide-react';

import { Icon, Menu, MenuItem, MenuSection, MenuSeparator, Tooltip } from '@bitfun/ui';
import { SSHConnectionDialog } from '@/features/ssh-remote/SSHConnectionDialog';
import { useAccountLoginState } from '@/infrastructure/account/useAccountLoginState';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useI18n } from '@/infrastructure/i18n';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { DispatchInstallDialog } from './DispatchInstallDialog';
import type {
  DispatchSelection,
  DispatchTarget,
  DispatchTargetOption,
} from './types';
import { useDispatchTargets } from './useDispatchTargets';
import './DispatchTargetPicker.scss';

interface DispatchTargetPickerProps {
  target: DispatchTarget;
  sourceWorkspacePath?: string;
  locked: boolean;
  disabled?: boolean;
  localWorktreeControl?: {
    enabled: boolean;
    locked: boolean;
    label: string;
    description: string;
    onChange: (enabled: boolean) => void;
  };
  onSelectLocal?: () => void;
  onSelectTarget: (selection: DispatchSelection) => void;
}

const RemoteConnectDialog = lazy(
  () => import('@/app/components/RemoteConnectDialog'),
);

export const DispatchTargetPicker: React.FC<DispatchTargetPickerProps> = ({
  target,
  sourceWorkspacePath,
  locked,
  disabled = false,
  localWorktreeControl,
  onSelectLocal,
  onSelectTarget,
}) => {
  const { t } = useI18n('flow-chat');
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [configureTarget, setConfigureTarget] = useState<DispatchTargetOption | null>(null);
  const [sshDialogOpen, setSshDialogOpen] = useState(false);
  const [accountDialogOpen, setAccountDialogOpen] = useState(false);
  const { loggedIn } = useAccountLoginState();
  const { targets, loading, error, refresh } = useDispatchTargets(open);
  const menuLayout = useAnchoredPopoverPosition({
    open,
    anchorRef: triggerRef,
    popoverRef: menuRef,
    preferredPlacement: 'top',
    alignment: 'end',
    gap: 7,
    layoutRevision: `${targets.length}:${loading}:${error ?? ''}`,
  });

  const localDisplayLabel = localWorktreeControl?.enabled
    ? localWorktreeControl.label
    : t('chatInput.dispatch.local');
  const displayLabel = target.kind === 'local'
    ? localDisplayLabel
    : target.displayName;
  const tooltip = locked
    ? t('chatInput.dispatch.locked', { target: displayLabel })
    : t('chatInput.dispatch.current', { target: displayLabel });

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: PointerEvent) => {
      const targetNode = event.target as Node;
      if (
        !rootRef.current?.contains(targetNode)
        && !menuRef.current?.contains(targetNode)
      ) {
        setOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [open]);

  const sshTargets = useMemo(
    () => targets.filter(
      (item): item is DispatchTargetOption & { kind: 'ssh'; connectionId: string } =>
        item.kind === 'ssh' && !!item.connectionId,
    ),
    [targets],
  );
  const deviceTargets = useMemo(
    () => targets.filter(
      (item): item is DispatchTargetOption & { kind: 'device'; deviceId: string } =>
        item.kind === 'device' && !!item.deviceId,
    ),
    [targets],
  );

  const selectLocalMode = (worktreeEnabled: boolean) => {
    setOpen(false);
    onSelectLocal?.();
    if (
      localWorktreeControl
      && localWorktreeControl.enabled !== worktreeEnabled
    ) {
      localWorktreeControl.onChange(worktreeEnabled);
    }
  };

  const localDirectorySelected =
    target.kind === 'local' && !localWorktreeControl?.enabled;
  const localWorktreeSelected =
    target.kind === 'local' && !!localWorktreeControl?.enabled;

  const menu = open ? (
    <Menu
      ref={menuRef}
      className="dispatch-target-picker__menu"
      data-bf-component="dispatch-target-picker"
      data-bf-part="menu"
      data-bf-placement={menuLayout?.placement ?? 'top'}
      style={{
        top: `${menuLayout?.top ?? 0}px`,
        left: `${menuLayout?.left ?? 0}px`,
        visibility: menuLayout ? 'visible' : 'hidden',
      }}
      aria-label={t('chatInput.dispatch.menuLabel')}
      data-testid="dispatch-target-menu"
      autoFocusFirstItem
    >
      <MenuSection title={t('chatInput.dispatch.menuLabel')}>
        <div className="dispatch-target-picker__status">
          {t('chatInput.dispatch.sessionScope')}
        </div>
      </MenuSection>
      <MenuSeparator />
      <MenuSection title={t('chatInput.dispatch.localSection')}>
        <MenuItem
          role="menuitemradio"
          checked={localDirectorySelected}
          className="dispatch-target-picker__option-row"
          data-bf-component="dispatch-target-picker"
          data-bf-part="option"
          data-testid="dispatch-target-local-option"
          disabled={localWorktreeControl?.locked}
          leading={<Laptop size={15} aria-hidden />}
          metadata={localDirectorySelected ? <Icon name="check-line" size="sm" aria-hidden /> : null}
          onClick={() => selectLocalMode(false)}
        >
          <span className="dispatch-target-picker__option-copy">
            <strong>{t('chatInput.dispatch.local')}</strong>
            <small>{t('chatInput.dispatch.localDescription')}</small>
          </span>
        </MenuItem>
        {localWorktreeControl ? (
          <MenuItem
            role="menuitemradio"
            checked={localWorktreeSelected}
            className="dispatch-target-picker__option-row"
            data-bf-component="dispatch-target-picker"
            data-bf-part="option"
            data-testid="dispatch-target-new-worktree-option"
            disabled={localWorktreeControl.locked}
            leading={<FolderGit2 size={15} aria-hidden />}
            metadata={localWorktreeSelected ? <Icon name="check-line" size="sm" aria-hidden /> : null}
            onClick={() => selectLocalMode(true)}
          >
            <span className="dispatch-target-picker__option-copy">
              <strong>{localWorktreeControl.label}</strong>
              <small>{localWorktreeControl.description}</small>
            </span>
          </MenuItem>
        ) : null}
      </MenuSection>

      <MenuSeparator />
      <MenuSection title={t('chatInput.dispatch.deviceSection')}>
        {!loggedIn ? (
          <MenuItem
            leading={<MonitorSmartphone size={14} aria-hidden />}
            onClick={() => {
              setOpen(false);
              setAccountDialogOpen(true);
            }}
          >
            {t('chatInput.dispatch.signInDevices')}
          </MenuItem>
        ) : null}
        {loggedIn && !loading && deviceTargets.length === 0 ? (
          <div className="dispatch-target-picker__status">
            {t('chatInput.dispatch.noDeviceTargets')}
          </div>
        ) : null}
        {deviceTargets.map(option => {
          const selected = target.kind === 'device' && target.deviceId === option.deviceId;
          const online = option.online !== false;
          return (
            <MenuItem
              key={option.deviceId}
              role="menuitemradio"
              checked={selected}
              className="dispatch-target-picker__option-row"
              disabled={!online}
              leading={<MonitorSmartphone size={15} aria-hidden />}
              metadata={selected ? <Icon name="check-line" size="sm" aria-hidden /> : null}
              onClick={() => {
                setOpen(false);
                setConfigureTarget(option);
              }}
            >
              <span className="dispatch-target-picker__option-copy">
                <strong>{option.displayName}</strong>
                <small>
                  {online
                    ? t('chatInput.dispatch.deviceDescription')
                    : t('chatInput.dispatch.deviceOffline')}
                </small>
              </span>
            </MenuItem>
          );
        })}
      </MenuSection>

      <MenuSeparator />
      <MenuSection title={t('chatInput.dispatch.sshSection')}>
        {loading ? (
          <div className="dispatch-target-picker__status">
            <Loader2 size={14} className="dispatch-target-picker__spin" />
            {t('chatInput.dispatch.loading')}
          </div>
        ) : null}
        {!loading && error ? (
          <MenuItem
            leading={<Icon name="refresh" size="sm" aria-hidden />}
            onClick={() => void refresh()}
          >
            {t('chatInput.dispatch.targetLoadFailed')}
          </MenuItem>
        ) : null}
        {!loading && !error && sshTargets.length === 0 ? (
          <div className="dispatch-target-picker__status">
            {t('chatInput.dispatch.noSshTargets')}
          </div>
        ) : null}
        {sshTargets.map(option => {
          const selected = target.kind === 'ssh' && target.connectionId === option.connectionId;
          return (
            <MenuItem
              key={option.connectionId}
              role="menuitemradio"
              checked={selected}
              className="dispatch-target-picker__option-row"
              leading={<Server size={15} aria-hidden />}
              metadata={selected ? <Icon name="check-line" size="sm" aria-hidden /> : null}
              onClick={() => {
                setOpen(false);
                setConfigureTarget(option);
              }}
            >
              <span className="dispatch-target-picker__option-copy">
                <strong>{option.displayName}</strong>
                <small>{option.description || t('chatInput.dispatch.sshDescription')}</small>
              </span>
            </MenuItem>
          );
        })}
      </MenuSection>

      <MenuSeparator />
      <MenuItem
        leading={<Icon name="plus" size="sm" aria-hidden />}
        onClick={() => {
          setOpen(false);
          setSshDialogOpen(true);
        }}
      >
        {t('chatInput.dispatch.addSsh')}
      </MenuItem>
    </Menu>
  ) : null;

  return (
    <>
      <div
        ref={rootRef}
        className="dispatch-target-picker"
        data-bf-component="dispatch-target-picker"
        data-bf-part="root"
      >
        <Tooltip content={tooltip} placement="top">
          <button
            ref={triggerRef}
            type="button"
            className="dispatch-target-picker__trigger"
            data-bf-component="dispatch-target-picker"
            data-bf-part="trigger"
            aria-haspopup="menu"
            aria-expanded={open}
            aria-label={tooltip}
            disabled={disabled || locked}
            data-testid="chat-input-dispatch-trigger"
            data-dispatch-kind={target.kind}
            onClick={event => {
              event.stopPropagation();
              setOpen(current => !current);
            }}
          >
            {target.kind === 'local'
              ? localWorktreeControl?.enabled
                ? <FolderGit2 size={12} />
                : <Laptop size={12} />
              : target.kind === 'device'
                ? <MonitorSmartphone size={12} />
                : <Server size={12} />}
            <span>{displayLabel}</span>
          </button>
        </Tooltip>
        {menu && createPortal(menu, getAppearanceOverlayHost())}
      </div>

      <DispatchInstallDialog
        open={!!configureTarget}
        target={configureTarget}
        sourceWorkspacePath={sourceWorkspacePath}
        onClose={() => setConfigureTarget(null)}
        onReady={selection => {
          setConfigureTarget(null);
          onSelectTarget(selection);
        }}
      />

      <SSHConnectionDialog
        open={sshDialogOpen}
        onClose={() => {
          setSshDialogOpen(false);
          void refresh();
        }}
      />
      {accountDialogOpen ? (
        <Suspense fallback={null}>
          <RemoteConnectDialog
            isOpen={accountDialogOpen}
            initialGroup="account"
            onClose={() => {
              setAccountDialogOpen(false);
              void refresh();
            }}
          />
        </Suspense>
      ) : null}
    </>
  );
};
