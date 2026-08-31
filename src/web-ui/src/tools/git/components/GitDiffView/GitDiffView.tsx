/** Git diff view. */

import { Button, Icon, IconButton, SegmentedControl } from '@bitfun/ui';
import React, { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { FileText, Minus, EyeOff, AlertCircle } from 'lucide-react';

import { gitService } from '../../services';
import { createLogger } from '@/shared/utils/logger';
import './GitDiffView.scss';

const log = createLogger('GitDiffView');

interface GitDiffViewProps {
  /** Repository path */
  repositoryPath: string;
  /** Source commit hash */
  sourceCommit?: string;
  /** Target commit hash */
  targetCommit?: string;
  /** Optional file path filter */
  filePath?: string;
  /** Whether to show staged diff */
  showStaged?: boolean;
  /** Class name */
  className?: string;
}

interface DiffFile {
  path: string;
  oldPath?: string;
  status: 'added' | 'modified' | 'deleted' | 'renamed';
  additions: number;
  deletions: number;
  diff: string;
  expanded: boolean;
}

/** Parses a `git diff` output into a list of file-level diffs. */
const parseDiffOutput = (diffOutput: string): DiffFile[] => {
  const files: DiffFile[] = [];
  

  if (!diffOutput || diffOutput.trim() === '') {
    return files;
  }
  

  const fileSections = diffOutput.split(/^diff --git /m).filter(section => section.trim() !== '');
  
  for (const section of fileSections) {
    const lines = section.split('\n');
    const firstLine = `diff --git ${lines[0]}`;
    

    const pathMatch = firstLine.match(/diff --git a\/(.+) b\/(.+)/) || 
                     firstLine.match(/diff --git "a\/(.+)" "b\/(.+)"/);
    if (!pathMatch) {
      log.warn('Failed to parse file path', { firstLine });
      continue;
    }
    
    const oldPath = pathMatch[1];
    const newPath = pathMatch[2];
    

    let status: DiffFile['status'] = 'modified';
    let additions = 0;
    let deletions = 0;
    

    const sectionText = section.toLowerCase();
    if (sectionText.includes('new file mode')) {
      status = 'added';
    } else if (sectionText.includes('deleted file mode')) {
      status = 'deleted';
    } else if (sectionText.includes('similarity index') && sectionText.includes('rename')) {
      status = 'renamed';
    } else if (oldPath !== newPath) {
      status = 'renamed';
    }
    

    for (const line of lines) {
      if (line.startsWith('+') && !line.startsWith('+++')) {
        additions++;
      } else if (line.startsWith('-') && !line.startsWith('---')) {
        deletions++;
      }
    }
    

    const fullDiff = firstLine + '\n' + lines.slice(1).join('\n');
    
    files.push({
      path: newPath,
      oldPath: oldPath !== newPath ? oldPath : undefined,
      status,
      additions,
      deletions,
      diff: fullDiff,
      expanded: files.length === 0
    });
  }
  
  return files;
};

const GitDiffView: React.FC<GitDiffViewProps> = ({
  repositoryPath,
  sourceCommit,
  targetCommit,
  filePath,
  showStaged = false,
  className = ''
}) => {
  const { t } = useTranslation('panels/git');
  const [diffFiles, setDiffFiles] = useState<DiffFile[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [currentShowStaged, setCurrentShowStaged] = useState(showStaged);
  const [allExpanded, setAllExpanded] = useState(false);

  const loadDiff = useCallback(async () => {
    if (!repositoryPath) {
      setError(t('diffView.errors.repositoryPathEmpty'));
      return;
    }

    setLoading(true);
    setError(null);

    try {

      const diffParams = {
        source: sourceCommit,
        target: targetCommit,
        files: filePath ? [filePath] : undefined,
        staged: currentShowStaged,
        stat: false
      };


      const diffOutput = await gitService.getDiff(repositoryPath, diffParams);
      
      if (!diffOutput || diffOutput.trim() === '') {
        setDiffFiles([]);
        return;
      }


      const parsedFiles = parseDiffOutput(diffOutput);
      setDiffFiles(parsedFiles);
      
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : t('diffView.errors.loadFailed');
      setError(errorMessage);
      log.error('Failed to load diff', { repositoryPath, sourceCommit, targetCommit, filePath, error: err });
    } finally {
      setLoading(false);
    }
  }, [repositoryPath, sourceCommit, targetCommit, filePath, currentShowStaged, t]);

  const toggleFileExpansion = useCallback((index: number) => {
    setDiffFiles(prev => prev.map((file, i) => 
      i === index ? { ...file, expanded: !file.expanded } : file
    ));
  }, []);

  const toggleAllExpansion = useCallback(() => {
    const newExpanded = !allExpanded;
    setAllExpanded(newExpanded);
    setDiffFiles(prev => prev.map(file => ({ ...file, expanded: newExpanded })));
  }, [allExpanded]);

  const getFileStatusIcon = useCallback((status: DiffFile['status']) => {
    switch (status) {
      case 'added': return <Icon name="plus" size="sm" />;
      case 'deleted': return <Minus size={14} />;
      default: return <FileText size={14} />;
    }
  }, []);

  const renderDiffContent = useCallback((diff: string) => {
    const lines = diff.split('\n');
    const diffLines: React.ReactElement[] = [];

    let inHunk = false;
    let lineNumber = 0;
    
    lines.forEach((line, index) => {
      let lineType = 'context';
      let content = line;

      if (line.startsWith('@@')) {
        lineType = 'hunk-header';
        inHunk = true;
      } else if (inHunk) {
        if (line.startsWith('+')) {
          lineType = 'added';
          content = line.substring(1);
        } else if (line.startsWith('-')) {
          lineType = 'deleted';
          content = line.substring(1);
        } else if (line.startsWith(' ')) {
          lineType = 'context';
          content = line.substring(1);
        }
      }


      if (!line.startsWith('diff ') && !line.startsWith('index ') && 
          !line.startsWith('--- ') && !line.startsWith('+++ ')) {
        lineNumber++;
        diffLines.push(
          <div data-bf-component="git-diff-view" data-bf-part="diffLine" key={index} className={`bitfun-git-diff-view__diff-line bitfun-git-diff-view__diff-line--${lineType}`}>
            <span data-bf-component="git-diff-view" data-bf-part="lineNumber" className="bitfun-git-diff-view__line-number">{lineNumber}</span>
            <span data-bf-component="git-diff-view" data-bf-part="lineContent" className="bitfun-git-diff-view__line-content">{content}</span>
          </div>
        );
      }
    });

    return diffLines;
  }, []);


  useEffect(() => {
    if (repositoryPath) {
      loadDiff();
    }
  }, [repositoryPath, loadDiff]);

  if (loading) {
    return (
      <div className={`bitfun-git-diff-view ${className}`} data-bf-component="git-diff-view" data-bf-part="root" data-bf-state="loading">
        <div data-bf-component="git-diff-view" data-bf-part="loading" className="bitfun-git-diff-view__loading-state">
          <div className="bitfun-git-diff-view__loading-spinner" />
          <p>{t('diffView.loading')}</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={`bitfun-git-diff-view ${className}`} data-bf-component="git-diff-view" data-bf-part="root" data-bf-state="error">
        <div data-bf-component="git-diff-view" data-bf-part="error" className="bitfun-git-diff-view__error-state">
          <FileText size={48} />
          <h3>{t('diffView.loadFailedTitle')}</h3>
          <p>{error}</p>
          <Button onClick={loadDiff} variant="fill" size="sm">
            {t('common.retry')}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className={`bitfun-git-diff-view ${className}`} data-bf-component="git-diff-view" data-bf-part="root">
      <div className="bitfun-git-diff-view__header" data-bf-component="git-diff-view" data-bf-part="header">
        <div data-bf-component="git-diff-view" data-bf-part="headerLeft" className="bitfun-git-diff-view__header-left">
          {sourceCommit && targetCommit && (
            <span className="bitfun-git-diff-view__commit-range">
              {sourceCommit.substring(0, 7)}...{targetCommit.substring(0, 7)}
            </span>
          )}
          {!sourceCommit && !targetCommit && (
            <SegmentedControl
              className="bitfun-git-diff-view__diff-type-switcher"
              options={[
                { value: 'working', label: t('diffView.workingTree') },
                { value: 'staged', label: t('diffView.staged') },
              ]}
              value={currentShowStaged ? 'staged' : 'working'}
              onValueChange={(value) => setCurrentShowStaged(value === 'staged')}
            />
          )}
          {loading && (
            <span className="bitfun-git-diff-view__loading-indicator">
              <Icon name="refresh" size="sm" className="spinning" />
              {t('common.loading')}
            </span>
          )}
        </div>
        
        <div data-bf-component="git-diff-view" data-bf-part="headerRight" className="bitfun-git-diff-view__header-right">
          <div className="bitfun-git-diff-view__view-options">
            <IconButton
              aria-label={allExpanded ? t('diffView.collapseAll') : t('diffView.expandAll')}
              onClick={toggleAllExpansion}
              size="sm"
              title={allExpanded ? t('diffView.collapseAll') : t('diffView.expandAll')}
              icon={allExpanded ? <EyeOff size={14} /> : <Icon name="eye" size="sm" />}
            />
            <IconButton
              aria-label={t('common.refresh')}
              onClick={loadDiff}
              disabled={loading}
              size="sm"
              title={t('common.refresh')}
              icon={<Icon name="refresh" size="md" />}
            />
          </div>
        </div>
      </div>

      <div className="bitfun-git-diff-view__content" data-bf-component="git-diff-view" data-bf-part="content">
        {error ? (
          <div data-bf-component="git-diff-view" data-bf-part="error" data-bf-state="error" className="bitfun-git-diff-view__error-state">
            <div className="error-icon">
              <AlertCircle size={20} />
            </div>
            <h3>{t('diffView.loadFailedTitle')}</h3>
            <p>{error}</p>
            <Button onClick={loadDiff} variant="fill" size="sm" leadingIcon={<Icon name="refresh" size="md" />}>

              {t('common.retry')}
            </Button>
          </div>
        ) : loading ? (
          <div data-bf-component="git-diff-view" data-bf-part="loading" data-bf-state="loading" className="bitfun-git-diff-view__loading-state">
            <div className="bitfun-git-diff-view__loading-spinner" />
            <p>{t('diffView.loadingData')}</p>
          </div>
        ) : diffFiles.length > 0 ? (
          <div data-bf-component="git-diff-view" data-bf-part="fileList" className="bitfun-git-diff-view__file-list">
            {diffFiles.map((file, index) => (
              <div data-bf-component="git-diff-view" data-bf-part="file" data-bf-state={file.expanded ? 'expanded' : undefined} key={file.path} className="bitfun-git-diff-view__file-item">
                <div 
                  data-bf-component="git-diff-view"
                  data-bf-part="fileHeader"
                  className="bitfun-git-diff-view__file-header"
                  onClick={() => toggleFileExpansion(index)}
                >
                  <div data-bf-component="git-diff-view" data-bf-part="fileInfo" className="bitfun-git-diff-view__file-info">
                    <span className={`bitfun-git-diff-view__expand-icon ${file.expanded ? 'bitfun-git-diff-view__expand-icon--expanded' : ''}`}>
                      {file.expanded ? <Icon name="chevron-down" size="md" /> : <Icon name="chevron-right" size="md" />}
                    </span>
                    
                    <span className="bitfun-git-diff-view__file-status-icon">
                      {getFileStatusIcon(file.status)}
                    </span>
                    
                    <span className="bitfun-git-diff-view__file-path">{file.path}</span>
                    
                    {file.oldPath && file.oldPath !== file.path && (
                      <span className={`bitfun-git-diff-view__file-status bitfun-git-diff-view__file-status--${file.status}`}>
                        ← {file.oldPath}
                      </span>
                    )}
                  </div>
                  
                  <div data-bf-component="git-diff-view" data-bf-part="fileStats" className="bitfun-git-diff-view__file-stats">
                    {file.additions > 0 && (
                      <span className="bitfun-git-diff-view__additions">
                        <Icon name="plus" size="xs" />
                        {file.additions}
                      </span>
                    )}
                    {file.deletions > 0 && (
                      <span className="bitfun-git-diff-view__deletions">
                        <Minus size={12} />
                        {file.deletions}
                      </span>
                    )}
                  </div>
                </div>
                
                {file.expanded && (
                  <div data-bf-component="git-diff-view" data-bf-part="diffContent" className="bitfun-git-diff-view__diff-content">
                    {renderDiffContent(file.diff)}
                  </div>
                )}
              </div>
            ))}
          </div>
        ) : (
          <div data-bf-component="git-diff-view" data-bf-part="empty" className="bitfun-git-diff-view__empty-state">
            <FileText size={48} />
            <h3>{t('diffView.empty.title')}</h3>
            <p>
              {!sourceCommit && !targetCommit 
                ? t('diffView.empty.workingTreeClean')
                : t('diffView.empty.noDiffBetweenCommits')
              }
            </p>
            {!repositoryPath && (
              <p>{t('diffView.empty.selectRepository')}</p>
            )}
          </div>
        )}
      </div>

    </div>
  );
};

export default GitDiffView;
