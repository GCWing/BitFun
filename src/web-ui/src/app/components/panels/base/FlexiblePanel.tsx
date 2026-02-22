import React, { useCallback, memo, useMemo } from 'react';
import { Download, Copy, X, AlertCircle } from 'lucide-react';
import { MarkdownRenderer, IconButton } from '@/component-library';
import { CodeEditor, MarkdownEditor, ImageViewer, DiffEditor } from '@/tools/editor';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('FlexiblePanel');

// Stable lazy components at module level to avoid re-creation on each render
const MermaidPanel = React.lazy(() => 
  import('@/tools/mermaid-editor/components').then(module => ({ default: module.MermaidPanel }))
);

const MermaidEditor = React.lazy(() => 
  import('@/tools/mermaid-editor/components').then(module => ({ default: module.MermaidEditor }))
);

const MermaidErrorBoundary = React.lazy(() => 
  import('@/tools/mermaid-editor/components').then(module => ({ default: module.MermaidErrorBoundary }))
);

const GitDiffView = React.lazy(() => 
  import('@/tools/git/components/GitDiffView/GitDiffView')
);

const GitSettingsView = React.lazy(() => 
  import('@/tools/git/components/GitSettingsView/GitSettingsView')
);

// Directly imported (not lazy-loaded) to avoid loading delay in frequently used Git panel
import { GitDiffEditor } from '@/tools/git/components/GitDiffEditor/GitDiffEditor';

const GitGraphView = React.lazy(() => 
  import('@/tools/git/components/GitGraphView/GitGraphView').then(module => ({ 
    default: module.GitGraphView 
  }))
);

const GitBranchHistoryView = React.lazy(() =>
  import('@/tools/git/components/GitBranchHistoryView/GitBranchHistoryView').then(module => ({
    default: module.GitBranchHistoryView
  }))
);

// Plan viewer component
const PlanViewer = React.lazy(() => 
  import('@/tools/editor/components/PlanViewer').then(module => ({ 
    default: module.default 
  }))
);

// Uses ConnectedTerminal to auto-connect backend
const TerminalTabPanel = React.lazy(() => 
  import('@/tools/terminal').then(module => ({ 
    default: module.ConnectedTerminal 
  }))
);

const TaskDetailPanel = React.lazy(() => 
  import('@/flow_chat/components/TaskDetailPanel').then(module => ({ 
    default: module.TaskDetailPanel 
  }))
);

// CodePreview, ChartRenderer and CodeNode removed - visualization features disabled
import { 
  FlexiblePanelProps
} from './types';
import { 
  getContentIcon, 
  getContentTypeName, 
  shouldShowHeader,
  generateFileName
} from './utils';
import './FlexiblePanel.scss';

interface ExtendedFlexiblePanelProps extends FlexiblePanelProps {
  onDirtyStateChange?: (isDirty: boolean) => void;
}

