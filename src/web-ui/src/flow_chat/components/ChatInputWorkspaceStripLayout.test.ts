import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readLocalFile(name: string): string {
  return readFileSync(fileURLToPath(new URL(`./${name}`, import.meta.url)), 'utf8')
    .replace(/\r\n/g, '\n');
}

const readWorkspaceStripStylesheet = () => readLocalFile('ChatInputWorkspaceStrip.scss');
const readChatInputStylesheet = () => readLocalFile('ChatInput.scss');
const readWorkspaceStripComponent = () => readLocalFile('ChatInputWorkspaceStrip.tsx');

describe('status track layout', () => {
  it('is two fixed rails rather than a conditional column template', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toContain('justify-content: space-between;');
    expect(stylesheet).toContain('&__context {');
    expect(stylesheet).toContain('&__next {');
    expect(stylesheet).toContain('min-height: 20px;');
    expect(stylesheet).toContain('border: 0;');
    expect(stylesheet).toContain('background: transparent;');
    // The eight conditional grid templates are what made a control appearing
    // on one side reflow the other. There is no replacement for them.
    expect(stylesheet).not.toContain('grid-template-columns');
    expect(stylesheet).not.toContain('--with-harness');
    expect(stylesheet).not.toContain('--with-policy');
    expect(stylesheet).not.toContain('--with-runtime');
    expect(stylesheet).not.toContain('--actions-only');
    expect(stylesheet).not.toContain('justify-self: end;');
  });

  it('gives the left rail the slack and lets the right rail keep its width', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toMatch(/&__context \{[\s\S]*?flex: 1 1 auto;/);
    expect(stylesheet).toMatch(/&__next \{[\s\S]*?flex: 0 0 auto;/);
  });

  it('keeps one text size across both rails', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toContain(
      'font-size: var(--bf-appearance-token-flowchat-font-size-xs);\n  line-height:',
    );
    expect(stylesheet).toMatch(/&__workspace \{[\s\S]*?font-size: inherit;/);
    expect(stylesheet).toMatch(/&__branch \{[\s\S]*?font-size: inherit;/);
    expect(stylesheet).toMatch(/&__permission-trigger \{[\s\S]*?font-size: inherit;/);
    expect(stylesheet).toMatch(
      /&__usage-btn \{[\s\S]*?font-size: var\(--bf-appearance-token-flowchat-font-size-xs\);/,
    );
  });

  it('orders the left rail as situation and the right rail as the next turn', () => {
    const component = readWorkspaceStripComponent();
    const contextIndex = component.indexOf('data-bf-part="context"');
    const nextIndex = component.indexOf('data-bf-part="next"');

    expect(contextIndex).toBeGreaterThan(-1);
    expect(nextIndex).toBeGreaterThan(contextIndex);
    // Situation: destination, workspace, branch, and the goal being chased.
    expect(component).toContain('<DispatchTargetPicker');
    expect(component).toContain('<FolderPen size={14}');
    expect(component).toContain('bitfun-chat-input-workspace-strip__breadcrumb-separator');
    expect(component).toContain('aria-hidden>/</span>');
    expect(component).toContain('bitfun-chat-input-workspace-strip__rail-separator');
    expect(component).toContain('<ThreadGoalStripButton');
    // Next turn: how much confirmation it asks for, and how much room is left.
    expect(component).toContain('data-testid="chat-input-permission-trigger"');
    expect(component).toContain('<span>{usagePercentage}%</span>');
    expect(component).toContain('bitfun-chat-input-workspace-strip__usage-ring');
    expect(component).toContain('data-testid="dispatch-sync-trigger"');
  });

  it('drops the relay hosts now that each control renders where it belongs', () => {
    const component = readWorkspaceStripComponent();

    expect(component).not.toContain('reasoning-host');
    expect(component).not.toContain('permission-host');
    expect(component).not.toContain('HarnessProfileSelector');
    expect(readWorkspaceStripStylesheet()).not.toContain('&__permission-host');
    expect(readLocalFile('ModelSelector.tsx')).not.toContain('reasoningControlHost');
  });

  it('orders the add entry, Harness, and selected Agent/Mode in semantic DOM', () => {
    const chatInput = readLocalFile('ChatInput.tsx');
    const harnessIndex = chatInput.indexOf('<HarnessProfileSelector');
    const agentBoostIndex = chatInput.indexOf('data-testid="chat-input-agent-boost"');
    const agentModeChipIndex = chatInput.indexOf('data-testid="chat-input-agent-mode-chip"');

    expect(harnessIndex).toBeGreaterThan(-1);
    expect(agentBoostIndex).toBeGreaterThan(-1);
    expect(harnessIndex).toBeGreaterThan(agentBoostIndex);
    expect(agentModeChipIndex).toBeGreaterThan(harnessIndex);
    expect(chatInput).toContain('harnessProfilePolicy.userConfigurable ? (');
  });

  it('carries the Harness gear and the model pair inside the capsule', () => {
    const chatInput = readLocalFile('ChatInput.tsx');
    const stylesheet = readChatInputStylesheet();

    expect(chatInput).toContain('<HarnessProfileSelector');
    expect(chatInput).not.toContain('harnessControl');
    // Reasoning belongs beside the model it configures, so the capsule no
    // longer hides it; only the context percentage stays on the status track.
    expect(stylesheet).not.toContain('.bitfun-reasoning-preset-selector,');
    expect(stylesheet).toContain('.bitfun-model-selector__ctx-usage {');
    expect(stylesheet).toContain('.bitfun-reasoning-preset-selector__trigger {');
    expect(stylesheet).toContain('padding-bottom: 30px;');
  });

  it('expresses Harness intensity by shape so the gauge survives a narrow composer', () => {
    const component = readLocalFile('HarnessProfileSelector.tsx');
    const stylesheet = readLocalFile('HarnessProfileSelector.scss');

    expect(component).toMatch(/minimal: Scan,[\s\S]*?balanced: Grid2X2,[\s\S]*?ultimate: Grid3X3,/);
    expect(component).toContain(
      'data-harness-density={densityProfile ? PROFILE_GEARS[densityProfile] : 0}',
    );
    expect(component).toContain('className="bitfun-harness-selector__density-core"');
    expect(stylesheet).toMatch(
      /@media \(max-width: 560px\)[\s\S]*?__trigger-value \{\n {4}display: none;/,
    );
  });

  it('states thinking strength with a compact concentric mark so the pill cannot widen', () => {
    const component = readLocalFile('ReasoningPresetSelector.tsx');
    const stylesheet = readLocalFile('ReasoningPresetSelector.scss');

    expect(component).toContain('__status-meter');
    // A word beside the concentric mark only repeats it, and it does so where the
    // capsule has the least width to spare. The level stays in the name.
    expect(component).not.toContain('__label');
    expect(component).toContain('aria-label={tooltip}');
    expect(stylesheet).toMatch(/&__trigger \{[\s\S]*?width: 18px;[\s\S]*?height: 18px;/);
    expect(stylesheet).not.toContain('max-width: 116px;');
    expect(readChatInputStylesheet()).toMatch(
      /\.bitfun-reasoning-preset-selector__trigger \{\n {10}width: 22px;\n {10}height: 22px;/,
    );
  });

  it('lifts the permission risk ramp onto the trigger, not just the menu rows', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    const riskRamp = stylesheet.slice(
      stylesheet.indexOf('&__permission-overview-icon {'),
      stylesheet.indexOf('&__permission-label {'),
    );
    expect(riskRamp).toContain('permission-trigger--ask &');
    expect(riskRamp).toContain('var(--bf-appearance-token-color-success)');
    expect(riskRamp).toContain('permission-trigger--auto &');
    expect(riskRamp).toContain('var(--bf-appearance-token-color-warning)');
    expect(riskRamp).toContain('permission-trigger--full_access &');
    expect(riskRamp).toContain('var(--bf-appearance-token-color-error)');
    // Full access keeps a body of its own so the risk survives the label being
    // dropped on a narrow composer.
    expect(stylesheet).toMatch(
      /&__permission-trigger \{[\s\S]*?&--full_access \{[\s\S]*?color-error\) 10%/,
    );
  });

  it('degrades prose before labels and never drops state color or a number', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    // Labels go on a narrow track…
    expect(stylesheet).toMatch(
      /@media \(max-width: 560px\)[\s\S]*?__permission-label \{\n {6}display: none;/,
    );
    expect(stylesheet).toMatch(
      /@media \(max-width: 460px\)[\s\S]*?__goal-btn\.icon-btn span \{\n {6}display: none;/,
    );
    // …while the goal's tone, the permission shield, and the context ring stay.
    expect(stylesheet).not.toMatch(/@media[\s\S]*?__usage-ring \{\n {6}display: none;/);
    expect(stylesheet).not.toMatch(/@media[\s\S]*?__permission-overview-icon \{\n {6}display: none;/);
  });

  it('gives every goal state a tone of its own so a stuck goal is not read as none', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toContain('&__goal-btn--active.icon-btn');
    expect(stylesheet).toContain('&__goal-btn--paused.icon-btn');
    expect(stylesheet).toContain('&__goal-btn--blocked.icon-btn');
    expect(stylesheet).toContain('&__goal-btn--complete.icon-btn');
  });

  it('answers a blocked turn from the composer stack, not from over the transcript', () => {
    const chatInput = readLocalFile('ChatInput.tsx');
    const container = readLocalFile('modern/ModernFlowChatContainer.tsx');
    const band = readLocalFile('ChatInputApprovalBand.scss');

    expect(chatInput).toContain('<ChatInputApprovalBand');
    // The panel used to be positioned by measuring the composer's height, which
    // put it on top of the output the reader needed in order to decide.
    expect(container).not.toContain('PermissionRequestPanel');
    expect(container).not.toContain('permissionPanelAboveChatInput');
    expect(band).not.toContain('position: absolute');
    expect(band).not.toContain('position: fixed');
  });

  it('reuses the composer as the rejection reason instead of carrying a second field', () => {
    const band = readLocalFile('ChatInputApprovalBand.tsx');

    expect(band).not.toContain('<textarea');
    expect(band).toContain('rejectReason');
    expect(band).toContain('onRejectReasonConsumed');
    // A half-typed next message must not silently become a reason, so the
    // reason is a separate answer rather than a modifier on rejecting.
    expect(band).toContain('data-testid="chat-input-approval-reject-with-reason"');
    expect(band).toMatch(/reply === 'reject' && withReason && reason/);
  });

  it('keeps the local target breadcrumb visible while Git gates mutating controls', () => {
    const component = readWorkspaceStripComponent();

    expect(component).toContain(
      "import { DispatchTargetPicker } from '@/features/dispatch/DispatchTargetPicker';",
    );
    expect(component).toContain(
      'const isGitWorkspace = isRepository || isWorktree || worktreeEnabled;',
    );
    expect(component).toContain('const showWorktreeToggle = !!worktreeControl && isGitWorkspace;');
    expect(component).toContain('const showDispatchPicker = !!dispatchControl;');
    expect(component).toContain(
      'const dispatchPickerLocked = !!dispatchControl && (dispatchControl.locked || !isGitWorkspace);',
    );
    expect(component).toContain('locked={dispatchPickerLocked}');
  });
});
