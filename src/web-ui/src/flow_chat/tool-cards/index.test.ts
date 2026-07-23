// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';

import { getToolCardComponent } from './index';
import { TaskToolDisplay } from './TaskToolDisplay';

describe('tool card registry', () => {
  it('projects managed Review workers through the unified coverage card', () => {
    expect(getToolCardComponent('LaunchReviewAgent')).toBe(TaskToolDisplay);
  });
});
