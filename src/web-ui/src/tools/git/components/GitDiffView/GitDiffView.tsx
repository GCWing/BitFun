/** Git diff view. */

import React, { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { 
  FileText, 
  RefreshCw,
  ChevronDown,
  ChevronRight,
  Plus,
  Minus,
  Eye,
  EyeOff,
  AlertCircle
} from 'lucide-react';
import { Button, IconButton } from '@/component-library';
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
      case 'added': return <Plus size={14} />;
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
          <div key={index} className={`bitfun-git-diff-view__diff-line bitfun-git-diff-view__diff-line--${lineType}`}>
            <span className="bitfun-git-diff-view__line-number">{lineNumber}</span>
            <span className="bitfun-git-diff-view__line-content">{content}</span>
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
      <div className={`bitfun-git-diff-view ${className}`}>
        <div className="bitfun-git-diff-view__loading-state">
          <div className="bitfun-git-diff-view__loading-spinner" />
          <p>{t('diffView.loading')}</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={`bitfun-git-diff-view ${className}`}>
        <div className="bitfun-git-diff-view__error-state">
          <FileText size={48} />
          <h3>{t('diffView.loadFailedTitle')}</h3>
          <p>{error}</p>
          <Button onClick={loadDiff} variant="primary" size="small">
            {t('common.retry')}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className={`bitfun-git-diff-view ${className}`}>
      <div className="bitfun-git-diff-view__header">
        <div className="bitfun-git-diff-view__header-left">
          {sourceCommit && targetCommit && (
            <span className="bitfun-git-diff-view__commit-range">
              {sourceCommit.substring(0, 7)}...{targetCommit.substring(0, 7)}
            </span>
          )}
          {!sourceCommit && !targetCommit && (
            <div className="bitfun-git-diff-view__diff-type-switcher">
              <button 
                className={`bitfun-git-diff-view__type-btn ${!currentShowStaged ? 'bitfun-git-diff-view__type-btn--active' : ''}`}
                onClick={() => setCurrentShowStaged(false)}
              >
                {t('diffView.workingTree')}
              </button>
              <button 
                className={`bitfun-git-diff-view__type-btn ${currentShowStaged ? 'bitfun-git-diff-view__type-btn--active' : ''}`}
                onClick={() => setCurrentShowStaged(true)}
              >
                {t('diffView.staged')}
              </button>
            </div>
          )}
          {loading && (
            <span className="bitfun-git-diff-view__loading-indicator">
              <RefreshCw size={14} className="spinning" />
              {t('common.loading')}
            </span>
          )}
        </div>
        
        <div className="bitfun-git-diff-view__header-right">
          <div className="bitfun-git-diff-view__view-options">
            <IconButton
              onClick={toggleAllExpansion}
              size="small"
              variant="ghost"
              title={allExpanded ? t('diffView.collapseAll') : t('diffView.expandAll')}
            >
              {allExpanded ? <EyeOff size={14} /> : <Eye size={14} />}
            </IconButton>
            <IconButton
              onClick={loadDiff}
              disabled={loading}
              size="small"
              variant="ghost"
              title={t('common.refresh')}
            >
              <RefreshCw size={16} />
            </IconButton>
          </div>
        </div>
      </div>

      <div className="bitfun-git-diff-view__content">
        {error ? (
          <div className="bitfun-git-diff-view__error-state">
            <div className="error-icon">
              <AlertCircle size={20} />
            </div>
            <h3>{t('diffView.loadFailedTitle')}</h3>
            <p>{error}</p>
            <Button onClick={loadDiff} variant="primary" size="small">
              <RefreshCw size={16} />
              {t('common.retry')}
            </Button>
          </div>
        ) : loading ? (
          <div className="bitfun-git-diff-view__loading-state">
            <div className="bitfun-git-diff-view__loading-spinner" />
            <p>{t('diffView.loadingData')}</p>
          </div>
        ) : diffFiles.length > 0 ? (
          <div className="bitfun-git-diff-view__file-list">
            {diffFiles.map((file, index) => (
              <div key={file.path} className="bitfun-git-diff-view__file-item">
                <div 
                  className="bitfun-git-diff-view__file-header"
                  onClick={() => toggleFileExpansion(index)}
                >
                  <div className="bitfun-git-diff-view__file-info">
                    <span className={`bitfun-git-diff-view__expand-icon ${file.expanded ? 'bitfun-git-diff-view__expand-icon--expanded' : ''}`}>
                      {file.expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
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
                  
                  <div className="bitfun-git-diff-view__file-stats">
                    {file.additions > 0 && (
                      <span className="bitfun-git-diff-view__additions">
                        <Plus size={12} />
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
                  <div className="bitfun-git-diff-view__diff-content">
                    {renderDiffContent(file.diff)}
                  </div>
                )}
              </div>
            ))}
          </div>
        ) : (
          <div className="bitfun-git-diff-view__empty-state">
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