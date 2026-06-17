import { describe, expect, it } from 'vitest';
import {
  joinMarkdownFrontmatter,
  parseIdentityDocument,
  serializeIdentityDocument,
  splitMarkdownFrontmatter,
} from './identityDocument';

describe('identityDocument frontmatter helpers', () => {
  it('splits markdown frontmatter from the body', () => {
    const content = [
      '---',
      'name: Demo',
      'emoji: 🙂',
      '---',
      '',
      '# Body',
      '',
      'Hello',
    ].join('\n');

    expect(splitMarkdownFrontmatter(content)).toEqual({
      hasFrontmatter: true,
      frontmatter: 'name: Demo\nemoji: 🙂',
      body: '# Body\n\nHello',
    });
  });

  it('rejoins markdown frontmatter and body as a markdown document', () => {
    const content = joinMarkdownFrontmatter('name: Demo\nemoji: 🙂', '# Body\n\nHello');

    expect(content).toBe('---\nname: Demo\nemoji: 🙂\n---\n\n# Body\n\nHello\n');
  });

  it('can preserve an empty frontmatter block while editing', () => {
    const content = joinMarkdownFrontmatter('', '# Body', {
      preserveFrontmatterBlock: true,
    });

    expect(content).toBe('---\n\n---\n\n# Body\n');
  });

  it('serializes identity documents through the frontmatter join helper', () => {
    const serialized = serializeIdentityDocument({
      name: 'Demo',
      creature: 'Cat',
      vibe: 'Calm',
      emoji: '🙂',
      body: '# Body\n\nHello',
      modelPrimary: '',
      modelFast: '',
    });

    expect(serialized).toBe([
      '---',
      'name: Demo',
      'creature: Cat',
      'vibe: Calm',
      'emoji: 🙂',
      '---',
      '',
      '# Body',
      '',
      'Hello',
      '',
    ].join('\n'));
  });

  it('parses identity frontmatter fields and markdown body independently', () => {
    const parsed = parseIdentityDocument([
      '---',
      'name: Demo',
      'creature: Cat',
      'vibe: Calm',
      'emoji: 🙂',
      '---',
      '',
      '# Body',
      '',
      'Hello',
    ].join('\n'));

    expect(parsed).toEqual({
      name: 'Demo',
      creature: 'Cat',
      vibe: 'Calm',
      emoji: '🙂',
      body: '# Body\n\nHello',
      modelPrimary: '',
      modelFast: '',
    });
  });
});
