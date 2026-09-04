import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const readSource = (relativePath: string): string => readFileSync(
  fileURLToPath(new URL(relativePath, import.meta.url)),
  'utf8',
);

describe('GalleryDetailModal presentation contract', () => {
  it('supports an accessible hero title without changing the default gallery layout', () => {
    const source = readSource('./GalleryDetailModal.tsx');
    const appearance = readSource('./GalleryDetailModal.appearance.ts');

    expect(source).toContain("titlePlacement = 'header'");
    expect(source).toContain('aria-labelledby={usesHeroTitle ? heroTitleId : undefined}');
    expect(source).toContain('<DialogTitle data-testid={titleTestId}>{title}</DialogTitle>');
    expect(source).toContain('data-openbitfun-part="title"');
    expect(appearance).toContain("{ id: 'title' }");
  });

  it('lets Agent details use one stable configuration view in the real application dialog', () => {
    const source = readSource('./GalleryDetailModal.tsx');
    const styles = readSource('./GalleryDetailModal.scss');
    const agentsScene = readSource('../../scenes/agents/AgentsScene.tsx');
    const agentCardStyles = readSource('../../scenes/agents/components/AgentCard.scss');

    expect(source).toContain("size = 'md'");
    expect(source).toContain('size={size}');
    expect(agentsScene).toContain('titlePlacement="hero"');
    expect(agentsScene).toContain('size="2xl"');
    expect(agentsScene).toContain('stableHeight');
    expect(agentsScene).toContain('agent-card__configuration');
    expect(agentsScene).toContain('agent-card__config-nav');
    expect(agentsScene).not.toContain('AgentDetailView');
    expect(agentsScene).not.toContain('agent-card__detail-view-tabs');
    expect(agentsScene).not.toContain('agent-detail-overview');
    expect(agentsScene).toContain('agent-detail-basic-section');
    expect(agentsScene).toContain('agent-detail-capabilities-section');
    expect(styles).toContain('container-name: gallery-detail-modal;');
    expect(styles).not.toContain('width: min(900px, 100%);');
    expect(styles).toContain('height: min(540px, calc(100vh - 48px));');
    expect(styles).toContain('@container gallery-detail-modal (max-width: 360px)');
    expect(styles).not.toContain('@media (max-width: 720px)');
    expect(agentCardStyles).toContain('@container gallery-detail-modal (max-width: 620px)');
    expect(agentCardStyles).toContain('@container gallery-detail-modal (max-width: 460px)');
    expect(agentCardStyles).toContain('grid-template-columns: 150px minmax(0, 1fr);');
    expect(agentCardStyles).toMatch(/&__config-main\s*\{[\s\S]*?height:\s*100%;/);
    expect(agentCardStyles).toMatch(/> \.agent-card__config-panel,[\s\S]*?min-height:\s*100%;/);
    expect(agentCardStyles).toContain('grid-template-rows: repeat(2, minmax(52px, auto)) minmax(76px, 1fr);');
    expect(agentCardStyles).not.toContain('&__detail-view-tabs');
    expect(agentCardStyles).not.toContain('&__overview');
    expect(agentCardStyles).toMatch(/&__config-nav-item\s*\{[\s\S]*?border:\s*0;/);
    expect(agentCardStyles).not.toContain('box-shadow: inset 2px 0 0');
    expect(agentCardStyles).not.toContain('box-shadow: inset 0 -2px 0');
  });
});
