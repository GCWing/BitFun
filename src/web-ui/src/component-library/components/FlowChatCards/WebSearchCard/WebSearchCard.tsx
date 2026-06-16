/**
 * WebSearchCard - Web Search Tool Card Component
 * Displays web search and URL fetch results
 */

import React, { useState } from 'react';
import { Globe, ExternalLink, ChevronDown, ChevronUp } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { BaseToolCard, BaseToolCardProps } from '../BaseToolCard';
import './WebSearchCard.scss';

export interface WebSearchResult {
  title: string;
  url: string;
  snippet?: string;
}

export interface WebSearchCardProps extends Omit<BaseToolCardProps, 'toolName' | 'displayName'> {
  searchType?: 'search' | 'fetch';
  query?: string;
  results?: WebSearchResult[];
}

export const WebSearchCard: React.FC<WebSearchCardProps> = ({
  searchType = 'search',
  query,
  results,
  input,
  result,
  status = 'pending',
  displayMode = 'compact',
  ...baseProps
}) => {
  const { t } = useI18n('components');
  const [isExpanded, setIsExpanded] = useState(false);

  const resolvedQuery = query || input?.query || input?.search_query || input?.url || t('flowChatCards.webSearchCard.unspecifiedQuery');
  const resolvedResults = results || result?.results || [];

  const isSearch = searchType === 'search';
  const cardTitle = isSearch ? t('flowChatCards.webSearchCard.searchTitle') : t('flowChatCards.webSearchCard.fetchTitle');

  if (displayMode === 'compact') {
    return (
      <div className={`web-search-card web-search-card--compact web-search-card--${status} web-search-card--${searchType}`}>
        <Globe className="web-search-card__icon" size={14} />
        <span className="web-search-card__action">{cardTitle}:</span>
        <span className="web-search-card__query" title={resolvedQuery}>
          {resolvedQuery}
        </span>
        {status === 'completed' && resolvedResults.length > 0 && (
          <span className="web-search-card__count">
            {t('flowChatCards.webSearchCard.resultsCount', { count: resolvedResults.length })}
          </span>
        )}
      </div>
    );
  }

  return (
    <BaseToolCard
      toolName={isSearch ? 'WebSearch' : 'WebFetch'}
      displayName={cardTitle}
      icon={<Globe size={18} />}
      description={isSearch ? t('flowChatCards.webSearchCard.searchDesc') : t('flowChatCards.webSearchCard.fetchDesc')}
      status={status}
      displayMode={displayMode}
      input={input}
      result={result}
      primaryColor="var(--tool-card-web-search-color)"
      className={`web-search-card web-search-card--${searchType}`}
      {...baseProps}
    >
      <div className="web-search-card__info">
        <div className="web-search-card__info-row">
          <span className="web-search-card__label">{isSearch ? t('flowChatCards.webSearchCard.searchQuery') : t('flowChatCards.webSearchCard.url')}:</span>
          <span className="web-search-card__value">{resolvedQuery}</span>
        </div>
        {status === 'completed' && resolvedResults.length > 0 && (
          <div className="web-search-card__info-row">
            <span className="web-search-card__label">{t('flowChatCards.webSearchCard.resultCount')}:</span>
            <span className="web-search-card__value">{t('flowChatCards.webSearchCard.resultsCount', { count: resolvedResults.length })}</span>
          </div>
        )}
      </div>

      {status === 'completed' && resolvedResults.length > 0 && (
        <div className="web-search-card__results-section">
          <button
            className="web-search-card__results-header"
            onClick={() => setIsExpanded(!isExpanded)}
          >
            <Globe size={14} />
            <span>{t('flowChatCards.webSearchCard.results')}</span>
            {isExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>
          
          {isExpanded && (
            <div className="web-search-card__results-list">
              {resolvedResults.map((item: WebSearchResult, index: number) => (
                <div key={index} className="web-search-card__result-item">
                  <div className="web-search-card__result-header">
                    <span className="web-search-card__result-title">{item.title}</span>
                    {item.url && (
                      <a
                        href={item.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="web-search-card__result-link"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <ExternalLink size={12} />
                      </a>
                    )}
                  </div>
                  {item.snippet && (
                    <div className="web-search-card__result-snippet">{item.snippet}</div>
                  )}
                  {item.url && (
                    <div className="web-search-card__result-url">{item.url}</div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {status === 'completed' && resolvedResults.length === 0 && (
        <div className="web-search-card__no-results">
          {t('flowChatCards.webSearchCard.noResults')}
        </div>
      )}

      {(status === 'running' || status === 'streaming') && (
        <div className="web-search-card__searching">
          <Globe className="web-search-card__searching-icon" size={14} />
          <span>{t('flowChatCards.webSearchCard.searching')}</span>
        </div>
      )}
    </BaseToolCard>
  );
};
