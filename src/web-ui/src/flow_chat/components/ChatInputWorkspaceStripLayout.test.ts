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

  it('keeps passive context aligned and promotes consequential controls', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    // One role for the whole track, declared once per species. The rail sets
    // the meta size — a quiet footnote under the capsule above — and its
    // facts inherit it; every control restates the same step through the
    // shared mixin, so the row reads as a single hushed line.
    expect(stylesheet).toContain(
      'font-size: flow-type.$meta-size;\n  line-height:',
    );
    expect(stylesheet).toMatch(/&__workspace \{[\s\S]*?font-size: inherit;/);
    expect(stylesheet).toMatch(/&__branch \{[\s\S]*?font-size: inherit;/);
    expect(stylesheet).toMatch(
      /@mixin strip-control \{[\s\S]*?font-size: flow-type\.\$meta-size;/,
    );
    // No control names a size of its own — that is how the track drifted into
    // four of them before.
    const rails = stylesheet.slice(0, stylesheet.indexOf('  &__permission-option-row {'));
    expect(rails.match(/font-size:/g)?.length).toBe(2);
    // The rails end where the permission popover begins; that menu is its own
    // surface and is allowed a denser scale than the track.
    expect(rails).not.toContain('$micro-size');
  });

  it('gives the track one pill shape, with a hover fill only on live controls', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    // One shape, declared once. Four heights, three radii and two hover fills
    // across five neighbouring controls is what "unified" is the fix for.
    const mixin = stylesheet.slice(
      stylesheet.indexOf('@mixin strip-control {'),
      stylesheet.indexOf('.bitfun-chat-input-workspace-strip {'),
    );
    expect(mixin).toContain('height: 18px;');
    expect(mixin).toContain('border-radius: 999px;');
    expect(mixin).toContain('background: transparent;');
    expect(mixin).toMatch(/> svg \{[\s\S]*?width: 12px;/);

    for (const control of [
      '    .dispatch-target-picker__trigger {\n      @include strip-control;',
      '  &__dispatch-result {\n    @include strip-control;',
      '  &__permission-trigger {\n    @include strip-control;',
      '  &__usage-btn {\n    @include strip-control;',
      '  &__worktree-toggle {\n    @include strip-control;',
    ]) {
      expect(stylesheet).toContain(control);
    }

    // The workspace is the row's subject and, with more than one open, its
    // switcher too. It cannot take the mixin — it must shrink on narrow
    // rails, which the mixin's `flex: none` forbids — so it restates the same
    // pill, and the `--switchable` modifier alone owns the hover fill; the
    // static form stays inert beside it at exactly the same size.
    expect(stylesheet).toMatch(/&__workspace \{[\s\S]*?height: 18px;/);
    expect(stylesheet).toMatch(/&__workspace \{[\s\S]*?padding: 0 7px;/);
    expect(stylesheet).toMatch(/&__workspace \{[\s\S]*?border-radius: 999px;/);
    expect(stylesheet).toMatch(/&__workspace \{[\s\S]*?cursor: default;/);
    expect(stylesheet).toMatch(
      /&__workspace--switchable \{[\s\S]*?&:hover:not\(:disabled\)[\s\S]*?background: var\(--bf-appearance-token-element-bg-soft\);/,
    );

    // Branch wears the same pill so the three left-rail segments keep one
    // rhythm, but it is still a fact — the picker and its uncommitted-changes
    // guard are a follow-up — so nothing on it offers a click it cannot
    // answer yet.
    const chip = stylesheet.slice(
      stylesheet.indexOf('  &__chip {'),
      stylesheet.indexOf('  // Isolation switch.'),
    );
    expect(chip).toContain('cursor: default;');
    expect(chip).not.toContain(':hover');
    expect(chip).toMatch(/&--branch \{[\s\S]*?height: 18px;/);
    expect(chip).toMatch(/&--branch \{[\s\S]*?padding: 0 7px;/);
    expect(chip).toMatch(/&--branch \{[\s\S]*?border-radius: 999px;/);

    // A hairline parts the segments; inside a segment spacing is the only
    // grouping, so the two gaps are named and must stay far enough apart to
    // read as a ratio.
    expect(stylesheet).toContain('$track-part-gap: 4px;');
    expect(stylesheet).toContain('$track-item-gap: 10px;');
    expect(stylesheet).toMatch(/&__context \{[\s\S]*?gap: \$track-item-gap;/);
    expect(stylesheet).toMatch(/&__next \{[\s\S]*?gap: \$track-item-gap;/);
    expect(stylesheet).toMatch(/&__chip \{[\s\S]*?gap: \$track-part-gap;/);
    expect(mixin).toContain('gap: $track-part-gap;');

    // Workspace and branch are one coordinate held at the inner gap, so they
    // read as one phrase rather than as two adjacent facts.
    expect(stylesheet).toMatch(/&__location \{[\s\S]*?gap: \$track-part-gap;/);
  });

  it('lines the track up with the capsule it belongs to, ink to ink', () => {
    const stylesheet = readWorkspaceStripStylesheet();
    const chatInput = readChatInputStylesheet();

    // The two rows share an origin, so the track can only look attached if it
    // accounts for the capsule's border and pad — and for the transparent
    // padding its own outermost items carry.
    const capsule = chatInput.slice(
      chatInput.indexOf('    // Capsule (single-line) mode appearance'),
      chatInput.indexOf('    &--multi-line {'),
    );
    expect(capsule).toContain('padding: 0 8px;');
    expect(capsule).toContain('border: 1px solid');
    // …and the drop zone's own inset, the third term in the 17px.
    expect(chatInput).toMatch(
      /\.bitfun-chat-input-drop-zone \{[\s\S]*?padding: 0 \$size-gap-2;/,
    );
    expect(stylesheet).toContain('$track-edge: 17px;');
    expect(stylesheet).toContain('$track-pad-left: $track-edge - 7px;');
    expect(stylesheet).toContain('$track-pad-right: $track-edge - 3px;');
    expect(stylesheet).toContain('padding: 0 $track-pad-right 0 $track-pad-left;');
  });

  it('orders the left rail as situation and the right rail as the next turn', () => {
    const component = readWorkspaceStripComponent();
    const stylesheet = readWorkspaceStripStylesheet();
    const contextIndex = component.indexOf('data-bf-part="context"');
    const nextIndex = component.indexOf('data-bf-part="next"');

    expect(contextIndex).toBeGreaterThan(-1);
    expect(nextIndex).toBeGreaterThan(contextIndex);
    // Situation: destination, workspace, branch, and whether it is isolated.
    expect(component).toContain('<DispatchTargetPicker');
    expect(component).toContain('__location');
    expect(component).toContain('__chip--branch');
    expect(component).toContain('data-testid="chat-input-worktree-toggle"');
    // The workspace doubles as the rail's switcher once more than one is
    // open; the menu lists every open workspace and marks the active one.
    expect(component).toContain('data-testid="chat-input-workspace-trigger"');
    expect(component).toContain('data-testid="chat-input-workspace-menu"');
    expect(component).toContain('data-bf-part="workspaceOption"');
    // Segments part on a hairline rule, never on a slash: a slash claimed a
    // path that a host, a workspace and a branch do not form.
    expect(component).toContain('__divider');
    expect(stylesheet).toMatch(/&__divider \{[\s\S]*?width: 1px;/);
    expect(component).not.toContain('breadcrumb-separator');
    expect(component).not.toContain('>/</span>');
    // Next turn: how much confirmation it asks for, and how much room is left.
    expect(component).toContain('data-testid="chat-input-permission-trigger"');
    // The ring is the whole reading. A number beside it only said the same
    // thing twice, in the rail with the least room to say anything.
    expect(component).not.toContain('{usagePercentage}%');
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
    // longer hides it; the context reading stays on the status track, as a
    // ring rather than a number.
    expect(stylesheet).not.toContain('.bitfun-reasoning-preset-selector,');
    expect(stylesheet).toContain('.bitfun-model-selector__ctx-usage {');
    expect(stylesheet).toContain('.bitfun-reasoning-preset-selector__trigger {');
  });

  it('centres the status track in the band the capsule reserves below it', () => {
    const chatInput = readChatInputStylesheet();
    const stylesheet = readWorkspaceStripStylesheet();

    // 36px band = 10px of air + the 20px track + a 6px offset, and the drop
    // zone floats 4px off the window edge, so the track keeps the same air
    // below (6 + 4) as above (10). Change any one of the four numbers and the
    // row drifts off-centre.
    expect(chatInput).toContain('padding-bottom: 36px;');
    expect(chatInput).toMatch(
      /\.bitfun-chat-input-drop-zone \{[\s\S]*?bottom: 4px;/,
    );
    expect(stylesheet).toContain('min-height: 20px;');
    expect(stylesheet).toContain('bottom: 6px;');
  });

  it('keeps the model pair borderless at rest and reveals its boundary on interaction', () => {
    const stylesheet = readChatInputStylesheet();

    expect(stylesheet).toMatch(
      /\.bitfun-model-selector \{[\s\S]*?border: 1px solid transparent;[\s\S]*?&:hover,[\s\S]*?border-color: var\(--bf-appearance-token-border-medium\);/,
    );
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
      /@media \(max-width: 460px\)[\s\S]*?__worktree-label \{\n {6}display: none;/,
    );
    // …while the isolation checkbox, the permission shield, and the context
    // ring stay.
    expect(stylesheet).not.toMatch(/@media[\s\S]*?__worktree-toggle \{\n {6}display: none;/);
    expect(stylesheet).not.toMatch(/@media[\s\S]*?__usage-ring \{\n {6}display: none;/);
    expect(stylesheet).not.toMatch(/@media[\s\S]*?__permission-overview-icon \{\n {6}display: none;/);
  });

  it('states worktree isolation as a checkbox rather than hiding it on the branch', () => {
    const component = readWorkspaceStripComponent();
    const stylesheet = readWorkspaceStripStylesheet();

    // The branch is a fact and the isolation is a control. Folding the switch
    // into the branch label left the composer with no visible way to say the
    // session can run somewhere else.
    expect(component).toContain('role="switch"');
    expect(component).toContain('__worktree-toggle');
    expect(component).not.toContain('__chip--branch-toggle');
    expect(stylesheet).toMatch(/&__worktree-toggle \{\n\s*@include strip-control;/);
    // On/off is a colour, not an outline: one bordered item in a borderless
    // row reads as an error state rather than a mode.
    expect(stylesheet).toMatch(
      /&--on \{\n\s*color: var\(--bf-appearance-token-color-accent-500\);/,
    );
    expect(stylesheet).not.toMatch(/&__worktree-toggle \{[\s\S]*?border: 1px/);
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
    expect(component).toContain('<DispatchTargetPicker');
    // One Git probe decides both controls, so they can never disagree about
    // whether the workspace is a repository. A repository Git refuses to read
    // for ownership reasons counts: `isRepository` only turns true after a
    // status call that rejection blocks, and hiding the controls there would
    // hide the very state the user has to act on.
    expect(component).toContain(
      'const isGitWorkspace = isRepository || repositoryTrustRequired || isWorktree || worktreeEnabled;',
    );
    expect(component).toContain('const showWorktreeToggle = !!worktreeControl && isGitWorkspace;');
    expect(component).toContain('const showDispatchPicker = !!dispatchControl;');
    expect(component).toContain(
      'const dispatchPickerLocked = !!dispatchControl && (dispatchControl.locked || !isGitWorkspace);',
    );
    expect(component).toContain('locked={dispatchPickerLocked}');
  });
});
