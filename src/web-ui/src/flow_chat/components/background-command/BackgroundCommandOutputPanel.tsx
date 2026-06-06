import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AlertCircle, ClipboardCopy, Copy, Keyboard, Loader2, Terminal } from 'lucide-react';
import { Button, Checkbox, IconButton, Textarea, Tooltip } from '@/component-library';
import { useTranslation } from 'react-i18next';
import { agentAPI } from '@/infrastructure/api';
import type {
  BackgroundCommandOutputMetadata,
  BackgroundCommandOutputStatus,
} from '@/infrastructure/api/service-api/AgentAPI';
import { TerminalOutputRenderer } from '@/tools/terminal/components';
import { notificationService } from '@/shared/notification-system';
import './BackgroundCommandOutputPanel.scss';

const BACKGROUND_COMMAND_OUTPUT_POLL_INTERVAL_MS = 1000;

export interface BackgroundCommandOutputPanelData {
  execSessionKey: string;
  execSessionId: number;
  remote: boolean;
  title?: string;
  command?: string;
  mockKind?: string;
}

interface BackgroundCommandOutputPanelProps {
  data: BackgroundCommandOutputPanelData;
}

function mockOutputForKind(mockKind: string | undefined): {
  metadata: BackgroundCommandOutputMetadata;
  output: string;
} {
  const kind = mockKind || 'test';
  const command = kind === 'build'
    ? 'pnpm run desktop:dev -- --profile heavy-ui-check'
    : kind === 'interactive-input'
      ? 'node interactive-test.js'
      : kind === 'finished'
        ? 'node scripts/i18n-audit.mjs'
        : 'cargo test -p terminal-core lifecycle_reports_running_and_natural_exit';
  const status: BackgroundCommandOutputStatus = kind === 'finished' ? 'exited' : 'running';
  const now = Math.floor(Date.now() / 1000);
  const execSessionId = kind === 'interactive-input'
    ? 4216
    : kind === 'build'
      ? 4218
      : kind === 'finished'
        ? undefined
        : 4217;
  const output = kind === 'interactive-input'
    ? '\x1b[?9001h\x1b[?1004h\x1b[?25l\x1b[2J\x1b[m\x1b[HEnter your name:\x1b[1C\x1b]0;PowerShell\x07\x1b[?25h'
    : [
        `$ ${command}`,
        'Compiling terminal-core v0.1.0',
        'running 1 test',
        'test exec::tests::lifecycle_reports_running_and_natural_exit ... ok',
        '',
        kind === 'build'
          ? '... earlier output was truncated from the beginning ...'
          : 'test result: ok. 1 passed; 0 failed; 0 ignored',
      ].join('\n');

  return {
    metadata: {
      execSessionId,
      command,
      remote: kind === 'build',
      tty: kind !== 'finished',
      status,
      exitCode: status === 'exited' ? 0 : undefined,
      startedAt: now - 42,
      endedAt: status === 'exited' ? now - 1 : undefined,
      retainedBytes: 734,
      retainedLimitBytes: 1024 * 1024,
      truncatedFromStart: kind === 'build',
    },
    output,
  };
}

function statusLabelKey(status: BackgroundCommandOutputStatus): string {
  return `backgroundCommandOutput.status.${status}`;
}

function sanitizeTerminalOutputForLogView(output: string): string {
  return output
    // OSC/DCS/PM/APC payloads update terminal metadata or device state; they are
    // not readable command output in a linear log view.
    // eslint-disable-next-line no-control-regex -- terminal control sequences are intentional here.
    .replace(/\x1b[\]PX_^][\s\S]*?(?:\x07|\x1b\\)/g, '')
    // Keep SGR color/style sequences for xterm rendering, but strip all other
    // CSI sequences because they mutate screen state, cursor position, or modes.
    // eslint-disable-next-line no-control-regex -- terminal control sequences are intentional here.
    .replace(/\x1b\[([0-?]*)([ -/]*)([@-~])/g, (sequence, _params, _intermediates, finalByte) => (
      finalByte === 'm' ? sequence : ''
    ))
    // Strip remaining simple ESC sequences such as RIS/charset selection.
    // eslint-disable-next-line no-control-regex -- terminal control sequences are intentional here.
    .replace(/\x1b[ -/]*[@-~]/g, '');
}

