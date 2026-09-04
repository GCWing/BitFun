import { describe, expect, it } from 'vitest';
import * as sdk from './index';
import { CANVAS_SDK_RUNTIME_EXPORTS } from './contract.generated';

describe('Canvas SDK generated contract', () => {
  it('matches the actual runtime exports exactly', () => {
    expect(Object.keys(sdk).sort()).toEqual([...CANVAS_SDK_RUNTIME_EXPORTS].sort());
  });
});
