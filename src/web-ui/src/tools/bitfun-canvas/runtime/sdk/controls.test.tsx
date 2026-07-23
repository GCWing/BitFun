import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
  Checkbox,
  IconButton,
  Select,
  TextArea,
  TextInput,
  Toggle,
} from './controls';

describe('BitFun Canvas control adapters', () => {
  it('renders toggle and checkbox through component-library controls', () => {
    const markup = renderToStaticMarkup(
      <>
        <Toggle checked label="Enabled" />
        <Checkbox checked label="Reviewed" />
      </>,
    );

    expect(markup).toContain('bitfun-switch');
    expect(markup).toContain('bitfun-checkbox');
    expect(markup).toContain('Enabled');
    expect(markup).toContain('Reviewed');
  });

  it('renders a sandbox-native select without host i18n requirements', () => {
    const markup = renderToStaticMarkup(
      <Select
        value="beta"
        placeholder="Choose"
        options={[
          'alpha',
          { label: 'Beta', value: 'beta' },
          { label: 'Disabled', value: 'disabled', disabled: true },
        ]}
      />,
    );

    expect(markup).toContain('class="bf-select bf-select--medium"');
    expect(markup).toContain('<option value="">Choose</option>');
    expect(markup).toContain('<option value="beta" selected="">Beta</option>');
    expect(markup).toContain('disabled=""');
  });

  it('renders text inputs and icon button adapters', () => {
    const markup = renderToStaticMarkup(
      <>
        <TextInput value="query" label="Search" readOnly />
        <TextArea value="notes" label="Notes" readOnly />
        <IconButton title="Refresh">R</IconButton>
      </>,
    );

    expect(markup).toContain('bitfun-input-wrapper');
    expect(markup).toContain('bitfun-textarea');
    expect(markup).toContain('icon-btn');
    expect(markup).toContain('Refresh');
  });
});