export const BackgroundCommandOutputPanel: React.FC<BackgroundCommandOutputPanelProps> = ({ data }) => {
  const { t } = useTranslation('flow-chat');
  const [metadata, setMetadata] = useState<BackgroundCommandOutputMetadata | null>(null);
  const [output, setOutput] = useState('');
  const [sanitizeOutput, setSanitizeOutput] = useState(false);
  const [isInputEditorOpen, setIsInputEditorOpen] = useState(false);
  const [inputChars, setInputChars] = useState('');
  const [inputAppendEnter, setInputAppendEnter] = useState(true);
  const [maskInput, setMaskInput] = useState(false);
  const [isSendingInput, setIsSendingInput] = useState(false);
  const [cursor, setCursor] = useState<number | undefined>(undefined);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const cursorRef = useRef<number | undefined>(undefined);
  const inputEditorRef = useRef<HTMLTextAreaElement | null>(null);
  const autoOpenedInputForSessionRef = useRef<string | null>(null);

  useEffect(() => {
    cursorRef.current = cursor;
  }, [cursor]);

  useEffect(() => {
    let cancelled = false;

    const readOutput = async (initial = false) => {
      if (data.mockKind) {
        const mock = mockOutputForKind(data.mockKind);
        if (!cancelled) {
          setMetadata(mock.metadata);
          setOutput(mock.output);
          setCursor(1);
          setLoading(false);
          setError(null);
        }
        return;
      }

      try {
        const response = await agentAPI.readBackgroundCommandOutput({
          execSessionId: data.execSessionId,
          remote: data.remote,
          cursor: initial ? undefined : cursorRef.current,
        });
        if (cancelled) {
          return;
        }

        setMetadata(response.metadata);
        setCursor(response.cursor);
        setError(null);
        setOutput((previous) => {
          if (response.snapshot != null || response.reset) {
            return response.snapshot ?? '';
          }
          if (response.chunks.length === 0) {
            return previous;
          }
          return `${previous}${response.chunks.join('')}`;
        });
      } catch (readError) {
        if (!cancelled) {
          setError(readError instanceof Error ? readError.message : String(readError));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    void readOutput(true);
    const intervalId = window.setInterval(() => {
      if (metadata?.status && metadata.status !== 'running') {
        return;
      }
      void readOutput(false);
    }, BACKGROUND_COMMAND_OUTPUT_POLL_INTERVAL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [data.execSessionId, data.mockKind, data.remote, metadata?.status]);

  const command = metadata?.command || data.command || data.title || data.execSessionKey;
  const displayedOutput = useMemo(
    () => sanitizeOutput ? sanitizeTerminalOutputForLogView(output) : output,
    [output, sanitizeOutput],
  );

  const copyOutput = () => {
    void navigator.clipboard.writeText(displayedOutput);
  };

  const copyCommand = () => {
    void navigator.clipboard.writeText(command);
  };

  const canSendInput =
    metadata?.status === 'running' &&
    metadata.execSessionId != null &&
    metadata.tty === true;

  useEffect(() => {
    autoOpenedInputForSessionRef.current = null;
    setIsInputEditorOpen(false);
  }, [data.execSessionKey]);

  useEffect(() => {
    if (!canSendInput) {
      return;
    }
    if (autoOpenedInputForSessionRef.current === data.execSessionKey) {
      return;
    }
    autoOpenedInputForSessionRef.current = data.execSessionKey;
    setIsInputEditorOpen(true);
  }, [canSendInput, data.execSessionKey]);

  const handleToggleInputEditor = () => {
    if (!canSendInput) {
      return;
    }
    setIsInputEditorOpen((open) => !open);
  };

  const handleCloseInputEditor = () => {
    if (isSendingInput) {
      return;
    }
    setIsInputEditorOpen(false);
  };

  const canSubmitInput = canSendInput && (inputChars.length > 0 || inputAppendEnter);

  const handleSendInput = async () => {
    if (!canSendInput || metadata?.execSessionId == null) {
      return;
    }

    setIsSendingInput(true);
    try {
      if (data.mockKind) {
        await new Promise<void>((resolve) => window.setTimeout(resolve, 350));
      } else {
        await agentAPI.sendBackgroundCommandInput({
          execSessionId: metadata.execSessionId,
          remote: metadata.remote === true,
          chars: inputChars,
          appendEnter: inputAppendEnter,
        });
      }
      setInputChars('');
    } catch {
      notificationService.error(
        t('backgroundCommandInput.sendFailed'),
        { duration: 5000 },
      );
    } finally {
      setIsSendingInput(false);
    }
  };

  useEffect(() => {
    if (!canSendInput) {
      setIsInputEditorOpen(false);
    }
  }, [canSendInput]);

  useEffect(() => {
    if (!isInputEditorOpen) {
      return;
    }

    const frameId = window.requestAnimationFrame(() => {
      inputEditorRef.current?.focus();
    });

    return () => {
      window.cancelAnimationFrame(frameId);
    };
  }, [isInputEditorOpen]);

  return (
    <>
      <section className="background-command-output-panel">
        <header className="background-command-output-panel__header">
          <div className="background-command-output-panel__title-group">
            <span className="background-command-output-panel__icon">
              <Terminal size={16} aria-hidden="true" />
            </span>
            <div>
              <h2>{t('backgroundCommandOutput.title')}</h2>
              <p title={command}>{command}</p>
            </div>
          </div>
          <div className="background-command-output-panel__header-actions">
            <IconButton
              variant="ghost"
              size="small"
              onClick={handleToggleInputEditor}
              tooltip={canSendInput
                ? t('backgroundCommandOutput.sendInput')
                : t('backgroundCommandOutput.sendInputUnavailable')}
              aria-label={t('backgroundCommandOutput.sendInput')}
              disabled={!canSendInput}
            >
              <Keyboard size={14} aria-hidden="true" />
            </IconButton>
            <IconButton
              variant="ghost"
              size="small"
              onClick={copyCommand}
              tooltip={t('backgroundCommandOutput.copyCommand')}
              aria-label={t('backgroundCommandOutput.copyCommand')}
              disabled={!command}
            >
              <ClipboardCopy size={14} aria-hidden="true" />
            </IconButton>
            <IconButton
              variant="ghost"
              size="small"
              onClick={copyOutput}
              tooltip={t('backgroundCommandOutput.copy')}
              aria-label={t('backgroundCommandOutput.copy')}
              disabled={!displayedOutput}
            >
              <Copy size={14} aria-hidden="true" />
            </IconButton>
          </div>
        </header>

        <div className="background-command-output-panel__meta">
          <div className="background-command-output-panel__meta-status">
            {metadata ? (
              <>
                <span>{t(statusLabelKey(metadata.status))}</span>
                {metadata.remote ? <span>{t('backgroundCommandOutput.remote')}</span> : null}
                {metadata.execSessionId != null ? (
                  <span>{t('backgroundCommandOutput.session', { id: metadata.execSessionId })}</span>
                ) : null}
                {metadata.exitCode != null ? (
                  <span>{t('backgroundCommandOutput.exitCode', { code: metadata.exitCode })}</span>
                ) : null}
              </>
            ) : loading ? (
              <span className="background-command-output-panel__loading">
                <Loader2 size={13} aria-hidden="true" />
                {t('backgroundCommandOutput.loading')}
              </span>
            ) : null}
          </div>
          <Tooltip content={t('backgroundCommandOutput.simplifiedViewTooltip')}>
            <span className="background-command-output-panel__sanitize-toggle-trigger">
              <Checkbox
                className="background-command-output-panel__sanitize-toggle"
                size="small"
                checked={sanitizeOutput}
                onChange={(event) => setSanitizeOutput(event.target.checked)}
                label={t('backgroundCommandOutput.simplifiedView')}
              />
            </span>
          </Tooltip>
        </div>

        {metadata?.truncatedFromStart ? (
          <div className="background-command-output-panel__notice">
            <AlertCircle size={14} aria-hidden="true" />
            <span>{t('backgroundCommandOutput.truncatedFromStart')}</span>
          </div>
        ) : null}

        {error ? (
          <div className="background-command-output-panel__error">
            <AlertCircle size={14} aria-hidden="true" />
            <span>{t('backgroundCommandOutput.error', { message: error })}</span>
          </div>
        ) : null}

        <div className="background-command-output-panel__output">
          {displayedOutput ? (
            <TerminalOutputRenderer
              content={displayedOutput}
              className="background-command-output-panel__terminal"
              minHeight={420}
              maxHeight={1200}
            />
          ) : (
            <div className="background-command-output-panel__empty">
              {loading ? t('backgroundCommandOutput.loading') : t('backgroundCommandOutput.empty')}
            </div>
          )}
        </div>
        {isInputEditorOpen ? (
          <form
            className="background-command-output-panel__input-editor"
            onSubmit={(event) => {
              event.preventDefault();
              if (canSubmitInput && !isSendingInput) {
                void handleSendInput();
              }
            }}
          >
            <Textarea
              ref={inputEditorRef}
              className={maskInput ? 'background-command-output-panel__input-textarea background-command-output-panel__input-textarea--masked' : 'background-command-output-panel__input-textarea'}
              value={inputChars}
              onChange={(event) => setInputChars(event.target.value)}
              placeholder={t('backgroundCommandInput.inputPlaceholder')}
              rows={3}
              disabled={!canSendInput || isSendingInput}
              autoComplete="off"
              spellCheck={false}
            />
            <div className="background-command-output-panel__input-editor-footer">
              <div className="background-command-output-panel__input-options">
                <Checkbox
                  className="background-command-output-panel__input-option"
                  size="small"
                  checked={inputAppendEnter}
                  onChange={(event) => setInputAppendEnter(event.target.checked)}
                  disabled={!canSendInput || isSendingInput}
                  label={t('backgroundCommandInput.appendEnter')}
                />
                <Checkbox
                  className="background-command-output-panel__input-option"
                  size="small"
                  checked={maskInput}
                  onChange={(event) => setMaskInput(event.target.checked)}
                  disabled={!canSendInput || isSendingInput}
                  label={t('backgroundCommandInput.maskInput')}
                />
              </div>
              <div className="background-command-output-panel__input-editor-actions">
                <Button
                  type="button"
                  variant="secondary"
                  size="small"
                  onClick={handleCloseInputEditor}
                  disabled={isSendingInput}
                >
                  {t('backgroundCommandInput.cancel')}
                </Button>
                <Button
                  type="submit"
                  variant="primary"
                  size="small"
                  isLoading={isSendingInput}
                  disabled={!canSubmitInput}
                >
                  {t('backgroundCommandInput.send')}
                </Button>
              </div>
            </div>
          </form>
        ) : null}
      </section>
    </>
  );
};

export default BackgroundCommandOutputPanel;