const FlexiblePanel: React.FC<ExtendedFlexiblePanelProps> = memo(({
  content,
  onContentChange,
  className = '',
  onInteraction,
  workspacePath,
  onBeforeClose,
  onDirtyStateChange
}) => {
  const { t } = useI18n('components');

  // Use ref to save latest content, avoiding it in callback dependencies
  const contentRef = React.useRef(content);
  React.useEffect(() => {
    contentRef.current = content;
  }, [content]);

  // Sync dirty state from MonacoModelManager on component mount
  React.useEffect(() => {
    if (content?.type !== 'code-editor' && content?.type !== 'markdown-editor') {
      return;
    }
    
    const filePath = content?.data?.filePath;
    if (!filePath || !onDirtyStateChange) return;
    
    import('@/tools/editor/services/MonacoModelManager').then(({ monacoModelManager }) => {
      const metadata = monacoModelManager.getModelMetadata(filePath);
      if (metadata !== undefined) {
        onDirtyStateChange(metadata.isDirty);
      }
    }).catch(() => {});
  }, [content?.type, content?.data?.filePath, onDirtyStateChange]);

  const handleClose = useCallback(async () => {
    if (onBeforeClose) {
      const canClose = await onBeforeClose(content);
      if (!canClose) {
        return;
      }
    }
    
    onContentChange?.(null);
  }, [onContentChange, onBeforeClose, content]);

  const handleCopy = useCallback(() => {
    if (!content?.data) return;
    
    let textToCopy = '';
    if (typeof content.data === 'string') {
      textToCopy = content.data;
    } else if (content.data.content) {
      textToCopy = content.data.content;
    }
    
    navigator.clipboard.writeText(textToCopy).then(() => {
      // User feedback for successful copy can be implemented via global notification system
      if (onInteraction) {
        onInteraction('copy', 'success');
      }
    }).catch(() => {
      if (onInteraction) {
        onInteraction('copy', 'failed');
      }
    });
  }, [content]);

  const handleDownload = useCallback(() => {
    if (!content?.data) return;
    
    let textToDownload = '';
    if (typeof content.data === 'string') {
      textToDownload = content.data;
    } else if (content.data.content) {
      textToDownload = content.data.content;
    }
    
    const filename = generateFileName(content.type, content.title);
    const blob = new Blob([textToDownload], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }, [content]);

  const mermaidEditorProps = useMemo(() => {
    if (content?.type !== 'mermaid-editor') return null;
    
    const mermaidData = content.data || {};
    const metadata = content.metadata || {};
    
    return {
      initialSourceCode: mermaidData.sourceCode || t('flexiblePanel.fallback.mermaidDefaultCode'),
      onSave: async (sourceCode: string) => {
        if (onContentChange) {
          onContentChange({
            ...content,
            data: {
              ...mermaidData,
              sourceCode
            }
          });
        }
        
        if (onDirtyStateChange) {
          onDirtyStateChange(false);
        }
        
        if (onInteraction) {
          await onInteraction('save', JSON.stringify({
            sourceCode,
            filePath: mermaidData.filePath
          }));
        }
      },
      onExport: async (format: string, data: string) => {
        if (onInteraction) {
          await onInteraction('export', JSON.stringify({
            format,
            data,
            fileName: content.title
          }));
        }
      }
    };
  }, [content, onContentChange, onDirtyStateChange, onInteraction]);

  const renderContent = () => {
    if (!content || content.type === 'empty') {
      return (
        <div className="bitfun-flexible-panel__empty-content">
          <div className="bitfun-flexible-panel__empty-icon">
            {getContentIcon('empty')}
          </div>
          <h3>{t('flexiblePanel.empty.title')}</h3>
          <p>{t('flexiblePanel.empty.description')}</p>
        </div>
      );
    }

    switch (content.type) {
      case 'code-preview':
        const previewData = content.data || {};
        const hasFixNeeded = previewData.migrationContext?.hasUpgradePoints || previewData.needsFix || false;
        
        return (
          <div className={`bitfun-flexible-panel__code-content ${hasFixNeeded ? 'needs-fix' : ''}`}>
            <pre><code>{typeof content.data === 'string' ? content.data : t('flexiblePanel.fallback.noCodeContent')}</code></pre>
          </div>
        );

      case 'markdown-viewer':
        return (
          <div className="bitfun-flexible-panel__markdown-content">
            <MarkdownRenderer content={typeof content.data === 'string' ? content.data : ''} />
          </div>
        );

      case 'markdown-editor':
        const markdownEditorData = content.data || {};
        const markdownFilePath = markdownEditorData.filePath;
        const markdownInitialContent = markdownEditorData.initialContent;
        const markdownFileName = markdownEditorData.fileName || content.title;
        const markdownWorkspacePath = markdownEditorData.workspacePath || workspacePath;
        const markdownJumpToLine = markdownEditorData.jumpToLine;
        const markdownJumpToColumn = markdownEditorData.jumpToColumn;
        
        return (
          <div className="bitfun-flexible-panel__markdown-editor">
            {markdownFilePath || markdownInitialContent !== undefined ? (
              <MarkdownEditor
                filePath={markdownFilePath}
                initialContent={markdownInitialContent}
                fileName={markdownFileName}
                workspacePath={markdownWorkspacePath}
                readOnly={markdownEditorData.readOnly || false}
                jumpToLine={markdownJumpToLine}
                jumpToColumn={markdownJumpToColumn}
                onContentChange={(_newContent, hasChanges) => {
                  if (onDirtyStateChange) {
                    onDirtyStateChange(hasChanges);
                  }
                }}
                onSave={(_savedContent) => {
                  if (onDirtyStateChange) {
                    onDirtyStateChange(false);
                  }
                }}
              />
            ) : (
              <div className="bitfun-flexible-panel__error-message">
                <AlertCircle size={20} />
                <p>{t('flexiblePanel.errors.markdownEditorMissingPath')}</p>
              </div>
            )}
          </div>
        );


      case 'mermaid-editor':
        const mermaidData = content.data || {};
        
        if (mermaidData.mode || mermaidData.interactive_config || mermaidData.mermaid_code) {
          return (
            <React.Suspense fallback={<div>{t('flexiblePanel.loading.mermaidPanel')}</div>}>
              <MermaidErrorBoundary>
                <MermaidPanel
                  data={{
                    mermaid_code: mermaidData.mermaid_code || mermaidData.sourceCode || t('flexiblePanel.fallback.mermaidDefaultCode'),
                    title: content.title || t('flexiblePanel.fallback.mermaidChartTitle'),
                    session_id: mermaidData.session_id,
                    mode: mermaidData.mode || 'editor',
                    allow_mode_switch: mermaidData.allow_mode_switch !== false,
                    editor_config: mermaidData.editor_config,
                    interactive_config: mermaidData.interactive_config
                  }}
                  onDataChange={(newData) => {
                    if (onContentChange) {
                      onContentChange({
                        ...content,
                        data: {
                          ...mermaidData,
                          ...newData
                        }
                      });
                    }
                    // Write back updated mermaid code to flowChatStore and persist to disk.
                    const source = mermaidData._source;
                    if (source?.type === 'tool-call' && source.toolCallId && newData.mermaid_code) {
                      import('@/flow_chat/store/FlowChatStore').then(({ flowChatStore }) => {
                        import('@/flow_chat/services/FlowChatManager').then(({ flowChatManager }) => {
                          const state = flowChatStore.getState();
                          const activeSessionId = state.activeSessionId;
                          if (!activeSessionId) return;

                          const session = state.sessions.get(activeSessionId);
                          if (!session) return;

                          for (const turn of session.dialogTurns) {
                            for (const round of turn.modelRounds) {
                              const item = round.items.find(
                                (it: any) =>
                                  it.type === 'tool' &&
                                  (it.toolCall?.id === source.toolCallId || it.id === source.toolItemId)
                              );
                              if (item) {
                                const toolItem = item as any;
                                flowChatStore.updateModelRoundItem(activeSessionId, turn.id, toolItem.id, {
                                  toolCall: {
                                    ...toolItem.toolCall,
                                    input: {
                                      ...toolItem.toolCall.input,
                                      mermaid_code: newData.mermaid_code,
                                    }
                                  }
                                } as any);
                                flowChatManager.saveDialogTurn(activeSessionId, turn.id).catch(() => {});
                                return;
                              }
                            }
                          }
                        });
                      });
                    }
                  }}
                  onInteraction={onInteraction}
                />
              </MermaidErrorBoundary>
            </React.Suspense>
          );
        } else {
          if (!mermaidEditorProps) return null;

          return (
            <React.Suspense fallback={<div>{t('flexiblePanel.loading.mermaidEditor')}</div>}>
              <MermaidErrorBoundary>
                <MermaidEditor
                  initialSourceCode={mermaidEditorProps.initialSourceCode}
                  onSave={mermaidEditorProps.onSave}
                  onExport={mermaidEditorProps.onExport}
                />
              </MermaidErrorBoundary>
            </React.Suspense>
          );
        }

      case 'text-viewer':
        return (
          <div className="bitfun-flexible-panel__text-content">
            <pre>{typeof content.data === 'string' ? content.data : 'No text content available'}</pre>
          </div>
        );

      case 'file-viewer':
        const fileViewerData = content.data || {};
        const fileNeedsFix = fileViewerData.migrationContext?.hasUpgradePoints || fileViewerData.needsFix || false;
        const fileViewerClass = `bitfun-flexible-panel__panel-code-viewer ${fileNeedsFix ? 'needs-fix' : ''}`;
        
        return (
          <div className="bitfun-flexible-panel__code-viewer-container">
            <CodeEditor
              filePath={fileViewerData.filePath || ''}
              fileName={content.title}
              readOnly={true}
              showLineNumbers={true}
              showMinimap={true}
              theme="vs-dark"
              className={fileViewerClass}
            />
          </div>
        );

      case 'image-viewer':
        const imageViewerData = content.data || {};
        
        return (
          <div className="bitfun-flexible-panel__image-viewer-container">
            <ImageViewer
              filePath={imageViewerData.filePath || ''}
              fileName={content.title}
              workspacePath={workspacePath}
              className="bitfun-flexible-panel__image-viewer"
            />
          </div>
        );

      case 'code-viewer':
        const codeData = content.data || {};
        const migrationContext = codeData.migrationContext || {};
        const needsFix = migrationContext.hasUpgradePoints || codeData.needsFix || false;
        
        return (
          <div className="bitfun-flexible-panel__code-viewer-container">
            <div className={`bitfun-flexible-panel__code-content ${needsFix ? 'needs-fix' : ''}`}>
              <CodeEditor
                filePath={codeData.filePath || ''}
                fileName={codeData.fileName}
                language={codeData.language || 'typescript'}
                readOnly={codeData.readOnly !== false}
                showLineNumbers={true}
                showMinimap={true}
                theme="vs-dark"
                onContentChange={codeData.onContentChange}
              />
            </div>
          </div>
        );

      case 'code-editor':
        const editorData = content.data || {};
        const filePath = editorData.filePath || '';
        const fileName = editorData.fileName || content.title;
        const editorLanguage = editorData.language;
        
        return (
          <CodeEditor
            filePath={filePath}
            workspacePath={workspacePath}
            fileName={fileName}
            language={editorLanguage}
            readOnly={editorData.readOnly || false}
            showLineNumbers={editorData.showLineNumbers !== false}
            showMinimap={editorData.showMinimap !== false}
            theme={editorData.theme || 'vs-dark'}
            jumpToLine={editorData.jumpToLine}
            jumpToColumn={editorData.jumpToColumn}
            jumpToRange={editorData.jumpToRange}
            onContentChange={(newContent, hasChanges) => {
                if (onContentChange) {
                  onContentChange({
                    ...content,
                    data: {
                      ...editorData,
                      content: newContent,
                      hasChanges
                    }
                  });
                }
                
                if (onDirtyStateChange) {
                  onDirtyStateChange(hasChanges);
                }
              }}
              onSave={(content) => {
                if (onInteraction) {
                  onInteraction('save', JSON.stringify({ filePath, content }));
                }
                
                if (onDirtyStateChange) {
                  onDirtyStateChange(false);
                }
              }}
          />
        );

      case 'diff-code-editor':
        const diffData = content.data || {};
        const originalCode = diffData.originalCode || '';
        const modifiedCode = diffData.modifiedCode || originalCode;
        const diffFilePath = diffData.filePath;
        const diffMigrationContext = diffData.migrationContext;
        const diffRepositoryPath = diffData.repositoryPath;
        
        const diffViewerKey = `diff-${diffFilePath || 'unknown'}-${originalCode.length}-${modifiedCode.length}`;
        
        if (diffRepositoryPath && diffFilePath) {
          return (
            <GitDiffEditor
              key={diffViewerKey}
              originalContent={originalCode}
              modifiedContent={modifiedCode}
              filePath={diffFilePath}
              repositoryPath={diffRepositoryPath}
              onAcceptAll={() => {
                diffMigrationContext?.onAcceptAll?.();
                window.dispatchEvent(new CustomEvent('git-status-changed', {
                  detail: { repositoryPath: diffRepositoryPath }
                }));
              }}
              onRejectAll={() => {
                diffMigrationContext?.onRejectAll?.();
                window.dispatchEvent(new CustomEvent('git-status-changed', {
                  detail: { repositoryPath: diffRepositoryPath }
                }));
              }}
              onClose={() => {}}
              onContentChange={(_newContent, hasChanges) => {
                if (onDirtyStateChange) {
                  onDirtyStateChange(hasChanges);
                }
              }}
              onSave={(savedContent) => {
                if (onDirtyStateChange) {
                  onDirtyStateChange(false);
                }
              }}
            />
          );
        }
        
        return (
          <DiffEditor
            key={diffViewerKey}
            originalContent={originalCode}
            modifiedContent={modifiedCode}
            filePath={diffFilePath}
            workspacePath={workspacePath || diffMigrationContext?.workspacePath}
            revealLine={diffData.revealLine}
            readOnly={false}
            renderSideBySide={true}
            enableLsp={true}
            onSave={async (content) => {
              try {
                const targetWorkspacePath = workspacePath || diffMigrationContext?.workspacePath;
                if (!targetWorkspacePath || !diffFilePath) {
                  log.warn('DiffEditor save failed: missing workspacePath or filePath');
                  return;
                }

                const { workspaceAPI } = await import('@/infrastructure/api');
                await workspaceAPI.writeFileContent(targetWorkspacePath, diffFilePath, content);

                if (onDirtyStateChange) {
                  onDirtyStateChange(false);
                }
              } catch (error) {
                log.error('DiffEditor save failed', error);
                throw error;
              }
            }}
          />
        );

      case 'git-diff':
        return (
          <React.Suspense fallback={<div>{t('flexiblePanel.loading.gitDiff')}</div>}>
            <GitDiffView 
              repositoryPath={content.data?.repositoryPath || workspacePath || ''}
              sourceCommit={content.data?.sourceCommit}
              targetCommit={content.data?.targetCommit}
              filePath={content.data?.filePath}
            />
          </React.Suspense>
        );

      case 'git-graph':
        return (
          <React.Suspense fallback={<div>{t('flexiblePanel.loading.gitGraph')}</div>}>
            <GitGraphView 
              repositoryPath={content.data?.repositoryPath || workspacePath || ''}
              maxCount={content.data?.maxCount}
            />
          </React.Suspense>
        );

      case 'git-branch-history':
        return (
          <React.Suspense fallback={<div>{t('flexiblePanel.loading.gitBranchHistory')}</div>}>
            <GitBranchHistoryView 
              repositoryPath={content.data?.repositoryPath || workspacePath || ''}
              branchName={content.data?.branchName || 'main'}
              currentBranch={content.data?.currentBranch}
              maxCount={content.data?.maxCount || 100}
            />
          </React.Suspense>
        );

      case 'ai-session':
        return (
          <div className="ai-session-content">
            <div className="session-header">
              <h3>{t('flexiblePanel.aiSession.title', { sessionId: content.data?.sessionId?.slice(0, 8) || t('flexiblePanel.aiSession.unknown') })}</h3>
              <div className="session-info">
                <span className="agent-type">{content.data?.agent_info?.agent_type || t('flexiblePanel.aiSession.unknown')}</span>
                <span className="model-name">({content.data?.agent_info?.model_name || t('flexiblePanel.aiSession.unknown')})</span>
              </div>
            </div>
            <div className="session-details">
              <div className="detail-item">
                <span className="label">{t('flexiblePanel.aiSession.sessionStatus')}</span>
                <span className={`status status-${content.data?.status?.toLowerCase() || 'unknown'}`}>
                  {content.data?.status || t('flexiblePanel.aiSession.unknown')}
                </span>
              </div>
              <div className="detail-item">
                <span className="label">{t('flexiblePanel.aiSession.operationsCount')}</span>
                <span className="value">{t('flexiblePanel.aiSession.operationsValue', { count: content.data?.operations?.length || 0 })}</span>
              </div>
              <div className="detail-item">
                <span className="label">{t('flexiblePanel.aiSession.startTime')}</span>
                <span className="value">
                  {content.data?.start_time ? new Date(content.data.start_time).toLocaleString() : t('flexiblePanel.aiSession.unknownTime')}
                </span>
              </div>
            </div>
            {content.data?.operations && content.data.operations.length > 0 && (
              <div className="operations-list">
                <h4>{t('flexiblePanel.aiSession.fileOperations')}</h4>
                {content.data.operations.map((operation: any, index: number) => (
                  <div key={operation.operation_id || index} className="operation-item">
                    <div className="operation-header">
                      <span className={`operation-type type-${operation.operation_type?.toLowerCase() || 'unknown'}`}>
                        {operation.operation_type || t('flexiblePanel.aiSession.unknown')}
                      </span>
                      <span className="file-path">{operation.file_path || t('flexiblePanel.aiSession.unknown')}</span>
                    </div>
                    <div className="operation-status">
                      <span className={`status status-${operation.status?.toLowerCase() || 'unknown'}`}>
                        {operation.status || t('flexiblePanel.aiSession.unknown')}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        );

      case 'git-settings':
        return (
          <React.Suspense fallback={<div>{t('flexiblePanel.loading.gitSettings')}</div>}>
            <GitSettingsView 
              repositoryPath={content.data?.repositoryPath || workspacePath || ''}
            />
          </React.Suspense>
        );


      case 'task-detail':
        const taskDetailData = content.data || {};
        return (
          <React.Suspense fallback={<div className="bitfun-flexible-panel__loading">{t('flexiblePanel.loading.taskDetail')}</div>}>
            <TaskDetailPanel data={taskDetailData} />
          </React.Suspense>
        );

      case 'plan-viewer':
        const planViewerData = content.data || {};
        const planFilePath = planViewerData.filePath || '';
        const planFileName = planViewerData.fileName || content.title;
        const planWorkspacePath = planViewerData.workspacePath || workspacePath;
        const planJumpToLine = planViewerData.jumpToLine;
        const planJumpToColumn = planViewerData.jumpToColumn;
        
        if (!planFilePath) {
          return (
            <div className="bitfun-flexible-panel__error-message">
              <AlertCircle size={20} />
              <p>{t('flexiblePanel.errors.planViewerMissingPath')}</p>
            </div>
          );
        }
        
        return (
          <React.Suspense fallback={<div className="bitfun-flexible-panel__loading">{t('flexiblePanel.loading.planViewer')}</div>}>
            <PlanViewer
              filePath={planFilePath}
              fileName={planFileName}
              workspacePath={planWorkspacePath}
              jumpToLine={planJumpToLine}
              jumpToColumn={planJumpToColumn}
            />
          </React.Suspense>
        );

      case 'terminal':
        // Terminal panel
        const terminalData = content.data || {};
        const sessionId = terminalData.sessionId;
        
        if (!sessionId) {
          return (
            <div className="bitfun-flexible-panel__error-message">
              <AlertCircle size={20} />
              <p>{t('flexiblePanel.errors.terminalMissingSessionId')}</p>
            </div>
          );
        }
        
        return (
          <React.Suspense fallback={<div className="bitfun-flexible-panel__loading">{t('flexiblePanel.loading.terminal')}</div>}>
            <div className="bitfun-flexible-panel__terminal-container">
              <TerminalTabPanel
                sessionId={sessionId}
                autoFocus={true}
              />
            </div>
          </React.Suspense>
        );

      default:
        return (
          <div className="bitfun-flexible-panel__unknown-content">
            <div className="bitfun-flexible-panel__unknown-icon">
              <AlertCircle size={48} />
            </div>
            <h3>{t('flexiblePanel.unknownContent.title')}</h3>
            <p>{t('flexiblePanel.unknownContent.description')}</p>
            <div className="bitfun-flexible-panel__unknown-meta">
              <code>{t('flexiblePanel.unknownContent.contentType', { type: content.type })}</code>
            </div>
          </div>
        );
    }
  };

  const showHeader = content && shouldShowHeader(content.type);

  return (
    <div className={`bitfun-flexible-panel ${className}`}>
      {showHeader && (
        <div className="bitfun-flexible-panel__header">
          <div className="bitfun-flexible-panel__header-left">
            <div className="bitfun-flexible-panel__content-icon">
              {getContentIcon(content.type)}
            </div>
            <div className="bitfun-flexible-panel__content-info">
              <span className="bitfun-flexible-panel__content-title">
                {content.title || getContentTypeName(content.type)}
              </span>
              <span className="bitfun-flexible-panel__content-type">
                {getContentTypeName(content.type)}
              </span>
            </div>
          </div>

          <div className="bitfun-flexible-panel__header-right">
            {content && content.type !== 'empty' && (
              <>
                <IconButton
                  size="xs"
                  onClick={handleCopy}
                  tooltip={t('flexiblePanel.actions.copyContent')}
                >
                  <Copy size={14} />
                </IconButton>

                <IconButton
                  size="xs"
                  onClick={handleDownload}
                  tooltip={t('flexiblePanel.actions.downloadContent')}
                >
                  <Download size={14} />
                </IconButton>
              </>
            )}
            
            <IconButton
              size="xs"
              variant="danger"
              onClick={handleClose}
              tooltip={t('flexiblePanel.actions.close')}
            >
              <X size={14} />
            </IconButton>
          </div>
        </div>
      )}

      <div className="bitfun-flexible-panel__content">
        {renderContent()}
      </div>
    </div>
  );
});

FlexiblePanel.displayName = 'FlexiblePanel';

export default FlexiblePanel;

