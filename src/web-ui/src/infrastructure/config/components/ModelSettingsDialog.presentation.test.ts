import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const source = readFileSync(
  fileURLToPath(new URL('./ModelSettingsPage.tsx', import.meta.url)),
  'utf8',
);
const styles = readFileSync(
  fileURLToPath(new URL('./ModelSettingsPage.scss', import.meta.url)),
  'utf8',
);

describe('ModelSettingsPage dialog presentation', () => {
  it('keeps model configuration editors at a moderate responsive size', () => {
    const editorDialogStart = source.indexOf('open={isEditing && !!editingConfig}');
    const editorDialog = source.slice(editorDialogStart, source.indexOf('<ConfirmDialog', editorDialogStart));
    const editingFormStart = source.indexOf('const renderEditingForm = () => {');
    const editingForm = source.slice(
      editingFormStart,
      source.indexOf('const renderModelCollectionItem', editingFormStart),
    );

    expect(editorDialogStart).toBeGreaterThan(-1);
    expect(editingFormStart).toBeGreaterThan(-1);
    expect(editorDialog).toContain('className="bitfun-model-settings__editor-dialog"');
    expect(editorDialog).toContain('size="xl"');
    expect(editorDialog).not.toContain('size="2xl"');
    expect(editorDialog).toContain('<DialogFooter appearance="floating">');
    expect(editorDialog).toContain(
      '<Button variant="secondary" onClick={requestCloseEditingModal} disabled={isEditorSaving}>',
    );
    expect(editorDialog).toContain('<DialogClose disabled={isEditorSaving} />');
    expect(editorDialog).toContain('loading={isEditorSaving}');
    expect(editingForm.match(/fieldSurface="default"/g)).toHaveLength(2);
    expect(editorDialog).not.toContain('bitfun-model-settings__editor-dialog-footer');
    expect(editorDialog).not.toContain('bitfun-model-settings__editor-dialog-cancel');
    expect(styles).toMatch(
      /&__editor-dialog\s*{[\s\S]*?max-block-size:\s*min\(\s*640px,\s*calc\(100vh - 2 \* var\(--bf-overlay-dialog-viewport-gutter\)\)\s*\);/,
    );
    expect(styles).not.toContain('&__editor-dialog-footer');
    expect(styles).not.toContain('&__editor-dialog-cancel');
    expect(styles).toMatch(/&__selected-model-row\s*{[\s\S]*?background:\s*var\(--bf-color-surface-raised\);/);
    expect(styles).toMatch(/&__reasoning-summary\s*{[\s\S]*?background:\s*var\(--bf-color-surface-tertiary\);/);
  });

  it('keeps unsaved editor state behind an explicit draft decision', () => {
    expect(source).toContain('if (editingModalHasUnsavedChanges) {');
    expect(source).toContain('setDraftCloseConfirmOpen(true);');
    expect(source).toContain('onConfirm={preserveEditingDraftAndClose}');
    expect(source).toContain('onSecondary={closeEditingModal}');
    expect(source).toContain("confirmText={t('draftClose.keepAndClose')}");
    expect(source).toContain("cancelText={t('draftClose.continueEditing')}");
    expect(source).toContain("statusMessage={t('draftClose.retainedHint')}");
  });

  it('protects a retained draft when another editor target is requested', () => {
    expect(source).toContain('pendingEditorOpenRef.current = { open };');
    expect(source).toContain('onConfirm={continueEditingCurrentDraft}');
    expect(source).toContain('onSecondary={discardDraftBeforeOpeningPendingEditor}');
  });
});
