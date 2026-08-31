import { describe, expect, it } from 'vitest';

import { parsePlanMarkdown, serializePlanMarkdown } from './planDocument';

const VALID_PLAN = `---
name: Replace CreatePlan
overview: Create plans through the Write tool.
todos:
  - id: add-write-card
    content: Render .plan.md writes as plan cards
    status: pending
---

# Replace CreatePlan

Implement the Write-backed plan flow.`;

describe('planDocument', () => {
  it('parses a valid plan document', () => {
    expect(parsePlanMarkdown(VALID_PLAN)).toMatchObject({
      name: 'Replace CreatePlan',
      overview: 'Create plans through the Write tool.',
      todos: [{
        id: 'add-write-card',
        content: 'Render .plan.md writes as plan cards',
        status: 'pending',
      }],
      planContent: '# Replace CreatePlan\n\nImplement the Write-backed plan flow.',
    });
  });

  it('rejects incomplete plan frontmatter', () => {
    expect(() => parsePlanMarkdown(`---\nname: Missing overview\n---\n\n# Plan`))
      .toThrow("Plan frontmatter field 'overview'");
  });

  it('preserves unknown frontmatter fields while updating todos', () => {
    const document = parsePlanMarkdown(VALID_PLAN.replace('todos:', 'owner: product\ntodos:'));
    const serialized = serializePlanMarkdown(document, {
      todos: document.todos.map(todo => ({ ...todo, status: 'completed' })),
    });

    expect(serialized).toContain('owner: product');
    expect(parsePlanMarkdown(serialized).todos[0].status).toBe('completed');
  });
});
