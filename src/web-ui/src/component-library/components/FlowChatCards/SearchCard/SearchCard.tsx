/**
 * SearchCard - search tool card component
 * Supports Grep text search and Glob file search
 */

import React, { useState, useMemo } from 'react';
import { Search, File, FolderOpen, ChevronDown, ChevronUp, Loader2, CheckCircle, XCircle } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { BaseToolCard, BaseToolCardProps } from '../BaseToolCard';
import { ToolProcessingDots } from '../ToolProcessingDots';
import './SearchCard.scss';

export interface SearchCardProps extends Omit<BaseToolCardProps, 'toolName' | 'displayName'> {
  searchType?: 'grep' | 'glob';
  pattern?: string;
  searchPath?: string;
  matches?: any[];
}

export const SearchCard: React.FC<SearchCardProps> = ({
  searchType = 'grep',
  pattern,
  searchPath,
  matches,
  input,
  result,
  status = 'pending',
  displayMode = 'compact',
  ...baseProps
}) => {
  const { t } = useI18n('components');
  const [isExpanded, setIsExpanded] = useState(false);

  const searchPattern = useMemo(() => {
    if (pattern) return pattern;
    if (searchType === 'grep') {
      return input?.pattern || input?.search_pattern || input?.query || input?.text || t('flowChatCards.searchCard.unspecifiedPattern');
    }
    return input?.pattern || input?.glob_pattern || input?.query || t('flowChatCards.searchCard.unspecifiedPattern');
  }, [pattern, searchType, input, t]);

  const resolvedPath = useMemo(() => {
    return searchPath || input?.path || t('flowChatCards.searchCard.currentDir');
  }, [searchPath, input, t]);

  const searchMatches = useMemo(() => {
    if (matches) return matches;
    if (!result) return [];
    
    if (Array.isArray(result)) return result;
    if (result.matches && Array.isArray(result.matches)) return result.matches;
    if (result.results && Array.isArray(result.results)) return result.results;
    if (result.files && Array.isArray(result.files)) return result.files;
    
    return [];
  }, [matches, result]);

  const stats = useMemo(() => {
    if (searchMatches.length === 0) return { matches: 0, files: 0 };
    
    const files = new Set(searchMatches.map((match: any) => 
      match.file || match.filename || match.path || match
    )).size;
    
    return {
      matches: searchMatches.length,
      files
    };
  }, [searchMatches]);

  const topFiles = useMemo(() => {
    const fileMap = new Map();
    
    searchMatches.forEach((match: any) => {
      const file = match.file || match.filename || match.path || match;
      if (file && typeof file === 'string') {
        fileMap.set(file, (fileMap.get(file) || 0) + 1);
      }
    });
    
    return Array.from(fileMap.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([file, count]) => ({ file, count }));
  }, [searchMatches]);

  const isGrepSearch = searchType === 'grep';
  const cardTitle = isGrepSearch ? t('flowChatCards.searchCard.grepTitle') : t('flowChatCards.searchCard.globTitle');
  const cardIcon = isGrepSearch ? <Search size={18} /> : <FolderOpen size={18} />;
  const cardColor = isGrepSearch ? '#8b5cf6' : '#06b6d4';

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="search-card__status-spinner" size={12} />;
      case 'completed':
        return <CheckCircle className="search-card__status-success" size={12} />;
      case 'error':
        return <XCircle className="search-card__status-error" size={12} />;
      default:
        return <ToolProcessingDots className="search-card__status-pending" size={12} />;
    }
  };

  if (displayMode === 'compact') {
    return (
      <div className={`search-card search-card--compact search-card--${searchType} status-${status}`}>
        {isGrepSearch ? (
          <Search className="search-card__icon" size={14} />
        ) : (
          <FolderOpen className="search-card__icon" size={14} />
        )}
        <span className="search-card__action">{cardTitle}:</span>
        <span className="search-card__pattern">"{searchPattern}"</span>
        {status === 'completed' && stats.matches > 0 && (
          <span className="search-card__result">
            → {stats.matches} {t('flowChatCards.searchCard.matches')}
          </span>
        )}
        <span className="search-card__status">{getStatusIcon()}</span>
      </div>
    );
  }

  return (
    <BaseToolCard
      toolName={isGrepSearch ? 'Grep' : 'Glob'}
      displayName={cardTitle}
      icon={cardIcon}
      description={isGrepSearch ? t('flowChatCards.searchCard.grepDesc') : t('flowChatCards.searchCard.globDesc')}
      status={status}
      displayMode={displayMode}
      input={input}
      result={result}
      primaryColor={cardColor}
      className={`search-card search-card--${searchType}`}
      {...baseProps}
    >
      <div className="search-card__info">
        <div className="search-card__info-row">
          <span className="search-card__label">{t('flowChatCards.searchCard.pattern')}:</span>
          <span className="search-card__value">{searchPattern}</span>
        </div>
        <div className="search-card__info-row">
          <span className="search-card__label">{t('flowChatCards.searchCard.path')}:</span>
          <span className="search-card__value">{resolvedPath}</span>
        </div>
      </div>

      {status === 'completed' && (
        <div className="search-card__stats-box">
          <div className="search-card__stat-item">
            <span className="search-card__stat-value">{stats.matches}</span>
            <span className="search-card__stat-label">{t('flowChatCards.searchCard.matches')}</span>
          </div>
          <div className="search-card__stat-item">
            <span className="search-card__stat-value">{stats.files}</span>
            <span className="search-card__stat-label">{t('flowChatCards.searchCard.files')}</span>
          </div>
        </div>
      )}

      {status === 'completed' && topFiles.length > 0 && (
        <div className="search-card__top-files">
          <button
            className="search-card__expand-button"
            onClick={() => setIsExpanded(!isExpanded)}
          >
            <File size={14} />
            <span>{t('flowChatCards.searchCard.matchingFiles')} ({topFiles.length})</span>
            {isExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>
          
          {isExpanded && (
            <div className="search-card__file-list">
              {topFiles.map(({ file, count }, index) => (
                <div key={index} className="search-card__file-item">
                  <File size={12} />
                  <span className="search-card__file-name" title={file}>
                    {file.split('/').pop() || file}
                  </span>
                  <span className="search-card__file-count">{t('flowChatCards.searchCard.matchCount', { count })}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {status === 'completed' && stats.matches === 0 && (
        <div className="search-card__no-results">
          {isGrepSearch ? t('flowChatCards.searchCard.noTextMatch') : t('flowChatCards.searchCard.noFileMatch')}
        </div>
      )}
    </BaseToolCard>
  );
};
