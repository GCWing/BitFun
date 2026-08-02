import React, {
  lazy,
  Suspense,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  Check,
  ChevronDown,
  Laptop,
  Loader2,
  MonitorSmartphone,
  Plus,
  RefreshCw,
  Server,
} from 'lucide-react';
import { Tooltip } from '@/component-library';
import { SSHConnectionDialog } from '@/features/ssh-remote/SSHConnectionDialog';
import { useAccountLoginState } from '@/infrastructure/account/useAccountLoginState';
import { useI18n } from '@/infrastructure/i18n';
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
  onSelectLocal,
  onSelectTarget,
}) => {
  const { t } = useI18n('flow-chat');
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [configureTarget, setConfigureTarget] = useState<DispatchTargetOption | null>(null);
  const [sshDialogOpen, setSshDialogOpen] = useState(false);
  const [accountDialogOpen, setAccountDialogOpen] = useState(false);
  const { loggedIn } = useAccountLoginState();
  const { targets, loading, error, refresh } = useDispatchTargets(open);

  const displayLabel = target.kind === 'local'
    ? t('chatInput.dispatch.local')
    : target.displayName;
  const tooltip = locked
    ? t('chatInput.dispatch.locked', { target: displayLabel })
    : t('chatInput.dispatch.current', { target: displayLabel });

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
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

  const menu = open ? (
    <div
      className="dispatch-target-picker__menu"
      role="menu"
      aria-label={t('chatInput.dispatch.menuLabel')}
      data-testid="dispatch-target-menu"
    >
      <div className="dispatch-target-picker__header">
        <span>{t('chatInput.dispatch.menuLabel')}</span>
        <small>{t('chatInput.dispatch.sessionScope')}</small>
      </div>
      <div className="dispatch-target-picker__section">
        <div className="dispatch-target-picker__section-title">
          {t('chatInput.dispatch.localSection')}
        </div>
        <button
          type="button"
          role="menuitemradio"
          aria-checked={target.kind === 'local'}
          className="dispatch-target-picker__option"
          onClick={() => {
            setOpen(false);
            onSelectLocal?.();
          }}
        >
          <Laptop size={15} aria-hidden />
          <span>
            <strong>{t('chatInput.dispatch.local')}</strong>
            <small>{t('chatInput.dispatch.localDescription')}</small>
          </span>
          {target.kind === 'local' ? <Check size={14} aria-hidden /> : null}
        </button>
      </div>

      <div className="dispatch-target-picker__divider" role="separator" />
      <div className="dispatch-target-picker__section">
        <div className="dispatch-target-picker__section-title">
          {t('chatInput.dispatch.deviceSection')}
        </div>
        {!loggedIn ? (
          <button
            type="button"
            role="menuitem"
            className="dispatch-target-picker__footer-action"
            onClick={() => {
              setOpen(false);
              setAccountDialogOpen(true);
            }}
          >
            <MonitorSmartphone size={14} aria-hidden />
            <span>{t('chatInput.dispatch.signInDevices')}</span>
          </button>
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
            <button
              key={option.deviceId}
              type="button"
              role="menuitemradio"
              aria-checked={selected}
              aria-disabled={!online}
              className="dispatch-target-picker__option"
              disabled={!online}
              onClick={() => {
                setOpen(false);
                setConfigureTarget(option);
              }}
            >
              <MonitorSmartphone size={15} aria-hidden />
              <span>
                <strong>{option.displayName}</strong>
                <small>
                  {online
                    ? t('chatInput.dispatch.deviceDescription')
                    : t('chatInput.dispatch.deviceOffline')}
                </small>
              </span>
              {selected ? <Check size={14} aria-hidden /> : null}
            </button>
          );
        })}
      </div>

      <div className="dispatch-target-picker__divider" role="separator" />
      <div className="dispatch-target-picker__section">
        <div className="dispatch-target-picker__section-title">
          {t('chatInput.dispatch.sshSection')}
        </div>
        {loading ? (
          <div className="dispatch-target-picker__status">
            <Loader2 size={14} className="dispatch-target-picker__spin" />
            {t('chatInput.dispatch.loading')}
          </div>
        ) : null}
        {!loading && error ? (
          <button
            type="button"
            role="menuitem"
            className="dispatch-target-picker__footer-action"
            onClick={() => void refresh()}
          >
            <RefreshCw size={14} aria-hidden />
            <span>{t('chatInput.dispatch.targetLoadFailed')}</span>
          </button>
        ) : null}
        {!loading && !error && sshTargets.length === 0 ? (
          <div className="dispatch-target-picker__status">
            {t('chatInput.dispatch.noSshTargets')}
          </div>
        ) : null}
        {sshTargets.map(option => {
          const selected = target.kind === 'ssh' && target.connectionId === option.connectionId;
          return (
            <button
              key={option.connectionId}
              type="button"
              role="menuitemradio"
              aria-checked={selected}
              className="dispatch-target-picker__option"
              onClick={() => {
                setOpen(false);
                setConfigureTarget(option);
              }}
            >
              <Server size={15} aria-hidden />
              <span>
                <strong>{option.displayName}</strong>
                <small>{option.description || t('chatInput.dispatch.sshDescription')}</small>
              </span>
              {selected ? <Check size={14} aria-hidden /> : null}
            </button>
          );
        })}
      </div>

      <div className="dispatch-target-picker__divider" role="separator" />
      <button
        type="button"
        role="menuitem"
        className="dispatch-target-picker__footer-action"
        onClick={() => {
          setOpen(false);
          setSshDialogOpen(true);
        }}
      >
        <Plus size={14} aria-hidden />
        <span>{t('chatInput.dispatch.addSsh')}</span>
      </button>
    </div>
  ) : null;

  return (
    <>
      <div ref={rootRef} className="dispatch-target-picker">
        <Tooltip content={tooltip} placement="top">
          <button
            type="button"
            className="dispatch-target-picker__trigger"
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
              ? <Laptop size={12} />
              : target.kind === 'device'
                ? <MonitorSmartphone size={12} />
                : <Server size={12} />}
            <span>{displayLabel}</span>
            {!locked ? (
              <ChevronDown
                className="dispatch-target-picker__chevron"
                size={11}
                aria-hidden
              />
            ) : null}
          </button>
        </Tooltip>
        {menu}
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
