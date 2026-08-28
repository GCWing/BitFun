import test from 'node:test';
import assert from 'node:assert/strict';

import { mobilePreviewScenarios } from './generated/mobile-design-data.js';

const expectedScenarioIds = [
  'connected-conversation',
  'streaming-dark',
  'reconnecting-wide',
];
const requiredScenarioFields = [
  'id',
  'title',
  'appearance',
  'header.title',
  'header.subtitle',
  'composer.phase',
  'composer.streaming',
];
const sensitiveContentPatterns = [
  /akid/i,
  /sk-/i,
  /-----begin/i,
  /password/i,
  /secret/i,
  /token/i,
  /[A-Za-z0-9+/=]{40,}/,
];

const hasOwn = (value, key) => Object.prototype.hasOwnProperty.call(value, key);


test('mobile preview scenarios satisfy the deterministic contract', () => {
  const scenarios = mobilePreviewScenarios.scenarios;

  assert.equal(scenarios.length, 3);
  assert.deepEqual(
    scenarios.map((scenario) => scenario.id),
    expectedScenarioIds,
  );

  for (const scenario of scenarios) {
    for (const field of requiredScenarioFields) {
      const [root, nested] = field.split('.');
      const value = nested === undefined ? scenario : scenario[root]?.[nested];
      assert.ok(
        nested === undefined ? hasOwn(scenario, root) : hasOwn(scenario[root], nested),
        `scenario ${scenario.id} is missing ${field}`,
      );
      assert.notEqual(value, undefined, `scenario ${scenario.id} has undefined ${field}`);
    }

    assert.ok(hasOwn(scenario.viewport, 'width'));
    assert.ok(hasOwn(scenario.viewport, 'height'));
    assert.ok(Array.isArray(scenario.messages));
    assert.ok(scenario.messages.length > 0);
  }

  const serializedScenarios = JSON.stringify(mobilePreviewScenarios);
  for (const pattern of sensitiveContentPatterns) {
    assert.doesNotMatch(serializedScenarios, pattern);
  }
});
