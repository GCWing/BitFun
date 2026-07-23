/**
 * SearchFilter component.
 * Search filter in mission control using the component library Search.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Search } from '@/component-library';
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
    <div className="canvas-search-filter">
      <Search
        value={value}
        onChange={onChange}
        placeholder={t('canvas.searchPlaceholder')}
        autoFocus={autoFocus}
        clearable
        size="medium"
        className="canvas-search-filter__search"
        suffixContent={
          <span className="canvas-search-filter__count">{countText}</span>
        }
      />
    </div>
  );
};

SearchFilter.displayName = 'SearchFilter';

export default SearchFilter;
