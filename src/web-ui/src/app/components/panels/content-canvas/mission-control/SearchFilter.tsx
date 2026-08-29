/**
 * SearchFilter component.
 * Search filter in mission control using the design system SearchField.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
;
import { Icon, SearchField } from '@bitfun/ui';
import './SearchFilter.scss';

export interface SearchFilterProps {
  /** Search query */
  value: string;
  /** Change callback */
  onChange: (value: string) => void;
  /** Match count */
  matchCount: number;
  /** Total count */
  totalCount: number;
  /** Auto focus */
  autoFocus?: boolean;
}

export const SearchFilter: React.FC<SearchFilterProps> = ({
  value,
  onChange,
  matchCount,
  totalCount,
  autoFocus = true,
}) => {
  const { t } = useTranslation('components');
  const countText = value
    ? `${matchCount} / ${totalCount}`
    : t('canvas.filesCount', { count: totalCount });

  return (
    <div data-bf-component="content-canvas" data-bf-part="searchFilter" className="canvas-search-filter">
      <SearchField
        value={value}
        onValueChange={onChange}
        placeholder={t('canvas.searchPlaceholder')}
        aria-label={t('canvas.searchPlaceholder')}
        leadingIcon={<Icon name="search" size="sm" aria-hidden />}
        autoFocus={autoFocus}
        size="md"
        className="canvas-search-filter__search"
        trailing={
          <span className="canvas-search-filter__count">{countText}</span>
        }
        clearLabel={value ? t('search.clear') : undefined}
        onClear={value ? () => onChange('') : undefined}
      />
    </div>
  );
};

SearchFilter.displayName = 'SearchFilter';

export default SearchFilter;
