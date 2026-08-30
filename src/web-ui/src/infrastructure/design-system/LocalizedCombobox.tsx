import {
  Combobox as DesignSystemCombobox,
  type ComboboxProps as DesignSystemComboboxProps,
} from '@bitfun/ui';
import { forwardRef } from 'react';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useI18n } from '@/infrastructure/i18n';

export type LocalizedComboboxProps = Omit<DesignSystemComboboxProps, 'portalContainer'> & {
  portalContainer?: DesignSystemComboboxProps['portalContainer'];
};

/**
 * Product adapter for the public Combobox contract.
 *
 * @bitfun/ui owns selection, search, keyboard behavior, accessibility and
 * styling. Web UI supplies only locale copy and the Appearance overlay host.
 */
export const LocalizedCombobox = forwardRef<HTMLDivElement, LocalizedComboboxProps>(
  function LocalizedCombobox({
    clearLabel,
    customValueHint,
    emptyText,
    loadingText,
    placeholder,
    portalContainer,
    searchPlaceholder,
    selectAllLabel,
    ...props
  }, ref) {
    const { t } = useI18n('components');
    return (
      <DesignSystemCombobox
        {...props}
        clearLabel={clearLabel ?? t('input.clear')}
        customValueHint={customValueHint ?? (customValue => t('select.useCustomValue', {
          value: customValue,
        }))}
        emptyText={emptyText ?? t('select.emptyText')}
        loadingText={loadingText ?? t('select.loading')}
        placeholder={placeholder ?? t('select.placeholder')}
        portalContainer={portalContainer ?? getAppearanceOverlayHost}
        ref={ref}
        searchPlaceholder={searchPlaceholder ?? t('select.search')}
        selectAllLabel={selectAllLabel ?? t('select.selectAll')}
      />
    );
  },
);
