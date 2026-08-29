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
  it('renders toggle and checkbox through the design system', () => {
    const markup = renderToStaticMarkup(
      <>
        <Toggle checked label="Enabled" />
        <Checkbox checked label="Reviewed" />
      </>,
    );

    expect(markup).toContain('data-bf-component="switch"');
    expect(markup).toContain('data-bf-component="checkbox"');
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

    expect(markup).toContain('data-bf-component="select"');
    expect(markup).toContain('data-size="md"');
    expect(markup).toContain('<option disabled="" value="">Choose</option>');
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

    expect(markup).toContain('data-bf-component="input"');
    expect(markup).toContain('data-bf-component="textarea"');
    expect(markup).toContain('data-bf-component="icon-button"');
    expect(markup).toContain('Refresh');
  });
});
