/**
 * W4 P2-1 一致性断言反向分支测试（2026-08-13，梦情退回修正）。
 *
 * detectMapSetDivergence 纯逻辑：
 *  - 两端都启用 / 都未启用 → null（一致）
 *  - setup 启用但 main 未启用 → 正向分支错误（生产缺失，081cbb536 教训）
 *  - main 启用但 setup 未启用 → 反向分支错误（测试缺失——必须可测，非死代码）
 */
import { describe, expect, it } from 'vitest';
import { detectMapSetDivergence } from './setup';

const MAIN_WITH_MAPSET = 'import { enableMapSet } from "immer";\nenableMapSet();\n';
const SETUP_WITH_MAPSET = "import { enableMapSet } from 'immer';\nenableMapSet();\n";
const NO_MAPSET = 'export const x = 1;\n';

describe('global plugin initialization consistency (W4 P2-1)', () => {
  it('两端都启用 → 一致（null）', () => {
    expect(detectMapSetDivergence(MAIN_WITH_MAPSET, SETUP_WITH_MAPSET)).toBeNull();
  });

  it('两端都未启用 → 一致（null，都不崩则无分叉）', () => {
    expect(detectMapSetDivergence(NO_MAPSET, NO_MAPSET)).toBeNull();
  });

  it('正向分支：setup 启用但 main 未启用 → 报错（生产缺失）', () => {
    const err = detectMapSetDivergence(NO_MAPSET, SETUP_WITH_MAPSET);
    expect(err).not.toBeNull();
    expect(err).toContain('test/setup.ts enables enableMapSet()');
  });

  it('反向分支：main 启用但 setup 未启用 → 报错（测试缺失，可测非死代码）', () => {
    const err = detectMapSetDivergence(MAIN_WITH_MAPSET, NO_MAPSET);
    expect(err).not.toBeNull();
    expect(err).toContain('main.tsx enables enableMapSet()');
  });
});
